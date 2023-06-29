use crate::{ChannelOptions, Error, Hash, Post};
use async_std::{
    channel,
    pin::Pin,
    prelude::*,
    stream::Stream,
    sync::{Arc, Mutex, RwLock},
    task,
    task::{Context, Poll, Waker},
};

pub type PostStream<'a> = Box<dyn Stream<Item = Result<Post, Error>> + Unpin + Send + 'a>;
pub type HashStream<'a> = Box<dyn Stream<Item = Result<Hash, Error>> + Unpin + Send + 'a>;

#[derive(Clone)]
pub struct LiveStream {
    id: usize,
    options: ChannelOptions,
    sender: channel::Sender<Post>,
    receiver: channel::Receiver<Post>,
    live_streams: Arc<RwLock<Vec<Self>>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl LiveStream {
    pub fn new(id: usize, options: ChannelOptions, live_streams: Arc<RwLock<Vec<Self>>>) -> Self {
        let (sender, receiver) = channel::bounded(options.limit);
        Self {
            id,
            options,
            sender,
            receiver,
            live_streams,
            waker: Arc::new(Mutex::new(None)),
        }
    }
    pub async fn send(&mut self, post: Post) {
        if let Err(_) = self.sender.try_send(post) {}
        if let Some(waker) = self.waker.lock().await.as_ref() {
            waker.wake_by_ref();
        }
    }
    pub fn matches(&self, post: &Post) -> bool {
        if Some(&self.options.channel) != post.get_channel() {
            return false;
        }
        match (self.options.time_start, self.options.time_end) {
            (0, 0) => true,
            (0, end) => post.get_timestamp().map(|t| t <= end).unwrap_or(false),
            (start, 0) => post.get_timestamp().map(|t| start <= t).unwrap_or(false),
            (start, end) => post
                .get_timestamp()
                .map(|t| start <= t && t <= end)
                .unwrap_or(false),
        }
    }
}

impl Stream for LiveStream {
    type Item = Result<Post, Error>;
    fn poll_next(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Option<Self::Item>> {
        let r = Pin::new(&mut self.receiver.recv()).poll(ctx);
        match r {
            Poll::Ready(Ok(x)) => {
                let m_waker = self.waker.clone();
                task::block_on(async move {
                    *m_waker.lock().await = None;
                });
                Poll::Ready(Some(Ok(x)))
            }
            Poll::Ready(Err(x)) => {
                let m_waker = self.waker.clone();
                task::block_on(async move {
                    *m_waker.lock().await = None;
                });
                Poll::Ready(Some(Err(x.into())))
            }
            Poll::Pending => {
                let m_waker = self.waker.clone();
                let waker = ctx.waker().clone();
                task::block_on(async move {
                    *m_waker.lock().await = Some(waker);
                });
                Poll::Pending
            }
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
