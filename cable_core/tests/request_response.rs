//! Test the cable manager by creating a TCP stream, invoking
//! the cable listener and then writing and reading requests and responses
//! to and from the stream. In this way, we can trace the request-response
//! handling.
//!
//! Run the test with debug logging enabled in a terminal:
//!
//! `RUST_LOG=debug cargo test request_response`
//!
//! An outline of the actions taken in this test:
//!
//! 1) Publish a post.
//!
//! 2) Send a channel time range request matching the channel to which the post
//! was published. Ensure that a hash response is returned.
//!
//! 3) Use the hash from the hash response to send a post request, ensuring the
//! correct post is returned.
//!
//! 4) Publish two more posts, each to a different channel.
//!
//! 5) Send a channel list request. Ensure that three channels are returned and
//! that they match the channels to which the three posts were published.
//!
//! 6) Publish a second post to the first channel.
//!
//! 7) Ensure a hash response is returned with hashes for both posts.
//!
//! 8) Send a post request for the first channel, ensuring both post payloads
//! are returned.

use std::{thread, time::Duration};

use async_std::{
    net::{TcpListener, TcpStream},
    stream::StreamExt,
    task,
};
use cable::{
    constants::{CHANNEL_LIST_RESPONSE, HASH_RESPONSE, NO_CIRCUIT, POST_RESPONSE},
    message::{MessageBody, ResponseBody},
    ChannelOptions, Error, Message,
};
use desert::{FromBytes, ToBytes};
use futures::{AsyncReadExt, AsyncWriteExt};
use log::info;

use cable_core::{CableManager, MemoryStore};

// The circuit_id field is not currently in use; set to all zeros.
const CIRCUIT_ID: [u8; 4] = NO_CIRCUIT;
const TTL: u8 = 1;

// Initialise the logger in test mode.
//
// Set `is_test()` to `false` if you wish to see logging output during the
// test run.
fn init() {
    let _ = env_logger::builder().is_test(false).try_init();
}

// Get the current system time in seconds since the UNIX epoch.
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[async_std::test]
async fn request_response() -> Result<(), Error> {
    init();

    // Create a store and a cable manager.
    let store = MemoryStore::default();
    let mut cable = CableManager::new(store);
    let cable_clone = cable.clone();

    // Publish a test post to the "tao" channel.
    let post_hash = cable
        .post_text(
            "tao",
            "Need little, want less. Forget the rules. Be untroubled.",
        )
        .await?;

    // Generate a novel request ID.
    let (_req_id, req_id_bytes) = cable.new_req_id().await?;

    // Channel time range request parameters.
    let opts = ChannelOptions::new("tao", now(), 0, 10);

    // Create a channel time range request.
    let channel_time_range_req =
        Message::channel_time_range_request(CIRCUIT_ID, req_id_bytes, TTL, opts);
    let req_bytes = channel_time_range_req.to_bytes()?;

    // Deploy a TCP listener.
    //
    // Assigning port to 0 means that the OS selects an available port for us.
    let listener = TcpListener::bind("127.0.0.1:0").await?;

    // Retrieve the address of the TCP listener to be able to connect later on.
    let addr = listener.local_addr()?;
    info!("Deployed TCP server on {}", addr);

    task::spawn(async move {
        // Listen for incoming TCP connections and pass any inbound streams to
        // the cable manager.
        let mut incoming = listener.incoming();
        while let Some(stream) = incoming.next().await {
            if let Ok(stream) = stream {
                let cable = cable_clone.clone();
                task::spawn(async move {
                    cable.listen(stream).await.unwrap();
                });
            }
        }
    });

    let mut stream = TcpStream::connect(addr).await?;
    info!("Connected to TCP server on {}", addr);

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
            // Ensure the returned hash matches the hash of the original
            // text post.
            assert_eq!(hashes[0], post_hash);

            // Generate a novel request ID.
            let (_req_id, req_id_bytes) = cable.new_req_id().await?;

            // Create a post request.
            let post_req = Message::post_request(CIRCUIT_ID, req_id_bytes, TTL, hashes);
            let req_bytes = post_req.to_bytes()?;

            // Write the request bytes to the stream.
            stream.write_all(&req_bytes).await?;

            // Sleep briefly to allow time for the cable manager to respond.
            thread::sleep(five_millis);

            // Read the response from the stream.
            let _n = stream.read(&mut res_bytes).await?;

            // Ensure that a post response was returned by the listening peer.
            let (_bytes_len, msg) = Message::from_bytes(&res_bytes)?;
            assert_eq!(msg.message_type(), POST_RESPONSE);
        }
    }

    // Publish a test post to the "bamboo" channel.
    let _post_hash = cable
        .post_text(
            "bamboo",
            "Without bamboo, we lose serenity and culture itself.",
        )
        .await?;

    // Publish a test post to the "bikes" channel.
    let _post_hash = cable.post_text("bikes", "I got no brakes!").await?;

    // Generate a novel request ID.
    let (_req_id, req_id_bytes) = cable.new_req_id().await?;

    // Create a channel list request.
    let channel_list_req = Message::channel_list_request(CIRCUIT_ID, req_id_bytes, TTL, 0, 0);
    let req_bytes = channel_list_req.to_bytes()?;

    // Write the request bytes to the stream.
    stream.write_all(&req_bytes).await?;

    // Sleep briefly to allow time for the cable manager to respond.
    thread::sleep(five_millis);

    // Read the response from the stream.
    let _n = stream.read(&mut res_bytes).await?;

    // Ensure that a channel list response was returned by the listening peer.
    let (_bytes_len, msg) = Message::from_bytes(&res_bytes)?;
    assert_eq!(msg.message_type(), CHANNEL_LIST_RESPONSE);

    // Publish a second post to the "tao" channel.
    let post_hash = cable
        .post_text("tao", "Lie low to be on top, be on top by lying low.")
        .await?;

    // Sleep briefly to allow time for the cable manager to respond.
    thread::sleep(five_millis);

    // Read the response from the stream.
    let _n = stream.read(&mut res_bytes).await?;

    // Ensure that a hash response was returned by the listening peer.
    let (_bytes_len, msg) = Message::from_bytes(&res_bytes)?;
    assert_eq!(msg.message_type(), HASH_RESPONSE);

    if let MessageBody::Response { body } = msg.body {
        if let ResponseBody::Hash { hashes } = body {
            // Two post hashes should be returned (for channel "tao").
            assert_eq!(hashes.len(), 2);
            // Ensure the second returned hash matches the hash of the second
            // text post.
            assert_eq!(hashes[1], post_hash);

            // Generate a novel request ID.
            let (_req_id, req_id_bytes) = cable.new_req_id().await?;

            // Create a post request.
            let post_req = Message::post_request(CIRCUIT_ID, req_id_bytes, TTL, hashes);
            let req_bytes = post_req.to_bytes()?;

            // Write the request bytes to the stream.
            stream.write_all(&req_bytes).await?;

            // Sleep briefly to allow time for the cable manager to respond.
            thread::sleep(five_millis);

            // Read the response from the stream.
            let _n = stream.read(&mut res_bytes).await?;

            // Ensure that a post response was returned by the listening peer.
            let (_bytes_len, msg) = Message::from_bytes(&res_bytes)?;
            assert_eq!(msg.message_type(), POST_RESPONSE);

            if let MessageBody::Response { body } = msg.body {
                if let ResponseBody::Post { posts } = body {
                    // Two posts should be returned (for channel "tao").
                    assert_eq!(posts.len(), 2);
                }
            }
        }
    }

    Ok(())
}
