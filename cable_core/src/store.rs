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
    Channel, ChannelOptions, Error, Hash, Nickname, Payload, Timestamp, Topic, UserInfo,
};
use desert::{FromBytes, ToBytes};
use sodiumoxide::crypto;

use crate::stream::{HashStream, LiveStream, PostStream};

/// A public key.
pub type PublicKey = [u8; 32];

/// A public-private keypair.
pub type Keypair = ([u8; 32], [u8; 64]);

/// A `HashMap` of live streams with a key of channel name and a value
/// of a `Vec` of streams (wrapped in an `Arc` and `RwLock`).
pub type LiveStreamMap = HashMap<Channel, Arc<RwLock<Vec<LiveStream>>>>;

/// A `HashMap` of peer names with a key of public key and a value of a
/// `BTreeMap`. The `BTreeMap` has a key of timestamp and a value of a tuple
/// of name and hash. The hash is of the `post/info` post which defined the
/// stored name.
pub type NameHashMap = HashMap<PublicKey, BTreeMap<Timestamp, (Nickname, Hash)>>;

/// A `HashMap` of posts with a key of an option-enclosed channel name and a
/// value of a `BTreeMap`. The `BTreeMap` has a key of timestamp and value of
/// a `Vec` of tuple with post and post hash.
///
/// The key is an `Option` to allow for storage and retrieved of post types
/// which do not have an associated channel; these posts are stored with a
/// key of `None`.
pub type PostMap = HashMap<Option<Channel>, BTreeMap<Timestamp, Vec<(Post, Hash)>>>;

/// A `HashMap` of channel topics with a key of channel name and a value of a
/// `BTreeMap`. The `BTreeMap` has a key of timestamp and a value of a tuple
/// of topic and hash. The hash is of the `post/topic` post which defined the
/// stored topic.
pub type TopicHashMap = HashMap<Channel, BTreeMap<Timestamp, (Topic, Hash)>>;

