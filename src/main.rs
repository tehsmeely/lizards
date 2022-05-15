use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::ops::Range;

use offset_len::OffsetLen;
use output_stream::OutputStream;

mod decode;
mod encode;
mod helpers;
mod offset_len;
mod output_stream;

const MAX_LOOKBACK_BUFFER_LEN: usize = 128;
const MAX_READ_BUFFER_LEN: usize = 64;

//Storing matches using 2 bytes, there's no point making a match of 2 or less
const MIN_MATCH_SIZE: usize = 4;

const DEBUG: bool = false;

fn main() {
    println!("Encoding!");
    encode::encode("sample3");
    println!("Decoding!");
    decode::decode("sample3");
}

enum EncodedValue {
    OffsetLen(OffsetLen),
    RawU8(u8),
}

impl EncodedValue {
    fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::RawU8(v) => vec![*v],
            Self::OffsetLen(offset_len) => Vec::from(offset_len.to_bytes_new()),
        }
    }
    // TODO: Implement Write to write to a buffer instead of having to make a vec each time?
}

struct ChunkMarker {
    len: u8,
}

impl ChunkMarker {
    fn to_u8(&self) -> u8 {
        let mask = 0b11000000;
        self.len | mask
    }

    fn from_encoded_u8(v: u8) -> Self {
        Self {
            len: v & 0b00111111,
        }
    }

    fn to_debug_bytes(&self) -> Vec<u8> {
        let s = format!("<{}>", self.len);
        s.into_bytes()
    }
}
