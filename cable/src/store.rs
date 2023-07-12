//! Store trait implementation and associated methods for cable, along with
//! an in-memory implementation of the `Store` trait.

use std::{
    collections::{BTreeMap, HashMap},
    convert::TryInto,
};

use async_std::{
    prelude::*,
    stream,
    sync::{Arc, Mutex, RwLock},
    task,
};
use desert::ToBytes;
use sodiumoxide::crypto;

use crate::{
    error::Error,
    post::{Post, PostBody},
    stream::{HashStream, LiveStream, PostStream},
    Channel, ChannelOptions, Hash, Payload,
};

/// A public-private keypair.
pub type Keypair = ([u8; 32], [u8; 64]);
/// Post request options (same as `ChannelOptions`).
pub type GetPostOptions = ChannelOptions;

#[async_trait::async_trait]
/// Storage trait with methods for storing and retrieving cryptographic
/// keypairs, hashes and posts.
pub trait Store: Clone + Send + Sync + Unpin + 'static {
    /// Retrieve the keypair associated with the store.
    async fn get_keypair(&mut self) -> Result<Option<Keypair>, Error>;

    /// Define the keypair associated with the store.
    async fn set_keypair(&mut self, keypair: Keypair) -> Result<(), Error>;

    /// Retrieve the keypair associated with the store, creating a new keypair
    /// if one does not yet exist.
    async fn get_or_create_keypair(&mut self) -> Result<Keypair, Error> {
        if let Some(kp) = self.get_keypair().await? {
            Ok(kp)
        } else {
            let (pk, sk) = crypto::sign::gen_keypair();
            let kp = (
                pk.as_ref().try_into().unwrap(),
                sk.as_ref().try_into().unwrap(),
            );
            self.set_keypair(kp).await?;
            Ok(kp)
        }
    }

    /// Retrieve the hash of the most recently published post in the given
    /// channel.
    async fn get_latest_hash(&mut self, channel: &[u8]) -> Result<[u8; 32], Error>;

    /// Insert the given post into the store.
    async fn insert_post(&mut self, post: &Post) -> Result<(), Error>;

    /// Retrieve all posts matching the parameters defined by the given
    /// `GetPostOptions`.
    async fn get_posts<'a>(&'a mut self, opts: &GetPostOptions) -> Result<PostStream, Error>;

    /// Retrieve all posts matching the parameters defined by the given
    /// `GetPostOptions` and continue to return new messages as they become
    /// available (stream remains active).
    async fn get_posts_live<'a>(&'a mut self, opts: &GetPostOptions) -> Result<PostStream, Error>;

    /// Retrieve the hashes of all posts matching the parameters defined by the
    /// given `GetPostOptions`.
    async fn get_post_hashes<'a>(&'a mut self, opts: &GetPostOptions) -> Result<HashStream, Error>;

    /// Retrieve the hashes of all posts representing the subset of the given
    /// hashes for which post data is not available locally (ie. the hashes of
    /// all posts which are not already in the store).
    async fn want(&mut self, hashes: &[Hash]) -> Result<Vec<Hash>, Error>;

    /// Retrieve the post data for all posts represented by the given hashes.
    async fn get_data(&mut self, hashes: &[Hash]) -> Result<Vec<Payload>, Error>;
}

#[derive(Clone)]
/// An in-memory store containing a keypair and post data.
pub struct MemoryStore {
    keypair: Keypair,
    /// All posts in the store divided according to channel (the outer key)
    /// and indexed by timestamp (the inner key).
    posts: Arc<RwLock<HashMap<Channel, BTreeMap<u64, Vec<Post>>>>>,
    /// All post hashes in the store divided according to channel (the outer
    /// key) and indexed by timestamp (the inner key).
    post_hashes: Arc<RwLock<HashMap<Channel, BTreeMap<u64, Vec<Hash>>>>>,
    /// Binary payloads for all posts in the store, indexed by the post hash.
    data: Arc<RwLock<HashMap<Hash, Payload>>>,
    /// An empty `BTreeMap` of posts, indexed by timestamp.
    empty_post_bt: BTreeMap<u64, Vec<Post>>,
    /// An empty `BTreeMap` of post hashes, indexed by timestamp.
    empty_hash_bt: BTreeMap<u64, Vec<Hash>>,
    /// All active live streams, indexed by channel.
    live_streams: Arc<RwLock<HashMap<Channel, Arc<RwLock<Vec<LiveStream>>>>>>,
    /// The unique identifier of a live stream.
    live_stream_id: Arc<Mutex<usize>>,
}

