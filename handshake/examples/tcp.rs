// SPDX-FileCopyrightText: 2024 the cabal-club authors
//
// SPDX-License-Identifier: LGPL-3.0-or-later

//! Example demonstrating a synchronous handshake and (de)fragmented message
//! exchange over a TCP connection.

use std::{
    env,
    net::{TcpListener, TcpStream},
};

use cable_handshake::{sync::handshake, Result, Version};
use snow::Builder as NoiseBuilder;

fn help() {
    println!(
        "Usage:

tcp
    Execute the handshake as a client (initiator) over TCP.
tcp {{-s|--server}}
    Execute the handshake as a server (responder) over TCP."
    );
}

fn setup() -> Result<(Version, [u8; 32], Vec<u8>)> {
    // Define handshake version.
    let version = Version::init(1, 0);

    // Define pre-shared key.
    let psk: [u8; 32] = [1; 32];

    // Generate keypair.
    let builder = NoiseBuilder::new("Noise_XXpsk0_25519_ChaChaPoly_BLAKE2b".parse()?);
    let keypair = builder.generate_keypair()?;
    let private_key = keypair.private;

    Ok((version, psk, private_key))
}

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.len() {
        1 => run_client().unwrap(),
        2 => match args[1].as_str() {
            "-s" | "--server" => run_server().unwrap(),
            _ => help(),
        },
        _ => help(),
    }
}

fn run_client() -> Result<()> {
    let (version, psk, private_key) = setup()?;

    println!("Connecting to TCP server on 127.0.0.1:9999");
    let mut stream = TcpStream::connect("127.0.0.1:9999")?;
    println!("Connected");

    println!("Initiating handshake...");
    let mut encrypted = handshake::client(&mut stream, version, psk, private_key)?;

    // Return the public key of the remote peer (as bytes).
    if let Some(key) = encrypted.get_remote_public_key() {
        println!("Completed handshake with {:?}", key)
    }

    // Write a short encrypted message.
    let msg = b"An impeccably polite pangolin";
    encrypted.write_message_to_stream(&mut stream, msg)?;
    println!("Sent message");

    // Write a long encrypted message.
    let msg = [7; 70000];
    encrypted.write_message_to_stream(&mut stream, &msg)?;
    println!("Sent message");

    // Write zero-length message (end-of-stream marker).
    encrypted.write_eos_marker_to_stream(&mut stream)?;
    println!("Sent end-of-stream marker");

    // Handle end-of-stream marker.
    if encrypted.read_message_from_stream(&mut stream)?.is_empty() {
        println!("Received end-of-stream marker");
    }

    Ok(())
}

fn run_server() -> Result<()> {
    let (version, psk, private_key) = setup()?;

    // Deploy a TCP listener.
    let listener = TcpListener::bind("127.0.0.1:9999")?;

    println!("TCP server listening on 127.0.0.1:9999");

    // Accept connection.
    if let Ok((mut stream, _addr)) = listener.accept() {
        println!("Accepted connection");

        println!("Responding to handshake...");
        let mut encrypted = handshake::server(&mut stream, version, psk, private_key)?;

        // Read a short encrypted message.
        let msg = encrypted.read_message_from_stream(&mut stream)?;
        println!("Received message: {:?}", msg);

        // Read a long encrypted message.
        let msg = encrypted.read_message_from_stream(&mut stream)?;
        // Print the first nine bytes of the message.
        println!("Received message: {:?}", &msg[..9]);

        // Handle end-of-stream marker.
        if encrypted.read_message_from_stream(&mut stream)?.is_empty() {
            println!("Received end-of-stream marker");

            // Write zero-length message (end-of-stream marker).
            encrypted.write_eos_marker_to_stream(&mut stream)?;
            println!("Sent end-of-stream marker");
        }
    }

    Ok(())
}
