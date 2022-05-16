use std::ops::Range;

#[derive(Debug, PartialEq)]
pub struct OffsetLen {
    offset: u64,
    pub len: u64,
    matched_bytes: Option<Vec<u8>>,
}

impl OffsetLen {
    pub fn new_with_match(offset: u64, len: u64, matched_bytes: Option<Vec<u8>>) -> Self {
        Self {
            offset,
            len,
            matched_bytes,
        }
    }
    const SIZES: [u64; 8] = [
        2u64.pow(8) - 1,
        2u64.pow(16) - 1,
        2u64.pow(24) - 1,
        2u64.pow(32) - 1,
        2u64.pow(40) - 1,
        2u64.pow(48) - 1,
        2u64.pow(56) - 1,
        u64::MAX,
    ];

    fn find_num_bytes(v: u64) -> usize {
        for (i, size) in Self::SIZES.iter().enumerate() {
            if v <= *size {
                return i + 1;
            }
        }
        panic!("BUG: Though a u64 wouldn't fit into any sizes including a u64")
    }
    fn take_bytes(value: u64, num_bytes: usize) -> Vec<u8> {
        let mut output = Vec::<u8>::new();
        for i in 0..num_bytes {
            output.push((value >> (i * 8)) as u8);
        }
        output
    }

    fn value_of_bytes(bytes: &[u8]) -> u64 {
        let mut result: u64 = 0;
        for (i, byte) in bytes.iter().enumerate() {
            result = result | (*byte as u64) << (i * 8);
        }
        result
    }

    pub fn new(offset: u64, len: u64) -> Self {
        Self::new_with_match(offset, len, None)
    }

    pub fn to_bytes_new(&self) -> Vec<u8> {
        // 8 16 24 32 40 48 56 64
        let num_bytes_for_offset = Self::find_num_bytes(self.offset);
        let num_bytes_for_len = Self::find_num_bytes(self.len);

        // We convert each number of bytes into 3 bits
        // (we get 0-7, by subtracting 1 from this number
        //  we never support 0 of either so this gives us 1-8, up to u64 )
        // Then stuff into the first byte
        //  [10aaabbb]
        //  a: offset
        //  b: len
        let num_byte = {
            let offset_bytes = (num_bytes_for_offset - 1) as u8;
            let len_bytes = (num_bytes_for_len - 1) as u8;
            0b10000000 | (offset_bytes << 3) | len_bytes
        };
        let mut result = vec![num_byte];
        // Then bytes: [num_bytes; offset_0; ...; offset_i; len_0; ... len_i]
        // Where 0th is the right hand u8
        // To reconstruct one would do e.g. [offset_2; offset_1; offset_1]
        result.extend(Self::take_bytes(self.offset, num_bytes_for_offset));
        result.extend(Self::take_bytes(self.len, num_bytes_for_len));
        result
    }

    pub fn read_header_byte(header_byte: u8) -> (usize, usize) {
        // Increasing number by 1 as it was decreased when encoded to fit in 3 bits
        let num_bytes_for_offset = (header_byte >> 3 & 0b00000111) as usize + 1;
        let num_bytes_for_len = (header_byte & 0b00000111) as usize + 1;
        (num_bytes_for_offset, num_bytes_for_len)
    }

    pub fn of_bytes_new(bytes: &Vec<u8>) -> Self {
        let len_byte = *bytes.get(0).unwrap();
        let (num_bytes_for_offset, num_bytes_for_len) = Self::read_header_byte(len_byte);
        let expected_num_bytes = 1 + num_bytes_for_offset + num_bytes_for_len;
        if bytes.len() != expected_num_bytes {
            panic!(
                "Did not receive as many bytes ({}) to unpack as expected ({}). {:?}",
                bytes.len(),
                expected_num_bytes,
                bytes
            )
        }
        let offset_bytes = &bytes[1..(1 + num_bytes_for_offset)];
        let len_bytes = &bytes[(1 + num_bytes_for_offset)..];
        let offset = Self::value_of_bytes(offset_bytes);
        let len = Self::value_of_bytes(len_bytes);
        Self {
            offset,
            len,
            matched_bytes: None,
        }
    }

    pub fn to_bytes_debug(&self) -> Vec<u8> {
        let matched_string = if let Some(bytes) = &self.matched_bytes {
            match String::from_utf8(bytes.clone()) {
                Ok(s) => format!("{:?}", s),
                Err(_) => format!("{:?}", bytes),
            }
        } else {
            String::from("No matched bytes recorded")
        };
        let s = format!("({},{}: {})", self.offset, self.len, matched_string);
        s.into_bytes()
    }

    pub fn to_range(&self) -> Range<usize> {
        Range {
            start: self.offset as usize,
            end: self.range_end(),
        }
    }

    pub fn range_end(&self) -> usize {
        (self.offset + self.len) as usize
    }
}

mod test {
    use super::OffsetLen;
    #[test]
    fn offset_len_round_trip() {
        let a = OffsetLen::new(5, 10);
        let bytes = a.to_bytes_new();
        println!("{:?}: {:?}", a, bytes);
        let b = OffsetLen::of_bytes_new(&bytes);
        assert_eq!(a, b)
    }

    #[test]
    fn two_byte_offset_len_round_trip() {
        let a = OffsetLen::new(2047, 2047);
        let bytes = a.to_bytes_new();
        println!("{:?}: {:?}", a, bytes);
        let b = OffsetLen::of_bytes_new(&bytes);
        assert_eq!(a, b)
    }

    #[test]
    fn three_byte_offset_len_round_trip() {
        let a = OffsetLen::new(OffsetLen::SIZES[2], OffsetLen::SIZES[2]);
        let bytes = a.to_bytes_new();
        println!("{:?}: {:?}", a, bytes);
        let b = OffsetLen::of_bytes_new(&bytes);
        assert_eq!(a, b)
    }

    #[test]
    fn all_bytes_offset_len_round_trip() {
        for size in OffsetLen::SIZES {
            let a = OffsetLen::new(size, size);
            let bytes = a.to_bytes_new();
            println!("{:?}: {:?}", a, bytes);
            let b = OffsetLen::of_bytes_new(&bytes);
            assert_eq!(a, b)
        }
    }
}
