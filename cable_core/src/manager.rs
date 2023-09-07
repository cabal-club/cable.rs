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
const TTL: u8 = 1;

/// A locally-defined peer ID used to track requests.
pub type PeerId = usize;

/// A `HashMap` of peer requests with a key of peer ID and a value of a `Vec`
/// of request ID and `LiveRequest`.
pub type PeerRequestMap = HashMap<PeerId, Vec<LiveRequest>>;

/// Inbound requests for which the keep-alive option has been selected.
///
/// This helps us to respond to live requests with new hashes as they become
/// known. The hashes must satisfy the given request parameters, either channel
/// name or channel options, in order to be sent.
#[derive(Debug, PartialEq)]
pub enum LiveRequest {
    /// A channel state request with specified ID and channel.
    ChannelState(ReqId, Channel),
    /// A channel time range request with specified ID and channel options.
    ChannelTimeRange(ReqId, ChannelOptions),
}

impl LiveRequest {
    fn req_id(&self) -> &ReqId {
        match self {
            LiveRequest::ChannelState(req_id, _channel) => req_id,
            LiveRequest::ChannelTimeRange(req_id, _channel_opts) => req_id,
        }
    }
}

/// The origin of a request.
#[derive(Debug)]
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

/// Generate a timestamp for the current time.
fn now() -> Result<u64, Error> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis()
        // Convert from u128 to u64.
        .try_into()?;

    Ok(timestamp)
}

/// The manager for a single cable instance.
#[derive(Clone)]
pub struct CableManager<S: Store> {
    /// Hashes of posts which remote peers have marked for deletion, or which
    /// have been authored and deleted by the local peer.
    deleted_posts: Arc<RwLock<HashSet<Hash>>>,
    /// Requests of remote origin which have been forwarded to other peers.
    forwarded_requests: Arc<RwLock<HashMap<ReqId, HashSet<PeerId>>>>,
    /// Request IDs of requests which have been handled.
    handled_requests: Arc<RwLock<HashSet<ReqId>>>,
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
    /// Active outbound requests (includes requests of local and remote origin).
    outbound_requests: Arc<RwLock<HashMap<ReqId, (RequestOrigin, Message)>>>,
    /// Peers with whom communication is underway.
    peers: Arc<RwLock<HashMap<PeerId, channel::Sender<Message>>>>,
    /// Hashes of posts which have been requested from remote peers by the
    /// local peer.
    requested_posts: Arc<RwLock<HashSet<Hash>>>,
    /// A cable store.
    pub store: S,
}

