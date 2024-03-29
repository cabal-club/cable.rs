#![doc=include_str!("../README.md")]

// SPDX-FileCopyrightText: 2024 the cabal-club authors
//
// SPDX-License-Identifier: LGPL-3.0-or-later

pub mod async_std;
mod constants;
mod error;
pub mod post_handshake;
pub mod sync;
#[macro_use]
mod utils;
mod version;

use desert::{FromBytes, ToBytes};
use log::warn;
use snow::{
    Builder as NoiseBuilder, HandshakeState as NoiseHandshakeState,
    TransportState as NoiseTransportState,
};

use constants::{
    EPHEMERAL_AND_STATIC_KEY_BYTES_LEN, EPHEMERAL_KEY_BYTES_LEN, PUBLIC_KEY_BYTES_LEN,
    STATIC_KEY_BYTES_LEN,
};

pub use crate::{error::HandshakeError, version::Version};

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

/// The initialization data of a handshake that exists in every state of the
/// handshake.
#[derive(Debug, PartialEq)]
pub struct HandshakeBase {
    /// The handshake protocol version.
    version: Version,
    /// The pre-shared key (aka. the "cabal key").
    psk: [u8; 32],
    // TODO: Could this rather be a sized array?
    //private_key: [u8; 64],
    /// The private key of the cabal keypair belonging to the handshaker.
    private_key: Vec<u8>,
    /// The public key of the remote peer with whom the handshake has
    /// been conducted.
    pub remote_public_key: Option<[u8; PUBLIC_KEY_BYTES_LEN]>,
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

/// The role taken by a peer during the handshake.
#[derive(Debug)]
enum Role {
    Initiator,
    Responder,
}

// Client states. The client acts as the handshake initiator.

/// The client state that can send the version.
#[derive(Debug)]
struct ClientSendVersion;

/// The client state that can receive the version.
#[derive(Debug)]
struct ClientRecvVersion;

/// The client state that can build the Noise handshake state machine.
#[derive(Debug)]
struct ClientBuildNoiseStateMachine;

/// The client state that can send the ephemeral key.
#[derive(Debug)]
struct ClientSendEphemeralKey(NoiseHandshakeState);

/// The client state that can receive the ephemeral and static keys.
#[derive(Debug)]
struct ClientRecvEphemeralAndStaticKey(NoiseHandshakeState);

/// The client state that can send the static key.
#[derive(Debug)]
struct ClientSendStaticKey(NoiseHandshakeState);

/// The client state that can initialise transport mode.
#[derive(Debug)]
struct ClientInitTransportMode(NoiseHandshakeState);

// Server states. The server acts as the handshake responder.

/// The server state that can receive the version.
#[derive(Debug)]
struct ServerRecvVersion;

/// The server state that can receive the version.
#[derive(Debug)]
struct ServerSendVersion;

/// The server state that can build the Noise handshake state machine.
#[derive(Debug)]
struct ServerBuildNoiseStateMachine;

/// The server state that can receive the ephemeral key.
#[derive(Debug)]
struct ServerRecvEphemeralKey(NoiseHandshakeState);

/// The server state that can send the ephemeral and static keys.
#[derive(Debug)]
struct ServerSendEphemeralAndStaticKey(NoiseHandshakeState);

/// The server state that can receive the static key.
#[derive(Debug)]
struct ServerRecvStaticKey(NoiseHandshakeState);

/// The server state that can initialise transport mode.
#[derive(Debug)]
struct ServerInitTransportMode(NoiseHandshakeState);

// Shared client / server states.

/// The client / server state that has completed the handshake.
#[derive(Debug)]
pub struct HandshakeComplete(NoiseTransportState);

// The `State` trait is used to implement the typestate pattern for the
// `Handshake`.
//
// The state machine is as follows:
//
// Client:
//
// - [`ClientSendVersion`] - `send_client_version()` -> [`ClientRecvVersion`]
// - [`ClientRecvVersion`] - `recv_server_version()` -> [`ClientBuildNoiseStateMachine`]
// - [`ClientBuildNoiseStateMachine`] - `build_client_noise_state_machine()` -> [`ClientSendEphemeralKey`]
// - [`ClientSendEphemeralKey`] - `send_client_ephemeral_key()` -> [`ClientRecvEphemeralAndStaticKey`]
// - [`ClientRecvEphemeralAndStaticKey`] - `recv_server_ephemeral_and_static_key()` -> [`ClientSendStaticKey`]
// - [`ClientSendStaticKey`] - `send_client_static_key()` -> [`ClientInitTransportMode`]
// - [`ClientInitTransportMode`] - `init_client_transport_mode()` -> [`HandshakeComplete`]
//
// Server:
//
// - [`ServerRecvVersion`] - `recv_client_version()` -> [`ServerSendVersion`]
// - [`ServerSendVersion`] - `send_server_version()` -> [`ServerBuildNoiseStateMachine`]
// - [`ServerBuildNoiseStateMachine`] - `build_server_noise_state_machine()` -> [`ServerRecvEphemeralKey`]
// - [`ServerRecvEphemeralKey`] - `recv_client_ephemeral_key()` -> [`ServerSendEphemeralAndStaticKey`]
// - [`ServerSendEphemeralAndStaticKey`] - `send_server_ephemeral_and_static_key()` -> [`ServerRecvStaticKey`]
// - [`ServerRecvStaticKey`] - `recv_client_static_key()` -> [`ServerInitTransportMode`]
// - [`ServerInitTransportMode`] - `init_server_transport_mode()` -> [`HandshakeComplete`]
pub trait State {}

impl State for ClientSendVersion {}
impl State for ClientRecvVersion {}
impl State for ClientBuildNoiseStateMachine {}
impl State for ClientSendEphemeralKey {}
impl State for ClientRecvEphemeralAndStaticKey {}
impl State for ClientSendStaticKey {}
impl State for ClientInitTransportMode {}

impl State for ServerRecvVersion {}
impl State for ServerSendVersion {}
impl State for ServerBuildNoiseStateMachine {}
impl State for ServerRecvEphemeralKey {}
impl State for ServerSendEphemeralAndStaticKey {}
impl State for ServerRecvStaticKey {}
impl State for ServerInitTransportMode {}

impl State for HandshakeComplete {}

/// Initialise the Noise handshake state machine according to the given role,
/// ie. initiator or responder.
fn build_noise_state_machine(
    role: Role,
    psk: [u8; 32],
    private_key: Vec<u8>,
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
    fn new_client(
        version: Version,
        psk: [u8; 32],
        private_key: Vec<u8>,
    ) -> Handshake<ClientSendVersion> {
        let base = HandshakeBase {
            version,
            psk,
            private_key,
            remote_public_key: None,
        };
        let state = ClientSendVersion;

        Handshake { base, state }
    }

    /// Send the client version data to the server and advance to the next
    /// client state.
    fn send_client_version(self, send_buf: &mut [u8]) -> Result<Handshake<ClientRecvVersion>> {
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
    fn recv_server_version(
        self,
        recv_buf: &mut [u8],
    ) -> Result<Handshake<ClientBuildNoiseStateMachine>> {
        let (_n, server_version) = Version::from_bytes(recv_buf)?;
        if server_version.major() != self.base.version.major() {
            warn!("Received incompatible major version from handshake responder");
            return Err(HandshakeError::IncompatibleServerVersion {
                received: server_version.major(),
                expected: self.base.version.major(),
            }
            .into());
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
    /// Build the Noise handshake state machine for the client with the PSK and
    /// private key.
    fn build_client_noise_state_machine(self) -> Result<Handshake<ClientSendEphemeralKey>> {
        let noise_state_machine = build_noise_state_machine(
            Role::Initiator,
            self.base.psk,
            // TODO: Can we avoid this clone?
            self.base.private_key.clone(),
        )?;

        let state = ClientSendEphemeralKey(noise_state_machine);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ClientSendEphemeralKey> {
    /// Send the client ephemeral key to the server and advance to the next client state.
    fn send_client_ephemeral_key(
        mut self,
        send_buf: &mut [u8],
    ) -> Result<Handshake<ClientRecvEphemeralAndStaticKey>> {
        let mut write_buf = [0; EPHEMERAL_KEY_BYTES_LEN];

        // Send the client ephemeral key to the server.
        let len = self.state.0.write_message(&[], &mut write_buf)?;

        concat_into!(send_buf, &write_buf[..len]);

        let state = ClientRecvEphemeralAndStaticKey(self.state.0);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ClientRecvEphemeralAndStaticKey> {
    /// Receive the ephemeral and static keys from the server and advance to
    /// the next client state.
    fn recv_server_ephemeral_and_static_key(
        mut self,
        recv_buf: &mut [u8],
    ) -> Result<Handshake<ClientSendStaticKey>> {
        let mut read_buf = [0u8; EPHEMERAL_AND_STATIC_KEY_BYTES_LEN];

        // Receive the ephemeral and static keys from the server.
        self.state.0.read_message(recv_buf, &mut read_buf)?;

        // Set the value of the server's public key.
        self.base.remote_public_key = match self.state.0.get_remote_static() {
            // Convert the key from a slice (`&[u8]`) to a sized array.
            Some(key) => Some(key.try_into()?),
            None => None,
        };

        let state = ClientSendStaticKey(self.state.0);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ClientSendStaticKey> {
    /// Send the client static key to the server and advance to the next client state.
    fn send_client_static_key(
        mut self,
        send_buf: &mut [u8],
    ) -> Result<Handshake<ClientInitTransportMode>> {
        let mut write_buf = [0u8; STATIC_KEY_BYTES_LEN];

        // Send the client static key to the server.
        let len = self.state.0.write_message(&[], &mut write_buf)?;

        concat_into!(send_buf, &write_buf[..len]);

        let state = ClientInitTransportMode(self.state.0);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ClientInitTransportMode> {
    /// Complete the client handshake by initialising the encrypted transport.
    fn init_client_transport_mode(self) -> Result<Handshake<HandshakeComplete>> {
        let transport_state = self.state.0.into_transport_mode()?;

        let state = HandshakeComplete(transport_state);
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
    fn new_server(
        version: Version,
        psk: [u8; 32],
        private_key: Vec<u8>,
    ) -> Handshake<ServerRecvVersion> {
        let base = HandshakeBase {
            version,
            psk,
            private_key,
            remote_public_key: None,
        };
        let state = ServerRecvVersion;

        Handshake { base, state }
    }

    /// Receive the version data from the client and validate it before
    /// advancing to the next client state.
    fn recv_client_version(self, recv_buf: &mut [u8]) -> Result<Handshake<ServerSendVersion>> {
        let (_n, client_version) = Version::from_bytes(recv_buf)?;
        if client_version.major() != self.base.version.major() {
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
    fn send_server_version(
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
    /// Build the Noise handshake state machine for the server with the PSK and
    /// private key.
    fn build_server_noise_state_machine(self) -> Result<Handshake<ServerRecvEphemeralKey>> {
        let noise_state_machine = build_noise_state_machine(
            Role::Responder,
            self.base.psk,
            self.base.private_key.clone(),
        )?;

        let state = ServerRecvEphemeralKey(noise_state_machine);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ServerRecvEphemeralKey> {
    /// Receive the ephemeral key from the client and advance to the next server state.
    fn recv_client_ephemeral_key(
        mut self,
        recv_buf: &mut [u8],
    ) -> Result<Handshake<ServerSendEphemeralAndStaticKey>> {
        let mut read_buf = [0u8; 1024];

        // Receive the ephemeral key from the client.
        self.state.0.read_message(recv_buf, &mut read_buf)?;

        let state = ServerSendEphemeralAndStaticKey(self.state.0);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ServerSendEphemeralAndStaticKey> {
    /// Send the ephemeral and static keys to the client and advance to
    /// the next server state.
    fn send_server_ephemeral_and_static_key(
        mut self,
        send_buf: &mut [u8],
    ) -> Result<Handshake<ServerRecvStaticKey>> {
        let mut write_buf = [0u8; 1024];

        // Send the ephemeral and static keys to the client.
        let len = self.state.0.write_message(&[], &mut write_buf)?;

        concat_into!(send_buf, &write_buf[..len]);

        let state = ServerRecvStaticKey(self.state.0);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ServerRecvStaticKey> {
    /// Receive the static key from the clientand advance to the next server
    /// state.
    fn recv_client_static_key(
        mut self,
        recv_buf: &mut [u8],
    ) -> Result<Handshake<ServerInitTransportMode>> {
        let mut read_buf = [0u8; 1024];

        // Receive the static key to the client.
        self.state.0.read_message(recv_buf, &mut read_buf)?;

        // Set the value of the client's static key.
        self.base.remote_public_key = match self.state.0.get_remote_static() {
            // Convert the key from a slice (`&[u8]`) to a sized array.
            Some(key) => Some(key.try_into()?),
            None => None,
        };

        let state = ServerInitTransportMode(self.state.0);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

impl Handshake<ServerInitTransportMode> {
    /// Complete the server handshake by initialising the encrypted transport.
    fn init_server_transport_mode(self) -> Result<Handshake<HandshakeComplete>> {
        let transport_state = self.state.0.into_transport_mode()?;

        let state = HandshakeComplete(transport_state);
        let handshake = Handshake {
            base: self.base,
            state,
        };

        Ok(handshake)
    }
}

#[cfg(test)]
mod tests {
    use constants::{
        EPHEMERAL_AND_STATIC_KEY_BYTES_LEN, EPHEMERAL_KEY_BYTES_LEN, STATIC_KEY_BYTES_LEN,
        VERSION_BYTES_LEN,
    };

    use super::*;

    fn init_handshakers(
        client_version: (u8, u8),
        server_version: (u8, u8),
    ) -> Result<(Handshake<ClientSendVersion>, Handshake<ServerRecvVersion>)> {
        let psk: [u8; 32] = [1; 32];

        let builder = NoiseBuilder::new("Noise_XXpsk0_25519_ChaChaPoly_BLAKE2b".parse()?);

        let client_keypair = builder.generate_keypair()?;
        let client_private_key = client_keypair.private;

        let server_keypair = builder.generate_keypair()?;
        let server_private_key = server_keypair.private;

        let client_version = Version::init(client_version.0, client_version.1);
        let server_version = Version::init(server_version.0, server_version.1);

        let hs_client = Handshake::new_client(client_version, psk, client_private_key);
        let hs_server = Handshake::new_server(server_version, psk, server_private_key);

        Ok((hs_client, hs_server))
    }

    #[test]
    fn version_to_bytes() -> Result<()> {
        let version = Version::init(0, 1);

        let version_to_bytes = version.to_bytes()?;
        let version_from_bytes = Version::from_bytes(&version_to_bytes)?;

        assert_eq!(version, version_from_bytes.1);

        Ok(())
    }

    #[test]
    fn version_exchange_success() -> Result<()> {
        let (hs_client, hs_server) = init_handshakers((1, 0), (1, 0))?;

        let mut buf = [0; 8];

        let mut client_buf = &mut buf[..VERSION_BYTES_LEN];
        let hs_client = hs_client.send_client_version(&mut client_buf)?;

        let mut server_buf = &mut buf[..VERSION_BYTES_LEN];
        let hs_server = hs_server.recv_client_version(&mut server_buf)?;

        let mut server_buf = &mut buf[..VERSION_BYTES_LEN];
        hs_server.send_server_version(&mut server_buf)?;

        let mut client_buf = &mut buf[..VERSION_BYTES_LEN];
        hs_client.recv_server_version(&mut client_buf)?;

        Ok(())
    }

    #[test]
    fn version_exchange_failure() -> Result<()> {
        let (hs_client, hs_server) = init_handshakers((3, 7), (1, 0))?;

        let mut buf = [0; 8];

        let mut client_buf = &mut buf[..VERSION_BYTES_LEN];
        let hs_client = hs_client.send_client_version(&mut client_buf)?;

        let mut server_buf = &mut buf[..VERSION_BYTES_LEN];
        let hs_server = hs_server.recv_client_version(&mut server_buf)?;

        let mut server_buf = &mut buf[..VERSION_BYTES_LEN];
        let _hs_server = hs_server.send_server_version(&mut server_buf)?;

        let mut client_buf = &mut buf[..VERSION_BYTES_LEN];
        let hs_client = hs_client.recv_server_version(&mut client_buf);

        assert!(hs_client.is_err());

        let err = hs_client.unwrap_err().downcast::<HandshakeError>().unwrap();
        assert_eq!(
            *err,
            HandshakeError::IncompatibleServerVersion {
                received: 1,
                expected: 3
            }
        );

        Ok(())
    }

    #[test]
    fn handshake() -> Result<()> {
        // Build the handshake client and server.
        let (hs_client, hs_server) = init_handshakers((1, 0), (1, 0))?;

        // Define a shared buffer for sending and receiving messages.
        let mut buf = [0; 1024];

        // Send and receive client version.
        let (hs_client, hs_server) = {
            let mut client_buf = &mut buf[..VERSION_BYTES_LEN];
            let hs_client = hs_client.send_client_version(&mut client_buf)?;
            let mut server_buf = &mut buf[..VERSION_BYTES_LEN];
            let hs_server = hs_server.recv_client_version(&mut server_buf)?;
            (hs_client, hs_server)
        };

        // Send and receive server version.
        let (hs_client, hs_server) = {
            let mut server_buf = &mut buf[..VERSION_BYTES_LEN];
            let hs_server = hs_server.send_server_version(&mut server_buf)?;
            let mut client_buf = &mut buf[..VERSION_BYTES_LEN];
            let hs_client = hs_client.recv_server_version(&mut client_buf)?;
            (hs_client, hs_server)
        };

        // Build client and server Noise state machines.
        let (hs_client, hs_server) = {
            let hs_client = hs_client.build_client_noise_state_machine()?;
            let hs_server = hs_server.build_server_noise_state_machine()?;
            (hs_client, hs_server)
        };

        // Send and receive client ephemeral key.
        let (hs_client, hs_server) = {
            let hs_client = hs_client.send_client_ephemeral_key(&mut buf)?;
            let mut server_buf = &mut buf[..EPHEMERAL_KEY_BYTES_LEN];
            let hs_server = hs_server.recv_client_ephemeral_key(&mut server_buf)?;
            (hs_client, hs_server)
        };

        // Send and receive server ephemeral and static keys.
        let (hs_client, hs_server) = {
            let hs_server = hs_server.send_server_ephemeral_and_static_key(&mut buf)?;
            let mut client_buf = &mut buf[..EPHEMERAL_AND_STATIC_KEY_BYTES_LEN];
            let hs_client = hs_client.recv_server_ephemeral_and_static_key(&mut client_buf)?;
            (hs_client, hs_server)
        };

        // Send and receive client static key.
        let (hs_client, hs_server) = {
            let hs_client = hs_client.send_client_static_key(&mut buf)?;
            let mut server_buf = &mut buf[..STATIC_KEY_BYTES_LEN];
            let hs_server = hs_server.recv_client_static_key(&mut server_buf)?;
            (hs_client, hs_server)
        };

        // Initialise client and server transport mode.
        let mut hs_client = hs_client.init_client_transport_mode()?;
        let mut hs_server = hs_server.init_server_transport_mode()?;

        // Write an encrypted message.
        let msg_text = b"An impeccably polite pangolin";
        let (write_len, encrypted_msg) = hs_client.write_message(msg_text)?;

        // Read an encrypted message.
        let msg = hs_server.read_message(&encrypted_msg[..], write_len.try_into()?)?;

        assert_eq!(msg_text, &msg[..]);

        Ok(())
    }
}
