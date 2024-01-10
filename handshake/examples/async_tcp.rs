// SPDX-FileCopyrightText: 2024 the cabal-club authors
//
// SPDX-License-Identifier: LGPL-3.0-or-later

//! Example demonstrating an asynchronous handshake and (de)fragmented message
//! exchange over a TCP connection.

use std::env;

use async_std::net::{TcpListener, TcpStream};

use cable_handshake::{async_std::handshake, Result, Version};
use snow::Builder as NoiseBuilder;

fn help() {
    println!(
        "Usage:

async_tcp
    Execute the handshake as a client (initiator) over TCP.
async_tcp {{-s|--server}}
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

#[async_std::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    match args.len() {
        1 => run_client().await.unwrap(),
        2 => match args[1].as_str() {
            "-s" | "--server" => run_server().await.unwrap(),
            _ => help(),
        },
        _ => help(),
    }
}

async fn run_client() -> Result<()> {
    let (version, psk, private_key) = setup()?;

    println!("Connecting to TCP server on 127.0.0.1:9999");
    let mut stream = TcpStream::connect("127.0.0.1:9999").await?;
    println!("Connected");

    println!("Initiating handshake...");
    let mut encrypted = handshake::client(&mut stream, version, psk, private_key).await?;

    // Write a short encrypted message.
    let short_msg = b"An impeccably polite pangolin";
    encrypted
        .write_message_to_async_stream(&mut stream, short_msg)
        .await?;

    println!("Sent message");

    // Read a long encrypted message.
    let long_msg = encrypted
        .read_message_from_async_stream(&mut stream)
        .await?;

    println!("Received message: {:?}", long_msg);

    Ok(())
}

async fn run_server() -> Result<()> {
    let (version, psk, private_key) = setup()?;

    // Deploy a TCP listener.
    let listener = TcpListener::bind("127.0.0.1:9999").await?;

    println!("TCP server listening on 127.0.0.1:9999");

    // Accept connection.
    let (mut stream, _addr) = listener.accept().await?;

    // Perform the handshake.
    let mut encrypted = handshake::server(&mut stream, version, psk, private_key)
        .await
        .unwrap();

    // Read a short encrypted message.
    let short_msg = encrypted
        .read_message_from_async_stream(&mut stream)
        .await
        .unwrap();

    println!("Received message: {:?}", short_msg);

    // This message is more than 65535 bytes and will therefore be fragmented
    // when sent and defragmented when received.
    let long_msg = [7; 77777];

    // Write a long encrypted message.
    encrypted
        .write_message_to_async_stream(&mut stream, &long_msg)
        .await
        .unwrap();

    println!("Sent message");

    Ok(())
}
