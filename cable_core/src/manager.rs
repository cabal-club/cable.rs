//! The cable manager module is responsible for tracking peer interactions,
//! handling request and response messages and querying and updating the store.
//! It is intended to serve as the main entrypoint for running a cable peer.

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
    constants::NO_CIRCUIT,
    message::{Message, MessageBody, MessageHeader, RequestBody, ResponseBody},
    Channel, ChannelOptions, Error, Hash, Post, ReqId, Timestamp, UserInfo,
};
use desert::{FromBytes, ToBytes};
use futures::io::{AsyncRead, AsyncWrite};
use length_prefixed_stream::{decode_with_options, DecodeOptions};
use log::debug;

use crate::{store::Store, stream::PostStream};

// Define the TTL (how many times a request will be
// forwarded.
//
// NOTE: We may want to set this dynamically in the
// future, either based on user choice or connectivity
// status.
const TTL: u8 = 0;

/// A locally-defined peer ID used to track requests.
pub type PeerId = usize;

/// A `HashMap` of peer requests with a key of peer ID and a value of a `Vec`
/// of request ID and channel options.
pub type PeerRequestMap = HashMap<PeerId, Vec<(ReqId, ChannelOptions)>>;

/// The origin of a request.
enum RequestOrigin {
    /// Local request.
    Local,
    /// Remote request (from a peer).
    Remote,
}

impl RequestOrigin {
    fn is_local(&self) -> bool {
        match self {
            RequestOrigin::Local => true,
            RequestOrigin::Remote => false,
        }
    }
}

