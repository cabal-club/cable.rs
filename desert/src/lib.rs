// Vendored version of desert 2.0.1
//
// The original source files from which this is derived is
// Copyright (c) 2019 James Halliday
//
// and released under the MIT or APACHE-2.0 license:
// https://docs.rs/crate/desert/2.0.1/source/LICENSE-MIT
// https://www.apache.org/licenses/LICENSE-2.0.txt

#![cfg_attr(feature = "nightly-features", feature(backtrace))]
#![doc=include_str!("../README.md")]

pub mod error;
pub mod varint;

pub use error::*;

/// Serialize a type into a sequence of bytes with unspecified endianness.
/// The implementations for the built-in types are in big endian for this trait.
pub trait ToBytes {
    /// Serialize into a newly-allocated byte vector.
    fn to_bytes(&self) -> Result<Vec<u8>, Error>;

    /// Serialize into an existing mutable byte slice.
    /// The usize Result contains how many bytes were written to `dst`.
    fn write_bytes(&self, dst: &mut [u8]) -> Result<usize, Error> {
        let bytes = self.to_bytes()?;
        if dst.len() < bytes.len() {
            return DesertErrorKind::DstInsufficient {
                provided: dst.len(),
                required: bytes.len(),
            }
            .raise();
        }
        dst[0..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
}

/// Serialize a type into a sequence of bytes in big endian.
pub trait ToBytesBE {
    /// Serialize into a newly-allocated byte vector in big endian.
    fn to_bytes_be(&self) -> Result<Vec<u8>, Error>;

    /// Serialize into an existing mutable byte slice in big endian.
    /// The usize Result contains how many bytes were written to `dst`.
    fn write_bytes_be(&self, dst: &mut [u8]) -> Result<usize, Error> {
        let bytes = self.to_bytes_be()?;
        if dst.len() < bytes.len() {
            return DesertErrorKind::DstInsufficient {
                provided: dst.len(),
                required: bytes.len(),
            }
            .raise();
        }
        dst[0..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
}

/// Serialize a type into a sequence of bytes in little endian.
pub trait ToBytesLE {
    /// Serialize into a newly-allocated byte vector in little endian.
    fn to_bytes_le(&self) -> Result<Vec<u8>, Error>;

    /// Serialize into an existing mutable byte slice in little endian.
    /// The usize Result contains how many bytes were written to `dst`.
    fn write_bytes_le(&self, dst: &mut [u8]) -> Result<usize, Error> {
        let bytes = self.to_bytes_le()?;
        if dst.len() < bytes.len() {
            return DesertErrorKind::DstInsufficient {
                provided: dst.len(),
                required: bytes.len(),
            }
            .raise();
        }
        dst[0..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
}

/// Deserialize a sequence of bytes into a type.
/// The implementations for the built-in types are in big endian for this trait.
pub trait FromBytes: Sized {
    /// Read data from `src` in order to create an instance `Self`.
    /// The `usize` in the `Result` is the number of bytes read from `src`.
    fn from_bytes(src: &[u8]) -> Result<(usize, Self), Error>;
}

/// Deserialize a sequence of bytes into a type in big endian.
pub trait FromBytesBE: Sized {
    /// Read data from `src` in order to create an instance `Self` in big endian.
    /// The `usize` in the `Result` is the number of bytes read from `src`.
    fn from_bytes_be(src: &[u8]) -> Result<(usize, Self), Error>;
}

/// Deserialize a sequence of bytes into a type in little endian.
pub trait FromBytesLE: Sized {
    /// Read data from `src` in order to create an instance `Self` in little
    /// endian.
    /// The `usize` in the `Result` is the number of bytes read from `src`.
    fn from_bytes_le(src: &[u8]) -> Result<(usize, Self), Error>;
}

/// Count how many bytes to read from a byte slice for a type and calculate how
/// many bytes the serialization would contain.
/// The implementations for the built-in types are in big endian for this trait.
pub trait CountBytes {
    /// Return how many bytes from `buf` would be required to deserialize Self.
    fn count_from_bytes(buf: &[u8]) -> Result<usize, Error>;

    /// Return how many bytes from `buf` would be required to deserialize Self,
    /// where if there are not enough bytes in `buf` to know how many bytes would
    /// be required, you will receive `None` or otherwise `Some(nbytes)`.
    fn count_from_bytes_more(buf: &[u8]) -> Result<Option<usize>, Error> {
        Ok(Some(Self::count_from_bytes(buf)?))
    }

    /// Return the number of bytes that the serialization would require.
    fn count_bytes(&self) -> usize;
}

/// Count how many bytes to read from a byte slice for a type and calculate how
/// many bytes the serialization would contain. In big endian.
pub trait CountBytesBE {
    /// Return how many bytes from `buf` would be required to deserialize Self
    /// in big endian.
    fn count_from_bytes_be(buf: &[u8]) -> Result<usize, Error>;

    /// Return how many bytes from `buf` would be required to deserialize Self
    /// in big endian, where if there are not enough bytes in `buf` to know how
    /// many bytes would be required, you will receive `None` or otherwise
    /// `Some(nbytes)`.
    fn count_from_bytes_be_more(buf: &[u8]) -> Result<Option<usize>, Error> {
        Ok(Some(Self::count_from_bytes_be(buf)?))
    }

    /// Return the number of bytes that the serialization would require.
    fn count_bytes_be(&self) -> usize;
}

/// Count how many bytes to read from a byte slice for a type and calculate how
/// many bytes the serialization would contain. In little endian.
pub trait CountBytesLE {
    /// Return how many bytes from `buf` would be required to deserialize Self
    /// in little endian.
    fn count_from_bytes_le(buf: &[u8]) -> Result<usize, Error>;

    /// Return how many bytes from `buf` would be required to deserialize Self
    /// in little endian, where if there are not enough bytes in `buf` to know how
    /// many bytes would be required, you will receive `None` or otherwise
    /// `Some(nbytes)`.
    fn count_from_bytes_le_more(buf: &[u8]) -> Result<Option<usize>, Error> {
        Ok(Some(Self::count_from_bytes_le(buf)?))
    }

    /// Return the number of bytes that the serialization would require.
    fn count_bytes_le(&self) -> usize;
}

macro_rules! buf_array {
    ($b:tt,1) => {
        [$b[0]]
    };
    ($b:tt,2) => {
        [$b[0], $b[1]]
    };
    ($b:tt,4) => {
        [$b[0], $b[1], $b[2], $b[3]]
    };
    ($b:tt,8) => {
        [$b[0], $b[1], $b[2], $b[3], $b[4], $b[5], $b[6], $b[7]]
    };
    ($b:tt,16) => {
        [
            $b[0], $b[1], $b[2], $b[3], $b[4], $b[5], $b[6], $b[7], $b[8], $b[9], $b[10], $b[11],
            $b[12], $b[13], $b[14], $b[15],
        ]
    };
}

macro_rules! define_static_builtins {
  ($(($T:tt,$n:tt)),+) => {$(
    impl CountBytes for $T {
      fn count_from_bytes(_buf: &[u8]) -> Result<usize,Error> {
        Ok($n)
      }

      fn count_bytes(&self) -> usize { $n }
    }

    impl CountBytesBE for $T {
      fn count_from_bytes_be(_buf: &[u8]) -> Result<usize,Error> {
        Ok($n)
      }

      fn count_bytes_be(&self) -> usize { $n }
    }

    impl CountBytesLE for $T {
      fn count_from_bytes_le(_buf: &[u8]) -> Result<usize,Error> {
        Ok($n)
      }

      fn count_bytes_le(&self) -> usize { $n }
    }

    impl ToBytes for $T {
      fn to_bytes(&self) -> Result<Vec<u8>,Error> {
        self.to_bytes_be()
      }

      fn write_bytes(&self, dst: &mut [u8]) -> Result<usize,Error> {
        self.write_bytes_be(dst)
      }
    }

    impl ToBytesBE for $T {
      fn to_bytes_be(&self) -> Result<Vec<u8>,Error> {
        Ok(self.to_be_bytes().to_vec())
      }

      fn write_bytes_be(&self, dst: &mut [u8]) -> Result<usize,Error> {
        let bytes = self.to_be_bytes();
        if dst.len() < bytes.len() {
          return DesertErrorKind::DstInsufficient { provided: dst.len(), required: bytes.len() }.raise();
        }
        dst[0..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
      }
    }

    impl ToBytesLE for $T {
      fn to_bytes_le(&self) -> Result<Vec<u8>,Error> {
        Ok(self.to_le_bytes().to_vec())
      }

      fn write_bytes_le(&self, dst: &mut [u8]) -> Result<usize,Error> {
        let bytes = self.to_le_bytes();
        if dst.len() < bytes.len() {
          return DesertErrorKind::DstInsufficient { provided: dst.len(), required: bytes.len() }.raise();
        }
        dst[0..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
      }
    }

    impl FromBytes for $T {
      fn from_bytes(src: &[u8]) -> Result<(usize,Self),Error> {
        Self::from_bytes_be(src)
      }
    }

    impl FromBytesBE for $T {
      fn from_bytes_be(src: &[u8]) -> Result<(usize,Self),Error> {
        if src.len() < $n {
          return DesertErrorKind::SrcInsufficient { provided: src.len(), required: $n }.raise();
        } else {
          Ok(($n,$T::from_be_bytes(buf_array![src,$n])))
        }
      }
    }

    impl FromBytesLE for $T {
      fn from_bytes_le(src: &[u8]) -> Result<(usize,Self),Error> {
        if src.len() < $n {
          return DesertErrorKind::SrcInsufficient { provided: src.len(), required: $n }.raise();
        } else {
          Ok(($n,$T::from_le_bytes(buf_array![src,$n])))
        }
      }
    }
  )*};
}

define_static_builtins![
    (u8, 1),
    (u16, 2),
    (u32, 4),
    (u64, 8),
    (u128, 16),
    (i8, 1),
    (i16, 2),
    (i32, 4),
    (i64, 8),
    (i128, 16),
    (f32, 4),
    (f64, 8)
];

impl CountBytes for bool {
    fn count_from_bytes(_buf: &[u8]) -> Result<usize, Error> {
        Ok(1)
    }

    fn count_bytes(&self) -> usize {
        1
    }
}

impl CountBytesBE for bool {
    fn count_from_bytes_be(_buf: &[u8]) -> Result<usize, Error> {
        Ok(1)
    }

    fn count_bytes_be(&self) -> usize {
        1
    }
}

impl CountBytesLE for bool {
    fn count_from_bytes_le(_buf: &[u8]) -> Result<usize, Error> {
        Ok(1)
    }

    fn count_bytes_le(&self) -> usize {
        1
    }
}

impl ToBytes for bool {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(vec![if *self { 1 } else { 0 }])
    }

    fn write_bytes(&self, dst: &mut [u8]) -> Result<usize, Error> {
        if dst.is_empty() {
            return DesertErrorKind::DstInsufficient {
                provided: dst.len(),
                required: 1,
            }
            .raise();
        }
        dst[0] = *self as u8;
        Ok(1)
    }
}

impl ToBytesBE for bool {
    fn to_bytes_be(&self) -> Result<Vec<u8>, Error> {
        self.to_bytes()
    }

    fn write_bytes_be(&self, dst: &mut [u8]) -> Result<usize, Error> {
        self.write_bytes(dst)
    }
}

impl ToBytesLE for bool {
    fn to_bytes_le(&self) -> Result<Vec<u8>, Error> {
        self.to_bytes()
    }

    fn write_bytes_le(&self, dst: &mut [u8]) -> Result<usize, Error> {
        self.write_bytes(dst)
    }
}

impl FromBytes for bool {
    fn from_bytes(src: &[u8]) -> Result<(usize, Self), Error> {
        if src.is_empty() {
            DesertErrorKind::SrcInsufficient {
                provided: src.len(),
                required: 1,
            }
            .raise()
        } else {
            Ok((1, src[0] != 0))
        }
    }
}

impl FromBytesBE for bool {
    fn from_bytes_be(src: &[u8]) -> Result<(usize, Self), Error> {
        Self::from_bytes(src)
    }
}

impl FromBytesLE for bool {
    fn from_bytes_le(src: &[u8]) -> Result<(usize, Self), Error> {
        Self::from_bytes(src)
    }
}

macro_rules! define_tuple {
  ($(($T:tt,$i:tt)),+) => {
    impl<$($T),+> CountBytes for ($($T),+) where $($T: CountBytes),+ {
      fn count_from_bytes(buf: &[u8]) -> Result<usize,Error> {
        let mut offset = 0;
        $(
          offset += $T::count_from_bytes(&buf[offset..])?;
        )+
        Ok(offset)
      }

      fn count_bytes(&self) -> usize {
        $(self.$i.count_bytes() +)+ 0
      }
    }

    impl<$($T),+> CountBytesBE for ($($T),+) where $($T: CountBytesBE),+ {
      fn count_from_bytes_be(buf: &[u8]) -> Result<usize,Error> {
        let mut offset = 0;
        $(
          offset += $T::count_from_bytes_be(&buf[offset..])?;
        )+
        Ok(offset)
      }

      fn count_bytes_be(&self) -> usize {
        $(self.$i.count_bytes_be() +)+ 0
      }
    }

    impl<$($T),+> CountBytesLE for ($($T),+) where $($T: CountBytesLE),+ {
      fn count_from_bytes_le(buf: &[u8]) -> Result<usize,Error> {
        let mut offset = 0;
        $(
          offset += $T::count_from_bytes_le(&buf[offset..])?;
        )+
        Ok(offset)
      }

      fn count_bytes_le(&self) -> usize {
        $(self.$i.count_bytes_le() +)+ 0
      }
    }

    impl<$($T),+> ToBytes for ($($T),+) where $($T: ToBytes+CountBytes),+ {
      fn to_bytes(&self) -> Result<Vec<u8>,Error> {
        let mut buf = vec![0u8;$(self.$i.count_bytes() +)+ 0];
        self.write_bytes(&mut buf)?;
        Ok(buf)
      }

      fn write_bytes(&self, dst: &mut [u8]) -> Result<usize,Error> {
        let len = $(self.$i.count_bytes() +)+ 0;
        if dst.len() < len {
          return DesertErrorKind::DstInsufficient { provided: dst.len(), required: len }.raise();
        }
        let mut offset = 0;
        $(
          offset += self.$i.write_bytes(&mut dst[offset..])?;
        )+
        Ok(offset)
      }
    }

    impl<$($T),+> ToBytesBE for ($($T),+) where $($T: ToBytesBE+CountBytesBE),+ {
      fn to_bytes_be(&self) -> Result<Vec<u8>,Error> {
        let mut buf = vec![0u8;$(self.$i.count_bytes_be() +)+ 0];
        self.write_bytes_be(&mut buf)?;
        Ok(buf)
      }

      fn write_bytes_be(&self, dst: &mut [u8]) -> Result<usize,Error> {
        let len = $(self.$i.count_bytes_be() +)+ 0;
        if dst.len() < len {
          return DesertErrorKind::DstInsufficient { provided: dst.len(), required: len }.raise();
        }
        let mut offset = 0;
        $(
          offset += self.$i.write_bytes_be(&mut dst[offset..])?;
        )+
        Ok(offset)
      }
    }

    impl<$($T),+> ToBytesLE for ($($T),+) where $($T: ToBytesLE+CountBytesLE),+ {
      fn to_bytes_le(&self) -> Result<Vec<u8>,Error> {
        let mut buf = vec![0u8;$(self.$i.count_bytes_le() +)+ 0];
        self.write_bytes_le(&mut buf)?;
        Ok(buf)
      }

      fn write_bytes_le(&self, dst: &mut [u8]) -> Result<usize,Error> {
        let len = $(self.$i.count_bytes_le() +)+ 0;
        if dst.len() < len {
          return DesertErrorKind::DstInsufficient { provided: dst.len(), required: len }.raise();
        }
        let mut offset = 0;
        $(
          offset += self.$i.write_bytes_le(&mut dst[offset..])?;
        )+
        Ok(offset)
      }
    }

    impl<$($T),+> FromBytes for ($($T),+) where $($T: FromBytes),+ {
      fn from_bytes(src: &[u8]) -> Result<(usize,Self),Error> {
        let mut offset = 0;
        let result = ($({
          let (size,x) = $T::from_bytes(&src[offset..])?;
          offset += size;
          x
        }),+);
        Ok((offset,result))
      }
    }

    impl<$($T),+> FromBytesBE for ($($T),+) where $($T: FromBytesBE),+ {
      fn from_bytes_be(src: &[u8]) -> Result<(usize,Self),Error> {
        let mut offset = 0;
        let result = ($({
          let (size,x) = $T::from_bytes_be(&src[offset..])?;
          offset += size;
          x
        }),+);
        Ok((offset,result))
      }
    }

    impl<$($T),+> FromBytesLE for ($($T),+) where $($T: FromBytesLE),+ {
      fn from_bytes_le(src: &[u8]) -> Result<(usize,Self),Error> {
        let mut offset = 0;
        let result = ($({
          let (size,x) = $T::from_bytes_le(&src[offset..])?;
          offset += size;
          x
        }),+);
        Ok((offset,result))
      }
    }
  }
}

define_tuple![(A, 0), (B, 1)];
define_tuple![(A, 0), (B, 1), (C, 2)];
define_tuple![(A, 0), (B, 1), (C, 2), (D, 3)];
define_tuple![(A, 0), (B, 1), (C, 2), (D, 3), (E, 4)];
define_tuple![(A, 0), (B, 1), (C, 2), (D, 3), (E, 4), (F, 5)];
define_tuple![(A, 0), (B, 1), (C, 2), (D, 3), (E, 4), (F, 5), (G, 6)];
define_tuple![
    (A, 0),
    (B, 1),
    (C, 2),
    (D, 3),
    (E, 4),
    (F, 5),
    (G, 6),
    (H, 7)
];
define_tuple![
    (A, 0),
    (B, 1),
    (C, 2),
    (D, 3),
    (E, 4),
    (F, 5),
    (G, 6),
    (H, 7),
    (I, 8)
];
define_tuple![
    (A, 0),
    (B, 1),
    (C, 2),
    (D, 3),
    (E, 4),
    (F, 5),
    (G, 6),
    (H, 7),
    (I, 8),
    (J, 9)
];
define_tuple![
    (A, 0),
    (B, 1),
    (C, 2),
    (D, 3),
    (E, 4),
    (F, 5),
    (G, 6),
    (H, 7),
    (I, 8),
    (J, 9),
    (K, 10)
];
define_tuple![
    (A, 0),
    (B, 1),
    (C, 2),
    (D, 3),
    (E, 4),
    (F, 5),
    (G, 6),
    (H, 7),
    (I, 8),
    (J, 9),
    (K, 10),
    (L, 11)
];

impl<T, const N: usize> CountBytes for [T; N]
where
    T: CountBytes,
{
    fn count_from_bytes(buf: &[u8]) -> Result<usize, Error> {
        let mut offset = 0;
        for _i in 0..N {
            offset += T::count_from_bytes(&buf[offset..])?;
        }
        Ok(offset)
    }

    fn count_bytes(&self) -> usize {
        let mut size = 0;
        for item in self.iter().take(N) {
            size += item.count_bytes();
        }
        size
    }
}

impl<T, const N: usize> CountBytesBE for [T; N]
where
    T: CountBytesBE,
{
    fn count_from_bytes_be(buf: &[u8]) -> Result<usize, Error> {
        let mut offset = 0;
        for _i in 0..N {
            offset += T::count_from_bytes_be(&buf[offset..])?;
        }
        Ok(offset)
    }

    fn count_bytes_be(&self) -> usize {
        let mut size = 0;
        for item in self.iter().take(N) {
            size += item.count_bytes_be();
        }
        size
    }
}

impl<T, const N: usize> CountBytesLE for [T; N]
where
    T: CountBytesLE,
{
    fn count_from_bytes_le(buf: &[u8]) -> Result<usize, Error> {
        let mut offset = 0;
        for _i in 0..N {
            offset += T::count_from_bytes_le(&buf[offset..])?;
        }
        Ok(offset)
    }

    fn count_bytes_le(&self) -> usize {
        let mut size = 0;
        for item in self.iter().take(N) {
            size += item.count_bytes_le();
        }
        size
    }
}

impl<T, const N: usize> ToBytes for [T; N]
where
    T: ToBytes + CountBytes,
{
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0u8; self.count_bytes()];
        self.write_bytes(&mut buf)?;
        Ok(buf)
    }

    fn write_bytes(&self, dst: &mut [u8]) -> Result<usize, Error> {
        let mut len = 0;
        for item in self.iter().take(N) {
            len += item.count_bytes();
        }
        if dst.len() < len {
            return DesertErrorKind::DstInsufficient {
                provided: dst.len(),
                required: len,
            }
            .raise();
        }
        let mut offset = 0;
        for item in self.iter().take(N) {
            offset += item.write_bytes(&mut dst[offset..])?;
        }
        Ok(offset)
    }
}

impl<T, const N: usize> ToBytesBE for [T; N]
where
    T: ToBytesBE + CountBytesBE,
{
    fn to_bytes_be(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0u8; self.count_bytes_be()];
        self.write_bytes_be(&mut buf)?;
        Ok(buf)
    }

    fn write_bytes_be(&self, dst: &mut [u8]) -> Result<usize, Error> {
        let mut len = 0;
        for item in self.iter().take(N) {
            len += item.count_bytes_be();
        }
        if dst.len() < len {
            return DesertErrorKind::DstInsufficient {
                provided: dst.len(),
                required: len,
            }
            .raise();
        }
        let mut offset = 0;
        for item in self.iter().take(N) {
            offset += item.write_bytes_be(&mut dst[offset..])?;
        }
        Ok(offset)
    }
}

impl<T, const N: usize> ToBytesLE for [T; N]
where
    T: ToBytesLE + CountBytesLE,
{
    fn to_bytes_le(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0u8; self.count_bytes_le()];
        self.write_bytes_le(&mut buf)?;
        Ok(buf)
    }

    fn write_bytes_le(&self, dst: &mut [u8]) -> Result<usize, Error> {
        let mut len = 0;
        for item in self.iter().take(N) {
            len += item.count_bytes_le();
        }
        if dst.len() < len {
            return DesertErrorKind::DstInsufficient {
                provided: dst.len(),
                required: len,
            }
            .raise();
        }
        let mut offset = 0;
        for item in self.iter().take(N) {
            offset += item.write_bytes_le(&mut dst[offset..])?;
        }
        Ok(offset)
    }
}

impl<T, const N: usize> FromBytes for [T; N]
where
    T: FromBytes + Default + Copy,
{
    fn from_bytes(src: &[u8]) -> Result<(usize, Self), Error> {
        let mut res = [T::default(); N];
        let mut offset = 0;
        for byte in res.iter_mut().take(N) {
            let (size, x) = T::from_bytes(&src[offset..])?;
            offset += size;
            *byte = x;
        }
        Ok((offset, res))
    }
}

impl<T, const N: usize> FromBytesBE for [T; N]
where
    T: FromBytesBE + Default + Copy,
{
    fn from_bytes_be(src: &[u8]) -> Result<(usize, Self), Error> {
        let mut res = [T::default(); N];
        let mut offset = 0;
        for byte in res.iter_mut().take(N) {
            let (size, x) = T::from_bytes_be(&src[offset..])?;
            offset += size;
            *byte = x;
        }
        Ok((offset, res))
    }
}

impl<T, const N: usize> FromBytesLE for [T; N]
where
    T: FromBytesLE + Default + Copy,
{
    fn from_bytes_le(src: &[u8]) -> Result<(usize, Self), Error> {
        let mut res = [T::default(); N];
        let mut offset = 0;
        for byte in res.iter_mut().take(N) {
            let (size, x) = T::from_bytes_le(&src[offset..])?;
            offset += size;
            *byte = x;
        }
        Ok((offset, res))
    }
}

impl<T> ToBytes for [T]
where
    T: ToBytes + CountBytes,
{
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0u8; self.count_bytes()];
        self.write_bytes(&mut buf)?;
        Ok(buf)
    }

    fn write_bytes(&self, dst: &mut [u8]) -> Result<usize, Error> {
        let mut len = 0;
        for x in self.iter() {
            len += x.count_bytes();
        }
        let hlen = varint::length(len as u64);
        if dst.len() < len + hlen {
            return DesertErrorKind::DstInsufficient {
                provided: dst.len(),
                required: len + hlen,
            }
            .raise();
        }
        let mut offset = varint::encode(len as u64, dst)?;
        for x in self.iter() {
            offset += x.write_bytes(&mut dst[offset..])?;
        }
        Ok(offset)
    }
}

impl<T> ToBytesBE for [T]
where
    T: ToBytesBE + CountBytesBE,
{
    fn to_bytes_be(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0u8; self.count_bytes_be()];
        self.write_bytes_be(&mut buf)?;
        Ok(buf)
    }

