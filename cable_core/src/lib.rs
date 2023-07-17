#![cfg_attr(feature = "nightly-features", feature(async_closure, drain_filter))]
#![doc=include_str!("../README.md")]

mod manager;
mod store;
mod stream;

use cable::{Channel, Timestamp};

#[derive(Clone, Debug, PartialEq)]
pub struct ChannelOptions {
    pub channel: Channel,
    pub time_start: Timestamp,
    pub time_end: Timestamp,
    pub limit: usize,
}
