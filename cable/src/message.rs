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
    Channel, CircuitId, EncodedChannel, Hash, ReqId, Timestamp,
};

#[derive(Clone, Debug)]
pub struct Message {
    pub header: MessageHeader,
    pub body: MessageBody,
}

impl Message {
    /// Convenience method to construct a `Message` from a header and body.
    pub fn new(header: MessageHeader, body: MessageBody) -> Self {
        Message { header, body }
    }

    /// Return the numeric type identifier for the message.
    pub fn message_type(&self) -> u64 {
        match &self.body {
            MessageBody::Request { body, .. } => match body {
                RequestBody::Post { .. } => 2,
                RequestBody::Cancel { .. } => 3,
                RequestBody::ChannelTimeRange { .. } => 4,
                RequestBody::ChannelState { .. } => 5,
                RequestBody::ChannelList { .. } => 6,
                RequestBody::Unrecognized { msg_type } => *msg_type,
            },
            MessageBody::Response { body } => match body {
                ResponseBody::Hash { .. } => 0,
                ResponseBody::Post { .. } => 1,
                ResponseBody::ChannelList { .. } => 7,
                ResponseBody::Unrecognized { msg_type } => *msg_type,
            },
        }
    }
}

#[derive(Clone, Debug)]
/// The header of a request or response message.
pub struct MessageHeader {
    /// Type identifier for the message (controls which fields follow the header).
    pub msg_type: u64,
    /// ID of a circuit for an established path; `[0,0,0,0]` for no circuit (current default).
    pub circuit_id: CircuitId,
    /// Unique ID of this request (randomly-assigned).
    pub req_id: ReqId,
}

impl MessageHeader {
    /// Convenience method to construct a `MessageHeader`.
    pub fn new(msg_type: u64, circuit_id: CircuitId, req_id: ReqId) -> Self {
        MessageHeader {
            msg_type,
            circuit_id,
            req_id,
        }
    }
}

#[derive(Clone, Debug)]
/// The body of a request or response message.
pub enum MessageBody {
    Request {
        /// Number of network hops remaining (must be between 0 and 16).
        ttl: u8,
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
        /// Hashes of the posts being requested.
        hashes: Vec<Hash>,
    },
    /// Conclude a given request identified by `req_id` and stop receiving responses for that request.
    ///
    /// Message type (`msg_type`) is `3`.
    Cancel {
        /// The `req_id` of the request to be cancelled.
        cancel_id: ReqId,
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
        time_start: Timestamp,
        /// End of the time range (in milliseconds since the UNIX Epoch).
        ///
        /// This represents the age of the newest post the requester is interested in.
        ///
        /// A value of `0` is a keep-alive request; the responder should continue
        /// to send chat messages as they learn of them in the future.
        time_end: Timestamp,
        /// Maximum numbers of hashes to return.
        limit: u64,
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
        // TODO: Rather use a `bool` here and convert to 0 / 1 where required.
        future: u64,
    },
    /// Request a list of known channels from peers.
    ///
    /// The combination of `offset` and `limit` fields allows clients to paginate
    /// through the list of all channel names known by a peer.
    ///
    /// Message type (`msg_type`) is `6`.
    ChannelList {
        /// Number of channel names to skip (`0` to skip none).
        // NOTE: The naming of this field deviates from the spec, which
        // names it `offset`. The change has been made to avoid a naming
        // collision with the `offset` variable used in the `ToBytes` and
        // `FromBytes` implementations for `Message`.
        skip: u64,
        /// Maximum number of channel names to return.
        ///
        /// If set to `0`, the responder must respond with all known channels
        /// (after skipping the first `offset` entries).
        limit: u64,
    },
    /// A request message type which is not recognised as part of the cable
    /// specification.
    Unrecognized { msg_type: u64 },
}

