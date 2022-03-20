use crate::{Error,Post,PostBody,Channel,Hash,Payload,ChannelOptions};
use sodiumoxide::crypto;
use std::convert::TryInto;
use std::collections::{HashMap,BTreeMap};
use async_std::{
  prelude::*,
  stream::Stream,stream,channel,sync::{Arc,RwLock,Mutex},
  task,task::{Waker,Context,Poll},pin::Pin,
};
use desert::ToBytes;
pub type Keypair = ([u8;32],[u8;64]);
pub type GetPostOptions = ChannelOptions;
pub type PostStream<'a> = Box<dyn Stream<Item=Result<Post,Error>>+Unpin+Send+'a>;
pub type HashStream<'a> = Box<dyn Stream<Item=Result<Hash,Error>>+Unpin+Send+'a>;

#[derive(Clone)]
struct LiveStream {
  id: usize,
  options: ChannelOptions,
  sender: channel::Sender<Post>,
  receiver: channel::Receiver<Post>,
  live_streams: Arc<RwLock<Vec<Self>>>,
  waker: Arc<Mutex<Option<Waker>>>,
}

impl LiveStream {
  pub fn new(
    id: usize,
    options: ChannelOptions,
    live_streams: Arc<RwLock<Vec<Self>>>,
  ) -> Self {
    let (sender,receiver) = channel::bounded(options.limit);
    Self { id, options, sender, receiver, live_streams, waker: Arc::new(Mutex::new(None)) }
  }
  pub async fn send(&mut self, post: Post) {
    if let Err(_) = self.sender.try_send(post) {}
    if let Some(waker) = self.waker.lock().await.as_ref() {
      waker.wake_by_ref();
    }
  }
  pub fn matches(&self, post: &Post) -> bool {
    if Some(&self.options.channel) != post.get_channel() { return false }
    match (self.options.time_start, self.options.time_end) {
      (0,0) => true,
      (0,end) => post.get_timestamp().map(|t| t <= end).unwrap_or(false),
      (start,0) => post.get_timestamp().map(|t| start <= t).unwrap_or(false),
      (start,end) => post.get_timestamp().map(|t| start <= t && t <= end).unwrap_or(false),
    }
  }
}

impl Stream for LiveStream {
  type Item = Result<Post,Error>;
  fn poll_next(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Option<Self::Item>> {
    let r = Pin::new(&mut self.receiver.recv()).poll(ctx);
    match r {
      Poll::Ready(Ok(x)) => {
        let m_waker = self.waker.clone();
        task::block_on(async move { *m_waker.lock().await = None; });
        Poll::Ready(Some(Ok(x)))
      },
      Poll::Ready(Err(x)) => {
        let m_waker = self.waker.clone();
        task::block_on(async move { *m_waker.lock().await = None; });
        Poll::Ready(Some(Err(x.into())))
      },
      Poll::Pending => {
        let m_waker = self.waker.clone();
        let waker = ctx.waker().clone();
        task::block_on(async move { *m_waker.lock().await = Some(waker); });
        Poll::Pending
      },
    }
  }
}

impl Drop for LiveStream {
  fn drop(&mut self) {
    let live_streams = self.live_streams.clone();
    let id = self.id;
    task::block_on(async move {
      live_streams.write().await.drain_filter(|s| s.id == id);
    });
  }
}

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
  async fn get_posts<'a>(&'a mut self, opts: &GetPostOptions) -> Result<PostStream,Error>;
  async fn get_posts_live<'a>(&'a mut self, opts: &GetPostOptions) -> Result<PostStream,Error>;
  async fn get_post_hashes<'a>(&'a mut self, opts: &GetPostOptions) -> Result<HashStream,Error>;
  async fn want(&mut self, hashes: &[Hash]) -> Result<Vec<Hash>,Error>;
  async fn get_data(&mut self, hashes: &[Hash]) -> Result<Vec<Payload>,Error>;
}

#[derive(Clone)]
pub struct MemoryStore {
  keypair: Keypair,
  posts: Arc<RwLock<HashMap<Channel,BTreeMap<u64,Vec<Post>>>>>,
  post_hashes: Arc<RwLock<HashMap<Channel,BTreeMap<u64,Vec<Hash>>>>>,
  data: Arc<RwLock<HashMap<Hash,Payload>>>,
  empty_post_bt: BTreeMap<u64,Vec<Post>>,
  empty_hash_bt: BTreeMap<u64,Vec<Hash>>,
  live_streams: Arc<RwLock<HashMap<Channel,Arc<RwLock<Vec<LiveStream>>>>>>,
  live_stream_id: Arc<Mutex<usize>>,
}

