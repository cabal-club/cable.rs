//! Message formats for all request and response message types supported by cable.
//!
//! Includes type definitions for all request and response message types,
//! as well as message header and body types. Helper methods are included.
//!
//! Also includes implementations of the `CountBytes`, `FromBytes` and `ToBytes`
//! traits for `Message`. This forms the core of the cable protocol.

use desert::{varint, CountBytes, FromBytes, ToBytes};

use crate::{
    constants::{
        CANCEL_REQUEST, CHANNEL_LIST_REQUEST, CHANNEL_LIST_RESPONSE, CHANNEL_STATE_REQUEST,
        CHANNEL_TIME_RANGE_REQUEST, HASH_RESPONSE, POST_REQUEST, POST_RESPONSE,
    },
    error::{CableErrorKind, Error},
    post::EncodedPost,
    Channel, ChannelOptions, CircuitId, Hash, ReqId, Timestamp,
};

/// A complete message including header and body values.
#[derive(Clone, Debug)]
pub struct Message {
    pub header: MessageHeader,
    pub body: MessageBody,
}

impl Message {
    /// Construct a `Message` from a header and body.
    pub fn new(header: MessageHeader, body: MessageBody) -> Self {
        Message { header, body }
    }

    /// Return the numeric type identifier for the message.
    pub fn message_type(&self) -> u64 {
        match &self.body {
            MessageBody::Request { body, .. } => match body {
                RequestBody::Post { .. } => POST_REQUEST,
                RequestBody::Cancel { .. } => CANCEL_REQUEST,
                RequestBody::ChannelTimeRange { .. } => CHANNEL_TIME_RANGE_REQUEST,
                RequestBody::ChannelState { .. } => CHANNEL_STATE_REQUEST,
                RequestBody::ChannelList { .. } => CHANNEL_LIST_REQUEST,
            },
            MessageBody::Response { body } => match body {
                ResponseBody::Hash { .. } => HASH_RESPONSE,
                ResponseBody::Post { .. } => POST_RESPONSE,
                ResponseBody::ChannelList { .. } => CHANNEL_LIST_RESPONSE,
            },
            MessageBody::Unrecognized { msg_type } => *msg_type,
        }
    }

    /// Construct a channel time range request `Message` with the given parameters.
    pub fn channel_time_range_request(
        circuit_id: CircuitId,
        req_id: ReqId,
        ttl: u8,
        channel_opts: ChannelOptions,
    ) -> Self {
        let ChannelOptions {
            channel,
            time_start,
            time_end,
            limit,
        } = channel_opts;

        let header = MessageHeader::new(CHANNEL_TIME_RANGE_REQUEST, circuit_id, req_id);
        let body = MessageBody::Request {
            ttl,
            body: RequestBody::ChannelTimeRange {
                channel,
                time_start,
                time_end,
                limit,
            },
        };

        Message::new(header, body)
    }

    /// Construct a hash response `Message` with the given parameters.
    pub fn hash_response(circuit_id: CircuitId, req_id: ReqId, hashes: Vec<Hash>) -> Self {
        let header = MessageHeader::new(HASH_RESPONSE, circuit_id, req_id);
        let body = MessageBody::Response {
            body: ResponseBody::Hash { hashes },
        };

        Message::new(header, body)
    }

    /// Construct a post response `Message` with the given parameters.
    pub fn post_response(circuit_id: CircuitId, req_id: ReqId, posts: Vec<EncodedPost>) -> Self {
        let header = MessageHeader::new(HASH_RESPONSE, circuit_id, req_id);
        let body = MessageBody::Response {
            body: ResponseBody::Post { posts },
        };

        Message::new(header, body)
    }

    /// Construct a post request `Message` with the given parameters.
    pub fn post_request(circuit_id: CircuitId, req_id: ReqId, ttl: u8, hashes: Vec<Hash>) -> Self {
        let header = MessageHeader::new(POST_REQUEST, circuit_id, req_id);
        let body = MessageBody::Request {
            ttl,
            body: RequestBody::Post { hashes },
        };

        Message::new(header, body)
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
    /// A message type which is not recognised as part of the cable specification.
    Unrecognized {
        msg_type: u64,
    },
}

#[derive(Clone, Debug)]
/// The body of a request message.
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
}

#[derive(Clone, Debug)]
/// The body of a response message.
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
        // TODO: Should this be `Post` instead of `EncodedPost`?
        posts: Vec<EncodedPost>,
    },
    /// Respond with a list of names of known channels.
    ///
    /// Message type (`msg_type`) is `7`.
    ChannelList {
        /// A list of channels, with each one including the length and name of a channel.
        channels: Vec<Channel>,
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
            },
            MessageBody::Unrecognized { .. } => 0,
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
            },
            MessageBody::Unrecognized { msg_type } => {
                return CableErrorKind::MessageWriteUnrecognizedType {
                    msg_type: *msg_type,
                }
                .raise();
            }
        }

        Ok(offset)
    }
}

