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

    let f = File::open("testfile.txt").unwrap();
    let mut reader = BufReader::new(f);

    let mut prev_control_byte: Option<u8> = None;
    let control_byte_mask = 0b10000000;

    loop {
        let result = reader.read(&mut input_buffer);

        match result {
            Err(e) => panic!("Error reading file: {}", e),
            Ok(0) => break,
            Ok(1) => {
                let is_control = input_buffer[0] & control_byte_mask != 0;
                match (is_control, prev_control_byte) {
                    (true, Some(byte)) => {
                        let offset_len = OffsetLen::of_control_bytes(byte, input_buffer[0]);
                        let values_from_buf: Vec<u8> =
                            read_buffer.range(offset_len.to_range()).copied().collect();
                        read_buffer.extend(values_from_buf.iter());
                    }
                    (true, None) => prev_control_byte = Some(input_buffer[0]),
                    (false, Some(byte)) => {
                        panic!("Only got one control byte: {}", byte)
                    }
                    (false, None) => read_buffer.push_back(input_buffer[0]),
                }
                if let Some(prev_byte) = prev_control_byte {}
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

fn encode() {
    println!("Lizards!");

    let mut input_buffer: [u8; 1] = [0b0; 1];
    let mut read_buffer = VecDeque::<u8>::new();
    let mut lookback_buffer = VecDeque::<u8>::new();

    let mut encoded_values: Vec<EncodedValue> = Vec::new();

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
        encoded_values.push(next_value);
        step_buffers(
            step_size,
            &mut reader,
            &mut input_buffer,
            &mut read_buffer,
            &mut lookback_buffer,
            true,
        );
    }

    println!("Writing out");
    let outf = File::create("testfile.lizard").unwrap();
    let mut writer = BufWriter::new(outf);
    for encoded_value in encoded_values.iter() {
        writer.write_all(&encoded_value.to_bytes()).unwrap();
    }
    println!("Done");
}

fn main() {
    encode();
    decode();
}

fn find_match(read_buffer: &VecDeque<u8>, lookback_buffer: &VecDeque<u8>) -> EncodedValue {
    let total_len = read_buffer.len() + lookback_buffer.len();
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
                current_match.0 += 1;
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
            offset: *offset as u8,
            len: matched_values.len() as u8,
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

struct OffsetLen {
    offset: u8,
    len: u8,
}

impl OffsetLen {
    fn to_bytes(&self) -> Vec<u8> {
        if DEBUG {
            return self.to_bytes_debug();
        }
        let mask: u8 = 0b10000000;
        vec![self.offset | mask, self.len | mask]
    }

    fn to_bytes_debug(&self) -> Vec<u8> {
        let s = format!("({},{})", self.offset, self.len);
        s.into_bytes()
    }

    fn of_control_bytes(first: u8, second: u8) -> Self {
        let mask = 0b01111111;
        Self {
            offset: first & mask,
            len: second & mask,
        }
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
            Self::OffsetLen(offset_len) => offset_len.to_bytes(),
        }
    }
    // TODO: Implement Write to write to a buffer instead of having to make a vec each time?
}
