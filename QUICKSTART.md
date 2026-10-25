# Quickstart

A playable hotseat real-time strategy game in Rust: gather gold, train units,
fight, win. Up to eight players share one keyboard. There is no AI opponent
yet. Terrain, units, and buildings are drawn with the original Age of Empires
II: The Conquerors sprites, read at runtime from your own copy of the game.
Nothing from the game is in this repository.

This started as the proof of concept described in
[`docs/superpowers/specs/2026-08-04-rts-poc-design.md`](docs/superpowers/specs/2026-08-04-rts-poc-design.md)
and is growing towards the original game along
[`docs/superpowers/plans/2026-10-24-exe-parity-plan.md`](docs/superpowers/plans/2026-10-24-exe-parity-plan.md).
Read those to know *why* something is the way it is; read this to get it
running.

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
- Python 3, to unpack the game's installer.
- Your own Age of Empires II: The Conquerors installer
  (`AgeOfEmpires-TheConquerors.exe`).

## 2. Unpack the game's data

The game reads the original sprites from the installer's `data/` folder. The
installer is a program with a 7z archive attached; `reverse/tools/extract.py`
unpacks it without running anything. Run these from the repository root:
they unpack into `aoe2-install/`, a folder inside the project that git ignores
(see `.gitignore`), so the game's files are never committed.

```sh
mkdir aoe2-install
python3 -I reverse/tools/extract.py path/to/AgeOfEmpires-TheConquerors.exe "$PWD/aoe2-install"
```

If the installer is the one in `sample/`, the path is
`sample/AgeOfEmpires-TheConquerors.exe`. It should end with
`wrote 240 files, 0 empty files, 18 directories; refused 0` and leave
`aoe2-install/data/graphics.drs` among others. Treat the unpacked files as
untrusted: the game only reads its `.drs` archives, and you should not run
anything from that folder.

## 3. Run it

Tell the game where the install is, once, by adding this line to `~/.zshrc`
(or `~/.bashrc`), with the path to your copy of the repository:

```sh
export AOE_ASSETS=/path/to/ageOfAmpiresInRust/aoe2-install
```

Then open a new terminal (or run `source ~/.zshrc`) and, from the repository
root:

```sh
cargo run --release -p app
```

Without the variable, name the install each time instead:

```sh
cargo run --release -p app -- --assets aoe2-install
```

Without an install the game stops with a message saying how to give it one;
there is nothing to draw otherwise.

The first build takes a couple of minutes; after that it is seconds. `--release`
matters: a debug build simulates fine but the window feels sluggish.

You should see grass, a town centre and a barracks with blue trim, three
villagers, and a gold mine. The other base is far to the bottom-right, behind a
band of forest.

## 4. Controls

| Input | Does |
| --- | --- |
| Mouse at a screen edge | Scroll the map that way, diagonally in corners |
| `F10` | Lock the mouse inside the window (and free it again) |
| Mouse wheel | Zoom out / in |
| Left click a unit | Select it |
| Left click a building | Select it: yours or the enemy's, to look at. A white outline shows which |
| Left drag | Box-select your units (a box never selects buildings) |
| Left click empty ground | Clear the selection |
| Right click ground | Move there |
| Right click a gold mine | Send selected villagers to gather |
| Right click an enemy | Attack it |
| `1` to `9` | Train a unit type at one of your buildings: the overlay lists which key trains which (by default `1` villager, 50 gold; `2` soldier, 60 gold) |
| `Tab` | Hand the keyboard to the next player |
| `Esc` | Quit |

The mouse alone moves the view: the screen edge scrolls and the wheel zooms.
No key moves or zooms the camera; the keyboard is for game shortcuts, as in
the original.

The mouse is free to leave the window. Resting it at an edge scrolls the map.
Press `F10` to lock it inside the window, as AoE2's "lock mouse to window"
option does, which makes pushing against an edge scroll reliably.

One quirk while the mouse is free: the window is not told when the mouse
leaves it, so if you leave slowly through an edge the map keeps scrolling
until the mouse comes back. Lock the mouse with `F10` to avoid it.

The view always stays on the map: zooming out stops once the map fills the
open middle of the screen (between the top bar and the bottom panels), and
scrolling stops where the map's edge meets the interface. Scroll all the way
down and the map's last row sits just above the bottom panels, so every tile
can be seen and clicked.

The screen is framed by the original's interface: a top bar with the
resource slots, and along the bottom the command panel (left), the
information panel (middle, parchment), and the minimap frame (right). Clicks
on the interface never reach the map: a left click there keeps your
selection, a right click there orders nothing, and a box cannot be started
there. The panels are still empty; the next steps fill them in. The map
shows through the open middle; the game opens with your base in the middle of
it.

Units are easy to click at any zoom: zoomed out, a click within a few pixels
of a unit selects it, and a small wobble of the hand still counts as a click.

The overlay in the top-left always tells you whose turn it is, their food,
wood, gold, and stone, and how many units they have selected.

Each player has their own camera and their own selection, so `Tab` really is a
handover: your view and your selected units are still there when you come back.

## 5. Play the first game in three minutes

