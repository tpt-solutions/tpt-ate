# TPT ATE — Master Build Checklist

Tracking checklist for the project described in `spec.txt` (RFC-003: Test & Assembly Operational
Software, `tpt-ate`). That file remains the source-of-truth design doc; this file is the
task-tracking layer on top of it.

**Scope reminder:** `tpt-ate` is the *operational* half only (runs against real/simulated
equipment on the production floor). Design-time work — DFT insertion, ATPG pattern generation,
interposer/chiplet layout — belongs in `tpt-silicon`, not here.

**Decisions locked in from spec.txt Section 5 (Open Questions):**
- The shared SECS/GEM equipment-communication core is **not** split into its own crate on day
  one — built inline in `tpt-ate-test` first, extracted once `tpt-ate-assembly` (Phase 3) makes
  the duplication concrete. *(Resolved during Phase 2: extracted as workspace-internal crate
  `tpt-ate-comm` — see Phase 2 below.)*
- Wafer-map die-location data is carried through from `tpt-silicon`'s layout output **unmodified**
  — no new shared interchange format with `tpt-fab` unless a real mismatch surfaces later.
  *(Held through Phase 1–3 integration; `DieCoord` mirrors `tpt-silicon`'s nm/bottom-left
  conventions and no transformation layer was needed.)*

---

## Phase 0 — Workspace Scaffold
- [x] Initialize dual `LICENSE-MIT` / `LICENSE-APACHE` (MIT OR Apache-2.0), copyright TPT Solutions
- [x] Root `Cargo.toml` workspace manifest (members: `tpt-ate-test`, `tpt-ate-assembly`,
      `tpt-ate-aggregate`)
- [x] `rustfmt.toml`, `.gitignore`
- [x] `git init`, first commit
- [x] Scaffold empty member crates (`tpt-ate-test`, `tpt-ate-assembly`, `tpt-ate-aggregate`)
- [x] Base CI (build + test on push)
- [x] Root `README.md` explaining `tpt-ate`'s place relative to `tpt-silicon` and `tpt-fab`

## Phase 1 — `tpt-ate-test` core (Weeks 1–5)
*(Design-time counterpart — DFT/ATPG generation in `tpt-silicon` — tracked separately in that
repo, not here; `tpt-ate-test` has nothing to execute without it. The consumed test-program
shape is defined as a flagged placeholder in `tpt-ate-test/src/patterns.rs` until `tpt-silicon`
lands ATPG/DFT.)*
- [x] Equipment-communication layer: SECS/GEM message handling built directly against the
      published SEMI standards (E5 SECS-II items, E37.1 HSMS-SS framing/session, E30 GEM
      transactions), own implementation (not shared with `tpt-fab`)
- [x] STDF (Standard Test Data Format) reader — V4 core record set, endianness-aware,
      unknown-record-preserving
- [x] STDF writer — same record set, byte-lossless round-trip, FAR-first ordering enforced
- [x] Equipment simulator harness (`tpt-ate-test/src/sim.rs`: seeded deterministic tester speaking
      the full GEM flow over an in-memory HSMS duplex)
- [x] Wafer map model: physical die location, consumed unmodified from `tpt-silicon`'s layout
      output (`WaferDieMap`, die-index coordinates)
- [x] Bin-sort logic: consumes `tpt-silicon`'s ATPG pattern output + DFT-inserted netlist
      metadata (placeholder shape), executes against equipment (real or simulator), records
      pass/fail/bin-category per die keyed to wafer map (first-failure rule, per-test soft-bin
      policy, HBR/SBR rollup)
- [x] Explicitly out of scope here: test program *authoring* (fault targeting, pattern
      compaction) — that stays in `tpt-silicon`
- **Milestone: DONE** — `tpt-ate-test/tests/phase1_milestone.rs`: a simulated test run against
  synthetic ATPG patterns over in-memory HSMS/GEM produces a valid STDF file with correct bin
  categorization (verified on read-back: record counts, HBR/SBR/PCR/WRR counters, GDR-embedded
  wafer map all match the seeded fault model).

