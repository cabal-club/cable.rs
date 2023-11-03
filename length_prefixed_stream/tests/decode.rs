use futures::{stream, StreamExt, TryStreamExt};

use length_prefixed_stream::decode;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

#[test]
fn simple_0() -> Result<(), Error> {
    smol::block_on(async {
        let input = stream::iter(vec![
            Ok(vec![6, 97, 98, 99]),
            Ok(vec![100, 101]),
            Ok(vec![102, 4, 65, 66]),
            Ok(vec![67, 68]),
        ])
        .into_async_read();

        let mut decoder = decode(input);
        let mut observed = vec![];
        while let Some(chunk) = decoder.next().await {
            observed.push(chunk?);
        }

        assert_eq![
            observed,
            vec![vec![97, 98, 99, 100, 101, 102], vec![65, 66, 67, 68],]
        ];

        Ok(())
    })
}

#[test]
fn simple_1() -> Result<(), Error> {
    smol::block_on(async {
        let input = stream::iter(vec![
            Ok(vec![3, 10, 20, 30, 5]),
            Ok(vec![11, 12, 13, 14, 15]),
            Ok(vec![1, 6, 3, 103]),
            Ok(vec![102, 101]),
        ])
        .into_async_read();

        let mut decoder = decode(input);
        let mut observed = vec![];
        while let Some(chunk) = decoder.next().await {
            observed.push(chunk?);
        }

        assert_eq![
            observed,
            vec![
                vec![10, 20, 30],
                vec![11, 12, 13, 14, 15],
                vec![6],
                vec![103, 102, 101],
            ]
        ];

        Ok(())
    })
}

#[test]
fn multibyte_msg_len() -> Result<(), Error> {
    smol::block_on(async {
        let input = stream::iter(vec![
            Ok(vec![4, 200, 201, 202, 203, 144]), // encode(400) = [144,3]
            Ok([vec![3], (0..200).collect()].concat()),
            Ok((200..395).map(|c| (c % 256) as u8).collect()),
            Ok([(395..400).map(|c| (c % 256) as u8).collect(), vec![5, 99]].concat()),
            Ok(vec![98, 97, 96]),
            Ok(vec![95, 4, 150, 150, 150, 150, 1]),
            Ok(vec![55]),
        ])
        .into_async_read();

        let mut decoder = decode(input);
        let mut observed = vec![];
        while let Some(chunk) = decoder.next().await {
            observed.push(chunk?);
        }

        assert_eq![
            observed,
            vec![
                vec![200, 201, 202, 203],
                (0..400).map(|c| (c % 256) as u8).collect(),
                vec![99, 98, 97, 96, 95],
                vec![150, 150, 150, 150],
                vec![55],
            ]
        ];

        Ok(())
    })
}
