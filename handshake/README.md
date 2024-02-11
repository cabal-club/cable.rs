<!--
SPDX-FileCopyrightText: 2024 the cabal-club authors

SPDX-License-Identifier: CC0-1.0
-->

# cable-handshake.rs

`cable_handshake` is an implementation of the [Cable Handshake Protocol](https://github.com/cabal-club/cable/blob/main/handshake.md) and uses the [`snow`](https://crates.io/crates/snow) implementation of the Noise Protocol Framework.

This implementation conforms to version `1.0-draft7` of the protocol specification and thus includes automatic message (de)fragmentation when payloads exceed 65519 bytes.

Support is included for asynchronous streams, though currently only for `async_std` (the provided stream must implement `futures_util::io::{AsyncRead, AsyncWrite}`). The underlying handshake itself is always synchronous.

Support is also included for writing end-of-stream markers, as defined by the specification. Receipt of an empty vector when reading from a stream indicates an end-of-stream marker.

## Example

See `handshake/examples` for TCP, Unix socket and async examples.

Perform a synchronous handshake as server (initiator):

```rust,ignore
use cable_handshake::{sync::handshake, Version};

let version = Version::init(1, 0);

// `psk` is the Cabal key (`[u8; 32]`).
// `private_key` refers to the Cabal author keypair (`Vec<u8>`).
let mut server = handshake::server(&mut stream, version, psk, private_key)?;

if let Some(client_public_key) = server.get_remote_public_key() {
    println!("Completed handshake with {:?}", client_public_key)
}

let bytes_written = server.write_message_to_stream(&mut stream, b"Aesthetic ichneumonids")?;

let msg = server.read_message_from_stream(&mut stream)?;
```

Perform a synchronous handshake as client (responder):

```rust,ignore
use cable_handshake::{sync::handshake, Version};

let version = Version::init(1, 7);

let mut client = handshake::client(&mut stream, version, psk, private_key)?;

if let Some(server_public_key) = client.get_remote_public_key() {
    println!("Completed handshake with {:?}", server_public_key)
}

let msg = client.read_message_from_stream(&mut stream)?;

let bytes_written = client.write_message_to_stream(&mut stream, b"Elegant elaterids")?;

client.write_eos_marker_to_stream(&mut stream)?;

if client.read_message_from_stream(stream)?.is_empty() {
    println!("Received end-of-stream marker");
}
```

Perform an asynchronous handshake as server (initiator):

```rust,ignore
use cable_handshake::{sync::handshake, Version};

let version = Version::init(3, 1);

let mut server = handshake::server(&mut stream, version, psk, private_key)?;

let bytes_written = server.write_message_to_async_stream(&mut stream, b"Quizzical curculionids")?;

let msg = server.read_message_from_async_stream(&mut stream)?;

if client.read_message_from_stream(stream)?.is_empty() {
    client.write_eos_marker_to_stream(stream)?;
}
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
