use std::num::NonZero;

/// bit layout of a nibble:
/// xxxxxxxx
/// ^   ^    <---- unused at the moment, eventually will impl sign
///  ^^^     <---- upper nibble
///      ^^^ <---- lower nibble
#[derive(Debug)]
pub struct NibbleNibble(pub u8);

impl NibbleNibble {
    pub fn get_num_bytes(&self) -> (Option<NonZero<usize>>, Option<NonZero<usize>>) {
        let lower_nibble = self.0 & ((1 << 4) - 1);
        let upper_nibble = self.0 >> 4;
        (
            NonZero::new((lower_nibble & 7) as usize),
            NonZero::new((upper_nibble & 7) as usize),
        )
    }
}

impl From<u64> for NibbleNibble {
    fn from(value: u64) -> Self {
        const _1_BYTE: u64 = 1 << 8;
        const _2_BYTE: u64 = 1 << (8 * 2);
        const _3_BYTE: u64 = 1 << (8 * 3);
        const _4_BYTE: u64 = 1 << (8 * 4);
        const _5_BYTE: u64 = 1 << (8 * 5);
        const _6_BYTE: u64 = 1 << (8 * 6);
        const _7_BYTE: u64 = 1 << (8 * 7);
        let num_bytes: Option<NonZero<u8>> = match value {
            0.._1_BYTE => NonZero::new(1_u8),
            _1_BYTE.._2_BYTE => NonZero::new(2_u8),
            _2_BYTE.._3_BYTE => NonZero::new(3_u8),
            _3_BYTE.._4_BYTE => NonZero::new(4_u8),
            _4_BYTE.._5_BYTE => NonZero::new(5_u8),
            _5_BYTE.._6_BYTE => NonZero::new(6_u8),
            _6_BYTE.._7_BYTE => NonZero::new(7_u8),
            _ => None,
        };
        NibbleNibble(num_bytes.map_or(0, |n| n.into()))
    }
}

impl From<(u64, u64)> for NibbleNibble {
    fn from(value: (u64, u64)) -> Self {
        const _1_BYTE: u64 = 1 << 8;
        const _2_BYTE: u64 = 1 << (8 * 2);
        const _3_BYTE: u64 = 1 << (8 * 3);
        const _4_BYTE: u64 = 1 << (8 * 4);
        const _5_BYTE: u64 = 1 << (8 * 5);
        const _6_BYTE: u64 = 1 << (8 * 6);
        const _7_BYTE: u64 = 1 << (8 * 7);
        let num_bytes: Option<NonZero<u8>> = match value.0 {
            0.._1_BYTE => Some(NonZero::new(1_u8).unwrap()),
            _1_BYTE.._2_BYTE => Some(NonZero::new(2_u8).unwrap()),
            _2_BYTE.._3_BYTE => Some(NonZero::new(3_u8).unwrap()),
            _3_BYTE.._4_BYTE => Some(NonZero::new(4_u8).unwrap()),
            _4_BYTE.._5_BYTE => Some(NonZero::new(5_u8).unwrap()),
            _5_BYTE.._6_BYTE => Some(NonZero::new(6_u8).unwrap()),
            _6_BYTE.._7_BYTE => Some(NonZero::new(7_u8).unwrap()),
            _ => None,
        };
        let num_bytes2: Option<NonZero<u8>> = match value.1 {
            0.._1_BYTE => Some(NonZero::new(1_u8).unwrap()),
            _1_BYTE.._2_BYTE => Some(NonZero::new(2_u8).unwrap()),
            _2_BYTE.._3_BYTE => Some(NonZero::new(3_u8).unwrap()),
            _3_BYTE.._4_BYTE => Some(NonZero::new(4_u8).unwrap()),
            _4_BYTE.._5_BYTE => Some(NonZero::new(5_u8).unwrap()),
            _5_BYTE.._6_BYTE => Some(NonZero::new(6_u8).unwrap()),
            _6_BYTE.._7_BYTE => Some(NonZero::new(7_u8).unwrap()),
            _ => None,
        };
        let num_bytes = num_bytes.map_or(0, |n| n.into());
        let num_bytes2 = num_bytes2.map_or(0, |n| n.into());
        NibbleNibble(num_bytes | (num_bytes2 << 4))
    }
}

#[test]
fn nibble_creation() {
    assert_eq!(NibbleNibble::from(200).0, 1);
    assert_eq!(NibbleNibble::from(256).0, 2);
    assert_eq!(NibbleNibble::from(1 << 16).0, 3);
    assert_eq!(NibbleNibble::from(1 << 24).0, 4);
    assert_eq!(NibbleNibble::from(1 << 32).0, 5);
    assert_eq!(NibbleNibble::from(1 << 40).0, 6);
    assert_eq!(NibbleNibble::from(1 << 48).0, 7);
    assert_eq!(NibbleNibble::from(1 << 56).0, 0);

    assert_eq!(NibbleNibble::from((1 << 56, 1 << 56)).0, 0);
    assert_eq!(NibbleNibble::from((1, 1)).0, 17);
}

#[test]
fn nibble_values() {
    let n = NibbleNibble::from((1 << 55, 1));
    let (lower, upper) = n.get_num_bytes();
    assert_eq!(lower.unwrap(), NonZero::new(7).unwrap());
    assert_eq!(upper.unwrap(), NonZero::new(1).unwrap());
}
