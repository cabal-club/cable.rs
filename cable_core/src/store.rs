//! Store trait implementation and associated methods for cable, along with
//! an in-memory implementation of the `Store` trait.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    convert::TryInto,
};

use async_std::{
    prelude::*,
    stream,
    sync::{Arc, Mutex, RwLock},
    task,
};
use cable::{
    post::{Post, PostBody},
    Channel, ChannelOptions, Error, Hash, Nickname, Payload, Timestamp, Topic,
};
use desert::ToBytes;
use sodiumoxide::crypto;

use crate::stream::{HashStream, LiveStream, PostStream};

/// A public key.
pub type PublicKey = [u8; 32];

/// A public-private keypair.
pub type Keypair = ([u8; 32], [u8; 64]);

/// A `HashMap` of posts with a key of channel name and a value of a `BTreeMap`.
/// The `BTreeMap` has a key of timestamp and value of a `Vec` of tuple with
/// post and post hash.
pub type PostMap = HashMap<Channel, BTreeMap<u64, Vec<(Post, Hash)>>>;

/*
/// A `HashMap` of post hashes with a key of channel name and a value of a
/// `BTreeMap`. The `BTreeMap` has a key of timestamp and value of a `Vec`
/// of post hashes.
pub type PostHashMap = HashMap<Channel, BTreeMap<u64, Vec<Hash>>>;
*/

/// A `HashMap` of live streams with a key of channel name and a value
/// of a `Vec` of streams (wrapped in an `Arc` and `RwLock`).
pub type LiveStreamMap = HashMap<Channel, Arc<RwLock<Vec<LiveStream>>>>;

