# Phase 1: Data-driven content

**Date:** 2026-10-24
**Status:** Done (2026-10-24), pending review
**Plan:** [`2026-10-24-exe-parity-plan.md`](../plans/2026-10-24-exe-parity-plan.md), Phase 1
**Builds on:** [`2026-08-04-rts-poc-design.md`](2026-08-04-rts-poc-design.md), "Game content" and "Error handling"

## Goal

Unit and building numbers come from a RON file instead of `const fn` matches in
`crates/sim/src/entity.rs`. Changing a number in the file changes gameplay with
no code change.

## Reference note

`aoc.exe` keeps no unit stats in the binary. They live in
`data/empires2_x1_p1.dat` (see `reverse/README.md`, "Not in the binary"). This
phase uses our own schema. Reading the `.dat` is an open question in the plan.

## Design

### `sim::Content`

A plain struct, owned by `World`, read by every system that used to call
`UnitKind::stats()`, `UnitKind::cost()`, `UnitKind::train_ticks()`,
`BuildingKind::max_hp()`, `BuildingKind::is_drop_off()`, or
`BuildingKind::trains()`. Those functions are removed, so there is one source
of truth.

```rust
pub struct Content {
    pub villager: UnitStats,
    pub soldier: UnitStats,
    pub town_center: BuildingStats,
    pub barracks: BuildingStats,
    pub gather: GatherStats,
}
// UnitStats gains `cost` and `train_ticks`.
pub struct BuildingStats { pub max_hp: i32, pub drop_off: bool, pub trains: Vec<UnitKind> }
pub struct GatherStats { pub carry_capacity: i32, pub ticks_per_gold: u32 }
```

- `Content::default()` holds the PoC spec's numbers, so every existing test and
  replay keeps its behaviour.
- `Content::validate(&self) -> Result<(), ContentError>` checks the numbers. It
  is pure: no I/O, no floats. `step` still cannot fail.
- `World::with_content(seed, map, content)` builds a world from content.
  `World::new(seed, map)` uses the default.
- `WorldView::content()` exposes it to `app` (health bars read `max_hp`).

### What the removed functions did

Before this phase, each number lived in a `const fn` that matched on the
entity kind and returned a hard-coded value. Every system that needed a
number called one of these functions. This table shows what each one returned,
who called it, and what replaces it.

| Function | Returned | Called by | Why the game needs it | Replacement |
| --- | --- | --- | --- | --- |
| `UnitKind::stats()` | `UnitStats`: `max_hp`, `speed`, `damage`, `range`, `attack_cooldown`, `radius` | `Unit::new`, `move_unit`, `separation`, `radius_of`, `reach`, `order_attack`, `run_combat` in `world.rs`; health bars in `app/src/draw.rs` | `max_hp` sets a new unit's health. `speed` sets how far a unit moves per tick. `radius` sets how much room a unit takes, for crowd spacing and for how close an attacker must get. `range` sets attack reach. `damage` is the HP removed per hit, and zero means the unit cannot be ordered to attack. `attack_cooldown` is the wait between hits. | `content.unit(kind)` |
| `UnitKind::cost()` | Gold to train the unit | `order_train` | Charges gold when a unit is queued, and refuses the order when the player cannot pay. Charging at queue time stops five queued units from spending the same gold five times. | `content.unit(kind).cost` |
| `UnitKind::train_ticks()` | Ticks to train the unit | `order_train`, `run_production` | Starts the countdown for the unit at the head of a building's queue. The unit appears when the countdown reaches zero, and the next queued unit starts its own countdown. | `content.unit(kind).train_ticks` |
| `BuildingKind::max_hp()` | Building hit points | `Building::new`; health bars in `app/src/draw.rs` | Sets a new building's health. A player loses when every building reaches zero, so this decides how long a base survives an attack. | `content.building(kind).max_hp` |
| `BuildingKind::is_drop_off()` | Whether villagers may deposit gold here | Drop-off search in the gather loop (`run_gathering`) | A villager with a full load walks to the nearest building of its own where this is true. Only the town centre was a drop-off. | `content.building(kind).drop_off` |
| `BuildingKind::trains(unit)` | Whether this building can train `unit` | `order_train` | Rejects orders that make no sense, such as a town centre training a soldier. The simulation checks every command because commands will come from the network later. | `content.building(kind).trains.contains(&unit)` |

Two constants on `Unit` follow the same pattern and move in the same step as
the building functions:

