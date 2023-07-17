#![doc=include_str!("../README.md")]

pub mod constants;
pub mod error;
pub mod message;
pub mod post;

use crate::error::{CableErrorKind, Error};

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

/// Validate the length of a channel name (1 to 64 UTF-8 codepoints).
fn validate_channel(channel: &String) -> Result<(), Error> {
    // Determine the length of the given channel in UTF-8 codepoints.
    let channel_len = channel.chars().count();
    // The channel must be between 1 and 64 codepoints.
    if !(1..=64).contains(&channel_len) {
        return CableErrorKind::ChannelLengthIncorrect {
            channel: channel.to_owned(),
            len: channel_len,
        }
        .raise();
    }

    Ok(())
}

/// Validate the length of a topic name (1 to 512 UTF-8 codepoints).
fn validate_topic(topic: &String) -> Result<(), Error> {
    // Determine the length of the given channel topic in UTF-8 codepoints.
    let topic_len = topic.chars().count();
    // The topic must be between 0 and 512 codepoints.
    if topic_len > 521 {
        return CableErrorKind::TopicLengthIncorrect {
            topic: topic.to_owned(),
            len: topic_len,
        }
        .raise();
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::{validate_channel, validate_topic, Channel, Error, Topic, UserInfo};

    #[test]
    fn validate_username_len() -> Result<(), Error> {
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

    #[test]
    fn validate_channel_len() -> Result<(), Error> {
        // Test valid channels.
        let valid_channel: Channel = String::from("home");
        validate_channel(&valid_channel)?;
        let valid_channel_japanese: Channel = String::from("しろくまカフェ");
        validate_channel(&valid_channel_japanese)?;

        // Test invalid channels.

        let invalid_channel_short: Channel = String::from("");
        let invalid_channel_long: Channel = String::from("The Tao can't be perceived. Smaller than an electron, it contains uncountable galaxies.");

        // Channel too short.
        match validate_channel(&invalid_channel_short) {
            Err(e) => assert_eq!(
                e.to_string(),
                "expected channel between 1 and 64 codepoints; channel `` is 0 codepoints"
            ),
            _ => panic!(),
        }

        // Channel too long.
        match validate_channel(&invalid_channel_long) {
            Err(e) => assert_eq!(
                e.to_string(),
                "expected channel between 1 and 64 codepoints; channel `The Tao can't be perceived. Smaller than an electron, it contains uncountable galaxies.` is 87 codepoints"
            ),
            _ => panic!(),
        }

        Ok(())
    }

    #[test]
    fn validate_topic_len() -> Result<(), Error> {
        // Test valid topics.
        let valid_topic: Topic = String::from("");
        validate_topic(&valid_topic)?;
        let valid_topic_long: Topic = String::from("The great Tao flows everywhere. All things are born from it, yet it doesn't create them. It pours itself into its work, yet it makes no claim. It nourishes infinite worlds, yet it doesn't hold on to them. Since it is merged with all things and hidden in their hearts, it can be called humble.");
        validate_topic(&valid_topic_long)?;

        // Test invalid channels.

        let invalid_topic_long: Topic = String::from("Bees are winged insects closely related to wasps and ants, known for their roles in pollination and, in the case of the best-known bee species, the western honey bee, for producing honey. Bees are a monophyletic lineage within the superfamily Apoidea. They are presently considered a clade, called Anthophila. There are over 16,000 known species of bees in seven recognized biological families. Some species – including honey bees, bumblebees, and stingless bees – live socially in colonies while most species (>90%) – including mason bees, carpenter bees, leafcutter bees, and sweat bees – are solitary.");

        // Topic too long.
        match validate_topic(&invalid_topic_long) {
            Err(e) => assert_eq!(
                e.to_string(),
                "expected topic between 0 and 512 codepoints; topic `Bees are winged insects closely related to wasps and ants, known for their roles in pollination and, in the case of the best-known bee species, the western honey bee, for producing honey. Bees are a monophyletic lineage within the superfamily Apoidea. They are presently considered a clade, called Anthophila. There are over 16,000 known species of bees in seven recognized biological families. Some species – including honey bees, bumblebees, and stingless bees – live socially in colonies while most species (>90%) – including mason bees, carpenter bees, leafcutter bees, and sweat bees – are solitary.` is 604 codepoints"
                ),
            _ => panic!(),
        }

        Ok(())
    }
}
