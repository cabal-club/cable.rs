// Vendored version of length-prefixed-stream 1.0.0
//
// Minor changes have been made to fix compilation errors.
//
// The original source files from which this is derived is
// Copyright (c) 2021 James Halliday
//
// and released under the BSD-3-CLAUSE license:
// https://docs.rs/crate/length-prefixed-stream/1.0.0/source/LICENSE

#![allow(unused_assignments)]
#![doc=include_str!("../README.md")]

mod error;
mod unfold;

use std::{collections::VecDeque, marker::Unpin};

use async_std::{prelude::*, stream::Stream};
use desert::varint;
use futures::io::AsyncRead;

pub use error::{DecodeError, DecodeErrorKind};
use unfold::unfold;

pub fn decode(
    input: impl AsyncRead + Send + Sync + Unpin + 'static,
) -> Box<dyn Stream<Item = Result<Vec<u8>, DecodeError>> + Send + Sync + Unpin> {
    decode_with_options(input, DecodeOptions::default())
}

pub fn decode_with_options(
    input: impl AsyncRead + Send + Sync + Unpin + 'static,
    options: DecodeOptions,
) -> Box<dyn Stream<Item = Result<Vec<u8>, DecodeError>> + Send + Sync + Unpin> {
    let state = Decoder::new(input, options);
    Box::new(unfold(state, |mut state| async move {
        match state.next().await {
            Ok(Some(x)) => Some((Ok(x), state)),
            Ok(None) => None,
            Err(e) => Some((Err(e), state)),
        }
    }))
}

pub struct DecodeOptions {
    pub max_size: usize,
    pub include_len: bool,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            max_size: 50_000,
            include_len: false,
        }
    }
}

struct Decoder<AR: AsyncRead> {
    input: AR,
    buffer: Vec<u8>,
    queue: VecDeque<Vec<u8>>,
    write_offset: usize,
    options: DecodeOptions,
}

impl<AR> Decoder<AR>
where
    AR: AsyncRead + Unpin + 'static,
{
    pub fn new(input: AR, options: DecodeOptions) -> Self {
        Self {
            input,
            buffer: vec![0u8; options.max_size],
            write_offset: 0,
            queue: VecDeque::new(),
            options,
        }
    }
    pub async fn next(&mut self) -> Result<Option<Vec<u8>>, DecodeError> {
        if let Some(buf) = self.queue.pop_front() {
            return Ok(Some(buf));
        }
        let mut msg_len = 0;
        let mut read_offset = 0;
        loop {
            let n = self
                .input
                .read(&mut self.buffer[self.write_offset..])
                .await?;
            if n == 0 && self.write_offset == 0 {
                return Ok(None);
            } else if n == 0 {
                return DecodeErrorKind::UnexpectedEndVarint {}.raise();
            }
            self.write_offset += n;
            match varint::decode(&self.buffer) {
                Ok((s, len)) => {
                    msg_len = len as usize;
                    read_offset = s;
                    break;
                }
                Err(e) => {
                    if self.write_offset >= 10 {
                        return Err(e.into());
                    }
                }
            }
        }
        loop {
            if msg_len == 0 {
                break;
            }
            if msg_len + read_offset > self.write_offset {
                let n = self
                    .input
                    .read(&mut self.buffer[self.write_offset..])
                    .await?;
                if n == 0 {
                    return DecodeErrorKind::UnexpectedEndMessage {}.raise();
                }
                self.write_offset += n;
            } else {
                break;
            }
        }
        let buf = {
            if self.options.include_len {
                self.buffer[0..read_offset + msg_len].to_vec()
            } else {
                self.buffer[read_offset..read_offset + msg_len].to_vec()
            }
        };
        {
            let mut offset = read_offset + msg_len;
            let mut vlen = 0;
            loop {
                // push remaining complete records in this buffer to queue
                match varint::decode(&self.buffer[offset..]) {
                    Ok((s, len)) => {
                        msg_len = len as usize;
                        vlen = s;
                    }
                    _ => break,
                }
                if msg_len == 0 {
                    break;
                }
                if offset + vlen + msg_len > self.write_offset {
                    break;
                }
                offset += vlen;
                let qbuf = {
                    if self.options.include_len {
                        self.buffer[offset - vlen..offset + msg_len].to_vec()
                    } else {
                        self.buffer[offset..offset + msg_len].to_vec()
                    }
                };
                self.queue.push_back(qbuf);
                offset += msg_len;
            }
            self.buffer.copy_within(offset.., 0);
            self.write_offset -= offset;
        }
        Ok(Some(buf))
    }
}