    fn write_bytes_be(&self, dst: &mut [u8]) -> Result<usize, Error> {
        let mut len = 0;
        for x in self.iter() {
            len += x.count_bytes_be();
        }
        let hlen = varint::length(len as u64);
        if dst.len() < len + hlen {
            return DesertErrorKind::DstInsufficient {
                provided: dst.len(),
                required: len + hlen,
            }
            .raise();
        }
        let mut offset = varint::encode(len as u64, dst)?;
        for x in self.iter() {
            offset += x.write_bytes_be(&mut dst[offset..])?;
        }
        Ok(offset)
    }
}

impl<T> ToBytesLE for [T]
where
    T: ToBytesLE + CountBytesLE,
{
    fn to_bytes_le(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0u8; self.count_bytes_le()];
        self.write_bytes_le(&mut buf)?;
        Ok(buf)
    }

    fn write_bytes_le(&self, dst: &mut [u8]) -> Result<usize, Error> {
        let mut len = 0;
        for x in self.iter() {
            len += x.count_bytes_le();
        }
        let hlen = varint::length(len as u64);
        if dst.len() < len + hlen {
            return DesertErrorKind::DstInsufficient {
                provided: dst.len(),
                required: len + hlen,
            }
            .raise();
        }
        let mut offset = varint::encode(len as u64, dst)?;
        for x in self.iter() {
            offset += x.write_bytes_le(&mut dst[offset..])?;
        }
        Ok(offset)
    }
}

