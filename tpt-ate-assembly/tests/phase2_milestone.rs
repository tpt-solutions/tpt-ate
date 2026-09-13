//! Phase 2 milestone gate (RFC-003 Section 4, Phase 3 in spec numbering):
//!
//! "A simulated chiplet placement run validates against a synthetic
//! interposer layout with no equipment-technique assumptions hard-coded
//! into the core."
//!
//! Three different technique backends run real GEM sessions (Select,
//! handshake, S2F41 host commands, S6F11 placement events) against the
//! simulated assembly equipment — driven uniformly through
//! `Box<dyn AssemblyBackend>` — and their as-placed results are verified
//! against the reference layout by the technique-agnostic core.

use std::thread;

use tpt_ate_assembly::backend::{
    AssemblyBackend, AssemblyTechnique, PlacementCommand, PlacementPlan,
};
use tpt_ate_assembly::equipment::AssemblyEquipment;
use tpt_ate_assembly::layout::{DieSpec, InterposerLayout, Orientation, PointNm, SiteSpec};
use tpt_ate_assembly::link::AssemblyHost;
use tpt_ate_assembly::techniques::{
    simulated_equipment_tolerance_nm, ChipletPickPlaceBackend, FlipChipBackend, WireBondBackend,
};
use tpt_ate_assembly::{verify_placements, PlacementRecord};
use tpt_ate_comm::secs::transport::duplex;

const SESSION_ID: u16 = 0;

/// Synthetic interposer layout: three chiplet sites (the RFC-001-shaped
/// reference `tpt-silicon` will one day export directly).
fn interposer_layout() -> InterposerLayout {
    InterposerLayout {
        design_name: "pkg-a1-3chiplet".into(),
        extent_nm: (12_000, 10_000),
        dies: vec![
            DieSpec { chiplet_id: "core".into(), width_nm: 2500, height_nm: 2500 },
            DieSpec { chiplet_id: "io".into(), width_nm: 1500, height_nm: 3000 },
            DieSpec { chiplet_id: "hbm".into(), width_nm: 3000, height_nm: 2000 },
        ],
        sites: vec![
            SiteSpec {
                site_id: "s_core".into(),
                chiplet_id: "core".into(),
                position: PointNm::new(1000, 4000),
                orientation: Orientation::R0,
            },
            SiteSpec {
                site_id: "s_io".into(),
                chiplet_id: "io".into(),
                position: PointNm::new(4500, 3500),
                orientation: Orientation::R0,
            },
            SiteSpec {
                site_id: "s_hbm".into(),
                chiplet_id: "hbm".into(),
                position: PointNm::new(7500, 4000),
                orientation: Orientation::R0,
            },
        ],
    }
}

fn plan_for(layout: &InterposerLayout, plan_id: &str) -> PlacementPlan {
    PlacementPlan {
        plan_id: plan_id.into(),
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
    }
}

/// Drive one technique end-to-end: simulated equipment on a companion
/// thread, host link over the in-memory duplex, placement executed through
/// the `AssemblyBackend` trait object.
fn run_technique(
    technique: AssemblyTechnique,
    layout: &InterposerLayout,
    seed: u64,
) -> Vec<PlacementRecord> {
    let plan = plan_for(layout, &format!("{technique:?}-run"));
    let (equip_half, host_half) = duplex();
    let equipment = AssemblyEquipment::new(technique, seed);
    let commands = plan.commands.clone();
    let machine = thread::spawn(move || equipment.run(equip_half, SESSION_ID, &commands));

    let host = AssemblyHost::connect(host_half, SESSION_ID).expect("connect + handshake");
    let mut backend: Box<dyn AssemblyBackend> = match technique {
        AssemblyTechnique::WireBond => Box::new(WireBondBackend::new(host, 50.0, 5000.0)),
        AssemblyTechnique::FlipChip => Box::new(FlipChipBackend::new(host, 130.0, 5.0)),
        AssemblyTechnique::ChipletPickPlace => {
            Box::new(ChipletPickPlaceBackend::new(host, 300.0, 8.0))
        }
    };
    assert_eq!(backend.technique(), technique);

    let records = backend.place(&plan).expect("placement run");
    machine.join().expect("equipment thread").expect("equipment flow");
    records
}

#[test]
fn phase2_milestone_all_techniques_validate_against_layout() {
    let layout = interposer_layout();
    let tolerance_nm = simulated_equipment_tolerance_nm();

    for technique in [
        AssemblyTechnique::WireBond,
        AssemblyTechnique::FlipChip,
        AssemblyTechnique::ChipletPickPlace,
    ] {
        let records = run_technique(technique, &layout, 0xBEEF);
        assert_eq!(records.len(), layout.sites.len(), "{technique:?}: every site placed");
        assert!(records.iter().all(|r| r.technique == technique));

        let report = verify_placements(&records, &layout, tolerance_nm);
        assert!(report.is_ok(), "{technique:?} placements must validate: {:?}", report.violations);
        // As-placed positions really do carry equipment jitter (not just
        // echo of nominal), proving the GEM round trip happened.
        assert!(
            records.iter().any(|r| r.placed != r.nominal),
            "{technique:?}: equipment jitter should be visible"
        );
    }
}

#[test]
fn verification_flags_a_shifted_placement() {
    let layout = interposer_layout();
    // Simulate a mis-placed die: 500nm off target in X.
    let mut records = plan_for(&layout, "bad")
        .commands
        .iter()
        .map(|cmd| PlacementRecord {
            site_id: cmd.site_id.clone(),
            chiplet_id: cmd.chiplet_id.clone(),
            technique: AssemblyTechnique::ChipletPickPlace,
            nominal: cmd.target,
            placed: if cmd.site_id == "s_hbm" {
                PointNm::new(cmd.target.x + 500, cmd.target.y)
            } else {
                cmd.target
            },
            orientation: cmd.orientation,
        })
        .collect::<Vec<_>>();
    let _ = &mut records;

    let report = verify_placements(&records, &layout, 50);
    assert!(!report.is_ok());
    assert!(report
        .violations
        .iter()
        .any(|v| matches!(v, tpt_ate_assembly::PlacementViolation::OffTarget { site_id, deviation_nm, .. }
            if site_id == "s_hbm" && *deviation_nm == 500)));
}

#[test]
fn plans_are_serializable_for_file_based_exchange() {
    // The plan hand-off is file-based (no live API), so it must survive
    // JSON serialization untouched.
    let layout = interposer_layout();
    let plan = plan_for(&layout, "file-run");
    let json = serde_json::to_string_pretty(&plan).expect("serialize");
    let round_tripped: PlacementPlan = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(round_tripped, plan);
}
