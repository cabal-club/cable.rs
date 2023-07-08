/*
use crate::{error::CableErrorKind as E, Channel, Error, Hash, Payload, ReqId};
use desert::{varint, CountBytes, FromBytes, ToBytes};
*/

//! Message formats for all request and response message types supported by cable.
//!
//! Includes type definitions for all request and response message types,
//! as well as message header and body types. Helper methods are included.

use desert::{varint, CountBytes, FromBytes, ToBytes};

use crate::{
    error::{CableErrorKind, Error},
    post::EncodedPost,
    Channel, CircuitId, EncodedChannel, Hash, ReqId,
};

#[derive(Clone, Debug)]
pub struct Message {
    pub header: MessageHeader,
    pub body: MessageBody,
}

impl Message {
    /// Return the numeric type identifier for the message.
    pub fn message_type(&self) -> u64 {
        match &self.body {
            MessageBody::Request { body, .. } => match body {
                RequestBody::Post { .. } => 2,
                RequestBody::Cancel { .. } => 3,
                RequestBody::ChannelTimeRange { .. } => 4,
                RequestBody::ChannelState { .. } => 5,
                RequestBody::ChannelList { .. } => 6,
            },
            MessageBody::Response { body } => match body {
                ResponseBody::Hash { .. } => 0,
                ResponseBody::Post { .. } => 1,
                ResponseBody::ChannelList { .. } => 7,
            },
        }
    }
}

#[derive(Clone, Debug)]
/// The header of a request or response message.
pub struct MessageHeader {
    /// Type identifier for the message (controls which fields follow the header).
    pub msg_type: u64, // varint
    /// ID of a circuit for an established path; `[0,0,0,0]` for no circuit (current default).
    pub circuit_id: CircuitId,
    /// Unique ID of this request (randomly-assigned).
    pub req_id: ReqId,
}

#[derive(Clone, Debug)]
/// The body of a request or response message.
pub enum MessageBody {
    Request {
        /// Number of network hops remaining (must be between 0 and 16).
        ttl: u8, // varint
        body: RequestBody,
    },
    Response {
        body: ResponseBody,
    },
}

#[derive(Clone, Debug)]
pub enum RequestBody {
    /// Request a set of posts by their hashes.
    ///
    /// Message type (`msg_type`) is `2`.
    Post {
        /// Hashes being requested (concatenated together).
        // NOTE: I would prefer to represent this field as `Vec<Hash>`.
        // That results in a `Vec<[u8; 32]>`, which needs to be flattened when
        // copying to a buffer. `.flatten()` exists for this purpose but is
        // currently only available on nightly (unstable).
        // Using a `Vec<u8>` for now. See if there is another way.
        hashes: Vec<u8>,
    },
    /// Conclude a given request identified by `req_id` and stop receiving responses for that request.
    ///
    /// Message type (`msg_type`) is `3`.
    Cancel {
        /// The `req_id` of the request to be cancelled.
        cancel_id: ReqId, // varint
    },
    /// Request chat messages and chat message deletions written to a channel
    /// between a start and end time, optionally subscribing to future chat messages.
    ///
    /// Message type (`msg_type`) is `4`.
    ChannelTimeRange {
        /// Channel name (UTF-8).
        channel: Channel,
        /// Beginning of the time range (in milliseconds since the UNIX Epoch).
        ///
        /// This represents the age of the oldest post the requester is interested in.
        time_start: u64, // varint
        /// End of the time range (in milliseconds since the UNIX Epoch).
        ///
        /// This represents the age of the newest post the requester is interested in.
        ///
        /// A value of `0` is a keep-alive request; the responder should continue
        /// to send chat messages as they learn of them in the future.
        time_end: u64, // varint
        /// Maximum numbers of hashes to return.
        limit: u64, // varint
    },
    /// Request posts that describe the current state of a channel and it's members,
    /// and optionally subscribe to future state changes.
    ///
    /// Message type (`msg_type`) is `5`.
    ChannelState {
        /// Channel name (UTF-8).
        channel: Channel,
        /// Whether to include live/future state hashes.
        ///
        /// This value must be set to either `0` or `1`.
        ///
        /// A value of `0` means that only the latest state posts will be included
        /// and the request will not be held open.
        ///
        /// A value of `1` means that the responder will respond with future channel
        /// state changes as they become known to the responder. The request will be
        /// held open indefinitely on both the requester and responder side until
        /// either a Cancel Request is issued by the requester or the responder
        /// elects to end the request by sending a Hash Response with hash_count = 0.
        future: bool, // varint
    },
    /// Request a list of known channels from peers.
    ///
    /// The combination of `offset` and `limit` fields allows clients to paginate
    /// through the list of all channel names known by a peer.
    ///
    /// Message type (`msg_type`) is `6`.
    ChannelList {
        /// Number of channel names to skip (`0` to skip none).
        offset: u64, // varint
        /// Maximum number of channel names to return.
        ///
        /// If set to `0`, the responder must respond with all known channels
        /// (after skipping the first `offset` entries).
        limit: u64, // varint
    },
}