#[derive(Clone, Debug)]
pub enum ResponseBody {
    /// Respond with a list of zero or more hashes.
    ///
    /// Message type (`msg_type`) is `0`.
    Hash {
        /// Hashes being sent in response (concatenated together).
        hashes: Vec<Hash>,
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
        /// A list of channels, with each one including the length and name of a channel.
        channels: Vec<Channel>,
    },
    /// A response message type which is not recognised as part of the cable
    /// specification.
    Unrecognized { msg_type: u64 },
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
            MessageBody::Request { body, ttl } => match body {
                RequestBody::Post { hashes } => {
                    varint::length(*ttl as u64)
                        + varint::length(hashes.len() as u64)
                        + hashes.len() * 32
                }
                RequestBody::Cancel { .. } => varint::length(*ttl as u64) + 4,
                RequestBody::ChannelTimeRange {
                    channel,
                    time_start,
                    time_end,
                    limit,
                } => {
                    varint::length(*ttl as u64)
                        + varint::length(channel.len() as u64)
                        + channel.len()
                        + varint::length(*time_start)
                        + varint::length(*time_end)
                        + varint::length(*limit)
                }
                RequestBody::ChannelState { channel, future } => {
                    varint::length(*ttl as u64)
                        + varint::length(channel.len() as u64)
                        + channel.len()
                        + varint::length(*future)
                }
                RequestBody::ChannelList { skip, limit } => {
                    varint::length(*ttl as u64) + varint::length(*skip) + varint::length(*limit)
                }
                RequestBody::Unrecognized { .. } => varint::length(*ttl as u64),
            },
            MessageBody::Response { body } => match body {
                ResponseBody::Hash { hashes } => {
                    varint::length(hashes.len() as u64) + hashes.len() * 32
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
                ResponseBody::Unrecognized { .. } => 0,
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

impl ToBytes for Message {
    /// Convert a `Message` data type to bytes.
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0; self.count_bytes()];
        self.write_bytes(&mut buf)?;
        Ok(buf)
    }

    /// Write bytes to the given buffer (mutable byte array).
    fn write_bytes(&self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut offset = 0;

        /* MESSAGE HEADER BYTES */

        // Count the bytes comprising the message.
        let mut msg_len = self.count_bytes();
        // Minus the varint-encoded length of msg_len.
        msg_len -= varint::length(msg_len as u64);

        // Encode msg_len as a varint, write the resulting bytes to the
        // buffer and increment the offset.
        offset += varint::encode(msg_len as u64, &mut buf[offset..])?;

        // Encode the message type as a varint, write the resulting bytes to
        // the buffer and increment the offset.
        offset += varint::encode(self.message_type(), &mut buf[offset..])?;

        // Write the circuit ID bytes to the buffer and increment the offset.
        offset += self.header.circuit_id.write_bytes(&mut buf[offset..])?;

        // Write the request ID bytes to the buffer and increment the offset.
        offset += self.header.req_id.write_bytes(&mut buf[offset..])?;

        /* MESSAGE BODY BYTES */

        match &self.body {
            MessageBody::Request { body, ttl } => match body {
                RequestBody::Post { hashes } => {
                    offset += varint::encode(*ttl as u64, &mut buf[offset..])?;

                    offset += varint::encode(hashes.len() as u64, &mut buf[offset..])?;
                    for hash in hashes.iter() {
                        if offset + hash.len() > buf.len() {
                            return CableErrorKind::DstTooSmall {
                                required: offset + hash.len(),
                                provided: buf.len(),
                            }
                            .raise();
                        }
                        buf[offset..offset + hash.len()].copy_from_slice(hash);
                        offset += hash.len();
                    }
                }
                RequestBody::Cancel { cancel_id } => {
                    offset += varint::encode(*ttl as u64, &mut buf[offset..])?;
                    offset += cancel_id.write_bytes(&mut buf[offset..])?;
                }
                RequestBody::ChannelTimeRange {
                    channel,
                    time_start,
                    time_end,
                    limit,
                } => {
                    offset += varint::encode(*ttl as u64, &mut buf[offset..])?;

                    offset += varint::encode(channel.len() as u64, &mut buf[offset..])?;
                    buf[offset..offset + channel.len()].copy_from_slice(channel.as_bytes());
                    offset += channel.len();

                    offset += varint::encode(*time_start, &mut buf[offset..])?;
                    offset += varint::encode(*time_end, &mut buf[offset..])?;
                    offset += varint::encode(*limit, &mut buf[offset..])?;
                }
                RequestBody::ChannelState { channel, future } => {
                    offset += varint::encode(*ttl as u64, &mut buf[offset..])?;

                    offset += varint::encode(channel.len() as u64, &mut buf[offset..])?;
                    buf[offset..offset + channel.len()].copy_from_slice(channel.as_bytes());
                    offset += channel.len();

                    offset += varint::encode(*future, &mut buf[offset..])?;
                }
                RequestBody::ChannelList { skip, limit } => {
                    offset += varint::encode(*ttl as u64, &mut buf[offset..])?;
                    offset += varint::encode(*skip, &mut buf[offset..])?;
                    offset += varint::encode(*limit, &mut buf[offset..])?;
                }
                RequestBody::Unrecognized { msg_type } => {
                    return CableErrorKind::MessageWriteUnrecognizedType {
                        msg_type: *msg_type,
                    }
                    .raise();
                }
            },
            MessageBody::Response { body, .. } => match body {
                ResponseBody::Hash { hashes } => {
                    offset += varint::encode(hashes.len() as u64, &mut buf[offset..])?;
                    for hash in hashes {
                        if offset + hash.len() > buf.len() {
                            return CableErrorKind::DstTooSmall {
                                required: offset + hash.len(),
                                provided: buf.len(),
                            }
                            .raise();
                        }
                        buf[offset..offset + hash.len()].copy_from_slice(hash);
                        offset += hash.len();
                    }
                }
                ResponseBody::Post { posts } => {
                    offset += varint::encode(posts.len() as u64, &mut buf[offset..])?;
                    for post in posts {
                        if offset + post.len() > buf.len() {
                            return CableErrorKind::DstTooSmall {
                                required: offset + post.len(),
                                provided: buf.len(),
                            }
                            .raise();
                        }
                        offset += varint::encode(post.len() as u64, &mut buf[offset..])?;
                        buf[offset..offset + post.len()].copy_from_slice(post);
                        offset += post.len();
                    }

                    // Indicate the end of the posts by setting the final
                    // post_len to 0.
                    offset += varint::encode(0, &mut buf[offset..])?;
                }
                ResponseBody::ChannelList { channels } => {
                    offset += varint::encode(channels.len() as u64, &mut buf[offset..])?;
                    for channel in channels {
                        if offset + channel.len() > buf.len() {
                            return CableErrorKind::DstTooSmall {
                                required: offset + channel.len(),
                                provided: buf.len(),
                            }
                            .raise();
                        }
                        offset += varint::encode(channel.len() as u64, &mut buf[offset..])?;
                        buf[offset..offset + channel.len()].copy_from_slice(channel.as_bytes());
                        offset += channel.len();
                    }

                    // Indicate the end of the channels by setting the final
                    // channel_len to 0.
                    offset += varint::encode(0, &mut buf[offset..])?;
                }
                ResponseBody::Unrecognized { msg_type } => {
                    return CableErrorKind::MessageWriteUnrecognizedType {
                        msg_type: *msg_type,
                    }
                    .raise();
                }
            },
        }

        Ok(offset)
    }
}

#[cfg(test)]
mod test {
    use super::{
        Error, Hash, Message, MessageBody, MessageHeader, RequestBody, ResponseBody, ToBytes,
    };

