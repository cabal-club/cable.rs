//! Generate an example for each of the post and message types and print it to
//! `stdout`.
//!
//! A JSON-like representation is printed for each type, along with the encoded
//! bytes (aka. the over-the-wire format of the post or message).
//!
//! The output of this example can be used to test this implementation against
//! others.

use hex::FromHex;

use cable::{
    constants::NO_CIRCUIT, error::Error, message::Message, post::Post, ChannelOptions, Hash,
    UserInfo,
};
use desert::ToBytes;

const PUBLIC_KEY: &str = "aead820c67703da78dba364338d8b0d65d65c03a7e9310de87b2b36818ca1e5d";
const SECRET_KEY: &str = "5ff6fabec79407bde6402701d55be2b43a57adf3d4d454c8ad56c1397950f4a6aead820c67703da78dba364338d8b0d65d65c03a7e9310de87b2b36818ca1e5d";

// Blake2b 32 byte hashes.
//
// "Two hands clap and there is a sound. What is the sound of one hand?"
const HASH_1: &str = "fcd7c41883c3564c5a6abec78e214159efe62d50f124b4afafc184ea3b764cd4";
// "茶色"
const HASH_2: &str = "46b321c236880cd861dafae3040cf8cc52990516d1a69ab2c170b1e615a7ebd5";
// "elf"
const HASH_3: &str = "ffe809405a3e1eaf77938bde2138832b177a51e47df02935edc12aacf8279f61";

// Blake2b 32 byte hash: "love collapses spacetime".
const LINK: &str = "fea16c09f8aa581500fcf6ee2f6aabc59ccaa271d2a3568843930b7ff929ad86";
const TTL: u8 = 7;
const LIMIT: u64 = 21;
const TIMESTAMP: u64 = 9876543210;
const TIME_START: u64 = 0;
const TIME_END: u64 = 2033;
const FUTURE: u64 = 0;
const CHANNEL_1: &str = "myco";
const CHANNEL_2: &str = "bike_life";
const CHANNEL_3: &str = "qigong";
const USERNAME: &str = "ripple";
const SKIP: u64 = 0;
const TEXT: &str = "もしもし";
const TOPIC: &str = "tracklocross";
const REQ_ID: &str = "04baaffb";
const CANCEL_ID: &str = "31b5c9e1";
const CIRCUIT_ID: [u8; 4] = NO_CIRCUIT;

