use crate::error::{DesertErrorKind, Error};

/// Decode a varint into an unsigned 64 bit integer (includes offset in result).
pub fn decode(buf: &[u8]) -> Result<(usize, u64), Error> {
    let mut value = 0u64;
    let mut m = 1u64;
    let mut offset = 0usize;
    for _i in 0..8 {
        if offset >= buf.len() {
            return DesertErrorKind::VarintSrcInsufficient {}.raise();
        }
        let byte = buf[offset];
        offset += 1;
        value += m * u64::from(byte & 127);
        m *= 128;
        if byte & 128 == 0 {
            break;
        }
    }
    Ok((offset, value))
}

/// Encode an unsigned 64 bit integer as a varint and write the bytes to the
/// given buffer, returning the byte length of the value.
pub fn encode(value: u64, buf: &mut [u8]) -> Result<usize, Error> {
    let len = length(value);
    if buf.len() < len {
        return DesertErrorKind::VarintDstInsufficient {}.raise();
    }
    let mut offset = 0;
    let mut v = value;
    while v > 127 {
        buf[offset] = (v as u8) | 128;
        offset += 1;
        v >>= 7;
    }
    buf[offset] = v as u8;
    Ok(len)
}

/// Determine the length of an unsigned 64 bit integer.
pub fn length(value: u64) -> usize {
    // Determine the most-significant bit (MSB).
    let msb = (64 - value.leading_zeros()) as usize;
    (msb.max(1) + 6) / 7
}
