# RTS Proof of Concept — Design

**Date:** 2026-08-04
**Status:** Approved
**Scope:** First vertical slice of a real-time strategy game in Rust

## Purpose

Build a playable hotseat RTS slice that proves the architecture can carry a shipped game later. The slice exists to answer two questions before any content is built:

1. Does a deterministic, fixed-point simulation hold up under real gameplay?
2. Does pathfinding stay inside the tick budget at 200 units?

Long-term intent is to ship an RTS to online players. This spec covers only the proof of concept. Networking, AI opponents, fog of war, and a production renderer are deliberately deferred to their own specs, but the architecture here is chosen so that none of them requires a rewrite.

## Goals

- A playable game loop: gather gold, train units, fight, win.
- Deterministic simulation verified by property tests and replay checksums.
- 200 units on a 128x128 map at under 5 ms per simulation tick.
- Clean separation between simulation, rendering, and application wiring, enforced by the compiler.

## Non-goals

Each of these is a follow-on spec, not an omission:

AI opponent, fog of war, in-game building construction, networking and lockstep transport, 3D or Bevy renderer, sound, multiple unit types beyond the two specified, tech tree, mid-game save/load, map editor.

## Key decisions

| Decision | Choice | Reasoning |
| --- | --- | --- |
| Multiplayer timing | Single-player first, determinism paid up front | Retrofitting determinism onto a float simulation is a rewrite of every system. Paying roughly 15% extra now buys the option later, and yields replays for free. |
| Finish line | Playable loop, no AI opponent | Smallest slice that exercises economy, combat, and win condition together. |
| Visuals | Debug shapes, no art | Art pipeline built before the simulation is proven is wasted if the simulation design changes. |
| Framework | macroquad | Minimal ceremony for drawing quads. Sits behind a trait so a Bevy backend can replace it without touching application logic. |
| Second player | Hotseat | Zero AI code, and driving both sides is the fastest way to test combat, replay, and determinism. |
| Scale target | 200 units, 128x128 tiles | Forces the interesting design (flow fields, spatial hashing, tick budgeting) without turning the project into a performance exercise. |
| Testing | TDD on the simulation, plus property tests | Determinism fails silently and late. Property tests over recorded command streams are the only practical defence. |

## Architecture

Three crates in a Cargo workspace. The crate boundaries are the design: the compiler enforces them, so a stray import cannot quietly break determinism.

```
aoe/
  Cargo.toml              # workspace
  crates/
    sim/                  # pure state machine, no floats, no I/O
    render/               # Renderer and InputSource traits, macroquad backend
    app/                  # binary: main loop, command mapping, wiring
```

Dependency rules:

- `sim` depends on `serde` and `slotmap` only. It cannot see `render` or `app`.
- `render` depends on its backend framework only. It cannot see `sim`. It draws primitives, not game entities.
- `app` depends on both and is the only place they meet.

Because `render` never sees `sim`, translating game state into draw calls happens in `app`. This keeps `render` a dumb drawing surface, which makes it both swappable and testable with a recording fake.

### `sim` public interface

```rust
pub struct World { /* private */ }

impl World {
    pub fn new(seed: u64, map: MapSpec) -> World;
    pub fn step(&mut self, cmds: &[Command]);   // exactly one tick
    pub fn checksum(&self) -> u64;
    pub fn view(&self) -> WorldView<'_>;        // read-only accessors
}
```

`step` cannot fail and returns nothing. Invalid commands are dropped, not reported as errors.

### `render` public interface

```rust
pub trait Renderer {
    fn begin_frame(&mut self, cam: &Camera);
    fn quad(&mut self, pos: Vec2, size: Vec2, color: Color);
    fn line(&mut self, a: Vec2, b: Vec2, color: Color);
    fn text(&mut self, s: &str, pos: Vec2, px: f32, color: Color);
    fn end_frame(&mut self);
}

pub trait InputSource {
    fn poll(&mut self) -> InputState;   // framework-neutral mouse and key state
}
```

Floating point is fine in `render` and `app`; it is presentation only. The macroquad implementation of both traits lives in `render::macroquad_backend`, behind a default-on `backend-macroquad` feature. A Bevy backend later is a new module and feature; `app` is untouched.

### `app` responsibilities

Fixed-timestep loop, translation of `InputState` into `Command` values, camera, selection state, hotseat side switching, replay recording and playback, and translation of `WorldView` into draw calls.

