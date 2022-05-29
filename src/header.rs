use crate::huffman::HuffmanTree;
use std::collections::VecDeque;
use std::convert::TryInto;

#[derive(Debug, PartialEq)]
pub struct Header {
    huffman_tree: HuffmanTree,
    lookback_buffer_len: u64,
}

impl Header {
    pub fn to_bytes(&self) -> Vec<u8> {
        let serialised_tree = rmp_serde::to_vec(&self.huffman_tree).unwrap();
        // Total len is tree serialised length, [lookback_buffer_len] usize, and the size byte
        // this will go into
        let total_len = serialised_tree.len() + 8 + 1;
        if total_len > (u8::MAX as usize) {
            panic!("length byte not enough, consider using >u8");
        }

        let mut output = vec![total_len as u8];
        output.extend_from_slice(&self.lookback_buffer_len.to_be_bytes());
        output.extend(serialised_tree.iter());
        output
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        //Assert bytes is correctly sized
        let len = bytes[0];
        if (len as usize) != bytes.len() {
            panic!("Not enough bytes!");
        }

        let be_bytes: [u8; 8] = (&bytes[1..9]).try_into().unwrap();
        let lookback_buffer_len = u64::from_be_bytes(be_bytes);
        let huffman_tree = rmp_serde::from_slice::<HuffmanTree>(&bytes[9..]).unwrap();

        Self {
            huffman_tree,
            lookback_buffer_len,
        }
    }
}

mod test {
    use crate::header::Header;
    use crate::MAX_LOOKBACK_BUFFER_LEN;

    #[test]
    fn test() {
        // This example string courtesy of the wikipedia page on huffman coding
        // https://en.wikipedia.org/wiki/Huffman_coding
        let input = "A_DEAD_DAD_CEDED_A_BAD_BABE_A_BEADED_ABACA_BED";
        // ENCODE
        let mut stats = crate::huffman::ByteStats::new();
        for byte in input.as_bytes().iter() {
            let mut count = stats.entry(*byte).or_insert(0);
            *count += 1;
        }
        let huffman_tree = crate::huffman::build_tree(stats);
        let header = Header {
            huffman_tree,
            lookback_buffer_len: MAX_LOOKBACK_BUFFER_LEN as u64,
        };

        let header_as_bytes = header.to_bytes();
        assert_eq!(63, header_as_bytes.len());
        let output_header = Header::from_bytes(header_as_bytes);

        assert_eq!(header, output_header);
    }
}