impl<T> CountBytes for [T]
where
    T: CountBytes,
{
    fn count_from_bytes(buf: &[u8]) -> Result<usize, Error> {
        let (mut offset, len) = varint::decode(buf)?;
        let end = (offset as u64) + len;
        while (offset as u64) < end {
            offset += T::count_from_bytes(&buf[offset..])?;
        }
        Ok(offset)
    }

    fn count_bytes(&self) -> usize {
        let mut len = 0;
        for x in self.iter() {
            len += x.count_bytes();
        }
        len + varint::length(len as u64)
    }
}

impl<T> CountBytesBE for [T]
where
    T: CountBytesBE,
{
    fn count_from_bytes_be(buf: &[u8]) -> Result<usize, Error> {
        let (mut offset, len) = varint::decode(buf)?;
        let end = (offset as u64) + len;
        while (offset as u64) < end {
            offset += T::count_from_bytes_be(&buf[offset..])?;
        }
        Ok(offset)
    }

    fn count_bytes_be(&self) -> usize {
        let mut len = 0;
        for x in self.iter() {
            len += x.count_bytes_be();
        }
        len + varint::length(len as u64)
    }
}

impl<T> CountBytesLE for [T]
where
    T: CountBytesLE,
{
    fn count_from_bytes_le(buf: &[u8]) -> Result<usize, Error> {
        let (mut offset, len) = varint::decode(buf)?;
        let end = (offset as u64) + len;
        while (offset as u64) < end {
            offset += T::count_from_bytes_le(&buf[offset..])?;
        }
        Ok(offset)
    }

    fn count_bytes_le(&self) -> usize {
        let mut len = 0;
        for x in self.iter() {
            len += x.count_bytes_le();
        }
        len + varint::length(len as u64)
    }
}

