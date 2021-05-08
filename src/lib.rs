pub type ReqID = [u8;4];
pub type Hash = [u8;32];
pub type Payload = Vec<u8>;
pub type Channel = Vec<u8>;

/*
use async_std::{prelude::*,sync::Arc,io,fs::File};
use leveldb::database::{Database,batch::{Batch,Writebatch}};
use leveldb::options::{Options,WriteOptions,ReadOptions};
use leveldb::iterator::{LevelDBIterator,Iterable};
use leveldb::kv::KV;
*/

mod message;
pub use message::*;

mod post;
pub use post::*;

mod varint;

//type Error = Box<dyn std::error::Error+Send+Sync>;

/*
type LDB = Arc<Database<Key>>;

#[derive(Clone)]
struct Key { pub data: Vec<u8> }
impl Key { fn from(key: Vec<u8>) -> Self { Key { data: key } } }
impl db_key::Key for Key {
  fn from_u8(key: &[u8]) -> Self { Key { data: key.into() } }
  fn as_slice<T, F: Fn(&[u8]) -> T>(&self, f: F) -> T { f(&self.data) }
}
*/
