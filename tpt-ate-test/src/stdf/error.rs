use thiserror::Error;

/// Errors raised by the STDF (Standard Test Data Format) reader/writer.
#[derive(Debug, Error)]
pub enum StdfError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("stream ended mid-record (wanted {wanted} bytes, had {had})")]
    UnexpectedEof { wanted: usize, had: usize },

    #[error("record header too short ({0} bytes, need 4)")]
    ShortRecordHeader(usize),

    #[error("record length byte count {count} is not 1-3")]
    InvalidStringLengthBits { count: usize },

    #[error("string field of length {length} overruns record (remaining {remaining})")]
    StringOverrun { length: usize, remaining: usize },

    #[error("unknown record type {typ}, subtype {sub} encountered in strict mode")]
    UnknownRecord { typ: u8, sub: u8 },

    #[error("record data for {typ}/{sub} is malformed: {detail}")]
    MalformedRecord { typ: u8, sub: u8, detail: String },

    #[error("cannot write record: {detail}")]
    WriteError { detail: String },
}

/// Result alias for the STDF layer.
pub type Result<T> = std::result::Result<T, StdfError>;