impl<T> ToBytes for Vec<T>
where
    T: ToBytes + CountBytes,
{
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0u8; self.count_bytes()];
        self.write_bytes(&mut buf)?;
        Ok(buf)
    }

    fn write_bytes(&self, dst: &mut [u8]) -> Result<usize, Error> {
        let mut len = 0;
        for x in self.iter() {
            len += x.count_bytes();
        }
        let hlen = varint::length(len as u64);
        if dst.len() < len + hlen {
            return DesertErrorKind::DstInsufficient {
                provided: dst.len(),
                required: len + hlen,
            }
            .raise();
        }
        let mut offset = varint::encode(len as u64, dst)?;
        for x in self.iter() {
            offset += x.write_bytes(&mut dst[offset..])?;
        }
        Ok(offset)
    }
}

impl<T> ToBytesBE for Vec<T>
where
    T: ToBytesBE + CountBytesBE,
{
    fn to_bytes_be(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0u8; self.count_bytes_be()];
        self.write_bytes_be(&mut buf)?;
        Ok(buf)
    }

    fn write_bytes_be(&self, dst: &mut [u8]) -> Result<usize, Error> {
        let mut len = 0;
        for x in self.iter() {
            len += x.count_bytes_be();
        }
        let hlen = varint::length(len as u64);
        if dst.len() < len + hlen {
            return DesertErrorKind::DstInsufficient {
                provided: dst.len(),
                required: len + hlen,
            }
            .raise();
        }
        let mut offset = varint::encode(len as u64, dst)?;
        for x in self.iter() {
            offset += x.write_bytes_be(&mut dst[offset..])?;
        }
        Ok(offset)
    }
}

