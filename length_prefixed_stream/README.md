# length-prefixed-stream

Decode a byte stream of varint length-encoded messages into a stream of chunks.

This crate is async-runtime agnostic and is similar to and compatible with the
[javascript length-prefixed-stream](https://www.npmjs.com/package/length-prefixed-stream) package.

# Example

Note that we're using the [smol](https://crates.io/crates/smol) async runtime in this example.
One could just as easily use [tokio](https://crates.io/crates/tokio) or [async-std](https://crates.io/crates/async-std).

```rust
use futures::stream::{self, TryStreamExt, StreamExt};
use smol;

use length_prefixed_stream::decode;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

// This program will print:
//
// [97,98,99,100,101,102]
// [65,66,67,68]

fn main() -> Result<(), Error> {
    smol::block_on(async {
        let input = stream::iter(vec![
            Ok(vec![6, 97, 98, 99]),
            Ok(vec![100, 101]),
            Ok(vec![102, 4, 65, 66]),
            Ok(vec![67, 68]),
        ])
        .into_async_read();

        let mut decoder = decode(input);
        while let Some(chunk) = decoder.next().await {
            println!["{:?}", chunk?];
        }

        Ok(())
    })
}
```
