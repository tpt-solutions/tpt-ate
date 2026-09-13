# TPT ATE — Master Build Checklist

Tracking checklist for the project described in `spec.txt` (RFC-003: Test & Assembly Operational
Software, `tpt-ate`). That file remains the source-of-truth design doc; this file is the
task-tracking layer on top of it.

**Scope reminder:** `tpt-ate` is the *operational* half only (runs against real/simulated
equipment on the production floor). Design-time work — DFT insertion, ATPG pattern generation,
interposer/chiplet layout — belongs in `tpt-silicon`, not here. One repo, three crates, sharing an
equipment-communication core.

**Decisions locked in from spec.txt Section 5 (Open Questions):**
- The shared SECS/GEM equipment-communication core is **not** split into its own crate on day
  one — built inline in `tpt-ate-test` first, extracted once `tpt-ate-assembly` (Phase 3) makes
  the duplication concrete.
- Wafer-map die-location data is carried through from `tpt-silicon`'s layout output **unmodified**
  — no new shared interchange format with `tpt-fab` unless a real mismatch surfaces later.

---

## Phase 0 — Workspace Scaffold
- [x] Initialize dual `LICENSE-MIT` / `LICENSE-APACHE` (MIT OR Apache-2.0), copyright TPT Solutions
- [x] Root `Cargo.toml` workspace manifest (members: `tpt-ate-test`, `tpt-ate-assembly`,
      `tpt-ate-aggregate`)
- [x] `rustfmt.toml`, `.gitignore`
- [ ] `git init`, first commit
- [ ] Scaffold empty member crates (`tpt-ate-test`, `tpt-ate-assembly`, `tpt-ate-aggregate`)
- [ ] Base CI (build + test on push)
- [ ] Root `README.md` explaining `tpt-ate`'s place relative to `tpt-silicon` and `tpt-fab`

## Phase 1 — `tpt-ate-test` core (Weeks 1–5)
*(Design-time counterpart — DFT/ATPG generation in `tpt-silicon` — tracked separately in that
repo, not here; `tpt-ate-test` has nothing to execute without it.)*
- [ ] Equipment-communication layer: SECS/GEM message handling built directly against the
      published SEMI standards, own implementation (not shared with `tpt-fab`)
- [ ] STDF (Standard Test Data Format) reader
- [ ] STDF writer
- [ ] Equipment simulator harness (so bin-sort logic is testable without real ATE access)
- [ ] Wafer map model: physical die location, consumed unmodified from `tpt-silicon`'s layout
      output
- [ ] Bin-sort logic: consumes `tpt-silicon`'s ATPG pattern output + DFT-inserted netlist
      metadata, executes against equipment (real or simulator), records pass/fail/bin-category
      per die keyed to wafer map
- [ ] Explicitly out of scope here: test program *authoring* (fault targeting, pattern
      compaction) — that stays in `tpt-silicon`
- **Milestone:** a simulated test run against synthetic ATPG patterns produces a valid STDF file
  with correct bin categorization.

## Phase 2 — `tpt-ate-assembly` (Weeks 6–10)
- [ ] Extract shared equipment-communication core from Phase 1's SECS/GEM work into a form both
      `tpt-ate-test` and `tpt-ate-assembly` can use (internal module or crate — decide based on
      actual duplication observed, not speculatively)
- [ ] `AssemblyBackend` trait (mirrors `tpt-fab-litho`'s pluggable-backend pattern): one core,
      swappable technique implementations, no equipment-technique assumptions hard-coded into
      the core
  - [ ] Wire bonding backend
  - [ ] Flip-chip placement backend
  - [ ] Chiplet pick-and-place backend
- [ ] Placement verification logic against `tpt-silicon`'s RFC-001 interposer/chiplet layout as
      the placement reference
- **Milestone:** a simulated chiplet placement run validates against a synthetic interposer
  layout with no equipment-technique assumptions hard-coded into the core.

## Phase 3 — `tpt-ate-aggregate` and schema extension (Weeks 11–14)
- [ ] `TestOutcome` struct: `wafer_map` (`WaferDieMap`), `bin_summary` (`BinDistribution`,
      aggregated per spec3 Section 4.4's minimum-cohort rule), `package_test`
      (`Vec<ElectricalMeasurement>`, reusing spec3's shape)
- [ ] `ManufacturingTrack::TestAssembly` variant added to RFC-002's shared track enum
- [ ] Carry `TestOutcome` inside `OutcomeReport.process_notes` / the new track variant — one
      schema, no parallel file format
- [ ] Confirm no new transmission mechanism: file-based, manually sent, per spec3 Section 4.8
      (offline signed-file mode)
- [ ] Wire into the same `tpt-ai` ingestion path `tpt-fab`'s outcome data already uses
- [ ] Note (not build here): bin-sort/yield ground truth is a calibration input for `tpt-ai`'s
      yield-heatmap model and `tpt-fab-process`'s OPC/etch simulator — flag as a value point but
      the actual calibration wiring belongs to those consumers, not `tpt-ate`
- **Milestone:** a synthetic end-to-end run — design, DFT insertion, simulated test, simulated
  assembly, outcome file generated and manually ingested — completes without a design partner.

## Phase 4 — Design partner pilot (Month 4+)
- [ ] Identify candidate: a smaller or emerging OSAT, or a fab handling test/assembly in-house
      without an enterprise software budget (per spec.txt Section 1 positioning — explicitly not
      Amkor/ASE-tier)
- [ ] Pilot the live loop beyond the synthetic end-to-end run
- Gated on someone outside `tpt-solutions` saying yes — no fixed timeline beyond "Phase 3 is
  solid."

---

## Backlog / Open Questions (from spec.txt Section 5)
- [ ] Revisit: does wafer-map die-location data ever need its own interchange format shared with
      `tpt-fab`, or does carrying it through unmodified from `tpt-silicon` hold up in practice?
      (Default resolved above as "carry through unmodified" — revisit only if a real mismatch
      surfaces during Phase 1/2 integration.)
- [ ] Track as a likely next RFC, not scoped here: pre-RTL / architecture-exploration layer (HLS,
      performance modeling, design-space exploration) as new `tpt-silicon` front-end crates —
      mirrors RFC-001's downstream PCB extension but upstream. Lives in `tpt-silicon`, not
      `tpt-ate`, if it happens.

## Cross-repo dependencies (tracked here for visibility, not owned by this repo)
- `tpt-silicon`: DFT/ATPG generation (Phase 1 blocker), interposer/chiplet layout (Phase 2
  blocker), RFC-002's `OutcomeReport`/`ManufacturingTrack`/`ElectricalMeasurement` schema (Phase 3
  blocker, defined in `tpt-silicon/spec3.txt` Section 4).
- `tpt-fab`: SECS/GEM implementation pattern to reuse (not share code with) for Phase 1.
- `tpt-telos`, `tpt-ai`: cross-cutting dependencies per portfolio convention; `tpt-ai` is the
  ingestion target in Phase 3.
