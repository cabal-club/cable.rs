# cable_core

Cable manager, store and stream implementations to facitilate creation of cable peers.

This library should be relied upon when implementing a `cable.rs` chat application.

**Status**: alpha (under active construction; expect changes).

This library does not handle encoding and decoding cable binary payloads. See the [cable]("../cable") crate for those features.

## Example

See `cable_core/examples/chat.rs` for a basic two-peer chat over TCP.

## Documentation

**Compile the documentation and open it in a browser.**

`cargo doc --open`

Additional documentation can be found as code comments in the source.

## Tests

**Run the test suite.**

`cargo test`
