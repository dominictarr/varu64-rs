//! Implementation of the [varu64 format](https://github.com/AljoschaMeyer/varu64-rs) in rust.

#[cfg(test)]
#[macro_use]
extern crate quickcheck;

use std::{fmt, error, io};

pub mod nb;

/// Return how many bytes the encoding of `n` will take up.
pub fn encoding_length(n: u64) -> usize {
    if n < 248 {
        1
    } else if n < 256 {
        2
    } else if n < 65536 {
        3
    } else if n < 16777216 {
        4
    } else if n < 4294967296 {
        5
    } else if n < 1099511627776 {
        6
    } else if n < 281474976710656 {
        7
    } else if n < 72057594037927936 {
        8
    } else {
        9
    }
}

/// Encodes `n` into the output buffer, returning how many bytes have been written.
///
/// # Panics
/// Panics if the buffer is not large enough to hold the encoding.
pub fn encode(n: u64, out: &mut [u8]) -> usize {
    if n < 248 {
        out[0] = n as u8;
        1
    } else if n < 256 {
        out[0] = 248;
        write_bytes(n, 1, &mut out[1..]);
        2
    } else if n < 65536 {
        out[0] = 249;
        write_bytes(n, 2, &mut out[1..]);
        3
    } else if n < 16777216 {
        out[0] = 250;
        write_bytes(n, 3, &mut out[1..]);
        4
    } else if n < 4294967296 {
        out[0] = 251;
        write_bytes(n, 4, &mut out[1..]);
        5
    } else if n < 1099511627776 {
        out[0] = 252;
        write_bytes(n, 5, &mut out[1..]);
        6
    } else if n < 281474976710656 {
        out[0] = 253;
        write_bytes(n, 6, &mut out[1..]);
        7
    } else if n < 72057594037927936 {
        out[0] = 254;
        write_bytes(n, 7, &mut out[1..]);
        8
    } else {
        out[0] = 255;
        write_bytes(n, 8, &mut out[1..]);
        9
    }
}

/// Encodes `n` into the writer, returning how many bytes have been written.
pub fn encode_write<W: io::Write>(n: u64, mut w: W) -> Result<usize, io::Error> {
    let mut tmp = [0u8; 9];
    let written = encode(n, &mut tmp[..]);
    w.write_all(&tmp[..written]).map(|_| written)
}

// Write the k least significant bytes of n into out, in big-endian byteorder, panicking
// if out is too small.
//
// k must be smaller than 8.
fn write_bytes(n: u64, k: usize, out: &mut [u8]) {
    let bytes: [u8; 8] = unsafe { std::mem::transmute(u64::to_be(n)) };
    for i in 0..k {
        out[i] = bytes[(8 - k) + i];
    }
}

/// Decode a `u64` from the `input` buffer, returning the number and the remaining bytes.
///
/// # Errors
/// On error, this also returns how many bytes were read (including the erroneous byte). In case
/// of noncanonical data (encodings that are valid except they are not the smallest possible
/// encoding), the full data is parsed, even if the non-canonicty could be detected early on.
///
/// If there is not enough input data, an `UnexpectedEndOfInput` error is returned, never
/// a `NonCanonical` error (even if the partial input could already be detected to be
/// noncanonical).
pub fn decode(input: &[u8]) -> Result<(u64, &[u8]), (DecodeError, &[u8])> {
    let first: u8;
    match input.get(0) {
        Some(b) => first = *b,
        None => return Err((UnexpectedEndOfInput, input)),
    }

    if (first | 0b0000_0111) == 0b1111_1111 {
        // first five bytes are ones, value is 248 or more

        // Total length of the encoded data is 1 byte for the tag plus the value of
        // the three least sgnificant bits incremented by 1.
        let length = (first & 0b0000_0111) as usize + 2;
        let mut out: u64 = 0;

        for i in 1..length {
            out <<= 8;
            match input.get(i) {
                Some(b) => out += *b as u64,
                None => return Err((UnexpectedEndOfInput, &input[i..])),
            }
        }

        if length > encoding_length(out) {
            return Err((NonCanonical(out), &input[length..]));
        } else {
            return Ok((out, &input[length..]));
        }
    } else {
        // value is less than 248
        return Ok((first as u64, &input[1..]));
    }
}

/// Everything that can go wrong when decoding a varu64.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DecodeError {
    /// The encoding is not the shortest possible one for the number.
    /// Contains the encoded number.
    NonCanonical(u64),
    /// The slice contains less data than the encoding needs.
    UnexpectedEndOfInput,
}
use DecodeError::*;

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        match self {
            NonCanonical(n) => write!(f, "Invalid varu64: NonCanonical encoding of {}", n),
            UnexpectedEndOfInput => write!(f, "Invalid varu64: Not enough input bytes"),
        }
    }
}

impl error::Error for DecodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    // Assert that the given u64 encodes to the expected encoding, and that the
    // expected encoding decodes to the u64.
    fn test_fixture(n: u64, exp: &[u8]) {
        let mut foo = [0u8; 9];

        let enc_len = encode(n, &mut foo[..]);
        assert_eq!(&foo[..enc_len], exp);

        let (dec, tail) = decode(exp).unwrap();
        assert_eq!(dec, n);
        assert_eq!(tail, &[][..]);
    }

    #[test]
    fn fixtures() {
        test_fixture(0, &[0]);
        test_fixture(1, &[1]);
        test_fixture(247, &[247]);
        test_fixture(248, &[248, 248]);
        test_fixture(255, &[248, 255]);
        test_fixture(256, &[249, 1, 0]);
        test_fixture(65535, &[249, 255, 255]);
        test_fixture(65536, &[250, 1, 0, 0]);
        test_fixture(72057594037927935, &[254, 255, 255, 255, 255, 255, 255, 255]);
        test_fixture(72057594037927936, &[255, 1, 0, 0, 0, 0, 0, 0, 0]);

        assert_eq!(decode(&[]).unwrap_err(), (UnexpectedEndOfInput, &[][..]));
        assert_eq!(decode(&[248]).unwrap_err(), (UnexpectedEndOfInput, &[][..]));
        assert_eq!(decode(&[255, 0, 1, 2, 3, 4, 5]).unwrap_err(),
                   (UnexpectedEndOfInput, &[][..]));
        assert_eq!(decode(&[255, 0, 1, 2, 3, 4, 5, 6]).unwrap_err(),
                   (UnexpectedEndOfInput, &[][..]));

        assert_eq!(decode(&[248, 42]).unwrap_err(), (NonCanonical(42), &[][..]));
        assert_eq!(decode(&[249, 0, 42]).unwrap_err(),
                   (NonCanonical(42), &[][..]));
    }
}