/// A `HashMap` of channel topics with a key of channel name and a value of a
/// `BTreeMap`. The `BTreeMap` has a key of timestamp and a value of a tuple
/// of topic and hash. The hash is of the `post/topic` post which defined the
/// stored topic.
pub type TopicHashMap = HashMap<Channel, BTreeMap<Timestamp, (Topic, Hash)>>;

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

    /// Retrieve the hash(es) of the most recently published post(s) in the
    /// given channel.
    ///
    /// More than one hash will be returned if several posts were
    /// made to the same channel at the same time. Usually though, only one
    /// hash or no hashes will be returned.
    async fn get_latest_hashes(&mut self, channel: &Channel) -> Option<Vec<Hash>>;

    /// Insert the given channel into the store.
    async fn insert_channel(&mut self, channel: &Channel) -> Result<(), Error>;

    /// Retrieve all channels from the store.
    async fn get_channels<'a>(&'a mut self) -> Result<Vec<Channel>, Error>;

    /// Insert the given public key into the store using the key defined by the
    /// given channel.
    async fn insert_channel_member(
        &mut self,
        channel: &Channel,
        public_key: &PublicKey,
    ) -> Result<(), Error>;

    /// Remove the given public key from the membership store using the key
    /// defined by the given channel.
    async fn remove_channel_member(
        &mut self,
        channel: &Channel,
        public_key: &PublicKey,
    ) -> Result<(), Error>;

    /// Query whether the given public key is a member of the given channel.
    async fn is_channel_member<'a>(
        &'a mut self,
        channel: &Channel,
        public_key: &PublicKey,
    ) -> Result<bool, Error>;

    /// Retrieve all members of the given channel.
    async fn get_channel_members<'a>(
        &'a mut self,
        channel: &Channel,
    ) -> Result<Vec<PublicKey>, Error>;

    /// Update the membership store with the hash of the latest `post/join` or
    /// `post/leave` post made to the given channel by the given public key.
    async fn update_channel_membership_hashes(
        &mut self,
        channel: &Channel,
        public_key: &PublicKey,
        hash: &Hash,
    ) -> Result<(), Error>;

    // TODO: Consider returning a `HashStream` instead of a vector.
    /// Retrieve all of the latest `post/join` or `post/leave` post hashes
    /// for the given channel.
    async fn get_channel_membership_hashes<'a>(
        &'a mut self,
        channel: &Channel,
    ) -> Result<Vec<Hash>, Error>;

    /// Remove the channel membership data for the given post hash.
    async fn remove_channel_membership_hash(&mut self, hash: &Hash) -> Result<(), Error>;

    /// Insert the given channel topic, timestamp and hash into the store if
    /// the timestamp is later than the timestamp of the stored topic post.
    async fn insert_channel_topic(
        &mut self,
        channel: &Channel,
        topic: &Topic,
        timestamp: &Timestamp,
        hash: &Hash,
    ) -> Result<(), Error>;

    /// Remove the channel topic data for the given post hash.
    async fn remove_channel_topic(&mut self, hash: &Hash) -> Result<(), Error>;

    /// Retrieve the latest `post/topic` topic and hash for the given channel.
    async fn get_channel_topic_and_hash<'a>(
        &'a mut self,
        channel: &Channel,
    ) -> Result<Option<(Topic, Hash)>, Error>;

    /*
    /// Insert the given hash into the store using the key defined by the
    /// given public key.
    async fn insert_latest_join_or_leave_post_hash(
        &mut self,
        public_key: &PublicKey,
        hash: Hash,
    ) -> Result<(), Error>;

    /// Retrieve the latest join or leave post hash for all known peers.
    async fn get_latest_join_or_leave_post_hashes<'a>(&'a mut self) -> Result<Vec<Hash>, Error>;
    */

    /// Send the given post to each live stream for which the channel option
    /// criteria are satisfied.
    async fn send_post_to_live_streams(&mut self, post: &Post, channel: &Channel);

    /// Insert the given nickname into the store using the key defined by the
    /// given public key.
    async fn insert_name(&mut self, public_key: &PublicKey, name: &Nickname);

    /// Retrieve the nickname associated with the given public key.
    async fn get_name(&mut self, public_key: &PublicKey) -> Option<Nickname>;

    /// Insert the given post into the store and return the hash.
    async fn insert_post(&mut self, post: &Post) -> Result<Hash, Error>;

    /// Remove the given post from the posts and post hashes stores.
    async fn remove_post(&mut self, hash: &Hash) -> Result<(), Error>;

    /// Delete the given post from all stores, leaving no trace.
    ///
    /// This method combines several removal methods to achieve complete
    /// removal of the post.
    async fn delete_post(&mut self, hash: &Hash) -> Result<(), Error>;

    /// Update the posts store by inserting the given post.
    ///
    /// This method is more specific than `insert_post()`. It updates only
    /// the `posts` store, whereas `insert_post()` is responsible for updating
    /// multiple stores based on the post type.
    async fn update_posts(
        &mut self,
        post: &Post,
        channel: &Channel,
        timestamp: &Timestamp,
        hash: Hash,
    );

    /// Retrieve all posts matching the parameters defined by the given
    /// `ChannelOptions`.
    async fn get_posts<'a>(&'a mut self, opts: &ChannelOptions) -> Result<PostStream, Error>;

    /// Retrieve all posts matching the parameters defined by the given
    /// `ChannelOptions` and continue to return new messages as they become
    /// available (stream remains active).
    async fn get_posts_live<'a>(&'a mut self, opts: &ChannelOptions) -> Result<PostStream, Error>;

    /// Retrieve the hashes of all posts matching the parameters defined by the
    /// given `ChannelOptions`.
    async fn get_post_hashes<'a>(&'a mut self, opts: &ChannelOptions) -> Result<HashStream, Error>;

    /// Retrieve the post payloads for all posts represented by the given hashes.
    async fn get_post_payloads(&mut self, hashes: &[Hash]) -> Result<Vec<Payload>, Error>;

    /// Retrieve the hashes of all posts representing the subset of the given
    /// hashes for which post data is not available locally (ie. the hashes of
    /// all posts which are not already in the store).
    async fn want(&mut self, hashes: &[Hash]) -> Result<Vec<Hash>, Error>;
}

