use std::{
    convert, io,
    io::{Read, Write},
};

use crate::{
    ephemeral_and_static_key_bytes_len, ephemeral_key_bytes_len, static_key_bytes_len,
    version_bytes_len, Handshake, HandshakeComplete, Result, Version,
};

pub fn handshake_client<T: Read + Write>(
    stream: &mut T,
    version: Version,
    psk: [u8; 32],
    private_key: Vec<u8>,
) -> Result<Handshake<HandshakeComplete>> {
    let mut buf = [0; 256];

    let handshake = Handshake::new_client(version, psk, private_key);

    // Send version.
    let send_buf = &mut buf[..version_bytes_len()];
    let handshake = handshake.send_client_version(send_buf)?;
    stream.write_all(send_buf)?;

    // Receive version.
    let recv_buf = &mut buf[..version_bytes_len()];
    stream.read_exact(recv_buf)?;
    let handshake = handshake.recv_server_version(recv_buf)?;

    // Build Noise state machine.
    let handshake = handshake.build_client_noise_state_machine()?;

    // Send ephemeral key.
    let send_buf = &mut buf[..ephemeral_key_bytes_len()];
    let handshake = handshake.send_client_ephemeral_key(send_buf)?;
    stream.write_all(send_buf)?;

    // Receive ephemeral and static keys.
    let recv_buf = &mut buf[..ephemeral_and_static_key_bytes_len()];
    stream.read_exact(recv_buf)?;
    let handshake = handshake.recv_server_ephemeral_and_static_key(recv_buf)?;

    // Send static key.
    let send_buf = &mut buf[..static_key_bytes_len()];
    let handshake = handshake.send_client_static_key(send_buf)?;
    stream.write_all(send_buf)?;

    let handshake = handshake.init_client_transport_mode()?;

    Ok(handshake)
}
