// SPDX-FileCopyrightText: 2024 the cabal-club authors
//
// SPDX-License-Identifier: LGPL-3.0-or-later

//! Example demonstrating a synchronous handshake and (de)fragmented message
//! exchange over stdin and stdout.

use std::{env, fs, io::Write};

use cable_handshake::{sync::handshake, Result, Version};
use io_streams::StreamDuplexer;
use snow::Builder as NoiseBuilder;

fn help() {
    println!(
        "Usage:

<PSK> <initiator|responder> <MSG>
  Provide a PSK, set mode as initiator or responder and include a message to be sent.

<PSK> <initiator|responder> --file <MSG_FILE_PATH>
  Provide a PSK, set mode as initiator or responder and include the path to a file containing a message to be sent."
    );
}

fn setup() -> Result<(Version, Vec<u8>, StreamDuplexer)> {
    // Define handshake version.
    let version = Version::init(1, 0);

    // Generate keypair.
    let builder = NoiseBuilder::new("Noise_XXpsk0_25519_ChaChaPoly_BLAKE2b".parse()?);
    let keypair = builder.generate_keypair()?;
    let private_key = keypair.private;

    // Construct a duplex stream.
    let stream = StreamDuplexer::stdin_stdout()?;

    Ok((version, private_key, stream))
}

fn parse_psk_and_mode<'a>(psk_arg: &'a str, mode_arg: &'a str) -> ([u8; 32], &'a str) {
    // Parse the PSK argument.
    let mut psk = [0u8; 32];
    {
        let mut psk = &mut psk[..];
        psk.write_all(psk_arg.as_bytes()).unwrap();
    }

    // Parse the mode argument.
    let mode = mode_arg;

    (psk, mode)
}

fn run_client(psk: [u8; 32], msg: &str) -> Result<()> {
    let (version, private_key, mut stream) = setup()?;

    let mut encrypted = handshake::client(&mut stream, version, psk, private_key)?;

    // Write message.
    encrypted.write_message_to_stream(&mut stream, msg.as_bytes())?;
    eprintln!("Sent message");

    // Read message.
    let received_msg = encrypted.read_message_from_stream(&mut stream)?;
    eprintln!(
        "Received message: {}",
        std::str::from_utf8(&received_msg).unwrap()
    );

    // Handle end-of-stream marker.
    if encrypted.read_message_from_stream(&mut stream)?.is_empty() {
        eprintln!("Received end-of-stream marker");

        // Write zero-length message (end-of-stream marker).
        encrypted.write_eos_marker_to_stream(&mut stream)?;
        eprintln!("Sent end-of-stream marker");
    }

    eprintln!("Stream closed");

    Ok(())
}

fn run_server(psk: [u8; 32], msg: &str) -> Result<()> {
    let (version, private_key, mut stream) = setup()?;

    let mut encrypted = handshake::server(&mut stream, version, psk, private_key)?;

    // Read message.
    let received_msg = encrypted.read_message_from_stream(&mut stream)?;
    eprintln!(
        "Received message: {}",
        std::str::from_utf8(&received_msg).unwrap()
    );

    // Write message.
    encrypted.write_message_to_stream(&mut stream, msg.as_bytes())?;
    eprintln!("Sent message");

    // Write zero-length message (end-of-stream marker).
    encrypted.write_eos_marker_to_stream(&mut stream)?;
    eprintln!("Sent end-of-stream marker");

    // Handle end-of-stream marker.
    if encrypted.read_message_from_stream(&mut stream)?.is_empty() {
        eprintln!("Received end-of-stream marker");
    }

    eprintln!("Stream closed");

    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let args_len = args.len();
    if args_len != 4 | 5 {
        help();
        return Ok(());
    }

    let (psk, mode) = parse_psk_and_mode(&args[1], &args[2]);

    let msg = if args_len == 4 {
        args[3].to_owned()
    } else {
        let msg = fs::read_to_string(&args[4])?;
        msg
    };

    match mode {
        "initiator" => run_client(psk, &msg.trim())?,
        "responder" => run_server(psk, &msg.trim())?,
        _ => help(),
    }

    Ok(())
}
