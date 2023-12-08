#[macro_use]
mod utils;

use std::fmt::{self, Display};

use desert::{FromBytes, ToBytes};
use log::warn;
use snow::{
    Builder as NoiseBuilder, HandshakeState as NoiseHandshakeState,
    TransportState as NoiseTransportState,
};

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, PartialEq)]
pub enum HandshakeError {
    /// The received major server version does not match that of the client.
    // TODO: Add `received` and `expected` context.
    IncompatibleServerVersion,
}

impl std::error::Error for HandshakeError {}

impl Display for HandshakeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HandshakeError::IncompatibleServerVersion => {
                write!(
                    f,
                    "Received major server version does not match major client version"
                )
            }
        }
    }
}

/// Size of the version message.
pub const VERSION_BYTES_LEN: usize = 2;

/// Number of bytes that will be written to the `send_buf` and `recv_buf`
/// during the version exchange.
pub const fn version_bytes_len() -> usize {
    VERSION_BYTES_LEN
}

/// The initialization data of a handshake that exists in every state of the
/// handshake.
#[derive(Debug, PartialEq)]
pub struct HandshakeBase {
    version: Version,
    psk: [u8; 32],
    private_key: [u8; 64],
    // TODO: How about we add a shared buffer here?
}

/// The `Handshake` type maintains the different states that happen in each
/// step of the handshake, allowing it to advance to completion.
///
/// The `Handshake` follows the [typestate pattern](http://cliffle.com/blog/rust-typestate/).
#[derive(Debug, PartialEq)]
pub struct Handshake<S: State> {
    pub base: HandshakeBase,
    pub state: S,
}

// Client states. The client acts as the handshake initiator.

/// The client state that can send the version.
#[derive(Debug)]
pub struct ClientSendVersion;

/// The client state that can receive the version.
#[derive(Debug)]
pub struct ClientRecvVersion;

/// The client state that can build the Noise handshake state machine.
#[derive(Debug)]
pub struct ClientBuildNoiseStateMachine;

/// The client state that can send the ephemeral key.
#[derive(Debug)]
pub struct ClientSendEphemeralKey(NoiseHandshakeState);

/// The client state that can receive the ephemeral and static keys.
#[derive(Debug)]
pub struct ClientRecvEphemeralAndStaticKeys(NoiseHandshakeState);

/// The client state that can send the static key.
#[derive(Debug)]
pub struct ClientSendStaticKey(NoiseHandshakeState);

/// The client state that can initialise transport mode.
#[derive(Debug)]
pub struct ClientInitTransportMode(NoiseHandshakeState);

/// The client state that has completed the handshake.
#[derive(Debug)]
pub struct ClientHandshakeComplete(NoiseTransportState);

// Server states. The server acts as the handshake responder.

/// The server state that can receive the version.
#[derive(Debug)]
pub struct ServerRecvVersion;

/// The server state that can receive the version.
#[derive(Debug)]
pub struct ServerSendVersion;

/// The server state that can build the Noise handshake state machine.
#[derive(Debug)]
pub struct ServerBuildNoiseStateMachine;

/// The server state that can receive the ephemeral key.
#[derive(Debug)]
pub struct ServerRecvEphemeralKey(NoiseHandshakeState);

/// The server state that can send the ephemeral and static keys.
#[derive(Debug)]
pub struct ServerSendEphemeralAndStaticKeys(NoiseHandshakeState);

/// The server state that can receive the static key.
#[derive(Debug)]
pub struct ServerRecvStaticKey(NoiseHandshakeState);

/// The server state that can initialise transport mode.
#[derive(Debug)]
pub struct ServerInitTransportMode(NoiseHandshakeState);

/// The server state that has completed the handshake.
#[derive(Debug)]
pub struct ServerHandshakeComplete(NoiseTransportState);

