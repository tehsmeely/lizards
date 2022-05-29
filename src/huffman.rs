use priority_queue::double_priority_queue::DoublePriorityQueue;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::Entry::Vacant;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::sync::atomic::{AtomicUsize, Ordering};

pub type ByteStats = HashMap<u8, usize>;

#[derive(Debug)]
pub struct CodeMap {
    codes: HashMap<u8, Bits>,
    end_code: Bits,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct HuffmanTree {
    root_node: Option<Box<Node>>,
}

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
struct Node {
    value: Option<u8>, //Only leaves have values
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,

    // end_node handles the case of reading nonsense bytes at the end of decompression
    // TODO: Do away with this once we rework to use [NodeType]
    is_end_node: bool,
}

#[derive(Clone, Hash, PartialEq, Eq)]
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

    // add end_node as lowest frequency pair
    {
        let (node, count) = priority_queue.pop_min().unwrap();
        let combined_node = Node::new_vertex(Some(Box::new(node)), Some(Box::new(Node::new_end())));
        priority_queue.push(combined_node, count);
    }

    // Pick off two lowest, and combine
    while priority_queue.len() > 1 {
        let (node0, count0) = priority_queue.pop_min().unwrap();
        let (node1, count1) = priority_queue.pop_min().unwrap();
        let combined_node = Node::new_vertex(Some(Box::new(node0)), Some(Box::new(node1)));
        priority_queue.push(combined_node, count0 + count1);
    }

    //Now our only element in the queue is our root node
    let (root_node, _count) = priority_queue.pop_min().unwrap();
    tree.root_node = Some(Box::new(root_node));
    tree
}

pub fn tree_to_code_map(tree: &HuffmanTree) -> CodeMap {
    let mut codes = HashMap::new();
    let mut end_code = None;

    fn rec(
        bits: Bits,
        node: &Box<Node>,
        code_map: &mut HashMap<u8, Bits>,
        mut end_code: &mut Option<Bits>,
    ) {
        if let Some(value) = node.value {
            code_map.insert(value, bits.clone());
            return;
        } else if node.is_end_node {
            end_code.insert(bits.clone());
            return;
        }
        if let Some(left_node) = &node.left {
            rec(
                bits.clone_with_increase(true),
                left_node,
                code_map,
                end_code,
            );
        }
        if let Some(right_node) = &node.right {
            rec(
                bits.clone_with_increase(false),
                right_node,
                code_map,
                end_code,
            );
        }
    }

    rec(
        Bits::default(),
        &tree.root_node.as_ref().unwrap(),
        &mut codes,
        &mut end_code,
    );
    CodeMap {
        codes,
        end_code: end_code.unwrap(),
    }
}

pub fn pack_to_u8<I: Iterator<Item = u8>>(code_map: CodeMap, input_stream: I) -> Vec<u8> {
    let mut output = Vec::new();
    let mut working_bytes: u64 = 0;
    let mut bits_left = 64;
    for v in input_stream {
        let value_bits = code_map.codes.get(&v).unwrap();
        if value_bits.bit_size > bits_left {
            //Split up. use the [bits_left] left bits from value_bits, then slap what's left
            // in a new working_bytes

            // if working_bytes = 0b11111100
            // so bits_left = 2;
            // and bits_inserting: 0b00011010, len 5

            // working bytes += bits_inserting >> 5 - 2
            // i.e. stamp 2msb of inserting: 0b000[11]010
            // save working bytes

            // Then take the remaining bits, slide them all the way left, losing the bits we
            // previously wrote so: 0b00011[010] -> 0b[010]00000
            // Rust is not a fan of obliterating bits by over-shifting, so we need to mask
            // mask = 0b11111000
            // working_bytes = bits_inserting << (64 - bits_left)
            // bits_left = 64 - (len - bits_left)

            working_bytes |= (value_bits.set_bits >> (value_bits.bit_size - bits_left));
            output.extend_from_slice(&working_bytes.to_be_bytes());

            //
            println!(
                "{:b}, shifting left by {}",
                value_bits.set_bits,
                (64 - bits_left)
            );
            working_bytes = value_bits.set_bits << (64 - bits_left);
            bits_left = 64 - (value_bits.bit_size - bits_left);
        } else {
            // let working_bytes = 0b11100000;
            // let bits_left = 5;
            // let bits_inserting = 0b101, len 3
            // > shift left by (bits_left - len)
            bits_left -= value_bits.bit_size;
            working_bytes |= value_bits.set_bits << bits_left;
        }

        if bits_left == 0 {
            output.extend_from_slice(&working_bytes.to_be_bytes());
            working_bytes = 0;
            bits_left = 64;
        }
    }
    // put as many bits of END_NODE's code on the end
    if bits_left >= code_map.end_code.bit_size {
        bits_left -= code_map.end_code.bit_size;
        let bits_to_add = code_map.end_code.set_bits << bits_left;
        working_bytes |= bits_to_add;
    }

    // Now stuff what remains in [working_bytes] into output
    let bytes_populated = {
        let floor = (64 - bits_left) / 8;
        if (64 - bits_left) % 8 != 0 {
            floor + 1
        } else {
            floor
        }
    };
    output.extend_from_slice(&working_bytes.to_be_bytes()[0..bytes_populated]);
    output
}

pub fn unpack_bytes(mut input_bytes: Vec<u8>, tree: &HuffmanTree) -> Vec<u8> {
    input_bytes.reverse();
    let bit_stream = BitStream::new(move || input_bytes.pop());
    let mut output = Vec::new();
    let root_node = tree.root_node.as_ref().unwrap();
    let mut current_node = root_node;

    for move_right in bit_stream {
        current_node = if move_right {
            current_node.right.as_ref().unwrap()
        } else {
            current_node.left.as_ref().unwrap()
        };
        if let Some(value) = current_node.value {
            output.push(value);
            current_node = root_node;
        } else if current_node.is_end_node {
            break;
        } else {
            ()
            // keep going
        }
    }
    output
}

