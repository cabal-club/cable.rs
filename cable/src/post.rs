/*
use crate::{error::CableErrorKind as E, Channel, Error, Hash};
use desert::{varint, CountBytes, FromBytes, ToBytes};
use sodiumoxide::crypto;
use std::convert::TryInto;
*/

//! Post formats for all post types supported by cable.
//!
//! Includes type definitions for all post types, as well as post header and
//! body types. Helper methods are included.

use desert::{varint, CountBytes, FromBytes, ToBytes};
use sodiumoxide::crypto::{
    sign,
    sign::{PublicKey, SecretKey, Signature},
};

use crate::{
    error::{CableErrorKind, Error},
    Channel, ChannelLen, Hash, Text, Topic,
};

#[derive(Clone, Debug)]
/// Information self-published by a user.
pub struct UserInfo {
    pub key_len: u64, // varint
    pub key: Vec<u8>,
    pub val_len: u64, // varint
    pub val: Vec<u8>,
}

#[derive(Clone, Debug)]
/// The length and data of an encoded post.
pub struct EncodedPost {
    /// The length of the post in bytes.
    pub post_len: u64, // varint
    /// The post data.
    pub post_data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Post {
    pub header: PostHeader,
    pub body: PostBody,
}

// TODO: think about appropriate integer sizes.
// E.g. Should `num_links` and `post_type` be `u64` or smaller?

#[derive(Clone, Debug)]
/// The header of a post.
pub struct PostHeader {
    /// Public key that authored this post.
    pub public_key: [u8; 32],
    /// Signature of the fields that follow.
    pub signature: [u8; 64],
    /// Number of hashes this post links back to (0+).
    // TODO: Consider removing `varint` comments; may be confusing.
    // These fields are eventually encoded as `varint` but are not expressed in
    // that form when decoded.
    pub num_links: u64, // varint
    /// Hashes of the latest posts in this channel/context.
    // NOTE: I would prefer to represent this field as `Vec<Hash>`.
    // That results in a `Vec<[u8; 32]>`, which needs to be flattened when
    // copying to a buffer. `.flatten()` exists for this purpose but is
    // currently only available on nightly (unstable).
    // Using a `Vec<u8>` for now. See if there is another way.
    //pub links: Vec<Hash>,
    pub links: Vec<u8>,
    /// Post type.
    pub post_type: u64, // varint
    /// Time at which the post was created (in milliseconds since the UNIX Epoch).
    pub timestamp: u64, // varint
}

// TODO: remember to write validators for post type data.
// E.g. "A topic field MUST be a valid UTF-8 string, between 0 and 512 codepoints."

#[derive(Clone, Debug)]
/// The body of a post.
pub enum PostBody {
    /// Post a chat message to a channel.
    Text {
        /// Length of the channel's name in bytes.
        channel_len: ChannelLen, // varint
        /// Channel name (UTF-8).
        channel: Channel,
        /// Length of the text field in bytes.
        text_len: u64, // varint
        /// Chat message text (UTF-8).
        text: Text,
    },
    /// Request that peers encountering this post delete the referenced posts
    /// from their local storage, and not store the referenced posts in the future.
    Delete {
        /// Number of posts to be deleted (specified by number of hashes).
        num_deletions: u64, // varint
        /// Concatenated hashes of posts to be deleted.
        // NOTE: I would prefer to represent this field as `Vec<Hash>`.
        // That results in a `Vec<[u8; 32]>`, which needs to be flattened when
        // copying to a buffer. `.flatten()` exists for this purpose but is
        // currently only available on nightly (unstable).
        // Using a `Vec<u8>` for now. See if there is another way.
        //hashes: Vec<Hash>,
        hashes: Vec<u8>,
    },
    /// Set public information about oneself.
    Info {
        /// The complete description of a user's self-published information.
        info: Vec<UserInfo>,
    },
    /// Set a topic for a channel.
    Topic {
        /// Length of the channel's name in bytes.
        channel_len: ChannelLen, // varint
        /// Channel name (UTF-8).
        channel: Channel,
        /// Length of the topic field in bytes.
        topic_len: u64, // varint
        /// Topic content (UTF-8).
        topic: Topic,
    },
    /// Publicly announce membership in a channel.
    Join {
        /// Length of the channel's name in bytes.
        channel_len: ChannelLen, // varint
        /// Channel name (UTF-8).
        channel: Channel,
    },
    /// Publicly announce termination of membership in a channel.
    Leave {
        /// Length of the channel's name in bytes.
        channel_len: ChannelLen, // varint
        /// Channel name (UTF-8).
        channel: Channel,
    },
    /// A post type which is not recognised as part of the cable specification.
    Unrecognized { post_type: u64 },
}

impl Post {
    /// Return the channel name associated with a post.
    pub fn get_channel(&self) -> Option<&Channel> {
        match &self.body {
            PostBody::Text { channel, .. } => Some(channel),
            PostBody::Delete { .. } => None,
            PostBody::Info { .. } => None,
            PostBody::Topic { channel, .. } => Some(channel),
            PostBody::Join { channel, .. } => Some(channel),
            PostBody::Leave { channel, .. } => Some(channel),
            PostBody::Unrecognized { .. } => None,
        }
    }

