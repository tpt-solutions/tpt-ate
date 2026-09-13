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

- **`tpt-ate-test`** — equipment communication (SECS/GEM built against the published SEMI
  standards), STDF V4 read/write, wafer map model, bin-sort logic, and an equipment
  simulator harness so all of it is testable without real ATE access.
- **`tpt-ate-assembly`** — equipment communication for physical assembly behind a pluggable
  `AssemblyBackend` (wire bonding / flip-chip / chiplet pick-and-place), with placement
  verification against `tpt-silicon`'s interposer/chiplet layout.
- **`tpt-ate-aggregate`** — extends RFC-002's `OutcomeReport` with `TestOutcome` (wafer map,
  bin distribution, package-level electrical measurements). File-based, manually sent —
  no new transmission mechanism.

The SECS/GEM equipment-communication core is built inline in `tpt-ate-test` first and
extracted into a shared internal crate once `tpt-ate-assembly` makes the duplication
concrete (per RFC-003 Section 5's locked decision).

## Status

Early development — see [`todo.md`](todo.md) for the phase-by-phase build checklist and
`spec.txt` for the source-of-truth design.

## License

Dual-licensed under MIT or Apache-2.0 — see `LICENSE-MIT` and `LICENSE-APACHE`.
Copyright TPT Solutions.
