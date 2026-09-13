# tpt-ate

Test & Assembly Operational Software for the TPT Solutions EDA/manufacturing portfolio.

`tpt-ate` covers the two operational stages between "wafer comes off the line" and "chip
ships": **electrical test** of every die, and **physical assembly** of dies into packages
(wire bond, flip-chip, chiplet pick-and-place). It runs against real test and assembly
equipment on a production floor — the same category of software `tpt-fab` occupies one
physical stage earlier.

## Where it sits in the portfolio

| Repo | Stage | Runtime vs. design time |
| --- | --- | --- |
| `tpt-silicon` | DFT insertion, ATPG pattern generation, interposer/chiplet layout | Design time |
| **`tpt-ate`** (this repo) | Test execution, bin sorting, assembly, outcome aggregation | Production floor |
| `tpt-fab` | Wafer-level fabrication control | Production floor |

Design-time test and packaging work — deciding which faults to target, pattern compaction,
interposer/chiplet placement synthesis — stays in `tpt-silicon`. `tpt-ate` *executes* what
`tpt-silicon` hands it: ATPG patterns and DFT-inserted netlist metadata flow in for test,
the interposer/chiplet layout flows in as the placement reference for assembly, and bin/yield
results flow back out through the same file-based outcome loop RFC-002 (`spec3.txt`)
established for the wafer-fab stage — one schema, one ingestion path into `tpt-ai`.

Positioning is the same tier as `tpt-fab`: built for smaller and emerging OSATs and fabs
handling test/assembly in-house, not the vertically-integrated tier whose software is as
proprietary as their equipment. See `spec.txt` (RFC-003) for the full design.

## Crates

- **`tpt-ate-comm`** — the shared equipment-communication core: SECS/GEM built against the
  published SEMI standards (E5 SECS-II, E37.1 HSMS, E30 GEM) plus the deterministic simulator
  RNG. Extracted from `tpt-ate-test` once `tpt-ate-assembly` made the duplication concrete
  (per RFC-003 Section 5's locked decision).
- **`tpt-ate-test`** — STDF V4 read/write, wafer map model, bin-sort logic, and the test
  equipment simulator harness, so all of it is testable without real ATE access.
- **`tpt-ate-assembly`** — physical assembly behind a pluggable `AssemblyBackend`
  (wire bonding / flip-chip / chiplet pick-and-place) over one shared GEM execution flow, with
  placement verification against `tpt-silicon`'s interposer/chiplet layout.
- **`tpt-ate-aggregate`** — extends RFC-002's `OutcomeReport` with `TestOutcome` (wafer map,
  bin distribution under the minimum-cohort rule, package-level electrical measurements).
  File-based, manually sent — no new transmission mechanism.

## Status

Phase 0–3 of [`todo.md`](todo.md) are implemented and gated by milestone integration tests
(`phase{1,2,3}_milestone.rs`): simulated wafer sort → valid STDF with correct bins; simulated
chiplet placement validated against a synthetic interposer layout through all three backends;
and a deterministic end-to-end run producing an outcome file that round-trips through manual
ingestion. Remaining: Phase 4 (design-partner pilot, gated externally) and the cross-repo
dependencies tracked in `todo.md` (`tpt-silicon`'s DFT/ATPG/layout/schema work).

## License

Dual-licensed under MIT or Apache-2.0 — see `LICENSE-MIT` and `LICENSE-APACHE`.
Copyright TPT Solutions.
