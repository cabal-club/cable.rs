//! Test the asynchronous handshake and (de)fragmented message exchange.

use std::{thread, time::Duration};

use async_std::{
    net::{TcpListener, TcpStream},
    task,
};

use cable_handshake::{async_std::handshake, Result, Version};
use snow::Builder as NoiseBuilder;

const MSG_1: &[u8; 29] = b"An impeccably polite pangolin";
const MSG_2: [u8; 77777] = [7; 77777];

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

async fn client_handshake() -> Result<()> {
    let (version, psk, private_key) = setup()?;

    // Connect to the TCP server.
    let mut stream = TcpStream::connect("127.0.0.1:9999").await?;

    // Perform the handshake.
    let mut encrypted = handshake::client(&mut stream, version, psk, private_key).await?;

    // Write a short encrypted message.
    encrypted
        .write_message_to_async_stream(&mut stream, MSG_1)
        .await?;

    // Read a long encrypted message.
    let msg = encrypted
        .read_message_from_async_stream(&mut stream)
        .await?;
    assert_eq!(msg, MSG_2);

    Ok(())
}

async fn server_handshake() -> Result<()> {
    let (version, psk, private_key) = setup()?;

    // Deploy a TCP listener.
    let listener = TcpListener::bind("127.0.0.1:9999").await?;

    // Accept connection.
    let (mut stream, _addr) = listener.accept().await?;

    // Perform the handshake.
    let mut encrypted = handshake::server(&mut stream, version, psk, private_key).await?;

    // Read a short encrypted message.
    let msg = encrypted
        .read_message_from_async_stream(&mut stream)
        .await?;
    assert_eq!(msg, MSG_1);

    // Write a long encrypted message.
    encrypted
        .write_message_to_async_stream(&mut stream, &MSG_2)
        .await?;

    Ok(())
}

#[async_std::test]
async fn async_handshake_works() -> Result<()> {
    let server_task = task::spawn(async { server_handshake().await.unwrap() });

    // The client handshake sometimes attempts the TCP connection before the
    // server is ready, resulting in "Connection refused" and a test failure.
    //
    // Sleep briefly to avoid the aforementioned scenario.
    thread::sleep(Duration::from_millis(10));

    client_handshake().await.unwrap();

    let _res = server_task.await;

    Ok(())
}
