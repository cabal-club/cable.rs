// Vendored from kuska-handshake 0.2.0
//
// The original source file from which this is derived is
// Copyright (c) 2020 Dhole and Adria Massanet
//
// and released under the AGPL3 license:
// https://docs.rs/crate/kuska-handshake/0.2.0/source/LICENSE

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

/*
macro_rules! concat {
    ( $n:expr, $( $x:expr ),* ) => {
        {
            let mut dst = [0; $n];
            concat_into!(dst, $( $x ),*);
            dst
        }
    };
}

// Helper struct to easily append u8 slices into another slice.
pub struct Buffer<'a> {
    buf: &'a mut [u8],
    n: usize,
}

impl<'a> Buffer<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Buffer { buf, n: 0 }
    }

    pub fn append(&mut self, src: &[u8]) {
        self.buf[self.n..src.len()].copy_from_slice(src);
        self.n += src.len();
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn capacity(&self) -> usize {
        self.buf.len()
    }
}
*/
