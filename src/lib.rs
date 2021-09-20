#![feature(backtrace)]

use async_std::{prelude::*,sync::{Arc,RwLock}};
use futures::io::AsyncRead;
use desert::{ToBytes,FromBytes};
use std::convert::TryInto;

pub type ReqID = [u8;4];
pub type Hash = [u8;32];
pub type Payload = Vec<u8>;
pub type Channel = Vec<u8>;
pub type Error = Box<dyn std::error::Error+Send+Sync>;

mod message;
pub use message::*;
mod post;
pub use post::*;
mod store;
pub use store::*;
mod error;
pub use error::*;
use length_prefixed_stream::{decode_with_options,DecodeOptions};

#[derive(Clone)]
pub struct Cable<S: Store> {
  pub store: Arc<RwLock<Box<S>>>,
}

impl<S> Cable<S> where S: Store {
  pub fn new(store: Box<S>) -> Self {
    Self { store: Arc::new(RwLock::new(store)) }
  }
  pub fn client(&self) -> Client<S> {
    Client {
      //cable: Arc::new(RwLock::new((*self).clone())),
      store: self.store.clone(),
      public_key: None,
    }
  }
}

#[derive(Clone)]
pub struct Client<S: Store> {
  //cable: Arc<RwLock<Cable<S>>>,
  store: Arc<RwLock<Box<S>>>,
  public_key: Option<[u8;32]>,
}

impl<S> Client<S> where S: Store {
  pub async fn post_text(&self, channel: &[u8], text: &[u8]) -> Result<(),Error> {
    let timestamp = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)?.as_secs();
    self.post(Post {
      header: PostHeader {
        public_key: self.get_public_key().await?,
        signature: [0;64],
        link: self.get_link(channel).await?,
      },
      body: PostBody::Text {
        channel: channel.to_vec(),
        timestamp,
        text: text.to_vec(),
      }
    }).await
  }
  pub async fn post(&self, post: Post) -> Result<(),Error> {
    let mut bytes = post.to_bytes()?;
    if !post.is_signed() {
      Post::sign(&mut bytes, &self.get_secret_key().await?)
    }
    // store in the db or something...
    panic!["store in the db or something goes here"]
  }
  pub async fn get_link(&self, channel: &[u8]) -> Result<[u8;32],Error> {
    let link = self.store.write().await.get_latest(channel).await?;
    Ok(link)
  }
  pub async fn get_public_key(&self) -> Result<[u8;32],Error> {
    let (pk,_sk) = self.store.write().await.get_or_create_keypair().await?;
    Ok(pk)
  }
  pub async fn get_secret_key(&self) -> Result<[u8;64],Error> {
    let (_pk,sk) = self.store.write().await.get_or_create_keypair().await?;
    Ok(sk)
  }
  pub async fn listen<T>(&self, input: T) -> Result<(),Error>
  where T: AsyncRead+Unpin+Send+Sync+'static {
    let mut options = DecodeOptions::default();
    options.include_len = true;
    let mut lps = decode_with_options(input, options);
    while let Some(rbuf) = lps.next().await {
      let buf = rbuf?;
      println!["buf={:?}", &buf];
      let msg = Message::from_bytes(&buf);
      println!["msg={:?}", &msg];
    }
    Ok(())
  }
}
