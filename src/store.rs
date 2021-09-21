use crate::{Error,Post,PostBody};
use sodiumoxide::crypto;
use std::convert::TryInto;
use std::collections::{HashMap,BTreeMap};
pub type Keypair = ([u8;32],[u8;64]);

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
}

#[derive(Clone)]
pub struct MemoryStore {
  keypair: Keypair,
  posts: HashMap<Vec<u8>,BTreeMap<u64,Post>>,
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
    match &post.body {
      PostBody::Text { channel, timestamp, .. } => {
        if let Some(posts) = self.posts.get_mut(channel) {
          posts.insert(*timestamp, post.clone());
        } else {
          let mut posts = BTreeMap::new();
          posts.insert(*timestamp, post.clone());
          self.posts.insert(channel.to_vec(), posts);
        }
      },
      _ => {},
    }
    Ok(())
  }
}
