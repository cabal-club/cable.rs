// SPDX-FileCopyrightText: 2024 the cabal-club authors
//
// SPDX-License-Identifier: LGPL-3.0-or-later

use std::{
    error::Error,
    fmt::{Display, Formatter, Result},
};

#[derive(Debug, PartialEq)]
/// Error that can occur during the handshake.
pub enum HandshakeError {
    /// The received major server version does not match that of the client.
    IncompatibleServerVersion { received: u8, expected: u8 },
}

impl Error for HandshakeError {}

impl Display for HandshakeError {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            HandshakeError::IncompatibleServerVersion { received, expected } => {
                write!(
                    f,
                    "Received server major version `{}` does not match client major version `{}`",
                    received, expected
                )
            }
        }
    }
}