    /// Return the numeric type identifier for the post.
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

    pub fn sign(&mut self, secret_key: &[u8; 64]) -> Result<(), Error> {
        let buf = self.to_bytes()?;
        let sk = SecretKey::from_slice(secret_key).unwrap();
        // todo: return NoneError
        self.header.signature = sign::sign_detached(&buf[32 + 64..], &sk).to_bytes();
        Ok(())
    }

    /// Verify the signature of an encoded post.
    pub fn verify(buf: &[u8]) -> bool {
        // Since the public key is 32 bytes and the signature is 64 bytes,
        // a valid message must be greater than 32 + 64 bytes.
        if buf.len() < 32 + 64 {
            return false;
        }

        let public_key = PublicKey::from_slice(&buf[0..32]);
        let signature = Signature::from_bytes(&buf[32..32 + 64]);

        match (public_key, signature) {
            (Some(pk), Ok(sig)) => sign::verify_detached(&sig, &buf[32 + 64..], &pk),
            _ => false,
        }
    }
}

impl ToBytes for Post {
    /// Convert a `Post` data type to bytes.
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0; self.count_bytes()];
        self.write_bytes(&mut buf)?;
        Ok(buf)
    }

    /// Write bytes to the given buffer (mutable byte array).
    fn write_bytes(&self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut offset = 0;

        // Validate the length of the public key, signature and links fields.
        assert_eq![self.header.public_key.len(), 32];
        assert_eq![self.header.signature.len(), 64];
        // The links field should be an exact multiple of 32 (32 bytes per
        // link).
        assert_eq![self.header.links.len() % 32, 0];

        /* POST HEADER BYTES */

        // Write the public key bytes to the buffer and increment the offset.
        buf[offset..offset + 32].copy_from_slice(&self.header.public_key);
        offset += self.header.public_key.len();

        // Write the signature bytes to the buffer and increment the offset.
        buf[offset..offset + 64].copy_from_slice(&self.header.signature);
        offset += self.header.signature.len();

        // Encode num_links as a varint, write the resulting bytes to the
        // buffer and increment the offset.
        offset += varint::encode(self.header.num_links, &mut buf[offset..])?;

        // Write the links bytes to the buffer and increment the offset.
        //
        // NOTE: I feel unsure about the correctness here.
        // We might be adding the encoded bytes twice...
        // Check with tests.
        //offset += varint::encode(self.header.links.len() as u64, &mut buf[offset..])?;
        buf[offset..offset + self.header.links.len()].copy_from_slice(&self.header.links);

        // Encode the post type as a varint, write the resulting bytes to the
        // buffer and increment the offset.
        offset += varint::encode(self.post_type(), &mut buf[offset..])?;

        // Encode the timestamp as a varint, write the resulting bytes to the
        // buffer and increment the offset.
        offset += varint::encode(self.header.timestamp, &mut buf[offset..])?;

        /* POST BODY BYTES */

        match &self.body {
            PostBody::Text {
                channel_len,
                channel,
                text_len,
                text,
            } => {
                offset += varint::encode(*channel_len, &mut buf[offset..])?;
                buf[offset..offset + channel.len()].copy_from_slice(channel);
                offset += channel.len();

                offset += varint::encode(*text_len, &mut buf[offset..])?;
                buf[offset..offset + text.len()].copy_from_slice(text);
                offset += text.len();
            }
            PostBody::Delete {
                num_deletions,
                hashes,
            } => {
                offset += varint::encode(*num_deletions, &mut buf[offset..])?;
                buf[offset..offset + hashes.len()].copy_from_slice(&hashes);
                offset += hashes.len();
            }
            PostBody::Info { info } => {
                for UserInfo {
                    key_len,
                    key,
                    val_len,
                    val,
                } in info
                {
                    offset += varint::encode(*key_len, &mut buf[offset..])?;
                    offset += varint::encode(key.len() as u64, &mut buf[offset..])?;
                    buf[offset..offset + key.len()].copy_from_slice(key);
                    offset += key.len();
                    offset += varint::encode(*val_len, &mut buf[offset..])?;
                    // TODO: Check that this line is necessary; may be made
                    // redundant by previous LOC.
                    offset += varint::encode(val.len() as u64, &mut buf[offset..])?;
                    buf[offset..offset + val.len()].copy_from_slice(val);
                    offset += val.len();
                }
            }
            PostBody::Topic {
                channel_len,
                channel,
                topic_len,
                topic,
            } => {
                offset += varint::encode(*channel_len, &mut buf[offset..])?;
                buf[offset..offset + channel.len()].copy_from_slice(channel);
                offset += channel.len();
                offset += varint::encode(*topic_len, &mut buf[offset..])?;
                // TODO: Check that this line is necessary; may be made
                // redundant by previous LOC.
                offset += varint::encode(topic.len() as u64, &mut buf[offset..])?;
                buf[offset..offset + topic.len()].copy_from_slice(topic);
                offset += topic.len();
            }
            PostBody::Join {
                channel_len,
                channel,
            } => {
                offset += varint::encode(*channel_len, &mut buf[offset..])?;
                buf[offset..offset + channel.len()].copy_from_slice(channel);
                offset += channel.len();
            }
            PostBody::Leave {
                channel_len,
                channel,
            } => {
                offset += varint::encode(*channel_len, &mut buf[offset..])?;
                buf[offset..offset + channel.len()].copy_from_slice(channel);
                offset += channel.len();
            }
            PostBody::Unrecognized { post_type } => {
                return CableErrorKind::PostWriteUnrecognizedType {
                    post_type: *post_type,
                }
                .raise();
            }
        }
        Ok(offset)
    }
}

