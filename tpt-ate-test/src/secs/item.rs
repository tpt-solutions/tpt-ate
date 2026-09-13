//! SECS-II (SEMI E5) data items: the typed value model every SECS message body
//! is built from, plus the wire encoder/decoder.
//!
//! Item wire format: one format byte `(format_code << 2) | length_bits`, then
//! `length_bits` (1-3) big-endian bytes giving the element count, then the
//! elements. Lists (format 0x00) contain that many nested items; every other
//! format is a flat array of fixed-width elements (A/B/J count bytes).

use crate::secs::error::{Result, SecsError};
use std::fmt;

/// Format codes from SEMI E5, Table 1 (upper 6 bits of the item header byte,
/// shifted left two in the wire encoding).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FormatCode {
    List = 0x00,
    Binary = 0x20,
    Boolean = 0x24,
    Ascii = 0x40,
    Jis8 = 0x44,
    I8 = 0x60,
    I1 = 0x64,
    I2 = 0x68,
    I4 = 0x70,
    F8 = 0x80,
    F4 = 0x84,
    U8 = 0xA0,
    U1 = 0xA4,
    U2 = 0xA8,
    U4 = 0xB0,
}

impl FormatCode {
    fn from_byte(header: u8) -> Option<Self> {
        let code = header & 0b1111_1100;
        match code {
            0x00 => Some(FormatCode::List),
            0x20 => Some(FormatCode::Binary),
            0x24 => Some(FormatCode::Boolean),
            0x40 => Some(FormatCode::Ascii),
            0x44 => Some(FormatCode::Jis8),
            0x60 => Some(FormatCode::I8),
            0x64 => Some(FormatCode::I1),
            0x68 => Some(FormatCode::I2),
            0x70 => Some(FormatCode::I4),
            0x80 => Some(FormatCode::F8),
            0x84 => Some(FormatCode::F4),
            0xA0 => Some(FormatCode::U8),
            0xA4 => Some(FormatCode::U1),
            0xA8 => Some(FormatCode::U2),
            0xB0 => Some(FormatCode::U4),
            _ => None,
        }
    }
}

/// A SECS-II data item. Every variant holds a homogeneous array — SECS-II has
/// no scalars, only length-1 arrays.
#[derive(Debug, Clone, PartialEq)]
pub enum SecsItem {
    List(Vec<SecsItem>),
    Binary(Vec<u8>),
    Boolean(Vec<bool>),
    /// ASCII text (format `A`).
    Ascii(String),
    /// JIS-8 encoded data (format `J`), carried opaquely.
    Jis(Vec<u8>),
    I8(Vec<i64>),
    I1(Vec<i8>),
    I2(Vec<i16>),
    I4(Vec<i32>),
    F8(Vec<f64>),
    F4(Vec<f32>),
    U8(Vec<u64>),
    U1(Vec<u8>),
    U2(Vec<u16>),
    U4(Vec<u32>),
}

impl SecsItem {
    /// Convenience constructor: a one-element unsigned 4-byte item.
    pub fn u4(value: u32) -> SecsItem {
        SecsItem::U4(vec![value])
    }

    /// Convenience constructor: a one-element unsigned 2-byte item.
    pub fn u2(value: u16) -> SecsItem {
        SecsItem::U2(vec![value])
    }

    /// Convenience constructor: ASCII text.
    pub fn a(text: impl Into<String>) -> SecsItem {
        SecsItem::Ascii(text.into())
    }

    /// First element of a `U4` item, if this is one.
    pub fn as_u4(&self) -> Option<u32> {
        match self {
            SecsItem::U4(v) => v.first().copied(),
            _ => None,
        }
    }

    /// First element of a `U2` item, if this is one.
    pub fn as_u2(&self) -> Option<u16> {
        match self {
            SecsItem::U2(v) => v.first().copied(),
            _ => None,
        }
    }

    /// First element of a `U1` item, if this is one.
    pub fn as_u1(&self) -> Option<u8> {
        match self {
            SecsItem::U1(v) => v.first().copied(),
            _ => None,
        }
    }

    /// First element of an `F4` item, if this is one.
    pub fn as_f4(&self) -> Option<f32> {
        match self {
            SecsItem::F4(v) => v.first().copied(),
            _ => None,
        }
    }

