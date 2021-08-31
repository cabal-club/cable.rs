#![feature(backtrace)]

use async_std::{prelude::*,task,sync::{Arc,RwLock}};
use futures::io::{AsyncRead,AsyncWrite};
use desert::{varint,FromBytes};
use std::marker::Unpin;

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

pub struct Cable<S: Store> {
  store: Arc<RwLock<Box<S>>>,
}

impl<S> Cable<S> where S: Store {
  pub fn new(store: Box<S>) -> Self {
    Self { store: Arc::new(RwLock::new(store)) }
  }
  pub fn connect<T: AsyncRead+AsyncWrite+Unpin+Send+'static>(&self, mut stream: Box<T>) {
    // todo: do this properly with a streams impl or whatever
    let store_c = self.store.clone();
    task::spawn(async move {
      let mut buf_offset = 0;
      let mut buf = vec![0u8; 100*1024];
      let mut o_msg_len = None;
      loop {
        let n = stream.read(&mut buf[buf_offset..]).await.unwrap();
        if n == 0 { break }
        if let Some((s,msg_len)) = o_msg_len {
          if msg_len+s <= buf_offset+n {
            Self::handle_buf(&buf[0..buf_offset+n], store_c.clone());
          } else {
            buf_offset += n;
          }
        }
        if let Ok((s,msg_len)) = varint::decode(&buf[0..buf_offset+n]) {
          o_msg_len = Some((s,msg_len as usize));
          if msg_len as usize + s <= buf_offset+n {
            Self::handle_buf(&buf[0..buf_offset+n], store_c.clone());
          } else {
            buf_offset += n;
          }
        } else {
          buf_offset += n;
        }
      }
    });
  }
  fn handle_buf(buf: &[u8], _store: Arc<RwLock<Box<S>>>) {
    let msg = Message::from_bytes(buf);
    println!["msg={:?}", &msg];
  }
}