#[async_trait::async_trait]
/// Storage trait with methods for storing and retrieving cryptographic
/// keypairs, hashes and posts.
pub trait Store: Clone + Send + Sync + Unpin + 'static {
    // TODO: Getters do not need a mutable reference to self.
    //
    /// Retrieve the keypair associated with the store.
    async fn get_keypair(&self) -> Option<Keypair>;

    /// Define the keypair associated with the store.
    async fn set_keypair(&mut self, keypair: Keypair);

    /// Retrieve the keypair associated with the store, creating a new keypair
    /// if one does not yet exist.
    async fn get_or_create_keypair(&mut self) -> Keypair {
        if let Some(kp) = self.get_keypair().await {
            kp
        } else {
            let (pk, sk) = crypto::sign::gen_keypair();
            let kp = (
                pk.as_ref().try_into().unwrap(),
                sk.as_ref().try_into().unwrap(),
            );
            self.set_keypair(kp).await;
            kp
        }
    }

    /// Retrieve all channels from the store.
    async fn get_channels(&self) -> Option<Vec<Channel>>;

    /// Insert the given channel into the store.
    async fn insert_channel(&mut self, channel: &Channel);

    /// Retrieve all members of the given channel.
    async fn get_channel_members(&self, channel: &Channel) -> Option<Vec<PublicKey>>;

    /// Insert the given public key into the store using the key defined by the
    /// given channel.
    async fn insert_channel_member(&mut self, channel: &Channel, public_key: &PublicKey);

    /// Query whether the given public key is a member of the given channel.
    async fn is_channel_member(&self, channel: &Channel, public_key: &PublicKey) -> bool;

    /// Remove the given public key from the membership store using the key
    /// defined by the given channel.
    async fn remove_channel_member(&mut self, channel: &Channel, public_key: &PublicKey);

    /// Retrieve all of the latest `post/join` or `post/leave` post hashes
    /// for the given channel.
    async fn get_channel_membership_hashes(&self, channel: &Channel) -> Option<Vec<Hash>>;

    /// Remove the channel membership data for the given post hash.
    async fn remove_channel_membership_hash(&mut self, hash: &Hash);

    /// Update the membership store with the hash of the latest `post/join` or
    /// `post/leave` post made to the given channel by the given public key.
    async fn update_channel_membership_hashes(
        &mut self,
        channel: &Channel,
        public_key: &PublicKey,
        hash: &Hash,
    );

    /// Retrieve all ex-members of the given channel.
    async fn get_ex_channel_members(&self, channel: &Channel) -> Option<Vec<PublicKey>>;

    /// Insert the given public key into the store using the key defined by the
    /// given channel.
    async fn insert_ex_channel_member(&mut self, channel: &Channel, public_key: &PublicKey);

    /// Remove the given public key from the ex-membership store using the key
    /// defined by the given channel.
    async fn remove_ex_channel_member(&mut self, channel: &Channel, public_key: &PublicKey);

    /// Retrieve the latest `post/topic` topic and hash for the given channel.
    async fn get_channel_topic_and_hash(&self, channel: &Channel) -> Option<(Topic, Hash)>;

    /// Insert the given channel topic, timestamp and hash into the store if
    /// the timestamp is later than the timestamp of the stored topic post.
    async fn insert_channel_topic(
        &mut self,
        channel: &Channel,
        topic: &Topic,
        timestamp: &Timestamp,
        hash: &Hash,
    );

    /// Remove the channel topic data for the given post hash.
    async fn remove_channel_topic(&mut self, hash: &Hash);

    /// Retrieve the hashes of all known delete posts authored by the given
    /// public key.
    async fn get_delete_hashes(&self, public_key: &PublicKey) -> Option<Vec<Hash>>;

    /// Insert the given delete post hash into the store using the key defined
    /// by the given public key.
    async fn insert_delete_hash(&mut self, public_key: &PublicKey, hash: &Hash);

    /// Retrieve the hashes of all known info posts authored by the given
    /// public key.
    async fn get_info_hashes(&self, public_key: &PublicKey) -> Option<Vec<Hash>>;

    /// Insert the given info post hash into the store using the key defined by
    /// the given public key.
    async fn insert_info_hash(&mut self, public_key: &PublicKey, hash: &Hash);

    /// Remove the info post data for the given post hash.
    async fn remove_info_hash(&mut self, hash: &Hash);

    /// Retrieve the hash(es) of the most recently published post(s) in the
    /// given channel.
    ///
    /// More than one hash will be returned if several posts were
    /// made to the same channel at the same time. Usually though, only one
    /// hash or no hashes will be returned.
    async fn get_latest_hashes(&self, channel: &Channel) -> Option<Vec<Hash>>;

    /// Retrieve the latest `post/info` name and hash for the given public key.
    async fn get_peer_name_and_hash(&self, public_key: &PublicKey) -> Option<(Nickname, Hash)>;

    /// Insert the given nickname, timestamp and hash into the store if the
    /// timestamp is later than the timestamp of the stored topic post.
    async fn insert_peer_name(
        &mut self,
        public_key: &PublicKey,
        name: &Nickname,
        timestamp: &Timestamp,
        hash: &Hash,
    );

    /// Remove the peer name data for the given post hash.
    async fn remove_peer_name(&mut self, hash: &Hash);

    /// Retrieve all posts matching the parameters defined by the given
    /// `ChannelOptions`.
    async fn get_posts(&self, opts: &ChannelOptions) -> PostStream;

    /// Retrieve all posts matching the parameters defined by the given
    /// `ChannelOptions` and continue to return new messages as they become
    /// available (stream remains active).
    async fn get_posts_live<'a>(&'a mut self, opts: &ChannelOptions) -> PostStream;

    /// Retrieve the hashes of all posts matching the parameters defined by the
    /// given `ChannelOptions`.
    async fn get_post_hashes(&self, opts: &ChannelOptions) -> HashStream;

    /// Insert the given post into the store and return the hash.
    async fn insert_post(&mut self, post: &Post) -> Result<Hash, Error>;

    /// Remove the given post from the posts and post hashes stores.
    async fn remove_post(&mut self, hash: &Hash);

    /// Delete the given post from all stores, leaving no trace.
    ///
    /// This method combines several removal methods to achieve complete
    /// removal of the post.
    async fn delete_post(&mut self, hash: &Hash);

    /// Update the posts store by inserting the given post.
    ///
    /// This method is more specific than `insert_post()`. It updates only
    /// the `posts` store, whereas `insert_post()` is responsible for updating
    /// multiple stores based on the post type.
    async fn update_posts(
        &mut self,
        post: &Post,
        channel: Option<Channel>,
        timestamp: &Timestamp,
        hash: Hash,
    );

    /// Retrieve the post payload for the post represented by the given hash.
    async fn get_post_payload(&self, hash: &Hash) -> Option<Payload>;

    /// Retrieve the post payloads for all posts represented by the given hashes.
    async fn get_post_payloads(&self, hashes: &[Hash]) -> Vec<Payload>;

    /// Insert the given hash and post payload into the store.
    async fn insert_post_payload(&mut self, hash: &Hash, payload: Payload);

    /// Remove the given post from the post payloads store.
    async fn remove_post_payload(&mut self, hash: &Hash);

    /// Send the given post to each live stream for which the channel option
    /// criteria are satisfied.
    async fn send_post_to_live_streams(&self, post: &Post, channel: &Channel);

    /// Retrieve the hashes of all posts representing the subset of the given
    /// hashes for which post data is not available locally (ie. the hashes of
    /// all posts which are not already in the store).
    async fn want(&self, hashes: &[Hash]) -> Vec<Hash>;
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
    /// The public keys of all ex-members, indexed by channel.
    ///
    /// This map is updated according to received / published `post/join`
    /// and `post/leave` posts.
    ex_channel_members: Arc<RwLock<HashMap<Channel, Vec<PublicKey>>>>,
    /// The hash of the latest `post/join` or `post/leave` post for each known
    /// peer, indexed by channel (the outer key) and public key (the first
    /// element of the tuple).
    channel_membership: Arc<RwLock<HashMap<Channel, HashMap<PublicKey, Hash>>>>,
    /// The topic, timestamp and hash of the latest `post/topic` post for each
    /// known channel, indexed by channel.
    channel_topics: Arc<RwLock<TopicHashMap>>,
    /// The hashes of all known `post/delete` posts.
    delete_hashes: Arc<RwLock<HashMap<PublicKey, Vec<Hash>>>>,
    /// The hashes of all known `post/info` posts.
    info_hashes: Arc<RwLock<HashMap<PublicKey, Vec<Hash>>>>,
    /// The nickname, timestamp and hash of the latest `post/info` post for
    /// each known peer, indexed by public key.
    peer_names: Arc<RwLock<NameHashMap>>,
    /// All posts and hashes in the store divided according to channel (the
    /// outer key) and indexed by timestamp (the inner key).
    posts: Arc<RwLock<PostMap>>,
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
                pk.as_ref().try_into().unwrap(),
                sk.as_ref().try_into().unwrap(),
            ),
            channels: Arc::new(RwLock::new(BTreeSet::new())),
            channel_members: Arc::new(RwLock::new(HashMap::new())),
            ex_channel_members: Arc::new(RwLock::new(HashMap::new())),
            channel_membership: Arc::new(RwLock::new(HashMap::new())),
            channel_topics: Arc::new(RwLock::new(HashMap::new())),
            delete_hashes: Arc::new(RwLock::new(HashMap::new())),
            info_hashes: Arc::new(RwLock::new(HashMap::new())),
            peer_names: Arc::new(RwLock::new(HashMap::new())),
            posts: Arc::new(RwLock::new(HashMap::new())),
            post_payloads: Arc::new(RwLock::new(HashMap::new())),
            empty_post_bt: BTreeMap::new(),
            live_streams: Arc::new(RwLock::new(HashMap::new())),
            live_stream_id: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait::async_trait]
