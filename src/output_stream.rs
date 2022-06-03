use std::fs::File;
use std::io::{BufWriter, Write};

use crate::header::Header;
use crate::huffman::CodeMap;
use crate::{ChunkMarker, EncodedValue};

pub struct OutputStream<W: Write> {
    buf: Vec<u8>,
    output: BufWriter<W>,
    debug_output: Option<BufWriter<File>>,
    code_map: CodeMap,
}

impl<W: Write> OutputStream<W> {
    pub fn new(
        code_map: CodeMap,
        output: BufWriter<W>,
        debug_output: Option<BufWriter<File>>,
    ) -> Self {
        Self {
            buf: Vec::new(),
            output,
            debug_output,
            code_map,
        }
    }

    fn end_chunk(&mut self) {
        let bytes = crate::huffman::pack_to_u8(&self.code_map, self.buf.iter().map(|x| *x));
        //split into chunks of max size the size we can fit into one chunk marker
        for chunk in bytes.chunks(ChunkMarker::MAX_VALUE) {
            let chunk_marker = ChunkMarker {
                len: chunk.len() as u8,
            };
            self.output.write(&[chunk_marker.to_u8()]);
            self.output.write_all(chunk);
            if let Some(writer) = &mut self.debug_output {
                writer.write_all(&chunk_marker.to_debug_bytes());
                //TODO: Writing buf here is a lie if there are >1 chunks as buf is everything
                // This is hard to do because we don't know how many actual bytes we've fitted into
                // the chunks. Solution would be to make [huffman:pack_to_u8] give us chunks with
                // some char size data
                let bytes: String = chunk
                    .iter()
                    .map(|x| format!("{:08b}", x))
                    .collect::<Vec<String>>()
                    .join("");
                writer.write_all(&bytes.into_bytes());
                writer.write_all(&self.buf);
            }
        }
        self.buf.clear();
    }

    pub fn write_header(&mut self, header: &Header) {
        self.output.write_all(&header.to_bytes());
        if let Some(writer) = &mut self.debug_output {
            writer.write_all(&header.to_debug_bytes());
        }
    }

    pub fn add(&mut self, value: &EncodedValue) {
        match value {
            EncodedValue::RawU8(v) => {
                self.buf.push(*v);
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
        self.output.flush();
        if let Some(writer) = &mut self.debug_output {
            writer.flush();
        }
    }
}

mod test {
    use std::collections::HashMap;
    use std::io::{BufWriter, Write};

    use crate::huffman::{Bits, CodeMap};
    use crate::output_stream::OutputStream;
    use crate::{helpers, EncodedValue};

    #[test]
    fn expected_output() {
        let mut output_buf = Vec::new();
        {
            let mut output_writer = BufWriter::new(&mut output_buf);
            let code_map = {
                let mut codes = HashMap::new();
                codes.insert(0b00000001, Bits::from((0b00001011, 4)));
                codes.insert(0b00000010, Bits::from((0b00001001, 4)));
                let end_code = Bits::from((0b00001111, 4));
                CodeMap::new(codes, end_code)
            };
            let mut output_stream = OutputStream::new(code_map, output_writer, None);

            let values: [u8; 4] = [1, 2, 1, 1];
            for value in values.iter() {
                output_stream.add(&EncodedValue::RawU8(*value));
            }
            output_stream.finalise();
        }
        let expected = {
            //The chunk marker for 3 bytes
            let chunk_marker: u8 = 0b11000011;
            let encoded_bit: Vec<u8> = vec![chunk_marker, 0b10111001, 0b10111011, 0b11110000];
            helpers::u8_iter_str(encoded_bit.iter())
        };
        assert_eq!(expected, helpers::u8_iter_str(output_buf.iter()));
    }
}