#[derive(Clone)]
/// An in-memory store containing a keypair and post data.
pub struct MemoryStore {
    keypair: Keypair,
    /// All channels in the store.
    channels: Arc<RwLock<BTreeSet<Channel>>>,
    /// The public keys of all members, indexed by channel.
    ///
    /// This map is updated according to received / published `post/join`
    /// and `post/leave` posts.
    channel_members: Arc<RwLock<HashMap<Channel, Vec<PublicKey>>>>,
    /// The hash of the latest `post/join` or `post/leave` post for each known
    /// peer, indexed by channel (the outer key) and public key (the first
    /// element of the tuple).
    channel_membership: Arc<RwLock<HashMap<Channel, HashMap<PublicKey, Hash>>>>,
    /// The topic, timestamp and hash of the latest `post/topic` post for each
    /// known channel, indexed by channel.
    channel_topics: Arc<RwLock<TopicHashMap>>,
    /// The nickname of each known peer, indexed by public key.
    peer_names: Arc<RwLock<HashMap<PublicKey, Nickname>>>,
    /// All posts and hashes in the store divided according to channel (the
    /// outer key) and indexed by timestamp (the inner key).
    posts: Arc<RwLock<PostMap>>,
    /*
    /// All post hashes in the store divided according to channel (the outer
    /// key) and indexed by timestamp (the inner key).
    post_hashes: Arc<RwLock<PostHashMap>>,
    */
    /// Binary payloads for all posts in the store, indexed by the post hash.
    post_payloads: Arc<RwLock<HashMap<Hash, Payload>>>,
    /// An empty `BTreeMap` of posts and hashes, indexed by timestamp.
    empty_post_bt: BTreeMap<u64, Vec<(Post, Hash)>>,
    /// All active live streams, indexed by channel.
    live_streams: Arc<RwLock<LiveStreamMap>>,
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
            channels: Arc::new(RwLock::new(BTreeSet::new())),
            channel_members: Arc::new(RwLock::new(HashMap::new())),
            channel_membership: Arc::new(RwLock::new(HashMap::new())),
            channel_topics: Arc::new(RwLock::new(HashMap::new())),
            peer_names: Arc::new(RwLock::new(HashMap::new())),
            posts: Arc::new(RwLock::new(HashMap::new())),
            //post_hashes: Arc::new(RwLock::new(HashMap::new())),
            post_payloads: Arc::new(RwLock::new(HashMap::new())),
            empty_post_bt: BTreeMap::new(),
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

    async fn get_latest_hashes(&mut self, channel: &Channel) -> Option<Vec<Hash>> {
        // Open the posts store for reading.
        let posts_map = self.posts.read().await;

        // Get the BTree associated with the given channel.
        if let Some(posts_btree) = posts_map.get(channel) {
            // Return the most recently added hash(es).
            posts_btree.last_key_value().map(|(_, post_vec)| {
                post_vec
                    .iter()
                    // Only return the hash from each tuple in the vector.
                    .map(|(_post, hash)| hash.to_owned())
                    .collect()
            })
        } else {
            None
        }
    }

    async fn insert_channel(&mut self, channel: &Channel) -> Result<(), Error> {
        // Open the channel store for writing.
        let mut channel_store = self.channels.write().await;
        channel_store.insert(channel.to_owned());

        Ok(())
    }

    async fn get_channels(&mut self) -> Result<Vec<Channel>, Error> {
        let channels = self.channels.read().await.iter().cloned().collect();

        Ok(channels)
    }

    async fn insert_channel_member(
        &mut self,
        channel: &Channel,
        public_key: &PublicKey,
    ) -> Result<(), Error> {
        // Open the channel members store for writing.
        let mut channel_members = self.channel_members.write().await;
        // Retrieve the stored members matching the given channel.
        if let Some(members) = channel_members.get_mut(channel) {
            // Add the public key to the vector of public keys indexed by the
            // given channel.
            members.push(public_key.to_owned())
        } else {
            // Insert the channel into the hash map, using the
            // given public key to create the value vec.
            channel_members.insert(channel.to_owned(), vec![*public_key]);
        }

        Ok(())
    }

    async fn remove_channel_member(
        &mut self,
        channel: &Channel,
        public_key: &PublicKey,
    ) -> Result<(), Error> {
        // Open the channel members store for writing.
        let mut channel_members = self.channel_members.write().await;
        // Retrieve the stored members matching the given channel.
        if let Some(members) = channel_members.get_mut(channel) {
            // Iterate over the members and retain only those for which the
            // member does not match the given public key.
            members.retain(|member| member != public_key);
        }

        Ok(())
    }

    async fn get_channel_members(&mut self, channel: &Channel) -> Result<Vec<PublicKey>, Error> {
        let channel_members = self
            .channel_members
            .read()
            .await
            .get(channel)
            .map(|member| member.to_owned())
            // Return an empty vector if no members are found matching the
            // given channel.
            .unwrap_or(Vec::new());

        Ok(channel_members)
    }

    async fn is_channel_member(
        &mut self,
        channel: &Channel,
        public_key: &PublicKey,
    ) -> Result<bool, Error> {
        let channel_members = self.get_channel_members(channel).await?;

        Ok(channel_members.contains(public_key))
    }

