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
    pub key: Vec<u8>,
    pub val: Vec<u8>,
}

impl UserInfo {
    /// Convenience method to construct `UserInfo`.
    pub fn new<T: Into<Vec<u8>>, U: Into<Vec<u8>>>(key: T, val: U) -> Self {
        UserInfo {
            key: key.into(),
            val: val.into(),
        }
    }
}

#[derive(Clone, Debug)]
/// The length and data of an encoded post.
pub struct EncodedPost {
    /// The length of the post in bytes.
    pub post_len: u64,
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
    /// Hashes of the latest posts in this channel/context.
    // NOTE: I would prefer to represent this field as `Vec<Hash>`.
    // That results in a `Vec<[u8; 32]>`, which needs to be flattened when
    // copying to a buffer. `.flatten()` exists for this purpose but is
    // currently only available on nightly (unstable).
    // Using a `Vec<u8>` for now. See if there is another way.
    //pub links: Vec<Hash>,
    pub links: Vec<u8>,
    /// Post type.
    pub post_type: u64,
    /// Time at which the post was created (in milliseconds since the UNIX Epoch).
    pub timestamp: u64,
}

impl PostHeader {
    /// Convenience method to construct a `PostHeader`.
    pub fn new(
        public_key: [u8; 32],
        signature: [u8; 64],
        links: Vec<u8>,
        post_type: u64,
        timestamp: u64,
    ) -> Self {
        PostHeader {
            public_key,
            signature,
            links,
            post_type,
            timestamp,
        }
    }
}

// TODO: remember to write validators for post type data.
// E.g. "A topic field MUST be a valid UTF-8 string, between 0 and 512 codepoints."

#[derive(Clone, Debug)]
/// The body of a post.
pub enum PostBody {
    /// Post a chat message to a channel.
    Text {
        /// Channel name (UTF-8).
        channel: Channel,
        /// Chat message text (UTF-8).
        text: Text,
    },
    /// Request that peers encountering this post delete the referenced posts
    /// from their local storage, and not store the referenced posts in the future.
    Delete {
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
        /// Channel name (UTF-8).
        channel: Channel,
        /// Topic content (UTF-8).
        topic: Topic,
    },
    /// Publicly announce membership in a channel.
    Join {
        /// Channel name (UTF-8).
        channel: Channel,
    },
    /// Publicly announce termination of membership in a channel.
    Leave {
        /// Channel name (UTF-8).
        channel: Channel,
    },
    /// A post type which is not recognised as part of the cable specification.
    Unrecognized { post_type: u64 },
}

impl Post {
    /// Convenience method to construct a `Post` from a header and body.
    pub fn new(header: PostHeader, body: PostBody) -> Self {
        Post { header, body }
    }

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
        offset += varint::encode((self.header.links.len() / 32) as u64, &mut buf[offset..])?;

        // Write the links bytes to the buffer and increment the offset.
        buf[offset..offset + self.header.links.len()].copy_from_slice(&self.header.links);
        offset += self.header.links.len();

        // Encode the post type as a varint, write the resulting bytes to the
        // buffer and increment the offset.
        offset += varint::encode(self.post_type(), &mut buf[offset..])?;

        // Encode the timestamp as a varint, write the resulting bytes to the
        // buffer and increment the offset.
        offset += varint::encode(self.header.timestamp, &mut buf[offset..])?;

        /* POST BODY BYTES */

