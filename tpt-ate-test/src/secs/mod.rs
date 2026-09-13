//! SECS/GEM equipment communication (SEMI E5 / E30 / E37), built directly
//! against the published standards — `tpt-ate`'s own implementation, kept
//! separate from `tpt-fab` per the RFC-003 decision.
//!
//! Layering mirrors the standards:
//! - [`item`]: SECS-II (E5) typed data items and their wire encoding.
//! - [`hsms`]: HSMS-SS (E37.1) message framing and session state.
//! - [`gem`]: GEM (E30) stream/function transactions and the ATE event
//!   vocabulary (CEIDs/VIDs/RPTID) the simulator speaks.
//! - [`transport`]: an in-memory duplex pipe so the whole stack is exercisable
//!   without real network I/O.

pub mod error;
pub mod gem;
pub mod hsms;
pub mod item;
pub mod transport;

pub use error::{Result, SecsError};
pub use hsms::{HsmsMessage, HsmsSession, HsmsState};
pub use item::SecsItem;
