//! `tpt-ate-test` — equipment communication and test execution for
//! automated test equipment (RFC-003, `spec.txt` Section 2A).
//!
//! This crate is the operational counterpart to `tpt-silicon`'s design-time
//! DFT/ATPG work: it executes the patterns and limits a test program
//! provides against real or simulated equipment, sorts dies into bins keyed
//! to the wafer map, and records results in STDF.
//!
//! Layers:
//! - [`secs`]: SECS/GEM equipment communication (SEMI E5/E30/E37), our own
//!   implementation built directly against the published standards.
//! - [`stdf`]: STDF V4 read/write — the industry-standard result-data
//!   format (distinct from SECS/GEM, which is the communication layer).
//! - [`wafer`]: wafer map model keyed by physical die location, carried
//!   through unmodified from `tpt-silicon`'s layout output.
//! - [`patterns`]: the test-program input (ATPG pattern groups + parametric
//!   limits); authoring semantics stay in `tpt-silicon`.
//! - [`bins`] / [`bin_sort`]: bin taxonomy and pass/fail → hardware/software
//!   bin sorting.
//! - [`sim`]: equipment simulator harness so the whole flow runs without
//!   real ATE access.
//! - [`recording`]: assembles a completed wafer-sort run into its STDF
//!   record stream.
//!
//! Explicitly out of scope: test program *authoring* (fault targeting,
//! pattern compaction) — that belongs to `tpt-silicon`.

pub mod bin_sort;
pub mod bins;
pub mod patterns;
pub mod recording;
pub mod rng;
pub mod secs;
pub mod sim;
pub mod stdf;
pub mod wafer;