impl Node {
    fn new_leaf(v: u8) -> Self {
        Self {
            value: Some(v),
            left: None,
            right: None,
            is_end_node: false,
        }
    }
    fn new_vertex(left: Option<Box<Self>>, right: Option<Box<Self>>) -> Self {
        Self {
            value: None,
            left,
            right,
            is_end_node: false,
        }
    }
    fn new_end() -> Self {
        Self {
            value: None,
            left: None,
            right: None,
            is_end_node: true,
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

impl Debug for Bits {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bits")
            .field("set_bits", &format!("{:064b}", self.set_bits))
            .field("bit_size", &self.bit_size)
            .finish()
    }
}

impl Bits {
    fn clone_with_increase(&self, is_left: bool) -> Self {
        // Some {set_bits:"11"; bit_size:2}, should become {set_bits:"110"; bit_size:3}
        // i.e. it needs to append to the right
        let one_or_zero = // TODO: this is just bool to int ...
            if is_left { 0 } else { 1 };

        let mut new_set_bits = self.set_bits << 1;
        new_set_bits |= one_or_zero;
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

impl HuffmanTree {
    fn to_dot(&self) {
        let mut nodes = Vec::new();
        let mut relationships = Vec::new();
        let mut node_id = AtomicUsize::new(1);

        fn walk(
            nodes: &mut Vec<String>,
            relationships: &mut Vec<String>,
            node: &Option<Box<Node>>,
            parent_id: String,
            node_id: std::rc::Rc<AtomicUsize>,
            is_left: bool,
        ) {
            match node {
                None => return,
                Some(node) => {
                    let this_node_id = format!("node{}", node_id.fetch_add(1, Ordering::Relaxed));
                    let rel_label = if is_left {
                        format!("[label=\"left\"]")
                    } else {
                        format!("[label=\"right\"]")
                    };
                    relationships.push(format!("{} -> {} {};", parent_id, this_node_id, rel_label));
                    if let Some(value) = node.value {
                        let as_str = String::from_utf8(vec![value]);
                        let label = match as_str {
                            Ok(s) => format!("{}({:#09b})", s, value),
                            Err(_) => format!("{:#09b}", value),
                        };
                        nodes.push(format!("{} [label = \"{}\"];", this_node_id, label));
                        return;
                    } else if node.is_end_node {
                        nodes.push(format!("{} [label = \"END\"];", this_node_id));
                    } else {
                        nodes.push(format!("{} [label = \"\"];", this_node_id));
                    }

                    walk(
                        nodes,
                        relationships,
                        &node.left,
                        this_node_id.clone(),
                        node_id.clone(),
                        true,
                    );
                    walk(
                        nodes,
                        relationships,
                        &node.right,
                        this_node_id.clone(),
                        node_id.clone(),
                        false,
                    );
                }
            }
        }
        let root_id = String::from("node0");
        let node_id = std::rc::Rc::new(node_id);
        walk(
            &mut nodes,
            &mut relationships,
            &self.root_node.as_ref().unwrap().left,
            root_id.clone(),
            node_id.clone(),
            true,
        );
        walk(
            &mut nodes,
            &mut relationships,
            &self.root_node.as_ref().unwrap().right,
            root_id,
            node_id.clone(),
            false,
        );

        let start = "digraph G {";
        let end = "}";

        let node_definitions = nodes.join("\n");
        let relationship_definitions = relationships.join("\n");
        println!(
            "{}\n\n{}\n\n{}\n{}",
            start, node_definitions, relationship_definitions, end
        );
    }
}

mod test {
    use crate::huffman::{
        build_tree, pack_to_u8, tree_to_code_map, unpack_bytes, BitStream, ByteStats,
    };
    use std::collections::HashMap;
    use std::io::{BufReader, Read};
    use std::panic::panic_any;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn smol_round_trip() {
        let input = "ABA";

        // ENCODE
        let mut stats = ByteStats::new();
        for byte in input.as_bytes().iter() {
            let mut count = stats.entry(*byte).or_insert(0);
            *count += 1;
        }
        let tree = build_tree(stats);
        println!("Tree: {:?}", tree);
        tree.to_dot();
        let code_map = tree_to_code_map(&tree);
        println!("Code map: {:?}", code_map);
        let encoded_bytes = pack_to_u8(code_map, input.as_bytes().iter().cloned());

        //DECODE
        let output_bytes = unpack_bytes(encoded_bytes, &tree);
        let output_string = String::from_utf8(output_bytes).unwrap();

        //Check
        assert_eq!(input, &output_string);
        ()
    }
    #[test]
    fn round_trip() {
        // This example string courtesy of the wikipedia page on huffman coding
        // https://en.wikipedia.org/wiki/Huffman_coding
        let input = "A_DEAD_DAD_CEDED_A_BAD_BABE_A_BEADED_ABACA_BED";

        // ENCODE
        let mut stats = ByteStats::new();
        for byte in input.as_bytes().iter() {
            let mut count = stats.entry(*byte).or_insert(0);
            *count += 1;
        }
        let tree = build_tree(stats);
        tree.to_dot();
        let code_map = tree_to_code_map(&tree);
        let encoded_bytes = pack_to_u8(code_map, input.as_bytes().iter().cloned());

        for byte in encoded_bytes.iter() {
            println!("{:08b}", byte);
        }

        //DECODE
        let output_bytes = unpack_bytes(encoded_bytes, &tree);
        let output_string = String::from_utf8(output_bytes).unwrap();

        //Check
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
