use async_std::{prelude::*, stream, task};
use futures::stream::TryStreamExt;
use length_prefixed_stream::{decode_with_options, DecodeOptions};
type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

#[test]
fn options_include_len() -> Result<(), Error> {
    task::block_on(async {
        let input = stream::from_iter(vec![
            Ok(vec![6, 97, 98, 99]),
            Ok(vec![100, 101]),
            Ok(vec![102, 4, 65, 66]),
            Ok(vec![67, 68]),
        ])
        .into_async_read();
        let mut options = DecodeOptions::default();
        options.include_len = true;
        let mut decoder = decode_with_options(input, options);
        let mut observed = vec![];
        while let Some(chunk) = decoder.next().await {
            observed.push(chunk?);
        }
        assert_eq![
            observed,
            vec![vec![6, 97, 98, 99, 100, 101, 102], vec![4, 65, 66, 67, 68],]
        ];
        Ok(())
    })
}