/// The `State` trait is used to implement the typestate pattern for the
/// `Handshake`.
///
/// The state machine is as follows:
///
/// Client:
///
/// - [`ClientSendVersion`] - `send_client_version()` -> [`ClientRecvVersion`]
/// - [`ClientRecvVersion`] - `recv_server_version()` -> [`ClientBuildNoiseStateMachine`]
/// - [`ClientBuildNoiseStateMachine`] - `build_client_noise_state_machine()` -> [`ClientSendEphemeralKey`]
/// - [`ClientSendEphemeralKey`] - `send_client_ephemeral_key()` -> [`ClientRecvEphemeralAndStaticKeys`]
/// - [`ClientRecvEphemeralAndStaticKeys`] - `recv_server_ephemeral_and_static_keys()` -> [`ClientSendStaticKey`]
/// - [`ClientSendStaticKey`] - `send_client_static_key()` -> [`ClientInitTransportMode`]
/// - [`ClientInitTransportMode`] - `init_client_transport_mode()` -> [`ClientHandshakeComplete`]
///
/// Server:
///
/// - [`ServerRecvVersion`] - `recv_client_version()` -> [`ServerSendVersion`]
/// - [`ServerSendVersion`] - `send_server_version()` -> [`ServerBuildNoiseStateMachine`]
/// - [`ServerBuildNoiseStateMachine`] - `build_server_noise_state_machine()` -> [`ServerRecvEphemeralKey`]
/// - [`ServerRecvEphemeralKey`] - `recv_client_ephemeral_key()` -> [`ServerSendEphemeralAndStaticKeys`]
/// - [`ServerSendEphemeralAndStaticKeys`] - `send_server_ephemeral_and_static_keys()` -> [`ServerRecvStaticKey`]
/// - [`ServerRecvStaticKey`] - `recv_client_static_key()` -> [`ServerInitTransportMode`]
/// - [`ServerInitTransportMode`] - `init_server_transport_mode()` -> [`ServerHandshakeComplete`]
pub trait State {}

impl State for ClientSendVersion {}
impl State for ClientRecvVersion {}
impl State for ClientBuildNoiseStateMachine {}
impl State for ClientSendEphemeralKey {}
impl State for ClientRecvEphemeralAndStaticKeys {}
impl State for ClientSendStaticKey {}
impl State for ClientInitTransportMode {}
impl State for ClientHandshakeComplete {}

impl State for ServerRecvVersion {}
impl State for ServerSendVersion {}
impl State for ServerBuildNoiseStateMachine {}
impl State for ServerRecvEphemeralKey {}
impl State for ServerSendEphemeralAndStaticKeys {}
impl State for ServerRecvStaticKey {}
impl State for ServerInitTransportMode {}
impl State for ServerHandshakeComplete {}

#[derive(Debug)]
enum Role {
    Initiator,
    Responder,
}

/// Initialise the Noise handshake state machine according to the given role,
/// ie. initiator or responder.
fn build_noise_state_machine(
    role: Role,
    psk: [u8; 32],
    private_key: [u8; 64],
) -> Result<NoiseHandshakeState> {
    let handshake_state = match role {
        Role::Initiator => NoiseBuilder::new("Noise_XXpsk0_25519_ChaChaPoly_BLAKE2b".parse()?)
            .local_private_key(&private_key)
            .prologue("CABLE".as_bytes())
            .psk(0, &psk)
            .build_initiator()?,
        Role::Responder => NoiseBuilder::new("Noise_XXpsk0_25519_ChaChaPoly_BLAKE2b".parse()?)
            .local_private_key(&private_key)
            .prologue("CABLE".as_bytes())
            .psk(0, &psk)
            .build_responder()?,
    };

    Ok(handshake_state)
}

// Client state implementations.