fn main() -> Result<(), Error> {
    let public_key = <[u8; 32]>::from_hex(PUBLIC_KEY)?;
    let secret_key = <[u8; 64]>::from_hex(SECRET_KEY)?;
    let hashes: Vec<Hash> = vec![
        <[u8; 32]>::from_hex(HASH_1)?,
        <[u8; 32]>::from_hex(HASH_2)?,
        <[u8; 32]>::from_hex(HASH_3)?,
    ];
    let links: Vec<Hash> = vec![<[u8; 32]>::from_hex(LINK)?];
    let req_id = <[u8; 4]>::from_hex(REQ_ID)?;
    let cancel_id = <[u8; 4]>::from_hex(CANCEL_ID)?;
    let channels = vec![
        CHANNEL_1.to_string(),
        CHANNEL_2.to_string(),
        CHANNEL_3.to_string(),
    ];
    let channel_opts = ChannelOptions::new(CHANNEL_3, TIME_START, TIME_END, LIMIT);
    let info = vec![UserInfo::new("name", USERNAME)];

    /* POSTS */

    let mut text_post = Post::text(
        public_key,
        links.clone(),
        TIMESTAMP,
        CHANNEL_2.to_string(),
        TEXT.to_string(),
    );
    text_post.sign(&secret_key)?;
    let text_post_bytes = text_post.to_bytes()?;
    println!("text post: {}", text_post);
    println!(
        "text post binary: {:?}",
        hex::encode(text_post_bytes.clone())
    );
    println!();

    let mut delete_post = Post::delete(public_key, links.clone(), TIMESTAMP, hashes.clone());
    delete_post.sign(&secret_key)?;
    let delete_post_bytes = delete_post.to_bytes()?;
    println!("delete post: {}", delete_post);
    println!("delete post binary: {:?}", hex::encode(delete_post_bytes));
    println!();

    let mut info_post = Post::info(public_key, links.clone(), TIMESTAMP, info);
    info_post.sign(&secret_key)?;
    let info_post_bytes = info_post.to_bytes()?;
    println!("info post: {}", info_post);
    println!("info post binary: {:?}", hex::encode(info_post_bytes));
    println!();

    let mut topic_post = Post::topic(
        public_key,
        links.clone(),
        TIMESTAMP,
        CHANNEL_2.to_string(),
        TOPIC.to_string(),
    );
    topic_post.sign(&secret_key)?;
    let topic_post_bytes = topic_post.to_bytes()?;
    println!("topic post: {}", topic_post);
    println!("topic post binary: {:?}", hex::encode(topic_post_bytes));
    println!();

    let mut join_post = Post::join(public_key, links.clone(), TIMESTAMP, CHANNEL_3.to_string());
    join_post.sign(&secret_key)?;
    let join_post_bytes = join_post.to_bytes()?;
    println!("join post: {}", join_post);
    println!("join post binary: {:?}", hex::encode(join_post_bytes));
    println!();

    let mut leave_post = Post::leave(public_key, links, TIMESTAMP, CHANNEL_3.to_string());
    leave_post.sign(&secret_key)?;
    let leave_post_bytes = leave_post.to_bytes()?;
    println!("leave post: {}", leave_post);
    println!("leave post binary: {:?}", hex::encode(leave_post_bytes));
    println!();

    /* REQUESTS */

    let post_request = Message::post_request(CIRCUIT_ID, req_id, TTL, hashes.clone());
    let post_request_bytes = post_request.to_bytes()?;
    println!("post request: {}", post_request);
    println!("post request binary: {:?}", hex::encode(post_request_bytes));
    println!();

    let cancel_request = Message::cancel_request(CIRCUIT_ID, req_id, TTL, cancel_id);
    let cancel_request_bytes = cancel_request.to_bytes()?;
    println!("cancel request: {}", cancel_request);
    println!(
        "cancel request binary: {:?}",
        hex::encode(cancel_request_bytes)
    );
    println!();

    let channel_time_range_request =
        Message::channel_time_range_request(CIRCUIT_ID, req_id, TTL, channel_opts);
    let channel_time_range_request_bytes = channel_time_range_request.to_bytes()?;
    println!("channel time range request: {}", channel_time_range_request);
    println!(
        "channel time range request binary: {:?}",
        hex::encode(channel_time_range_request_bytes)
    );
    println!();

    let channel_state_request =
        Message::channel_state_request(CIRCUIT_ID, req_id, TTL, CHANNEL_2.to_string(), FUTURE);
    let channel_state_request_bytes = channel_state_request.to_bytes()?;
    println!("channel state request: {}", channel_state_request);
    println!(
        "channel state request binary: {:?}",
        hex::encode(channel_state_request_bytes)
    );
    println!();

    let channel_list_request = Message::channel_list_request(CIRCUIT_ID, req_id, TTL, SKIP, LIMIT);
    let channel_list_request_bytes = channel_list_request.to_bytes()?;
    println!("channel list request: {}", channel_list_request);
    println!(
        "channel list request binary: {:?}",
        hex::encode(channel_list_request_bytes)
    );
    println!();

    /* RESPONSES */

    let hash_response = Message::hash_response(CIRCUIT_ID, req_id, hashes);
    let hash_response_bytes = hash_response.to_bytes()?;
    println!("hash response: {}", hash_response);
    println!(
        "hash response binary: {:?}",
        hex::encode(hash_response_bytes)
    );
    println!();

    let encoded_posts = vec![text_post_bytes];
    let post_response = Message::post_response(CIRCUIT_ID, req_id, encoded_posts);
    let post_response_bytes = post_response.to_bytes()?;
    println!("post response: {}", post_response);
    println!(
        "post response binary: {:?}",
        hex::encode(post_response_bytes)
    );
    println!();

    let channel_list_response = Message::channel_list_response(CIRCUIT_ID, req_id, channels);
    let channel_list_response_bytes = channel_list_response.to_bytes()?;
    println!("channel list response: {}", channel_list_response);
    println!(
        "channel list response binary: {:?}",
        hex::encode(channel_list_response_bytes)
    );

    Ok(())
}
