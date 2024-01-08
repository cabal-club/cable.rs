use cable_handshake::{
    ephemeral_and_static_key_bytes_len, ephemeral_key_bytes_len, static_key_bytes_len,
    version_bytes_len, ClientSendVersion, Handshake, Result, ServerRecvVersion, Version,
};
use snow::Builder as NoiseBuilder;

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

fn main() -> Result<()> {
    // Build the handshake client and server.
    let (hs_client, hs_server) = init_handshakers((1, 0), (1, 0))?;

    // Define a shared buffer for sending and receiving messages.
    let mut buf = [0; 1024];

    // Send and receive client version.
    let (hs_client, hs_server) = {
        let mut client_buf = &mut buf[..version_bytes_len()];
        let hs_client = hs_client.send_client_version(&mut client_buf)?;
        let mut server_buf = &mut buf[..version_bytes_len()];
        let hs_server = hs_server.recv_client_version(&mut server_buf)?;
        (hs_client, hs_server)
    };

    // Send and receive server version.
    let (hs_client, hs_server) = {
        let mut server_buf = &mut buf[..version_bytes_len()];
        let hs_server = hs_server.send_server_version(&mut server_buf)?;
        let mut client_buf = &mut buf[..version_bytes_len()];
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
        let mut server_buf = &mut buf[..ephemeral_key_bytes_len()];
        let hs_server = hs_server.recv_client_ephemeral_key(&mut server_buf)?;
        (hs_client, hs_server)
    };

    // Send and receive server ephemeral and static key.
    let (hs_client, hs_server) = {
        let hs_server = hs_server.send_server_ephemeral_and_static_key(&mut buf)?;
        let mut client_buf = &mut buf[..ephemeral_and_static_key_bytes_len()];
        let hs_client = hs_client.recv_server_ephemeral_and_static_key(&mut client_buf)?;
        (hs_client, hs_server)
    };

    // Send and receive client static key.
    let (hs_client, hs_server) = {
        let hs_client = hs_client.send_client_static_key(&mut buf)?;
        let mut server_buf = &mut buf[..static_key_bytes_len()];
        let hs_server = hs_server.recv_client_static_key(&mut server_buf)?;
        (hs_client, hs_server)
    };

    // Initialise client and server transport mode.
    let mut hs_client = hs_client.init_client_transport_mode()?;
    let mut hs_server = hs_server.init_server_transport_mode()?;

    // Retrieve the static keys (public key of remote).
    let _server_static_key = hs_client.get_remote_static();
    let _client_static_key = hs_server.get_remote_static();

    // Write an encrypted message.
    let msg_text = b"An impeccably polite pangolin";
    let write_len = hs_client.write_message(msg_text, &mut buf)?;

    // Read an encrypted message.
    let mut msg_buf = [0; 48];
    let _read_len = hs_server.read_message(&buf[..write_len], &mut msg_buf)?;

    // Write another encrypted message.
    let msg_text = b"strolled across the crowded concourse.";
    let _write_len = hs_client.write_message(msg_text, &mut buf)?;

    Ok(())
}
