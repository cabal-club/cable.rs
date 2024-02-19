<!--
SPDX-FileCopyrightText: 2024 the cabal-club authors

SPDX-License-Identifier: CC0-1.0
-->

# Examples

## `tcp`

Run the server in one terminal:

`cargo run --example tcp -- -s`

Then run the client in a second terminal:

`cargo run --example tcp`

Expected server output:

```
TCP server listening on 127.0.0.1:9999
Accepted connection
Responding to handshake...
Received message: [65, 110, 32, 105, 109, 112, 101, 99, 99, 97, 98, 108, 121, 32, 112, 111, 108, 105, 116, 101, 32, 112, 97, 110, 103, 111, 108, 105, 110]
Received message: [7, 7, 7, 7, 7, 7, 7, 7, 7]
Received end-of-stream marker
Sent end-of-stream marker
```

Expected client output:

```
Connecting to TCP server on 127.0.0.1:9999
Connected
Initiating handshake...
Completed handshake with [230, 7, 43, 153, 18, 180, 176, 218, 120, 244, 105, 86, 137, 205, 199, 54, 197, 94, 155, 243, 215, 220, 152, 223, 37, 133, 128, 196, 151, 19, 48, 8]
Sent message
Sent message
Sent end-of-stream marker
Received end-of-stream marker
```

## `async_tcp`

Run the server in one terminal:

`cargo run --example async_tcp -- -s`

```
TCP server listening on 127.0.0.1:9999
Received message: [65, 110, 32, 105, 109, 112, 101, 99, 99, 97, 98, 108, 121, 32, 112, 111, 108, 105, 116, 101, 32, 112, 97, 110, 103, 111, 108, 105, 110]
Sent message
Received end-of-stream marker
Sent end-of-stream marker
```

Run the client in a second terminal:

`cargo run --example async_tcp`

```
Connecting to TCP server on 127.0.0.1:9999
Connected
Initiating handshake...
Sent message
Received message: [7, 7, 7, 7, 7, 7, 7, 7, 7]
Sent end-of-stream marker
Received end-of-stream marker
```

## `unix`

Run the server in one terminal:

`cargo run --example unix -- -s`

```
Unix socket listening on /tmp/handshake.sock
Accepted connection
Responding to handshake...
Received message: [65, 110, 32, 105, 109, 112, 101, 99, 99, 97, 98, 108, 121, 32, 112, 111, 108, 105, 116, 101, 32, 112, 97, 110, 103, 111, 108, 105, 110]
Received message: [7, 7, 7, 7, 7, 7, 7, 7, 7]
Sent end-of-stream marker
Received end-of-stream marker
```

Run the client in a second terminal:

`cargo run --example unix`

```
Connecting to Unix socket at /tmp/handshake.sock
Connected
Initiating handshake...
Sent message
Sent message
Received end-of-stream marker
Sent end-of-stream marker
```

## Rust-Node interop using `cli`

This example can be used to test the handshake and message sending / receiving
between the Rust and TypeScript implementations.

First clone and compile [cable-handshake.ts](https://github.com/cabal-club/cable-handshake.ts):

```
git@github.com:cabal-club/cable-handshake.ts.git
cd cable-handshake.ts
nvm use node
tsc
```

Make a named FIFO pipe and run the Node server in one terminal:

```
mkfifo hand
cat hand | node examples/cli.js '3131313131313131313131313131313131313131313131313131313131313131' 'responder' 'greetings from nodeville!' | nc -l 1234 > hand
```

```
f3450e4e: responder ready!
f3450e4e: got "hi from rustland!"
f3450e4e: wrote msg
f3450e4e: wrote eos
f3450e4e: got eos
```

Make a named FIFO pipe and run the Rust client in a second terminal:

```
mkfifo shake
cat shake | cargo run --example cli -- 11111111111111111111111111111111 initiator 'hi from rustland!' | nc 127.0.0.1 1234 > shake
```

```
Sent message
Received message: greetings from nodeville!
Received end-of-stream marker
Sent end-of-stream marker
Stream closed
```

The message can also be supplied by passing a filepath:

`cat shake | cargo run --example cli -- 11111111111111111111111111111111 initiator --file msg.txt | nc 127.0.0.1 1234 > shake`