impl<T> ToBytesLE for Vec<T>
where
    T: ToBytesLE + CountBytesLE,
{
    fn to_bytes_le(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0u8; self.count_bytes_le()];
        self.write_bytes_le(&mut buf)?;
        Ok(buf)
    }

    fn write_bytes_le(&self, dst: &mut [u8]) -> Result<usize, Error> {
        let mut len = 0;
        for x in self.iter() {
            len += x.count_bytes_le();
        }
        let hlen = varint::length(len as u64);
        if dst.len() < len + hlen {
            return DesertErrorKind::DstInsufficient {
                provided: dst.len(),
                required: len + hlen,
            }
            .raise();
        }
        let mut offset = varint::encode(len as u64, dst)?;
        for x in self.iter() {
            offset += x.write_bytes_le(&mut dst[offset..])?;
        }
        Ok(offset)
    }
}

impl<T> FromBytes for Vec<T>
where
    T: FromBytes,
{
    fn from_bytes(src: &[u8]) -> Result<(usize, Self), Error> {
        let (mut offset, len) = varint::decode(src)?;
        let end = offset + (len as usize);
        let mut v = vec![];
        while offset < end {
            let (size, x) = T::from_bytes(&src[offset..])?;
            v.push(x);
            offset += size;
        }
        Ok((end, v))
    }
}

