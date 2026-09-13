//! `tpt-ate-aggregate` — extends RFC-002's `OutcomeReport` file exchange
//! with bin/yield data from the test & assembly stage (RFC-003, `spec.txt`
//! Section 2C). Extends, does not replace: same schema, same file-based
//! manually-sent loop, same `tpt-ai` ingestion path — no parallel file
//! format and no new transmission mechanism.
//!
//! Sub-modules:
//! - [`schema`]: the RFC-002 Section 4.1 `OutcomeReport` schema and its
//!   supporting types (first materialization — see the module docs for the
//!   ownership note), extended with [`ManufacturingTrack::TestAssembly`].
//! - [`outcome`]: `TestOutcome` — wafer map, `BinDistribution` with the
//!   spec3 Section 4.4 minimum-cohort rule, package-level electrical
//!   measurements.
//! - [`file`]: RFC-002 Section 4.8 offline signed-file mode (JSON, manual,
//!   no network).
//! - [`ingest`]: the `ManufacturingOutcomeIngestor` contract shared with
//!   `tpt-fab`'s outcome data.

pub mod file;
pub mod ingest;
pub mod outcome;
pub mod schema;

pub use file::{AggregateError, JsonOutcomeFile, OutcomeFileReader, OutcomeFileWriter};
pub use ingest::{ManufacturingOutcomeIngestor, RecordingIngestor};
pub use outcome::{BinCount, BinDistribution, TestOutcome};
pub use schema::{
    ConsentScope, ElectricalMeasurement, GeometryDeviation, ManufacturingTrack,
    MeasurementQuantity, OutcomeReport, PatterningTech, ProcessDeviation, SchemaVersion, Signature,
    YieldSummary,
};