impl FromBytes for Message {
    /// Read bytes from the given buffer (byte array), returning the total
    /// number of bytes and the decoded `Message` type.
    fn from_bytes(buf: &[u8]) -> Result<(usize, Self), Error> {
        if buf.is_empty() {
            return CableErrorKind::MessageEmpty {}.raise();
        }

        let mut offset = 0;

        /* MESSAGE HEADER BYTES */

        // Read the message length byte from the buffer and increment the
        // offset.
        let (s, _num_bytes) = varint::decode(&buf[offset..])?;
        offset += s;

        // Read the message-type byte from the buffer and increment the offset.
        let (s, msg_type) = varint::decode(&buf[offset..])?;
        offset += s;

        // Read the circuit ID bytes from the buffer and increment the offset.
        let mut circuit_id = [0; 4];
        circuit_id.copy_from_slice(&buf[offset..offset + 4]);
        offset += 4;

        // Read the request ID bytes from the buffer and increment the offset.
        let mut req_id = [0; 4];
        req_id.copy_from_slice(&buf[offset..offset + 4]);
        offset += 4;

        // Construct the message header.
        let header = MessageHeader {
            msg_type,
            circuit_id,
            req_id,
        };

        /* MESSAGE BODY BYTES */

        // Read message body field bytes.
        let body = match msg_type {
            HASH_RESPONSE => {
                // Read the number of hashes byte and increment the offset.
                let (s, num_hashes) = varint::decode(&buf[offset..])?;
                offset += s;

                let mut hashes = Vec::with_capacity(num_hashes as usize);

                // Iterate over the hashes, reading the bytes from the buffer
                // and incrementing the offset for each one.
                for _ in 0..num_hashes {
                    if offset + 32 > buf.len() {
                        return CableErrorKind::MessageHashResponseEnd {}.raise();
                    }

                    let mut hash = [0; 32];
                    hash.copy_from_slice(&buf[offset..offset + 32]);
                    offset += 32;

                    hashes.push(hash);
                }

                // Construct a new response body.
                let res_body = ResponseBody::Hash { hashes };

                MessageBody::Response { body: res_body }
            }
            POST_RESPONSE => {
                // Create an empty vector to store encoded posts.
                let mut posts: Vec<EncodedPost> = Vec::new();

                // Since there may be several posts, we use a loop
                // to iterate over the bytes.
                loop {
                    // Read the post length byte and increment the offset.
                    let (s, post_len) = varint::decode(&buf[offset..])?;
                    offset += s;

                    // A post length value of 0 indicates that there are no
                    // more posts to come.
                    if post_len == 0 {
                        // Break out of the loop.
                        break;
                    }

                    // Read the post bytes and increment the offset.
                    let post = buf[offset..offset + post_len as usize].to_vec();
                    offset += post_len as usize;

                    posts.push(post);
                }

                // Construct a new response body.
                let res_body = ResponseBody::Post { posts };

                MessageBody::Response { body: res_body }
            }
            POST_REQUEST => {
                // Read the TTL byte and increment the offset.
                let (s, ttl) = varint::decode(&buf[offset..])?;
                offset += s;

                // Read the number of hashes byte and increment the offset.
                let (s, num_hashes) = varint::decode(&buf[offset..])?;
                offset += s;

                let mut hashes = Vec::with_capacity(num_hashes as usize);

                // Iterate over the hashes, reading the bytes from the buffer
                // and incrementing the offset for each one.
                for _ in 0..num_hashes {
                    if offset + 32 > buf.len() {
                        return CableErrorKind::MessageHashResponseEnd {}.raise();
                    }

                    let mut hash = [0; 32];
                    hash.copy_from_slice(&buf[offset..offset + 32]);
                    offset += 32;

                    hashes.push(hash);
                }

                // Construct a new request body.
                let req_body = RequestBody::Post { hashes };

                MessageBody::Request {
                    ttl: ttl as u8,
                    body: req_body,
                }
            }
            CANCEL_REQUEST => {
                // Read the TTL byte and increment the offset.
                let (s, ttl) = varint::decode(&buf[offset..])?;
                offset += s;

                // Read the cancel request ID bytes from the buffer and
                // increment the offset.
                let mut cancel_id = [0; 4];
                cancel_id.copy_from_slice(&buf[offset..offset + 4]);
                offset += 4;

                // Construct a new request body.
                let req_body = RequestBody::Cancel { cancel_id };

                MessageBody::Request {
                    ttl: ttl as u8,
                    body: req_body,
                }
            }
            CHANNEL_TIME_RANGE_REQUEST => {
                // Read the TTL byte and increment the offset.
                let (s, ttl) = varint::decode(&buf[offset..])?;
                offset += s;

                // Read the channel length byte and increment the offset.
                let (s, channel_len) = varint::decode(&buf[offset..])?;
                offset += s;

                // Read the channel bytes and increment the offset.
                let channel =
                    String::from_utf8(buf[offset..offset + channel_len as usize].to_vec())?;
                offset += channel_len as usize;

                // Read the time start byte and increment the offset.
                let (s, time_start) = varint::decode(&buf[offset..])?;
                offset += s;

                // Read the time end byte and increment the offset.
                let (s, time_end) = varint::decode(&buf[offset..])?;
                offset += s;

                // Read the limit byte and increment the offset.
                let (s, limit) = varint::decode(&buf[offset..])?;
                offset += s;

                // Construct a new request body.
                let req_body = RequestBody::ChannelTimeRange {
                    channel,
                    time_start,
                    time_end,
                    limit,
                };
                MessageBody::Request {
                    ttl: ttl as u8,
                    body: req_body,
                }
            }
            CHANNEL_STATE_REQUEST => {
                // Read the TTL byte and increment the offset.
                let (s, ttl) = varint::decode(&buf[offset..])?;
                offset += s;

                // Read the channel length byte and increment the offset.
                let (s, channel_len) = varint::decode(&buf[offset..])?;
                offset += s;

                // Read the channel bytes and increment the offset.
                let channel =
                    String::from_utf8(buf[offset..offset + channel_len as usize].to_vec())?;
                offset += channel_len as usize;

                // Read the future byte and increment the offset.
                let (s, future) = varint::decode(&buf[offset..])?;
                offset += s;

                // Construct a new request body.
                let req_body = RequestBody::ChannelState { channel, future };

                MessageBody::Request {
                    ttl: ttl as u8,
                    body: req_body,
                }
            }
            CHANNEL_LIST_REQUEST => {
                // Read the TTL byte and increment the offset.
                let (s, ttl) = varint::decode(&buf[offset..])?;
                offset += s;

                // Read the skip byte and increment the offset.
                let (s, skip) = varint::decode(&buf[offset..])?;
                offset += s;

                // Read the limit byte and increment the offset.
                let (s, limit) = varint::decode(&buf[offset..])?;
                offset += s;

                // Construct a new request body.
                let req_body = RequestBody::ChannelList { skip, limit };

                MessageBody::Request {
                    ttl: ttl as u8,
                    body: req_body,
                }
            }
            CHANNEL_LIST_RESPONSE => {
                // Create an empty vector to store channel names.
                let mut channels: Vec<Channel> = Vec::new();

                // Since there may be several channels, we use a loop
                // to iterate over the bytes.
                loop {
                    // Read the channel length byte and increment the offset.
                    let (s, channel_len) = varint::decode(&buf[offset..])?;
                    offset += s;

                    // A channel length value of 0 indicates that there are no
                    // more key-value pairs to come.
                    if channel_len == 0 {
                        // Break out of the loop.
                        break;
                    }

                    // Read the key bytes and increment the offset.
                    let channel =
                        String::from_utf8(buf[offset..offset + channel_len as usize].to_vec())?;
                    offset += channel_len as usize;

                    channels.push(channel);
                }

                // Construct a new response body.
                let res_body = ResponseBody::ChannelList { channels };

                MessageBody::Response { body: res_body }
            }
            msg_type => MessageBody::Unrecognized { msg_type },
        };

        Ok((offset, Message { header, body }))
    }
}

