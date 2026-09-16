# tpt-ate-assembly

![CI](https://github.com/tpt-solutions/tpt-ate/actions/workflows/ci.yml/badge.svg)

Equipment communication for physical assembly — wire bonding, flip-chip placement, and
chiplet pick-and-place — with placement verification against the design-side interposer
layout (RFC-003, `spec.txt` §2B).

Part of [`tpt-ate`](https://github.com/tpt-solutions/tpt-ate). Equipment communication
lives in [`tpt-ate-comm`](../tpt-ate-comm) (SECS/GEM per SEMI E5/E30/E37); this crate adds
the assembly-domain half.

## Design: one core, swappable techniques

Like `tpt-fab-litho`'s pluggable-backend pattern from RFC-002, assembly technique is a
swappable [`AssemblyBackend`] behind one core. The execution flow — GEM Select +
handshake, one S2F41 host command per placement (S2F42 HCACK back), one S6F11
placement-complete event per site — is identical regardless of which physical technique an
OSAT runs, and the core never branches on technique: everything technique-specific
(process parameters, the GEM `RCMD` label, guaranteed tolerances) lives behind the trait.
The milestone test drives all three backends uniformly through `Box<dyn AssemblyBackend>`.

## What it implements

- **Placement reference** ([`layout`]) — `InterposerLayout`: chiplet footprints, intended
  sites, and die-to-die links in nanometer `i64` coordinates (bottom-left corners,
  mirroring `tpt-silicon`'s conventions), so the RFC-001 layout carries through
  unmodified. The serde shapes are byte-compatible with `tpt-silicon`'s `tpt-silicon-interposer`
  crate — its exported JSON deserializes here directly.
- **Backends** ([`techniques`]) — `WireBondBackend`, `FlipChipBackend`,
  `ChipletPickPlaceBackend`, each carrying its process parameters (wire pitch, bump pitch,
  nozzle, tolerances) behind [`AssemblyBackend`].
- **Equipment link** ([`link`]) — `AssemblyHost`: the host side of an assembly machine's
  GEM session (`connect` runs Select + S1F1/S1F13; `place` issues S2F41 and waits for the
  matching S6F11).
- **Equipment simulator** ([`equipment`]) — `AssemblyEquipment`: the machine side, with
  deterministic per-command placement jitter (seeded, like all portfolio simulators).
- **Verification** ([`verify`]) — technique-agnostic checks of executed placements against
  the reference layout: off-target (Manhattan deviation vs. tolerance), out-of-bounds,
  overlap, orientation mismatch, duplicate and missing sites, unknown sites.

## Installation

```toml
[dependencies]
tpt-ate-assembly = "0.1"
```

## Usage

Run a pick-and-place backend against the simulated placer and verify the result:

```rust
use tpt_ate_assembly::backend::{
    AssemblyBackend, AssemblyTechnique, PlacementCommand, PlacementPlan,
};
use tpt_ate_assembly::equipment::AssemblyEquipment;
use tpt_ate_assembly::layout::{InterposerLayout, PointNm};
use tpt_ate_assembly::link::AssemblyHost;
use tpt_ate_assembly::techniques::ChipletPickPlaceBackend;
use tpt_ate_comm::secs::transport::duplex;

let layout = InterposerLayout::inline_row("pkg-a1", &[("core", 2500, 2500)], 500);
let plan = PlacementPlan {
    plan_id: "run-1".into(),
    commands: layout
        .sites
        .iter()
        .map(|site| PlacementCommand {
            site_id: site.site_id.clone(),
            chiplet_id: site.chiplet_id.clone(),
            target: site.position,
            orientation: site.orientation,
        })
        .collect(),
};

// Simulated placer on a companion thread (a TCP stream works identically).
let (equip_half, host_half) = duplex();
let equipment = AssemblyEquipment::new(AssemblyTechnique::ChipletPickPlace, 0xC0FFEE);
let commands = plan.commands.clone();
let machine = std::thread::spawn(move || equipment.run(equip_half, 0, &commands));

let host = AssemblyHost::connect(host_half, 0).unwrap();
let mut backend = ChipletPickPlaceBackend::new(host, 300.0, 8.0);
let records = backend.place(&plan).unwrap();
machine.join().unwrap().unwrap();

let tolerance = tpt_ate_assembly::techniques::simulated_equipment_tolerance_nm();
let report = tpt_ate_assembly::verify_placements(&records, &layout, tolerance);
assert!(report.is_ok(), "{:?}", report.violations);
```

Any `AssemblyBackend` implementation is interchangeable at the call site — swap
`ChipletPickPlaceBackend` for `FlipChipBackend` or `WireBondBackend` (or hold one behind
`Box<dyn AssemblyBackend>`) with no changes to the driving code.

## Position in tpt-ate

```text
tpt-silicon (RFC-001 interposer/chiplet layout)
      │  JSON (tpt-silicon-interposer's InterposerLayout — byte-compatible)
      ▼
tpt-ate-assembly ──drives──▶ bonders / placers over SECS/GEM ──▶ verified placements
      └─────────────────────▶ tpt-ate-aggregate (as-built geometry in OutcomeReport)
```

## Scope and limitations

- Placement verification is geometry over reported positions; process-level checks
  (wire-bond loop profiles, bump metrology) are future work.
- The equipment simulator models placement jitter only — no travel-time, no multi-site,
  no conveyor modeling.
- Falling-edge and multi-domain sequencing is a test-equipment concern; assembly machines
  here run a single GEM session per backend.

## Testing

`cargo test -p tpt-ate-assembly` — layout/validation unit tests, verification unit tests
per violation class, and `tests/phase2_milestone.rs`, the Phase 2 gate: all three
techniques validate against a synthetic 3-chiplet interposer layout through
`Box<dyn AssemblyBackend>`, with a negative test pinning that a shifted placement is
flagged.

## License

Dual-licensed under MIT or Apache-2.0 — see `LICENSE-MIT` and `LICENSE-APACHE` in the
repository root. Copyright TPT Solutions.

[`AssemblyBackend`]: https://docs.rs/tpt-ate-assembly/latest/tpt_ate_assembly/backend/trait.AssemblyBackend.html
[`layout`]: https://docs.rs/tpt-ate-assembly/latest/tpt_ate_assembly/layout/index.html
[`techniques`]: https://docs.rs/tpt-ate-assembly/latest/tpt_ate_assembly/techniques/index.html
[`link`]: https://docs.rs/tpt-ate-assembly/latest/tpt_ate_assembly/link/index.html
[`equipment`]: https://docs.rs/tpt-ate-assembly/latest/tpt_ate_assembly/equipment/index.html
[`verify`]: https://docs.rs/tpt-ate-assembly/latest/tpt_ate_assembly/verify/index.html
