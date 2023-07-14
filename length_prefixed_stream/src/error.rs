pub type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
use std::backtrace::Backtrace;

#[derive(Debug)]
pub struct DecodeError {
    kind: DecodeErrorKind,
    backtrace: Backtrace,
}

#[derive(Debug)]
pub enum DecodeErrorKind {
    Source { error: Error },
    UnexpectedEndVarint {},
    UnexpectedEndMessage {},
}

impl DecodeErrorKind {
    pub fn raise<T>(self) -> Result<T, DecodeError> {
        Err(DecodeError {
            kind: self,
            backtrace: Backtrace::capture(),
        })
    }
}

impl std::error::Error for DecodeError {
    /*
    fn source(&'_ self) -> Option<&'_ (dyn std::error::Error+'static)> {
      match self.kind {
        DecodeErrorKind::Source { error } => Some(error),
        _ => None,
      }
    }
    */
    fn backtrace(&'_ self) -> Option<&'_ Backtrace> {
        Some(&self.backtrace)
    }
}

impl From<Error> for DecodeError {
    fn from(error: Error) -> Self {
        DecodeError {
            kind: DecodeErrorKind::Source { error },
            backtrace: Backtrace::capture(),
        }
    }
}

impl From<std::io::Error> for DecodeError {
    fn from(error: std::io::Error) -> Self {
        DecodeError {
            kind: DecodeErrorKind::Source {
                error: Box::new(error),
            },
            backtrace: Backtrace::capture(),
        }
    }
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            DecodeErrorKind::Source { error } => {
                write![f, "{}", error]
            }
            DecodeErrorKind::UnexpectedEndVarint {} => {
                write![f, "unexpected end of input stream while decoding varint"]
            }
            DecodeErrorKind::UnexpectedEndMessage {} => {
                write![f, "unexpected end of input stream while decoding message"]
            }
        }
    }
}