#[cfg(test)]
mod test {
    use crate::constants::NO_CIRCUIT;

    use super::{
        EncodedPost, Error, FromBytes, Hash, Message, MessageBody, MessageHeader, RequestBody,
        ResponseBody, ToBytes, CANCEL_REQUEST, CHANNEL_LIST_REQUEST, CHANNEL_LIST_RESPONSE,
        CHANNEL_STATE_REQUEST, CHANNEL_TIME_RANGE_REQUEST, HASH_RESPONSE, POST_REQUEST,
        POST_RESPONSE,
    };

    use hex::FromHex;

    // Field values sourced from https://github.com/cabal-club/cable.js#examples.

    // The circuit_id field is not currently in use; set to all zeros.
    const CIRCUIT_ID: [u8; 4] = NO_CIRCUIT;
    const REQ_ID: &str = "04baaffb";
    const TTL: u8 = 1;

    const CANCEL_ID: &str = "31b5c9e1";

    const ENCODED_POST: &str = "25b272a71555322d40efe449a7f99af8fd364b92d350f1664481b2da340a02d0abb083ecdca569f064564942ddf1944fbf550dc27ea36a7074be798d753cb029703de77b1a9532b6ca2ec5706e297dce073d6e508eeb425c32df8431e4677805015049d089a650aa896cb25ec35258653be4df196b4a5e5b6db7ed024aaa89e1b305500764656661756c74";