**What you start with.** Each player has a town centre, a barracks (the
smaller building to the town centre's right), three villagers standing below
the town centre, a gold mine to the lower left, and 200 gold. There are **no
soldiers on the map at the start**: you train them (step 3). Player 1 starts in
the top-left of the map, player 2 in the bottom-right, with a band of forest
between them. The overlay at the top shows whose turn it is, their resources,
and which number key trains which unit.

1. **Gather.** Box-select your three villagers. Right click the gold mine near
   your base. They walk over, mine a load of ten gold, carry it back to the
   town centre, deposit it, and go again: one order, no micromanagement.
2. **Grow.** Once you are above 50 gold, press `1` a few times to queue
   villagers. Gold is charged when you queue, not when the unit appears, so you
   cannot queue five units on one unit's worth of gold.
3. **Build an army.** Press `2` to train a soldier at your barracks for 60
   gold. About 6 seconds later it appears next to the barracks. Press `2`
   again to queue more, up to five, while you can pay. (The soldier is drawn
   with the original's two-handed swordsman for now; Phase 1.6 switches it to
   the militia's own sprite.) Villagers take 5 seconds with `1`.
4. **Switch sides.** Press `Tab` and do the same for the other player. Each
   player keeps their own view, so `Tab` jumps to the other base; `Tab` again
   brings yours back as you left it.
5. **Fight.** Select some soldiers (left-drag a box around them), scroll to the
   enemy by moving the mouse to the screen edge, and right click an enemy unit
   or building. They walk into range and start hitting it. Health bars appear
   above anything that has taken damage. A town centre takes a while: 1000 hit
   points at 8 damage a second per soldier.
6. **Win.** A player is out when all of their buildings are destroyed; the last
   player left wins. The overlay announces the winner, and the simulation keeps
   running afterwards.

The forest across the middle of the map has two gaps in it. Ordering a group of
units to the far side is the fastest way to see the pathfinding work: one order
to twenty units builds one flow field, not twenty paths.

## 6. Change the numbers

Unit and building types and their numbers come from
`crates/app/content/default.ron`, built into the binary. Copy it, change a
number or add a whole new unit type (with the sprite ID to draw it with), and
run with `--content my.ron`. A broken file is an error message, never a
crash.

## 7. Record and replay a game

The simulation is deterministic, so a recording is just the seed, the map, the
content, and the commands; nothing about positions is stored.

```sh
# Play, and write everything you did to a file.
cargo run --release -- --record my-game.replay

# Watch it back. Camera controls still work; your clicks do not.
cargo run --release -- --play my-game.replay

# Check a recording without opening a window or needing the game's data.
# Prints the first divergence, if any, and exits non-zero.
cargo run --release -- --verify my-game.replay
```

`--seed N` starts a specific game; the default is 24065.

Quit with `Esc` rather than closing the window: the recording is written on the
way out, so killing the process loses it. A replay from an older build is
refused with a message naming both format versions.

If `--verify` ever reports a desync, that is a real bug and the replay file is
the whole bug report. Attach it to the issue.

## 8. Run the tests

```sh
# Everything, headless: no window, no GPU, no game data. This is what CI runs.
cargo test --workspace --no-default-features

# Also the tests that read the real game files (needs AOE_ASSETS, step 3).
cargo test --workspace -- --include-ignored

# The tick budget: 200 units on a 128x128 map, target under 5 ms.
cargo bench -p sim
```

The drawing tests assert against a `RecordingRenderer` that captures draw calls
instead of putting pixels on a screen, with fake sprites. The asset readers are
tested with archives and sprites built byte by byte in the tests.

Benchmark numbers are recorded in [`docs/benchmarks.md`](docs/benchmarks.md).

## 9. Where the code lives

```
crates/
  sim/      the simulation. No floats, no I/O, no clock, no threads.
  render/   sprites, lines, text, camera, input. Does not know what a unit is.
  assets/   readers for the original's DRS archives, palette, and SLP sprites.
  app/      the binary. The only place sim, render, and assets meet.
```

- **`sim`** is a pure state machine: `World::new(seed, map)`, then
  `world.step(&commands)` once per tick. It cannot fail and it never reads the
  filesystem. All arithmetic is 16.16 fixed point (`Fx`) because `f32` results
  differ between machines and would desync a networked game later.
- **`render`** deliberately cannot see `sim`. That is why translating game state
  into draw calls happens in `app/src/draw.rs`, and why the draw logic is
  testable without a window.
- **`assets`** parses byte slices and nothing else: no files, no game logic.
- **`app`** owns the fixed-timestep loop, the camera, selection, hotseat
  switching, loading the sprites, and replay and content files.

Good places to start reading, in order: `crates/sim/src/world.rs` (the tick and
its phases), `crates/sim/src/path.rs` (flow fields and A*), then
`crates/app/src/session.rs` (clicks into commands).

## 10. Known limits

These are planned, each in its own phase of the plan; they are not bugs:

no AI opponent, no fog of war, no building construction in game, no networking,
no sound, two unit types by default (more can be added in content), only gold
can be gathered, no tech tree, no
save/load, no map editor. Units show one still frame facing south, the town
centre is only its upper section, and the view is top-down rather than the
original's isometric one.
