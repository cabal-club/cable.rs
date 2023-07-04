#[cfg(feature = "nightly-features")]
use std::backtrace::Backtrace;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug)]
pub struct DesertError {
    kind: DesertErrorKind,
    #[cfg(feature = "nightly-features")]
    backtrace: Backtrace,
}

#[derive(Debug)]
pub enum DesertErrorKind {
    DstInsufficient { required: usize, provided: usize },
    SrcInsufficient { required: usize, provided: usize },
    VarintSrcInsufficient {},
    VarintDstInsufficient {},
}

impl DesertErrorKind {
    pub fn raise<T>(self) -> Result<T, Error> {
        Err(Box::new(DesertError {
            kind: self,
            #[cfg(feature = "nightly-features")]
            backtrace: Backtrace::capture(),
        }))
    }
}

impl std::error::Error for DesertError {
    #[cfg(feature = "nightly-features")]
    fn backtrace<'a>(&'a self) -> Option<&'a Backtrace> {
        Some(&self.backtrace)
    }
}

impl std::fmt::Display for DesertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            DesertErrorKind::DstInsufficient { required, provided } => {
                write![
                    f,
                    "dst buffer too small. required {} bytes, provided {} bytes",
                    required, provided
                ]
            }
            DesertErrorKind::SrcInsufficient { required, provided } => {
                write![
                    f,
                    "src buffer too small. required {} bytes, provided {} bytes",
                    required, provided
                ]
            }
            DesertErrorKind::VarintSrcInsufficient {} => {
                write![f, "end of buffer reached while decoding varint"]
            }
            DesertErrorKind::VarintDstInsufficient {} => {
                write![f, "dst buffer too small to encode varint"]
            }
        }
    }
}
