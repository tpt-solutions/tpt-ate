# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-09-17

### Added

- `AssemblyBackend` trait (the `tpt-fab-litho` pluggable-backend pattern): one core,
  swappable technique implementations, no equipment-technique assumptions in the core —
  execution flows through the trait only.
- Three technique backends over one shared GEM execution flow: `WireBondBackend`
  (wire pitch / max wire length), `FlipChipBackend` (bump pitch / alignment tolerance),
  and `ChipletPickPlaceBackend` (nozzle / placement tolerance).
- `InterposerLayout` placement reference: chiplet footprints, sites, orientation, and
  extent in nm `i64` conventions mirroring `tpt-silicon`; serde shapes byte-compatible
  with `tpt-silicon`'s `tpt-silicon-interposer` crate, so layout JSON carries through unmodified.
- `AssemblyHost` equipment link: Select + S1F1/S1F13 handshake, S2F41 host commands with
  HCACK checking, S6F11 placement-event collection matched by site id.
- `AssemblyEquipment` simulator: deterministic seeded placement jitter per command,
  full equipment-side GEM flow, per-technique model names.
- Technique-agnostic placement verification: off-target (Manhattan), out-of-bounds,
  overlap, orientation mismatch, duplicate, missing, and unknown-site violations, with
  serde-serializable reports.
- GEM host-command support consumed from `tpt-ate-comm` (S2F41/S2F42 added to the shared
  layer for this crate's flow).
- Milestone integration test (`tests/phase2_milestone.rs`): all three backends driven
  uniformly through `Box<dyn AssemblyBackend>` validate against a synthetic interposer
  layout — the RFC-003 Phase 2 gate — plus a negative test flagging a shifted placement
  and a plan-serialization round trip for the file-based exchange.

[0.1.0]: https://github.com/tpt-solutions/tpt-ate/releases/tag/tpt-ate-assembly-v0.1.0
