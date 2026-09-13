//! STDF file writer over any byte stream. Files are written little-endian
//! (FAR `CPU_TYPE` = x86), the dominant interchange flavor.

use crate::stdf::codec::{cpu_type, STDF_VERSION_V4};
use crate::stdf::error::Result;
use crate::stdf::records::{Far, Record};
use std::io::Write;

/// Streaming STDF writer. The first record written must be FAR; writing any
/// other record first is an error so a mis-ordered file can never be
/// produced by accident.
pub struct StdfWriter<W: Write> {
    inner: W,
    wrote_far: bool,
}

impl<W: Write> StdfWriter<W> {
    pub fn new(inner: W) -> Self {
        StdfWriter { inner, wrote_far: false }
    }

    /// Write one record. The first call must carry a FAR record; if it does
    /// not, a V4/x86 FAR is prepended automatically.
    pub fn write(&mut self, record: &Record) -> Result<()> {
        if !self.wrote_far {
            self.wrote_far = true;
            let far = match record {
                Record::Far(far) => far.clone(),
                _ => Far { cpu_type: cpu_type::X86, stdf_ver: STDF_VERSION_V4 },
            };
            let mut bytes = Vec::new();
            Record::Far(far).encode(&mut bytes);
            self.inner.write_all(&bytes)?;
            if matches!(record, Record::Far(_)) {
                self.inner.flush()?;
                return Ok(());
            }
        }
        let mut bytes = Vec::new();
        record.encode(&mut bytes);
        self.inner.write_all(&bytes)?;
        self.inner.flush()?;
        Ok(())
    }

    /// Write a batch of records.
    pub fn write_all(&mut self, records: &[Record]) -> Result<()> {
        for record in records {
            self.write(record)?;
        }
        Ok(())
    }

    /// Flush and return the inner stream.
    pub fn into_inner(self) -> W {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdf::reader::StdfReader;
    use crate::stdf::records::{Hbr, Ptr};

    #[test]
    fn auto_prepends_far_when_first_record_is_not_far() {
        let mut buf = Vec::new();
        {
            let mut writer = StdfWriter::new(&mut buf);
            writer.write(&Record::Ptr(Ptr::verdict(1, 1, 1, true))).unwrap();
            writer
                .write(&Record::Hbr(Hbr {
                    head_num: 1,
                    site_num: 255,
                    hbin_num: 1,
                    hbin_cnt: 1,
                    hbin_pf: b'P',
                    hbin_nam: "PASS".into(),
                }))
                .unwrap();
        }
        let mut reader = StdfReader::new(std::io::Cursor::new(buf));
        let records = reader.read_all().unwrap();
        assert!(matches!(&records[0], Record::Far(_)));
        assert_eq!(records.len(), 3);
    }
}