    use hex::FromHex;

    // Field values sourced from https://github.com/cabal-club/cable.js#examples.

    // The circuit_id field is not currently in use; set to all zeros.
    const CIRCUIT_ID: [u8; 4] = [0, 0, 0, 0];

    /* MESSAGE TO BYTES TESTS */

    #[test]
    fn cancel_request_to_bytes() -> Result<(), Error> {
        /* HEADER FIELD VALUES */

        let msg_len = 14;
        let msg_type = 3;
        let req_id = <[u8; 4]>::from_hex("04baaffb")?;

        // Construct a new message header.
        let header = MessageHeader::new(msg_type, CIRCUIT_ID, req_id);

        /* BODY FIELD VALUES */

        let ttl = 1;
        let cancel_id = <[u8; 4]>::from_hex("31b5c9e1")?;

        // Construct a new request body.
        let req_body = RequestBody::Cancel { cancel_id };
        // Construct a new message body.
        let body = MessageBody::Request {
            body: req_body,
            ttl,
        };

        // Construct a new message.
        let msg = Message::new(header, body);
        // Convert the message to bytes.
        let msg_bytes = msg.to_bytes()?;

        // Test vector binary.
        let expected_bytes = <Vec<u8>>::from_hex("0e030000000004baaffb0131b5c9e1")?;

        // Ensure the number of generated message bytes matches the number of
        // expected bytes.
        assert_eq!(msg_bytes.len(), expected_bytes.len());

        // Ensure the generated message bytes match the expected bytes.
        assert_eq!(msg_bytes, expected_bytes);

        Ok(())
    }