impl Store for MemoryStore {
    async fn get_keypair(&self) -> Option<Keypair> {
        Some(self.keypair)
    }

    async fn set_keypair(&mut self, keypair: Keypair) {
        self.keypair = keypair;
    }

    async fn get_channels(&self) -> Option<Vec<Channel>> {
        let channels = self.channels.read().await;

        if channels.is_empty() {
            None
        } else {
            Some(channels.iter().cloned().collect())
        }
    }

    async fn insert_channel(&mut self, channel: &Channel) {
        let mut channel_store = self.channels.write().await;
        channel_store.insert(channel.to_owned());
    }

    async fn get_channel_members(&self, channel: &Channel) -> Option<Vec<PublicKey>> {
        self.channel_members
            .read()
            .await
            .get(channel)
            .map(|member| member.to_owned())
    }

    async fn insert_channel_member(&mut self, channel: &Channel, public_key: &PublicKey) {
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
    }

    async fn is_channel_member(&self, channel: &Channel, public_key: &PublicKey) -> bool {
        if let Some(channel_members) = self.get_channel_members(channel).await {
            channel_members.contains(public_key)
        } else {
            false
        }
    }

    async fn remove_channel_member(&mut self, channel: &Channel, public_key: &PublicKey) {
        // Open the channel members store for writing.
        let mut channel_members = self.channel_members.write().await;
        // Retrieve the stored members matching the given channel.
        if let Some(members) = channel_members.get_mut(channel) {
            // Iterate over the members and retain only those for which the
            // member does not match the given public key.
            members.retain(|member| member != public_key);
        }
    }

