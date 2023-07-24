//! Generate an example for each of the request and response type messages.
//!
//! The output of this example can be used to test this implementation against
//! others.

use hex::FromHex;

use cable::{constants::NO_CIRCUIT, error::Error, message::Message, ChannelOptions, Hash, Payload};
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

    /* REQUESTS */

    let post_request = Message::post_request(CIRCUIT_ID, req_id, TTL, hashes.clone());
    let post_request_bytes = post_request.to_bytes()?;
    println!("post request: {}", post_request);
    println!("post request binary: {}", hex::encode(post_request_bytes));

    let cancel_request = Message::cancel_request(CIRCUIT_ID, req_id, TTL, cancel_id);
    println!("cancel request: {}", cancel_request);

    let channel_time_range_request =
        Message::channel_time_range_request(CIRCUIT_ID, req_id, TTL, channel_opts);
    println!("channel time range request: {}", channel_time_range_request);

    let channel_state_request =
        Message::channel_state_request(CIRCUIT_ID, req_id, TTL, CHANNEL_2.to_string(), FUTURE);
    println!("channel state request: {}", channel_state_request);

    let channel_list_request = Message::channel_list_request(CIRCUIT_ID, req_id, TTL, SKIP, LIMIT);
    println!("channel list request: {}", channel_list_request);

    /* RESPONSES */

    let hash_response = Message::hash_response(CIRCUIT_ID, req_id, hashes);
    println!("hash response: {}", hash_response);

    // TODO: Change `hashes` to `posts` once posts have been created.
    //let post_response = Message::post_response(CIRCUIT_ID, req_id, hashes);
    //println!("post response: {}", post_response);

    let channel_list_response = Message::channel_list_response(CIRCUIT_ID, req_id, channels);
    println!("channel list response: {:?}", channel_list_response);

    Ok(())
}
