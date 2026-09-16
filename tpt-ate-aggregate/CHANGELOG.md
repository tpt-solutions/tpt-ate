# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-09-17

### Added

- `TestOutcome` (wafer map, bin summary, package test) carried inside the shared
  `OutcomeReport` via the `ManufacturingTrack::TestAssembly` variant — one schema, no
  parallel file format.
- `BinDistribution` with RFC-002 §4.4's minimum-cohort rule applied at the source
  (`redact_below_cohort`), withheld buckets keeping their keys but hiding exact counts.
- RFC-002 §4.1's `OutcomeFileWriter`/`OutcomeFileReader` traits with a JSON file
  implementation: schema-version gated on read (spec §4.5), `NoSharing` consent blocking
  export, signatures carried through untouched (spec §4.8 offline signed-file mode — no
  network transport by design).
- `ManufacturingOutcomeIngestor` trait verbatim from spec §4.1 plus a reference
  `RecordingIngestor`; the real `tpt-ai` pipeline implements the same trait.
- First materializations of the RFC-named-but-unimplemented supporting types
  (`ElectricalMeasurement`, `GeometryDeviation`, `YieldSummary`, `ProcessDeviation`,
  `ConsentScope`, `SchemaVersion`, `Signature`, `ManufacturingTrack`,
  `PatterningTech`); `tpt-silicon`'s `tpt-silicon-mfg-schema` crate now holds the canonical
  copies with identical serde shapes.
- Milestone integration test (`tests/phase3_milestone.rs`): deterministic synthetic
  end-to-end run — design stand-in, simulated wafer sort, simulated chiplet assembly,
  outcome file generated and manually re-ingested — completing without a design partner
  (the RFC-003 Phase 3 gate).

[0.1.0]: https://github.com/tpt-solutions/tpt-ate/releases/tag/tpt-ate-aggregate-v0.1.0