    async fn update_channel_membership_hashes(
        &mut self,
        channel: &Channel,
        public_key: &PublicKey,
        hash: &Hash,
    ) -> Result<(), Error> {
        // Open the channel members store for writing.
        let mut channel_membership = self.channel_membership.write().await;
        // Retrieve the stored public key / hash hash map matching the given
        // channel.
        if let Some(membership_map) = channel_membership.get_mut(channel) {
            // Add the public key to the vector of public keys indexed by the
            // given channel.
            membership_map.insert(public_key.to_owned(), *hash);
        } else {
            // No hashes have previously been stored for the
            // given channel.

            let mut membership_map = HashMap::new();
            membership_map.insert(*public_key, *hash);

            // Insert the members hash map into the channel membership hash map.
            channel_membership.insert(channel.to_owned(), membership_map);
        }

        Ok(())
    }

    async fn get_channel_membership_hashes(
        &mut self,
        channel: &Channel,
    ) -> Result<Vec<Hash>, Error> {
        let channel_membership: Vec<Hash> = self
            .channel_membership
            .read()
            .await
            .get(channel)
            // Return an empty map if no posts are found matching the given
            // channel.
            .unwrap_or(&HashMap::new())
            // Retrieve the hash for each entry in the hash map.
            .values()
            .cloned()
            .collect();

        Ok(channel_membership)
    }

    async fn remove_channel_membership_hash(&mut self, hash: &Hash) -> Result<(), Error> {
        // Open the channel membership store for writing.
        let mut channel_membership = self.channel_membership.write().await;

        // Iterate over all key-value pairs in the hash map.
        //
        // The `membership_map` is a `HashMap`.
        channel_membership
            .iter_mut()
            .for_each(|(_channel, membership_map)| {
                // Remove any key-value pair for which the stored hash of the join
                // or leave post matches the given hash.
                membership_map.retain(|_public_key, stored_hash| stored_hash != hash)
            });

        Ok(())
    }

    async fn insert_channel_topic(
        &mut self,
        channel: &Channel,
        topic: &Topic,
        timestamp: &Timestamp,
        hash: &Hash,
    ) -> Result<(), Error> {
        // Open the channel topics store for writing.
        let mut channel_topics = self.channel_topics.write().await;
        // Retrieve the stored tuple of topic, timestamp and hash matching the
        // given channel.
        if let Some(topic_map) = channel_topics.get_mut(channel) {
            // Insert the given topic and hash into the map, using the
            // timestamp as the key.
            topic_map.insert(*timestamp, (topic.to_owned(), *hash));
        } else {
            // No topic data has previously been stored for the
            // given channel.

            let mut topic_map = BTreeMap::new();
            // Insert the topic data into the `BTreeMap`, using the timestamp
            // as the key.
            topic_map.insert(*timestamp, (topic.to_owned(), *hash));
            // Insert the `BTreeMap` into the channel topics `HashMap`,
            // using the channel name as the key.
            channel_topics.insert(channel.to_owned(), topic_map);
        }

        Ok(())
    }

    async fn remove_channel_topic(&mut self, hash: &Hash) -> Result<(), Error> {
        // Open the channel topics store for writing.
        let mut channel_topics = self.channel_topics.write().await;

        // Iterate over all key-value pairs in the hash map.
        //
        // The `topic_map` is a `BTreeMap`.
        channel_topics.iter_mut().for_each(|(_channel, topic_map)| {
            // Remove any key-value pair for which the stored hash of the topic
            // post matches the given hash.
            topic_map.retain(|_timestamp, (_topic, stored_hash)| stored_hash != hash)
        });

        Ok(())
    }