    async fn get_channel_membership_hashes(&self, channel: &Channel) -> Option<Vec<Hash>> {
        self.channel_membership
            .read()
            .await
            .get(channel)
            .map(|members| {
                members
                    // Retrieve the hash for each entry in the hash map.
                    .values()
                    .cloned()
                    .collect()
            })
    }

    async fn remove_channel_membership_hash(&mut self, hash: &Hash) {
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
    }

    async fn update_channel_membership_hashes(
        &mut self,
        channel: &Channel,
        public_key: &PublicKey,
        hash: &Hash,
    ) {
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
    }

    async fn get_ex_channel_members(&self, channel: &Channel) -> Option<Vec<PublicKey>> {
        self.ex_channel_members
            .read()
            .await
            .get(channel)
            .map(|member| member.to_owned())
    }

    async fn insert_ex_channel_member(&mut self, channel: &Channel, public_key: &PublicKey) {
        // Open the ex-channel members store for writing.
        let mut ex_channel_members = self.ex_channel_members.write().await;
        // Retrieve the stored ex-members matching the given channel.
        if let Some(ex_members) = ex_channel_members.get_mut(channel) {
            // Add the public key to the vector of public keys indexed by the
            // given channel.
            ex_members.push(public_key.to_owned())
        } else {
            // Insert the channel into the hash map, using the
            // given public key to create the value vec.
            ex_channel_members.insert(channel.to_owned(), vec![*public_key]);
        }
    }

    async fn remove_ex_channel_member(&mut self, channel: &Channel, public_key: &PublicKey) {
        // Open the ex-channel members store for writing.
        let mut ex_channel_members = self.ex_channel_members.write().await;
        // Retrieve the stored ex-members matching the given channel.
        if let Some(ex_members) = ex_channel_members.get_mut(channel) {
            // Iterate over the ex-members and retain only those for which the
            // member does not match the given public key.
            ex_members.retain(|ex_member| ex_member != public_key);
        }
    }

    async fn get_channel_topic_and_hash(&self, channel: &Channel) -> Option<(Topic, Hash)> {
        self.channel_topics
            .read()
            .await
            .get(channel)
            .and_then(|topics| {
                topics
                    // Get the key-value pair with the largest timestamp.
                    .last_key_value()
                    // Ignore the key (timestamp); return the topic and hash.
                    .map(|(_, (topic, hash))| (topic.to_owned(), hash.to_owned()))
            })
    }

    async fn insert_channel_topic(
        &mut self,
        channel: &Channel,
        topic: &Topic,
        timestamp: &Timestamp,
        hash: &Hash,
    ) {
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
    }

