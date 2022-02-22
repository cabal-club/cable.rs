use crate::{Error,Post,PostBody,Channel,Hash,Payload,ChannelOptions};
use sodiumoxide::crypto;
use std::convert::TryInto;
use std::collections::{HashMap,BTreeMap};
use async_std::{stream::Stream,stream};
use desert::ToBytes;
pub type Keypair = ([u8;32],[u8;64]);
pub type GetPostOptions = ChannelOptions;

#[async_trait::async_trait]
pub trait Store: Clone+Send+Sync+Unpin+'static {
  async fn get_keypair(&mut self) -> Result<Option<Keypair>,Error>;
  async fn set_keypair(&mut self, keypair: Keypair) -> Result<(),Error>;
  async fn get_or_create_keypair(&mut self) -> Result<Keypair,Error> {
    if let Some(kp) = self.get_keypair().await? {
      Ok(kp)
    } else {
      let (pk,sk) = crypto::sign::gen_keypair();
      let kp = (
        pk.as_ref().try_into().unwrap(),
        sk.as_ref().try_into().unwrap()
      );
      self.set_keypair(kp.clone()).await?;
      Ok(kp)
    }
  }
  async fn get_latest_hash(&mut self, channel: &[u8]) -> Result<[u8;32],Error>;
  async fn insert_post(&mut self, post: &Post) -> Result<(),Error>;
  fn get_posts(
    &mut self, opts: &GetPostOptions
  ) -> Box<dyn Stream<Item=Result<Post,Error>>+Unpin+Send+'_>;
  fn get_post_hashes(
    &mut self, opts: &GetPostOptions
  ) -> Box<dyn Stream<Item=Result<Hash,Error>>+Unpin+Send+'_>;
  async fn want(&mut self, hashes: &[Hash]) -> Result<Vec<Hash>,Error>;
  async fn get_data(&mut self, hashes: &[Hash]) -> Result<Vec<Payload>,Error>;
}

#[derive(Clone)]
pub struct MemoryStore {
  keypair: Keypair,
  posts: HashMap<Channel,BTreeMap<u64,Vec<Post>>>,
  post_hashes: HashMap<Channel,BTreeMap<u64,Vec<Hash>>>,
  data: HashMap<Hash,Payload>,
  empty_post_bt: BTreeMap<u64,Vec<Post>>,
  empty_hash_bt: BTreeMap<u64,Vec<Hash>>,
}

impl Default for MemoryStore {
  fn default() -> Self {
    let (pk,sk) = crypto::sign::gen_keypair();
    Self {
      keypair: (
        pk.as_ref().try_into().unwrap(),
        sk.as_ref().try_into().unwrap()
      ),
      posts: HashMap::new(),
      post_hashes: HashMap::new(),
      data: HashMap::new(),
      empty_post_bt: BTreeMap::new(),
      empty_hash_bt: BTreeMap::new(),
    }
  }
}

#[async_trait::async_trait]
impl Store for MemoryStore {
  async fn get_keypair(&mut self) -> Result<Option<Keypair>,Error> {
    Ok(Some(self.keypair.clone()))
  }
  async fn set_keypair(&mut self, keypair: Keypair) -> Result<(),Error> {
    self.keypair = keypair;
    Ok(())
  }
  async fn get_latest_hash(&mut self, channel: &[u8]) -> Result<[u8;32],Error> {
    // todo: actually use latest message if available instead of zeros
    Ok([0;32])
  }
  async fn insert_post(&mut self, post: &Post) -> Result<(),Error> {
    println!["insert {:?}", post];
    match &post.body {
      PostBody::Text { channel, timestamp, .. } => {
        if let Some(post_map) = self.posts.get_mut(channel) {
          if let Some(posts) = post_map.get_mut(timestamp) {
            posts.push(post.clone());
          } else {
            post_map.insert(*timestamp, vec![post.clone()]);
          }
        } else {
          let mut post_map = BTreeMap::new();
          post_map.insert(*timestamp, vec![post.clone()]);
          self.posts.insert(channel.to_vec(), post_map);
        }
        if let Some(hash_map) = self.post_hashes.get_mut(channel) {
          if let Some(hashes) = hash_map.get_mut(timestamp) {
            hashes.push(post.hash()?);
          } else {
            let hash = post.hash()?;
            hash_map.insert(*timestamp, vec![hash.clone()]);
            self.data.insert(hash, post.to_bytes()?);
          }
        } else {
          let mut hash_map = BTreeMap::new();
          let hash = post.hash()?;
          hash_map.insert(*timestamp, vec![hash.clone()]);
          self.post_hashes.insert(channel.to_vec(), hash_map);
          self.data.insert(hash, post.to_bytes()?);
        }
      },
      _ => {},
    }
    Ok(())
  }
  fn get_posts(
    &mut self, opts: &GetPostOptions
  ) -> Box<dyn Stream<Item=Result<Post,Error>>+Unpin+Send+'_> {
    let post_iter = self.posts.get(&opts.channel)
      .unwrap_or(&self.empty_post_bt)
      .range(opts.time_start..opts.time_end)
      .flat_map(|(_time,posts)| posts.iter().map(|post| Ok(post.clone())));
    Box::new(stream::from_iter(post_iter))
  }
  fn get_post_hashes(
    &mut self, opts: &GetPostOptions
  ) -> Box<dyn Stream<Item=Result<Hash,Error>>+Unpin+Send+'_> {
    let start = opts.time_start;
    let end = opts.time_end;
    let empty = self.empty_hash_bt.range(..);
    let hash_iter = self.post_hashes.get(&opts.channel)
      .map(|x| {
        match (start,end) {
          (0,0) => x.range(..),
          (0,end) => x.range(..end),
          (start,0) => x.range(start..),
          _ => x.range(start..end),
        }
      })
      .unwrap_or(empty)
      .flat_map(|(_time,hashes)| hashes.iter().map(|hash| Ok(*hash)));
    Box::new(stream::from_iter(hash_iter))
  }
  async fn want(&mut self, hashes: &[Hash]) -> Result<Vec<Hash>,Error> {
    Ok(hashes.iter().filter(|hash| !self.data.contains_key(hash.clone())).cloned().collect())
  }
  async fn get_data(&mut self, hashes: &[Hash]) -> Result<Vec<Payload>,Error> {
    Ok(hashes.iter().filter_map(|hash| self.data.get(hash)).cloned().collect())
  }
}
