use std::fmt::Display;

use desert::{CountBytes, FromBytes, ToBytes};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

struct Config {
    role: Role,
    version: Version,
}

enum Role {
    Initiator,
    Responder,
}

#[derive(Debug, PartialEq)]
/// Major and minor identifiers for a particular version of the Cable Handshake
/// protocol.
struct Version {
    major: u8,
    minor: u8,
}

impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl ToBytes for Version {
    /// Convert a `Version` data type to bytes.
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0; 2];
        self.write_bytes(&mut buf)?;

        Ok(buf)
    }

    /// Write bytes to the given buffer (mutable byte array).
    fn write_bytes(&self, buf: &mut [u8]) -> Result<usize, Error> {
        buf[0..1].copy_from_slice(&self.major.to_be_bytes());
        buf[1..2].copy_from_slice(&self.minor.to_be_bytes());

        Ok(2)
    }
}

impl FromBytes for Version {
    /// Read bytes from the given buffer (byte array), returning the total
    /// number of bytes and the decoded `Version` type.
    fn from_bytes(buf: &[u8]) -> Result<(usize, Self), Error> {
        let major = buf[0];
        let minor = buf[1];

        let version = Version { major, minor };

        Ok((2, version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::prelude::*;
    use std::net::TcpStream;
    use std::thread;

    #[test]
    fn version_to_bytes() -> Result<(), Error> {
        let version = Version { major: 0, minor: 1 };

        let version_to_bytes = version.to_bytes()?;
        let version_from_bytes = Version::from_bytes(&version_to_bytes)?;

        assert_eq!(version, version_from_bytes.1);

        Ok(())
    }

    /*
    fn _setup_tcp_connection() {}

    #[test]
    fn version_exchange_works() -> std::io::Result<()> {
        // Deploy a TCP listener.
        //
        // Assigning port to 0 means that the OS selects an available port for us.
        let listener = TcpListener::bind("127.0.0.1:0")?;

        // Retrieve the address of the TCP listener to be able to connect later on.
        let addr = listener.local_addr()?;

        thread::spawn(move || {
            // Accept connections and process them serially.
            for stream in listener.incoming() {
                stream.read(&mut [0; 8])?;
            }
        });

        let mut stream = TcpStream::connect(addr)?;

        stream.write(&[1])?;
    }
    */
}