## Phase 2 — `tpt-ate-assembly` (Weeks 6–10)
- [x] Extract shared equipment-communication core from Phase 1's SECS/GEM work into a form both
      `tpt-ate-test` and `tpt-ate-assembly` can use — **decision made: own workspace-internal
      crate `tpt-ate-comm`** (duplication became concrete: assembly must not depend on the test
      crate, and both drive different equipment classes over the same SECS/GEM + simulator-RNG
      layer; GEM host commands S2F41/S2F42 added there too)
- [x] `AssemblyBackend` trait (mirrors `tpt-fab-litho`'s pluggable-backend pattern): one core,
      swappable technique implementations, no equipment-technique assumptions hard-coded into
      the core — execution flow lives in one shared function; the core dispatches through the
      trait only (milestone test drives all three via `Box<dyn AssemblyBackend>`)
  - [x] Wire bonding backend
  - [x] Flip-chip placement backend
  - [x] Chiplet pick-and-place backend
- [x] Placement verification logic against `tpt-silicon`'s RFC-001 interposer/chiplet layout as
      the placement reference (`InterposerLayout` in nm/bottom-left conventions mirroring
      `tpt-silicon`; bounds/off-target/overlap/orientation/duplicate/missing checks — flagged as
      the local stand-in until `tpt-silicon` implements RFC-001 layout types)
- **Milestone: DONE** — `tpt-ate-assembly/tests/phase2_milestone.rs`: a simulated chiplet
  placement run validates against a synthetic interposer layout with no equipment-technique
  assumptions hard-coded into the core.

## Phase 3 — `tpt-ate-aggregate` and schema extension (Weeks 11–14)
- [x] `TestOutcome` struct: `wafer_map` (`WaferDieMap`), `bin_summary` (`BinDistribution`,
      aggregated per spec3 Section 4.4's minimum-cohort rule — bucket counts below N are
      withheld at the source), `package_test`
      (`Vec<ElectricalMeasurement>`, reusing spec3's shape)
- [x] `ManufacturingTrack::TestAssembly` variant added to RFC-002's shared track enum
      *(first materialization of that enum — `tpt-silicon` has only the RFC prose; defined in
      `tpt-ate-aggregate/src/schema.rs` with an ownership note to move it to the shared schema
      home when `tpt-silicon` lands one)*
- [x] Carry `TestOutcome` inside `OutcomeReport.process_notes` / the new track variant — one
      schema, no parallel file format (carried in the track variant, per spec)
- [x] Confirm no new transmission mechanism: file-based, manually sent, per spec3 Section 4.8
      (offline signed-file mode) — `OutcomeFileWriter`/`OutcomeFileReader` traits verbatim +
      JSON implementation; `NoSharing` consent blocks export; `Signature` carried through
      untouched
- [ ] Wire into the same `tpt-ai` ingestion path `tpt-fab`'s outcome data already uses
      *(ingestor contract defined verbatim from spec3 Section 4.1 plus a reference
      `RecordingIngestor` in `tpt-ate-aggregate/src/ingest.rs`; the actual `tpt-ai`-side
      pipeline does not exist anywhere in the portfolio yet — cross-repo, tracked below)*
- [x] Note (not build here): bin-sort/yield ground truth is a calibration input for `tpt-ai`'s
      yield-heatmap model and `tpt-fab-process`'s OPC/etch simulator — flagged in
      `tpt-ate-aggregate/src/ingest.rs` docs; the actual calibration wiring belongs to those
      consumers, not `tpt-ate`
- **Milestone: DONE** — `tpt-ate-aggregate/tests/phase3_milestone.rs`: a synthetic end-to-end
  run — design (synthetic stand-in for `tpt-silicon` outputs), simulated wafer sort, simulated
  chiplet assembly, outcome file generated and manually ingested — completes without a design
  partner, and is byte-deterministic across runs.

## Phase 3.5 — Hardening (found by code audit, 2026-09-16)
Bugs and gaps surfaced by a full manual review of `tpt-ate-comm`, `tpt-ate-test`,
`tpt-ate-assembly`, `tpt-ate-aggregate`. Grouped as bug fixes (correctness issues reachable from
real/untrusted input) vs. real-equipment blockers (things that work fine in simulation but would
break or hang against actual ATE/assembly tools).

**Bug fixes:**
- [ ] `tpt-ate-comm/src/secs/item.rs` `decode_item`: cap recursion depth on nested `List` items —
      unbounded recursion on a crafted/corrupt SECS-II message can stack-overflow the process
- [ ] `tpt-ate-comm/src/secs/hsms.rs`: enforce a max frame-size guard on the length prefix before
      allocating, to bound worst-case allocation from a hostile/misbehaving peer
- [ ] `tpt-ate-test/src/stdf/reader.rs` `next_record`: retry on short reads of the 4-byte record
      header instead of treating any partial `Read::read` as `ShortRecordHeader` (matches the
      correct retry loop already used for record bodies) — as written this spuriously fails on
      real streaming (socket/pipe) sources
- [ ] `tpt-ate-test/src/recording.rs` `decode_wafer_map_gdr`: bounds-check `data[1..1+id_len]`
      before slicing; a short/malformed GDR record currently panics instead of returning `None`
- [ ] `tpt-ate-test/src/stdf/codec.rs` `write::cn`/`k_bytes`/`sn`: return `Result` instead of
      `assert!`-panicking when a field exceeds STDF's size limit
- [ ] `tpt-ate-test/src/stdf/records.rs`: either implement the documented tail-byte preservation
      (`Record::parse` currently drops unparsed trailing bytes on partially-modeled known
      records) or fix the module doc comment — the byte-lossless round-trip contract is currently
      false for vendor-extended files

**Real-equipment blockers (works in simulation only):**
- [ ] Add a real TCP transport for HSMS and make `tpt-ate-assembly`'s backends generic over
      `Read + Write` instead of hardcoded to `SimStream` — nothing today can open a socket to
      actual equipment
- [ ] Implement SEMI E37 T3/T5/T6/T7/T8 timers and retry/reconnect logic in the HSMS/GEM layer
      (`hsms.rs` `select`/`transact`, `assembly/link.rs` `await_placement_event`) — a stalled or
      silent peer currently hangs the process forever
- [ ] Ship a CLI binary (`clap`/`toml` are already workspace deps but unused by any `src/bin`) —
      e.g. `tpt-ate connect <ip:port> --config tester.toml`, `stdf convert/inspect/diff` — there is
      currently no way to run any of this against a real line
- [ ] Add `tracing`/`log` instrumentation through the comm and STDF I/O layers — zero production
      observability today
- [ ] Give `tpt-ate-aggregate`'s ingestor durable storage (append/index over a directory of
      outcome files) — the reference `RecordingIngestor` is an in-memory `Vec` that loses
      everything on restart

**CI / process:**
- [ ] Enforce `cargo clippy --all-targets -- -D warnings` in CI (lint config already sets
      `[workspace.lints.clippy] all = "warn"` but CI never runs clippy)
- [ ] Add `cargo fmt --check` and `cargo audit` (or `cargo-deny`) to CI
- [ ] Add adversarial/fuzz-style tests for STDF (truncated body, oversized length, bad FAR) and
      HSMS (malformed frame, mid-session disconnect) — currently only clean-roundtrip paths are
      tested

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
      surfaces during Phase 1/2 integration. No mismatch surfaced in Phases 1–3.)
