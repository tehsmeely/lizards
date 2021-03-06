use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::ops::Range;
use std::path::PathBuf;

use clap::{Args, Parser};
use log::info;

use file_io::FileInputOutput;
use offset_len::OffsetLen;
use output_stream::OutputStream;
use std::str::FromStr;

mod decode;
mod encode;
mod file_io;
mod header;
mod helpers;
mod huffman;
mod offset_len;
mod output_stream;

const MAX_LOOKBACK_BUFFER_LEN: usize = 1000;
const MAX_READ_BUFFER_LEN: usize = 400;

// It's not worth doing matches under a size where offset_len would take up more space
const MIN_MATCH_SIZE: usize = 4;

const DEBUG: bool = false;

#[derive(Args, Debug)]
struct CommandLineArgs {
    /// Input filename
    #[clap(short, long)]
    filename: String,

    /// Output filename. If not provided, will be derived based on input filename
    #[clap(short = 'O', long)]
    output_filename: Option<String>,

    /// By default if [output_filename] exists command will not run unless providing [overwrite]
    #[clap(long, short)]
    overwrite: bool,
}

#[derive(Args, Debug)]
struct CompressSpecificArgs {
    #[clap(flatten)]
    common: CommandLineArgs,

    /// Max bytes used as a buffer when compressing. Higher size will use more RAM to run but
    /// should result in better compression
    #[clap(long, default_value_t = MAX_LOOKBACK_BUFFER_LEN)]
    buffer_size: usize,
}
#[derive(Args, Debug)]
struct DecompressSpecificArgs {
    #[clap(flatten)]
    common: CommandLineArgs,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
enum CommandLineSubCommand {
    /// Compress a file to the lizards format
    Compress(CompressSpecificArgs),

    /// Decompress a lizards compressed file
    Decompress(DecompressSpecificArgs),
}

fn main() {
    match CommandLineSubCommand::parse() {
        CommandLineSubCommand::Compress(args) => {
            let file_input_output = FileInputOutput::new_from_unencoded(
                &args.common.filename,
                args.common.output_filename.as_deref(),
                true,
            );
            file_input_output.input_is_valid(true).unwrap();
            file_input_output
                .output_is_valid(true, args.common.overwrite)
                .unwrap();

            encode::encode(&file_input_output);
        }
        CommandLineSubCommand::Decompress(args) => {
            let file_input_output = FileInputOutput::new_from_encoded(
                &args.common.filename,
                args.common.output_filename.as_deref(),
            );

            file_input_output.input_is_valid(false).unwrap();
            file_input_output
                .output_is_valid(false, args.common.overwrite)
                .unwrap();
            decode::decode(&file_input_output);
        }
    }
}

pub enum EncodedValue {
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
    pub const MAX_VALUE: usize = 0b00111111;
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
