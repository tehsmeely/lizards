use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::ops::Range;

const MAX_LOOKBACK_BUFFER_LEN: usize = 128;
const MAX_READ_BUFFER_LEN: usize = 64;

//Storing matches using 2 bytes, there's no point making a match of 2 or less
const MIN_MATCH_SIZE: usize = 3;

const DEBUG: bool = false;

fn decode() {
    // Read file byte by byte
    // If msb is 1, start building OffsetLen

    let mut input_buffer: [u8; 1] = [0b0; 1];
    let mut read_buffer = VecDeque::<u8>::new();

    let f = File::open("testfile.lizard").unwrap();
    let mut reader = BufReader::new(f);

    let control_byte_mask = 0b10000000;
    let mut decode_state = DecodeParseState::None;

    loop {
        let result = reader.read(&mut input_buffer);

        println!("State: {:?}", decode_state);
        match result {
            Err(e) => panic!("Error reading file: {}", e),
            Ok(0) => break,
            Ok(1) => {
                let v = input_buffer[0];
                println!("{:#010b}", v);
                match decode_state {
                    DecodeParseState::None => {
                        match v >> 6 {
                            0b10 => decode_state = DecodeParseState::PartialCommandRead(v),
                            0b11 => {
                                let marker = ChunkMarker::from_encoded_u8(v);
                                decode_state = DecodeParseState::RawByteChunk(marker.len)
                            }
                            other => {
                                panic!("Did not get leading bits expected: {}  ({:#010b}", other, v)
                            }
                        }
                        //Accept either control byte or chunk marker
                    }
                    DecodeParseState::RawByteChunk(remaining) => {
                        //Read u8 as is.
                        // decr [remaining]
                        // if zero, state -> DecodeParseState::None
                        read_buffer.push_back(v);
                        match remaining - 1 {
                            0 => decode_state = DecodeParseState::None,
                            decr => decode_state = DecodeParseState::RawByteChunk(decr),
                        }
                    }
                    DecodeParseState::PartialCommandRead(first_byte) => {
                        decode_state = DecodeParseState::PartialCommandRead2(first_byte, v);
                    }
                    DecodeParseState::PartialCommandRead2(b0, b1) => {
                        //Expect command byte, build OffsetLen
                        // state -> DecodeParseState::None
                        let offset_len = OffsetLen::of_bytes_new(b0, b1, v);
                        let values_from_buf: Vec<u8> =
                            read_buffer.range(offset_len.to_range()).copied().collect();
                        read_buffer.extend(values_from_buf.iter());
                        decode_state = DecodeParseState::None;
                    }
                }
            }
            Ok(n) => panic!("Read more than expected bytes: {}", n),
        }
    }

    println!("Writing out");
    let outf = File::create("testfile.unenc.txt").unwrap();
    let mut writer = BufWriter::new(outf);
    read_buffer.make_contiguous();
    writer.write_all(read_buffer.as_slices().0);
    println!("Done");
}

#[derive(Debug)]
enum DecodeParseState {
    RawByteChunk(u8),
    None,
    PartialCommandRead(u8),
    PartialCommandRead2(u8, u8),
}

fn encode() {
    println!("Lizards!");

    let mut input_buffer: [u8; 1] = [0b0; 1];
    let mut read_buffer = VecDeque::<u8>::new();
    let mut lookback_buffer = VecDeque::<u8>::new();

    //let mut encoded_values: Vec<EncodedValue> = Vec::new();
    let outf = File::create("testfile.lizard").unwrap();
    let df = File::create("testfile.dblzd").unwrap();
    let mut writer = BufWriter::new(outf);
    let mut debug_writer = BufWriter::new(df);
    let mut output_stream = OutputStream::new_debug(writer, debug_writer);

    let f = File::open("testfile.txt").unwrap();
    let mut reader = BufReader::new(f);

    //Init read buffer
    for _i in 0..MAX_READ_BUFFER_LEN {
        step_buffers(
            1,
            &mut reader,
            &mut input_buffer,
            &mut read_buffer,
            &mut lookback_buffer,
            false,
        );
    }

    // Keep going until read_buffer is empty
    while read_buffer.len() > 0 {
        //Match
        let next_value = find_match(&read_buffer, &lookback_buffer);
        let step_size = match next_value {
            EncodedValue::RawU8(_) => 1,
            EncodedValue::OffsetLen(OffsetLen { len, offset: _ }) => len as usize,
        };
        //encoded_values.push(next_value);
        output_stream.add(next_value);
        step_buffers(
            step_size,
            &mut reader,
            &mut input_buffer,
            &mut read_buffer,
            &mut lookback_buffer,
            true,
        );
    }

    /*
    println!("Writing out");
    let outf = File::create("testfile.lizard").unwrap();
    let mut writer = BufWriter::new(outf);
    for encoded_value in encoded_values.iter() {
        writer.write_all(&encoded_value.to_bytes()).unwrap();
    }
    */
    println!("Done");
}

