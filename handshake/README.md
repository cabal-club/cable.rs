<!--
SPDX-FileCopyrightText: 2024 the cabal-club authors

SPDX-License-Identifier: CC0-1.0
-->

# cable-handshake.rs

`cable_handshake` is an implementation of the [Cable Handshake Protocol](https://github.com/cabal-club/cable/blob/main/handshake.md) and uses the [`snow`](https://crates.io/crates/snow) implementation of the Noise Protocol Framework.

This implementation conforms to version `1.0-draft5` of the protocol specification and thus includes automatic message (de)fragmentation when payloads exceed 65519 bytes.

Support is included for asynchronous streams, though currently only for `async_std` (the provided stream must implement `futures_util::io::{AsyncRead, AsyncWrite}`). The underlying handshake itself is always synchronous.

## Example

See `handshake/examples` for TCP, Unix socket and async examples.

Perform a synchronous handshake as server (initiator):

```rust,ignore
use cable_handshake::{sync::handshake, Version};

let version = Version::init(1, 0);

// `psk` is the Cabal key (`[u8; 32]`).
// `private_key` refers to the Cabal author keypair (`Vec<u8>`).
let mut server = handshake::server(&mut stream, version, psk, private_key)?;

let bytes_written = server.write_message_to_stream(&mut stream, b"Aesthetic ichneumonids")?;

let msg = server.read_message_from_stream(&mut stream)?;
```

Perform a synchronous handshake as client (responder):

```rust,ignore
use cable_handshake::{sync::handshake, Version};

let version = Version::init(1, 7);

let mut client = handshake::client(&mut stream, version, psk, private_key)?;

let msg = client.read_message_from_stream(&mut stream)?;

let bytes_written = client.write_message_to_stream(&mut stream, b"Elegant elaterids")?;
```

Perform an asynchronous handshake as server (initiator):

```rust,ignore
use cable_handshake::{sync::handshake, Version};

let version = Version::init(3, 1);

let mut server = handshake::server(&mut stream, version, psk, private_key)?;

let bytes_written = server.write_message_to_async_stream(&mut stream, b"Quizzical curculionids")?;

let msg = server.read_message_from_async_stream(&mut stream)?;
```

## Documentation

Compile the documentation and open it in a browser:

`cargo doc --open`

Additional documentation can be found as code comments in the source.

## Tests

Run the test suite:

`cargo test`

## Benchmarks

Simple benchmarks exist for the basic synchronous handshake (over Unix socket) and a synchronous handshake plus 10,000 messages read and written by each peer (also over a Unix socket).

Run the benchmarks:

`cargo bench`

## License

LGPL-3.0.