    /// The ASCII text of an `A` item, if this is one.
    pub fn as_ascii(&self) -> Option<&str> {
        match self {
            SecsItem::Ascii(s) => Some(s),
            _ => None,
        }
    }

    /// The elements of a list item, if this is one.
    pub fn as_list(&self) -> Option<&[SecsItem]> {
        match self {
            SecsItem::List(items) => Some(items),
            _ => None,
        }
    }

    /// Encode this item to the SECS-II wire format.
    pub fn encode(&self, out: &mut Vec<u8>) {
        match self {
            SecsItem::List(items) => {
                push_header(FormatCode::List, items.len(), out);
                for item in items {
                    item.encode(out);
                }
            }
            SecsItem::Binary(values) => push_fixed(FormatCode::Binary, values, 1, out),
            SecsItem::Boolean(values) => {
                let bytes: Vec<u8> = values.iter().map(|&b| b as u8).collect();
                push_fixed(FormatCode::Boolean, &bytes, 1, out);
            }
            SecsItem::Ascii(text) => {
                push_header(FormatCode::Ascii, text.len(), out);
                out.extend_from_slice(text.as_bytes());
            }
            SecsItem::Jis(bytes) => push_fixed(FormatCode::Jis8, bytes, 1, out),
            SecsItem::I8(values) => push_fixed(FormatCode::I8, values, 8, out),
            SecsItem::I1(values) => push_fixed(FormatCode::I1, values, 1, out),
            SecsItem::I2(values) => push_fixed(FormatCode::I2, values, 2, out),
            SecsItem::I4(values) => push_fixed(FormatCode::I4, values, 4, out),
            SecsItem::F8(values) => push_fixed(FormatCode::F8, values, 8, out),
            SecsItem::F4(values) => push_fixed(FormatCode::F4, values, 4, out),
            SecsItem::U8(values) => push_fixed(FormatCode::U8, values, 8, out),
            SecsItem::U1(values) => push_fixed(FormatCode::U1, values, 1, out),
            SecsItem::U2(values) => push_fixed(FormatCode::U2, values, 2, out),
            SecsItem::U4(values) => push_fixed(FormatCode::U4, values, 4, out),
        }
    }

    /// Encode to a fresh `Vec<u8>`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode(&mut out);
        out
    }

    /// Decode one item from the front of `data`. Returns the item and how many
    /// bytes it consumed. Fails if `data` does not contain exactly one item.
    pub fn from_bytes(data: &[u8]) -> Result<(SecsItem, usize)> {
        let (item, consumed) = decode_item(data)?;
        if consumed != data.len() {
            return Err(SecsError::TrailingBytesAfterItem { count: data.len() - consumed });
        }
        Ok((item, consumed))
    }
}

/// Write the item header: format byte + 1-3 length bytes.
fn push_header(format: FormatCode, length: usize, out: &mut Vec<u8>) {
    // Largest count encodable in 3 bytes; SECS-II allows 1-3 length bytes only.
    assert!(length <= 0x00FF_FFFF, "SECS-II item length {length} exceeds 3-byte encoding");
    let (bits, bytes): (u8, [u8; 3]) = if length <= 0xFF {
        (1, [length as u8, 0, 0])
    } else if length <= 0xFFFF {
        (2, [(length >> 8) as u8, length as u8, 0])
    } else {
        (3, [(length >> 16) as u8, (length >> 8) as u8, length as u8])
    };
    out.push((format as u8) | bits);
    out.extend_from_slice(&bytes[..bits as usize]);
}

/// Encode a fixed-width numeric/array format: header then big-endian elements.
fn push_fixed<T: Encodable>(format: FormatCode, values: &[T], elem_size: usize, out: &mut Vec<u8>) {
    push_header(format, values.len() * elem_size, out);
    for v in values {
        v.encode_into(out);
    }
}

/// Big-endian element encoding for every fixed-width type we carry.
trait Encodable {
    fn encode_into(&self, out: &mut Vec<u8>);
}