impl<T> FromBytesBE for Vec<T>
where
    T: FromBytesBE,
{
    fn from_bytes_be(src: &[u8]) -> Result<(usize, Self), Error> {
        let (mut offset, len) = varint::decode(src)?;
        let end = offset + (len as usize);
        let mut v = vec![];
        while offset < end {
            let (size, x) = T::from_bytes_be(&src[offset..])?;
            v.push(x);
            offset += size;
        }
        Ok((end, v))
    }
}

impl<T> FromBytesLE for Vec<T>
where
    T: FromBytesLE,
{
    fn from_bytes_le(src: &[u8]) -> Result<(usize, Self), Error> {
        let (mut offset, len) = varint::decode(src)?;
        let end = offset + (len as usize);
        let mut v = vec![];
        while offset < end {
            let (size, x) = T::from_bytes_le(&src[offset..])?;
            v.push(x);
            offset += size;
        }
        Ok((end, v))
    }
}

impl<T> CountBytes for Vec<T>
where
    T: CountBytes,
{
    fn count_from_bytes(buf: &[u8]) -> Result<usize, Error> {
        let (mut offset, len) = varint::decode(buf)?;
        let end = (offset as u64) + len;
        while (offset as u64) < end {
            offset += T::count_from_bytes(&buf[offset..])?;
        }
        Ok(offset)
    }

    fn count_bytes(&self) -> usize {
        let mut len = 0;
        for x in self.iter() {
            len += x.count_bytes();
        }
        len + varint::length(len as u64)
    }
}

impl<T> CountBytesBE for Vec<T>
where
    T: CountBytesBE,
{
    fn count_from_bytes_be(buf: &[u8]) -> Result<usize, Error> {
        let (mut offset, len) = varint::decode(buf)?;
        let end = (offset as u64) + len;
        while (offset as u64) < end {
            offset += T::count_from_bytes_be(&buf[offset..])?;
        }
        Ok(offset)
    }

    fn count_bytes_be(&self) -> usize {
        let mut len = 0;
        for x in self.iter() {
            len += x.count_bytes_be();
        }
        len + varint::length(len as u64)
    }
}

impl<T> CountBytesLE for Vec<T>
where
    T: CountBytesLE,
{
    fn count_from_bytes_le(buf: &[u8]) -> Result<usize, Error> {
        let (mut offset, len) = varint::decode(buf)?;
        let end = (offset as u64) + len;
        while (offset as u64) < end {
            offset += T::count_from_bytes_le(&buf[offset..])?;
        }
        Ok(offset)
    }

    fn count_bytes_le(&self) -> usize {
        let mut len = 0;
        for x in self.iter() {
            len += x.count_bytes_le();
        }
        len + varint::length(len as u64)
    }
}
