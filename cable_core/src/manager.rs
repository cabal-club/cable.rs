use std::{
    collections::{HashMap, HashSet},
    convert::TryInto,
};

use async_std::{
    channel,
    prelude::*,
    sync::{Arc, RwLock},
    task,
};
use cable::{
    constants::{HASH_RESPONSE, NO_CIRCUIT, TEXT_POST},
    error::Error,
    message::{Message, MessageBody, MessageHeader, ResponseBody},
    post::{Post, PostBody, PostHeader},
    Hash, ReqId,
};
use desert::{FromBytes, ToBytes};
use futures::io::{AsyncRead, AsyncWrite};
use length_prefixed_stream::{decode_with_options, DecodeOptions};

use crate::{
    store::{GetPostOptions, Store},
    stream::PostStream,
    ChannelOptions,
};

pub type PeerId = usize;

/// The manager for a single cable instance.
#[derive(Clone)]
pub struct CableManager<S: Store> {
    /// A cable store.
    pub store: S,
    peers: Arc<RwLock<HashMap<PeerId, channel::Sender<Message>>>>,
    last_peer_id: Arc<RwLock<PeerId>>,
    last_req_id: Arc<RwLock<u32>>,
    listening: Arc<RwLock<HashMap<PeerId, Vec<(ReqId, ChannelOptions)>>>>,
    requested: Arc<RwLock<HashSet<Hash>>>,
    open_requests: Arc<RwLock<HashMap<u32, Message>>>,
}