## Simulation core

### Fixed-point arithmetic

16.16 fixed point in `i32`. One tile equals `Fx::ONE`. Range is roughly plus or minus 32768 tiles at a resolution of 1/65536 tile, which is ample for a 128x128 map.

```rust
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Fx(i32);

impl Fx {
    pub const ONE: Fx = Fx(1 << 16);
    pub fn mul(self, o: Fx) -> Fx { Fx(((self.0 as i64 * o.0 as i64) >> 16) as i32) }
    pub fn div(self, o: Fx) -> Fx { Fx((((self.0 as i64) << 16) / o.0 as i64) as i32) }
    pub fn sqrt(self) -> Fx;   // integer Newton-Raphson, hand-rolled
}
```

`sim/src/lib.rs` carries `#![deny(clippy::float_arithmetic)]`, and no `f32` or `f64` appears in any public type. Distance comparisons use squared distance in `i64`, so `sqrt` stays out of hot paths entirely.

### Randomness

A hand-rolled seeded `xoshiro256**` generator stored inside `World`. Every draw advances world state and therefore participates in the checksum. The `rand` crate is not used in `sim`, and no thread-local state exists.

### Banned in `sim`

These are the determinism hazards, listed so they can be checked in review:

- `f32` and `f64` (enforced by clippy lint)
- `HashMap` and `HashSet` iteration; use `Vec` or `BTreeMap`
- `Instant` and `SystemTime`; simulation time is `tick: u64` and nothing else
- threads and `rayon`; the tick is single-threaded
- any ordering derived from pointer addresses

### State layout

```rust
pub struct World {
    tick: u64,
    rng: Rng,
    terrain: Grid<Tile>,           // 128 * 128
    units: SlotMap<UnitId, Unit>,
    buildings: SlotMap<BuildingId, Building>,
    mines: SlotMap<MineId, Mine>,
    players: [Player; 2],
    spatial: SpatialHash,          // rebuilt each tick
    flow_fields: FlowFieldCache,   // keyed by goal tile, LRU
    path_queue: VecDeque<PathRequest>,
}
```

Entities are referenced by generational IDs, never by Rust references. This sidesteps the cyclic-ownership problem that makes game object graphs painful in Rust, and it keeps the state trivially serializable.

A gold mine is an entity rather than a terrain tile so that it can hold depletable state and be targeted by a command. It occupies blocked tiles in the grid, but its remaining gold lives in `mines`. Commands that can target more than one kind of entity use a tagged identifier:

```rust
pub enum EntityId {
    Unit(UnitId),
    Building(BuildingId),
    Mine(MineId),
}
```

### Tick order

20 Hz. The order is part of the contract: changing it invalidates existing replays.

1. Apply commands
2. Production and training queues
3. Gathering
4. Pathfinding budget (drain N requests from `path_queue`)
5. Movement and local avoidance
6. Combat
7. Death and cleanup
8. Win check
9. Increment tick

### Checksum

FNV-1a over unit positions, unit HP, player resources, and tick. Computed on demand rather than every tick.

## Pathfinding and movement

Three layers, each solving a distinct problem. Conflating them is the standard failure mode in RTS movement code.

### Layer 1: static grid

`Grid<Tile>` carries a `passable` flag per tile. Buildings stamp tiles blocked when placed and unstamp on death. Units never write to the grid.

### Layer 2: flow field for group destinations

One order issued to N selected units produces one flow field, not N paths.

```rust
pub struct FlowField {
    goal: TilePos,
    dir: Vec<Dir8>,   // 128 * 128, one byte each, 16 KB
    cost: Vec<u16>,   // integral Dijkstra distance
}
```

Built by Dijkstra expansion outward from the goal over passable tiles, roughly 16k tile visits, budgeted across ticks if it proves too expensive in one. Cached in an LRU of capacity 8 keyed by goal tile, so repeated orders to the same area cost nothing. Fields whose region a newly placed building touches are invalidated.

The reason for flow fields over per-unit A*: 200 units each running A* on every order is precisely what makes naive RTS movement unusable. A flow field is O(map) once, independent of unit count.

### Layer 3: local avoidance

The flow field supplies a desired direction. Neighbours supply a separation force. Steering is the sum of the two in fixed point, clamped to unit speed.

