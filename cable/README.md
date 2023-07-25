# cable

An experimental [cable](https://github.com/cabal-club/cable) protocol implementation in Rust.

**Status**: alpha (under active construction; expect changes).

This library provides a means of encoding and decoding binary payloads corresponding to all cable post and message types. Constructor methods are exposed for each type.

## Example

See `cable/examples/types.rs` for a complete set of examples.

**Encode and decode a post.**

```rust,ignore
use cable::post::Post;

// Create a new text post.
let mut text_post = Post::text(
    public_key,
    links,
    timestamp,
    channel,
    text,
);

// Sign the post.
text_post.sign(&secret_key)?;

// Encode the post to bytes.
let text_post_bytes = text_post.to_bytes()?;

// Decode the post from bytes.
let decoded_text_post = Post::from_bytes(&text_post_bytes)?;
```

**Encode and decode a message.**

```rust,ignore
use cable::message::Message;

// Create a new post request message.
let mut post_request = Message::post_request(circuit_id, req_id, ttl, post_hashes);

// Encode the message to bytes.
let post_request_bytes = post_request.to_bytes()?;

// Decode the message from bytes.
let decoded_post_request = Message::from_bytes(&post_request_bytes)?;
```

## Tests

**Run the test suite.**

`cargo test`
