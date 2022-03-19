#![feature(backtrace,async_closure,drain_filter)]

use async_std::{prelude::*,sync::{Arc,RwLock},channel,task};
use std::collections::{HashMap,HashSet};
use futures::{io::{AsyncRead,AsyncWrite}};
use desert::{ToBytes,FromBytes};
use std::convert::TryInto;

pub type ReqId = [u8;4];
pub type ReplyId = [u8;4];
pub type PeerId = usize;
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

#[derive(Clone,Debug,PartialEq)]
pub struct ChannelOptions {
  pub channel: Vec<u8>,
  pub time_start: u64,
  pub time_end: u64,
  pub limit: usize,
}
impl ChannelOptions {
  pub fn matches(&self, post: &Post) -> bool {
    if Some(&self.channel) != post.get_channel() { return false }
    match (self.time_start, self.time_end) {
      (0,0) => true,
      (0,end) => post.get_timestamp().map(|t| t <= end).unwrap_or(false),
      (start,0) => post.get_timestamp().map(|t| start <= t).unwrap_or(false),
      (start,end) => post.get_timestamp().map(|t| start <= t && t <= end).unwrap_or(false),
    }
  }
}

#[derive(Clone)]
pub struct Cable<S: Store> {
  pub store: S,
  peers: Arc<RwLock<HashMap<usize,channel::Sender<Message>>>>,
  next_peer_id: Arc<RwLock<usize>>,
  next_req_id: Arc<RwLock<u32>>,
  listening: Arc<RwLock<HashMap<PeerId,Vec<(ReqId,ChannelOptions)>>>>,
  requested: Arc<RwLock<HashSet<Hash>>>,
  open_requests: Arc<RwLock<HashMap<u32,Message>>>,
}

impl<S> Cable<S> where S: Store {
  pub fn new(store: S) -> Self {
    Self {
      store,
      peers: Arc::new(RwLock::new(HashMap::new())),
      next_peer_id: Arc::new(RwLock::new(0)),
      next_req_id: Arc::new(RwLock::new(0)),
      listening: Arc::new(RwLock::new(HashMap::new())),
      requested: Arc::new(RwLock::new(HashSet::new())),
      open_requests: Arc::new(RwLock::new(HashMap::new())),
    }
  }
}

impl<S> Cable<S> where S: Store {
  pub async fn post_text(&mut self, channel: &[u8], text: &[u8]) -> Result<(),Error> {
    let timestamp = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let post = Post {
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
    };
    self.post(post).await
  }
  pub async fn post(&mut self, mut post: Post) -> Result<(),Error> {
    if !post.is_signed() {
      post.sign(&self.get_secret_key().await?)?;
    }
    self.store.insert_post(&post).await?;
    for (peer_id,reqs) in self.listening.read().await.iter() {
      for (req_id,opts) in reqs {
        let n_limit = opts.limit.min(4096);
        let mut hashes = vec![];
        {
          let mut stream = self.store.get_post_hashes(&opts).await?;
          while let Some(result) = stream.next().await {
            hashes.push(result?);
            if hashes.len() >= n_limit { break }
          }
        }
        let response = Message::HashResponse { req_id: req_id.clone(), hashes };
        self.send(*peer_id, &response).await?;
      }
    }
    Ok(())
  }
  pub async fn broadcast(&self, message: &Message) -> Result<(),Error> {
    for ch in self.peers.read().await.values() {
      ch.send(message.clone()).await?;
    }
    Ok(())
  }
  pub async fn send(&self, peer_id: usize, msg: &Message) -> Result<(),Error> {
    if let Some(ch) = self.peers.read().await.get(&peer_id) {
      ch.send(msg.clone()).await?;
    }
    Ok(())
  }
  pub async fn handle(&mut self, peer_id: usize, msg: &Message) -> Result<(),Error> {
    println!["msg={:?}", msg];
    // todo: forward requests
    match msg {
      Message::ChannelTimeRangeRequest { req_id, channel, time_start, time_end, limit, .. } => {
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
            if hashes.len() >= n_limit { break }
          }
        }
        let response = Message::HashResponse { req_id: *req_id, hashes };
        {
          let mut w = self.listening.write().await;
          if let Some(listeners) = w.get_mut(&peer_id) {
            listeners.push((req_id.clone(),opts));
          } else {
            w.insert(peer_id, vec![(req_id.clone(),opts)]);
          }
        }
        self.send(peer_id, &response).await?;
      },
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
      },
      Message::HashRequest { req_id, ttl, hashes } => {
        let response = Message::DataResponse {
          req_id: *req_id,
          data: self.store.get_data(hashes).await?,
        };
        self.send(peer_id, &response).await?
      },
      Message::DataResponse { req_id, data } => {
        for buf in data {
          if !Post::verify(&buf) { continue }
          let (s,post) = Post::from_bytes(&buf)?;
          if s != buf.len() { continue }
          let h = post.hash()?;
          {
            let mut mreq = self.requested.write().await;
            if !mreq.contains(&h) { continue } // didn't request this response
            mreq.remove(&h);
          }
          self.store.insert_post(&post).await?;
        }
      },
      _ => {
        println!["other message type: todo"];
      },
    }
    Ok(())
  }
  async fn req_id(&self) -> (u32,ReqId) {
    let mut n = self.next_req_id.write().await;
    let r = *n;
    *n = if *n == u32::MAX { 0 } else { *n + 1 };
    (r,r.to_bytes().unwrap().try_into().unwrap())
  }
  pub async fn open_channel(&mut self, options: &ChannelOptions) -> Result<PostStream<'_>,Error> {
    let (req_id,req_id_bytes) = self.req_id().await;
    let m = Message::ChannelTimeRangeRequest {
      req_id: req_id_bytes,
      ttl: 1,
      channel: options.channel.to_vec(),
      time_start: options.time_start,
      time_end: options.time_end,
      limit: options.limit,
    };
    self.open_requests.write().await.insert(req_id, m.clone());
    self.broadcast(&m).await?;
    Ok(self.store.get_posts_live(options).await?)
  }
  pub async fn close_channel(&self, channel: &[u8]) {
    unimplemented![]
  }
  pub async fn get_peer_ids(&self) -> Vec<usize> {
    self.peers.read().await.keys().copied().collect::<Vec<usize>>()
  }
  pub async fn get_link(&mut self, channel: &[u8]) -> Result<[u8;32],Error> {
    let link = self.store.get_latest_hash(channel).await?;
    Ok(link)
  }
  pub async fn get_public_key(&mut self) -> Result<[u8;32],Error> {
    let (pk,_sk) = self.store.get_or_create_keypair().await?;
    Ok(pk)
  }
  pub async fn get_secret_key(&mut self) -> Result<[u8;64],Error> {
    let (_pk,sk) = self.store.get_or_create_keypair().await?;
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

    for msg in self.open_requests.read().await.values() {
      stream.write_all(&msg.to_bytes()?).await?;
    }

    let w = {
      let mut cstream = stream.clone();
      task::spawn(async move {
        while let Ok(msg) = recv.recv().await {
          println!["write {:?}", &msg];
          cstream.write_all(&msg.to_bytes().unwrap()).await.unwrap();
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
      let (_,msg) = Message::from_bytes(&buf)?;
      let mut this = self.clone();
      task::spawn(async move {
        if let Err(e) = this.handle(peer_id, &msg).await {
          eprintln!["{}", e];
        }
      });
    }

    w.await?;
    self.peers.write().await.remove(&peer_id);
    Ok(())
  }
}
