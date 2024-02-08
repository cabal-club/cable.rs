// SPDX-FileCopyrightText: 2024 the cabal-club authors
//
// SPDX-License-Identifier: LGPL-3.0-or-later

//! Example demonstrating a synchronous handshake and (de)fragmented message
//! exchange over stdin and stdout.

use std::{
    env,
    io::{Read, Write},
};

use cable_handshake::{sync::handshake, Result, Version};
use io_streams::StreamDuplexer;
use snow::Builder as NoiseBuilder;

fn help() {
    println!(
        "Usage:

<PSK> <initiator|responder> <MSG>
    Provide a PSK, set mode as initiator or responder and include a message to be sent."
    );
}

fn setup() -> Result<(Version, Vec<u8>)> {
    // Define handshake version.
    let version = Version::init(1, 0);

    // Generate keypair.
    let builder = NoiseBuilder::new("Noise_XXpsk0_25519_ChaChaPoly_BLAKE2b".parse()?);
    let keypair = builder.generate_keypair()?;
    let private_key = keypair.private;

    Ok((version, private_key))
}

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.len() {
        0 | 1 | 2 | 3 => help(),
        4 => {
            let mut psk = [0u8; 32];
            {
                let mut psk = &mut psk[..];
                psk.write_all(args[1].as_bytes()).unwrap();
            }
            let mode = &args[2];
            let msg = &args[3];

            let mut stream = StreamDuplexer::stdin_stdout().unwrap();

            match mode.as_str() {
                "initiator" => run_client(&mut stream, psk, &msg).unwrap(),
                "responder" => run_server(&mut stream, psk, &msg).unwrap(),
                _ => help(),
            }
        }
        _ => help(),
    }

    eprintln!("Stream closed");
}

fn run_client<T: Read + Write>(stream: &mut T, psk: [u8; 32], msg: &str) -> Result<()> {
    let (version, private_key) = setup()?;

    let mut encrypted = handshake::client(stream, version, psk, private_key)?;

    // Write message.
    encrypted.write_message_to_stream(stream, msg.as_bytes())?;
    eprintln!("Sent message");

    // Read message.
    let received_msg = encrypted.read_message_from_stream(stream)?;
    eprintln!(
        "Received message: {}",
        std::str::from_utf8(&received_msg).unwrap()
    );

    // Handle end-of-stream marker.
    if encrypted.read_message_from_stream(stream)?.is_empty() {
        eprintln!("Received end-of-stream marker");

        // Write zero-length message (end-of-stream marker).
        encrypted.write_eos_marker_to_stream(stream)?;
        eprintln!("Sent end-of-stream marker");
    }

    Ok(())
}

fn run_server<T: Read + Write>(stream: &mut T, psk: [u8; 32], msg: &str) -> Result<()> {
    let (version, private_key) = setup()?;

    let mut encrypted = handshake::server(stream, version, psk, private_key)?;

    // Read message.
    let received_msg = encrypted.read_message_from_stream(stream)?;
    eprintln!(
        "Received message: {}",
        std::str::from_utf8(&received_msg).unwrap()
    );

    // Write message.
    encrypted.write_message_to_stream(stream, msg.as_bytes())?;
    eprintln!("Sent message");

    // Write zero-length message (end-of-stream marker).
    encrypted.write_eos_marker_to_stream(stream)?;
    eprintln!("Sent end-of-stream marker");

    // Handle end-of-stream marker.
    if encrypted.read_message_from_stream(stream)?.is_empty() {
        eprintln!("Received end-of-stream marker");
    }

    Ok(())
}
