use desert::{FromBytes,ToBytes,CountBytes,varint};
use crate::{Error,Hash,Channel,error::CableErrorKind as E};
use sodiumoxide::crypto;

#[derive(Clone,Debug)]
pub struct Post {
  pub header: PostHeader,
  pub body: PostBody,
}

#[derive(Clone,Debug)]
pub struct PostHeader {
  pub public_key: [u8;32],
  pub signature: [u8;64],
  pub link: [u8;32],
}

#[derive(Clone,Debug)]
pub enum PostBody {
  Text {
    channel: Channel,
    timestamp: u64,
    text: Vec<u8>,
  },
  Delete {
    timestamp: u64,
    hash: Hash,
  },
  Info {
    timestamp: u64,
    key: Vec<u8>,
    value: Vec<u8>,
  },
  Topic {
    channel: Channel,
    timestamp: u64,
    topic: Vec<u8>,
  },
  Join {
    channel: Channel,
    timestamp: u64,
  },
  Leave {
    channel: Channel,
    timestamp: u64,
  },
  Unrecognized {
    post_type: u64
  },
}

impl Post {
  pub fn post_type(&self) -> u64 {
    match &self.body {
      PostBody::Text { .. } => 0,
      PostBody::Delete { .. } => 1,
      PostBody::Info { .. } => 2,
      PostBody::Topic { .. } => 3,
      PostBody::Join { .. } => 4,
      PostBody::Leave { .. } => 5,
      PostBody::Unrecognized { post_type } => *post_type,
    }
  }
  pub fn verify(_buf: &[u8]) -> bool {
    unimplemented![]
  }
  pub fn sign(&mut self, secret_key: &[u8;64]) -> Result<(),Error> {
    let buf = self.to_bytes()?;
    let sk = crypto::sign::SecretKey::from_slice(secret_key).unwrap();
    // todo: return NoneError
    self.header.signature = crypto::sign::sign_detached(&buf[32+64..], &sk).to_bytes();
    Ok(())
  }
  pub fn is_signed(&self) -> bool {
    for i in 0..self.header.signature.len() {
      if self.header.signature[i] != 0 {
        return true;
      }
    }
    return false;
  }
}

impl CountBytes for Post {
  fn count_bytes(&self) -> usize {
    let post_type = self.post_type();
    let header_size = 32 + 64 + 32;
    let body_size = varint::length(post_type) + match &self.body {
      PostBody::Text { channel, timestamp, text } => {
        varint::length(channel.len() as u64) + channel.len()
          + varint::length(*timestamp)
          + varint::length(text.len() as u64) + text.len()
      },
      PostBody::Delete { timestamp, hash } => {
        varint::length(*timestamp) + hash.len()
      },
      PostBody::Info { timestamp, key, value } => {
        varint::length(*timestamp)
          + varint::length(key.len() as u64) + key.len()
          + varint::length(value.len() as u64) + value.len()
      },
      PostBody::Topic { channel, timestamp, topic } => {
        varint::length(channel.len() as u64) + channel.len()
          + varint::length(*timestamp)
          + varint::length(topic.len() as u64) + topic.len()
      },
      PostBody::Join { channel, timestamp } => {
        varint::length(channel.len() as u64) + channel.len() + varint::length(*timestamp)
      },
      PostBody::Leave { channel, timestamp } => {
        varint::length(channel.len() as u64) + channel.len() + varint::length(*timestamp)
      },
      PostBody::Unrecognized { .. } => 0,
    };
    header_size + body_size
  }
  fn count_from_bytes(_buf: &[u8]) -> Result<usize,Error> {
    unimplemented![]
  }
}

