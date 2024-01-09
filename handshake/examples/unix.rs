use std::{
    env,
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
};

use cable_handshake::{sync::handshake, Result, Version};
use snow::Builder as NoiseBuilder;

const SOCKET_PATH: &str = "/tmp/handshake.sock";

fn help() {
    println!(
        "Usage:

unix_handshake
    Execute the handshake as a client (initiator) over a Unix socket.
unix_handshake {{-s|--server}}
    Execute the handshake as a server (responder) over a Unix socket."
    );
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
    let version = Version::init(1, 0);

    let psk: [u8; 32] = [1; 32];

    let builder = NoiseBuilder::new("Noise_XXpsk0_25519_ChaChaPoly_BLAKE2b".parse()?);
    let keypair = builder.generate_keypair()?;
    let private_key = keypair.private;

    println!("Connecting to Unix socket at {}", SOCKET_PATH);
    let mut stream = UnixStream::connect(SOCKET_PATH)?;
    println!("Connected");

    println!("Initiating handshake...");
    let mut encrypted = handshake::client(&mut stream, version, psk, private_key)?;

    // Write a short encrypted message.
    let msg = b"An impeccably polite pangolin";
    encrypted.write_message_to_stream(&mut stream, msg)?;

    println!("Sent message");

    // Write a long encrypted message.
    let msg = [7; 70000];
    encrypted.write_message_to_stream(&mut stream, &msg)?;

    println!("Sent message");

    Ok(())
}

fn run_server() -> Result<()> {
    let version = Version::init(1, 0);

    let psk: [u8; 32] = [1; 32];

    let builder = NoiseBuilder::new("Noise_XXpsk0_25519_ChaChaPoly_BLAKE2b".parse()?);
    let keypair = builder.generate_keypair()?;
    let private_key = keypair.private;

    // Deploy a Unix socket listener.
    let listener = UnixListener::bind(SOCKET_PATH)?;

    println!("Unix socket listening on {}", SOCKET_PATH);

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

        println!("Received message: {:?}", msg);
    }

    // Remove the socket file so that the address will be bound successfully
    // the next time this example runs.
    if Path::new(SOCKET_PATH).exists() {
        std::fs::remove_file(SOCKET_PATH)?
    }

    Ok(())
}