/// The manager for a single cable instance.
#[derive(Clone)]
pub struct CableManager<S: Store> {
    /// A cable store.
    pub store: S,
    /// Peers with whom communication is underway.
    peers: Arc<RwLock<HashMap<PeerId, channel::Sender<Message>>>>,
    /// The most recently assigned peer ID.
    last_peer_id: Arc<RwLock<PeerId>>,
    /// The most recently assigned request ID.
    last_req_id: Arc<RwLock<u32>>,
    /// Live inbound requests to which the local peer is listening and
    /// responding.
    ///
    /// These are peer-generated channel time range requests with an end time
    /// of 0, indicating that the peer wishes to receive new post hashes as they
    /// become known.
    live_requests: Arc<RwLock<PeerRequestMap>>,
    /// Hashes of posts which have been requested from remote peers by the
    /// local peer.
    requested: Arc<RwLock<HashSet<Hash>>>,
    /// Active outbound requests (includes requests of local and remote origin).
    outbound_requests: Arc<RwLock<HashMap<ReqId, (RequestOrigin, Message)>>>,
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
            live_requests: Arc::new(RwLock::new(HashMap::new())),
            requested: Arc::new(RwLock::new(HashSet::new())),
            outbound_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<S> CableManager<S>
where
    S: Store,
{
    /// Post header value generator.
    async fn post_header_values(
        &mut self,
        channel: &Channel,
    ) -> Result<([u8; 32], Vec<Hash>, Timestamp), Error> {
        let public_key = self.get_public_key().await?;
        let links = if let Some(links) = self.get_links(channel).await {
            links
        } else {
            vec![]
        };
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        Ok((public_key, links, timestamp))
    }

    /// Publish a new text post.
    pub async fn post_text<T: Into<String>, U: Into<String>>(
        &mut self,
        channel: T,
        text: U,
    ) -> Result<(), Error> {
        debug!("Posting text post...");

        let channel = channel.into();
        let (public_key, links, timestamp) = self.post_header_values(&channel).await?;
        let text = text.into();

        // Construct a new text post.
        let post = Post::text(public_key, links, timestamp, channel, text);

        self.post(post).await
    }

    /// Publish a new delete post with the given post hashes.
    pub async fn post_delete(&mut self, hashes: Vec<Hash>) -> Result<(), Error> {
        let public_key = self.get_public_key().await?;
        let links = vec![];
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        // Construct a new delete post.
        let post = Post::delete(public_key, links, timestamp, hashes);

        self.post(post).await
    }

    /// Publish a new info post with the given name.
    pub async fn post_info_name(&mut self, username: &str) -> Result<(), Error> {
        let public_key = self.get_public_key().await?;
        let links = vec![];
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        let name_info = UserInfo::name(username)?;

        // Construct a new info post.
        let post = Post::info(public_key, links, timestamp, vec![name_info]);

        self.post(post).await
    }

    /// Publish a new topic post for the given channel.
    pub async fn post_topic<T: Into<String>, U: Into<String>>(
        &mut self,
        channel: T,
        topic: U,
    ) -> Result<(), Error> {
        let channel = channel.into();
        let (public_key, links, timestamp) = self.post_header_values(&channel).await?;
        let topic = topic.into();

        // Construct a new topic post.
        let post = Post::topic(public_key, links, timestamp, channel, topic);

        self.post(post).await
    }

    /// Publish a new join post for the given channel.
    pub async fn post_join<T: Into<String>>(&mut self, channel: T) -> Result<(), Error> {
        let channel = channel.into();
        let (public_key, links, timestamp) = self.post_header_values(&channel).await?;

        // Construct a new join post.
        let post = Post::join(public_key, links, timestamp, channel);

        self.post(post).await
    }

    /// Publish a new leave post for the given channel.
    pub async fn post_leave<T: Into<String>>(&mut self, channel: T) -> Result<(), Error> {
        let channel = channel.into();
        let (public_key, links, timestamp) = self.post_header_values(&channel).await?;

        // Construct a new leave post.
        let post = Post::leave(public_key, links, timestamp, channel);

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

        // Send post hashes to all peers for whom we hold inbound requests.
        self.send_post_hashes().await?;

        Ok(())
    }

    /// Send post hashes matching peer request parameters for all live
    /// requests.
    async fn send_post_hashes(&mut self) -> Result<(), Error> {
        // Iterate over all live peer requests.
        for (peer_id, reqs) in self.live_requests.read().await.iter() {
            // Iterate over peer requests.
            for (req_id, opts) in reqs {
                let limit = opts.limit.min(4096);
                let mut hashes = vec![];

                {
                    // Get all post hashes matching the request parameters.
                    let mut stream = self.store.get_post_hashes(opts).await?;
                    while let Some(result) = stream.next().await {
                        hashes.push(result?);
                        // Break once the request limit has been reached.
                        if hashes.len() as u64 >= limit {
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

    /// Decrement the TTL of a request message and write it to the outbound
    /// requests store.
    async fn decrement_ttl_and_write_to_outbound(&self, req_id: ReqId, msg: &Message) {
        let mut request = msg.clone();
        request.decrement_ttl();

        self.outbound_requests
            .write()
            .await
            .insert(req_id, (RequestOrigin::Remote, request));
    }

    /// Handle a request or response message.
    pub async fn handle(&mut self, peer_id: usize, msg: &Message) -> Result<(), Error> {
        let MessageHeader {
            msg_type: _,
            circuit_id,
            req_id,
        } = msg.header;

        // TODO: Forward requests.
        match &msg.body {
            MessageBody::Request { ttl, body } => match body {
                RequestBody::Post { hashes } => {
                    // If the request TTL is > 0, decrement it and add the
                    // message to `outbound_requests` so that it will be
                    // forwarded to other connected peers.
                    //
                    // TODO: Set the TTL to 16 if it is > 16.
                    if *ttl > 0 {
                        self.decrement_ttl_and_write_to_outbound(req_id, msg).await;
                    }

                    let posts = self.store.get_post_payloads(hashes).await?;
                    let response = Message::post_response(circuit_id, req_id, posts);

                    self.send(peer_id, &response).await?
                }
                RequestBody::Cancel { cancel_id } => {
                    if *ttl > 0 {
                        self.decrement_ttl_and_write_to_outbound(req_id, msg).await;
                    }

                    // Remove the request from the list of outbound requests.
                    // The associated message will no longer be sent to peers.
                    self.outbound_requests.write().await.remove(cancel_id);
                }
                RequestBody::ChannelTimeRange {
                    channel,
                    time_start,
                    time_end,
                    limit,
                } => {
                    if *ttl > 0 {
                        self.decrement_ttl_and_write_to_outbound(req_id, msg).await;
                    }

                    let opts = ChannelOptions::new(channel, *time_start, *time_end, *limit);
                    let n_limit = (*limit).min(4096);

                    let mut hashes = vec![];
                    {
                        // Create a stream of post hashes matching the given criteria.
                        let mut stream = self.store.get_post_hashes(&opts).await?;
                        // Iterate over the hashes in the stream.
                        while let Some(result) = stream.next().await {
                            hashes.push(result?);
                            // Break out of the loop once the requested limit is met.
                            if hashes.len() as u64 >= n_limit {
                                break;
                            }
                        }
                    }

                    let response = Message::hash_response(circuit_id, req_id, hashes);

                    // Add the peer and request ID to the request tracker if
                    // the end time has been set to 0 (i.e. keep this request
                    // alive and send new messages as they become available).
                    if *time_end == 0 {
                        let mut live_requests = self.live_requests.write().await;
                        if let Some(peer_requests) = live_requests.get_mut(&peer_id) {
                            peer_requests.push((req_id, opts));
                        } else {
                            live_requests.insert(peer_id, vec![(req_id, opts)]);
                        }
                    }

                    self.send(peer_id, &response).await?;
                }
                RequestBody::ChannelState {
                    channel: _,
                    future: _,
                } => {
                    if *ttl > 0 {
                        self.decrement_ttl_and_write_to_outbound(req_id, msg).await;
                    }

                    /*
                    TODO: We will require channel state indexes before this
                    handler can be completed.

                    Channel state includes (spec section 5.4.4):

                    The latest post/info post of all members and ex-members.
                    The latest of all users' post/join or post/leave posts to the channel.
                    The latest post/topic post made to the channel.
                    */
                }
                RequestBody::ChannelList { skip, limit } => {
                    if *ttl > 0 {
                        self.decrement_ttl_and_write_to_outbound(req_id, msg).await;
                    }

                    let n_limit = (*limit).min(4096);

                    let mut all_channels = self.store.get_channels().await?;
                    // Drain the channels matching the given range.
                    let channels = all_channels
                        .drain(*skip as usize..n_limit as usize)
                        .collect();

                    let response = Message::channel_list_response(circuit_id, req_id, channels);

                    self.send(peer_id, &response).await?
                }
            },
            MessageBody::Response { body } => match body {
                // TODO: A responder MUST send a Hash Response message with
                // hash_count = 0 to indicate that they do not intend to return
                // any further hashes for the given req_id and they have
                // concluded the request on their side.
                ResponseBody::Hash { hashes } => {
                    let wanted_hashes = self.store.want(hashes).await?;
                    if !wanted_hashes.is_empty() {
                        // If a hash appears in our list of wanted hashed,
                        // send a request for the associated post.
                        let request = Message::post_request(
                            circuit_id,
                            req_id,
                            TTL,
                            wanted_hashes.to_owned(),
                        );

                        self.send(peer_id, &request).await?;

                        // Update the list of requested posts.
                        let mut requested_posts = self.requested.write().await;
                        for hash in &wanted_hashes {
                            requested_posts.insert(*hash);
                        }
                    }

                    // TODO: If hash_count == 0, remove the request.
                    // This may be more relevant when responding to a channel
                    // time range request (ie. sending a hash response).
                }
                ResponseBody::Post { posts } => {
                    // Iterate over the encoded posts.
                    for post_bytes in posts {
                        // Verify the post signature.
                        if !Post::verify(post_bytes) {
                            // Skip to the next post, bypassing the rest of the
                            // code in this `for` loop.
                            continue;
                        }

                        // Deserialize the post.
                        let (s, post) = Post::from_bytes(post_bytes)?;

                        // Ensure the number of processed bytes matches the
                        // received amount.
                        if s != post_bytes.len() {
                            continue;
                        }

                        let post_hash = post.hash()?;

                        let mut requested_posts = self.requested.write().await;
                        // Check if this post was previously requested.
                        if !requested_posts.contains(&post_hash) {
                            // Skip this post if it was not requested.
                            continue;
                        }
                        // Remove the post hash from the list of requested
                        // posts.
                        requested_posts.remove(&post_hash);

                        // TODO: Hand the post over to an indexer.
                        // The indexer will be responsible for matching on
                        // the post type, extracting key info and indexing it
                        // in the store.

                        self.store.insert_post(&post).await?;
                    }
                }
                ResponseBody::ChannelList { channels } => {
                    // TODO: Do we need to take action to conclude the request
                    // which resulted in this response?
                    self.store.insert_channels(channels).await?;
                }
            },
            // Ignore unrecognized message type.
            MessageBody::Unrecognized { .. } => (),
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

    /// Create a channel time range request matching the given channel
    /// parameters and broadcast it to all peers, listening for responses.
    pub async fn open_channel(
        &mut self,
        channel_opts: &ChannelOptions,
    ) -> Result<PostStream<'_>, Error> {
        let (_req_id, req_id_bytes) = self.new_req_id().await?;

        let request = Message::channel_time_range_request(
            NO_CIRCUIT,
            req_id_bytes,
            TTL,
            channel_opts.to_owned(),
        );

        self.outbound_requests
            .write()
            .await
            .insert(req_id_bytes, (RequestOrigin::Local, request.clone()));

        self.broadcast(&request).await?;

        self.store.get_posts_live(channel_opts).await
    }

    /// Create a cancel request for all active outbound channel time range
    /// requests originating locally and matching the given channel name.
    /// Broadcast the cancel request(s) to all peers.
    pub async fn close_channel(&self, channel: &String) -> Result<(), Error> {
        let close_channel = channel;

        let mut outbound_requests = self.outbound_requests.write().await;

        // Vector to hold the request IDs of all outbound channel time range
        // requests with channel names matching the given channel.
        let mut channel_req_ids = Vec::new();

        for (req_id, (request_origin, msg)) in outbound_requests.iter() {
            if let MessageBody::Request {
                body: RequestBody::ChannelTimeRange { channel, .. },
                ..
            } = &msg.body
            {
                // Ignore remotely-generated requests and non-matching channel
                // names.
                if request_origin.is_local() && channel == close_channel {
                    channel_req_ids.push(*req_id);
                }
            }
        }

        for channel_req_id in channel_req_ids {
            let (_req_id, req_id_bytes) = self.new_req_id().await?;

            let request = Message::cancel_request(NO_CIRCUIT, req_id_bytes, TTL, channel_req_id);

            // TODO: Do we really want to store a cancel request?
            self.outbound_requests
                .write()
                .await
                .insert(req_id_bytes, (RequestOrigin::Local, request.clone()));

            self.broadcast(&request).await?;

            outbound_requests.remove(&channel_req_id);
        }

        Ok(())
    }

    pub async fn get_peer_ids(&self) -> Vec<usize> {
        self.peers
            .read()
            .await
            .keys()
            .copied()
            .collect::<Vec<usize>>()
    }

    pub async fn get_links(&mut self, channel: &Channel) -> Option<Vec<Hash>> {
        self.store.get_latest_hashes(channel).await
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

    /// Listen for incoming peer messages and respond with locally-generated
    /// messages.
    ///
    /// Decode each received message and pass it off to the handler.
    pub async fn listen<T>(&self, mut stream: T) -> Result<(), Error>
    where
        T: AsyncRead + AsyncWrite + Clone + Unpin + Send + Sync + 'static,
    {
        debug!("Listening for incoming peer messages...");

        // Generate a new peer ID.
        let peer_id = self.new_peer_id().await?;

        // Create a bounded message channel.
        let (send, recv) = channel::bounded(100);

        // Insert the peer ID and channel sender into the list of peers.
        self.peers.write().await.insert(peer_id, send);

        // Write all outbound request messages to the stream, as long as the
        // message TTL is not 0. If the TTL is 0, remove it from the outbound
        // requests store.
        for (req_id, (_, msg)) in self.outbound_requests.read().await.iter() {
            if let MessageBody::Request { ttl, .. } = msg.body {
                if ttl == 0 {
                    // The TTL for this request has been exhausted.
                    self.outbound_requests.write().await.remove(req_id);
                } else {
                    stream.write_all(&msg.to_bytes()?).await?;
                }
            }
        }

        let write_to_stream_res = {
            let mut stream_c = stream.clone();

            task::spawn(async move {
                // Listen for incoming locally-generated messages.
                while let Ok(msg) = recv.recv().await {
                    debug!("Sent a message to the TCP stream: {}", msg);

                    // Write the message to the stream.
                    stream_c.write_all(&msg.to_bytes()?).await?;
                }

                // Type inference fails without binding concretely to `Result`.
                Result::<(), Error>::Ok(())
            })
        };

        // Define the stream decoder parameters.
        let options = DecodeOptions {
            include_len: true,
            ..Default::default()
        };

        let mut length_prefixed_stream = decode_with_options(stream, options);

        // Iterate over the stream.
        while let Some(read_buf) = length_prefixed_stream.next().await {
            let buf = read_buf?;

            // Deserialize the received message.
            let (_, msg) = Message::from_bytes(&buf)?;

            debug!("Received a message from the TCP stream: {}", msg);

            let mut this = self.clone();
            task::spawn(async move {
                // Handle the received message.
                if let Err(e) = this.handle(peer_id, &msg).await {
                    // TODO: Consider a better way to report.
                    eprintln!["{}", e];
                }
            });
        }

        // Continue reading and writing to the peer stream until the stream is
        // closed (either intentionally or because of an error).
        write_to_stream_res.await?;

        // Remove the peer from the list of active peers.
        self.peers.write().await.remove(&peer_id);

        Ok(())
    }
}

/*

// Test brainstorm:
//
// Create two instances of `CableManager`.
// Invoke `listen` for each with an async stream.
// Make some posts?
*/

#[cfg(test)]
mod test {
    use std::{cmp::min, pin::Pin};

    use async_std::{
        io::{Read, Write},
        prelude::*,
        task,
    };
    use cable::{
        constants::{HASH_RESPONSE, POST_RESPONSE},
        ChannelOptions, Error, Message,
    };
    use desert::FromBytes;
    use futures::{
        io::Error as IoError,
        task::{Context, Poll},
    };
    use length_prefixed_stream::{decode_with_options, DecodeOptions};

    use crate::{CableManager, MemoryStore};

    // Mock TCP stream implementation to facilitate testing of `CableManager`.
    //
    // Based on https://rust-lang.github.io/async-book/09_example/03_tests.html

    #[derive(Clone)]
    struct MockTcpStream {
        read_data: Vec<u8>,
        write_data: Vec<u8>,
    }

    impl Read for MockTcpStream {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut Context,
            buf: &mut [u8],
        ) -> Poll<Result<usize, IoError>> {
            let size: usize = min(self.read_data.len(), buf.len());
            buf[..size].copy_from_slice(&self.read_data[..size]);
            Poll::Ready(Ok(size))
        }
    }

    impl Write for MockTcpStream {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _: &mut Context,
            buf: &[u8],
        ) -> Poll<Result<usize, IoError>> {
            self.write_data = Vec::from(buf);

            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context) -> Poll<Result<(), IoError>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _: &mut Context) -> Poll<Result<(), IoError>> {
            Poll::Ready(Ok(()))
        }
    }

    impl Unpin for MockTcpStream {}

    #[async_std::test]
    async fn request_response() -> Result<(), Error> {
        let store = MemoryStore::default();
        let mut peer = CableManager::new(store);

        let stream = MockTcpStream {
            read_data: Vec::new(),
            write_data: Vec::new(),
        };
        let stream_clone = stream.clone();

        let channel = "default".to_string();
        let text = "meow?";

        peer.post_text(channel, text).await.unwrap();

        // Define the stream decoder parameters.
        let options = DecodeOptions {
            include_len: true,
            ..Default::default()
        };

        let mut length_prefixed_stream = decode_with_options(stream_clone, options);

        task::spawn(async move {
            peer.listen(stream).await.unwrap();
        });

        // Iterate over the stream.
        if let Some(read_buf) = length_prefixed_stream.next().await {
            let buf = read_buf?;

            // Deserialize the received message.
            let (_, msg) = Message::from_bytes(&buf)?;

            assert_eq!(msg.message_type(), 13)
        } else {
            panic!()
        }

        /*
        let store_one = MemoryStore::default();
        let store_two = MemoryStore::default();

        let mut peer_one = CableManager::new(store_one);
        let mut peer_two = CableManager::new(store_two);

        let stream_one = MockTcpStream {
            read_data: Vec::new(),
            write_data: Vec::new(),
        };
        let stream_two = stream_one.clone();

        let channel = "default".to_string();
        let text = "meow?";

        let opts = ChannelOptions {
            channel: channel.clone(),
            time_start: 0,
            time_end: 0,
            limit: 20,
        };
        let opts_c = opts.clone();

        peer_one.post_text(channel, text).await.unwrap();
        task::spawn(async move {
            peer_one.listen(stream_one).await.unwrap();
        });

        task::spawn(async move { peer_two.listen(stream_two).await.unwrap() });

        task::spawn(async move {
            let mut post_stream = peer_two.open_channel(&opts_c).await.unwrap();
            while let Some(post) = post_stream.next().await {
                if let Ok(p) = post {
                    // Ensure the received post(s) meet expectations.
                    assert_eq!(p.post_type(), TEXT_POST);
                }
            }
        });

        let mut peer_one_c = peer_one.clone();

        task::spawn(async move {
            let _msg_stream = peer_one_c.open_channel(&opts).await.unwrap();
            peer_one_c.post_text(channel, text).await.unwrap();
        });
        */

        //task::spawn(async move { peer_one.listen(stream_one).await.unwrap() });

        // Peer one: channel time range request for "default" channel.
        // - Use `open_channel()`
        // - Check the message(s) emitted from the stream and assert

        //peer_two.post_text(channel, text).await?;

        // Peer one: channel time range request for "default" channel.

        //task::spawn(async move { peer_two.listen(stream_two).await.unwrap() });

        Ok(())
    }
}