        match &self.body {
            PostBody::Text { channel, text } => {
                offset += varint::encode(channel.len() as u64, &mut buf[offset..])?;
                buf[offset..offset + channel.len()].copy_from_slice(channel);
                offset += channel.len();

                offset += varint::encode(text.len() as u64, &mut buf[offset..])?;
                buf[offset..offset + text.len()].copy_from_slice(text);
                offset += text.len();
            }
            PostBody::Delete { hashes } => {
                offset += varint::encode((hashes.len() / 32) as u64, &mut buf[offset..])?;
                buf[offset..offset + hashes.len()].copy_from_slice(hashes);
                offset += hashes.len();
            }
            PostBody::Info { info } => {
                for UserInfo { key, val } in info {
                    offset += varint::encode(key.len() as u64, &mut buf[offset..])?;
                    buf[offset..offset + key.len()].copy_from_slice(key);
                    offset += key.len();
                    offset += varint::encode(val.len() as u64, &mut buf[offset..])?;
                    buf[offset..offset + val.len()].copy_from_slice(val);
                    offset += val.len();
                }
            }
            PostBody::Topic { channel, topic } => {
                offset += varint::encode(channel.len() as u64, &mut buf[offset..])?;
                buf[offset..offset + channel.len()].copy_from_slice(channel);
                offset += channel.len();
                offset += varint::encode(topic.len() as u64, &mut buf[offset..])?;
                buf[offset..offset + topic.len()].copy_from_slice(topic);
                offset += topic.len();
            }
            PostBody::Join { channel } => {
                offset += varint::encode(channel.len() as u64, &mut buf[offset..])?;
                buf[offset..offset + channel.len()].copy_from_slice(channel);
                offset += channel.len();
            }
            PostBody::Leave { channel } => {
                offset += varint::encode(channel.len() as u64, &mut buf[offset..])?;
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
            + varint::length((self.header.links.len() / 32) as u64) // Number of links.
            + self.header.links.len() // Links.
            + varint::length(post_type) // Post type.
            + varint::length(self.header.timestamp); // Timestamp.

        // Count the post body bytes.
        let body_size = match &self.body {
            PostBody::Text { channel, text } => {
                varint::length(channel.len() as u64)
                    + channel.len()
                    + varint::length(text.len() as u64)
                    + text.len()
            }
            PostBody::Delete { hashes } => {
                varint::length((hashes.len() / 32) as u64) + hashes.len()
            }
            PostBody::Info { info } => {
                let mut info_len = 0;

                for UserInfo { key, val } in info {
                    info_len += varint::length(key.len() as u64)
                        + key.len()
                        + varint::length(val.len() as u64)
                        + val.len();
                }

                info_len
            }
            PostBody::Topic { channel, topic } => {
                varint::length(channel.len() as u64)
                    + channel.len()
                    + varint::length(topic.len() as u64)
                    + topic.len()
            }
            PostBody::Join { channel } => varint::length(channel.len() as u64) + channel.len(),
            PostBody::Leave { channel } => varint::length(channel.len() as u64) + channel.len(),
            PostBody::Unrecognized { .. } => 0,
        };

        header_size + body_size
    }

    fn count_from_bytes(_buf: &[u8]) -> Result<usize, Error> {
        unimplemented![]
    }
}

#[cfg(test)]
mod test {
    use super::{Post, PostBody, PostHeader, ToBytes, UserInfo};

    use hex::FromHex;

