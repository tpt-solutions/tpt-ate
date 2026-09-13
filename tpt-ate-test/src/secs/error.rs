use thiserror::Error;

/// Errors raised by the SECS/GEM equipment-communication layer.
///
/// Built directly against the published SEMI standards (E5 SECS-II, E37 HSMS,
/// E30 GEM) per `spec.txt` Section 2A — this is `tpt-ate`'s own implementation,
/// deliberately not shared with `tpt-fab`.
#[derive(Debug, Error)]
pub enum SecsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("stream ended mid-message (wanted {wanted} bytes, had {had})")]
    UnexpectedEof { wanted: usize, had: usize },

    #[error("invalid SECS-II item header byte 0x{0:02X}")]
    InvalidItemHeader(u8),

    #[error("SECS-II item length uses {0} length bytes (allowed: 1-3)")]
    InvalidItemLengthBits(u8),

    #[error("SECS-II item length {length} exceeds remaining data ({remaining})")]
    ItemLengthOverflow { length: usize, remaining: usize },

    #[error("trailing bytes after SECS-II item ({count} left)")]
    TrailingBytesAfterItem { count: usize },

    #[error("non-data HSMS message where data expected (SType {stype})")]
    NotDataMessage { stype: u8 },

    #[error("HSMS header must be 10 bytes, got {0}")]
    BadHeaderSize(usize),

    #[error("HSMS PType {0} not supported (only SECS-II, PType 0)")]
    UnsupportedPType(u8),

    #[error("HSMS connection is not in the Selected state (state: {state:?})")]
    NotSelected { state: crate::secs::hsms::HsmsState },

    #[error("SECS-II body missing from data message (stream {stream}, function {function})")]
    MissingDataBody { stream: u8, function: u8 },

    #[error("unexpected SECS message S{stream}F{function} (wanted {wanted})")]
    UnexpectedStreamFunction { stream: u8, function: u8, wanted: String },

    #[error("GEM semantic error: {0}")]
    Gem(String),
}

/// Result alias for the SECS/GEM layer.
pub type Result<T> = std::result::Result<T, SecsError>;
