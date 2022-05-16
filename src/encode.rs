use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, BufWriter};

use crate::offset_len::OffsetLen;
use crate::output_stream::OutputStream;
use crate::{helpers, EncodedValue, MAX_READ_BUFFER_LEN, MIN_MATCH_SIZE};

pub fn encode(fname: &str) {
    println!("Lizards!");

    let mut input_buffer: [u8; 1] = [0b0; 1];
    let mut read_buffer = VecDeque::<u8>::new();
    let mut lookback_buffer = VecDeque::<u8>::new();

    //let mut encoded_values: Vec<EncodedValue> = Vec::new();
    let outf = File::create(format!("{}.lizard", fname)).unwrap();
    let df = File::create(format!("{}.dblzd", fname)).unwrap();
    let mut writer = BufWriter::new(outf);
    let mut debug_writer = BufWriter::new(df);
    let mut output_stream = OutputStream::new_debug(writer, debug_writer);

    let f = File::open(format!("{}.txt", fname)).unwrap();
    let mut reader = BufReader::new(f);

    //Init read buffer
    for _i in 0..MAX_READ_BUFFER_LEN {
        helpers::step_buffers(
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
            EncodedValue::OffsetLen(OffsetLen { len, .. }) => len as usize,
        };
        //encoded_values.push(next_value);
        output_stream.add(next_value);
        helpers::step_buffers(
            step_size,
            &mut reader,
            &mut input_buffer,
            &mut read_buffer,
            &mut lookback_buffer,
            true,
        );
    }
    output_stream.finalise();
    println!("Done");
}

fn find_match(read_buffer: &VecDeque<u8>, lookback_buffer: &VecDeque<u8>) -> EncodedValue {
    // TODO support the max values in the OffsetLen
    let total_len = read_buffer.len() + lookback_buffer.len();
    // Current match: offset, matched bytes
    // TODO: Type this up a bit?
    let mut current_match = (0, Vec::new());
    let mut best_match: Option<(usize, Vec<u8>)> = None;
    for i in 0..total_len {
        // TODO: Disabled looking ahead into read_buffer because repetitions into it are broken
        if i >= lookback_buffer.len() {
            break;
        }
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
    match best_match {
        None => EncodedValue::RawU8(*read_buffer.front().unwrap()),
        Some((_, matched_values)) if matched_values.len() < MIN_MATCH_SIZE => {
            EncodedValue::RawU8(*read_buffer.front().unwrap())
        }
        Some((offset, matched_values)) => EncodedValue::OffsetLen(OffsetLen::new_with_match(
            offset as u64,
            matched_values.len() as u64,
            Some(matched_values),
        )),
    }
}