impl<S> CableManager<S>
where
    S: Store,
{
    pub fn new(store: S) -> Self {
        Self {
            store,
            peers: Arc::new(RwLock::new(HashMap::new())),
            last_peer_id: Arc::new(RwLock::new(0)),
            last_req_id: Arc::new(RwLock::new(0)),
            listening: Arc::new(RwLock::new(HashMap::new())),
            requested: Arc::new(RwLock::new(HashSet::new())),
            open_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<S> CableManager<S>
where
    S: Store,
{
    /// Publish a new text post.
    pub async fn post_text<T: Into<String>, U: Into<String>>(
        &mut self,
        channel: T,
        text: U,
    ) -> Result<(), Error> {
        let public_key = self.get_public_key().await?;
        let signature = [0; 64];
        let links = self.get_links(channel).await?;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let channel = channel.into();
        let text = text.into();

        // Construct a new text post.
        let post = Post::text(public_key, signature, links, timestamp, channel, text);

        self.post(post).await
    }

    /// Publish a post.
    pub async fn post(&mut self, mut post: Post) -> Result<(), Error> {
        // Sign the post if required.
        if !post.is_signed() {
            post.sign(&self.get_secret_key().await?)?;
        }

        // Insert the post into the local store.
        self.store.insert_post(&post).await?;

        // Iterate over all peers and requests to whom we are listening.
        for (peer_id, reqs) in self.listening.read().await.iter() {
            // Iterate over peer requests.
            for (req_id, opts) in reqs {
                let n_limit = opts.limit.min(4096);
                let mut hashes = vec![];
                {
                    // Get all posts matching the request parameters.
                    let mut stream = self.store.get_post_hashes(&opts).await?;
                    while let Some(result) = stream.next().await {
                        hashes.push(result?);
                        // Break once the request limit has been reached.
                        if hashes.len() >= n_limit {
                            break;
                        }
                    }
                }

                // Construct a new hash response message.
                let response = Message::hash_response(NO_CIRCUIT, *req_id, hashes);

                // Send the response to the peer.
                self.send(*peer_id, &response).await?;
            }
        }

        Ok(())
    }

    /// Broadcast a message to all peers.
    pub async fn broadcast(&self, message: &Message) -> Result<(), Error> {
        for ch in self.peers.read().await.values() {
            ch.send(message.clone()).await?;
        }
        Ok(())
    }

    /// Send a message to a single peer identified by the given peer ID.
    pub async fn send(&self, peer_id: usize, msg: &Message) -> Result<(), Error> {
        if let Some(ch) = self.peers.read().await.get(&peer_id) {
            ch.send(msg.clone()).await?;
        }
        Ok(())
    }

    pub async fn handle(&mut self, peer_id: usize, msg: &Message) -> Result<(), Error> {
        // todo: forward requests
        match msg {
            Message::ChannelTimeRangeRequest {
                req_id,
                channel,
                time_start,
                time_end,
                limit,
                ..
            } => {
                let opts = GetPostOptions {
                    channel: channel.to_vec(),
                    time_start: *time_start,
                    time_end: *time_end,
                    limit: *limit,
                };
                let n_limit = (*limit).min(4096);
                let mut hashes = vec![];
                {
                    let mut stream = self.store.get_post_hashes(&opts).await?;
                    while let Some(result) = stream.next().await {
                        hashes.push(result?);
                        if hashes.len() >= n_limit {
                            break;
                        }
                    }
                }
                let response = Message::HashResponse {
                    req_id: *req_id,
                    hashes,
                };
                {
                    let mut w = self.listening.write().await;
                    if let Some(listeners) = w.get_mut(&peer_id) {
                        listeners.push((req_id.clone(), opts));
                    } else {
                        w.insert(peer_id, vec![(req_id.clone(), opts)]);
                    }
                }
                self.send(peer_id, &response).await?;
            }
            Message::HashResponse { req_id, hashes } => {
                let want = self.store.want(hashes).await?;
                if !want.is_empty() {
                    {
                        let mut mreq = self.requested.write().await;
                        for hash in &want {
                            mreq.insert(hash.clone());
                        }
                    }
                    let hreq = Message::HashRequest {
                        req_id: *req_id,
                        ttl: 1,
                        hashes: want,
                    };
                    self.send(peer_id, &hreq).await?;
                }
            }
            Message::HashRequest {
                req_id,
                ttl: _,
                hashes,
            } => {
                let response = Message::DataResponse {
                    req_id: *req_id,
                    data: self.store.get_data(hashes).await?,
                };
                self.send(peer_id, &response).await?
            }
            Message::DataResponse { req_id: _, data } => {
                for buf in data {
                    if !Post::verify(&buf) {
                        continue;
                    }
                    let (s, post) = Post::from_bytes(&buf)?;
                    if s != buf.len() {
                        continue;
                    }
                    let h = post.hash()?;
                    {
                        let mut mreq = self.requested.write().await;
                        if !mreq.contains(&h) {
                            continue;
                        } // didn't request this response
                        mreq.remove(&h);
                    }
                    self.store.insert_post(&post).await?;
                }
            }
            _ => {
                //println!["other message type: todo"];
            }
        }
        Ok(())
    }

    /// Generate a new request ID.
    async fn new_req_id(&self) -> Result<(u32, ReqId), Error> {
        let mut last_req_id = self.last_req_id.write().await;

        // Reset request ID to 0 if the maximum u32 has been reached.
        // Otherwise, increment the last request ID by one.
        *last_req_id = if *last_req_id == u32::MAX {
            0
        } else {
            *last_req_id + 1
        };

        let req_id = *last_req_id;

        Ok((req_id, req_id.to_bytes()?.try_into().unwrap()))
    }

    /// Generate a new peer ID.
    async fn new_peer_id(&self) -> Result<usize, Error> {
        let mut last_peer_id = self.last_peer_id.write().await;

        // Increment the last peer ID.
        *last_peer_id += 1;
        let peer_id = *last_peer_id;

        Ok(peer_id)
    }

    pub async fn open_channel(
        &mut self,
        options: &ChannelOptions,
    ) -> Result<PostStream<'_>, Error> {
        let (req_id, req_id_bytes) = self.new_req_id().await?;

        let m = Message::ChannelTimeRangeRequest {
            req_id: req_id_bytes,
            ttl: 1,
            channel: options.channel,
            time_start: options.time_start,
            time_end: options.time_end,
            limit: options.limit,
        };

        self.open_requests.write().await.insert(req_id, m.clone());
        self.broadcast(&m).await?;

        Ok(self.store.get_posts_live(options).await?)
    }

    pub async fn close_channel(&self, _channel: &[u8]) {
        unimplemented![]
    }

    pub async fn get_peer_ids(&self) -> Vec<usize> {
        self.peers
            .read()
            .await
            .keys()
            .copied()
            .collect::<Vec<usize>>()
    }

    // TODO: Convert to `get_links()`?
    pub async fn get_link(&mut self, channel: &[u8]) -> Result<[u8; 32], Error> {
        let link = self.store.get_latest_hash(channel).await?;
        Ok(link)
    }

    /// Retrieve the public key of the local peer.
    pub async fn get_public_key(&mut self) -> Result<[u8; 32], Error> {
        let (pk, _sk) = self.store.get_or_create_keypair().await?;
        Ok(pk)
    }

    /// Retrieve the secret key of the local peer.
    pub async fn get_secret_key(&mut self) -> Result<[u8; 64], Error> {
        let (_pk, sk) = self.store.get_or_create_keypair().await?;
        Ok(sk)
    }

    pub async fn listen<T>(&self, mut stream: T) -> Result<(), Error>
    where
        T: AsyncRead + AsyncWrite + Clone + Unpin + Send + Sync + 'static,
    {
        // Generate a new peer ID.
        let peer_id = self.new_peer_id().await?;

        // Create a bounded message channel.
        let (send, recv) = channel::bounded(100);

        // Insert the peer ID and channel sender into the list of peers.
        self.peers.write().await.insert(peer_id, send);

        // Write all open request messages to the stream.
        for msg in self.open_requests.read().await.values() {
            stream.write_all(&msg.to_bytes()?).await?;
        }

        let write_to_stream_res = {
            let mut stream_c = stream.clone();

            task::spawn(async move {
                // Listen for incoming messages (local).
                while let Ok(msg) = recv.recv().await {
                    // Write the message to the stream.
                    stream_c.write_all(&msg.to_bytes()?).await?;
                }

                Ok(())
            })
        };

        // Define the stream decoder parameters.
        let mut options = DecodeOptions::default();
        options.include_len = true;

        let mut length_prefixed_stream = decode_with_options(stream, options);

        // Iterate over the stream.
        while let Some(read_buf) = length_prefixed_stream.next().await {
            let buf = read_buf?;

            // Deserialize the received message.
            let (_, msg) = Message::from_bytes(&buf)?;

            let mut this = self.clone();
            task::spawn(async move {
                // Handle the received message.
                if let Err(_e) = this.handle(peer_id, &msg).await {
                    // TODO: Report the error.
                    //eprintln!["{}", e];
                }
            });
        }

        write_to_stream_res.await?;
        self.peers.write().await.remove(&peer_id);

        Ok(())
    }
}
