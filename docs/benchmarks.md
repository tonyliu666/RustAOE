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

## 2026-10-25: end of Phase 1.5

Same machine and toolchain. Up to 8 players plus Gaia, four resources, a
wider checksum, and unit and building types as data.

| Benchmark | Median | Range |
| --- | --- | --- |
| `step 200 units moving` | 46.13 µs | 45.93–46.41 µs |
| `flow field 128x128` | 290.51 µs | 287.47–294.01 µs |

About 4% slower per step than Phase 0, still over 100x under the 5 ms budget.

Re-run this benchmark at the end of Phase 6 (unit roster and combat), as the
plan requires.

## 2026-10-25: end of Phase 1.6

Same machine and toolchain. Range measured between edges (`World::in_reach`).

| Benchmark | Median | Range |
| --- | --- | --- |
| `step 200 units moving` | 47.58 µs | 47.35–47.85 µs |
| `flow field 128x128` | 294.56 µs | 292.31–296.97 µs |

About 3% slower per step than the end of Phase 1.5, still about 105x under
the 5 ms budget.

**Sprite loading** (`load_images` on the real install, release build, median
of five; `cargo test --release -p app --test load_time -- --ignored
--nocapture` with `AOE_ASSETS` set, and `AOE_CONTENT` naming a content file):

| Content | Types | Load |
| --- | --- | --- |
| Default | 2 unit, 2 building (the town centre in 8 parts) | 55.3 ms |
| `datconv --all`, British | 201 unit, 116 building | 187.6 ms |

The Phase 1.6 baseline was 145 ms for the default content, before archives
were parsed once. The full table is well under the 2-second limit the Phase
1.6 spec set for decoding lazily, so every type is still decoded at startup,
nine times (one per player colour). Lookups stay a direct index:
`Content::unit` and `Sprites::unit` index a `Vec` by type ID.

**Per-lookup cost** (`content lookup` in `crates/sim/benches/step.rs`): 1000
lookups of a unit type's numbers in a 201-type table take 1.17 µs (1.16–1.17),
about 1.2 ns each: a direct index, whatever the table's size.
