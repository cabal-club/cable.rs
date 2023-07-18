#![cfg_attr(feature = "nightly-features", feature(async_closure, drain_filter))]
#![doc=include_str!("../README.md")]

mod manager;
mod store;
mod stream;
