#![feature(backtrace)]

use async_std::{prelude::*,sync::{Arc,RwLock},channel,task};
use std::collections::HashMap;
use futures::io::{AsyncRead,AsyncWrite};
use desert::{ToBytes,FromBytes};

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

pub struct ChannelOptions {
  gt: Option<u64>,
  lt: Option<u64>,
  limit: u64,
}

#[derive(Clone)]
pub struct Cable<S: Store> {
  store: Arc<RwLock<Box<S>>>,
  peers: Arc<RwLock<HashMap<usize,channel::Sender<Message>>>>,
  next_peer_id: Arc<RwLock<usize>>,
  listening: Arc<RwLock<HashMap<Vec<u8>,Vec<ChannelOptions>>>>,
}

impl<S> Cable<S> where S: Store {
  pub fn new(store: Box<S>) -> Self {
    Self {
      store: Arc::new(RwLock::new(store)),
      peers: Arc::new(RwLock::new(HashMap::new())),
      next_peer_id: Arc::new(RwLock::new(0)),
      listening: Arc::new(RwLock::new(HashMap::new())),
    }
  }
  pub fn client(&self) -> Client<S> {
    Client {
      //cable: Arc::new(RwLock::new((*self).clone())),
      store: self.store.clone(),
      peers: self.peers.clone(),
      next_peer_id: self.next_peer_id.clone(),
      listening: self.listening.clone(),
    }
  }
}

#[derive(Clone)]
pub struct Client<S: Store> {
  //cable: Arc<RwLock<Cable<S>>>,
  store: Arc<RwLock<Box<S>>>,
  listening: Arc<RwLock<HashMap<Vec<u8>,Vec<ChannelOptions>>>>,
  peers: Arc<RwLock<HashMap<usize,channel::Sender<Message>>>>,
  next_peer_id: Arc<RwLock<usize>>,
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
  pub async fn post(&self, mut post: Post) -> Result<(),Error> {
    if !post.is_signed() {
      post.sign(&self.get_secret_key().await?)?;
    }
    self.store.write().await.insert_post(&post).await?;
    Ok(())
  }
  pub async fn broadcast(&self, message: &Message) -> Result<(),Error> {
    for ch in self.peers.read().await.values() {
      ch.send(message.clone()).await?;
    }
    Ok(())
  }
  pub async fn send(&self, peer_id: usize, message: &Message) -> Result<(),Error> {
    if let Some(ch) = self.peers.read().await.get(&peer_id) {
      ch.send(message.clone()).await?;
    }
    Ok(())
  }
  pub async fn get_peer_ids(&self) -> Vec<usize> {
    self.peers.read().await.keys().copied().collect::<Vec<usize>>()
  }
  pub async fn get_link(&self, channel: &[u8]) -> Result<[u8;32],Error> {
    let link = self.store.write().await.get_latest_hash(channel).await?;
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
  pub async fn listen<T>(&self, mut stream: T) -> Result<(),Error>
  where T: AsyncRead+AsyncWrite+Clone+Unpin+Send+Sync+'static {
    let peer_id = {
      let mut n = self.next_peer_id.write().await;
      let peer_id = *n;
      *n += 1;
      peer_id
    };
    let (send,recv) = channel::bounded(100);
    self.peers.write().await.insert(peer_id, send);
    let w = {
      let mut cstream = stream.clone();
      task::spawn(async move {
        while let Ok(msg) = recv.recv().await {
          cstream.write_all(&msg.to_bytes()?);
        }
        let res: Result<(),Error> = Ok(());
        res
      })
    };

    let mut options = DecodeOptions::default();
    options.include_len = true;
    let mut lps = decode_with_options(stream, options);
    while let Some(rbuf) = lps.next().await {
      let buf = rbuf?;
      println!["buf={:?}", &buf];
      let msg = Message::from_bytes(&buf);
      println!["msg={:?}", &msg];
    }

    w.await?;
    Ok(())
  }
}
