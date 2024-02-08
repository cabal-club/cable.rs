// SPDX-FileCopyrightText: 2024 the cabal-club authors
//
// SPDX-License-Identifier: LGPL-3.0-or-later

//! Constant values used during handshake operations.

/// Size of the ephemeral key.
pub const EPHEMERAL_KEY_BYTES_LEN: usize = 48;

/// Size of the ephemeral and static keys.
pub const EPHEMERAL_AND_STATIC_KEY_BYTES_LEN: usize = 96;

/// Size of the public key received via the static key exchange.
pub const PUBLIC_KEY_BYTES_LEN: usize = 32;

/// Size of the static key.
///
/// In the context of the Cable Handshake implementation, this value is the
/// private key of the author keypair.
pub const STATIC_KEY_BYTES_LEN: usize = 64;

/// Size of the version message.
pub const VERSION_BYTES_LEN: usize = 2;
