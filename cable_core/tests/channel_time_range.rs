//! Test the various channel time range options.
//!
//! Create a cable manager by creating a TCP stream, invoke the cable listener
//! and then write and read requests and responses to and from the stream.
//!
//! Run the test with debug logging enabled in a terminal:
//!
//! `RUST_LOG=debug cargo test channel_time_range`
//!
//! An outline of the actions taken in this test:
//!
//! 1) Publish three posts to the "myco" channel.
//!
//! 2) Send a channel time range request for the "myco" channel with a start
//! and end time of `now()`. Ensure that no hashes are returned in the
//! response.
//!
//! 3) Send a channel time range request for the "myco" channel with a start
//! time before the three posts were published and an end time of `now()`.
//! Ensure that all three hashes are returned in the response.
//!
//! 4) Send a channel time range request for the "myco" channel with a start
//! time before the three posts were published, an end time of `now()` and a
//! limit of 2. Ensure that only two hashes are returned in the response.
//!
//! 5) Publish a post to the "books" channel.
//!
//! 6) Send a channel time range request for the "books" channel with a start
//! time before the post was published and an end time of 0. Ensure that
//! a single hash is returned in the response.
//!
//! 7) Publish a second post to the "books" channel.
//!
//! 8) Ensure that a hash response is received with two hashes.

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
async fn request_response() -> Result<(), Error> {
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

    // Create a timestamp for later use.
    let time_before_posts_were_published = now();

    // Publish a post to the "myco" channel.
    let _post_hash_1 = cable
        .post_text(
            "myco",
            "Listen, if you're a mushroom, you live cheap; besides, I'm telling you, this was a very nice neighborhood until the monkeys got out of control.",
        )
        .await?;

    // Publish a second post to the "myco" channel.
    let _post_hash_2 = cable.post_text("myco", "Ganoderma neo-japonicum").await?;

    // Publish a third post to the "myco" channel.
    let _post_hash_3 = cable.post_text("myco", "hyphal fusion").await?;

    /* FIRST REQUEST */

    // Generate a novel request ID.
    let (_req_id, req_id_bytes) = cable.new_req_id().await?;

    // Channel time range request parameters.
    //
    // These parameters should result in zero post hashes being returned, due
    // to the fact that the three posts were published before the given
    // `time_start` parameter.
    let opts = ChannelOptions::new("myco", now(), now(), 10);

    // Create a channel time range request.
    let channel_time_range_req =
        Message::channel_time_range_request(CIRCUIT_ID, req_id_bytes, TTL, opts);
    let req_bytes = channel_time_range_req.to_bytes()?;

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
            // No post hashes should be returned.
            assert_eq!(hashes.len(), 0);
        }
    }

    /* SECOND REQUEST */

    // Generate a novel request ID.
    let (_req_id, req_id_bytes) = cable.new_req_id().await?;

    // Sleep briefly to allow time to elapse since the posts were published.
    let one_second = Duration::from_millis(1000);
    thread::sleep(one_second);

    // Channel time range request parameters.
    //
    // These parameters should result in three post hashes being returned, due
    // to the fact that three posts were published after the given `time_start`
    // parameter.
    let opts = ChannelOptions::new("myco", time_before_posts_were_published, now(), 5);

    // Create a channel time range request.
    let channel_time_range_req =
        Message::channel_time_range_request(CIRCUIT_ID, req_id_bytes, TTL, opts);
    let req_bytes = channel_time_range_req.to_bytes()?;

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
            // Three post hashes should be returned.
            assert_eq!(hashes.len(), 3);
        }
    }

    /* THIRD REQUEST */

    // Generate a novel request ID.
    let (_req_id, req_id_bytes) = cable.new_req_id().await?;

    // Channel time range request parameters.
    //
    // These parameters should result in two post hashes being returned, due
    // to the fact that three posts were published after the given `time_start`
    // parameter but the `limit` parameter is set to 2.
    let opts = ChannelOptions::new("myco", time_before_posts_were_published, now(), 2);

    // Create a channel time range request.
    let channel_time_range_req =
        Message::channel_time_range_request(CIRCUIT_ID, req_id_bytes, TTL, opts);
    let req_bytes = channel_time_range_req.to_bytes()?;

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
            // Two post hashes should be returned.
            assert_eq!(hashes.len(), 2);
        }
    }

    /* FOURTH REQUEST */

    // Publish a post to the "books" channel.
    let _post_hash_1 = cable
        .post_text("books", "Just started reading 'Strange Weather in Tokyo'")
        .await?;

    // Generate a novel request ID.
    let (_req_id, req_id_bytes) = cable.new_req_id().await?;

    // Channel time range request parameters.
    //
    // These parameters should result in a single post hash being returned, due
    // to the fact that only one post was published after the given `time_start`
    // parameter.
    //
    // The request should also result in additional responses being sent for
    // every post subsequently published to the "books" channel, due to the
    // fact that the `time_end` parameter is set to 0 (keep the request alive
    // and continue to send new posts as they're published).
    let opts = ChannelOptions::new("books", time_before_posts_were_published, 0, 10);

    // Create a channel time range request.
    let channel_time_range_req =
        Message::channel_time_range_request(CIRCUIT_ID, req_id_bytes, TTL, opts);
    let req_bytes = channel_time_range_req.to_bytes()?;

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
            // One post hash should be returned.
            assert_eq!(hashes.len(), 1);
        }
    }

    // Publish a second post to the "books" channel.
    let _post_hash_2 = cable
        .post_text("books", "Have you set any reading goals for the year?")
        .await?;

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
            // Two post hashes should be returned.
            assert_eq!(hashes.len(), 2);
        }
    }

    Ok(())
}