Neighbour lookup uses a spatial hash rebuilt each tick:

```rust
pub struct SpatialHash {
    cell_size: i32,                      // 2 tiles
    cells: Vec<SmallVec<[UnitId; 8]>>,   // flat 64 * 64 grid, cleared not reallocated
}
```

A query inspects 9 cells. Nothing in the movement path is O(n^2).

### Single-unit fallback

A lone unit uses grid A* rather than building a 16 KB field. Requests enter `path_queue` and are drained at a fixed budget, starting at 16 per tick, so a mass order cannot spike a single tick. The budget is a tuning constant to be measured at milestone 4.

### Stuck handling

A unit with zero net movement for 30 ticks that has not reached its goal abandons its order and idles. This is crude but prevents the vibrating-blob failure mode. Better handling belongs to a later spec.

## Game content

Deliberately minimal, and data-driven from RON files so that later content additions are data rather than code.

Single resource: gold.

| Entity | Role | Stats |
| --- | --- | --- |
| Villager | Gathers gold | 40 HP, no attack |
| Soldier | Fights | 60 HP, 8 damage, 1.5 tile range, 1 attack per second |
| Town Center | Drop-off point, trains Villager for 50 gold | 1000 HP, 4x4 tiles |
| Barracks | Trains Soldier for 60 gold | 600 HP, 3x3 tiles |
| Gold Mine | Depletable entity, 5000 gold, unowned, blocks its tiles | 2x2 tiles |

Each player starts with one Town Center, one Barracks, three Villagers, and 200 gold. Buildings are placed by the map, not constructed in game. This removes build-site validation, construction progress, and builder assignment from the slice entirely.

### Gather loop

A villager ordered onto a mine walks to it, mines up to a carry capacity of 10 gold, walks to the nearest owned drop-off building, deposits, and returns. This is a state machine on the unit; the player issues one order and does not micromanage further.

The gather rate is one gold every 40 ticks, or 0.5 gold per second at 20 Hz. It is expressed in whole gold on a tick counter rather than as a fractional per-tick rate, so that gold is always an integer and the conservation property test has nothing to round.

### Commands

The command enum is simultaneously the input abstraction, the replay format, and the future wire format.

```rust
pub enum Command {
    Move   { player: PlayerId, units: SmallVec<[UnitId; 32]>, target: TilePos },
    Attack { player: PlayerId, units: SmallVec<[UnitId; 32]>, target: EntityId },
    Gather { player: PlayerId, units: SmallVec<[UnitId; 32]>, mine: MineId },
    Train  { player: PlayerId, building: BuildingId, unit: UnitKind },
    Stop   { player: PlayerId, units: SmallVec<[UnitId; 32]> },
}
```

`step` validates every command against the issuing player. Wrong owner, dead or missing entity, insufficient gold, and out-of-bounds targets are all dropped silently. The simulation never trusts its caller. This is the same check that will make the code safe when commands arrive from the network.

### Combat

Direct damage at range against a specified target. No projectiles, no armour types, no damage bonuses. Dead units are removed during the cleanup phase.

### Win condition

A player loses when all of their buildings are destroyed. The simulation sets `outcome: Option<PlayerId>` once and continues ticking; presentation of the result is the application's concern.

### Hotseat

`app` holds an `active_player` and swaps it on Tab. All commands are stamped with the active player. Camera and selection are per-player state held in `app` and never enter `sim`.

## Application loop

Fixed timestep with an accumulator, decoupled from render rate.

```rust
const TICK: Duration = Duration::from_millis(50);   // 20 Hz

loop {
    let input = input_source.poll();
    pending_cmds.extend(map_input_to_commands(&input, &ui_state, world.view()));

    accumulator += frame_dt;
    while accumulator >= TICK {
        prev_snapshot = world.view().snapshot();   // positions only
        world.step(&pending_cmds);
        recorder.record(world.tick(), &pending_cmds);
        pending_cmds.clear();
        accumulator -= TICK;
    }

    let alpha = accumulator.as_secs_f32() / TICK.as_secs_f32();
    draw(&mut renderer, &prev_snapshot, world.view(), alpha);
}
```

Rendering interpolates unit positions between the previous snapshot and the current state, so a 20 Hz simulation appears smooth at high frame rates. Interpolation uses `f32` and lives entirely in `app`.