impl<S> CableManager<S>
where
    S: Store,
{
    pub fn new(store: S) -> Self {
        Self {
            deleted_posts: Arc::new(RwLock::new(HashSet::new())),
            forwarded_requests: Arc::new(RwLock::new(HashMap::new())),
            handled_requests: Arc::new(RwLock::new(HashSet::new())),
            last_peer_id: Arc::new(RwLock::new(0)),
            // Generate a random u32 on startup to reduce chance of collisions.
            last_req_id: Arc::new(RwLock::new(fastrand::u32(..))),
            live_requests: Arc::new(RwLock::new(HashMap::new())),
            outbound_requests: Arc::new(RwLock::new(HashMap::new())),
            peers: Arc::new(RwLock::new(HashMap::new())),
            requested_posts: Arc::new(RwLock::new(HashSet::new())),
            store,
        }
    }

    /// Retrieve the public key of the local peer.
    pub async fn get_public_key(&mut self) -> Result<[u8; 32], Error> {
        let (pk, _sk) = self.store.get_or_create_keypair().await;

        Ok(pk)
    }

    /// Retrieve the secret key of the local peer.
    pub async fn get_secret_key(&mut self) -> Result<[u8; 64], Error> {
        let (_pk, sk) = self.store.get_or_create_keypair().await;

        Ok(sk)
    }

    /// Generate a new request ID.
    pub async fn new_req_id(&self) -> Result<(u32, ReqId), Error> {
        let mut last_req_id = self.last_req_id.write().await;

        // Reset request ID to 0 if the maximum u32 has been reached.
        // Otherwise, increment the last request ID by one.
        *last_req_id = if *last_req_id == u32::MAX {
            0
        } else {
            *last_req_id + 1
        };

        let req_id = *last_req_id;
        debug!("Generated a new request ID: {}", req_id);

        Ok((req_id, req_id.to_bytes()?.try_into().unwrap()))
    }

    /// Generate a new peer ID.
    async fn new_peer_id(&self) -> Result<usize, Error> {
        let mut last_peer_id = self.last_peer_id.write().await;

        // Increment the last peer ID.
        *last_peer_id += 1;
        let peer_id = *last_peer_id;
        debug!("Generated a new peer ID: {}", peer_id);

        Ok(peer_id)
    }

    /// Create a channel time range request and a channel state request matching
    /// the given channel parameters and broadcast them to all peers, listening
    /// for responses.
    pub async fn open_channel(
        &mut self,
        channel_opts: &ChannelOptions,
    ) -> Result<PostStream<'_>, Error> {
        debug!("Opening {}", channel_opts);

        let channel = channel_opts.channel.to_owned();
        let future = 1;

        // Create and broadcast a channel time range request.
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

        // Create and broadcast a channel state request.
        let (_req_id, req_id_bytes) = self.new_req_id().await?;
        let request =
            Message::channel_state_request(NO_CIRCUIT, req_id_bytes, TTL, channel, future);
        self.outbound_requests
            .write()
            .await
            .insert(req_id_bytes, (RequestOrigin::Local, request.clone()));
        self.broadcast(&request).await?;

        Ok(self.store.get_posts_live(channel_opts).await)
    }

    /// Create a cancel request for all active outbound channel time range
    /// requests originating locally and matching the given channel name.
    /// Broadcast the cancel request(s) to all peers.
    pub async fn close_channel(&self, channel: &String) -> Result<(), Error> {
        debug!("Closing channel {}", channel);
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
            self.broadcast(&request).await?;
            outbound_requests.remove(&channel_req_id);
        }

        Ok(())
    }

    /// Listen for incoming peer messages and respond with locally-generated
    /// messages.
    ///
    /// Decode each received message and pass it off to the handler.
    pub async fn listen<T>(&self, stream: T) -> Result<(), Error>
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

        // Process and send outbound requests to the connected peer.
        self.process_and_send_outbound_requests(stream.clone(), peer_id)
            .await?;

        let write_to_stream_res = {
            let mut stream_c = stream.clone();

            task::spawn(async move {
                // Listen for incoming locally-generated messages.
                while let Ok(msg) = recv.recv().await {
                    let msg_bytes = &msg.to_bytes()?;

                    // Write the message to the stream.
                    stream_c.write_all(msg_bytes).await?;

                    debug!("Wrote a message to the TCP stream: {}", msg,);
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

            debug!("Received a message from the TCP stream: {}", msg,);

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

    /// Query if the request defined by the given peer ID and request ID is an
    /// active live request.
    async fn is_live_request(&mut self, peer_id: &PeerId, req_id: &ReqId) -> bool {
        let live_requests = self.live_requests.read().await;
        // Check if there are active live requests for the given peer ID.
        if let Some(peer_requests) = live_requests.get(peer_id) {
            // Iterate over the peer requests and report on the presence of the
            // given request ID.
            peer_requests
                .iter()
                .any(|live_request| live_request.req_id() == req_id)
        } else {
            false
        }
    }

    /// Remove the live request defined by the given peer ID and request ID.
    async fn remove_live_request(&mut self, peer_id: &PeerId, req_id: &ReqId) -> Result<(), Error> {
        // Remove the request from the map of live requests.
        let mut live_requests = self.live_requests.write().await;
        if let Some(peer_requests) = live_requests.get_mut(peer_id) {
            // Iterate over the peer requests and retain only the
            // requests for which the ID does not match the given
            // request ID.
            peer_requests.retain(|live_request| live_request.req_id() != req_id);
        }

        Ok(())
    }

    /// Process all outbound requests, sending each one to the connected
    /// peer if it meets certain requirements.
    ///
    /// This method takes into account the TTL of the request. It also ensures
    /// that cancel requests are forwarded to peers to whom the referenced
    /// request was previously sent.
    pub async fn process_and_send_outbound_requests<T>(
        &self,
        mut stream: T,
        peer_id: usize,
    ) -> Result<(), Error>
    where
        T: AsyncRead + AsyncWrite + Clone + Unpin + Send + Sync + 'static,
    {
        'requests: for (req_id, (request_origin, msg)) in self.outbound_requests.read().await.iter()
        {
            if let MessageBody::Request { ttl, body } = &msg.body {
                // If the outbound request is a cancel request originating
                // remotely, check if we previously sent the referenced
                // request to the connected peer. If so, forward the cancel
                // request. If not, move on to the next request without sending
                // this one.
                if let RequestBody::Cancel { cancel_id } = body {
                    debug!("Processing cancel request...");
                    if let RequestOrigin::Remote = request_origin {
                        let mut forwarded_requests = self.forwarded_requests.write().await;
                        if let Some(peers) = forwarded_requests.get_mut(cancel_id) {
                            if peers.contains(&peer_id) {
                                stream.write_all(&msg.to_bytes()?).await?;

                                // Remove the connected peer from the set of
                                // forwarded requests for the given cancel ID.
                                peers.remove(&peer_id);

                                // If the peer set for given cancel ID is
                                // empty, remove the ID from the map of
                                // forwarded requests.
                                if peers.is_empty() {
                                    forwarded_requests.remove(cancel_id);
                                }
                            } else {
                                // Terminate the current iteration of the loop
                                // and process the next request.
                                continue 'requests;
                            }
                        }
                    }
                }
                if *ttl == 0 {
                    // The TTL for this request has been exhausted.
                    self.outbound_requests.write().await.remove(req_id);
                } else {
                    // Send the message to the connected peer.
                    stream.write_all(&msg.to_bytes()?).await?;

                    // If the request originated remotely, add it to the list
                    // of forwarded requests. This facilitates forwarding
                    // cancel requests to these peers in the future, if
                    // required.
                    if let RequestOrigin::Remote = request_origin {
                        let mut forwarded_requests = self.forwarded_requests.write().await;
                        if let Some(peers) = forwarded_requests.get_mut(req_id) {
                            peers.insert(peer_id);
                        } else {
                            let mut peer_set = HashSet::new();
                            peer_set.insert(peer_id);
                            forwarded_requests.insert(*req_id, peer_set);
                        }
                    }
                }
            }
        }

        Ok(())
    }

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
        let timestamp = now()?;

        Ok((public_key, links, timestamp))
    }

    /// Publish a new text post and return the hash.
    pub async fn post_text<T: Into<String>, U: Into<String>>(
        &mut self,
        channel: T,
        text: U,
    ) -> Result<Hash, Error> {
        debug!("Posting text post...");

        let channel = channel.into();
        let (public_key, links, timestamp) = self.post_header_values(&channel).await?;
        let text = text.into();

        // Construct a new text post.
        let post = Post::text(public_key, links, timestamp, channel, text);

        self.post(post).await
    }

    /// Publish a new delete post with the given post hashes, returning the
    /// hash of the new post.
    pub async fn post_delete(&mut self, hashes: Vec<Hash>) -> Result<Hash, Error> {
        let public_key = self.get_public_key().await?;
        let links = vec![];
        let timestamp = now()?;

        // Add the hashes to the store of deleted posts.
        //
        // This helps to ensure these posts won't be re-added to the
        // local store if they are ever returned by a remote peer.
        let mut deleted_posts = self.deleted_posts.write().await;
        deleted_posts.extend(&hashes);

        // Drop the mutable borrow of `self` to allow the later
        // call to `self.post()` (immutable borrow).
        drop(deleted_posts);

        // Construct a new delete post.
        let post = Post::delete(public_key, links, timestamp, hashes);

        self.post(post).await
    }

    /// Publish a new info post with the given name and return the hash.
    pub async fn post_info_name(&mut self, username: &str) -> Result<Hash, Error> {
        let public_key = self.get_public_key().await?;
        let links = vec![];
        let timestamp = now()?;

        let name_info = UserInfo::name(username)?;

        // Construct a new info post.
        let post = Post::info(public_key, links, timestamp, vec![name_info]);

        self.post(post).await
    }

    /// Publish a new topic post for the given channel and return the hash.
    pub async fn post_topic<T: Into<String>, U: Into<String>>(
        &mut self,
        channel: T,
        topic: U,
    ) -> Result<Hash, Error> {
        let channel = channel.into();
        let (public_key, links, timestamp) = self.post_header_values(&channel).await?;
        let topic = topic.into();

        // Construct a new topic post.
        let post = Post::topic(public_key, links, timestamp, channel, topic);

        self.post(post).await
    }

    /// Publish a new join post for the given channel and return the hash.
    pub async fn post_join<T: Into<String>>(&mut self, channel: T) -> Result<Hash, Error> {
        let channel = channel.into();
        let (public_key, links, timestamp) = self.post_header_values(&channel).await?;

        // Construct a new join post.
        let post = Post::join(public_key, links, timestamp, channel);

        self.post(post).await
    }

    /// Publish a new leave post for the given channel and return the hash.
    pub async fn post_leave<T: Into<String>>(&mut self, channel: T) -> Result<Hash, Error> {
        let channel = channel.into();
        let (public_key, links, timestamp) = self.post_header_values(&channel).await?;

        // Construct a new leave post.
        let post = Post::leave(public_key, links, timestamp, channel);

        self.post(post).await
    }

    /// Publish a post and return the hash.
    pub async fn post(&mut self, mut post: Post) -> Result<Hash, Error> {
        // Sign the post if required.
        if !post.is_signed() {
            post.sign(&self.get_secret_key().await?)?;
        }

        // Insert the post into the local store.
        let hash = self.store.insert_post(&post).await?;

        // Send post hashes to all peers for whom we hold inbound requests.
        if let Some(channel) = post.get_channel() {
            self.send_post_hashes(channel).await?;
        } else {
            // Info and delete post types do not have a channel.
            //
            // Retrieve all channels of which the author of the post is a
            // member and send post hashes for those channels. This ensures
            // that any "live" channel state requests for these channels
            // receive the latest `post/info` and `post/delete` hashes.
            let public_key = post.get_public_key();
            if let Some(channels) = self.store.get_channels().await {
                for channel in channels {
                    if self.store.is_channel_member(&channel, &public_key).await {
                        self.send_post_hashes(&channel).await?;
                    }
                }
            }
        }

        Ok(hash)
    }

    /// Send post hashes matching peer request parameters for all live
    /// requests.
    ///
    /// Live requests may have originated from a ChannelTimeRange (`time_end`
    /// of 0) or a ChannelState request (`future` of 1).
    ///
    /// Hashes of `post/delete` and `post/info` posts are sent as part of
    /// ChannelState responses.
    async fn send_post_hashes(&mut self, channel: &Channel) -> Result<(), Error> {
        // Iterate over all live peer requests.
        for (peer_id, live_requests) in self.live_requests.read().await.iter() {
            // Iterate over peer requests.
            for live_request in live_requests {
                // Create an empty vector to store post hashes to be sent
                // in response.
                let mut hashes = Vec::new();

                match live_request {
                    LiveRequest::ChannelState(req_id, req_channel) => {
                        debug!(
                            "Matched channel state live request: {}",
                            hex::encode(req_id)
                        );

                        // Only send hashes if the channel of the post which invoked
                        // the call to `send_post_hashes()` matches the channel of
                        // the peer request.
                        if req_channel == channel {
                            // Return all channel state post hashes for this channel.
                            if let Some(mut channel_membership_hashes) =
                                self.store.get_channel_membership_hashes(channel).await
                            {
                                hashes.append(&mut channel_membership_hashes)
                            }

                            // Return the channel topic post hash for this channel.
                            let channel_topic_hash =
                                self.store.get_channel_topic_and_hash(channel).await;
                            if let Some((_topic, hash)) = channel_topic_hash {
                                hashes.push(hash)
                            }

                            // Retrieve public keys of all channel members.
                            if let Some(channel_members) =
                                self.store.get_channel_members(channel).await
                            {
                                for public_key in channel_members {
                                    // Return all delete post hashes for members of
                                    // this channel.
                                    if let Some(peer_delete_hashes) =
                                        self.store.get_delete_hashes(&public_key).await
                                    {
                                        hashes.extend(peer_delete_hashes)
                                    }

                                    // Send the most-recent name-setting info
                                    // post hash for each peer.
                                    if let Some((_peer_name, peer_name_hash)) =
                                        self.store.get_peer_name_and_hash(&public_key).await
                                    {
                                        hashes.push(peer_name_hash)
                                    }
                                }
                            }

                            // Retrieve public keys of all ex-channel members.
                            if let Some(ex_channel_members) =
                                self.store.get_ex_channel_members(channel).await
                            {
                                for public_key in ex_channel_members {
                                    // Send the most-recent name-setting info
                                    // post hash for each peer.
                                    if let Some((_peer_name, peer_name_hash)) =
                                        self.store.get_peer_name_and_hash(&public_key).await
                                    {
                                        hashes.push(peer_name_hash)
                                    }
                                }
                            }

                            // Construct a new hash response message.
                            let response = Message::hash_response(NO_CIRCUIT, *req_id, hashes);

                            // Send the response to the peer.
                            self.send(*peer_id, &response).await?;
                        }
                    }
                    LiveRequest::ChannelTimeRange(req_id, channel_opts) => {
                        debug!(
                            "Matched channel time range live request: {}",
                            hex::encode(req_id)
                        );

                        // Only send hashes if the channel of the post which invoked
                        // the call to `send_post_hashes()` matches the channel of
                        // the peer request.
                        if &channel_opts.channel == channel {
                            let limit = channel_opts.limit.min(4096);

                            // Get all post hashes matching the request parameters.
                            let mut stream = self.store.get_post_hashes(channel_opts).await;
                            while let Some(result) = stream.next().await {
                                hashes.push(result?);
                                // Break once the request limit has been reached.
                                if limit != 0 && hashes.len() as u64 >= limit {
                                    break;
                                }
                            }
                            // Drop the mutable borrow of `self` to allow the later
                            // call to `self.send()` (immutable borrow).
                            drop(stream);

                            // Construct a new hash response message.
                            let response =
                                Message::hash_response(NO_CIRCUIT, *req_id, hashes.clone());

                            // Only send a response if there are post hashes matching
                            // the given request parameters.
                            if !hashes.is_empty() {
                                // Send the response to the peer.
                                self.send(*peer_id, &response).await?;
                            }
                        }
                    }
                }
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

        // Ignore this message if the request ID has previously been handled
        // and it is not an active live request or outbound request.
        if self.handled_requests.read().await.contains(&req_id)
            && !self.is_live_request(&peer_id, &req_id).await
            && !self.outbound_requests.read().await.contains_key(&req_id)
        {
            debug!(
                "Dropping message from handler; request ID has been seen before: {}",
                msg.header
            );

            return Ok(());
        }

        // TODO: Forward requests.
        match &msg.body {
            MessageBody::Request { ttl, body } => match body {
                RequestBody::Post { hashes } => {
                    debug!("Handling post request...");

                    // If the request TTL is > 0, decrement it and add the
                    // message to `outbound_requests` so that it will be
                    // forwarded to other connected peers.
                    //
                    // TODO: Set the TTL to 16 if it is > 16.
                    if *ttl > 0 {
                        self.decrement_ttl_and_write_to_outbound(req_id, msg).await;
                    }

                    let posts = self.store.get_post_payloads(hashes).await;
                    let response = Message::post_response(circuit_id, req_id, posts);

                    self.send(peer_id, &response).await?
                }
                RequestBody::Cancel { cancel_id } => {
                    debug!("Handling cancel request...");

                    // TTL is ignored for cancel requests so we decrement and
                    // write the message without first checking the value.
                    self.decrement_ttl_and_write_to_outbound(req_id, msg).await;

                    // Remove the request from the map of live requests.
                    self.remove_live_request(&peer_id, cancel_id).await?;

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
                    debug!("Handling channel time range request...");

                    if *ttl > 0 {
                        self.decrement_ttl_and_write_to_outbound(req_id, msg).await;
                    }

                    let channel_opts = ChannelOptions::new(channel, *time_start, *time_end, *limit);

                    let n_limit = (*limit).min(4096);

                    let mut hashes = vec![];
                    // Create a stream of post hashes matching the given criteria.
                    let mut stream = self.store.get_post_hashes(&channel_opts).await;
                    // Iterate over the hashes in the stream.
                    while let Some(result) = stream.next().await {
                        hashes.push(result?);
                        // Break out of the loop once the requested limit is
                        // met.
                        //
                        // A limit of 0 means there is no limit on the number
                        // of hashes that may be returned.
                        if n_limit != 0 && hashes.len() as u64 >= n_limit {
                            break;
                        }
                    }
                    // Drop the mutable borrow of `self` to allow the later
                    // call to `self.send()` (immutable borrow).
                    drop(stream);

                    let response = Message::hash_response(circuit_id, req_id, hashes.clone());

                    // Add the peer and request ID to the request tracker if
                    // the end time has been set to 0 (i.e. keep this request
                    // alive and send new messages as they become available).
                    if *time_end == 0 {
                        let live_request = LiveRequest::ChannelTimeRange(req_id, channel_opts);

                        let mut live_requests = self.live_requests.write().await;
                        if let Some(peer_requests) = live_requests.get_mut(&peer_id) {
                            // TODO: Only push if `peer_requests` does not
                            // already contain this request.
                            peer_requests.push(live_request);
                        } else {
                            live_requests.insert(peer_id, vec![live_request]);
                        }

                        // Only send a response if there are post hashes matching
                        // the given request parameters.
                        if !hashes.is_empty() {
                            self.send(peer_id, &response).await?
                        }
                    } else {
                        // Send a hash response, even if there are no known
                        // hashes matching the request parameters.
                        self.send(peer_id, &response).await?
                    }
                }
                RequestBody::ChannelState { channel, future } => {
                    debug!("Handling channel state request...");

                    if *ttl > 0 {
                        self.decrement_ttl_and_write_to_outbound(req_id, msg).await;
                    }

                    let mut hashes = Vec::new();

                    // Get the hash of the latest join or leave post for all
                    // channel members and ex-members.
                    if let Some(mut channel_membership_hashes) =
                        self.store.get_channel_membership_hashes(channel).await
                    {
                        hashes.append(&mut channel_membership_hashes)
                    }

                    // If a topic has been set for the channel, return the hash
                    // of the post.
                    if let Some((_topic, topic_hash)) =
                        self.store.get_channel_topic_and_hash(channel).await
                    {
                        hashes.push(topic_hash)
                    }

                    let response = Message::hash_response(circuit_id, req_id, hashes.clone());

                    // Send only the latest known hashes; do not keep the
                    // request alive after responding.
                    if *future == 0 {
                        if !hashes.is_empty() {
                            // Send the known hashes.
                            self.send(peer_id, &response).await?;

                            // Compost and send an empty hash response to
                            // terminate the request.
                            let closing_response =
                                Message::hash_response(circuit_id, req_id, Vec::new());
                            self.send(peer_id, &closing_response).await?;
                        } else {
                            // Send the empty hash response to terminate the
                            // request.
                            self.send(peer_id, &response).await?;
                        }
                    } else if *future == 1 {
                        // Add the peer and request ID to the request tracker if
                        // the future field has been set to 1 (i.e. keep this request
                        // alive and send new messages as they become available).
                        let live_request = LiveRequest::ChannelState(req_id, channel.to_string());

                        let mut live_requests = self.live_requests.write().await;
                        if let Some(peer_requests) = live_requests.get_mut(&peer_id) {
                            peer_requests.push(live_request);
                        } else {
                            live_requests.insert(peer_id, vec![live_request]);
                        }

                        // Only send a response if there are post hashes matching
                        // the given request parameters.
                        if !hashes.is_empty() {
                            self.send(peer_id, &response).await?
                        }
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
                    debug!("Handling channel list request...");

                    if *ttl > 0 {
                        self.decrement_ttl_and_write_to_outbound(req_id, msg).await;
                    }

                    let skip = *skip as usize;
                    let limit = *limit as usize;

                    // Retrieve all known channels, returning an empty vector
                    // if none are known.
                    let channels = if let Some(mut all_channels) = self.store.get_channels().await {
                        let channels_len = all_channels.len();

                        // Define the channel query limit based on the request
                        // limit and the number of known channels.
                        let limit = if limit == 0 || limit > channels_len {
                            channels_len
                        } else {
                            limit
                        };

                        // Drain the channels matching the given range.
                        all_channels.drain(skip..limit).collect()
                    } else {
                        Vec::new()
                    };

                    // Send a response, even if no channels are currently known.
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
                    debug!("Handling hash response...");

                    let wanted_hashes = self.store.want(hashes).await;
                    if !wanted_hashes.is_empty() {
                        let (_, new_req_id) = self.new_req_id().await?;

                        // If a hash appears in our list of wanted hashed,
                        // send a request for the associated post.
                        let request = Message::post_request(
                            circuit_id,
                            new_req_id,
                            TTL,
                            wanted_hashes.to_owned(),
                        );

                        self.send(peer_id, &request).await?;

                        // Update the list of requested posts.
                        let mut requested_posts = self.requested_posts.write().await;
                        for hash in &wanted_hashes {
                            requested_posts.insert(*hash);
                        }
                    }

                    // TODO: If hash_count == 0, remove the request.
                    // This may be more relevant when responding to a channel
                    // time range request (ie. sending a hash response).
                }
                ResponseBody::Post { posts } => {
                    debug!("Handling post response...");

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

                        let deleted_posts = self.deleted_posts.read().await;
                        // Check if a delete post has previously been
                        // encountered which references this post hash.
                        if deleted_posts.contains(&post_hash) {
                            // Skip processing this post so that we do not add
                            // it to the local store.
                            continue;
                        }

                        let mut requested_posts = self.requested_posts.write().await;
                        // Check if this post was previously requested.
                        if !requested_posts.contains(&post_hash) {
                            // Skip this post if it was not requested.
                            continue;
                        }
                        // Remove the post hash from the list of requested
                        // posts.
                        requested_posts.remove(&post_hash);

                        self.store.insert_post(&post).await?;
                    }
                }
                ResponseBody::ChannelList { channels } => {
                    debug!("Handling channel list response...");

                    // TODO: Do we need to take action to conclude the request
                    // which resulted in this response?
                    for channel in channels {
                        self.store.insert_channel(channel).await;
                    }
                }
            },
            // Ignore unrecognized message type.
            MessageBody::Unrecognized { .. } => {
                debug!("Received unrecognized message; skipping message handling...");
            }
        }

        // Mark this request as "handled" (to prevent request loops).
        self.handled_requests.write().await.insert(req_id);

        Ok(())
    }
}
