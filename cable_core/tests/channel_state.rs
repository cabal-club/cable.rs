//! Test channel state request / response.
//!
//! Create a cable manager by creating a TCP stream, invoke the cable listener
//! and then write and read requests and responses to and from the stream.
//!
//! Run the test with debug logging enabled in a terminal:
//!
//! `RUST_LOG=debug cargo test channel_state_request_response`
//!
//! An outline of the actions taken in this test:
//!
//! 1) Publish a join post to the "entomology" channel.
//!
//! 2) Send a channel state request for the "entomology" channel. Ensure that
//! a single hash is returned and that it matches the hash of the join post.

use std::{thread, time::Duration};

use async_std::{
    net::{TcpListener, TcpStream},
    stream::StreamExt,
    task,
};
use cable::{
    constants::{HASH_RESPONSE, NO_CIRCUIT},
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
async fn channel_state_request_response() -> Result<(), Error> {
    init();

    // Create a store and a cable manager.
    let store = MemoryStore::default();
    let mut cable = CableManager::new(store);
    let cable_clone = cable.clone();

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

    let channel = "entomology".to_string();

    // Publish a post to join the "entomology" channel.
    let join_post_hash = cable.post_join(&channel).await?;

    /* FIRST REQUEST */

    // Generate a novel request ID.
    let (_req_id, req_id_bytes) = cable.new_req_id().await?;

    // Create a channel state request.
    let channel_state_req =
        Message::channel_state_request(CIRCUIT_ID, req_id_bytes, TTL, channel.clone(), 0);
    let req_bytes = channel_state_req.to_bytes()?;

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
            // A single post hash should be returned.
            assert_eq!(hashes.len(), 1);
            // Ensure the returned hash matches the hash of the original
            // join post.
            assert_eq!(hashes[0], join_post_hash);
        }
    }

    // Publish a post to leave the "entomology" channel.
    let leave_post_hash = cable.post_leave(&channel).await?;

    /* SECOND REQUEST */

    // Generate a novel request ID.
    let (_req_id, req_id_bytes) = cable.new_req_id().await?;

    // Create a channel state request.
    let channel_state_req =
        Message::channel_state_request(CIRCUIT_ID, req_id_bytes, TTL, channel.clone(), 0);
    let req_bytes = channel_state_req.to_bytes()?;

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
            // A single post hash should be returned.
            assert_eq!(hashes.len(), 1);
            // Ensure the returned hash matches the hash of the recent
            // leave post.
            assert_eq!(hashes[0], leave_post_hash);
        }
    }

    // Set topic for channel.
    // Get hash of channel topic.
    // Update topic for channel.
    // Get hash of channel topic.
    // Delete latest topic for channel.
    // Get hash of channel topic (must be the first topic).

    let first_topic = "Insect appreciation and identification assistance".to_string();

    // Publish a post to set the topic for the "entomology" channel.
    let first_topic_hash = cable.post_topic(&channel, &first_topic).await?;

    /* THIRD REQUEST */

    // Generate a novel request ID.
    let (_req_id, req_id_bytes) = cable.new_req_id().await?;

    // Create a channel state request.
    let channel_state_req =
        Message::channel_state_request(CIRCUIT_ID, req_id_bytes, TTL, channel.clone(), 0);
    let req_bytes = channel_state_req.to_bytes()?;

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
            // Two post hashes should be returned (one for join / leave and one
            // for the latest topic).
            assert_eq!(hashes.len(), 2);
            // Ensure the first hash matches the hash of the recent leave post.
            assert_eq!(hashes[0], leave_post_hash);
            // Ensure the second hash matches the hash of the first topic post.
            assert_eq!(hashes[1], first_topic_hash);
        }
    }

    let second_topic =
        "Insect appreciation; please don't ask for identification assistance".to_string();

    // Publish a post to (re)set the topic for the "entomology" channel.
    let second_topic_hash = cable.post_topic(&channel, &second_topic).await?;

    /* FOURTH REQUEST */

    // Generate a novel request ID.
    let (_req_id, req_id_bytes) = cable.new_req_id().await?;

    // Create a channel state request.
    let channel_state_req =
        Message::channel_state_request(CIRCUIT_ID, req_id_bytes, TTL, channel.clone(), 0);
    let req_bytes = channel_state_req.to_bytes()?;

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
            // Two post hashes should be returned (one for join / leave and one
            // for the latest topic).
            assert_eq!(hashes.len(), 2);
            // Ensure the first hash matches the hash of the recent leave post.
            assert_eq!(hashes[0], leave_post_hash);
            // Ensure the second hash matches the hash of the second topic post.
            assert_eq!(hashes[1], second_topic_hash);
        }
    }

    // Delete the second (most recent) topic post for the "entomology" channel.
    let _delete_topic_hash = cable.post_delete(vec![second_topic_hash]).await?;

    /* FOURTH REQUEST */

    // Generate a novel request ID.
    let (_req_id, req_id_bytes) = cable.new_req_id().await?;

    // Create a channel state request.
    let channel_state_req =
        Message::channel_state_request(CIRCUIT_ID, req_id_bytes, TTL, channel.clone(), 0);
    let req_bytes = channel_state_req.to_bytes()?;

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
            // Two post hashes should be returned (one for join / leave and one
            // for the latest topic).
            assert_eq!(hashes.len(), 2);
            // Ensure the first hash matches the hash of the recent leave post.
            assert_eq!(hashes[0], leave_post_hash);
            // Ensure the second hash matches the hash of the first topic post.
            assert_eq!(hashes[1], first_topic_hash);
        }
    }

    Ok(())
}