    const POST_REQUEST_HEX_BINARY: &str = "6b020000000004baaffb010315ed54965515babf6f16be3f96b04b29ecca813a343311dae483691c07ccf4e597fc63631c41384226b9b68d9f73ffaaf6eac54b71838687f48f112e30d6db689c2939fec6d47b00bafe6967aeff697cf4b5abca01b04ba1b31a7e3752454bfa";
    const CANCEL_REQUEST_HEX_BINARY: &str = "0e030000000004baaffb0131b5c9e1";
    const CHANNEL_TIME_RANGE_REQUEST_HEX_BINARY: &str =
        "15040000000004baaffb010764656661756c74006414";
    const CHANNEL_STATE_REQUEST_HEX_BINARY: &str = "13050000000004baaffb010764656661756c7400";
    const CHANNEL_LIST_REQUEST_HEX_BINARY: &str = "0c060000000004baaffb010014";
    const HASH_RESPONSE_HEX_BINARY: &str = "6a000000000004baaffb0315ed54965515babf6f16be3f96b04b29ecca813a343311dae483691c07ccf4e597fc63631c41384226b9b68d9f73ffaaf6eac54b71838687f48f112e30d6db689c2939fec6d47b00bafe6967aeff697cf4b5abca01b04ba1b31a7e3752454bfa";
    const POST_RESPONSE_HEX_BINARY: &str = "9701010000000004baaffb8b0125b272a71555322d40efe449a7f99af8fd364b92d350f1664481b2da340a02d0abb083ecdca569f064564942ddf1944fbf550dc27ea36a7074be798d753cb029703de77b1a9532b6ca2ec5706e297dce073d6e508eeb425c32df8431e4677805015049d089a650aa896cb25ec35258653be4df196b4a5e5b6db7ed024aaa89e1b305500764656661756c7400";
    const CHANNEL_LIST_RESPONSE_HEX_BINARY: &str =
        "23070000000004baaffb0764656661756c74036465760c696e74726f64756374696f6e00";

    /* MESSAGE TO BYTES TESTS */

    #[test]
    fn post_request_to_bytes() -> Result<(), Error> {
        /* HEADER FIELD VALUES */

        let msg_type = POST_REQUEST;
        let req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        // Construct a new message header.
        let header = MessageHeader::new(msg_type, CIRCUIT_ID, req_id);

        /* BODY FIELD VALUES */

        // Create a vector of hashes.
        let hashes: Vec<Hash> = vec![
            <[u8; 32]>::from_hex(
                "15ed54965515babf6f16be3f96b04b29ecca813a343311dae483691c07ccf4e5",
            )?,
            <[u8; 32]>::from_hex(
                "97fc63631c41384226b9b68d9f73ffaaf6eac54b71838687f48f112e30d6db68",
            )?,
            <[u8; 32]>::from_hex(
                "9c2939fec6d47b00bafe6967aeff697cf4b5abca01b04ba1b31a7e3752454bfa",
            )?,
        ];

        // Construct a new request body.
        let req_body = RequestBody::Post { hashes };
        // Construct a new message body.
        let body = MessageBody::Request {
            ttl: TTL,
            body: req_body,
        };

        // Construct a new message.
        let msg = Message::new(header, body);
        // Convert the message to bytes.
        let msg_bytes = msg.to_bytes()?;

        // Test vector binary.
        let expected_bytes = <Vec<u8>>::from_hex(POST_REQUEST_HEX_BINARY)?;

        // Ensure the number of generated message bytes matches the number of
        // expected bytes.
        assert_eq!(msg_bytes.len(), expected_bytes.len());

        // Ensure the generated message bytes match the expected bytes.
        assert_eq!(msg_bytes, expected_bytes);

        Ok(())
    }

