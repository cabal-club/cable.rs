// SPDX-FileCopyrightText: 2024 the cabal-club authors
//
// SPDX-License-Identifier: LGPL-3.0-or-later

//! Benchmark of a synchronous handshake over a Unix socket.

use std::{
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
    thread,
};

use cable_handshake::{sync::handshake, Result, Version};
use criterion::{criterion_group, criterion_main, Criterion};
use snow::Builder as NoiseBuilder;

const SOCKET_PATH: &str = "/tmp/handshake.sock";

fn setup() -> Result<(Version, Version, [u8; 32], Vec<u8>, Vec<u8>)> {
    let client_version = Version::init(1, 0);
    let server_version = Version::init(1, 0);

    let psk: [u8; 32] = [1; 32];

    let builder = NoiseBuilder::new("Noise_XXpsk0_25519_ChaChaPoly_BLAKE2b".parse()?);
    let client_keypair = builder.generate_keypair()?;
    let server_keypair = builder.generate_keypair()?;
    let client_private_key = client_keypair.private;
    let server_private_key = server_keypair.private;

    Ok((
        client_version,
        server_version,
        psk,
        client_private_key,
        server_private_key,
    ))
}

fn handshake(
    client_version: Version,
    server_version: Version,
    psk: [u8; 32],
    client_private_key: Vec<u8>,
    server_private_key: Vec<u8>,
) -> Result<()> {
    // Deploy a Unix socket listener.
    let listener = UnixListener::bind(SOCKET_PATH)?;

    thread::spawn(move || {
        // Accept connection.
        if let Ok((mut stream, _addr)) = listener.accept() {
            let _encrypted =
                handshake::server(&mut stream, server_version, psk, server_private_key).unwrap();
        }
    });

    let mut stream = UnixStream::connect(SOCKET_PATH)?;

    let _encrypted = handshake::client(&mut stream, client_version, psk, client_private_key)?;

    Ok(())
}

fn cleanup() {
    // Remove the socket file so that the address will be bound successfully
    // the next time this benchmark runs.
    if Path::new(SOCKET_PATH).exists() {
        std::fs::remove_file(SOCKET_PATH).unwrap()
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let (client_version, server_version, psk, client_private_key, server_private_key) =
        setup().unwrap();

    c.bench_function("sync handshake", |b| {
        b.iter(|| {
            handshake(
                client_version.clone(),
                server_version.clone(),
                psk.clone(),
                client_private_key.clone(),
                server_private_key.clone(),
            )
        })
    });

    cleanup();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