fn main() {
    println!("Encoding!");
    encode();
    println!("Decoding!");
    decode();
}

fn find_match(read_buffer: &VecDeque<u8>, lookback_buffer: &VecDeque<u8>) -> EncodedValue {
    // TODO support the max values in the OffsetLen
    let total_len = read_buffer.len() + lookback_buffer.len();
    // Current match: offset, matched bytes
    // TODO: Type this up a bit?
    let mut current_match = (0, Vec::new());
    let mut best_match: Option<(usize, Vec<u8>)> = None;
    for i in 0..total_len {
        //Never start matching when looking at read buffer, or we'll always match read buffer on itself
        if i >= lookback_buffer.len() && current_match.1.is_empty() {
            break;
        }
        let looking_at = if i < lookback_buffer.len() {
            lookback_buffer[i]
        } else {
            read_buffer[i - lookback_buffer.len()]
        };
        let expecting = read_buffer.get(current_match.1.len());
        if let Some(expecting_v) = expecting {
            if looking_at == *expecting_v {
                if current_match.1.is_empty() {
                    current_match.0 = i;
                }
                current_match.1.push(looking_at);

                let is_best = match &best_match {
                    None => true,
                    Some((_, matched_values)) => current_match.1.len() > matched_values.len(),
                };
                if is_best {
                    best_match = Some(current_match.clone())
                }
            } else {
                current_match.0 = 0;
                current_match.1.clear();
            }
        }
    }
    match &best_match {
        None => EncodedValue::RawU8(*read_buffer.front().unwrap()),
        Some((_, matched_values)) if matched_values.len() < MIN_MATCH_SIZE => {
            EncodedValue::RawU8(*read_buffer.front().unwrap())
        }
        Some((offset, matched_values)) => EncodedValue::OffsetLen(OffsetLen {
            offset: *offset as u16,
            len: matched_values.len() as u16,
        }),
    }
}

fn step_buffers(
    n: usize,
    reader: &mut BufReader<File>,
    input_buffer: &mut [u8],
    read_buffer: &mut VecDeque<u8>,
    lookback_buffer: &mut VecDeque<u8>,
    always_drain_read: bool,
) {
    for _i in 0..n {
        let read = reader.read(input_buffer);
        match read {
            Err(e) => panic!("Error reading file: {}", e),
            Ok(0) => {
                println!("Got zero bytes");
                if always_drain_read {
                    // TODO: Unwind duplicated code
                    let transfer = read_buffer.pop_front();
                    if let Some(v) = transfer {
                        lookback_buffer.push_back(v);
                        if lookback_buffer.len() > MAX_LOOKBACK_BUFFER_LEN {
                            lookback_buffer.pop_front();
                        }
                    }
                }
            }
            Ok(1) => {
                read_buffer.push_back(input_buffer[0]);
                if read_buffer.len() > MAX_READ_BUFFER_LEN || always_drain_read {
                    let transfer = read_buffer.pop_front();
                    if let Some(v) = transfer {
                        lookback_buffer.push_back(v);
                        if lookback_buffer.len() > MAX_LOOKBACK_BUFFER_LEN {
                            lookback_buffer.pop_front();
                        }
                    }
                }
                println!("{:?}::{:?}", lookback_buffer, read_buffer);
            }
            Ok(n) => {
                panic!("Sadness, got more than 1 byte on [read] : {}", n)
            }
        }
    }
}

#[derive(Debug, PartialEq)]
struct OffsetLen {
    offset: u16,
    len: u16,
}