impl Handshake<ClientSendVersion> {
    /// Create a new handshake client that can send the version data.
    pub fn new_client(
        version: Version,
        psk: [u8; 32],
        private_key: [u8; 64],
    ) -> Handshake<ClientSendVersion> {
        let base = HandshakeBase {
            version,
            psk,
            private_key,
        };
        let state = ClientSendVersion;

        Handshake { base, state }
    }

    /// Send client version data to the server and advance to the next client
    /// state.
    pub fn send_client_version(self, send_buf: &mut [u8]) -> Result<Handshake<ClientRecvVersion>> {
        concat_into!(send_buf, &self.base.version.to_bytes()?);
        let state = ClientRecvVersion;
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ClientRecvVersion> {
    /// Receive the version data from the server and validate it before
    /// advancing to the next client state.
    ///
    /// Terminate the handshake with an error if the major version of the
    /// responder differs from that of the initiator.
    pub fn recv_server_version(
        self,
        recv_buf: &mut [u8],
    ) -> Result<Handshake<ClientBuildNoiseStateMachine>> {
        let (_n, server_version) = Version::from_bytes(recv_buf)?;
        if server_version.major != self.base.version.major {
            warn!("Received incompatible major version from handshake responder");
            return Err(HandshakeError::IncompatibleServerVersion.into());
        }
        let state = ClientBuildNoiseStateMachine;
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ClientBuildNoiseStateMachine> {
    fn build_client_noise_state_machine(self) -> Result<Handshake<ClientSendEphemeralKey>> {
        let noise_state_machine =
            build_noise_state_machine(Role::Initiator, self.base.psk, self.base.private_key)?;

        let state = ClientSendEphemeralKey(noise_state_machine);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ClientSendEphemeralKey> {
    fn send_client_ephemeral_key(mut self) -> Result<Handshake<ClientRecvEphemeralAndStaticKeys>> {
        // TODO: Figure out the optimal size for the Noise handshake buffer.
        let mut write_buf = [0u8; 1024];

        // Send the client ephemeral key to the server.
        self.state.0.write_message(&[], &mut write_buf)?;

        let state = ClientRecvEphemeralAndStaticKeys(self.state.0);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ClientRecvEphemeralAndStaticKeys> {
    fn recv_server_ephemeral_and_static_keys(mut self) -> Result<Handshake<ClientSendStaticKey>> {
        let mut read_buf = [0u8; 1024];

        // Receive the ephemeral and static keys from the server.
        self.state.0.read_message(&[], &mut read_buf)?;

        let state = ClientSendStaticKey(self.state.0);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ClientSendStaticKey> {
    fn send_client_static_key(mut self) -> Result<Handshake<ClientInitTransportMode>> {
        let mut write_buf = [0u8; 1024];

        // Send the client static key to the server.
        self.state.0.write_message(&[], &mut write_buf)?;

        let state = ClientInitTransportMode(self.state.0);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ClientInitTransportMode> {
    fn init_client_transport_mode(self) -> Result<Handshake<ClientHandshakeComplete>> {
        let transport_state = self.state.0.into_transport_mode()?;

        let state = ClientHandshakeComplete(transport_state);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

// Server state implementations.

impl Handshake<ServerRecvVersion> {
    /// Create a new handshake server that can receive the version data.
    pub fn new_server(
        version: Version,
        psk: [u8; 32],
        private_key: [u8; 64],
    ) -> Handshake<ServerRecvVersion> {
        let base = HandshakeBase {
            version,
            psk,
            private_key,
        };
        let state = ServerRecvVersion;

        Handshake { base, state }
    }

    /// Receive the version data from the client and validate it before
    /// advancing to the next client state.
    pub fn recv_client_version(self, recv_buf: &mut [u8]) -> Result<Handshake<ServerSendVersion>> {
        let (_n, client_version) = Version::from_bytes(recv_buf)?;
        if client_version.major != self.base.version.major {
            warn!("Received incompatible major version from handshake initiator");
            // There is no error returned here because the server must still
            // respond with it's own version data. The client will then error
            // and terminate the handshake.
        }
        let state = ServerSendVersion;
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ServerSendVersion> {
    /// Send server version data to the client and advance to the next server
    /// state.
    pub fn send_server_version(
        self,
        send_buf: &mut [u8],
    ) -> Result<Handshake<ServerBuildNoiseStateMachine>> {
        concat_into!(send_buf, &self.base.version.to_bytes()?);
        let state = ServerBuildNoiseStateMachine;
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ServerBuildNoiseStateMachine> {
    fn build_server_noise_state_machine(self) -> Result<Handshake<ServerRecvEphemeralKey>> {
        let noise_state_machine =
            build_noise_state_machine(Role::Responder, self.base.psk, self.base.private_key)?;

        let state = ServerRecvEphemeralKey(noise_state_machine);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ServerRecvEphemeralKey> {
    fn recv_client_ephemeral_key(mut self) -> Result<Handshake<ServerSendEphemeralAndStaticKeys>> {
        let mut read_buf = [0u8; 1024];

        // Receive the ephemeral key from the client.
        self.state.0.read_message(&[], &mut read_buf)?;

        let state = ServerSendEphemeralAndStaticKeys(self.state.0);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ServerRecvStaticKey> {
    fn recv_client_static_key(mut self) -> Result<Handshake<ServerInitTransportMode>> {
        let mut read_buf = [0u8; 1024];

        // Receive the static key to the client.
        self.state.0.read_message(&[], &mut read_buf)?;

        let state = ServerInitTransportMode(self.state.0);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ServerInitTransportMode> {
    fn init_server_transport_mode(self) -> Result<Handshake<ServerHandshakeComplete>> {
        let transport_state = self.state.0.into_transport_mode()?;

        let state = ServerHandshakeComplete(transport_state);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

#[derive(Debug, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_to_bytes() -> Result<()> {
        let version = Version { major: 0, minor: 1 };

        let version_to_bytes = version.to_bytes()?;
        let version_from_bytes = Version::from_bytes(&version_to_bytes)?;

        assert_eq!(version, version_from_bytes.1);

        Ok(())
    }

    #[test]
    fn version_exchange_success() -> Result<()> {
        let hs_client = Handshake::new_client(Version::init(1, 0));
        let hs_server = Handshake::new_server(Version::init(1, 0));

        let mut buf = [0; 8];

        let mut client_buf = &mut buf[..version_bytes_len()];
        let hs_client = hs_client.send_client_version(&mut client_buf)?;

        let mut server_buf = &mut buf[..version_bytes_len()];
        let hs_server = hs_server.recv_client_version(&mut server_buf)?;

        let mut server_buf = &mut buf[..version_bytes_len()];
        let hs_server = hs_server.send_server_version(&mut server_buf)?;

        let mut client_buf = &mut buf[..version_bytes_len()];
        let hs_client = hs_client.recv_server_version(&mut client_buf)?;

        Ok(())
    }

    #[test]
    fn version_exchange_failure() -> Result<()> {
        let hs_client = Handshake::new_client(Version::init(1, 0));
        let hs_server = Handshake::new_server(Version::init(3, 7));

        let mut buf = [0; 8];

        let mut client_buf = &mut buf[..version_bytes_len()];
        let hs_client = hs_client.send_client_version(&mut client_buf)?;

        let mut server_buf = &mut buf[..version_bytes_len()];
        let hs_server = hs_server.recv_client_version(&mut server_buf)?;

        let mut server_buf = &mut buf[..version_bytes_len()];
        let hs_server = hs_server.send_server_version(&mut server_buf)?;

        let mut client_buf = &mut buf[..version_bytes_len()];
        let hs_client = hs_client.recv_server_version(&mut client_buf)?;

        assert_eq!(hs_client, HandshakeError::IncompatibleServerVersion);

        Ok(())
    }

    /*
    TODO: Rather test with TCP connection in integration tests.

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