impl CountBytes for Post {
    fn count_bytes(&self) -> usize {
        let post_type = self.post_type();

        // Count the post header bytes.
        let header_size = 32 // Public key.
            + 64 // Signature.
            + varint::length(self.header.num_links) // Number of links.
            + self.header.links.len() // Links.
            + varint::length(post_type) // Post type.
            + varint::length(self.header.timestamp); // Timestamp.

        // Count the post body bytes.
        let body_size = match &self.body {
            PostBody::Text {
                channel_len,
                channel,
                text_len,
                text,
            } => {
                varint::length(*channel_len)
                    + channel.len()
                    + varint::length(*text_len)
                    + text.len()
            }
            PostBody::Delete {
                num_deletions,
                hashes,
            } => varint::length(*num_deletions) + hashes.len(),
            PostBody::Info { info } => {
                let mut info_len = 0;

                for UserInfo {
                    key_len,
                    key,
                    val_len,
                    val,
                } in info
                {
                    info_len +=
                        varint::length(*key_len) + key.len() + varint::length(*val_len) + val.len();
                }

                info_len
            }
            PostBody::Topic {
                channel_len,
                channel,
                topic_len,
                topic,
            } => {
                varint::length(*channel_len)
                    + channel.len()
                    + varint::length(*topic_len)
                    + topic.len()
            }
            PostBody::Join {
                channel_len,
                channel,
            } => varint::length(*channel_len) + channel.len(),
            PostBody::Leave {
                channel_len,
                channel,
            } => varint::length(*channel_len) + channel.len(),
            PostBody::Unrecognized { .. } => 0,
        };

        header_size + body_size
    }

