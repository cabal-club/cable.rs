# cable_core

Cable manager, store and stream implementations to facitilate creation of cable peers.

This library should be relied upon when implementing a `cable.rs` chat application.

**Status**: alpha (under active construction; expect changes).

This library does not handle encoding and decoding cable binary payloads. See the [cable](../cable) crate for those features.

## Usage

```rust,ignore
use async_std::task;

use cable::ChannelOptions;
use cable_core::{CableManager, MemoryStore};

let store = MemoryStore::default();
let cable = CableManager::new(store);

// Obtain a stream implementing the `AsyncRead` and `AsyncWrite` traits.
// For example, via a TCP listener.
let stream = ...;

// Start the cable stream listener.
//
// The listener is responsible for receiving inbound messages from peers,
// passing them to the relevant handlers and sending outbound messages in the
// form of requests or responses.
task::spawn(async move {
    if let Err(err) = cable.listen(stream).await {
        eprintln!("Cable listener error: {err}");
    }
});

// Define the channel options for a channel time range request and channel
// state request.
//
// These parameters request a maximum of 50 posts from the "default" channel,
// with a start time of `now`. The end time of 0 means that this is a live
// request (the peer will keep the request alive and send additional post
// hashes as they become known).
let opts = ChannelOptions::new("default", now(), 0, 50);

// Open a channel using the given channel options as parameters.
//
// Posts matching the given parameters will be streamed as they become
// known and available. This method can be used to update a user-interface
// with channel posts.
task::spawn(async move {
    if let Ok(mut post_stream) = client.open_channel(&opts).await {
        while let Some(Ok(post)) = post_stream.next().await {
            println!("{post}");
        }
    }
});
```

See [examples/chat.rs](examples/chat.rs) for a basic two-peer chat over TCP. A more comprehensive client implementation can be found in the [cabin](https://github.com/cabal-club/cabin) repository.

Additional examples of request-response patterns can be found in the integration [tests](tests/) directory.

## Documentation

Compile the documentation and open it in a browser:

`cargo doc --open`

Additional documentation can be found as code comments in the source.

## Tests

Run the test suite:

`cargo test`
