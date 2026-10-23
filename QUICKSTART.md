# Quickstart

A playable hotseat real-time strategy slice in Rust: gather gold, train units,
fight, win. Two players share one keyboard. There is no AI opponent and no art —
everything is drawn as coloured boxes on purpose, because the point of this
build is to prove the simulation, not to look good.

This is the proof of concept described in
[`docs/superpowers/specs/2026-08-04-rts-poc-design.md`](docs/superpowers/specs/2026-08-04-rts-poc-design.md).
Read that if you want to know *why* something is the way it is; read this to get
it running in five minutes.

## 1. Prerequisites

- Rust 1.87 or newer. `rustup update` if in doubt.
- On Linux, the X11 and OpenGL development headers that macroquad links against:

  ```sh
  # Debian / Ubuntu
  sudo apt install libx11-dev libxi-dev libgl1-mesa-dev

  # Fedora
  sudo dnf install libX11-devel libXi-devel mesa-libGL-devel
  ```

  macOS and Windows need nothing extra.

## 2. Run it

```sh
cargo run --release
```

The first build takes a couple of minutes; after that it is seconds. `--release`
matters: a debug build simulates fine but the window feels sluggish.

You should see a green field, a blue base in the top-left of the map, and a red
base far to the bottom-right, with a grey wall running between them.

## 3. Controls

| Input | Does |
| --- | --- |
| `W` `A` `S` `D` | Pan the camera |
| `Q` / `E`, or mouse wheel | Zoom out / in |
| Left click a unit | Select it |
| Left drag | Box-select your units |
| Left click empty ground | Clear the selection |
| Right click ground | Move there |
| Right click a gold mine | Send selected villagers to gather |
| Right click an enemy | Attack it |
| `1` | Train a villager at your town centre (50 gold) |
| `2` | Train a soldier at your barracks (60 gold) |
| `Tab` | Hand the keyboard to the other player |
| `Esc` | Quit |

The overlay in the top-left always tells you whose turn it is, how much gold
they have, and how many units they have selected.

Each player has their own camera and their own selection, so `Tab` really is a
handover: your view and your selected units are still there when you `Tab` back.

## 4. Play the first game in three minutes

1. **Gather.** Box-select your three villagers. Right click the yellow 2x2 gold
   mine near your base. They will walk over, mine a load of ten gold, carry it
   back to the town centre, deposit it, and go again — one order, no
   micromanagement. A villager carrying gold shows a small yellow nugget.
2. **Grow.** Once you are above 50 gold, press `1` a few times to queue
   villagers. Gold is charged when you queue, not when the unit pops, so you
   cannot queue five units on one unit's worth of gold.
3. **Build an army.** Press `2` to train soldiers at the barracks (the smaller
   3x3 building next to your town centre).
4. **Switch sides.** Press `Tab` and do the same for the red player.
5. **Fight.** Select some soldiers, right click an enemy unit or building. They
   will walk into range and start hitting it. Health bars appear above anything
   that has taken damage.
6. **Win.** A player loses when all of their buildings are destroyed. The
   overlay announces the winner, and the simulation keeps running afterwards so
   you can watch the mopping up.

The wall across the middle of the map has two gaps in it. Ordering a group of
units to the far side is the fastest way to see the pathfinding work: one order
to twenty units builds one flow field, not twenty paths.

## 5. Record and replay a game

The simulation is deterministic, so a recording is just the seed, the map, and
the commands — nothing about positions is stored.

```sh
# Play, and write everything you did to a file.
cargo run --release -- --record my-game.replay

# Watch it back. Camera controls still work; your clicks do not.
cargo run --release -- --play my-game.replay

# Check a recording without opening a window. Prints the first divergence,
# if any, and exits non-zero.
cargo run --release -- --verify my-game.replay
```

`--seed N` starts a specific game; the default is 24065.

Quit with `Esc` rather than closing the window: the recording is written on the
way out, so killing the process loses it.

If `--verify` ever reports a desync, that is a real bug and the replay file is
the whole bug report. Attach it to the issue.

## 6. Run the tests

```sh
# Everything, headless — no window, no GPU. This is what CI runs.
cargo test --workspace --no-default-features

# The tick budget: 200 units on a 128x128 map, target under 5 ms.
cargo bench -p sim
```

The `--no-default-features` flag turns off the `window` feature, which is what
lets the whole test suite — including the drawing code — run on a machine with
no display. The drawing tests assert against a `RecordingRenderer` that captures
draw calls into a vector instead of putting pixels on a screen.

Current benchmark numbers on a developer laptop, for reference: about 45 µs per
tick with 200 units moving, and about 275 µs to build one flow field. Both are
comfortably inside budget.

## 7. Where the code lives

Three crates, and the boundaries between them are enforced by the compiler:

```
crates/
  sim/      the simulation. No floats, no I/O, no clock, no threads.
  render/   quads, lines, text, camera, input. Does not know what a unit is.
  app/      the binary. The only place sim and render meet.
```

- **`sim`** is a pure state machine: `World::new(seed, map)`, then
  `world.step(&commands)` once per tick. It cannot fail and it never reads the
  filesystem. All arithmetic is 16.16 fixed point (`Fx`) because `f32` results
  differ between machines and would desync a networked game later.
- **`render`** deliberately cannot see `sim`. That is why translating game state
  into draw calls happens in `app/src/draw.rs`, and why the draw logic is
  testable without a window.
- **`app`** owns the fixed-timestep loop, the camera, selection, hotseat
  switching, and replay files.

Good places to start reading, in order: `crates/sim/src/world.rs` (the tick and
its phases), `crates/sim/src/path.rs` (flow fields and A*), then
`crates/app/src/session.rs` (clicks into commands).

## 8. Known limits

These are deferred on purpose, each to its own spec — they are not bugs:

no AI opponent, no fog of war, no building construction in game, no networking,
no sound, two unit types only, no tech tree, no save/load, no map editor.

Unit statistics live in `crates/sim/src/entity.rs` as plain tables rather than
RON data files. They are shaped so that loading them from RON later changes
where the table comes from and nothing else.