macro_rules! impl_encodable {
    ($($ty:ty),*) => {
        $(impl Encodable for $ty {
            fn encode_into(&self, out: &mut Vec<u8>) {
                out.extend_from_slice(&self.to_be_bytes());
            }
        })*
    };
}

impl_encodable!(u8, u16, u32, u64, i8, i16, i32, i64, f32, f64);

fn decode_item(data: &[u8]) -> Result<(SecsItem, usize)> {
    let (&header, rest) =
        data.split_first().ok_or(SecsError::UnexpectedEof { wanted: 1, had: 0 })?;
    let format = FormatCode::from_byte(header).ok_or(SecsError::InvalidItemHeader(header))?;
    let length_bits = header & 0b11;
    if length_bits == 0 {
        return Err(SecsError::InvalidItemLengthBits(0));
    }
    if rest.len() < length_bits as usize {
        return Err(SecsError::UnexpectedEof { wanted: length_bits as usize, had: rest.len() });
    }
    let mut length = 0usize;
    for &b in &rest[..length_bits as usize] {
        length = (length << 8) | b as usize;
    }
    let body = &rest[length_bits as usize..];
    if format == FormatCode::List {
        let mut cursor = body;
        let mut items = Vec::with_capacity(length.min(4096));
        for _ in 0..length {
            let (item, used) = decode_item(cursor)?;
            items.push(item);
            cursor = &cursor[used..];
        }
        let consumed = data.len() - cursor.len();
        Ok((SecsItem::List(items), consumed))
    } else {
        if length > body.len() {
            return Err(SecsError::ItemLengthOverflow { length, remaining: body.len() });
        }
        let bytes = &body[..length];
        let item = match format {
            FormatCode::Binary => SecsItem::Binary(bytes.to_vec()),
            FormatCode::Boolean => SecsItem::Boolean(bytes.iter().map(|&b| b != 0).collect()),
            FormatCode::Ascii => SecsItem::Ascii(String::from_utf8_lossy(bytes).into_owned()),
            FormatCode::Jis8 => SecsItem::Jis(bytes.to_vec()),
            FormatCode::I8 => SecsItem::I8(decode_fixed::<i64, 8>(bytes)),
            FormatCode::I1 => SecsItem::I1(decode_fixed::<i8, 1>(bytes)),
            FormatCode::I2 => SecsItem::I2(decode_fixed::<i16, 2>(bytes)),
            FormatCode::I4 => SecsItem::I4(decode_fixed::<i32, 4>(bytes)),
            FormatCode::F8 => SecsItem::F8(decode_fixed::<f64, 8>(bytes)),
            FormatCode::F4 => SecsItem::F4(decode_fixed::<f32, 4>(bytes)),
            FormatCode::U8 => SecsItem::U8(decode_fixed::<u64, 8>(bytes)),
            FormatCode::U1 => SecsItem::U1(bytes.to_vec()),
            FormatCode::U2 => SecsItem::U2(decode_fixed::<u16, 2>(bytes)),
            FormatCode::U4 => SecsItem::U4(decode_fixed::<u32, 4>(bytes)),
            FormatCode::List => unreachable!("list handled above"),
        };
        Ok((item, 1 + length_bits as usize + length))
    }
}

/// Decode big-endian fixed-width elements of `size` bytes each.
fn decode_fixed<T: Decodable, const SIZE: usize>(bytes: &[u8]) -> Vec<T> {
    bytes.as_chunks::<SIZE>().0.iter().map(|chunk| T::from_be_bytes(chunk.as_slice())).collect()
}

trait Decodable: Sized {
    fn from_be_bytes(bytes: &[u8]) -> Self;
}

macro_rules! impl_decodable {
    ($($ty:ty),*) => {
        $(impl Decodable for $ty {
            fn from_be_bytes(bytes: &[u8]) -> Self {
                let mut arr = [0u8; std::mem::size_of::<$ty>()];
                arr.copy_from_slice(bytes);
                <$ty>::from_be_bytes(arr)
            }
        })*
    };
}

impl_decodable!(i8, i16, i32, i64, u16, u32, u64, f32, f64);

