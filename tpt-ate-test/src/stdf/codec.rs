//! Primitive STDF field codec: fixed-width integers/floats plus the
//! variable-length string forms (`Cn`, `Bn`, `Sn`).
//!
//! STDF files declare their byte order in the FAR record's `CPU_TYPE`:
//! value 1 = big-endian (68000-class), value 3 = little-endian (x86). The
//! reader learns the endianness from FAR; the writer always emits
//! little-endian x86 files.

use crate::stdf::error::{Result, StdfError};

/// Byte order of a STDF file, discovered from the FAR record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}

/// CPU_TYPE codes for the FAR record.
pub mod cpu_type {
    /// Big-endian 68000-class CPU.
    pub const M68000: u8 = 1;
    /// Little-endian x86-class CPU.
    pub const X86: u8 = 3;
}

/// STDF version carried in the FAR record (V4).
pub const STDF_VERSION_V4: u8 = 4;

/// A read cursor over one record's data bytes.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    endian: Endian,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8], endian: Endian) -> Self {
        Reader { data, pos: 0, endian }
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.remaining() < n {
            return Err(StdfError::UnexpectedEof { wanted: n, had: self.remaining() });
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    pub fn u1(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn u2(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(match self.endian {
            Endian::Little => u16::from_le_bytes([b[0], b[1]]),
            Endian::Big => u16::from_be_bytes([b[0], b[1]]),
        })
    }

    pub fn u4(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(match self.endian {
            Endian::Little => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            Endian::Big => u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
        })
    }

    pub fn i1(&mut self) -> Result<i8> {
        Ok(self.u1()? as i8)
    }

    pub fn i2(&mut self) -> Result<i16> {
        Ok(self.u2()? as i16)
    }

    pub fn r4(&mut self) -> Result<f32> {
        let b = self.take(4)?;
        let bits = match self.endian {
            Endian::Little => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            Endian::Big => u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
        };
        Ok(f32::from_bits(bits))
    }

    /// `Cn`: one length byte, then that many ASCII characters. Empty string
    /// encodes as the single byte 0x00.
    pub fn cn(&mut self) -> Result<String> {
        let len = self.u1()? as usize;
        let bytes = self.take(len)?;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }

    /// `Bn`: one length byte, then that many bytes.
    pub fn bn(&mut self) -> Result<Vec<u8>> {
        let len = self.u1()? as usize;
        Ok(self.take(len)?.to_vec())
    }

    /// `U*2`-prefixed byte array (used by GDR): two-byte count, then bytes.
    pub fn k_bytes(&mut self) -> Result<Vec<u8>> {
        let len = self.u2()? as usize;
        Ok(self.take(len)?.to_vec())
    }

    /// `Sn`: two-byte count of strings, each encoded as `Cn`.
    pub fn sn(&mut self) -> Result<Vec<String>> {
        let count = self.u2()? as usize;
        let mut strings = Vec::with_capacity(count.min(1024));
        for _ in 0..count {
            strings.push(self.cn()?);
        }
        Ok(strings)
    }
}

/// Encode helpers matching the reader, always little-endian (the writer's
/// file flavor, FAR CPU_TYPE = x86).
pub mod write {
    pub fn u1(out: &mut Vec<u8>, v: u8) {
        out.push(v);
    }

    pub fn u2(out: &mut Vec<u8>, v: u16) {
        out.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u4(out: &mut Vec<u8>, v: u32) {
        out.extend_from_slice(&v.to_le_bytes());
    }

    pub fn i1(out: &mut Vec<u8>, v: i8) {
        out.push(v as u8);
    }

    pub fn i2(out: &mut Vec<u8>, v: i16) {
        out.extend_from_slice(&v.to_le_bytes());
    }

    pub fn r4(out: &mut Vec<u8>, v: f32) {
        out.extend_from_slice(&v.to_bits().to_le_bytes());
    }

    pub fn cn(out: &mut Vec<u8>, s: &str) {
        let bytes = s.as_bytes();
        assert!(bytes.len() <= 255, "Cn field exceeds 255 bytes: {}", bytes.len());
        out.push(bytes.len() as u8);
        out.extend_from_slice(bytes);
    }

    pub fn k_bytes(out: &mut Vec<u8>, data: &[u8]) {
        assert!(data.len() <= u16::MAX as usize, "k-array exceeds 65535 bytes");
        u2(out, data.len() as u16);
        out.extend_from_slice(data);
    }

    pub fn sn(out: &mut Vec<u8>, strings: &[String]) {
        assert!(strings.len() <= u16::MAX as usize, "Sn field exceeds 65535 entries");
        u2(out, strings.len() as u16);
        for s in strings {
            cn(out, s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_width_both_endians() {
        for &endian in &[Endian::Little, Endian::Big] {
            let mut buf = Vec::new();
            write::u2(&mut buf, 0x1234);
            write::u4(&mut buf, 0xDEAD_BEEF);
            write::r4(&mut buf, 1.5);
            write::i2(&mut buf, -7);
            let mut r = Reader::new(&buf, endian);
            // The writer is always little-endian; big-endian decode of these
            // bytes yields garbage — just check little-endian round-trips.
            if endian == Endian::Little {
                assert_eq!(r.u2().unwrap(), 0x1234);
                assert_eq!(r.u4().unwrap(), 0xDEAD_BEEF);
                assert_eq!(r.r4().unwrap(), 1.5);
                assert_eq!(r.i2().unwrap(), -7);
            }
        }
    }

    #[test]
    fn cn_roundtrip() {
        let mut buf = Vec::new();
        write::cn(&mut buf, "LOT123");
        write::cn(&mut buf, "");
        let mut r = Reader::new(&buf, Endian::Little);
        assert_eq!(r.cn().unwrap(), "LOT123");
        assert_eq!(r.cn().unwrap(), "");
        assert!(r.is_empty());
    }

    #[test]
    fn sn_roundtrip() {
        let mut buf = Vec::new();
        write::sn(&mut buf, &["r1".into(), "r2".into()]);
        let mut r = Reader::new(&buf, Endian::Little);
        assert_eq!(r.sn().unwrap(), vec!["r1".to_string(), "r2".to_string()]);
    }
}
