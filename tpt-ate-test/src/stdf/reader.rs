//! STDF file reader: yields parsed [`Record`]s from any byte stream.
//!
//! The FAR record must come first (it declares CPU endianness); the reader
//! defaults to little-endian if it encounters records before a FAR.

use crate::stdf::codec::{cpu_type, Endian};
use crate::stdf::error::{Result, StdfError};
use crate::stdf::records::Record;
use std::io::Read;

/// Streaming STDF reader.
pub struct StdfReader<R: Read> {
    inner: R,
    endian: Endian,
    /// Set once the FAR has been seen.
    saw_far: bool,
}

impl<R: Read> StdfReader<R> {
    pub fn new(inner: R) -> Self {
        StdfReader { inner, endian: Endian::Little, saw_far: false }
    }

    /// The endianness declared by the file's FAR record (defaults to little
    /// until FAR is read).
    pub fn endian(&self) -> Endian {
        self.endian
    }

    /// Read the next record. Returns `Ok(None)` at end of file.
    pub fn next_record(&mut self) -> Result<Option<Record>> {
        let mut header = [0u8; 4];
        match self.inner.read(&mut header) {
            Ok(0) => return Ok(None),
            Ok(4) => {}
            Ok(_) => return Err(StdfError::ShortRecordHeader(0)),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => return Ok(None),
            Err(e) => return Err(e.into()),
        }
        // If this is the very first record it must be a FAR (type 0, sub 10);
        // its body starts with CPU_TYPE, which decides endianness for the
        // whole file.
        if !self.saw_far && !(header[2] == 0 && header[3] == 10) {
            return Err(StdfError::MalformedRecord {
                typ: header[2],
                sub: header[3],
                detail: "first record in an STDF file must be FAR".into(),
            });
        }
        let len = u16::from_le_bytes([header[0], header[1]]) as usize;
        let mut body = vec![0u8; len];
        let mut filled = 0;
        while filled < len {
            let n = self.inner.read(&mut body[filled..])?;
            if n == 0 {
                return Err(StdfError::UnexpectedEof { wanted: len, had: filled });
            }
            filled += n;
        }
        if header[2] == 0 && header[3] == 10 {
            self.saw_far = true;
            if !body.is_empty() {
                self.endian = match body[0] {
                    cpu_type::M68000 => Endian::Big,
                    _ => Endian::Little,
                };
            }
        }
        Ok(Some(Record::parse(header[2], header[3], &body, self.endian)?))
    }

    /// Read all remaining records into a vector (convenience for tests and
    /// small files).
    pub fn read_all(&mut self) -> Result<Vec<Record>> {
        let mut records = Vec::new();
        while let Some(record) = self.next_record()? {
            records.push(record);
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdf::codec::STDF_VERSION_V4;
    use crate::stdf::records::{Far, Ptr};
    use crate::stdf::writer::StdfWriter;

    #[test]
    fn read_back_written_records() {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut writer = StdfWriter::new(&mut buf);
            writer
                .write(&Record::Far(Far { cpu_type: cpu_type::X86, stdf_ver: STDF_VERSION_V4 }))
                .unwrap();
            writer.write(&Record::Ptr(Ptr::measured(10, 1, 1, 3.3, "VDD", "V", 3.0, 3.6))).unwrap();
        }
        buf.set_position(0);
        let mut reader = StdfReader::new(buf);
        let records = reader.read_all().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(reader.endian(), Endian::Little);
        match &records[0] {
            Record::Far(far) => assert_eq!(far.stdf_ver, 4),
            other => panic!("expected FAR, got {other:?}"),
        }
    }

    #[test]
    fn rejects_file_not_starting_with_far() {
        // A PTR record as the first record: valid bytes, wrong order.
        let ptr = Record::Ptr(Ptr::verdict(1, 1, 1, true)).to_bytes();
        let mut reader = StdfReader::new(std::io::Cursor::new(ptr));
        let err = reader.next_record().unwrap_err();
        assert!(matches!(err, StdfError::MalformedRecord { typ: 15, sub: 10, .. }));
    }
}
