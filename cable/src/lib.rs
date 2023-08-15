#![doc=include_str!("../README.md")]

use std::fmt;

pub mod constants;
pub mod error;
pub mod message;
pub mod post;
pub mod validation;

// Public exports for library user convenience.
pub use crate::{error::Error, message::Message, post::Post};

use crate::error::CableErrorKind;

/// The name of a channel.
pub type Channel = String;
/// The circuit ID for an established path.
pub type CircuitId = [u8; 4];
/// An encoded channel name.
pub type EncodedChannel = Vec<u8>;
/// A BLAKE2b digest (hash).
pub type Hash = [u8; 32];
/// The binary payload of an encoded post or message.
pub type Payload = Vec<u8>;
/// The unique ID of a request and any corresponding responses.
pub type ReqId = [u8; 4];
/// The text of a post.
pub type Text = String;
/// Time in milliseconds since the UNIX Epoch.
pub type Timestamp = u64;
/// The topic of a channel.
pub type Topic = String;

#[derive(Clone, Debug, PartialEq)]
/// Query parameters defining a channel, time range and number of posts.
pub struct ChannelOptions {
    pub channel: Channel,
    pub time_start: Timestamp,
    pub time_end: Timestamp,
    pub limit: u64,
}

impl ChannelOptions {
    /// Create a new instance of `ChannelOptions`.
    pub fn new<T: Into<String>>(channel: T, time_start: u64, time_end: u64, limit: u64) -> Self {
        ChannelOptions {
            channel: channel.into(),
            time_start,
            time_end,
            limit,
        }
    }
}

/// Print channel options.
impl fmt::Display for ChannelOptions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "channel: {:?}, start: {}, end: {}, limit: {}",
            &self.channel, &self.time_start, &self.time_end, &self.limit
        )
    }
}

#[derive(Clone, PartialEq)]
/// Information self-published by a user.
pub struct UserInfo {
    pub key: String,
    pub val: String,
}

impl UserInfo {
    /// Create a new instance of `UserInfo`.
    pub fn new<T: Into<String>, U: Into<String>>(key: T, val: U) -> Self {
        UserInfo {
            key: key.into(),
            val: val.into(),
        }
    }

    /// Create an instance of `UserInfo` to set a user's display name.
    pub fn name<T: Into<String>>(username: T) -> Result<Self, Error> {
        let name = username.into();
        // Determine the length of the given username in UTF-8 codepoints.
        let name_len = name.chars().count();
        // The name must be between 1 and 32 codepoints.
        if !(1..=32).contains(&name_len) {
            return CableErrorKind::UsernameLengthIncorrect {
                name,
                len: name_len,
            }
            .raise();
        }

        Ok(UserInfo::new("name", name))
    }
}

/// Print debug representation of user info.
impl fmt::Debug for UserInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "key: {:?}, val: {:?}", &self.key, &self.val)
    }
}

/// Print user info.
impl fmt::Display for UserInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "key: {}, val: {}", &self.key, &self.val)
    }
}
