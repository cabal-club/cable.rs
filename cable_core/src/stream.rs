//! Live stream data type and associated methods, along with an implementation
//! of the asynchronous `Stream` trait (`async_std`) for the `LiveStream` type.

use async_std::{
    channel,
    pin::Pin,
    prelude::*,
    stream::Stream,
    sync::{Arc, Mutex, RwLock},
    task,
    task::{Context, Poll, Waker},
};
use cable::{error::Error, post::Post, Hash};

use crate::ChannelOptions;

/// An asynchronous stream of posts.
pub type PostStream<'a> = Box<dyn Stream<Item = Result<Post, Error>> + Unpin + Send + 'a>;
/// An asynchronous stream of post hashes.
pub type HashStream<'a> = Box<dyn Stream<Item = Result<Hash, Error>> + Unpin + Send + 'a>;

#[derive(Clone)]
/// A live stream manager with a unique ID and channel parameters.
pub struct LiveStream {
    id: usize,
    options: ChannelOptions,
    sender: channel::Sender<Post>,
    receiver: channel::Receiver<Post>,
    live_streams: Arc<RwLock<Vec<Self>>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl LiveStream {
    /// Create a new `LiveStream` with the given channel options and streams.
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

    /// Send a post to the live stream manager.
    pub async fn send(&mut self, post: Post) {
        if self.sender.try_send(post).is_err() {}
        if let Some(waker) = self.waker.lock().await.as_ref() {
            waker.wake_by_ref();
        }
    }

    /// Check if the given post matches the channel parameters
    /// defined for the live stream manager.
    pub fn matches(&self, post: &Post) -> bool {
        if Some(&self.options.channel) != post.get_channel() {
            return false;
        }
        match (self.options.time_start, self.options.time_end) {
            (0, 0) => true,
            (0, end) => post.get_timestamp() <= end,
            (start, 0) => start <= post.get_timestamp(),
            (start, end) => {
                let timestamp = post.get_timestamp();
                start <= timestamp && timestamp <= end
            }
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
            // NOTE: `drain_filter()` would be more efficient to use here but
            // it's not currrently available on the stable API (nightly-only
            // experimental API only).
            let mut i = 0;
            while i < live_streams.read().await.len() {
                if live_streams.read().await[i].id == id {
                    let _stream = live_streams.write().await.remove(i);
                } else {
                    i += 1;
                }
            }
        });
    }
}