| Constant | Value | Use | Replacement |
| --- | --- | --- | --- |
| `Unit::CARRY_CAPACITY` | 10 | Gold a villager carries before walking back to drop off | `content.gather.carry_capacity` |
| `Unit::MINE_TICKS_PER_GOLD` | 40 | Ticks at the mine per gold extracted | `content.gather.ticks_per_gold` |

**Why remove them instead of keeping them as defaults:** if the old functions
stayed, two places would hold each number. A system that still called
`kind.cost()` would ignore the RON file without any error, and a balance
change would only partly apply. Removing them makes the compiler find every
caller. Each caller then has to read `Content`, so the file is the only source.
The old numbers stay in `Content::default()`.

**Progress:** `UnitKind::stats()`, `cost()`, and `train_ticks()` were removed
in steps 1.1 and 1.2. The building functions and the two constants were
removed in step 1.3.

### Not moved in this phase

- **Footprint sizes** (`BuildingKind::size()`, `Mine::SIZE`). `MapSpec`,
  picking, and drawing read them with no world in hand. They are map geometry
  until Phase 3 (construction) owns placement.
- **Mine starting gold.** Already a per-mine `MapSpec` value.
- **Engine constants** (`STUCK_LIMIT`, `PATH_BUDGET_PER_TICK`, `QUEUE_LIMIT`,
  `INTERACT_RANGE`). They tune the engine, not the game.

### Validation rules

| Field | Rule |
| --- | --- |
| unit `max_hp`, building `max_hp` | positive |
| unit `speed`, `radius` | positive |
| unit `speed` | at most 1/2 tile per tick: a faster step overshoots the 1.5-tile arrival window, and a step over one tile can hop a wall |
| unit `damage`, `range`, `cost` | not negative |
| unit `attack_cooldown` | positive when `damage` is positive |
| unit `train_ticks` | positive |
| `gather.carry_capacity`, `gather.ticks_per_gold` | positive |

The error names the field, for example `villager.max_hp: must be positive`.

Damage and banking use saturating arithmetic in `world.rs`, so content at the
extremes (`damage: i32::MAX`, a full treasury) cannot panic a debug build or
wrap in a release build. The two builds must play the same game.

### Replay

`Replay` gains `content: Content` with `#[serde(default)]`. `verify` builds its
world from the recorded content. `app::load` validates a replay's content like
a content file and returns `ReplayError::Invalid` on failure, because a replay
is a shared bug report and therefore user input. A replay file from before this phase has no
`content` field and loads with the default content, which is what it was
recorded with. This is **not** a replay break: tick order and checksums of
default-content games are unchanged.

### RON file (`app`)

- `crates/app/content/default.ron` holds the default numbers. The binary embeds
  it with `include_str!` and accepts `--content FILE` to load another.
- Fixed-point fields are written as a fraction, `speed: (1, 8)`, meaning 1/8
  tile per tick. `app` converts them; `sim` never parses text.
- `app::parse_content(&str)` and `app::load_content(&Path)` return
  `Result<Content, ContentFileError>`: `Io`, `Malformed` (not RON, a missing
  section or field, or an unknown field), or `Invalid` (a zero or negative
  denominator, a fraction too large for 16.16 fixed point, or a failed
  `validate`). None of them panics.
- A recorded game stores the content it ran with. Playback uses the recording's
  content, not `--content`.

## Gate

1. A test parses RON with a changed stat and shows the game plays differently
   (for example a cheaper villager costs less gold to train).
2. A test loads a bad file and gets an error, not a panic.
3. `default.ron` parses to exactly `Content::default()`.
4. `cargo test --workspace` passes, and the existing determinism and replay
   property tests are unchanged.

## Gate results (2026-10-24)

| Gate | Evidence |
| --- | --- |
| 1. Changed stat changes the game | `app/tests/content_file.rs::changing_a_number_in_the_file_changes_the_game`: `cost: 30` in the file, training spends 30 gold |
| 2. Bad file is an error, not a panic | `content_file.rs`: not RON, missing section, unknown field, invalid number, zero denominator, too-large fraction, missing file. The binary prints the error and exits 1 for an invalid, a garbage, and a missing file |
| 3. `default.ron` equals `Content::default()` | `content_file.rs::the_shipped_file_holds_the_default_content` |
| 4. Workspace green, determinism and replay tests unchanged | `cargo test --workspace`: 202 passed, 0 failed (after the review fix pass). `cargo clippy --workspace --all-targets`: no warnings |

Not verified by a test: the `--content` flag in a live game. The binary starts
and runs with a valid custom file and with the default, but the in-game effect
was not checked by eye.