impl Default for MemoryStore {
  fn default() -> Self {
    let (pk,sk) = crypto::sign::gen_keypair();
    Self {
      keypair: (
        pk.as_ref().try_into().unwrap(),
        sk.as_ref().try_into().unwrap()
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
  async fn get_keypair(&mut self) -> Result<Option<Keypair>,Error> {
    Ok(Some(self.keypair.clone()))
  }
  async fn set_keypair(&mut self, keypair: Keypair) -> Result<(),Error> {
    self.keypair = keypair;
    Ok(())
  }
  async fn get_latest_hash(&mut self, _channel: &[u8]) -> Result<[u8;32],Error> {
    // todo: actually use latest message if available instead of zeros
    Ok([0;32])
  }
  async fn insert_post(&mut self, post: &Post) -> Result<(),Error> {
    match &post.body {
      PostBody::Text { channel, timestamp, .. } => {
        {
          let mut posts = self.posts.write().await;
          if let Some(post_map) = posts.get_mut(channel) {
            if let Some(posts) = post_map.get_mut(timestamp) {
              posts.push(post.clone());
            } else {
              post_map.insert(*timestamp, vec![post.clone()]);
            }
          } else {
            let mut post_map = BTreeMap::new();
            post_map.insert(*timestamp, vec![post.clone()]);
            posts.insert(channel.to_vec(), post_map);
          }
        }
        {
          let mut post_hashes = self.post_hashes.write().await;
          if let Some(hash_map) = post_hashes.get_mut(channel) {
            if let Some(hashes) = hash_map.get_mut(timestamp) {
              hashes.push(post.hash()?);
            } else {
              let hash = post.hash()?;
              hash_map.insert(*timestamp, vec![hash.clone()]);
              self.data.write().await.insert(hash, post.to_bytes()?);
            }
          } else {
            let mut hash_map = BTreeMap::new();
            let hash = post.hash()?;
            hash_map.insert(*timestamp, vec![hash.clone()]);
            post_hashes.insert(channel.to_vec(), hash_map);
            self.data.write().await.insert(hash, post.to_bytes()?);
          }
        }
        if let Some(senders) = self.live_streams.read().await.get(channel) {
          for stream in senders.write().await.iter_mut() {
            if stream.matches(&post) {
              stream.send(post.clone()).await;
            }
          }
        }
      },
      _ => {},
    }
    Ok(())
  }
  async fn get_posts(&mut self, opts: &GetPostOptions) -> Result<PostStream,Error> {
    let posts = self.posts.write().await.get(&opts.channel)
      .unwrap_or(&self.empty_post_bt)
      .range(opts.time_start..opts.time_end)
      .flat_map(|(_time,posts)| posts.iter().map(|post| Ok(post.clone())))
      .collect::<Vec<Result<Post,Error>>>();
    Ok(Box::new(stream::from_iter(posts.into_iter())))
  }
  async fn get_posts_live(&mut self, opts: &GetPostOptions) -> Result<PostStream,Error> {
    let live_stream = {
      let mut live_streams = self.live_streams.write().await;
      if let Some(streams) = live_streams.get_mut(&opts.channel) {
        let live_stream = {
          let mut id = self.live_stream_id.lock().await;
          *id += 1;
          LiveStream::new(*id, opts.clone(), streams.clone())
        };
        let live = live_stream.clone();
        task::block_on(async move {
          streams.write().await.push(live);
        });
        live_stream
      } else {
        let streams = Arc::new(RwLock::new(vec![]));
        let live_stream_id = {
          let mut id_r = self.live_stream_id.lock().await;
          let id = *id_r;
          *id_r += 1;
          id
        };
        let streams_c = streams.clone();
        let live_stream = task::block_on(async move {
          let live_stream = LiveStream::new(live_stream_id, opts.clone(), streams_c.clone());
          streams_c.write().await.push(live_stream.clone());
          live_stream
        });
        live_streams.insert(opts.channel.clone(), streams);
        live_stream
      }
    };
    let post_stream = self.get_posts(opts).await?;
    Ok(Box::new(post_stream.merge(live_stream)))
  }
  async fn get_post_hashes(&mut self, opts: &GetPostOptions) -> Result<HashStream,Error> {
    let start = opts.time_start;
    let end = opts.time_end;
    let empty = self.empty_hash_bt.range(..);
    let hashes = self.post_hashes.read().await.get(&opts.channel)
      .map(|x| {
        match (start,end) {
          (0,0) => x.range(..),
          (0,end) => x.range(..end),
          (start,0) => x.range(start..),
          _ => x.range(start..end),
        }
      })
      .unwrap_or(empty)
      .flat_map(|(_time,hashes)| hashes.iter().map(|hash| Ok(*hash)))
      .collect::<Vec<Result<Hash,Error>>>();
    Ok(Box::new(stream::from_iter(hashes.into_iter())))
  }
  async fn want(&mut self, hashes: &[Hash]) -> Result<Vec<Hash>,Error> {
    let data = self.data.read().await;
    Ok(hashes.iter().filter(|hash| !data.contains_key(hash.clone())).cloned().collect())
  }
  async fn get_data(&mut self, hashes: &[Hash]) -> Result<Vec<Payload>,Error> {
    let data = self.data.read().await;
    Ok(hashes.iter().filter_map(|hash| data.get(hash)).cloned().collect())
  }
}