Catch-up is capped at 5 ticks per frame. Under sustained load the simulation falls behind wall-clock time rather than freezing in a spiral of death.

## Replay

Replay is nearly free given the command-driven, deterministic design.

```rust
pub struct Replay {
    seed: u64,
    map: MapSpec,
    commands: Vec<(u64, Vec<Command>)>,   // tick to commands, sparse
}
```

The binary accepts `--record out.replay` and `--play in.replay`. Playback feeds recorded commands in place of live input and asserts that `world.checksum()` matches the checksums recorded every 100 ticks. This is the primary determinism regression test, and it doubles as a mechanism for producing reproducible bug reports.

## Error handling

Three classes, handled differently on purpose.

**Invalid commands** are not errors. `step` drops them. This is expected traffic, and will be more so once commands arrive from remote players.

**Simulation invariant violations** — negative HP, a unit off the map, a checksum mismatch during replay — are programmer bugs. `debug_assert!` in development. In release, log and continue rather than panicking mid-game. The simulation must never silently correct such a state: a quietly repaired desync is worse than a loud one.

**Application and I/O failures** — an unreadable replay file, malformed RON — propagate as `Result` to `main`, which prints and exits. `sim` itself returns no `Result` from `step` and cannot fail. RON content is loaded and validated once at startup into plain structs; the simulation never touches the filesystem.

## Testing

### Unit tests

The simulation is a pure function of state and commands with no I/O, so every test builds a small world, steps it, and asserts. No mocks and no fixtures are required.

Coverage targets: `Fx` arithmetic including overflow and negative-value edges; `Fx::sqrt` against a known table; flow fields never pointing into blocked tiles; A* returning `None` for unreachable goals and never a path through blocked tiles; a complete gather cycle from mine to deposit; combat killing at the expected tick; rejection of each class of invalid command; the win condition firing exactly once.

### Property tests

Using `proptest`:

1. **Determinism** — a random command stream run twice from the same seed produces identical checksums at every tick. This is the test that protects the whole architecture.
2. **Replay fidelity** — record a random session, play it back, and confirm checksums match.
3. **No tunneling** — for arbitrary maps and orders, no unit ever occupies a blocked tile.
4. **Gold conservation** — gold mined plus gold in transit plus gold banked equals gold removed from mines. Catches resource duplication bugs.
5. **Fixed-point round-trip** — `a.mul(b).div(b)` is within one ULP of `a` for non-zero `b`.

### Renderer tests

A `RecordingRenderer` implementing `Renderer` into a `Vec<DrawCall>`. This is the concrete payoff of the trait: it lets the application's draw logic be asserted headlessly in CI. The macroquad backend itself is verified by eye.

### Benchmarks

Criterion benchmark of `step` with 200 units all moving. Budget is under 5 ms per tick, one quarter of the 50 ms tick, leaving headroom for systems added in later specs. Flow field construction is benchmarked separately.

## Milestones

Each milestone ends in something playable or measurable.

| # | Deliverable | Verification |
| --- | --- | --- |
| 1 | Workspace, `Fx`, RNG, `Grid`, traits | `cargo test`; fixed-point property tests pass |
| 2 | Window, camera pan and zoom, tile rendering | Visual |
| 3 | Units, box selection, movement via A* | Visual plus no-tunneling property test |
| 4 | Flow field, spatial hash, 200 units | Criterion under 5 ms per tick |
| 5 | Gold mine, gather loop, deposit | Gold conservation property test |
| 6 | Training queue and gold cost | Unit tests |
| 7 | Combat, death, win condition | Unit tests |
| 8 | Hotseat swap, replay record and playback | Determinism and replay property tests |

## Risks

**Fixed-point precision.** Accumulated error in movement integration could cause drift over long games. Mitigated by storing positions as absolute fixed-point values rather than accumulating deltas through float-like intermediates, and by the determinism property test which would surface divergence.

**Flow field cache thrashing.** Eight cached fields may be too few if players issue many scattered orders. The cache size is a tuning constant; milestone 4 measures it.

**Tick budget.** If 200 units exceed 5 ms, the first lever is the pathfinding budget per tick, then the spatial hash cell size, then the separation radius. Data layout optimisation is the last resort, not the first.

**Scope creep into deferred systems.** Building construction and AI in particular are tempting during implementation. They are out of scope here and each gets its own spec.