- [ ] Track as a likely next RFC, not scoped here: pre-RTL / architecture-exploration layer (HLS,
      performance modeling, design-space exploration) as new `tpt-silicon` front-end crates —
      mirrors RFC-001's downstream PCB extension but upstream. Lives in `tpt-silicon`, not
      `tpt-ate`, if it happens.
- [ ] Move RFC-002 schema types (`OutcomeReport`, `ManufacturingTrack`, `ElectricalMeasurement`,
      supporting types) from `tpt-ate-aggregate/src/schema.rs` into the shared schema home once
      `tpt-silicon` materializes RFC-002 in code (they are currently first materializations
      there, kept minimal for that move).

## Adoption & innovation backlog (not gating Phase 4, but improves it)
- [ ] Wafer-map / bin-yield visualizer (SVG or terminal heatmap) rendered from `WaferDieMap` — the
      most demo-able artifact for a prospective design partner, and the data model already exists
- [ ] `tpt-ate-test replay` fuzz/property-test harness driving the deterministic simulator RNG
      with adversarial STDF/HSMS byte streams (also covers the adversarial-testing item above)
- [ ] STDF ⇄ JSON/CSV bridge for the future `tpt-ai` yield-calibration ingestion path — ship this
      before `tpt-ai`'s side exists so `tpt-ate` is the reference shape, not the follower
