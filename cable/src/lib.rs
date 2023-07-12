#![doc=include_str!("../README.md")]

pub mod error;
pub mod message;
pub mod post;

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
pub struct ChannelOptions {
    pub channel: Channel,
    pub time_start: Timestamp,
    pub time_end: Timestamp,
    pub limit: usize,
}
