use priority_queue::double_priority_queue::DoublePriorityQueue;
use std::collections::HashMap;

pub type ByteStats = HashMap<u8, usize>;

#[derive(Debug)]
pub struct CodeMap(pub HashMap<u8, Bits>);

#[derive(Debug)]
pub struct HuffmanTree {
    root_node: Option<Box<Node>>,
}

#[derive(Debug, Hash, PartialEq, Eq)]
struct Node {
    value: Option<u8>, //Only leaves have values
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
/// Supports an encoded bit pattern from two bits (e.g. 10) up to 64 bits
/// [bit_size] informs the user how many on the least signifigant bits of [set_bits] to care about
pub struct Bits {
    set_bits: u64,
    bit_size: usize,
}

pub fn build_tree(stats: ByteStats) -> HuffmanTree {
    let mut tree: HuffmanTree = HuffmanTree { root_node: None };
    let mut priority_queue: DoublePriorityQueue<Node, usize> = DoublePriorityQueue::new();

    for (val, count) in stats.iter() {
        priority_queue.push(Node::new_leaf(*val), *count);
    }

    // Pick off two lowest, and combine
    while priority_queue.len() > 1 {
        let (node0, count0) = priority_queue.pop_min().unwrap();
        let (node1, count1) = priority_queue.pop_min().unwrap();
        // By convention, lowest is left
        let combined_node = Node::new_vertex(Some(Box::new(node0)), Some(Box::new(node1)));
        priority_queue.push(combined_node, count0 + count1);
    }

    //Now our only element in the queue is our root node
    let (root_node, _count) = priority_queue.pop_min().unwrap();
    tree.root_node = Some(Box::new(root_node));
    tree
}

pub fn tree_to_code_map(tree: HuffmanTree) -> CodeMap {
    let mut code_map = CodeMap::new();

    fn rec(bits: Bits, node: &Box<Node>, code_map: &mut CodeMap) {
        if let Some(value) = node.value {
            code_map.0.insert(value, bits.clone());
            return;
        }
        if let Some(left_node) = &node.left {
            rec(bits.clone_with_increase(true), left_node, code_map);
        }
        if let Some(right_node) = &node.right {
            rec(bits.clone_with_increase(false), right_node, code_map);
        }
    }

    rec(Bits::default(), &tree.root_node.unwrap(), &mut code_map);
    code_map
}

pub fn pack_to_u8<I: Iterator<Item = u8>>(code_map: CodeMap, input_stream: I) -> Vec<u8> {
    let mut output = Vec::new();
    let mut working_bytes: u64 = 0;
    let mut bits_left = 64;
    for v in input_stream {
        let value_bits = code_map.0.get(&v).unwrap();
        if value_bits.bit_size > bits_left {
            //Split up. use the [bits_left] left bits from value_bits, then slap what's left
            // in a new working_bytes

            // if working_bytes = 0b11111100
            // so bits_left = 2;
            // and bits_inserting: 0b00011010, len 5

            // working bytes += bits_inserting >> 5 - 2
            // save working bytes
            // working_bytes = bits_inserting << (64 - bits_left)
            // bits_left = 64 - (len - bits_left)

            working_bytes =
                working_bytes | (value_bits.set_bits >> (value_bits.bit_size - bits_left));
            output.extend_from_slice(&working_bytes.to_be_bytes());
            working_bytes = value_bits.set_bits << (64 - bits_left);
            bits_left = 64 - (value_bits.bit_size - bits_left);
        } else {
            // let working_bytes = 0b11100000;
            // let bits_left = 5;
            // let bits_inserting = 0b101, len 3
            // > shift left by (bits_left - len)
            bits_left -= value_bits.bit_size;
            let bits_to_add = value_bits.set_bits << bits_left;
            working_bytes | bits_to_add;
        }

        if bits_left == 0 {
            output.extend_from_slice(&working_bytes.to_be_bytes());
        }
    }
    output
}

impl CodeMap {
    fn new() -> Self {
        CodeMap(HashMap::new())
    }
}

impl Node {
    fn new_leaf(v: u8) -> Self {
        Self {
            value: Some(v),
            left: None,
            right: None,
        }
    }
    fn new_vertex(left: Option<Box<Self>>, right: Option<Box<Self>>) -> Self {
        Self {
            value: None,
            left,
            right,
        }
    }
}

impl Default for Bits {
    fn default() -> Self {
        Self {
            set_bits: 0,
            bit_size: 0,
        }
    }
}

impl Bits {
    fn clone_with_increase(&self, is_left: bool) -> Self {
        // Some {set_bits:"10"; bit_size:2}, should become {set_bits:"110"; bit_size:3}
        let new_mask = {
            let one_or_zero = // TODO: this is just bool to int ...
                if is_left { 0 } else { 1 };
            one_or_zero << self.bit_size
        };
        let new_set_bits = self.set_bits | new_mask;
        Self {
            set_bits: new_set_bits,
            bit_size: self.bit_size + 1,
        }
    }
}

struct BitStream<F: FnMut() -> Option<u8>> {
    current_byte: u8,
    byte_pos: u8,
    read_byte: F,
}

impl<F: FnMut() -> Option<u8>> BitStream<F> {
    fn new(read_byte: F) -> Self {
        Self {
            current_byte: 0,
            byte_pos: 8,
            read_byte,
        }
    }
}

impl<F: FnMut() -> Option<u8>> Iterator for BitStream<F> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        // When bytepos = 0, we want to read the MSB, i.e. shift 0b00000001 left to
        // 0b10000000 (i.e. shift left 7)

