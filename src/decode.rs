use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

use crate::file_io::FileInputOutput;
use crate::header::Header;
use crate::offset_len::OffsetLen;
use crate::{helpers, ChunkMarker, MAX_LOOKBACK_BUFFER_LEN};

pub fn decode(file_io: &FileInputOutput) {
    let mut input_buffer: [u8; 1] = [0b0; 1];
    let mut output_buffer = Vec::<u8>::new();
    let mut read_buffer = VecDeque::<u8>::new();
    let mut raw_byte_buffer = Vec::<u8>::new();
    let mut offset_len_read_buffer = Vec::<u8>::new();
    let mut header_buffer = Vec::<u8>::new();

    let f = File::open(file_io.encoded_filename.as_path()).unwrap();
    let mut reader = BufReader::new(f);

    let mut decode_state = DecodeParseState::Start;
    let mut header = None;

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
                    DecodeParseState::Start => {
                        header_buffer = vec![v];
                        decode_state = DecodeParseState::ReadingHeaderLen(v);
                    }
                    DecodeParseState::ReadingHeaderLen(first_byte) => {
                        header_buffer.push(v);
                        let header_len = u16::from_be_bytes([first_byte, v]) as usize;
                        decode_state = DecodeParseState::ReadingHeader(header_len - 2);
                    }
                    DecodeParseState::ReadingHeader(remaining) => {
                        header_buffer.push(v);
                        match remaining - 1 {
                            0 => {
                                header = Some(Header::from_bytes(&header_buffer));
                                decode_state = DecodeParseState::ExpectingMatchOrRawChunk;
                            }
                            decr => {
                                decode_state = DecodeParseState::ReadingHeader(decr);
                            }
                        }
                    }
                    DecodeParseState::ExpectingMatchOrRawChunk => {
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
                        raw_byte_buffer.push(v);
                        match remaining - 1 {
                            0 => {
                                if let Some(header) = &header {
                                    let unpacked_bytes = crate::huffman::unpack_bytes(
                                        &raw_byte_buffer,
                                        &header.huffman_tree,
                                    );
                                    read_buffer.extend(unpacked_bytes);
                                    raw_byte_buffer.clear();
                                }
                                match on_finish {
                                    RawByteReadOnFinish::Nothing => (),
                                    RawByteReadOnFinish::FinaliseMatch(offset_len) => {
                                        finalise_match(&mut read_buffer, &offset_len);
                                    }
                                }
                                decode_state = DecodeParseState::ExpectingMatchOrRawChunk
                            }
                            decr => decode_state = DecodeParseState::RawByteChunk(decr, on_finish),
                        }
                    }
                    DecodeParseState::OffsetLenRead(remaining_bytes) => {
                        offset_len_read_buffer.push(v);
                        match remaining_bytes - 1 {
                            0 => {
                                let offset_len = OffsetLen::of_bytes_new(&offset_len_read_buffer);
                                finalise_match(&mut read_buffer, &offset_len);
                                decode_state = DecodeParseState::ExpectingMatchOrRawChunk
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
        DecodeParseState::Start | DecodeParseState::ExpectingMatchOrRawChunk => (),
        DecodeParseState::ReadingHeaderLen(_) | DecodeParseState::ReadingHeader(_) => {
            panic!("Ended parsing file while still reading header");
        }
        DecodeParseState::RawByteChunk(_, RawByteReadOnFinish::Nothing) => {
            panic!("Ended parsing file but still just expecting to read raw bytes");
        }
        DecodeParseState::RawByteChunk(
            num_bytes_left,
            RawByteReadOnFinish::FinaliseMatch(offset_len),
        ) => {
            //If we finish the file with a partial match, we can infer there was some repetition?
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
    Start,
    ReadingHeaderLen(u8),
    ReadingHeader(usize),
    RawByteChunk(u8, RawByteReadOnFinish),
    ExpectingMatchOrRawChunk,
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