    async fn remove_channel_topic(&mut self, hash: &Hash) {
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
    }

    async fn get_delete_hashes(&self, public_key: &PublicKey) -> Option<Vec<Hash>> {
        self.delete_hashes
            .read()
            .await
            .get(public_key)
            .map(|hashes| hashes.to_owned())
    }

    async fn insert_delete_hash(&mut self, public_key: &PublicKey, hash: &Hash) {
        // Open the delete hashes store for writing.
        let mut delete_hashes = self.delete_hashes.write().await;
        // Retrieve the stored hashes matching the given public key.
        if let Some(hashes) = delete_hashes.get_mut(public_key) {
            // Add the hash to the vector of hashes indexed by the
            // given public key.
            hashes.push(hash.to_owned())
        } else {
            // Insert the public key into the hash map, using the
            // given hash to create the value vec.
            delete_hashes.insert(public_key.to_owned(), vec![*hash]);
        }
    }

    async fn get_info_hashes(&self, public_key: &PublicKey) -> Option<Vec<Hash>> {
        self.info_hashes
            .read()
            .await
            .get(public_key)
            .map(|hashes| hashes.to_owned())
    }

    async fn insert_info_hash(&mut self, public_key: &PublicKey, hash: &Hash) {
        // Open the info hashes store for writing.
        let mut info_hashes = self.info_hashes.write().await;
        // Retrieve the stored hashes matching the given public key.
        if let Some(hashes) = info_hashes.get_mut(public_key) {
            // Add the hash to the vector of hashes indexed by the
            // given public key.
            hashes.push(hash.to_owned())
        } else {
            // Insert the public key into the hash map, using the
            // given hash to create the value vec.
            info_hashes.insert(public_key.to_owned(), vec![*hash]);
        }
    }

    async fn remove_info_hash(&mut self, hash: &Hash) {
        let mut info_hashes = self.info_hashes.write().await;

        info_hashes
            .iter_mut()
            .for_each(|(_public_key, hashes)| hashes.retain(|stored_hash| stored_hash != hash));
    }