    #[test]
    fn channel_time_range_request_to_bytes() -> Result<(), Error> {
        /* HEADER FIELD VALUES */

        let msg_len = 21;
        let msg_type = 4;
        let req_id = <[u8; 4]>::from_hex("04baaffb")?;

        // Construct a new message header.
        let header = MessageHeader::new(msg_type, CIRCUIT_ID, req_id);

        /* BODY FIELD VALUES */

        let ttl = 1;
        let channel = "default".to_string();
        let time_start = 0;
        let time_end = 100;
        let limit = 20;

        // Construct a new request body.
        let req_body = RequestBody::ChannelTimeRange {
            channel,
            time_start,
            time_end,
            limit,
        };
        // Construct a new message body.
        let body = MessageBody::Request {
            body: req_body,
            ttl,
        };

        // Construct a new message.
        let msg = Message::new(header, body);
        // Convert the message to bytes.
        let msg_bytes = msg.to_bytes()?;

        // Test vector binary.
        let expected_bytes = <Vec<u8>>::from_hex("15040000000004baaffb010764656661756c74006414")?;

        // Ensure the number of generated message bytes matches the number of
        // expected bytes.
        assert_eq!(msg_bytes.len(), expected_bytes.len());

        // Ensure the generated message bytes match the expected bytes.
        assert_eq!(msg_bytes, expected_bytes);

        Ok(())
    }

    #[test]
    fn channel_state_request_to_bytes() -> Result<(), Error> {
        /* HEADER FIELD VALUES */

        let msg_len = 19;
        let msg_type = 5;
        let req_id = <[u8; 4]>::from_hex("04baaffb")?;

        // Construct a new message header.
        let header = MessageHeader::new(msg_type, CIRCUIT_ID, req_id);

        /* BODY FIELD VALUES */

        let ttl = 1;
        let channel = "default".to_string();
        let future = 0;

        // Construct a new request body.
        let req_body = RequestBody::ChannelState { channel, future };
        // Construct a new message body.
        let body = MessageBody::Request {
            body: req_body,
            ttl,
        };

        // Construct a new message.
        let msg = Message::new(header, body);
        // Convert the message to bytes.
        let msg_bytes = msg.to_bytes()?;

        // Test vector binary.
        let expected_bytes = <Vec<u8>>::from_hex("13050000000004baaffb010764656661756c7400")?;

        // Ensure the number of generated message bytes matches the number of
        // expected bytes.
        assert_eq!(msg_bytes.len(), expected_bytes.len());

        // Ensure the generated message bytes match the expected bytes.
        assert_eq!(msg_bytes, expected_bytes);

        Ok(())
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
