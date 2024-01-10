// Test the asynchronous handshake and (de)fragmented message exchange.

use async_std::{
    net::{TcpListener, TcpStream},
    stream::StreamExt,
    task,
};

use cable_handshake::{async_std::handshake, Result, Version};
use snow::Builder as NoiseBuilder;

#[async_std::test]
async fn async_handshake_works() -> Result<()> {
    // Define handshake versions.
    let client_version = Version::init(1, 0);
    let server_version = Version::init(1, 7);

    let psk: [u8; 32] = [1; 32];

    // Generate keypairs.
    let builder = NoiseBuilder::new("Noise_XXpsk0_25519_ChaChaPoly_BLAKE2b".parse()?);
    let client_keypair = builder.generate_keypair()?;
    let server_keypair = builder.generate_keypair()?;
    let client_private_key = client_keypair.private;
    let server_private_key = server_keypair.private;

    // Deploy a TCP listener.
    //
    // Assigning port to 0 means that the OS selects an available port for us.
    let listener = TcpListener::bind("127.0.0.1:0").await?;

    // Retrieve the address of the TCP listener to be able to connect later on.
    let addr = listener.local_addr()?;

    // Define the messages to be sent and received.
    let msg_1 = b"An impeccably polite pangolin";
    // This message is more than 65535 bytes and will therefore be fragmented
    // when sent and defragmented when received.
    let msg_2 = [7; 77777];

    task::spawn(async move {
        let mut incoming = listener.incoming();

        // Accept connections and process them serially.
        while let Some(stream) = incoming.next().await {
            let mut stream = stream.unwrap();

            // Perform the handshake.
            let mut encrypted =
                handshake::server(&mut stream, server_version, psk, server_private_key)
                    .await
                    .unwrap();

            // Read a short encrypted message.
            let msg = encrypted
                .read_message_from_async_stream(&mut stream)
                .await
                .unwrap();
            assert_eq!(msg, msg_1);

            // Write a long encrypted message.
            encrypted
                .write_message_to_async_stream(&mut stream, &msg_2)
                .await
                .unwrap();

            return;
        }
    });

    // Connect to the TCP server.
    let mut stream = TcpStream::connect(addr).await?;

    // Perform the handshake.
    let mut encrypted =
        handshake::client(&mut stream, client_version, psk, client_private_key).await?;

    // Write a short encrypted message.
    encrypted
        .write_message_to_async_stream(&mut stream, msg_1)
        .await?;

    // Read a long encrypted message.
    let msg = encrypted
        .read_message_from_async_stream(&mut stream)
        .await?;
    assert_eq!(msg, msg_2);

    Ok(())
}