    fn count_from_bytes(_buf: &[u8]) -> Result<usize, Error> {
        unimplemented![]
    }
}

/*
impl CountBytes for Post {
    fn count_bytes(&self) -> usize {
        let post_type = self.post_type();

        let header_size = 32
            + 64
            + varint::length(self.header.num_links.len() as u64)
            + 32 * self.header.num_links.len()
            + varint::length(post_type)
            + varint::length(self.header.timestamp);

        let body_size = varint::length(post_type)
            + match &self.body {
                PostBody::Text {
                    channel,
                    timestamp,
                    text,
                } => {
                    varint::length(channel.len() as u64)
                        + channel.len()
                        + varint::length(*timestamp)
                        + varint::length(text.len() as u64)
                        + text.len()
                }
                PostBody::Delete { timestamp, hash } => varint::length(*timestamp) + hash.len(),
                PostBody::Info {
                    timestamp,
                    key,
                    value,
                } => {
                    varint::length(*timestamp)
                        + varint::length(key.len() as u64)
                        + key.len()
                        + varint::length(value.len() as u64)
                        + value.len()
                }
                PostBody::Topic {
                    channel,
                    timestamp,
                    topic,
                } => {
                    varint::length(channel.len() as u64)
                        + channel.len()
                        + varint::length(*timestamp)
                        + varint::length(topic.len() as u64)
                        + topic.len()
                }
                PostBody::Join { channel, timestamp } => {
                    varint::length(channel.len() as u64)
                        + channel.len()
                        + varint::length(*timestamp)
                }
                PostBody::Leave { channel, timestamp } => {
                    varint::length(channel.len() as u64)
                        + channel.len()
                        + varint::length(*timestamp)
                }
                PostBody::Unrecognized { .. } => 0,
            };
        header_size + body_size
    }
    fn count_from_bytes(_buf: &[u8]) -> Result<usize, Error> {
        unimplemented![]
    }
}

impl FromBytes for Post {
    fn from_bytes(buf: &[u8]) -> Result<(usize, Self), Error> {
        let mut offset = 0;
        let header = {
            let mut public_key = [0; 32];
            public_key.copy_from_slice(&buf[offset..offset + 32]);
            offset += 32;
            let mut signature = [0; 64];
            signature.copy_from_slice(&buf[offset..offset + 64]);
            offset += 64;
            let mut link = [0; 32];
            link.copy_from_slice(&buf[offset..offset + 32]);
            offset += 32;
            PostHeader {
                public_key,
                signature,
                link,
            }
        };
        let (s, post_type) = varint::decode(&buf[offset..])?;
        offset += s;
        let body = match post_type {
            0 => {
                let (s, channel_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let channel = buf[offset..offset + channel_len as usize].to_vec();
                offset += channel_len as usize;
                let (s, timestamp) = varint::decode(&buf[offset..])?;
                offset += s;
                let (s, text_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let text = buf[offset..offset + text_len as usize].to_vec();
                offset += text_len as usize;
                PostBody::Text {
                    channel,
                    timestamp,
                    text,
                }
            }
            1 => {
                let (s, timestamp) = varint::decode(&buf[offset..])?;
                offset += s;
                let mut hash = [0; 32];
                hash.copy_from_slice(&buf[offset..offset + 32]);
                offset += 32;
                PostBody::Delete { timestamp, hash }
            }
            2 => {
                let (s, timestamp) = varint::decode(&buf[offset..])?;
                offset += s;
                let (s, key_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let key = buf[offset..offset + key_len as usize].to_vec();
                offset += key_len as usize;
                let (s, value_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let value = buf[offset..offset + value_len as usize].to_vec();
                offset += value_len as usize;
                PostBody::Info {
                    timestamp,
                    key,
                    value,
                }
            }
            3 => {
                let (s, channel_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let channel = buf[offset..offset + channel_len as usize].to_vec();
                offset += channel_len as usize;
                let (s, timestamp) = varint::decode(&buf[offset..])?;
                offset += s;
                let (s, topic_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let topic = buf[offset..offset + topic_len as usize].to_vec();
                offset += topic_len as usize;
                PostBody::Topic {
                    channel,
                    timestamp,
                    topic,
                }
            }
            4 => {
                let (s, channel_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let channel = buf[offset..offset + channel_len as usize].to_vec();
                offset += channel_len as usize;
                let (s, timestamp) = varint::decode(&buf[offset..])?;
                offset += s;
                PostBody::Join { channel, timestamp }
            }
            5 => {
                let (s, channel_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let channel = buf[offset..offset + channel_len as usize].to_vec();
                offset += channel_len as usize;
                let (s, timestamp) = varint::decode(&buf[offset..])?;
                offset += s;
                PostBody::Leave { channel, timestamp }
            }
            post_type => PostBody::Unrecognized { post_type },
        };
        Ok((offset, Post { header, body }))
    }
}
*/

