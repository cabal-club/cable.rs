# length-prefixed-stream

Decode a byte stream of varint length-encoded messages into a stream of chunks.

This crate is similar to and compatible with the
[javascript length-prefixed-stream](https://www.npmjs.com/package/length-prefixed-stream) package.

# example

```rust
use async_std::{prelude::*, stream, task};
use futures::stream::TryStreamExt;
use length_prefixed_stream::decode;
type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

// this program will print:
// [97,98,99,100,101,102]
// [65,66,67,68]

fn main() -> Result<(), Error> {
    task::block_on(async {
        let input = stream::from_iter(vec![
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