        // Allowed byte_pos values are 1 to 8. when we extend past it, we read and reset
        self.byte_pos += 1;

        if self.byte_pos > 8 {
            match (self.read_byte)() {
                Some(new_byte) => {
                    // Only resetting byte_pos here so if next() is called repeatedly when
                    // F returns none it won't reset
                    self.byte_pos = 1;
                    self.current_byte = new_byte
                }
                None => return None,
            }
        }

        let mask = 1 << (8 - self.byte_pos);
        Some(mask & self.current_byte > 0)
    }
}

mod test {
    use crate::huffman::{build_tree, tree_to_code_map, BitStream, ByteStats};
    use std::collections::HashMap;
    use std::io::{BufReader, Read};
    use std::panic::panic_any;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn round_trip() {
        // This example string courtesy of the wikipedia page on huffman coding
        // https://en.wikipedia.org/wiki/Huffman_coding
        let input = "A_DEAD_DAD_CEDED_A_BAD_BABE_A_BEADED_ABACA_BED";

        let mut stats = ByteStats::new();
        for byte in input.as_bytes().iter() {
            let mut count = stats.entry(*byte).or_insert(0);
            *count += 1;
        }

        let tree = build_tree(stats);
        let code_map = tree_to_code_map(tree);

        let encoded_bytes = {
            let mut bytes = Vec::new();
            for byte in input.as_bytes().iter() {
                let bits = code_map.0.get(byte).unwrap();
                bytes.push(bits.clone());
            }
            bytes
        };

        let inverse_code_map = {
            let mut map = HashMap::new();
            for (val, bits) in code_map.0.iter() {
                map.insert(bits.clone(), *val);
            }
            map
        };
        let mut output = Vec::new();
        for bits in encoded_bytes.iter() {
            let val = inverse_code_map.get(bits).unwrap();
            output.push(*val);
        }
        let output_string = String::from_utf8(output).unwrap();
        assert_eq!(input, &output_string);
        ()
    }

    #[test]
    fn bitstream_test() {
        let test_count = AtomicUsize::new(0);
        let mut bitstream = BitStream::new(|| match test_count.fetch_add(1, Ordering::Relaxed) {
            0 => Some(0b10100101),
            1 => Some(0b11110000),
            _ => None,
        });
        let as_bools: Vec<bool> = bitstream.collect();
        let expected = {
            let mut first_byte = vec![true, false, true, false, false, true, false, true];
            let mut second_byte = vec![true, true, true, true, false, false, false, false];
            first_byte.append(&mut second_byte);
            first_byte
        };

        assert_eq!(expected, as_bools);
    }

    #[test]
    fn bitstream_with_reader() {
        let data: [u8; 2] = [0b11111111, 0b10101010];
        let mut buffreader = BufReader::new(&data[..]);
        let mut buf: [u8; 1] = [0];

        let mut bitstream = BitStream::new(|| {
            let read = buffreader.read(&mut buf);
            match read {
                Ok(1) => Some(buf[0]),
                Ok(0) => None,
                Ok(invalid_num_bytes) => panic!(
                    "Bug, read invalid number of bytes for buff: {}",
                    invalid_num_bytes
                ),
                Err(e) => panic!("Error reading bytes: {}", e),
            }
        });
        let as_bools: Vec<bool> = bitstream.collect();
        let expected = {
            let mut first_byte = vec![true, true, true, true, true, true, true, true];
            let mut second_byte = vec![true, false, true, false, true, false, true, false];
            first_byte.append(&mut second_byte);
            first_byte
        };

        assert_eq!(expected, as_bools);
    }
}
