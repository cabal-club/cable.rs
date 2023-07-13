#![doc=include_str!("../README.md")]

pub mod error;
pub mod message;
pub mod post;

use crate::error::{CableErrorKind, Error};

/// The name of a channel.
// TODO: Add a validation function to check length.
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
// TODO: Add a validation function to check length.
/// The topic of a channel.
pub type Topic = String;

#[derive(Clone, Debug, PartialEq)]
/// Query parameters defining a channel, time range and number of posts.
pub struct ChannelOptions {
    pub channel: Channel,
    pub time_start: Timestamp,
    pub time_end: Timestamp,
    pub limit: usize,
}

#[derive(Clone, Debug, PartialEq)]
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

#[cfg(test)]
mod test {
    use super::{Error, UserInfo};

    #[test]
    fn validate_username() -> Result<(), Error> {
        // Test valid usernames.
        let _valid_name = UserInfo::name("glyph")?;
        let _valid_name_japanese = UserInfo::name("五十嵐大介")?;

        // Test invalid usernames.

        // Name too short.
        match UserInfo::name("") {
            Err(e) => assert_eq!(
                e.to_string(),
                "expected username between 1 and 32 codepoints; name `` is 0 codepoints"
            ),
            _ => panic!(),
        }

        // Name too long.
        match UserInfo::name("Kimmeridgebrachypteraeschnidium etchesi") {
            Err(e) => assert_eq!(
                e.to_string(),
                "expected username between 1 and 32 codepoints; name `Kimmeridgebrachypteraeschnidium etchesi` is 39 codepoints"
            ),
            _ => panic!(),
        }

        Ok(())
    }
}