    const PUBLIC_KEY: &str = "25b272a71555322d40efe449a7f99af8fd364b92d350f1664481b2da340a02d0";
    const TEXT_POST_HEX_BINARY: &str = "25b272a71555322d40efe449a7f99af8fd364b92d350f1664481b2da340a02d06725733046b35fa3a7e8dc0099a2b3dff10d3fd8b0f6da70d094352e3f5d27a8bc3f5586cf0bf71befc22536c3c50ec7b1d64398d43c3f4cde778e579e88af05015049d089a650aa896cb25ec35258653be4df196b4a5e5b6db7ed024aaa89e1b300500764656661756c740d68e282ac6c6c6f20776f726c64";
    const DELETE_POST_HEX_BINARY: &str = "25b272a71555322d40efe449a7f99af8fd364b92d350f1664481b2da340a02d0affe77e3b3156cda7feea042269bb7e93f5031662c70610d37baa69132b4150c18d67cb2ac24fb0f9be0a6516e53ba2f3bbc5bd8e7a1bff64d9c78ce0c2e4205015049d089a650aa896cb25ec35258653be4df196b4a5e5b6db7ed024aaa89e1b301500315ed54965515babf6f16be3f96b04b29ecca813a343311dae483691c07ccf4e597fc63631c41384226b9b68d9f73ffaaf6eac54b71838687f48f112e30d6db689c2939fec6d47b00bafe6967aeff697cf4b5abca01b04ba1b31a7e3752454bfa";
    const INFO_POST_HEX_BINARY: &str = "25b272a71555322d40efe449a7f99af8fd364b92d350f1664481b2da340a02d0f70273779147a3b756407d5660ed2e8e2975abc5ab224fb152aa2bfb3dd331740a66e0718cd580bc94978c1c3cd4524ad8cb2f4cca80df481010c3ef834ac700015049d089a650aa896cb25ec35258653be4df196b4a5e5b6db7ed024aaa89e1b30250046e616d65066361626c6572";
    const POST_HASH: &str = "5049d089a650aa896cb25ec35258653be4df196b4a5e5b6db7ed024aaa89e1b3";

    #[test]
    fn verify_post() {
        // Encoded text post.
        let buffer = <Vec<u8>>::from_hex(TEXT_POST_HEX_BINARY).unwrap();

        let result = Post::verify(&buffer);
        assert_eq!(result, true);
    }

    #[test]
    fn text_post_to_bytes() {
        // TODO: Return `Result` and replace `unwrap()` with `?`.

        // Field values sourced from https://github.com/cabal-club/cable.js#examples.

        /* HEADER FIELD VALUES */

        let public_key = <[u8; 32]>::from_hex(PUBLIC_KEY).unwrap();
        let signature = <[u8; 64]>::from_hex("6725733046b35fa3a7e8dc0099a2b3dff10d3fd8b0f6da70d094352e3f5d27a8bc3f5586cf0bf71befc22536c3c50ec7b1d64398d43c3f4cde778e579e88af05").unwrap();
        let links = <Vec<u8>>::from_hex(POST_HASH).unwrap();
        let post_type = 0;
        let timestamp = 80;

        /* BODY FIELD VALUES */

        let channel: Vec<u8> = "default".to_string().into();
        let text: Vec<u8> = "h€llo world".to_string().into();

        // Construct a new post header.
        let header = PostHeader::new(public_key, signature, links, post_type, timestamp);

        // Construct a new post body.
        let body = PostBody::Text { channel, text };

        // Construct a new post.
        let post = Post::new(header, body);
        // Convert the post to bytes.
        let post_bytes = post.to_bytes().unwrap();

        // Test vector binary.
        let expected_bytes = <Vec<u8>>::from_hex(TEXT_POST_HEX_BINARY).unwrap();

        // Ensure the number of generated post bytes matches the number of
        // expected bytes.
        assert_eq!(expected_bytes.len(), post_bytes.len());

        // Ensure the generated post bytes match the expected bytes.
        assert_eq!(expected_bytes, post_bytes);
    }

