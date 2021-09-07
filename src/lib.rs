#![feature(backtrace)]

use async_std::{prelude::*,sync::{Arc,RwLock}};
use futures::io::{AsyncRead,AsyncWrite};
use desert::{FromBytes};

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
  store: Arc<RwLock<Box<S>>>,
}

impl<S> Cable<S> where S: Store {
  pub fn new(store: Box<S>) -> Self {
    Self { store: Arc::new(RwLock::new(store)) }
  }
  pub fn client(&self) -> Client<S> {
    Client { cable: Arc::new(RwLock::new((*self).clone())) }
  }
}

#[derive(Clone)]
pub struct Client<S: Store> {
  cable: Arc<RwLock<Cable<S>>>,
}

impl<S> Client<S> where S: Store {
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
