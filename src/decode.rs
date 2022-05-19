use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

use crate::file_io::FileInputOutput;
use crate::offset_len::OffsetLen;
use crate::{helpers, ChunkMarker, MAX_LOOKBACK_BUFFER_LEN};

pub fn decode(file_io: &FileInputOutput) {
    let mut input_buffer: [u8; 1] = [0b0; 1];
    let mut output_buffer = Vec::<u8>::new();
    let mut read_buffer = VecDeque::<u8>::new();
    let mut offset_len_read_buffer = Vec::<u8>::new();

    let f = File::open(file_io.encoded_filename.as_path()).unwrap();
    let mut reader = BufReader::new(f);

    let mut decode_state = DecodeParseState::None;

    loop {
        let result = reader.read(&mut input_buffer);

        println!("State: {:?}", decode_state);
        match result {
            Err(e) => panic!("Error reading file: {}", e),
            Ok(0) => break,
            Ok(1) => {
                let v = input_buffer[0];
                println!("{:#010b} : {:?}", v, String::from_utf8(vec![v]));
                match decode_state {
                    DecodeParseState::None => {
                        match v >> 6 {
                            0b10 => {
                                let (num_offset_bytes, num_len_bytes) =
                                    OffsetLen::read_header_byte(v);
                                offset_len_read_buffer.clear();
                                offset_len_read_buffer.push(v);
                                decode_state = DecodeParseState::OffsetLenRead(
                                    num_offset_bytes + num_len_bytes,
                                );
                            }
                            0b11 => {
                                let marker = ChunkMarker::from_encoded_u8(v);
                                decode_state = DecodeParseState::RawByteChunk(
                                    marker.len,
                                    RawByteReadOnFinish::Nothing,
                                )
                            }
                            other => {
                                panic!("Did not get leading bits expected: {}  ({:#010b}", other, v)
                            }
                        }
                        //Accept either control byte or chunk marker
                    }
                    DecodeParseState::RawByteChunk(remaining, on_finish) => {
                        //Read u8 as is.
                        // decr [remaining]
                        // if zero, state -> DecodeParseState::None
                        read_buffer.push_back(v);
                        match remaining - 1 {
                            0 => {
                                match on_finish {
                                    RawByteReadOnFinish::Nothing => (),
                                    RawByteReadOnFinish::FinaliseMatch(offset_len) => {
                                        finalise_match(&mut read_buffer, &offset_len);
                                    }
                                }
                                decode_state = DecodeParseState::None
                            }
                            decr => decode_state = DecodeParseState::RawByteChunk(decr, on_finish),
                        }
                    }

                    DecodeParseState::PartialCommandRead(first_byte) => {
                        decode_state = DecodeParseState::PartialCommandRead2(first_byte, v);
                    }
                    DecodeParseState::PartialCommandRead2(b0, b1) => {
                        //Expect command byte, build OffsetLen
                        // state -> DecodeParseState::None
                        let offset_len = OffsetLen::of_bytes_new(&vec![b0, b1, v]);
                        println!(
                            "Got Match: {:?}: {:?} ({})",
                            offset_len,
                            helpers::read_buffer_to_string(&read_buffer),
                            read_buffer.len()
                        );
                        if offset_len.range_end() > read_buffer.len() {
                            let bytes_needed_to_read = offset_len.range_end() - read_buffer.len();
                            println!(
                                "More bytes needed to populate match: {}",
                                bytes_needed_to_read
                            );
                            decode_state = DecodeParseState::RawByteChunk(
                                bytes_needed_to_read as u8,
                                RawByteReadOnFinish::FinaliseMatch(offset_len),
                            );
                        } else {
                            finalise_match(&mut read_buffer, &offset_len);
                            decode_state = DecodeParseState::None;
                        }
                    }
                    DecodeParseState::OffsetLenRead(remaining_bytes) => {
                        offset_len_read_buffer.push(v);
                        match remaining_bytes - 1 {
                            0 => {
                                let offset_len = OffsetLen::of_bytes_new(&offset_len_read_buffer);
                                finalise_match(&mut read_buffer, &offset_len);
                                decode_state = DecodeParseState::None
                            }
                            decr => decode_state = DecodeParseState::OffsetLenRead(decr),
                        }
                    }
                }
                // We use max "Lookback" buffer len here because the offsets generated by
                // matching when encoding are from the lookback buffer
                while read_buffer.len() > MAX_LOOKBACK_BUFFER_LEN {
                    output_buffer.push(read_buffer.pop_front().unwrap());
                }
            }
            Ok(n) => panic!("Read more than expected bytes: {}", n),
        }
    }

    //Handle final decode state
    match decode_state {
        DecodeParseState::None => (),
        DecodeParseState::RawByteChunk(_, RawByteReadOnFinish::Nothing) => {
            panic!("Ended parsing file but still just expecting to read raw bytes");
        }
        DecodeParseState::RawByteChunk(
            num_bytes_left,
            RawByteReadOnFinish::FinaliseMatch(offset_len),
        ) => {
            //If we finish the file with a partial match, we can infer there was some repetition
        }
        DecodeParseState::PartialCommandRead(_) | DecodeParseState::PartialCommandRead2(_, _) => {
            panic!("Ended parsing file but still not finished reading command bytes")
        }
        DecodeParseState::OffsetLenRead(_) => {
            panic!("Ended parsing file but still not finished reading command bytes")
        }
    }

    println!("Writing out");
    let outf = File::create(file_io.unencoded_filename.as_path()).unwrap();
    let mut writer = BufWriter::new(outf);
    read_buffer.make_contiguous();
    output_buffer.extend_from_slice(read_buffer.as_slices().0);
    writer.write_all(&output_buffer);
    println!("Done");
}

#[derive(Debug)]
enum RawByteReadOnFinish {
    Nothing,
    FinaliseMatch(OffsetLen),
}

#[derive(Debug)]
enum DecodeParseState {
    RawByteChunk(u8, RawByteReadOnFinish),
    None,
    PartialCommandRead(u8),
    PartialCommandRead2(u8, u8),
    OffsetLenRead(usize),
}

fn finalise_match(read_buffer: &mut VecDeque<u8>, offset_len: &OffsetLen) {
    let values_from_buf: Vec<u8> = {
        let range = offset_len.to_range();
        if range.end > read_buffer.len() {
            panic!(
                "Range loaded from file exceeds read_buffer ({:?}):\n{} ({})",
                offset_len,
                helpers::read_buffer_to_string(&read_buffer),
                read_buffer.len()
            );
        }
        read_buffer.range(range).copied().collect()
    };
    read_buffer.extend(values_from_buf.iter());
}
