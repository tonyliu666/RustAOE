# Benchmarks

Results of `cargo bench -p sim --bench step` (`crates/sim/benches/step.rs`).

Budget from the PoC spec: `step` with 200 units moving stays under 5 ms per
tick. Flow field construction is measured on its own.

## 2026-10-24: Phase 0 of the exe-parity plan

Machine: Intel Core i5-14600KF (20 threads), 31 GiB RAM, Linux 6.8.0,
`rustc 1.96.0-nightly (80282b130 2026-03-06)`, release profile.

| Benchmark | Median | Budget | Headroom |
| --- | --- | --- | --- |
| `step 200 units moving` (128x128, wall with a 4-tile gap) | 44.35 µs | 5 ms | about 110x |
| `flow field 128x128` (same wall) | 281.45 µs | none set | n/a |

Criterion range: step 44.20–44.53 µs, flow field 280.58–282.45 µs.

A cold flow field (281 µs) costs about 6 ticks of movement. The cache keeps it
off the per-tick path. A tick that builds both players' fields still fits the
budget at about 0.6 ms.

Property tests from the spec's testing section, all present:

| Spec property | Test |
| --- | --- |
| Determinism | `determinism.rs::the_same_script_run_twice_agrees_at_every_tick` |
| Replay fidelity | `determinism.rs::a_recording_plays_back_to_the_checksums_it_recorded` |
| No tunnelling | `determinism.rs::no_unit_ever_stands_in_a_blocked_tile` |
| Gold conservation | `determinism.rs::gold_is_never_created_or_destroyed` |
| Fixed-point round-trip | `fx_properties.rs::multiplication_then_division_round_trips` |

Re-run this benchmark at the end of Phase 6 (unit roster and combat), as the
plan requires.