    #[test]
    fn cancel_request_to_bytes() -> Result<(), Error> {
        /* HEADER FIELD VALUES */

        let msg_type = CANCEL_REQUEST;
        let req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        // Construct a new message header.
        let header = MessageHeader::new(msg_type, CIRCUIT_ID, req_id);

        /* BODY FIELD VALUES */

        let cancel_id = <[u8; 4]>::from_hex(CANCEL_ID)?;

        // Construct a new request body.
        let req_body = RequestBody::Cancel { cancel_id };
        // Construct a new message body.
        let body = MessageBody::Request {
            body: req_body,
            ttl: TTL,
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

        let msg_type = CHANNEL_TIME_RANGE_REQUEST;
        let req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        // Construct a new message header.
        let header = MessageHeader::new(msg_type, CIRCUIT_ID, req_id);

        /* BODY FIELD VALUES */

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
            ttl: TTL,
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

        let msg_type = CHANNEL_STATE_REQUEST;
        let req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        // Construct a new message header.
        let header = MessageHeader::new(msg_type, CIRCUIT_ID, req_id);

        /* BODY FIELD VALUES */

        let channel = "default".to_string();
        let future = 0;

        // Construct a new request body.
        let req_body = RequestBody::ChannelState { channel, future };
        // Construct a new message body.
        let body = MessageBody::Request {
            body: req_body,
            ttl: TTL,
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

    #[test]
    fn channel_list_request_to_bytes() -> Result<(), Error> {
        /* HEADER FIELD VALUES */

        let msg_type = CHANNEL_LIST_REQUEST;
        let req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        // Construct a new message header.
        let header = MessageHeader::new(msg_type, CIRCUIT_ID, req_id);

        /* BODY FIELD VALUES */

        let skip = 0;
        let limit = 20;

        // Construct a new request body.
        let req_body = RequestBody::ChannelList { skip, limit };
        // Construct a new message body.
        let body = MessageBody::Request {
            body: req_body,
            ttl: TTL,
        };

        // Construct a new message.
        let msg = Message::new(header, body);
        // Convert the message to bytes.
        let msg_bytes = msg.to_bytes()?;

        // Test vector binary.
        let expected_bytes = <Vec<u8>>::from_hex("0c060000000004baaffb010014")?;

        // Ensure the number of generated message bytes matches the number of
        // expected bytes.
        assert_eq!(msg_bytes.len(), expected_bytes.len());

        // Ensure the generated message bytes match the expected bytes.
        assert_eq!(msg_bytes, expected_bytes);

        Ok(())
    }

    #[test]
    fn hash_response_to_bytes() -> Result<(), Error> {
        /* HEADER FIELD VALUES */

        let msg_type = HASH_RESPONSE;
        let req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        // Construct a new message header.
        let header = MessageHeader::new(msg_type, CIRCUIT_ID, req_id);

        /* BODY FIELD VALUES */

        // Create a vector of hashes.
        let hashes: Vec<Hash> = vec![
            <[u8; 32]>::from_hex(
                "15ed54965515babf6f16be3f96b04b29ecca813a343311dae483691c07ccf4e5",
            )?,
            <[u8; 32]>::from_hex(
                "97fc63631c41384226b9b68d9f73ffaaf6eac54b71838687f48f112e30d6db68",
            )?,
            <[u8; 32]>::from_hex(
                "9c2939fec6d47b00bafe6967aeff697cf4b5abca01b04ba1b31a7e3752454bfa",
            )?,
        ];

        // Construct a new response body.
        let res_body = ResponseBody::Hash { hashes };
        // Construct a new message body.
        let body = MessageBody::Response { body: res_body };

        // Construct a new message.
        let msg = Message::new(header, body);
        // Convert the message to bytes.
        let msg_bytes = msg.to_bytes()?;

        // Test vector binary.
        let expected_bytes = <Vec<u8>>::from_hex("6a000000000004baaffb0315ed54965515babf6f16be3f96b04b29ecca813a343311dae483691c07ccf4e597fc63631c41384226b9b68d9f73ffaaf6eac54b71838687f48f112e30d6db689c2939fec6d47b00bafe6967aeff697cf4b5abca01b04ba1b31a7e3752454bfa")?;

        // Ensure the number of generated message bytes matches the number of
        // expected bytes.
        assert_eq!(msg_bytes.len(), expected_bytes.len());

        // Ensure the generated message bytes match the expected bytes.
        assert_eq!(msg_bytes, expected_bytes);

        Ok(())
    }

    #[test]
    fn post_response_to_bytes() -> Result<(), Error> {
        /* HEADER FIELD VALUES */

        let msg_type = POST_RESPONSE;
        let req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        // Construct a new message header.
        let header = MessageHeader::new(msg_type, CIRCUIT_ID, req_id);

        /* BODY FIELD VALUES */

        // Create a vector of encoded posts.
        let posts: Vec<EncodedPost> = vec![<Vec<u8>>::from_hex("25b272a71555322d40efe449a7f99af8fd364b92d350f1664481b2da340a02d0abb083ecdca569f064564942ddf1944fbf550dc27ea36a7074be798d753cb029703de77b1a9532b6ca2ec5706e297dce073d6e508eeb425c32df8431e4677805015049d089a650aa896cb25ec35258653be4df196b4a5e5b6db7ed024aaa89e1b305500764656661756c74")?];

        // Construct a new response body.
        let res_body = ResponseBody::Post { posts };
        // Construct a new message body.
        let body = MessageBody::Response { body: res_body };

        // Construct a new message.
        let msg = Message::new(header, body);
        // Convert the message to bytes.
        let msg_bytes = msg.to_bytes()?;

        // Test vector binary.
        let expected_bytes = <Vec<u8>>::from_hex("9701010000000004baaffb8b0125b272a71555322d40efe449a7f99af8fd364b92d350f1664481b2da340a02d0abb083ecdca569f064564942ddf1944fbf550dc27ea36a7074be798d753cb029703de77b1a9532b6ca2ec5706e297dce073d6e508eeb425c32df8431e4677805015049d089a650aa896cb25ec35258653be4df196b4a5e5b6db7ed024aaa89e1b305500764656661756c7400")?;

        // Ensure the number of generated message bytes matches the number of
        // expected bytes.
        assert_eq!(msg_bytes.len(), expected_bytes.len());

        // Ensure the generated message bytes match the expected bytes.
        assert_eq!(msg_bytes, expected_bytes);

        Ok(())
    }

    #[test]
    fn channel_list_response_to_bytes() -> Result<(), Error> {
        /* HEADER FIELD VALUES */

        let msg_type = CHANNEL_LIST_RESPONSE;
        let req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        // Construct a new message header.
        let header = MessageHeader::new(msg_type, CIRCUIT_ID, req_id);

        /* BODY FIELD VALUES */

        // Create a vector of channels.
        let channels = vec![
            "default".to_string(),
            "dev".to_string(),
            "introduction".to_string(),
        ];

        // Construct a new response body.
        let res_body = ResponseBody::ChannelList { channels };
        // Construct a new message body.
        let body = MessageBody::Response { body: res_body };

        // Construct a new message.
        let msg = Message::new(header, body);
        // Convert the message to bytes.
        let msg_bytes = msg.to_bytes()?;

        // Test vector binary.
        let expected_bytes = <Vec<u8>>::from_hex(
            "23070000000004baaffb0764656661756c74036465760c696e74726f64756374696f6e00",
        )?;

        // Ensure the number of generated message bytes matches the number of
        // expected bytes.
        assert_eq!(msg_bytes.len(), expected_bytes.len());

        // Ensure the generated message bytes match the expected bytes.
        assert_eq!(msg_bytes, expected_bytes);

        Ok(())
    }

    /* BYTES TO MESSAGE TESTS */

    #[test]
    fn bytes_to_post_request() -> Result<(), Error> {
        // Test vector binary.
        let msg_bytes = <Vec<u8>>::from_hex(POST_REQUEST_HEX_BINARY)?;

        // Decode the byte slice to `Message`.
        let (_, msg) = Message::from_bytes(&msg_bytes)?;

        /* HEADER FIELD VALUES */

        let expected_msg_type = POST_REQUEST;
        let expected_circuit_id = CIRCUIT_ID;
        let expected_req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        let MessageHeader {
            msg_type,
            circuit_id,
            req_id,
        } = msg.header;

        // Ensure the message header fields are correct.
        assert_eq!(msg_type, expected_msg_type);
        assert_eq!(circuit_id, expected_circuit_id);
        assert_eq!(req_id, expected_req_id);

        /* BODY FIELD VALUES */

        let expected_hashes: Vec<Hash> = vec![
            <[u8; 32]>::from_hex(
                "15ed54965515babf6f16be3f96b04b29ecca813a343311dae483691c07ccf4e5",
            )?,
            <[u8; 32]>::from_hex(
                "97fc63631c41384226b9b68d9f73ffaaf6eac54b71838687f48f112e30d6db68",
            )?,
            <[u8; 32]>::from_hex(
                "9c2939fec6d47b00bafe6967aeff697cf4b5abca01b04ba1b31a7e3752454bfa",
            )?,
        ];

        // Ensure the message body fields are correct.
        if let MessageBody::Request { ttl, body } = msg.body {
            assert_eq!(ttl, TTL);
            if let RequestBody::Post { hashes } = body {
                assert_eq!(hashes, expected_hashes);
            } else {
                panic!("Incorrect message type: expected post request");
            }
        } else {
            panic!("Incorrect message body type: expected request");
        }

        Ok(())
    }

    #[test]
    fn bytes_to_cancel_request() -> Result<(), Error> {
        // Test vector binary.
        let msg_bytes = <Vec<u8>>::from_hex(CANCEL_REQUEST_HEX_BINARY)?;

        // Decode the byte slice to `Message`.
        let (_, msg) = Message::from_bytes(&msg_bytes)?;

        /* HEADER FIELD VALUES */

        let expected_msg_type = CANCEL_REQUEST;
        let expected_circuit_id = CIRCUIT_ID;
        let expected_req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        let MessageHeader {
            msg_type,
            circuit_id,
            req_id,
        } = msg.header;

        // Ensure the message header fields are correct.
        assert_eq!(msg_type, expected_msg_type);
        assert_eq!(circuit_id, expected_circuit_id);
        assert_eq!(req_id, expected_req_id);

        /* BODY FIELD VALUES */

        let expected_cancel_id = <[u8; 4]>::from_hex(CANCEL_ID)?;

        // Ensure the message body fields are correct.
        if let MessageBody::Request { ttl, body } = msg.body {
            assert_eq!(ttl, TTL);
            if let RequestBody::Cancel { cancel_id } = body {
                assert_eq!(cancel_id, expected_cancel_id);
            } else {
                panic!("Incorrect message type: expected cancel request");
            }
        } else {
            panic!("Incorrect message body type: expected request");
        }

        Ok(())
    }

    #[test]
    fn bytes_to_channel_time_range_request() -> Result<(), Error> {
        // Test vector binary.
        let msg_bytes = <Vec<u8>>::from_hex(CHANNEL_TIME_RANGE_REQUEST_HEX_BINARY)?;

        // Decode the byte slice to `Message`.
        let (_, msg) = Message::from_bytes(&msg_bytes)?;

        /* HEADER FIELD VALUES */

        let expected_msg_type = CHANNEL_TIME_RANGE_REQUEST;
        let expected_circuit_id = CIRCUIT_ID;
        let expected_req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        let MessageHeader {
            msg_type,
            circuit_id,
            req_id,
        } = msg.header;

        // Ensure the message header fields are correct.
        assert_eq!(msg_type, expected_msg_type);
        assert_eq!(circuit_id, expected_circuit_id);
        assert_eq!(req_id, expected_req_id);

        /* BODY FIELD VALUES */

        let expected_channel = "default".to_string();
        let expected_time_start = 0;
        let expected_time_end = 100;
        let expected_limit = 20;

        // Ensure the message body fields are correct.
        if let MessageBody::Request { ttl, body } = msg.body {
            assert_eq!(ttl, TTL);
            if let RequestBody::ChannelTimeRange {
                channel,
                time_start,
                time_end,
                limit,
            } = body
            {
                assert_eq!(channel, expected_channel);
                assert_eq!(time_start, expected_time_start);
                assert_eq!(time_end, expected_time_end);
                assert_eq!(limit, expected_limit);
            } else {
                panic!("Incorrect message type: expected channnel time range request");
            }
        } else {
            panic!("Incorrect message body type: expected request");
        }

        Ok(())
    }

    #[test]
    fn bytes_to_channel_state_request() -> Result<(), Error> {
        // Test vector binary.
        let msg_bytes = <Vec<u8>>::from_hex(CHANNEL_STATE_REQUEST_HEX_BINARY)?;

        // Decode the byte slice to `Message`.
        let (_, msg) = Message::from_bytes(&msg_bytes)?;

        /* HEADER FIELD VALUES */

        let expected_msg_type = CHANNEL_STATE_REQUEST;
        let expected_circuit_id = CIRCUIT_ID;
        let expected_req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        let MessageHeader {
            msg_type,
            circuit_id,
            req_id,
        } = msg.header;

        // Ensure the message header fields are correct.
        assert_eq!(msg_type, expected_msg_type);
        assert_eq!(circuit_id, expected_circuit_id);
        assert_eq!(req_id, expected_req_id);

        /* BODY FIELD VALUES */

        let expected_channel = "default".to_string();
        let expected_future = 0;

        // Ensure the message body fields are correct.
        if let MessageBody::Request { ttl, body } = msg.body {
            assert_eq!(ttl, TTL);
            if let RequestBody::ChannelState { channel, future } = body {
                assert_eq!(channel, expected_channel);
                assert_eq!(future, expected_future);
            } else {
                panic!("Incorrect message type: expected channnel state request");
            }
        } else {
            panic!("Incorrect message body type: expected request");
        }

        Ok(())
    }

    #[test]
    fn bytes_to_channel_list_request() -> Result<(), Error> {
        // Test vector binary.
        let msg_bytes = <Vec<u8>>::from_hex(CHANNEL_LIST_REQUEST_HEX_BINARY)?;

        // Decode the byte slice to `Message`.
        let (_, msg) = Message::from_bytes(&msg_bytes)?;

        /* HEADER FIELD VALUES */

        let expected_msg_type = CHANNEL_LIST_REQUEST;
        let expected_circuit_id = CIRCUIT_ID;
        let expected_req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        let MessageHeader {
            msg_type,
            circuit_id,
            req_id,
        } = msg.header;

        // Ensure the message header fields are correct.
        assert_eq!(msg_type, expected_msg_type);
        assert_eq!(circuit_id, expected_circuit_id);
        assert_eq!(req_id, expected_req_id);

        /* BODY FIELD VALUES */

        let expected_skip = 0;
        let expected_limit = 20;

        // Ensure the message body fields are correct.
        if let MessageBody::Request { ttl, body } = msg.body {
            assert_eq!(ttl, TTL);
            if let RequestBody::ChannelList { skip, limit } = body {
                assert_eq!(skip, expected_skip);
                assert_eq!(limit, expected_limit);
            } else {
                panic!("Incorrect message type: expected channnel list request");
            }
        } else {
            panic!("Incorrect message body type: expected request");
        }

        Ok(())
    }

    #[test]
    fn bytes_to_hash_response() -> Result<(), Error> {
        // Test vector binary.
        let msg_bytes = <Vec<u8>>::from_hex(HASH_RESPONSE_HEX_BINARY)?;

        // Decode the byte slice to `Message`.
        let (_, msg) = Message::from_bytes(&msg_bytes)?;

        /* HEADER FIELD VALUES */

        let expected_msg_type = HASH_RESPONSE;
        let expected_circuit_id = CIRCUIT_ID;
        let expected_req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        let MessageHeader {
            msg_type,
            circuit_id,
            req_id,
        } = msg.header;

        // Ensure the message header fields are correct.
        assert_eq!(msg_type, expected_msg_type);
        assert_eq!(circuit_id, expected_circuit_id);
        assert_eq!(req_id, expected_req_id);

        /* BODY FIELD VALUES */

        let expected_hashes: Vec<Hash> = vec![
            <[u8; 32]>::from_hex(
                "15ed54965515babf6f16be3f96b04b29ecca813a343311dae483691c07ccf4e5",
            )?,
            <[u8; 32]>::from_hex(
                "97fc63631c41384226b9b68d9f73ffaaf6eac54b71838687f48f112e30d6db68",
            )?,
            <[u8; 32]>::from_hex(
                "9c2939fec6d47b00bafe6967aeff697cf4b5abca01b04ba1b31a7e3752454bfa",
            )?,
        ];

        // Ensure the message body fields are correct.
        if let MessageBody::Response { body } = msg.body {
            if let ResponseBody::Hash { hashes } = body {
                assert_eq!(hashes, expected_hashes);
            } else {
                panic!("Incorrect message type: expected hash response");
            }
        } else {
            panic!("Incorrect message body type: expected response");
        }

        Ok(())
    }

    #[test]
    fn bytes_to_post_response() -> Result<(), Error> {
        // Test vector binary.
        let msg_bytes = <Vec<u8>>::from_hex(POST_RESPONSE_HEX_BINARY)?;

        // Decode the byte slice to `Message`.
        let (_, msg) = Message::from_bytes(&msg_bytes)?;

        /* HEADER FIELD VALUES */

        let expected_msg_type = POST_RESPONSE;
        let expected_circuit_id = CIRCUIT_ID;
        let expected_req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        let MessageHeader {
            msg_type,
            circuit_id,
            req_id,
        } = msg.header;

        // Ensure the message header fields are correct.
        assert_eq!(msg_type, expected_msg_type);
        assert_eq!(circuit_id, expected_circuit_id);
        assert_eq!(req_id, expected_req_id);

        /* BODY FIELD VALUES */

        let expected_posts: Vec<EncodedPost> = vec![<Vec<u8>>::from_hex(ENCODED_POST)?];

        // Ensure the message body fields are correct.
        if let MessageBody::Response { body } = msg.body {
            if let ResponseBody::Post { posts } = body {
                assert_eq!(posts, expected_posts);
            } else {
                panic!("Incorrect message type: expected post response");
            }
        } else {
            panic!("Incorrect message body type: expected response");
        }

        Ok(())
    }

    #[test]
    fn bytes_to_channel_list_response() -> Result<(), Error> {
        // Test vector binary.
        let msg_bytes = <Vec<u8>>::from_hex(CHANNEL_LIST_RESPONSE_HEX_BINARY)?;

        // Decode the byte slice to `Message`.
        let (_, msg) = Message::from_bytes(&msg_bytes)?;

        /* HEADER FIELD VALUES */

        let expected_msg_type = CHANNEL_LIST_RESPONSE;
        let expected_circuit_id = CIRCUIT_ID;
        let expected_req_id = <[u8; 4]>::from_hex(REQ_ID)?;

        let MessageHeader {
            msg_type,
            circuit_id,
            req_id,
        } = msg.header;

        // Ensure the message header fields are correct.
        assert_eq!(msg_type, expected_msg_type);
        assert_eq!(circuit_id, expected_circuit_id);
        assert_eq!(req_id, expected_req_id);

        /* BODY FIELD VALUES */

        // Create a vector of channels.
        let expected_channels = vec![
            "default".to_string(),
            "dev".to_string(),
            "introduction".to_string(),
        ];

        // Ensure the message body fields are correct.
        if let MessageBody::Response { body } = msg.body {
            if let ResponseBody::ChannelList { channels } = body {
                assert_eq!(channels, expected_channels);
            } else {
                panic!("Incorrect message type: expected channel list response");
            }
        } else {
            panic!("Incorrect message body type: expected response");
        }

        Ok(())
    }
}