    // TODO: Remove unnecessary `Result` return type.
    async fn get_channel_topic_and_hash<'a>(
        &'a mut self,
        channel: &Channel,
    ) -> Result<Option<(Topic, Hash)>, Error> {
        let channel_topic = self
            .channel_topics
            .read()
            .await
            .get(channel)
            .and_then(|topics| {
                topics
                    // Get the key-value pair with the largest timestamp.
                    .last_key_value()
                    // Ignore the key (timestamp); return the topic and hash.
                    .map(|(_, (topic, hash))| (topic.to_owned(), hash.to_owned()))
            });

        Ok(channel_topic)
    }

    /*
    async fn insert_latest_join_or_leave_post_hash(
        &mut self,
        public_key: &PublicKey,
        hash: Hash,
    ) -> Result<(), Error> {
        // Open the latest join or leave post store for writing.
        let mut latest_join_or_leave = self.latest_join_or_leave_post_hashes.write().await;
        latest_join_or_leave.insert(*public_key, hash);

        Ok(())
    }

    async fn get_latest_join_or_leave_post_hashes<'a>(&'a mut self) -> Result<Vec<Hash>, Error> {
        let hashes = self
            .latest_join_or_leave_post_hashes
            .read()
            .await
            .iter()
            .map(|(_public_key, hash)| hash.to_owned())
            .collect();

        Ok(hashes)
    }
    */

    async fn send_post_to_live_streams(&mut self, post: &Post, channel: &Channel) {
        if let Some(senders) = self.live_streams.read().await.get(channel) {
            for stream in senders.write().await.iter_mut() {
                if stream.matches(post) {
                    stream.send(post.clone()).await;
                }
            }
        }
    }

    async fn update_posts(
        &mut self,
        post: &Post,
        channel: &Channel,
        timestamp: &Timestamp,
        hash: Hash,
    ) {
        // Open the post store for writing.
        let mut posts = self.posts.write().await;

        // Retrieve the stored posts matching the given channel.
        if let Some(post_map) = posts.get_mut(channel) {
            // Retrieve the stored posts matching the given
            // timestamp.
            if let Some(posts) = post_map.get_mut(timestamp) {
                // Add the post to the vector of posts indexed
                // by the given timestamp.
                posts.push((post.clone(), hash));
            } else {
                // Insert the post and post hash (as a `Vec` of tuple)
                // into the `BTreeMap`, using the timestamp as the key.
                post_map.insert(*timestamp, vec![(post.clone(), hash)]);
            }
        } else {
            // No posts have previously been stored for the
            // given channel.

            let mut post_map = BTreeMap::new();
            // Insert the post (as a `Vec`) into the `BTreeMap`,
            // using the timestamp as the key.
            post_map.insert(*timestamp, vec![(post.clone(), hash)]);
            // Insert the `BTreeMap` into the posts `HashMap`,
            // using the channel name as the key.
            posts.insert(channel.to_owned(), post_map);
        }
    }

    async fn insert_name(&mut self, public_key: &PublicKey, name: &Nickname) {
        let mut peer_names = self.peer_names.write().await;
        peer_names.insert(*public_key, name.to_owned());
    }

    async fn get_name(&mut self, public_key: &PublicKey) -> Option<Nickname> {
        self.peer_names.read().await.get(public_key).cloned()
    }

    async fn insert_post(&mut self, post: &Post) -> Result<Hash, Error> {
        let timestamp = &post.get_timestamp();

        // Hash the post.
        let hash = post.hash()?;

        match &post.body {
            // TODO: Include matching arms for other post types.
            PostBody::Text { channel, text: _ } => {
                // Insert the post into the `posts` store.
                self.update_posts(post, channel, timestamp, hash).await;

                // Insert the binary payload of the post into the
                // `HashMap` of post data, indexed by the hash.
                self.post_payloads
                    .write()
                    .await
                    .insert(hash, post.to_bytes()?);

                self.send_post_to_live_streams(post, channel).await;
            }
            PostBody::Join { channel } => {
                let public_key = &post.get_public_key();

                self.update_channel_membership_hashes(channel, public_key, &hash)
                    .await?;
                self.insert_channel_member(channel, public_key).await?;

                // Insert the binary payload of the post into the
                // `HashMap` of post data, indexed by the hash.
                self.post_payloads
                    .write()
                    .await
                    .insert(hash, post.to_bytes()?);
            }
            PostBody::Leave { channel } => {
                let public_key = &post.get_public_key();

                self.update_channel_membership_hashes(channel, public_key, &hash)
                    .await?;
                self.remove_channel_member(channel, public_key).await?;

                // Insert the binary payload of the post into the
                // `HashMap` of post data, indexed by the hash.
                self.post_payloads
                    .write()
                    .await
                    .insert(hash, post.to_bytes()?);
            }
            PostBody::Topic { channel, topic } => {
                // Insert the post into the `posts` store.
                self.update_posts(post, channel, timestamp, hash).await;

                self.insert_channel_topic(channel, topic, timestamp, &hash)
                    .await?;

                // Insert the binary payload of the post into the
                // `HashMap` of post data, indexed by the hash.
                self.post_payloads
                    .write()
                    .await
                    .insert(hash, post.to_bytes()?);

                self.send_post_to_live_streams(post, channel).await;
            }
            PostBody::Delete { hashes } => {
                // Places / stores for checking and deletion:
                //
                // channel_topics
                for hash in hashes {
                    self.delete_post(hash).await?
                }

                // Insert the binary payload of the post into the
                // `HashMap` of post data, indexed by the hash.
                self.post_payloads
                    .write()
                    .await
                    .insert(hash, post.to_bytes()?);
            }
            _ => {}
        }

        let channel = post.get_channel();

        // Update the store of known channels.
        if let Some(channel) = channel {
            self.insert_channel(channel).await?;
        }

        Ok(hash)
    }

    // TODO: Consider splitting this into two separate methods:
    //
    // `remove_post()` and `remove_post_payload()`
    //
    // Do the same for `insert_post()` and `insert_post_payload()`.
    async fn remove_post(&mut self, hash: &Hash) -> Result<(), Error> {
        // Open the post store for writing.
        let mut posts = self.posts.write().await;

        // Iterate over all key-value pairs in the hash map.
        //
        // The `post_map` is a `BTreeMap`.
        posts.iter_mut().for_each(|(_channel, post_map)| {
            // Iterate over the key-value pairs of the post map.
            post_map.iter_mut().for_each(|(_timestamp, post_vec)| {
                // Remove any tuple from the vector for which the stored
                // hash matches the given hash.
                post_vec.retain(|(_post, stored_hash)| stored_hash != hash)
            })
        });

        // Open the post payloads store for writing.
        let mut post_payloads = self.post_payloads.write().await;

        // Remove any post payload for which the stored hash matches the given
        // hash.
        post_payloads.retain(|stored_hash, _payload| stored_hash != hash);

        Ok(())
    }

    async fn delete_post(&mut self, hash: &Hash) -> Result<(), Error> {
        // Remove post from all stores.

        // TODO: We have to check the author of the `post/delete` message
        // to ensure it matches the author of the target post before moving
        // ahead with deletion.

        self.remove_channel_topic(hash).await?;
        self.remove_channel_membership_hash(hash).await?;
        self.remove_post(hash).await?;

        Ok(())
    }

    async fn get_posts(&mut self, opts: &ChannelOptions) -> Result<PostStream, Error> {
        let posts = self
            .posts
            .write()
            .await
            .get(&opts.channel)
            // Return an empty map if no posts are found matching the given
            // channel.
            .unwrap_or(&self.empty_post_bt)
            .range(opts.time_start..opts.time_end)
            // Iterate over the post data and extract the post for each one,
            // wrapping it in a `Result`.
            .flat_map(|(_time, posts)| posts.iter().map(|(post, _hash)| Ok(post.clone())))
            .collect::<Vec<Result<Post, Error>>>();

        let post_stream = Box::new(stream::from_iter(posts.into_iter()));

        Ok(post_stream)
    }

    async fn get_posts_live(&mut self, opts: &ChannelOptions) -> Result<PostStream, Error> {
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

        // Retrieve all stored posts matching the channel options.
        let post_stream = self.get_posts(opts).await?;
        // Merge the existing post stream with the live post stream.
        let live_post_stream = Box::new(post_stream.merge(live_stream));

        Ok(live_post_stream)
    }

    async fn get_post_hashes(&mut self, opts: &ChannelOptions) -> Result<HashStream, Error> {
        let start = opts.time_start;
        let end = opts.time_end;
        let empty = self.empty_post_bt.range(..);

        let hashes = self
            .posts
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
            // Iterate over the post data and extract the hash for each one,
            // wrapping it in a `Result`.
            .flat_map(|(_time, posts)| posts.iter().map(|(_post, hash)| Ok(*hash)))
            .collect::<Vec<Result<Hash, Error>>>();

        let hash_stream = Box::new(stream::from_iter(hashes.into_iter()));

        Ok(hash_stream)
    }

    async fn get_post_payloads(&mut self, hashes: &[Hash]) -> Result<Vec<Payload>, Error> {
        let post_payloads = self.post_payloads.read().await;

        Ok(hashes
            .iter()
            .filter_map(|hash| post_payloads.get(hash))
            .cloned()
            .collect())
    }

    async fn want(&mut self, hashes: &[Hash]) -> Result<Vec<Hash>, Error> {
        let post_payloads = self.post_payloads.read().await;

        let wanted_hashes = hashes
            .iter()
            .filter(|hash| !post_payloads.contains_key(&(*hash).clone()))
            .cloned()
            .collect();

        Ok(wanted_hashes)
    }
}