mod test {
    use crate::OffsetLen;

    #[test]
    fn offset_len_round_trip() {
        let a = OffsetLen { offset: 5, len: 10 };
        let [a0, a1, a2] = a.to_bytes_new();
        let b = OffsetLen::of_bytes_new(a0, a1, a2);
        assert_eq!(a, b)
    }

    #[test]
    fn big_offset_len_round_trip() {
        let a = OffsetLen {
            offset: 2047,
            len: 2047,
        };
        let [a0, a1, a2] = a.to_bytes_new();
        let b = OffsetLen::of_bytes_new(a0, a1, a2);
        assert_eq!(a, b)
    }
}

impl OffsetLen {
    const MAX_VALUE: u16 = 2047;
    fn new(offset: u16, len: u16) -> Self {
        if offset > Self::MAX_VALUE {
            panic!("Offset above max value: {}", offset);
        }
        if len > Self::MAX_VALUE {
            panic!("Len above max value: {}", len);
        }
        Self { offset, len }
    }
    /* Stuffing into 3 bytes:
    b0        b1        b2
    [10aaaaaa][aaaaabbb][bbbbbbbb]
    each is 11 bits

    a and b are u16 so
    [_____aaaaaaaaaaa]
    [_____bbbbbbbbbbb]

    TODO: Consider using first byte as [num_bytes] instead for bigger values
     */
    fn to_bytes_new(&self) -> [u8; 3] {
        let b0 = ((self.offset >> 5) as u8) | 0b10000000;
        let b1 = ((self.offset << 3) as u8) | ((self.len >> 8) as u8);
        let b2 = self.len as u8;
        [b0, b1, b2]
    }

    fn of_bytes_new(b0: u8, b1: u8, b2: u8) -> Self {
        let mask = 0b0000011111111111;
        let a = ((b0 as u16) << 5) | (b1 >> 3) as u16;
        let offset = a & mask;
        let b = ((b1 as u16) << 8) | (b2 as u16);
        let len = b & mask;
        Self { offset, len }
    }

    fn to_bytes_debug(&self) -> Vec<u8> {
        let s = format!("({},{})", self.offset, self.len);
        s.into_bytes()
    }

    fn to_range(&self) -> Range<usize> {
        Range {
            start: self.offset as usize,
            end: (self.offset + self.len) as usize,
        }
    }
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

// TODO:
struct OutputStream {
    buf: Vec<u8>,
    output: BufWriter<File>,
    debug_output: Option<BufWriter<File>>,
}

impl OutputStream {
    // This max is driven by what we can fit in the chunk marker bytes
    // since we use the two left bits to identify it, the value is 2^6 -1
    const MAX_CHUNK_LEN: usize = 63;
    fn new(output: BufWriter<File>) -> Self {
        Self {
            buf: Vec::new(),
            output,
            debug_output: None,
        }
    }
    fn new_debug(output: BufWriter<File>, debug_output: BufWriter<File>) -> Self {
        Self {
            buf: Vec::new(),
            output,
            debug_output: Some(debug_output),
        }
    }

    fn end_chunk(&mut self) {
        let chunk_marker = ChunkMarker {
            len: self.buf.len() as u8,
        };
        self.output.write(&[chunk_marker.to_u8()]);
        self.output.write_all(&self.buf);
        if let Some(writer) = &mut self.debug_output {
            writer.write_all(&chunk_marker.to_debug_bytes());
            writer.write_all(&self.buf);
        }
        self.buf.clear();
    }

    fn add(&mut self, value: EncodedValue) {
        match value {
            EncodedValue::RawU8(v) => {
                self.buf.push(v);
                if self.buf.len() >= Self::MAX_CHUNK_LEN {
                    self.end_chunk()
                }
            }
            EncodedValue::OffsetLen(offset_len) => {
                if !self.buf.is_empty() {
                    self.end_chunk()
                }
                self.output.write_all(&offset_len.to_bytes_new()).unwrap();
                if let Some(writer) = &mut self.debug_output {
                    writer.write_all(&offset_len.to_bytes_debug());
                }
            }
        }
    }
    fn finalise(&mut self) {
        if !self.buf.is_empty() {
            self.end_chunk()
        }
    }
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
