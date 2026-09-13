//! STDF (Standard Test Data Format) V4 read/write — the industry-standard
//! format test results are recorded in, distinct from SECS/GEM (the
//! equipment-communication layer).
//!
//! Sub-modules:
//! - [`codec`]: primitive field encode/decode (fixed-width + `Cn`/`Bn`/`Sn`).
//! - [`records`]: the modeled record set and their wire (de)serialization.
//! - [`reader`] / [`writer`]: streaming file I/O over any byte stream.

pub mod codec;
pub mod error;
pub mod reader;
pub mod records;
pub mod writer;

pub use error::{Result, StdfError};
pub use reader::StdfReader;
pub use records::Record;
pub use writer::StdfWriter;
