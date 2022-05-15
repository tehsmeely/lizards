use std::ops::Range;

#[derive(Debug, PartialEq)]
pub struct OffsetLen {
    offset: u16,
    pub len: u16,
    matched_bytes: Option<Vec<u8>>,
}

impl OffsetLen {
    const MAX_VALUE: u16 = 2047;
    pub fn new_with_match(offset: u16, len: u16, matched_bytes: Option<Vec<u8>>) -> Self {
        if offset > Self::MAX_VALUE {
            panic!("Offset above max value: {}", offset);
        }
        if len > Self::MAX_VALUE {
            panic!("Len above max value: {}", len);
        }
        Self {
            offset,
            len,
            matched_bytes,
        }
    }

    pub fn new(offset: u16, len: u16) -> Self {
        Self::new_with_match(offset, len, None)
    }

    /* Stuffing into 3 bytes:
    b0        b1        b2
    [10aaaaaa][aaaaabbb][bbbbbbbb]
    each is 11 bits

    a and b are u16 so
    [_____aaaaaaaaaaa]
    [_____bbbbbbbbbbb]

    TODO: Consider using first byte as [num_bytes] instead for bigger values
     */
    pub fn to_bytes_new(&self) -> [u8; 3] {
        let b0 = ((self.offset >> 5) as u8) | 0b10000000;
        let b1 = ((self.offset << 3) as u8) | ((self.len >> 8) as u8);
        let b2 = self.len as u8;
        [b0, b1, b2]
    }

    pub fn of_bytes_new(b0: u8, b1: u8, b2: u8) -> Self {
        let mask = 0b0000011111111111;
        let a = ((b0 as u16) << 5) | (b1 >> 3) as u16;
        let offset = a & mask;
        let b = ((b1 as u16) << 8) | (b2 as u16);
        let len = b & mask;
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
        let [a0, a1, a2] = a.to_bytes_new();
        let b = OffsetLen::of_bytes_new(a0, a1, a2);
        assert_eq!(a, b)
    }

    #[test]
    fn big_offset_len_round_trip() {
        let a = OffsetLen::new(2047, 2047);
        let [a0, a1, a2] = a.to_bytes_new();
        let b = OffsetLen::of_bytes_new(a0, a1, a2);
        assert_eq!(a, b)
    }
}
