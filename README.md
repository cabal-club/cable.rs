# cable.rs

Experimental [cable](https://github.com/cabal-club/cable) protocol implementation in Rust.

**Status**: alpha (under active construction; expect changes).

**Crates**

- [cable](cable/) : Cable binary payload encoding and decoding (plus post and message types)
- [cable_core](cable_core/) : Store and stream implementations for creating cable peers
- [desert](desert/) : Serialization and deserialization traits (vendored version; authored by substack)
- [length_prefixed_stream](length_prefixed_stream/) : Decoder to convert a byte stream of varint length-encoded messages into a stream of chunks (vendored version; authored by substack)