#[derive(Clone, Debug)]
pub enum ResponseBody {
    /// Respond with a list of zero or more hashes.
    ///
    /// Message type (`msg_type`) is `0`.
    Hash {
        /// Hashes being sent in response (concatenated together).
        hashes: Vec<u8>,
    },
    /// Respond with a list of posts in response to a Post Request.
    ///
    /// Message type (`msg_type`) is `1`.
    Post {
        /// A list of encoded posts, with each one including the length and data of the post.
        posts: Vec<EncodedPost>,
    },
    /// Respond with a list of names of known channels.
    ///
    /// Message type (`msg_type`) is `7`.
    ChannelList {
        /// A list of encoded channels, with each one including the length and name of a channel.
        channels: Vec<EncodedChannel>,
    },
}

impl CountBytes for Message {
    /// Calculate the total number of bytes comprising the encoded message.
    fn count_bytes(&self) -> usize {
        let message_type = self.message_type();

        // Count the message header bytes.
        //
        // Encoded message type + circuit ID + request ID.
        let header_size = varint::length(message_type) + 4 + 4;

        // Count the message body bytes.
        let body_size = match &self.body {
            MessageBody::Request { body, .. } => match body {
                RequestBody::Post { hashes } => {
                    varint::length((hashes.len() / 32) as u64) + hashes.len()
                }
                RequestBody::Cancel { .. } => 4,
                RequestBody::ChannelTimeRange {
                    channel,
                    time_start,
                    time_end,
                    limit,
                } => {
                    varint::length(channel.len() as u64)
                        + channel.len()
                        + varint::length(*time_start)
                        + varint::length(*time_end)
                        + varint::length(*limit)
                }
                RequestBody::ChannelState { channel, future } => {
                    varint::length(channel.len() as u64)
                        + channel.len()
                        + varint::length(*future as u64)
                }
                RequestBody::ChannelList { offset, limit } => {
                    varint::length(*offset) + varint::length(*limit)
                }
            },
            MessageBody::Response { body } => match body {
                ResponseBody::Hash { hashes } => {
                    varint::length((hashes.len() / 32) as u64) + hashes.len()
                }
                ResponseBody::Post { posts } => {
                    posts.iter().fold(0, |sum, post| {
                        sum + varint::length(post.len() as u64) + post.len()
                    }) + varint::length(0)
                }
                ResponseBody::ChannelList { channels } => {
                    channels.iter().fold(0, |sum, channel| {
                        sum + varint::length(channel.len() as u64) + channel.len()
                    }) + varint::length(0)
                }
            },
        };

        let message_size = header_size + body_size;

        varint::length(message_size as u64) + message_size
    }

    /// Calculate the total number of bytes comprising the buffer.
    fn count_from_bytes(buf: &[u8]) -> Result<usize, Error> {
        if buf.is_empty() {
            return CableErrorKind::MessageEmpty {}.raise();
        }

        let (sum, msg_len) = varint::decode(buf)?;

        Ok(sum + (msg_len as usize))
    }
}

