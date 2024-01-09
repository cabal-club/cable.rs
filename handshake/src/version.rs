use std::fmt::Display;

use desert::{FromBytes, ToBytes};

use crate::Result;

#[derive(Clone, Debug, PartialEq)]
/// Major and minor identifiers for a particular version of the Cable Handshake
/// protocol.
pub struct Version {
    major: u8,
    minor: u8,
}

impl Version {
    /// Initialise a new version instance.
    pub fn init(major: u8, minor: u8) -> Self {
        Version { major, minor }
    }

    /// Return the major version identifier.
    pub fn major(&self) -> u8 {
        self.major
    }

    /// Return the minor version identifier.
    pub fn minor(&self) -> u8 {
        self.minor
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl ToBytes for Version {
    /// Convert a `Version` data type to bytes.
    fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = vec![0; 2];
        self.write_bytes(&mut buf)?;

        Ok(buf)
    }

    /// Write bytes to the given buffer (mutable byte array).
    fn write_bytes(&self, buf: &mut [u8]) -> Result<usize> {
        buf[0..1].copy_from_slice(&self.major.to_be_bytes());
        buf[1..2].copy_from_slice(&self.minor.to_be_bytes());

        Ok(2)
    }
}

impl FromBytes for Version {
    /// Read bytes from the given buffer (byte array), returning the total
    /// number of bytes and the decoded `Version` type.
    fn from_bytes(buf: &[u8]) -> Result<(usize, Self)> {
        let major = buf[0];
        let minor = buf[1];

        let version = Version { major, minor };

        Ok((2, version))
    }
}
