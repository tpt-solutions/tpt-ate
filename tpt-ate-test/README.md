# tpt-ate-test

![CI](https://github.com/tpt-solutions/tpt-ate/actions/workflows/ci.yml/badge.svg)

Test execution for automated test equipment: STDF V4 recording, wafer-map bin sorting, and
a deterministic ATE simulator — the operational counterpart to `tpt-silicon`'s design-time
DFT/ATPG work (RFC-003, `spec.txt` §2A).

Part of [`tpt-ate`](https://github.com/tpt-solutions/tpt-ate). The equipment-communication
layer lives in [`tpt-ate-comm`](../tpt-ate-comm) (SECS/GEM per SEMI E5/E30/E37); this crate
consumes it and adds the test-domain half.

## What it implements

- **STDF V4 read/write** ([`stdf`]) — the industry-standard result-data format, distinct
  from SECS/GEM (which is the communication layer). Covers the core production record set:
  `FAR`, `ATR`, `MIR`, `MRR`, `PCR`, `HBR`, `SBR`, `PMR`, `WIR`, `WRR`, `WCR`, `PTR`,
  `FTR`, `TSR`, `GDR`, `DTR`, `BPS`, `EPS`. Conditional-presence flag rules (PTR's optional
  scaling/limit fields, FTR's optional tail) follow the V4 spec; records this crate doesn't
  model survive verbatim as `Record::Unknown`, so a read→write pass is byte-lossless even
  on files with unsupported records.
- **Wafer map** ([`wafer`]) — `WaferDieMap`: per-die state keyed by physical die location,
  carried through unmodified from `tpt-silicon`'s layout output (RFC-003 §5 decision).
- **Bin taxonomy and sorting** ([`bins`], [`bin_sort`]) — hardware/software bin
  definitions, first-failure bin assignment with per-test soft-bin policy from the test
  program, and the run-level tallies the STDF summary records and `TestOutcome` are built
  from.
- **Test program input** ([`patterns`]) — the executable shape `tpt-silicon`'s ATPG/DFT
  flow hands over (parametric tests with limits, pattern groups, per-test soft bins).
  Explicitly *no* authoring semantics: fault targeting and pattern compaction stay in
  `tpt-silicon`. The JSON handoff file is produced by `tpt-silicon`'s `tpt-silicon-dft` crate and
  deserializes into this shape field-for-field.
- **Equipment simulator** ([`sim`]) — a seeded, deterministic tester that executes a test
  program per die and speaks the full GEM flow (Select, S1F13 handshake, S6F11 per-die
  event reports) over any transport — including the in-memory duplex, so the whole flow
  runs without real ATE access.
- **Run recording** ([`recording`]) — assembles a completed wafer sort into its STDF
  record stream (FAR/ATR/MIR up front, WIR/WCR per wafer, PTR/FTR per die, TSR synopses,
  WRR/PCR/HBR/SBR counters, MRR last), with the wafer map also embedded in a GDR record.

## Installation

```toml
[dependencies]
tpt-ate-test = "0.1"
```

## Usage

Execute a program against the simulated tester and bin-sort the results onto the wafer map:

```rust
use tpt_ate_test::bins::BinTaxonomy;
use tpt_ate_test::bin_sort::sort_wafer;
use tpt_ate_test::sim::{FaultModel, SimulatedTester};
use tpt_ate_test::wafer::{DieCoord, WaferDieMap};

let mut wafer = WaferDieMap::rectangular("W01", 5, 5, 120.0, 160.0);
let tester = SimulatedTester::new(
    program, // your TestProgram: tests, limits, pattern groups (see `patterns`)
    FaultModel::deterministic(0xA11CE, vec![DieCoord::new(1, 1), DieCoord::new(3, 2)]),
);

let results: Vec<_> = wafer
    .dies
    .keys()
    .copied()
    .map(|coord| tester.test_die(coord))
    .collect();
let unmatched = sort_wafer(&results, &mut wafer, &program, &BinTaxonomy::defaults());
assert_eq!(unmatched, 0);
let (tested, passed) = wafer.tested_counts();
```

The GEM round trip — equipment side and host side over an in-memory HSMS connection — is
what `tests/phase1_milestone.rs` exercises end to end. Recording the run to a real STDF
file:

```rust
use tpt_ate_test::stdf::StdfWriter;

let records = tpt_ate_test::recording::build_wafer_run_records(
    &program, &wafer, &results, &tally, &taxonomy, &metadata,
);
let file = std::fs::File::create("run.stdf")?;
StdfWriter::new(std::io::BufWriter::new(file)).write_all(&records)?;
```

Reading it back:

```rust
use tpt_ate_test::stdf::StdfReader;

let mut reader = StdfReader::new(std::io::BufReader::new(std::fs::File::open("run.stdf")?));
let records = reader.read_all()?; // endianness learned from the FAR record
```

## Position in tpt-ate

```text
tpt-silicon (DFT netlist + ATPG patterns + layout)
      │  JSON handoff (tpt-silicon-dft's ScanHandoff file)
      ▼
tpt-ate-test ──executes──▶ real or simulated ATE ──▶ STDF + WaferDieMap
      └────────────────────▶ tpt-ate-aggregate (TestOutcome → OutcomeReport)
```

## Scope and limitations

- Test program *authoring* (fault targeting, pattern compaction) is out of scope — that is
  `tpt-silicon`'s job; this crate executes what it is given.
- The `patterns::TestProgram` shape is a placeholder until `tpt-silicon`'s ATPG/DFT crates
  land; it carries exactly what bin sorting needs and is already the wire shape `tpt-silicon-dft`
  emits.
- The STDF record set covers the core production flow; exotic records round-trip verbatim
  but are not parsed into typed structs.
- PTR `TEST_FLG` semantics are exposed for the four stable bits (failed, unreliable,
  pass/fail field, not executed); the raw byte is always carried.

## Testing

`cargo test -p tpt-ate-test` — 33 unit tests plus `tests/phase1_milestone.rs`, the Phase 1
gate: a simulated 25-die wafer sort over in-memory HSMS/GEM produces a valid STDF file
whose bins, counters, and embedded wafer map match the seeded fault model on read-back.

## License

Dual-licensed under MIT or Apache-2.0 — see `LICENSE-MIT` and `LICENSE-APACHE` in the
repository root. Copyright TPT Solutions.

[`stdf`]: https://docs.rs/tpt-ate-test/latest/tpt_ate_test/stdf/index.html
[`wafer`]: https://docs.rs/tpt-ate-test/latest/tpt_ate_test/wafer/index.html
[`bins`]: https://docs.rs/tpt-ate-test/latest/tpt_ate_test/bins/index.html
[`bin_sort`]: https://docs.rs/tpt-ate-test/latest/tpt_ate_test/bin_sort/index.html
[`patterns`]: https://docs.rs/tpt-ate-test/latest/tpt_ate_test/patterns/index.html
[`sim`]: https://docs.rs/tpt-ate-test/latest/tpt_ate_test/sim/index.html
[`recording`]: https://docs.rs/tpt-ate-test/latest/tpt_ate_test/recording/index.html
