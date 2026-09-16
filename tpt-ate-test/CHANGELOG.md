# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-09-17

### Added

- STDF V4 reader and writer over any byte stream: core production record set (FAR, ATR,
  MIR, MRR, PCR, HBR, SBR, PMR, WIR, WRR, WCR, PTR, FTR, TSR, GDR, DTR, BPS, EPS) with
  endianness learned from the FAR record, conditional-presence flag rules for PTR/FTR
  optional fields, and byte-lossless round-trips (unmodeled records preserved verbatim as
  `Record::Unknown`, unconsumed tails preserved on modeled ones).
- Wafer map model (`WaferDieMap`): per-die state keyed by physical die location in
  die-index coordinates, carried through unmodified from the design-side layout output per
  the RFC-003 §5 decision; bin count/aggregate helpers and JSON exchange as coordinate
  entries.
- Bin taxonomy (`BinTaxonomy`) with configurable hardware/software bin definitions and
  sensible ATE defaults; run-level `BinTally` with yield computation.
- Bin-sort logic: first-failure-in-execution-order assignment honoring per-test soft-bin
  policy from the test program, abort binning, wafer-map fill, and tally construction.
- Test-program input model (`patterns::TestProgram`): parametric tests with limits and
  per-test soft-bin policy, ATPG pattern groups — the placeholder shape consumed from
  `tpt-silicon` until the real handoff format lands there (already wire-compatible with
  `tpt-silicon-dft`'s export).
- Deterministic equipment simulator (`SimulatedTester`): seeded per-die fault model,
  in-limit jitter for passing measurements, full GEM equipment-side flow over any
  `Read + Write` stream (Select, S1F1/S1F13 handshake, S6F11 per-die event reports).
- Wafer-run recording (`recording`): complete STDF record-stream assembly including
  per-test synopses, site and total PCR, HBR/SBR summaries, and a GDR-embedded wafer map
  with decode support.
- Milestone integration test (`tests/phase1_milestone.rs`): simulated wafer sort over
  in-memory HSMS/GEM producing a valid STDF file with bin categorization verified on
  read-back — the RFC-003 Phase 1 gate.
- This crate originally carried the SECS/GEM layer now extracted to `tpt-ate-comm`; the
  `secs`/`rng` modules are re-exports from there since the extraction.

[0.1.0]: https://github.com/tpt-solutions/tpt-ate/releases/tag/tpt-ate-test-v0.1.0