#[cfg(test)]
mod test {
    use super::Post;

    use hex::FromHex;

    const TEXT_POST_HEX_BINARY: &str = "25b272a71555322d40efe449a7f99af8fd364b92d350f1664481b2da340a02d06725733046b35fa3a7e8dc0099a2b3dff10d3fd8b0f6da70d094352e3f5d27a8bc3f5586cf0bf71befc22536c3c50ec7b1d64398d43c3f4cde778e579e88af05015049d089a650aa896cb25ec35258653be4df196b4a5e5b6db7ed024aaa89e1b300500764656661756c740d68e282ac6c6c6f20776f726c64";

    #[test]
    fn test_verify_post() {
        // Encoded text post.
        let buffer = <Vec<u8>>::from_hex(TEXT_POST_HEX_BINARY).unwrap();

        let result = Post::verify(&buffer);
        assert_eq!(result, true);
    }
}

/*
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
    pub fn verify(buf: &[u8]) -> bool {
        if buf.len() < 32 + 64 {
            return false;
        }
        let o_pk = crypto::sign::PublicKey::from_slice(&buf[0..32]);
        let o_sig = crypto::sign::Signature::from_bytes(&buf[32..32 + 64]);
        match (o_pk, o_sig) {
            (Some(pk), Ok(sig)) => crypto::sign::verify_detached(&sig, &buf[32 + 64..], &pk),
            _ => false,
        }
    }
    pub fn sign(&mut self, secret_key: &[u8; 64]) -> Result<(), Error> {
        let buf = self.to_bytes()?;
        let sk = crypto::sign::SecretKey::from_slice(secret_key).unwrap();
        // todo: return NoneError
        self.header.signature = crypto::sign::sign_detached(&buf[32 + 64..], &sk).to_bytes();
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
    pub fn hash(&self) -> Result<Hash, Error> {
        let buf = self.to_bytes()?;
        let digest = crypto::generichash::hash(&buf, Some(32), None).unwrap();
        Ok(digest.as_ref().try_into()?)
    }
    pub fn get_timestamp(&self) -> Option<u64> {
        match &self.body {
            PostBody::Text { timestamp, .. } => Some(*timestamp),
            PostBody::Delete { timestamp, .. } => Some(*timestamp),
            PostBody::Info { timestamp, .. } => Some(*timestamp),
            PostBody::Topic { timestamp, .. } => Some(*timestamp),
            PostBody::Join { timestamp, .. } => Some(*timestamp),
            PostBody::Leave { timestamp, .. } => Some(*timestamp),
            PostBody::Unrecognized { .. } => None,
        }
    }
    pub fn get_channel<'a>(&'a self) -> Option<&'a Channel> {
        match &self.body {
            PostBody::Text { channel, .. } => Some(channel),
            PostBody::Delete { .. } => None,
            PostBody::Info { .. } => None,
            PostBody::Topic { channel, .. } => Some(channel),
            PostBody::Join { channel, .. } => Some(channel),
            PostBody::Leave { channel, .. } => Some(channel),
            PostBody::Unrecognized { .. } => None,
        }
    }
}

impl CountBytes for Post {
    fn count_bytes(&self) -> usize {
        let post_type = self.post_type();
        let header_size = 32 + 64 + 32;
        let body_size = varint::length(post_type)
            + match &self.body {
                PostBody::Text {
                    channel,
                    timestamp,
                    text,
                } => {
                    varint::length(channel.len() as u64)
                        + channel.len()
                        + varint::length(*timestamp)
                        + varint::length(text.len() as u64)
                        + text.len()
                }
                PostBody::Delete { timestamp, hash } => varint::length(*timestamp) + hash.len(),
                PostBody::Info {
                    timestamp,
                    key,
                    value,
                } => {
                    varint::length(*timestamp)
                        + varint::length(key.len() as u64)
                        + key.len()
                        + varint::length(value.len() as u64)
                        + value.len()
                }
                PostBody::Topic {
                    channel,
                    timestamp,
                    topic,
                } => {
                    varint::length(channel.len() as u64)
                        + channel.len()
                        + varint::length(*timestamp)
                        + varint::length(topic.len() as u64)
                        + topic.len()
                }
                PostBody::Join { channel, timestamp } => {
                    varint::length(channel.len() as u64)
                        + channel.len()
                        + varint::length(*timestamp)
                }
                PostBody::Leave { channel, timestamp } => {
                    varint::length(channel.len() as u64)
                        + channel.len()
                        + varint::length(*timestamp)
                }
                PostBody::Unrecognized { .. } => 0,
            };
        header_size + body_size
    }
    fn count_from_bytes(_buf: &[u8]) -> Result<usize, Error> {
        unimplemented![]
    }
}

impl ToBytes for Post {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0; self.count_bytes()];
        self.write_bytes(&mut buf)?;
        Ok(buf)
    }
    fn write_bytes(&self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut offset = 0;
        assert_eq![self.header.public_key.len(), 32];
        assert_eq![self.header.signature.len(), 64];
        assert_eq![self.header.link.len(), 32];
        buf[offset..offset + 32].copy_from_slice(&self.header.public_key);
        offset += self.header.public_key.len();
        buf[offset..offset + 64].copy_from_slice(&self.header.signature);
        offset += self.header.signature.len();
        buf[offset..offset + 32].copy_from_slice(&self.header.link);
        offset += self.header.link.len();
        offset += varint::encode(self.post_type(), &mut buf[offset..])?;
        match &self.body {
            PostBody::Text {
                channel,
                timestamp,
                text,
            } => {
                offset += varint::encode(channel.len() as u64, &mut buf[offset..])?;
                buf[offset..offset + channel.len()].copy_from_slice(channel);
                offset += channel.len();
                offset += varint::encode(*timestamp, &mut buf[offset..])?;
                offset += varint::encode(text.len() as u64, &mut buf[offset..])?;
                buf[offset..offset + text.len()].copy_from_slice(text);
                offset += text.len();
            }
            PostBody::Delete { timestamp, hash } => {
                offset += varint::encode(*timestamp, &mut buf[offset..])?;
                buf[offset..offset + hash.len()].copy_from_slice(hash);
                offset += hash.len();
            }
            PostBody::Info {
                timestamp,
                key,
                value,
            } => {
                offset += varint::encode(*timestamp, &mut buf[offset..])?;
                offset += varint::encode(key.len() as u64, &mut buf[offset..])?;
                buf[offset..offset + key.len()].copy_from_slice(key);
                offset += key.len();
                offset += varint::encode(value.len() as u64, &mut buf[offset..])?;
                buf[offset..offset + value.len()].copy_from_slice(value);
                offset += value.len();
            }
            PostBody::Topic {
                channel,
                timestamp,
                topic,
            } => {
                offset += varint::encode(channel.len() as u64, &mut buf[offset..])?;
                buf[offset..offset + channel.len()].copy_from_slice(channel);
                offset += channel.len();
                offset += varint::encode(*timestamp, &mut buf[offset..])?;
                offset += varint::encode(topic.len() as u64, &mut buf[offset..])?;
                buf[offset..offset + topic.len()].copy_from_slice(topic);
                offset += topic.len();
            }
            PostBody::Join { channel, timestamp } => {
                offset += varint::encode(channel.len() as u64, &mut buf[offset..])?;
                buf[offset..offset + channel.len()].copy_from_slice(channel);
                offset += channel.len();
                offset += varint::encode(*timestamp, &mut buf[offset..])?;
            }
            PostBody::Leave { channel, timestamp } => {
                offset += varint::encode(channel.len() as u64, &mut buf[offset..])?;
                buf[offset..offset + channel.len()].copy_from_slice(channel);
                offset += channel.len();
                offset += varint::encode(*timestamp, &mut buf[offset..])?;
            }
            PostBody::Unrecognized { post_type } => {
                return E::PostWriteUnrecognizedType {
                    post_type: *post_type,
                }
                .raise();
            }
        }
        Ok(offset)
    }
}

impl FromBytes for Post {
    fn from_bytes(buf: &[u8]) -> Result<(usize, Self), Error> {
        let mut offset = 0;
        let header = {
            let mut public_key = [0; 32];
            public_key.copy_from_slice(&buf[offset..offset + 32]);
            offset += 32;
            let mut signature = [0; 64];
            signature.copy_from_slice(&buf[offset..offset + 64]);
            offset += 64;
            let mut link = [0; 32];
            link.copy_from_slice(&buf[offset..offset + 32]);
            offset += 32;
            PostHeader {
                public_key,
                signature,
                link,
            }
        };
        let (s, post_type) = varint::decode(&buf[offset..])?;
        offset += s;
        let body = match post_type {
            0 => {
                let (s, channel_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let channel = buf[offset..offset + channel_len as usize].to_vec();
                offset += channel_len as usize;
                let (s, timestamp) = varint::decode(&buf[offset..])?;
                offset += s;
                let (s, text_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let text = buf[offset..offset + text_len as usize].to_vec();
                offset += text_len as usize;
                PostBody::Text {
                    channel,
                    timestamp,
                    text,
                }
            }
            1 => {
                let (s, timestamp) = varint::decode(&buf[offset..])?;
                offset += s;
                let mut hash = [0; 32];
                hash.copy_from_slice(&buf[offset..offset + 32]);
                offset += 32;
                PostBody::Delete { timestamp, hash }
            }
            2 => {
                let (s, timestamp) = varint::decode(&buf[offset..])?;
                offset += s;
                let (s, key_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let key = buf[offset..offset + key_len as usize].to_vec();
                offset += key_len as usize;
                let (s, value_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let value = buf[offset..offset + value_len as usize].to_vec();
                offset += value_len as usize;
                PostBody::Info {
                    timestamp,
                    key,
                    value,
                }
            }
            3 => {
                let (s, channel_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let channel = buf[offset..offset + channel_len as usize].to_vec();
                offset += channel_len as usize;
                let (s, timestamp) = varint::decode(&buf[offset..])?;
                offset += s;
                let (s, topic_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let topic = buf[offset..offset + topic_len as usize].to_vec();
                offset += topic_len as usize;
                PostBody::Topic {
                    channel,
                    timestamp,
                    topic,
                }
            }
            4 => {
                let (s, channel_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let channel = buf[offset..offset + channel_len as usize].to_vec();
                offset += channel_len as usize;
                let (s, timestamp) = varint::decode(&buf[offset..])?;
                offset += s;
                PostBody::Join { channel, timestamp }
            }
            5 => {
                let (s, channel_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let channel = buf[offset..offset + channel_len as usize].to_vec();
                offset += channel_len as usize;
                let (s, timestamp) = varint::decode(&buf[offset..])?;
                offset += s;
                PostBody::Leave { channel, timestamp }
            }
            post_type => PostBody::Unrecognized { post_type },
        };
        Ok((offset, Post { header, body }))
    }
}
*/
