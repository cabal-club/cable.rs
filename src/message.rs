use crate::{error::CableErrorKind as E, Channel, Error, Hash, Payload, ReqId};
use desert::{varint, CountBytes, FromBytes, ToBytes};

#[derive(Clone, Debug)]
pub enum Message {
    HashResponse {
        req_id: ReqId,
        //reply_id: ReplyId,
        hashes: Vec<Hash>,
    },
    DataResponse {
        req_id: ReqId,
        //reply_id: ReplyId,
        data: Vec<Payload>,
    },
    HashRequest {
        req_id: ReqId,
        //reply_id: ReplyId,
        ttl: usize,
        hashes: Vec<Hash>,
    },
    CancelRequest {
        req_id: ReqId,
    },
    ChannelTimeRangeRequest {
        req_id: ReqId,
        //reply_id: ReplyId,
        ttl: usize,
        channel: Channel,
        time_start: u64,
        time_end: u64,
        limit: usize,
    },
    ChannelStateRequest {
        req_id: ReqId,
        //reply_id: ReplyId,
        ttl: usize,
        channel: Channel,
        limit: usize,
        updates: usize,
    },
    ChannelListRequest {
        req_id: ReqId,
        //reply_id: ReplyId,
        ttl: usize,
        limit: usize,
    },
    Unrecognized {
        msg_type: u64,
    },
}

impl CountBytes for Message {
    fn count_bytes(&self) -> usize {
        let size = match self {
            Self::HashResponse { hashes, .. } => {
                varint::length(0) + 4 + varint::length(hashes.len() as u64) + hashes.len() * 32
            }
            Self::DataResponse { data, .. } => {
                varint::length(1)
                    + 4
                    + data
                        .iter()
                        .fold(0, |sum, d| sum + varint::length(d.len() as u64) + d.len())
                    + varint::length(0)
            }
            Self::HashRequest { ttl, hashes, .. } => {
                varint::length(2)
                    + 4
                    + varint::length(*ttl as u64)
                    + varint::length(hashes.len() as u64)
                    + hashes.len() * 32
            }
            Self::CancelRequest { .. } => varint::length(3) + 4,
            Self::ChannelTimeRangeRequest {
                ttl,
                channel,
                time_start,
                time_end,
                limit,
                ..
            } => {
                varint::length(4)
                    + 4
                    + varint::length(*ttl as u64)
                    + varint::length(channel.len() as u64)
                    + channel.len()
                    + varint::length(*time_start)
                    + varint::length(*time_end)
                    + varint::length(*limit as u64)
            }
            Self::ChannelStateRequest {
                ttl,
                channel,
                limit,
                updates,
                ..
            } => {
                varint::length(5)
                    + 4
                    + varint::length(*ttl as u64)
                    + varint::length(channel.len() as u64)
                    + channel.len()
                    + varint::length(*limit as u64)
                    + varint::length(*updates as u64)
            }
            Self::ChannelListRequest { ttl, limit, .. } => {
                varint::length(6) + 4 + varint::length(*ttl as u64) + varint::length(*limit as u64)
            }
            Self::Unrecognized { .. } => 0,
        };
        varint::length(size as u64) + size
    }
    fn count_from_bytes(buf: &[u8]) -> Result<usize, Error> {
        if buf.is_empty() {
            return E::MessageEmpty {}.raise();
        }
        let (s, msg_len) = varint::decode(buf)?;
        Ok(s + (msg_len as usize))
    }
}

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
