//! This example tests the cable manager by creating a TCP stream, invoking
//! the cable listener and then writing and reading requests and responses
//! to and from the stream. In this way, we can trace the request-response
//! handling.
//!
//! Run the example with debug logging enabled in a terminal:
//!
//! `RUST_LOG=debug cargo run --example stream`
//!
//! The code here is very similar to the test code found in
//! `cable_core/src/manager.rs`, with the primary exception being that it uses
//! a proper TCP stream and not a mock IO stream.

use std::{thread, time::Duration};

use async_std::{
    net::{TcpListener, TcpStream},
    stream::StreamExt,
    task,
};
use cable::{
    constants::{HASH_RESPONSE, NO_CIRCUIT, POST_RESPONSE},
    message::{MessageBody, ResponseBody},
    ChannelOptions, Error, Message,
};
use desert::{FromBytes, ToBytes};
use futures::{AsyncReadExt, AsyncWriteExt};

use cable_core::{CableManager, MemoryStore};

// The circuit_id field is not currently in use; set to all zeros.
const CIRCUIT_ID: [u8; 4] = NO_CIRCUIT;
const TTL: u8 = 1;
const PORT: &str = "8007";

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[async_std::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    // Create a store and a cable manager.
    let store = MemoryStore::default();
    let mut cable = CableManager::new(store);
    let cable_clone = cable.clone();

    // Publish a test post to the "tao" channel.
    task::block_on(async {
        cable
            .post_text(
                "tao",
                "Need little, want less. Forget the rules. Be untroubled.",
            )
            .await
            .unwrap();
    });

    // Generate a novel request ID.
    let (_req_id, req_id_bytes) = cable.new_req_id().await?;

    // Channel time range request parameters.
    let opts = ChannelOptions::new("tao", 0, 0, 10);

    // Create a channel time range request.
    let channel_time_range_req =
        Message::channel_time_range_request(CIRCUIT_ID, req_id_bytes, TTL, opts);
    let req_bytes = channel_time_range_req.to_bytes()?;

    task::spawn(async move {
        // Deploy a TCP listener and pass the stream to the cable manager.
        println!("Deploying TCP server on localhost:{}", PORT);

        let listener = TcpListener::bind(format!("localhost:{}", PORT))
            .await
            .unwrap();
        let mut incoming = listener.incoming();
        while let Some(stream) = incoming.next().await {
            let stream = stream.unwrap();
            let client = cable.clone();
            task::spawn(async move {
                client.listen(stream).await.unwrap();
            });
        }
    });

    println!("Connecting to TCP server on localhost:{}", PORT);
    let mut stream = TcpStream::connect(format!("localhost:{}", PORT)).await?;

    // Write the request bytes to the stream.
    stream.write_all(&req_bytes).await?;

    // Sleep briefly to allow time for the cable manager to respond.
    let five_millis = Duration::from_millis(5);
    thread::sleep(five_millis);

    // Read the response from the stream.
    let mut res_bytes = [0u8; 1024];
    let _n = stream.read(&mut res_bytes).await?;

    // Ensure that a hash response was returned by the listening peer.
    let (_bytes_len, msg) = Message::from_bytes(&res_bytes)?;
    assert_eq!(msg.message_type(), HASH_RESPONSE);

    if let MessageBody::Response { body } = msg.body {
        if let ResponseBody::Hash { hashes } = body {
            // Only a single post hash should be returned.
            assert_eq!(hashes.len(), 1);

            // Generate a novel request ID.
            let (_req_id, req_id_bytes) = cable_clone.new_req_id().await?;

            // Create a post request.
            let post_req = Message::post_request(CIRCUIT_ID, req_id_bytes, TTL, hashes);
            let req_bytes = post_req.to_bytes()?;

            // Write the request bytes to the stream.
            stream.write_all(&req_bytes).await?;

            // Sleep briefly to allow time for the cable manager to respond.
            thread::sleep(five_millis);

            // Read the response from the stream.
            let mut res_bytes = [0u8; 1024];
            let _n = stream.read(&mut res_bytes).await?;

            // Ensure that a post response was returned by the listening peer.
            let (_bytes_len, msg) = Message::from_bytes(&res_bytes)?;
            assert_eq!(msg.message_type(), POST_RESPONSE);
        }
    }

    Ok(())
}
