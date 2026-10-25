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

You should see grass, the original's Dark Age town centre (three thatched
roofs on wooden posts) and barracks (a stone and palisade yard), a mill,
a lumber camp and a mining camp, three villagers, and around them a woodline,
gold and stone mines, forage bushes, and two farms. The other base is far to
the bottom-right, behind a band of forest.

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
| Right click a tree, a gold or stone mine, a forage bush, or a farm | Send selected villagers to gather it |
| Right click your town centre, mill, lumber camp, or mining camp | Selected villagers carrying a load that building takes drop it off there now, before they are full, then go back to where they were gathering |
| Right click an enemy | Attack it |
| Left click a button on the command panel | With one of your buildings selected, train the unit on that button there (town centre: villager, 50 food; barracks: soldier, 60 gold) |
| `1` to `9` | With one of your buildings selected, press its command panel button by position: `1` is the first button. With nothing selected, they do nothing |
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
there. The map shows through the open middle; the game opens with your base
in the middle of it.

Select a unit and the information panel shows its icon, name, and hit points,
and for a villager with a load, how much it carries (with several units
selected, the first of them). Select a building and the information panel
shows its icon, name, and hit
points. For one of your own, the command panel shows a button for each unit
it trains, and while it trains, the information panel shows the unit in
training with a progress bar filling up, and small icons of the units queued
behind it (up to five in all). An enemy building shows its name and hit points
only. The minimap is still empty.

Units are easy to click at any zoom: zoomed out, a click within a few pixels
of a unit selects it, and a small wobble of the hand still counts as a click.

The top bar shows the wood, food, gold, and stone of whoever holds the
keyboard, each in its slot beside the original's icon (the population slot
stays empty for now). Just below it, one small debug line says whose turn it
is, the tick, the zoom, and whether you are playing live or watching a
replay.

Each player has their own camera and their own selection, so `Tab` really is a
handover: your view and your selected units are still there when you come back.

## 5. Play the first game in three minutes

**What you start with.** Each player has, around their town centre:

- a barracks to its right;
- three villagers standing below it;
- a woodline to its left, with a lumber camp beside it (takes wood);
- seven gold mines below and to the left, with a mining camp (takes gold and
  stone), and five stone mines in a row further below;
- a mill to the lower right (takes food), with six forage bushes to its right
  and two farms below it;
- 200 food and 200 gold, and no wood or stone.

The town centre takes every resource. The amounts are the original's: a gold
mine holds 800, a stone mine 350, a forage bush 125, a farm 175, a tree 100.
A tree or mine that runs out disappears. There are **no soldiers on the map at
the start**: you train them (step 3). Player 1 starts in the top-left of the
map, player 2 in the bottom-right, with a band of forest between them three
trees deep, with two gaps to walk through (or cut your own). The top bar shows
your resources; the line under it says whose turn it is.

1. **Gather.** Box-select your three villagers. Right click a gold mine near
   your base. They walk over, mine a load of ten gold (one gold every 2
   seconds, so about 20 seconds a load), carry it to the nearest building
   that takes gold (the mining camp) on their own, deposit it, and go again:
   one order, no micromanagement. The gold in the top bar goes up when a load
   is deposited, not while it is mined. A villager mines and deposits only
   once it touches the mine or the building, as in the original. Trees,
   stone, bushes, and farms work the same way, each load going to the nearest
   building that takes it: wood to the lumber camp, food to the mill. A
   villager on a farm stands on it to work. Click anywhere on a tree's
   picture, crown included; a tree inside the woodline that nobody can stand
   beside sends the villagers to the nearest tree on its edge, as in the
   original. To bank a part load early, select
   the villager and right click a building that takes it: it drops off what
   it carries and walks back. Left click one villager to watch: the
   information panel shows its
   name, its hit points, and `carrying 4 gold` as the load grows to ten,
   back to nothing once it is deposited.
2. **Grow.** Left click your town centre. The command panel at the bottom
   left shows the villager's icon: click it (or press `1`) to queue a
   villager for 50 food, as in the original. Food is charged when you queue,
   not when the unit appears. Food cannot be gathered yet (plan Phase 2), so
   the 200 food you start with is four villagers.
3. **Build an army.** Left click your barracks; its command panel shows the
   soldier's icon. Click it (or press `1`) to train a soldier for 60 gold.
   The information panel's progress bar fills over about 6 seconds, and then
   the soldier appears next to the barracks. Click again to queue more, up to
   five, while you can pay; the queued ones wait as small icons under the bar.
   The soldier is the original's militia, and its button shows the militia's
   icon. Villagers take 5 seconds.
4. **Switch sides.** Press `Tab` and do the same for the other player. Each
   player keeps their own view, so `Tab` jumps to the other base; `Tab` again
   brings yours back as you left it.
5. **Fight.** Select some soldiers (left-drag a box around them), scroll to the
   enemy by moving the mouse to the screen edge, and right click an enemy unit
   or building. They walk up until they touch it, as the original's militia
   does (its range is measured between edges and is 0 for melee), and start
   hitting it. Health bars appear
   above anything that has taken damage. A town centre takes a while: 1000 hit
   points at 8 damage a second per soldier.
6. **Win.** A player is out when all of their buildings are destroyed; the last
   player left wins. A line under the debug line announces the winner, and the
   simulation keeps running afterwards.

The forest across the middle of the map has two gaps in it. Ordering a group of
units to the far side is the fastest way to see the pathfinding work: one order
to twenty units builds one flow field, not twenty paths.

## 6. Change the numbers

Unit and building types and their numbers come from
`crates/app/content/default.ron`, built into the binary. Copy it, change a
number or add a whole new unit type (with the sprite ID to draw it with), and
run with `--content my.ron`. A broken file is an error message, never a
crash.

### The original's numbers: `datconv`

`datconv` reads `data/empires2_x1_p1.dat` from your install and writes the
same RON format, with the original's numbers. Run it from the repository
root:

```sh
# The four types the game has (villager, soldier, town centre, barracks),
# with the original's numbers, for the British.
cargo run -p datconv -- --install aoe2-install --out original.ron

# Every trainable unit and building of one civilisation: here 6, the Chinese.
cargo run -p datconv -- --install aoe2-install --out all.ron --all --civ 6
```

`--civ` picks the civilisation: 0 Gaia, 1 British (the default), 2 French,
3 Goths, 4 Germans, 5 Japanese, 6 Chinese, 7 Byzantine, 8 Persians,
9 Saracens, 10 Turks, 11 Vikings, 12 Mongols, 13 Celts, 14 Spanish,
15 Aztecs, 16 Mayan, 17 Huns, 18 Koreans. With `--all` every entry is named
after the original's code and unit ID (`sprmn_74` is the militia), and the
file is a reference to read rather than one to play: the game's map needs the
four default names.

The tool prints what it had to round, what it left out and why, and which
graphics the original draws in several pieces. `original.ron` loads with
`--content original.ron`, but expect a different game: its soldier costs
food the game cannot gather yet. The game's own numbers stay in
`default.ron` until plan Phase 2 adopts the converted ones.

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

A replay holds how the game plays (every number in the content), not how it
looks: it is drawn with the sprites and icons of the content you start it
with (`--content`, or the built-in one), matched by type name. Changing a
picture never breaks a recording; a recorded type your content has no picture
for is an error naming it.

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
can be gathered, no tech tree, no save/load, no map editor, no minimap, no
population count. Gathering has no pickaxe cursor yet; it comes with Phase 2. Units show one still frame facing south, buildings are the British
Dark Age ones for every player, and the view is top-down rather than the
original's isometric one.
