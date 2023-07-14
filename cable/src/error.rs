//! Custom error type with backtrace.

#[cfg(feature = "nightly-features")]
use std::backtrace::Backtrace;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, PartialEq)]
pub struct CableError {
    kind: CableErrorKind,
    #[cfg(feature = "nightly-features")]
    backtrace: Backtrace,
}

#[derive(Debug, PartialEq)]
pub enum CableErrorKind {
    DstTooSmall { provided: usize, required: usize },
    MessageEmpty {},
    MessageWriteUnrecognizedType { msg_type: u64 },
    MessageHashResponseEnd {},
    MessageDataResponseEnd {},
    MessageHashRequestEnd {},
    MessageCancelRequestEnd {},
    MessageChannelTimeRangeRequestEnd {},
    MessageChannelStateRequestEnd {},
    MessageChannelListRequestEnd {},
    NoneError { context: String },
    PostWriteUnrecognizedType { post_type: u64 },
    PostHashingFailed {},
    UsernameLengthIncorrect { name: String, len: usize },
    ChannelLengthIncorrect { channel: String, len: usize },
    TopicLengthIncorrect { topic: String, len: usize },
}

impl CableErrorKind {
    pub fn raise<T>(self) -> Result<T, Error> {
        Err(Box::new(CableError {
            kind: self,
            #[cfg(feature = "nightly-features")]
            backtrace: Backtrace::capture(),
        }))
    }
}

impl std::error::Error for CableError {
    #[cfg(feature = "nightly-features")]
    fn backtrace<'a>(&'a self) -> Option<&'a Backtrace> {
        Some(&self.backtrace)
    }
}

impl std::fmt::Display for CableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            CableErrorKind::MessageEmpty {} => {
                write![f, "empty message"]
            }
            CableErrorKind::MessageWriteUnrecognizedType { msg_type } => {
                write![f, "cannot write unrecognized msg_type={}", msg_type]
            }
            CableErrorKind::DstTooSmall { provided, required } => {
                write![
                    f,
                    "destination buffer too small; {} bytes required, {} provided",
                    required, provided
                ]
            }
            CableErrorKind::MessageHashResponseEnd {} => {
                write![f, "unexpected end of HashResponse"]
            }
            CableErrorKind::MessageDataResponseEnd {} => {
                write![f, "unexpected end of DataResponse"]
            }
            CableErrorKind::MessageHashRequestEnd {} => {
                write![f, "unexpected end of HashRequest"]
            }
            CableErrorKind::MessageCancelRequestEnd {} => {
                write![f, "unexpected end of CancelRequest"]
            }
            CableErrorKind::MessageChannelTimeRangeRequestEnd {} => {
                write![f, "unexpected end of ChannelTimeRangeRequest"]
            }
            CableErrorKind::MessageChannelStateRequestEnd {} => {
                write![f, "unexpected end of ChannelStateRequest"]
            }
            CableErrorKind::MessageChannelListRequestEnd {} => {
                write![f, "unexpected end of ChannelListRequest"]
            }
            CableErrorKind::NoneError { context } => {
                write![f, "expected data but got none: {}", context]
            }
            CableErrorKind::PostHashingFailed {} => {
                write![f, "failed to compute hash for post"]
            }
            CableErrorKind::PostWriteUnrecognizedType { post_type } => {
                write![f, "cannot write unrecognized post_type={}", post_type]
            }
            CableErrorKind::UsernameLengthIncorrect { name, len } => {
                write![
                    f,
                    "expected username between 1 and 32 codepoints; name `{}` is {} codepoints",
                    name, len
                ]
            }
            CableErrorKind::ChannelLengthIncorrect { channel, len } => {
                write![
                    f,
                    "expected channel between 1 and 64 codepoints; channel `{}` is {} codepoints",
                    channel, len
                ]
            }
            CableErrorKind::TopicLengthIncorrect { topic, len } => {
                write![
                    f,
                    "expected topic between 0 and 512 codepoints; topic `{}` is {} codepoints",
                    topic, len
                ]
            }
        }
    }
}