impl Default for MemoryStore {
    fn default() -> Self {
        // Generate a new public-private keypair.
        let (pk, sk) = crypto::sign::gen_keypair();

        Self {
            keypair: (
                // TODO: Replace `unwrap` with try operator.
                pk.as_ref().try_into().unwrap(),
                sk.as_ref().try_into().unwrap(),
            ),
            posts: Arc::new(RwLock::new(HashMap::new())),
            post_hashes: Arc::new(RwLock::new(HashMap::new())),
            data: Arc::new(RwLock::new(HashMap::new())),
            empty_post_bt: BTreeMap::new(),
            empty_hash_bt: BTreeMap::new(),
            live_streams: Arc::new(RwLock::new(HashMap::new())),
            live_stream_id: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait::async_trait]
impl Store for MemoryStore {
    async fn get_keypair(&mut self) -> Result<Option<Keypair>, Error> {
        Ok(Some(self.keypair))
    }

    async fn set_keypair(&mut self, keypair: Keypair) -> Result<(), Error> {
        self.keypair = keypair;
        Ok(())
    }

    async fn get_latest_hash(&mut self, _channel: &[u8]) -> Result<[u8; 32], Error> {
        // TODO: Return the latest post hash, if available, instead of zeros.
        Ok([0; 32])
    }

    async fn insert_post(&mut self, post: &Post) -> Result<(), Error> {
        let timestamp = &post.get_timestamp();

        match &post.body {
            // TODO: Include matching arms for other post types.
            PostBody::Text { channel, text: _ } => {
                {
                    // Open the post store for writing.
                    let mut posts = self.posts.write().await;

                    // Retrieve the stored posts matching the given channel.
                    if let Some(post_map) = posts.get_mut(channel) {
                        // Retrieve the stored posts matching the given
                        // timestamp.
                        if let Some(posts) = post_map.get_mut(timestamp) {
                            // Add the post to the vector of posts indexed
                            // by the given timestamp.
                            posts.push(post.clone());
                        } else {
                            // Insert the post (as a `Vec`) into the `BTreeMap`,
                            // using the timestamp as the key.
                            post_map.insert(*timestamp, vec![post.clone()]);
                        }
                    } else {
                        // No posts have previously been stored for the
                        // given channel.

                        let mut post_map = BTreeMap::new();
                        // Insert the post (as a `Vec`) into the `BTreeMap`,
                        // using the timestamp as the key.
                        post_map.insert(*timestamp, vec![post.clone()]);
                        // Insert the `BTreeMap` into the posts `HashMap`,
                        // using the channel name as the key.
                        posts.insert(channel.to_owned(), post_map);
                    }
                }
                {
                    // Open the post hashes store for writing.
                    let mut post_hashes = self.post_hashes.write().await;

                    // Retrieve the stored post hashes matching the given
                    // channel.
                    if let Some(hash_map) = post_hashes.get_mut(channel) {
                        // Retrieve the stored post hashes matching the given
                        // timestamp.
                        if let Some(hashes) = hash_map.get_mut(timestamp) {
                            // Add the hash to the vector of hashes indexed by
                            // the given timestamp.
                            hashes.push(post.hash()?);
                        } else {
                            // Hash the post.
                            let hash = post.hash()?;
                            // Insert the hash (as a `Vec`) into the `BTreeMap`,
                            // using the timestampas the key.
                            hash_map.insert(*timestamp, vec![hash]);

                            // Insert the binary payload of the post into the
                            // `HashMap` of post data, indexed by the hash.
                            self.data.write().await.insert(hash, post.to_bytes()?);
                        }
                    } else {
                        // No hashes have previously been stored for the
                        // given channel.

                        let mut hash_map = BTreeMap::new();
                        // Hash the post.
                        let hash = post.hash()?;
                        // Insert the hash (as a `Vec`) into the `BTreeMap`,
                        // using the timestamp as the key.
                        hash_map.insert(*timestamp, vec![hash]);
                        // Insert the `BTreeMap` into the post hashes `HashMap`,
                        // using the channel name as the key.
                        post_hashes.insert(channel.to_owned(), hash_map);

                        // Insert the binary payload of the post into the
                        // `HashMap` of post data, indexed by the hash.
                        self.data.write().await.insert(hash, post.to_bytes()?);
                    }
                }
                // If we have open live streams matching the channel to which
                // this post was published...
                if let Some(senders) = self.live_streams.read().await.get(channel) {
                    for stream in senders.write().await.iter_mut() {
                        if stream.matches(post) {
                            // Send the post to each stream for which the channel
                            // option criteria are satisfied.
                            stream.send(post.clone()).await;
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn get_posts(&mut self, opts: &GetPostOptions) -> Result<PostStream, Error> {
        let posts = self
            .posts
            .write()
            .await
            .get(&opts.channel)
            // Return an empty map if no posts are found matching the given
            // channel.
            .unwrap_or(&self.empty_post_bt)
            .range(opts.time_start..opts.time_end)
            .flat_map(|(_time, posts)| posts.iter().map(|post| Ok(post.clone())))
            .collect::<Vec<Result<Post, Error>>>();

        Ok(Box::new(stream::from_iter(posts.into_iter())))
    }

    async fn get_posts_live(&mut self, opts: &GetPostOptions) -> Result<PostStream, Error> {
        let live_stream = {
            let mut live_streams = self.live_streams.write().await;

            // Select existing streams which match the given channel.
            if let Some(streams) = live_streams.get_mut(&opts.channel) {
                let live_stream = {
                    let mut id = self.live_stream_id.lock().await;
                    // Increment the live stream counter.
                    *id += 1;
                    // Return a new live stream.
                    LiveStream::new(*id, opts.clone(), streams.clone())
                };
                let live = live_stream.clone();
                task::block_on(async move {
                    // Add the newly-created stream to the streams store for
                    // the given channel.
                    streams.write().await.push(live);
                });

                live_stream
            } else {
                // No streams were found which match the given channel.

                let streams = Arc::new(RwLock::new(vec![]));
                // Generate a new live stream ID.
                let live_stream_id = {
                    let mut id = self.live_stream_id.lock().await;
                    *id += 1;
                    id
                };
                let streams_c = streams.clone();
                // Create a new stream and add it to the streams `Vec`.
                let live_stream = task::block_on(async move {
                    let live_stream =
                        LiveStream::new(*live_stream_id, opts.clone(), streams_c.clone());
                    streams_c.write().await.push(live_stream.clone());
                    live_stream
                });
                // Add the newly-created stream to the streams store
                // for the given channel.
                live_streams.insert(opts.channel.clone(), streams);

                live_stream
            }
        };

        let post_stream = self.get_posts(opts).await?;

        Ok(Box::new(post_stream.merge(live_stream)))
    }

    async fn get_post_hashes(&mut self, opts: &GetPostOptions) -> Result<HashStream, Error> {
        let start = opts.time_start;
        let end = opts.time_end;
        let empty = self.empty_hash_bt.range(..);

        let hashes = self
            .post_hashes
            .read()
            .await
            .get(&opts.channel)
            // Return only the hashes for which the key (timestamp: `x`)
            // matches the given range (provided via `opts`).
            .map(|x| match (start, end) {
                (0, 0) => x.range(..),
                (0, end) => x.range(..end),
                (start, 0) => x.range(start..),
                _ => x.range(start..end),
            })
            .unwrap_or(empty)
            .flat_map(|(_time, hashes)| hashes.iter().map(|hash| Ok(*hash)))
            .collect::<Vec<Result<Hash, Error>>>();

        Ok(Box::new(stream::from_iter(hashes.into_iter())))
    }

    async fn want(&mut self, hashes: &[Hash]) -> Result<Vec<Hash>, Error> {
        let data = self.data.read().await;

        Ok(hashes
            .iter()
            .filter(|hash| !data.contains_key(&(*hash).clone()))
            .cloned()
            .collect())
    }

    async fn get_data(&mut self, hashes: &[Hash]) -> Result<Vec<Payload>, Error> {
        let data = self.data.read().await;

        Ok(hashes
            .iter()
            .filter_map(|hash| data.get(hash))
            .cloned()
            .collect())
    }
}
