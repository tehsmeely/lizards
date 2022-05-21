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

mod test {
    use crate::huffman::{build_tree, tree_to_code_map, ByteStats};
    use std::collections::HashMap;

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
}
