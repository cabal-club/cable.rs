# cable.rs

Experimental [cable](https://github.com/cabal-club/cable) protocol implementation in Rust.

**Status**: alpha (under active construction; expect changes).

## Introduction

The `cable.rs` implementation is organised as a [workspace](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html) and includes all of the code required to successfully create cable peers and perform peer-to-peer communication. The workspace is divided into the following four crates:

- [cable](cable/) : Cable binary payload encoding and decoding (plus post and message types)
- [cable_core](cable_core/) : Manager, in-memory store and stream implementations for creating cable peers
- [desert](desert/) : Serialization and deserialization traits (vendored version; authored by substack)
- [length_prefixed_stream](length_prefixed_stream/) : Decoder to convert a byte stream of varint length-encoded messages into a stream of chunks (vendored version; authored by substack)

There is currently a single `cable.rs` chat client in the form of [cabin](https://github.com/cabal-club/cabin); a text-user interface (TUI) written in Rust.

## Limitations

The peer-to-peer networking layer of cable has not yet been designed nor implemented. An approach using simple TCP connections between peers is illustrated in the [cabin](https://github.com/cabal-club/cabin) TUI client.

`cable.rs` currently lacks a persistent data storage solution with keypair management; only an in-memory data store and indexes are available at present.

## Getting Started

If you are a developer interested in building or maintaining a chat application using `cable.rs`, the [cable_core](cable_core/) library is your best starting point. It provides the higher-level methods for listening and responding to cable messages, database stores and indexes, as well as convenient methods for opening and closing channels.

If you are a developer interested in contributing to `cable.rs`, the [cable protocol specification](https://github.com/cabal-club/cable) and the developer / contributor guide below should provide a good starting point.

## Developer / Contributor Guide

Wherever possible, idiomatic Rust conventions have been followed regarding code formatting and style. Doc and code comments can be found throughout the codebase and will guide you in any contribution efforts. In addition, there are examples and tests to read and learn from. With all that being said, there is still much room for improvement and contributions are welcome.

Before beginning work on any contributions, please open an issue introducing what you wish to work on. A project maintainer will respond and help to ensure that the intended contribution is a good fit for the project and that you are supported in your efforts. The issue can later be referenced in any subsequent pull-requests.

When it comes to code styling, it's recommended to refer to the codebase and follow the established stylistic conventions. This is not a hard requirement but a consistent codebase helps to facilitate clarity and ease of understanding. When in doubt, open an issue to ask for guidance.

There are many code comments throughout the codebase labelled with `TODO`; these may provide some inspiration for initial contributions.

## Documentation

Compile the documentation and open it in a browser:

`cargo doc --open`

Additional documentation can be found as code comments in the source.

## Tests

Run the test suite:

`cargo test`

## Contact

glyph (glyph@mycelial.technology).
