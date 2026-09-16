# tpt-ate-comm

![CI](https://github.com/tpt-solutions/tpt-ate/actions/workflows/ci.yml/badge.svg)

SECS/GEM equipment communication for the TPT Solutions manufacturing portfolio: the shared
core that lets `tpt-ate`'s test and assembly tools talk to production-floor equipment,
built directly against the published SEMI standards.

Part of [`tpt-ate`](https://github.com/tpt-solutions/tpt-ate) (RFC-003: Test & Assembly
Operational Software). This is `tpt-ate`'s **own** implementation of the protocol family —
per the RFC-003 §5 decision it is deliberately not shared with `tpt-fab`, which has its own
equipment-communication layer one manufacturing stage earlier.

## What it implements

| Layer | Standard | Contents |
| --- | --- | --- |
| [`secs::item`] | SEMI E5 (SECS-II) | Typed data items (`L`, `B`, `BOOLEAN`, `A`, `J`, `I1`–`I8`, `F4`/`F8`, `U1`–`U8`) with a wire encoder/decoder and SML-ish `Display` |
| [`secs::hsms`] | SEMI E37/E37.1 (HSMS-SS) | 10-byte-header message framing, Select/Linktest/Separate/Reject control messages, and a session type that walks the NOT CONNECTED → NOT SELECTED → SELECTED state machine |
| [`secs::gem`] | SEMI E30 (GEM) | The transactions the ATE flow uses: S1F1/F2, S1F13/F14, S2F41/F42 host commands, S6F11/F12 event reports, plus the equipment- and host-side session wrappers |
| [`secs::transport`] | — | An in-memory blocking duplex pipe with TCP-like semantics, so the whole stack is exercisable without network I/O |
| [`rng`] | — | Seeded xorshift64* used by the equipment simulators (`tpt-ate-test`, `tpt-ate-assembly`) for fully reproducible behavior |

Transport is generic over `Read + Write`: the in-memory duplex in tests and simulation, a
`TcpStream` on a real floor.

## Installation

```toml
[dependencies]
tpt-ate-comm = "0.1"
```

## Usage

A host (controller) connecting to an equipment over the in-memory transport and running
the standard GEM handshake:

```rust
use tpt_ate_comm::secs::gem::HostGem;
use tpt_ate_comm::secs::hsms::HsmsSession;
use tpt_ate_comm::secs::item::SecsItem;
use tpt_ate_comm::secs::transport::duplex;

let (equipment_side, host_side) = duplex();

// The equipment side accepts Select and answers S1F1/S1F13 — see
// `tpt-ate-test`'s `SimulatedTester` and `tpt-ate-assembly`'s
// `AssemblyEquipment` for two complete equipment implementations.
std::thread::spawn(move || {
    let mut session = HsmsSession::new(equipment_side, 0);
    session.accept_select().unwrap();
    // ... serve the handshake, then send/receive transactions
});

// Host side: Select, identify, establish communications.
let mut session = HsmsSession::new(host_side, 0);
session.select().unwrap();
let mut gem = HostGem::new(session);
let (model, rev) = gem.online_data().unwrap();          // S1F1 → S1F2
let established = gem.establish_communications().unwrap(); // S1F13 → S1F14
assert!(established);

// Event reports flow as S6F11 with an S6F12 acknowledge, and remote
// commands as S2F41 with an S2F42 HCACK:
let body = tpt_ate_comm::secs::gem::s2f41_host_command(
    "PLACE",
    &[("SITE".to_string(), SecsItem::a("s_core"))],
);
```

The HSMS header layout matches SEMI E37 (W-bit + stream in byte 3, function in byte 4,
PType/SType in bytes 5–6, system bytes in 7–10); a wire-format unit test pins this. One
documented E37 subtlety is handled explicitly: the Select.rsp status lives in header byte
10, overlapping the echoed system bytes, so the session takes the first `Select.rsp` as
authoritative rather than matching system bytes.

## Position in tpt-ate

Extracted from `tpt-ate-test` into this shared crate once `tpt-ate-assembly` made the
duplication concrete (the RFC-003 §5 locked decision, resolved during Phase 2):

```text
tpt-ate-comm  ←  tpt-ate-test      (automated test equipment)
              ←  tpt-ate-assembly  (bonders, placers)
```

Neither crate shares it with `tpt-fab`. Equipment-specific behavior (wafer sort, placement)
lives in the consumers; this crate is protocol only.

## Scope and limitations

- Synchronous, blocking I/O — the portfolio has no async runtime dependency.
- Single selected session (HSMS-SS, SEMI E37.1); multiplexed HSMS-GS is out of scope.
- Deselect req/rsp frames are carried but not used by the ATE flow.
- Timers T3–T8 are modeled as documented constants for the simulators; production
  timeout wiring belongs to the transport caller.

## Testing

`cargo test -p tpt-ate-comm` covers SECS-II round-trips (including 3-byte length encodings),
HSMS frame round-trips and truncation handling, S6F11/S2F41 parse/serialize, and duplex
pipe semantics (blocking reads, EOF on peer drop).

## License

Dual-licensed under MIT or Apache-2.0 — see `LICENSE-MIT` and `LICENSE-APACHE` in the
repository root. Copyright TPT Solutions.
