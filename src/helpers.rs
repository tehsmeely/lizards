use log::debug;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, Read};

use crate::huffman::ByteStats;
use crate::{MAX_LOOKBACK_BUFFER_LEN, MAX_READ_BUFFER_LEN};

pub fn read_buffer_to_string(vec: &VecDeque<u8>) -> String {
    let mut v = Vec::new();
    v.extend(vec.as_slices().0);
    v.extend(vec.as_slices().1);
    String::from_utf8(v).unwrap()
}

pub fn step_buffers(
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
                debug!("Got zero bytes");
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
            }
            Ok(n) => {
                panic!("Sadness, got more than 1 byte on [read] : {}", n)
            }
        }
    }
}

pub fn u8_iter_str<'a, I: Iterator<Item = &'a u8>>(i: I) -> String {
    i.map(|x| format!("{:08b}", x))
        .collect::<Vec<String>>()
        .join(", ")
}