impl ToBytes for Post {
  fn to_bytes(&self) -> Result<Vec<u8>,Error> {
    let mut buf = vec![0;self.count_bytes()];
    self.write_bytes(&mut buf)?;
    Ok(buf)
  }
  fn write_bytes(&self, buf: &mut [u8]) -> Result<usize,Error> {
    let mut offset = 0;
    assert_eq![self.header.public_key.len(), 32];
    assert_eq![self.header.signature.len(), 64];
    assert_eq![self.header.link.len(), 32];
    buf[offset..offset+32].copy_from_slice(&self.header.public_key);
    offset += self.header.public_key.len();
    buf[offset..offset+64].copy_from_slice(&self.header.signature);
    offset += self.header.signature.len();
    buf[offset..offset+32].copy_from_slice(&self.header.link);
    offset += self.header.link.len();
    offset += varint::encode(self.post_type(), &mut buf[offset..])?;
    match &self.body {
      PostBody::Text { channel, timestamp, text } => {
        offset += varint::encode(channel.len() as u64, &mut buf[offset..])?;
        buf[offset..offset+channel.len()].copy_from_slice(channel);
        offset += channel.len();
        offset += varint::encode(*timestamp, &mut buf[offset..])?;
        offset += varint::encode(text.len() as u64, &mut buf[offset..])?;
        buf[offset..offset+text.len()].copy_from_slice(text);
        offset += text.len();
      },
      PostBody::Delete { timestamp, hash } => {
        offset += varint::encode(*timestamp, &mut buf[offset..])?;
        buf[offset..offset+hash.len()].copy_from_slice(hash);
        offset += hash.len();
      },
      PostBody::Info { timestamp, key, value } => {
        offset += varint::encode(*timestamp, &mut buf[offset..])?;
        offset += varint::encode(key.len() as u64, &mut buf[offset..])?;
        buf[offset..offset+key.len()].copy_from_slice(key);
        offset += key.len();
        offset += varint::encode(value.len() as u64, &mut buf[offset..])?;
        buf[offset..offset+value.len()].copy_from_slice(value);
        offset += value.len();
      },
      PostBody::Topic { channel, timestamp, topic } => {
        offset += varint::encode(channel.len() as u64, &mut buf[offset..])?;
        buf[offset..offset+channel.len()].copy_from_slice(channel);
        offset += channel.len();
        offset += varint::encode(*timestamp, &mut buf[offset..])?;
        offset += varint::encode(topic.len() as u64, &mut buf[offset..])?;
        buf[offset..offset+topic.len()].copy_from_slice(topic);
        offset += topic.len();
      },
      PostBody::Join { channel, timestamp } => {
        offset += varint::encode(channel.len() as u64, &mut buf[offset..])?;
        buf[offset..offset+channel.len()].copy_from_slice(channel);
        offset += channel.len();
        offset += varint::encode(*timestamp, &mut buf[offset..])?;
      },
      PostBody::Leave { channel, timestamp } => {
        offset += varint::encode(channel.len() as u64, &mut buf[offset..])?;
        buf[offset..offset+channel.len()].copy_from_slice(channel);
        offset += channel.len();
        offset += varint::encode(*timestamp, &mut buf[offset..])?;
      },
      PostBody::Unrecognized { post_type } => {
        return E::PostWriteUnrecognizedType { post_type: *post_type }.raise();
      },
    }
    Ok(offset)
  }
}

impl FromBytes for Post {
  fn from_bytes(buf: &[u8]) -> Result<(usize,Self),Error> {
    let mut offset = 0;
    let header = {
      let mut public_key = [0;32];
      public_key.copy_from_slice(&buf[offset..offset+32]);
      offset += 32;
      let mut signature = [0;64];
      signature.copy_from_slice(&buf[offset..offset+64]);
      offset += 64;
      let mut link = [0;32];
      link.copy_from_slice(&buf[offset..offset+32]);
      offset += 32;
      PostHeader { public_key, signature, link }
    };
    let (s,post_type) = varint::decode(&buf[offset..])?;
    offset += s;
    let body = match post_type {
      0 => {
        let (s,channel_len) = varint::decode(&buf[offset..])?;
        offset += s;
        let channel = buf[offset..offset+channel_len as usize].to_vec();
        offset += channel_len as usize;
        let (s,timestamp) = varint::decode(&buf[offset..])?;
        offset += s;
        let (s,text_len) = varint::decode(&buf[offset..])?;
        offset += s;
        let text = buf[offset..offset+text_len as usize].to_vec();
        offset += text_len as usize;
        PostBody::Text { channel, timestamp, text }
      },
      1 => {
        let (s,timestamp) = varint::decode(&buf[offset..])?;
        offset += s;
        let mut hash = [0;32];
        hash.copy_from_slice(&buf[offset..offset+32]);
        offset += 32;
        PostBody::Delete { timestamp, hash }
      },
      2 => {
        let (s,timestamp) = varint::decode(&buf[offset..])?;
        offset += s;
        let (s,key_len) = varint::decode(&buf[offset..])?;
        offset += s;
        let key = buf[offset..offset+key_len as usize].to_vec();
        offset += key_len as usize;
        let (s,value_len) = varint::decode(&buf[offset..])?;
        offset += s;
        let value = buf[offset..offset+value_len as usize].to_vec();
        offset += value_len as usize;
        PostBody::Info { timestamp, key, value }
      },
      3 => {
        let (s,channel_len) = varint::decode(&buf[offset..])?;
        offset += s;
        let channel = buf[offset..offset+channel_len as usize].to_vec();
        offset += channel_len as usize;
        let (s,timestamp) = varint::decode(&buf[offset..])?;
        offset += s;
        let (s,topic_len) = varint::decode(&buf[offset..])?;
        offset += s;
        let topic = buf[offset..offset+topic_len as usize].to_vec();
        offset += topic_len as usize;
        PostBody::Topic { channel, timestamp, topic }
      },
      4 => {
        let (s,channel_len) = varint::decode(&buf[offset..])?;
        offset += s;
        let channel = buf[offset..offset+channel_len as usize].to_vec();
        offset += channel_len as usize;
        let (s,timestamp) = varint::decode(&buf[offset..])?;
        offset += s;
        PostBody::Join { channel, timestamp }
      },
      5 => {
        let (s,channel_len) = varint::decode(&buf[offset..])?;
        offset += s;
        let channel = buf[offset..offset+channel_len as usize].to_vec();
        offset += channel_len as usize;
        let (s,timestamp) = varint::decode(&buf[offset..])?;
        offset += s;
        PostBody::Leave { channel, timestamp }
      },
      post_type => {
        PostBody::Unrecognized { post_type }
      },
    };
    Ok((offset, Post { header, body }))
  }
}