/*
impl ToBytes for Message {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0; self.count_bytes()];
        self.write_bytes(&mut buf)?;
        Ok(buf)
    }
    fn write_bytes(&self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut offset = 0;
        let mut msg_len = self.count_bytes();
        msg_len -= varint::length(msg_len as u64);
        offset += varint::encode(msg_len as u64, &mut buf[offset..])?;
        let msg_type = match self {
            Self::HashResponse { .. } => 0,
            Self::DataResponse { .. } => 1,
            Self::HashRequest { .. } => 2,
            Self::CancelRequest { .. } => 3,
            Self::ChannelTimeRangeRequest { .. } => 4,
            Self::ChannelStateRequest { .. } => 5,
            Self::ChannelListRequest { .. } => 6,
            Self::Unrecognized { msg_type } => {
                return E::MessageWriteUnrecognizedType {
                    msg_type: *msg_type,
                }
                .raise()
            }
        };
        offset += varint::encode(msg_type, &mut buf[offset..])?;
        Ok(match self {
            Self::HashResponse { req_id, hashes } => {
                offset += req_id.write_bytes(&mut buf[offset..])?;
                offset += varint::encode(hashes.len() as u64, &mut buf[offset..])?;
                for hash in hashes.iter() {
                    if offset + hash.len() > buf.len() {
                        return E::DstTooSmall {
                            required: offset + hash.len(),
                            provided: buf.len(),
                        }
                        .raise();
                    }
                    buf[offset..offset + hash.len()].copy_from_slice(hash);
                    offset += hash.len();
                }
                offset
            }
            Self::DataResponse { req_id, data } => {
                offset += req_id.write_bytes(&mut buf[offset..])?;
                for d in data.iter() {
                    offset += varint::encode(d.len() as u64, &mut buf[offset..])?;
                    if offset + d.len() > buf.len() {
                        return E::DstTooSmall {
                            required: offset + d.len(),
                            provided: buf.len(),
                        }
                        .raise();
                    }
                    buf[offset..offset + d.len()].copy_from_slice(d);
                    offset += d.len();
                }
                offset += varint::encode(0, &mut buf[offset..])?;
                offset
            }
            Self::HashRequest {
                req_id,
                ttl,
                hashes,
            } => {
                offset += req_id.write_bytes(&mut buf[offset..])?;
                offset += varint::encode(*ttl as u64, &mut buf[offset..])?;
                offset += varint::encode(hashes.len() as u64, &mut buf[offset..])?;
                for hash in hashes.iter() {
                    if offset + hash.len() > buf.len() {
                        return E::DstTooSmall {
                            required: offset + hash.len(),
                            provided: buf.len(),
                        }
                        .raise();
                    }
                    buf[offset..offset + hash.len()].copy_from_slice(hash);
                    offset += hash.len();
                }
                offset
            }
            Self::CancelRequest { req_id } => {
                offset += req_id.write_bytes(&mut buf[offset..])?;
                offset
            }
            Self::ChannelTimeRangeRequest {
                req_id,
                ttl,
                channel,
                time_start,
                time_end,
                limit,
            } => {
                offset += req_id.write_bytes(&mut buf[offset..])?;
                offset += varint::encode(*ttl as u64, &mut buf[offset..])?;
                if offset + channel.len() > buf.len() {
                    return E::DstTooSmall {
                        required: offset + channel.len(),
                        provided: buf.len(),
                    }
                    .raise();
                }
                offset += channel.write_bytes(&mut buf[offset..])?;
                offset += varint::encode(*time_start, &mut buf[offset..])?;
                offset += varint::encode(*time_end, &mut buf[offset..])?;
                offset += varint::encode(*limit as u64, &mut buf[offset..])?;
                offset
            }
            Self::ChannelStateRequest {
                req_id,
                ttl,
                channel,
                limit,
                updates,
            } => {
                offset += req_id.write_bytes(&mut buf[offset..])?;
                offset += varint::encode(*ttl as u64, &mut buf[offset..])?;
                if offset + channel.len() > buf.len() {
                    return E::DstTooSmall {
                        required: offset + channel.len(),
                        provided: buf.len(),
                    }
                    .raise();
                }
                offset += channel.write_bytes(&mut buf[offset..])?;
                offset += varint::encode(*limit as u64, &mut buf[offset..])?;
                offset += varint::encode(*updates as u64, &mut buf[offset..])?;
                offset
            }
            Self::ChannelListRequest { req_id, ttl, limit } => {
                offset += req_id.write_bytes(&mut buf[offset..])?;
                offset += varint::encode(*ttl as u64, &mut buf[offset..])?;
                offset += varint::encode(*limit as u64, &mut buf[offset..])?;
                offset
            }
            Self::Unrecognized { msg_type } => {
                return E::MessageWriteUnrecognizedType {
                    msg_type: *msg_type,
                }
                .raise();
            }
        })
    }
}

impl FromBytes for Message {
    fn from_bytes(buf: &[u8]) -> Result<(usize, Self), Error> {
        if buf.is_empty() {
            return E::MessageEmpty {}.raise();
        }
        let mut offset = 0;
        let (s, nbytes) = varint::decode(&buf[offset..])?;
        offset += s;
        let msg_len = (nbytes as usize) + s;
        let (s, msg_type) = varint::decode(&buf[offset..])?;
        offset += s;
        Ok(match msg_type {
            0 => {
                if offset + 4 > buf.len() {
                    return E::MessageHashResponseEnd {}.raise();
                }
                let mut req_id = [0; 4];
                req_id.copy_from_slice(&buf[offset..offset + 4]);
                offset += 4;
                let (s, hash_count) = varint::decode(&buf[offset..])?;
                offset += s;
                let mut hashes = Vec::with_capacity(hash_count as usize);
                for _ in 0..hash_count {
                    if offset + 32 > buf.len() {
                        return E::MessageHashResponseEnd {}.raise();
                    }
                    let mut hash = [0; 32];
                    hash.copy_from_slice(&buf[offset..offset + 32]);
                    offset += 32;
                    hashes.push(hash);
                }
                (msg_len, Self::HashResponse { req_id, hashes })
            }
            1 => {
                if offset + 4 > buf.len() {
                    return E::MessageDataResponseEnd {}.raise();
                }
                let mut req_id = [0; 4];
                req_id.copy_from_slice(&buf[offset..offset + 4]);
                offset += 4;
                let mut data = vec![];
                loop {
                    let (s, data_len) = varint::decode(&buf[offset..])?;
                    offset += s;
                    if data_len == 0 {
                        break;
                    }
                    data.push(buf[offset..offset + (data_len as usize)].to_vec());
                    offset += data_len as usize;
                }
                (msg_len, Self::DataResponse { req_id, data })
            }
            2 => {
                if offset + 4 > buf.len() {
                    return E::MessageHashRequestEnd {}.raise();
                }
                let mut req_id = [0; 4];
                req_id.copy_from_slice(&buf[offset..offset + 4]);
                offset += 4;
                let (s, ttl) = varint::decode(&buf[offset..])?;
                offset += s;
                let (s, hash_count) = varint::decode(&buf[offset..])?;
                offset += s;
                let mut hashes = Vec::with_capacity(hash_count as usize);
                for _ in 0..hash_count {
                    if offset + 32 > buf.len() {
                        return E::MessageHashRequestEnd {}.raise();
                    }
                    let mut hash = [0; 32];
                    hash.copy_from_slice(&buf[offset..offset + 32]);
                    offset += 32;
                    hashes.push(hash);
                }
                (
                    msg_len,
                    Self::HashRequest {
                        req_id,
                        ttl: ttl as usize,
                        hashes,
                    },
                )
            }
            3 => {
                if offset + 4 > buf.len() {
                    return E::MessageCancelRequestEnd {}.raise();
                }
                let mut req_id = [0; 4];
                req_id.copy_from_slice(&buf[offset..offset + 4]);
                (msg_len, Self::CancelRequest { req_id })
            }
            4 => {
                if offset + 4 > buf.len() {
                    return E::MessageChannelTimeRangeRequestEnd {}.raise();
                }
                let mut req_id = [0; 4];
                req_id.copy_from_slice(&buf[offset..offset + 4]);
                offset += 4;
                let (s, ttl) = varint::decode(&buf[offset..])?;
                offset += s;
                let (s, channel_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let channel = buf[offset..offset + channel_len as usize].to_vec();
                offset += channel_len as usize;
                let (s, time_start) = varint::decode(&buf[offset..])?;
                offset += s;
                let (s, time_end) = varint::decode(&buf[offset..])?;
                offset += s;
                let (_, limit) = varint::decode(&buf[offset..])?;
                //offset += s;
                (
                    msg_len,
                    Self::ChannelTimeRangeRequest {
                        req_id,
                        ttl: ttl as usize,
                        channel,
                        time_start,
                        time_end,
                        limit: limit as usize,
                    },
                )
            }
            5 => {
                if offset + 4 > buf.len() {
                    return E::MessageChannelStateRequestEnd {}.raise();
                }
                let mut req_id = [0; 4];
                req_id.copy_from_slice(&buf[offset..offset + 4]);
                offset += 4;
                let (s, ttl) = varint::decode(&buf[offset..])?;
                offset += s;
                let (s, channel_len) = varint::decode(&buf[offset..])?;
                offset += s;
                let channel = buf[offset..offset + channel_len as usize].to_vec();
                offset += channel_len as usize;
                let (s, limit) = varint::decode(&buf[offset..])?;
                offset += s;
                let (_, updates) = varint::decode(&buf[offset..])?;
                (
                    msg_len,
                    Self::ChannelStateRequest {
                        req_id,
                        ttl: ttl as usize,
                        channel,
                        limit: limit as usize,
                        updates: updates as usize,
                    },
                )
            }
            6 => {
                if offset + 4 > buf.len() {
                    return E::MessageChannelListRequestEnd {}.raise();
                }
                let mut req_id = [0; 4];
                req_id.copy_from_slice(&buf[offset..offset + 4]);
                offset += 4;
                let (s, ttl) = varint::decode(&buf[offset..])?;
                offset += s;
                let (_, limit) = varint::decode(&buf[offset..])?;
                //offset += s;
                (
                    msg_len,
                    Self::ChannelListRequest {
                        req_id,
                        ttl: ttl as usize,
                        limit: limit as usize,
                    },
                )
            }
            msg_type => (msg_len, Self::Unrecognized { msg_type }),
        })
    }
}
*/