- [ ] `tpt-ate-simulate` as a standalone binary (not just test-harness code) exposing a GEM host
      on a real port, so an adopter can test their MES integration before owning ATE hardware
- [ ] `examples/quickstart`: one `cargo run --example quickstart` that runs simulator → bin-sort →
      STDF write → `OutcomeReport` → yield summary end-to-end, no real equipment required
- [ ] Sample `tester.toml`/`assembly.toml` config templates with inline comments per SECS/GEM
      parameter (device ID, timeouts once implemented, HSMS mode)
- [ ] "Integrating your ATE" doc: which SECS/GEM messages equipment must support, how to point
      `tpt-ate-test` at real equipment instead of the simulator, self-certification checklist
- [ ] Paired fixture/example showing a real `tpt-silicon` → `tpt-ate-test` handoff end-to-end, now
      that `tpt-silicon`'s DFT/ATPG/layout crates exist (see Cross-repo dependencies below)

## Cross-repo dependencies (tracked here for visibility, not owned by this repo)
- `tpt-silicon`: ~~DFT/ATPG generation~~, ~~interposer/chiplet layout~~, ~~RFC-002 schema~~ —
  **now implemented there** (commit `d27b566`, Aug/Sep 2026): `tpt-dft` (mux-D scan insertion +
  good-machine pattern generation, emitting the `ScanHandoff` JSON whose `AteTestProgram` mirrors
  `tpt-ate-test/src/patterns.rs` field-for-field — the milestone test pins that contract through
  a consumer-side mirror type), `tpt-interposer` (RFC-001 layout whose serde shapes are
  byte-compatible with `tpt-ate-assembly`'s `InterposerLayout`), and `tpt-mfg-schema` (the
  canonical RFC-002 §4 + RFC-003 outcome schema; `tpt-ate-aggregate/src/schema.rs` keeps a
  mirrored copy with identical serde shapes until crates publish cross-repo). The local
  placeholder/stand-in types here stay until `tpt-ate` switches to consuming those crates via
  published packages (path deps across repos would break this repo's CI checkout).
- `tpt-fab`: SECS/GEM implementation pattern to reuse (not share code with) for Phase 1.
  *(Survey finding: `tpt-fab` is spec-only — no SECS/GEM code exists there to reuse; `tpt-ate`'s
  layer is a green-field implementation against the published SEMI standards, which RFC-003
  already mandated.)*
- `tpt-telos`, `tpt-ai`: cross-cutting dependencies per portfolio convention; `tpt-ai` is the
  ingestion target in Phase 3 *(no outcome-ingestion pipeline exists in `tpt-ai` yet — the
  ingestor trait is defined here, wiring pending `tpt-ai`)*.