/// Render an item in SML-ish form (close to the notation used in SEMI
/// documentation) for logs and test diagnostics.
impl fmt::Display for SecsItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecsItem::List(items) => {
                write!(f, "<L [{}]", items.len())?;
                for item in items {
                    write!(f, " {item}")?;
                }
                write!(f, ">")
            }
            SecsItem::Binary(v) => write!(f, "<B {:02X?}>", v),
            SecsItem::Boolean(v) => write!(f, "<BOOLEAN {:?}>", v),
            SecsItem::Ascii(s) => write!(f, "<A {:?}>", s),
            SecsItem::Jis(v) => write!(f, "<J {:02X?}>", v),
            SecsItem::I8(v) => write!(f, "<I8 {:?}>", v),
            SecsItem::I1(v) => write!(f, "<I1 {:?}>", v),
            SecsItem::I2(v) => write!(f, "<I2 {:?}>", v),
            SecsItem::I4(v) => write!(f, "<I4 {:?}>", v),
            SecsItem::F8(v) => write!(f, "<F8 {:?}>", v),
            SecsItem::F4(v) => write!(f, "<F4 {:?}>", v),
            SecsItem::U8(v) => write!(f, "<U8 {:?}>", v),
            SecsItem::U1(v) => write!(f, "<U1 {:?}>", v),
            SecsItem::U2(v) => write!(f, "<U2 {:?}>", v),
            SecsItem::U4(v) => write!(f, "<U4 {:?}>", v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(item: SecsItem) {
        let bytes = item.to_bytes();
        let (decoded, used) = SecsItem::from_bytes(&bytes).expect("decode");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, item);
    }

    #[test]
    fn empty_list() {
        // SEMI E5 zero-length list: header 0x01, length byte 0x00.
        roundtrip(SecsItem::List(vec![]));
        assert_eq!(SecsItem::List(vec![]).to_bytes(), vec![0x01, 0x00]);
    }

    #[test]
    fn nested_list_mixed_items() {
        roundtrip(SecsItem::List(vec![
            SecsItem::U4(vec![42, 7]),
            SecsItem::a("hello"),
            SecsItem::List(vec![
                SecsItem::Binary(vec![0xDE, 0xAD]),
                SecsItem::Boolean(vec![true, false, true]),
            ]),
            SecsItem::F4(vec![1.5, -2.25]),
        ]));
    }

    #[test]
    fn all_numeric_formats() {
        roundtrip(SecsItem::I8(vec![-5]));
        roundtrip(SecsItem::I1(vec![-5]));
        roundtrip(SecsItem::I2(vec![-500]));
        roundtrip(SecsItem::I4(vec![-50000]));
        roundtrip(SecsItem::F8(vec![3.25]));
        roundtrip(SecsItem::U8(vec![u64::MAX]));
        roundtrip(SecsItem::U1(vec![255]));
        roundtrip(SecsItem::U2(vec![65535]));
        roundtrip(SecsItem::U4(vec![4_000_000_000]));
        roundtrip(SecsItem::Jis(vec![0x81, 0x40]));
    }

    #[test]
    fn three_byte_length_encoding() {
        let long = vec![0xABu8; 70_000];
        let bytes = SecsItem::Binary(long.clone()).to_bytes();
        // 70000 = 0x011170 → length bytes 01 11 70.
        assert_eq!(&bytes[..4], &[0x20 | 3, 0x01, 0x11, 0x70]);
        let (decoded, _) = SecsItem::from_bytes(&bytes).expect("decode");
        assert_eq!(decoded, SecsItem::Binary(long));
    }

    #[test]
    fn bad_header_rejected() {
        // 0x04 = format 0x01, which is not a defined SECS-II format.
        let err = SecsItem::from_bytes(&[0x04, 0x00]).unwrap_err();
        assert!(matches!(err, SecsError::InvalidItemHeader(0x04)));
    }

    #[test]
    fn zero_length_bits_rejected() {
        let err = SecsItem::from_bytes(&[0x20]).unwrap_err();
        assert!(matches!(err, SecsError::InvalidItemLengthBits(0)));
    }

    #[test]
    fn display_smlish() {
        let item = SecsItem::List(vec![SecsItem::u4(1), SecsItem::a("x")]);
        assert_eq!(item.to_string(), "<L [2] <U4 [1]> <A \"x\">>");
    }
}
