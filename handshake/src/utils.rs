// SPDX-FileCopyrightText: 2020 Dhole and Adria Massanet
//
// SPDX-License-Identifier: AGPL-3.0-only
//
// Vendored from kuska-handshake 0.2.0.

/// Helper macro to append one buffer to another.
macro_rules! concat_into {
    ( $dst:expr, $( $x:expr ),* ) => {
        {
            let mut n = 0;
            $(
                n += $x.len();
                $dst[n - $x.len()..n].copy_from_slice($x);
            )*
            $dst
        }
    };
}