    async fn get_latest_hashes(&self, channel: &Channel) -> Option<Vec<Hash>> {
        // Open the posts store for reading.
        let posts_map = self.posts.read().await;

        // Get the BTree associated with the given channel.
        if let Some(posts_btree) = posts_map.get(&Some(channel.to_owned())) {
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

    async fn get_peer_name_and_hash(&self, public_key: &PublicKey) -> Option<(Nickname, Hash)> {
        self.peer_names
            .read()
            .await
            .get(public_key)
            .and_then(|names| {
                names
                    // Get the key-value pair with the largest timestamp.
                    .last_key_value()
                    // Ignore the key (timestamp); return the name and hash.
                    .map(|(_, (name, hash))| (name.to_owned(), hash.to_owned()))
            })
    }

    async fn insert_peer_name(
        &mut self,
        public_key: &PublicKey,
        name: &Nickname,
        timestamp: &Timestamp,
        hash: &Hash,
    ) {
        let mut peer_names = self.peer_names.write().await;
        // Retrieve the stored tuple of name, timestamp and hash matching the
        // given public key.
        if let Some(name_map) = peer_names.get_mut(public_key) {
            // Insert the given name and hash into the map, using the
            // timestamp as the key.
            name_map.insert(*timestamp, (name.to_owned(), *hash));
        } else {
            // No name data has previously been stored for the
            // given public key.

            let mut name_map = BTreeMap::new();
            // Insert the name data into the `BTreeMap`, using the timestamp
            // as the key.
            name_map.insert(*timestamp, (name.to_owned(), *hash));
            // Insert the `BTreeMap` into the public key names `HashMap`,
            // using the public key as the key.
            peer_names.insert(public_key.to_owned(), name_map);
        }
    }

    async fn remove_peer_name(&mut self, hash: &Hash) {
        // Open the peer names store for writing.
        let mut peer_names = self.peer_names.write().await;

        // Iterate over all key-value pairs in the hash map.
        //
        // The `name_map` is a `BTreeMap`.
        peer_names.iter_mut().for_each(|(_public_key, name_map)| {
            // Remove any key-value pair for which the stored hash of the name
            // post matches the given hash.
            name_map.retain(|_timestamp, (_name, stored_hash)| stored_hash != hash)
        });
    }

    async fn get_posts(&self, opts: &ChannelOptions) -> PostStream {
        // Retrieve all posts matching the given channel options.
        let mut posts = self
            .posts
            .read()
            .await
            .get(&Some(opts.channel.to_owned()))
            // Return an empty map if no posts are found matching the given
            // channel.
            .unwrap_or(&self.empty_post_bt)
            .range(opts.time_start..opts.time_end)
            // Iterate over the post data and extract the post for each one,
            // wrapping it in a `Result`.
            .flat_map(|(_time, posts)| posts.iter().map(|(post, _hash)| Ok(post.clone())))
            .collect::<Vec<Result<Post, Error>>>();

        // TODO: Would it be better to split this into another method?
        // It will result in inefficient sending here; all non-channel posts
        // will be sent for each open channel (as a result of
        // `get_posts_live()` being called for each new channel).
        //
        // Retrieve all posts which do not have a channel field.
        // For example, `post/info` posts.
        let non_channel_posts = self
            .posts
            .read()
            .await
            .get(&None)
            .unwrap_or(&self.empty_post_bt)
            .iter()
            .flat_map(|(_time, posts)| posts.iter().map(|(post, _hash)| Ok(post.clone())))
            .collect::<Vec<Result<Post, Error>>>();

        // Add the non-channel posts to the channel posts.
        posts.extend(non_channel_posts);

        // Return a post stream.
        Box::new(stream::from_iter(posts.into_iter()))
    }

    async fn get_posts_live(&mut self, opts: &ChannelOptions) -> PostStream {
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

        // Retrieve all stored posts matching the channel options,
        // as well as all non-channel posts.
        let post_stream = self.get_posts(opts).await;

        // Merge the existing post stream with the live post stream.
        Box::new(post_stream.merge(live_stream))
    }

    async fn get_post_hashes(&self, opts: &ChannelOptions) -> HashStream {
        let start = opts.time_start;
        let end = opts.time_end;
        let empty = self.empty_post_bt.range(..);

        let hashes = self
            .posts
            .read()
            .await
            .get(&Some(opts.channel.to_owned()))
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

        // Return a hash stream.
        Box::new(stream::from_iter(hashes.into_iter()))
    }

    async fn insert_post(&mut self, post: &Post) -> Result<Hash, Error> {
        let timestamp = &post.get_timestamp();

        let hash = post.hash()?;

        match &post.body {
            PostBody::Text { channel, text: _ } => {
                // Insert the post into the `posts` store.
                self.update_posts(post, Some(channel.to_owned()), timestamp, hash)
                    .await;
                self.insert_post_payload(&hash, post.to_bytes()?).await;
                self.send_post_to_live_streams(post, channel).await;
            }
            PostBody::Join { channel } => {
                let public_key = &post.get_public_key();

                self.update_channel_membership_hashes(channel, public_key, &hash)
                    .await;
                self.insert_channel_member(channel, public_key).await;
                self.remove_ex_channel_member(channel, public_key).await;
                self.insert_post_payload(&hash, post.to_bytes()?).await;
            }
            PostBody::Leave { channel } => {
                let public_key = &post.get_public_key();

                self.update_channel_membership_hashes(channel, public_key, &hash)
                    .await;
                self.remove_channel_member(channel, public_key).await;
                self.insert_ex_channel_member(channel, public_key).await;
                self.insert_post_payload(&hash, post.to_bytes()?).await;
            }
            PostBody::Topic { channel, topic } => {
                // Insert the post into the `posts` store.
                self.update_posts(post, Some(channel.to_owned()), timestamp, hash)
                    .await;
                self.insert_channel_topic(channel, topic, timestamp, &hash)
                    .await;
                self.insert_post_payload(&hash, post.to_bytes()?).await;
                self.send_post_to_live_streams(post, channel).await;
            }
            PostBody::Delete { hashes } => {
                let public_key = &post.get_public_key();

                for post_hash in hashes {
                    if let Some(payload) = self.get_post_payload(post_hash).await {
                        // TODO: Consider whether it is more efficient to
                        // decode the payload or retrieve the post from the
                        // `posts` store.
                        let (_s, stored_post) = Post::from_bytes(&payload)?;
                        // Only delete the post if the author matches the
                        // author of the `post/delete` post.
                        if post.get_public_key() == stored_post.get_public_key() {
                            // Delete the post from all stores.
                            self.delete_post(post_hash).await;
                            // The hash of the `post/delete` post is inserted,
                            // not the hash of the post referenced by the
                            // `post/delete` post.
                            self.insert_delete_hash(public_key, &hash).await;
                        }
                    }
                }

                self.insert_post_payload(&hash, post.to_bytes()?).await;
            }
            PostBody::Info { info } => {
                // Insert the post into the `posts` store.
                self.update_posts(post, None, timestamp, hash).await;

                let public_key = &post.get_public_key();

                // Insert the public key of the post author and the assigned
                // name if the key of the info element is "name".
                for UserInfo { key, val } in info {
                    if key == "name" {
                        self.insert_peer_name(public_key, val, timestamp, &hash)
                            .await;
                    }
                }

                self.insert_info_hash(public_key, &hash).await;
                self.insert_post_payload(&hash, post.to_bytes()?).await;
            }
            _ => {}
        }

        let channel = post.get_channel();

        // Update the store of known channels.
        if let Some(channel) = channel {
            self.insert_channel(channel).await;
        }

        Ok(hash)
    }

    async fn remove_post(&mut self, hash: &Hash) {
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
    }

    async fn delete_post(&mut self, hash: &Hash) {
        // Remove post from all stores.
        self.remove_channel_topic(hash).await;
        self.remove_channel_membership_hash(hash).await;
        self.remove_peer_name(hash).await;
        self.remove_info_hash(hash).await;
        self.remove_post(hash).await;
        self.remove_post_payload(hash).await;
    }

    async fn update_posts(
        &mut self,
        post: &Post,
        channel: Option<Channel>,
        timestamp: &Timestamp,
        hash: Hash,
    ) {
        // Open the post store for writing.
        let mut posts = self.posts.write().await;

        // Retrieve the stored posts matching the given channel.
        if let Some(post_map) = posts.get_mut(&channel) {
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
            posts.insert(channel, post_map);
        }
    }

    async fn get_post_payload(&self, hash: &Hash) -> Option<Payload> {
        let post_payloads = self.post_payloads.read().await;
        let post_payload = post_payloads.get(hash);

        post_payload.cloned()
    }

    async fn get_post_payloads(&self, hashes: &[Hash]) -> Vec<Payload> {
        let post_payloads = self.post_payloads.read().await;

        hashes
            .iter()
            .filter_map(|hash| post_payloads.get(hash))
            .cloned()
            .collect()
    }

    async fn insert_post_payload(&mut self, hash: &Hash, payload: Payload) {
        self.post_payloads.write().await.insert(*hash, payload);
    }

    async fn remove_post_payload(&mut self, hash: &Hash) {
        // Open the post payloads store for writing.
        let mut post_payloads = self.post_payloads.write().await;

        // Remove any post payload for which the stored hash matches the given
        // hash.
        post_payloads.retain(|stored_hash, _payload| stored_hash != hash);
    }

    async fn send_post_to_live_streams(&self, post: &Post, channel: &Channel) {
        if let Some(senders) = self.live_streams.read().await.get(channel) {
            for stream in senders.write().await.iter_mut() {
                if stream.matches(post) {
                    stream.send(post.clone()).await;
                }
            }
        }
    }

    async fn want(&self, hashes: &[Hash]) -> Vec<Hash> {
        let post_payloads = self.post_payloads.read().await;

        // Return the "wanted" hashes.
        hashes
            .iter()
            .filter(|hash| !post_payloads.contains_key(&(*hash).clone()))
            .cloned()
            .collect()
    }
}
