use std::fs::File;
use std::io::{BufWriter, Write};

use crate::{ChunkMarker, EncodedValue};

pub struct OutputStream {
    buf: Vec<u8>,
    output: BufWriter<File>,
    debug_output: Option<BufWriter<File>>,
}

impl OutputStream {
    // This max is driven by what we can fit in the chunk marker bytes
    // since we use the two left bits to identify it, the value is 2^6 -1
    const MAX_CHUNK_LEN: usize = 63;
    pub fn new(output: BufWriter<File>) -> Self {
        Self {
            buf: Vec::new(),
            output,
            debug_output: None,
        }
    }
    pub fn new_debug(output: BufWriter<File>, debug_output: BufWriter<File>) -> Self {
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

    pub fn add(&mut self, value: EncodedValue) {
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
    pub fn finalise(&mut self) {
        if !self.buf.is_empty() {
            self.end_chunk()
        }
    }
}