    #[test]
    fn delete_post_to_bytes() {
        /* HEADER FIELD VALUES */

        let public_key = <[u8; 32]>::from_hex(PUBLIC_KEY).unwrap();
        let signature = <[u8; 64]>::from_hex("affe77e3b3156cda7feea042269bb7e93f5031662c70610d37baa69132b4150c18d67cb2ac24fb0f9be0a6516e53ba2f3bbc5bd8e7a1bff64d9c78ce0c2e4205").unwrap();
        let links = <Vec<u8>>::from_hex(POST_HASH).unwrap();
        let post_type = 1;
        let timestamp = 80;

        /* BODY FIELD VALUES */

        // Concatenate the hashes into a single `Vec<u8>`.
        let mut hashes =
            <Vec<u8>>::from_hex("15ed54965515babf6f16be3f96b04b29ecca813a343311dae483691c07ccf4e5")
                .unwrap();
        hashes.append(
            &mut <Vec<u8>>::from_hex(
                "97fc63631c41384226b9b68d9f73ffaaf6eac54b71838687f48f112e30d6db68",
            )
            .unwrap(),
        );
        hashes.append(
            &mut <Vec<u8>>::from_hex(
                "9c2939fec6d47b00bafe6967aeff697cf4b5abca01b04ba1b31a7e3752454bfa",
            )
            .unwrap(),
        );

        // Construct a new post header.
        let header = PostHeader::new(public_key, signature, links, post_type, timestamp);

        // Construct a new post body.
        let body = PostBody::Delete { hashes };

        // Construct a new post.
        let post = Post::new(header, body);
        // Convert the post to bytes.
        let post_bytes = post.to_bytes().unwrap();

        // Test vector binary.
        let expected_bytes = <Vec<u8>>::from_hex(DELETE_POST_HEX_BINARY).unwrap();

        // Ensure the number of generated post bytes matches the number of
        // expected bytes.
        assert_eq!(expected_bytes.len(), post_bytes.len());

        // Ensure the generated post bytes match the expected bytes.
        assert_eq!(expected_bytes, post_bytes);
    }

    /*
        {
      "name": "post/info",
      "type": "post",
      "id": 2,
      "binary": "25b272a71555322d40efe449a7f99af8fd364b92d350f1664481b2da340a02d0f70273779147a3b756407d5660ed2e8e2975abc5ab224fb152aa2bfb3dd331740a66e0718cd580bc94978c1c3cd4524ad8cb2f4cca80df481010c3ef834ac700015049d089a650aa896cb25ec35258653be4df196b4a5e5b6db7ed024aaa89e1b30250046e616d65066361626c6572",
      "obj": {
        "publicKey": "25b272a71555322d40efe449a7f99af8fd364b92d350f1664481b2da340a02d0",
        "signature": "f70273779147a3b756407d5660ed2e8e2975abc5ab224fb152aa2bfb3dd331740a66e0718cd580bc94978c1c3cd4524ad8cb2f4cca80df481010c3ef834ac700",
        "links": [
          "5049d089a650aa896cb25ec35258653be4df196b4a5e5b6db7ed024aaa89e1b3"
        ],
        "postType": 2,
        "timestamp": 80,
        "key": "name",
        "value": "cabler"
      }
    }
        */
    #[test]
    fn info_post_to_bytes() {
        /* HEADER FIELD VALUES */

        let public_key = <[u8; 32]>::from_hex(PUBLIC_KEY).unwrap();
        let signature = <[u8; 64]>::from_hex("f70273779147a3b756407d5660ed2e8e2975abc5ab224fb152aa2bfb3dd331740a66e0718cd580bc94978c1c3cd4524ad8cb2f4cca80df481010c3ef834ac700").unwrap();
        let links = <Vec<u8>>::from_hex(POST_HASH).unwrap();
        let post_type = 2;
        let timestamp = 80;

        /* BODY FIELD VALUES */
        let key = "name".to_string();
        let val = "cabler".to_string();
        let user_info = UserInfo::new(key, val);

        // Construct a new post header.
        let header = PostHeader::new(public_key, signature, links, post_type, timestamp);

        // Construct a new post body.
        let body = PostBody::Info {
            info: vec![user_info],
        };

        // Construct a new post.
        let post = Post::new(header, body);
        // Convert the post to bytes.
        let post_bytes = post.to_bytes().unwrap();

        // Test vector binary.
        let expected_bytes = <Vec<u8>>::from_hex(INFO_POST_HEX_BINARY).unwrap();

        // Ensure the number of generated post bytes matches the number of
        // expected bytes.
        assert_eq!(expected_bytes.len(), post_bytes.len());

        // Ensure the generated post bytes match the expected bytes.
        assert_eq!(expected_bytes, post_bytes);
    }
}

/*
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
