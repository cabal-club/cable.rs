use futures::stream::{self, StreamExt, TryStreamExt};

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
