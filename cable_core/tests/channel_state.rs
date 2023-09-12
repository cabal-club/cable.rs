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
//! 2) Send a channel state request for the "entomology" channel with `future`
//! set to 1 (requesting ongoing hash responses for each change of the channel
//! state). Ensure that a single hash is returned and that it matches the hash
//! of the join post.
//!
//! 3) Publish a topic post to the "entomology" channel.
//!
//! 4) Ensure that two hashes are returned: one matching the hash of the join
//! post and one matching the hash of the topic post.
//!
//! 5) Publish a second topic post to the "entomology" channel.
//!
//! 6) Ensure that two hashes are returned: one matching the hash of the join
//! post and one matching the hash of the second topic post.
//!
//! 7) Publish a delete post with the hash of the second topic post.
//!
//! 8) Ensure that three hashes are returned: one matching the hash of the join
//! post, one matching the hash of the first topic post and one matching the
//! hash of the delete topic post.
//!
//! 9) Publish an info post with the nickname "glyph".
//!
//! 10) Ensure that four hashes are returned: one matching the hash of the join
//! post, one matching the hash of the first topic post, one matching the hash
//! of the delete topic post and one matching the hash of the first name post.
//!
//! 11) Publish a second info post with the nickname "mycognosist".
//!
//! 12) Ensure that four hashes are returned: one matching the hash of the join
//! post, one matching the hash of the first topic post, one matching the hash
//! of the delete topic post and one matching the hash of the second name post.
//!
//! 13) Publish a leave post to the "entomology" channel.
//!
//! 10) Ensure that three hashes are returned: one matching the hash of the
//! leave post, one matching the hash of the first topic post and one matching
//! the hash of the second name post.

use std::{thread, time::Duration};

use async_std::{
    net::{TcpListener, TcpStream},
    stream::StreamExt,
    task,
};
use cable::{
    constants::{HASH_RESPONSE, NO_CIRCUIT},
    message::{MessageBody, ResponseBody},
    Error, Message,
};
use desert::{FromBytes, ToBytes};
use futures::{AsyncReadExt, AsyncWriteExt};
use log::{debug, info};

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

    /* FIRST REQUEST */

    let channel = "entomology".to_string();

    // Publish a post to join the "entomology" channel.
    let join_post_hash = cable.post_join(&channel).await?;
    debug!("Published join post for {} channel", &channel);

    // Generate a novel request ID.
    let (_req_id, req_id_bytes) = cable.new_req_id().await?;

    // Create a channel state request.
    //
    // Future is set to 1; we will automatically receive hash responses when
    // the "entomology" channel state changes.
    let channel_state_req =
        Message::channel_state_request(CIRCUIT_ID, req_id_bytes, TTL, channel.clone(), 1);
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

    /* SECOND RESPONSE */

    let first_topic = "Insect appreciation and identification assistance".to_string();

    // Publish a post to set the topic for the "entomology" channel.
    let first_topic_hash = cable.post_topic(&channel, &first_topic).await?;
    debug!("Published first topic post for {} channel", &channel);

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
            assert_eq!(hashes[0], join_post_hash);
            // Ensure the second hash matches the hash of the first topic post.
            assert_eq!(hashes[1], first_topic_hash);
        }
    }

    // Sleep briefly to ensure that the second topic post has a timestamp
    // larger than the first.
    let one_second = Duration::from_millis(1000);
    thread::sleep(one_second);

    /* THIRD RESPONSE */

    let second_topic =
        "Insect appreciation; please don't ask for identification assistance".to_string();
    debug!("Published second topic post for {} channel", &channel);

    // Publish a post to (re)set the topic for the "entomology" channel.
    let second_topic_hash = cable.post_topic(&channel, &second_topic).await?;

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
            // Ensure the first hash matches the hash of the recent join post.
            assert_eq!(hashes[0], join_post_hash);
            // Ensure the second hash matches the hash of the second topic post.
            assert_eq!(hashes[1], second_topic_hash);
        }
    }

    /* FOURTH RESPONSE */

    // Sleep briefly to allow time for the cable manager to respond.
    let five_millis = Duration::from_millis(500);
    thread::sleep(five_millis);

    // Delete the second (most recent) topic post for the "entomology" channel.
    let delete_topic_hash = cable.post_delete(vec![second_topic_hash]).await?;
    debug!(
        "Published delete post for second topic post to {} channel",
        &channel
    );

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
            // Three post hashes should be returned (one for join / leave, one
            // for the latest topic and one for the recent delete post).
            assert_eq!(hashes.len(), 3);
            // Ensure the first hash matches the hash of the recent join post.
            assert_eq!(hashes[0], join_post_hash);
            // Ensure the second hash matches the hash of the first topic post.
            assert_eq!(hashes[1], first_topic_hash);
            // Ensure the third hash matches the hash of the recent delete
            // post.
            assert_eq!(hashes[2], delete_topic_hash);
        }
    }

    /* FIFTH RESPONSE */

    let nickname = "glyph";

    // Publish a post to set a nickname.
    let first_name_hash = cable.post_info_name(nickname).await?;
    debug!("Published info post for name {:?}", nickname);

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
            // Five post hashes should be returned.
            assert_eq!(hashes.len(), 4);
            // Ensure the returned hash matches the hash of the recent
            // join post.
            assert_eq!(hashes[0], join_post_hash);
            // Ensure the second hash matches the hash of the first topic post.
            assert_eq!(hashes[1], first_topic_hash);
            // Ensure the third hash matches the hash of the recent delete
            // post.
            assert_eq!(hashes[2], delete_topic_hash);
            // Ensure the fourth hash matches the hash of the first name info
            // post.
            assert_eq!(hashes[3], first_name_hash);
        }
    }

    /* SIXTH RESPONSE */

    let second_nickname = "mycognosist";

    // Publish a post to (re)set a nickname.
    let second_name_hash = cable.post_info_name(second_nickname).await?;
    debug!("Published info post for name {:?}", second_nickname);

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
            // Four post hashes should be returned.
            assert_eq!(hashes.len(), 4);
            // Ensure the returned hash matches the hash of the recent
            // join post.
            assert_eq!(hashes[0], join_post_hash);
            // Ensure the second hash matches the hash of the first topic post.
            assert_eq!(hashes[1], first_topic_hash);
            // Ensure the third hash matches the hash of the recent delete
            // post.
            assert_eq!(hashes[2], delete_topic_hash);
            // Ensure the fourth hash matches the hash of the second name info
            // post.
            assert_eq!(hashes[3], second_name_hash);
        }
    }

    /* SEVENTH RESPONSE */

    // Publish a post to leave the "entomology" channel.
    let leave_post_hash = cable.post_leave(&channel).await?;
    debug!("Published leave post for {} channel", &channel);

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
            assert_eq!(hashes.len(), 3);
            // Ensure the returned hash matches the hash of the recent
            // leave post.
            assert_eq!(hashes[0], leave_post_hash);
            // Ensure the second hash matches the hash of the first topic post.
            assert_eq!(hashes[1], first_topic_hash);
            // Ensure the third hash matches the hash of the second name info
            // post.
            assert_eq!(hashes[2], second_name_hash);
        }
    }

    Ok(())
}
