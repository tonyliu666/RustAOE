# Pre-stage: edge scrolling and real sprites

**Date:** 2026-10-24
**Status:** Proposed, awaiting review
**Plan:** [`2026-10-24-exe-parity-plan.md`](../plans/2026-10-24-exe-parity-plan.md), inserted before Phase 1.5 step 4
**Pulls forward:** part of plan Phase 12 (assets)

## Goal

Two presentation changes the user asked for before the rest of Phase 1.5:

1. **Edge scrolling.** Moving the mouse to the edge of the screen pans the map,
   as in the original. The W, A, S, and D keys keep working.
2. **Real sprites.** Terrain, units, and buildings are drawn with the
   original game's graphics instead of squares and circles. The graphics are
   read at runtime from the user's own installer payload; nothing copyrighted
   is committed or downloaded.

Neither change touches `sim`, so neither affects replays or checksums.

## Task A: edge scrolling

- `CameraController::update` pans when the cursor is within
  `EDGE_SCROLL_PIXELS` (8) of a window edge, inside the window, at the same
  speed as the keys. A cursor outside the window does not pan: in a window
  that means the player is using another program. (The first version
  counted outside as an edge; a screenshot showed the map drifting while the
  mouse was over another program, so the rule was reversed.)
- Edges do nothing until the backend has reported a real cursor position
  (`InputState::cursor_seen`). macroquad reports (0, 0), a corner, until the
  mouse first moves over the window.
- Pan speed is 48 tiles per second, for keys and edges alike (was 24; the
  user found 24 slow).
- At an edge, the pan direction runs from the screen centre to the cursor,
  so the upper part of the left edge pans left and up and a corner pans
  diagonally (user request: "when the mouse goes left-up the map should also
  go left-up"). A window too small to have a middle does not pan.
- Keys and edges combine into one direction, normalised, so a corner or a key
  plus an edge never pans faster than one key.
- No pan while the left button is held, so a drag-select that touches the
  edge does not move the map under it.

**Tests** (`crates/app/tests/camera_controller.rs`): cursor at each edge pans
that way; cursor in a corner pans diagonally at normal speed; cursor in the
middle does not pan; cursor outside the window does not pan; no pan before
the cursor has been seen; edge plus the same key
is not faster than the key alone; no pan during a left-button drag.

### Found while testing: the world was drawn upside down

The user reported that up and down scrolled the wrong way while left and
right were fine. A screenshot showed the cause: the world was drawn with y
pointing up, while the camera maths and mouse picking use y pointing down.
macroquad 0.4 already flips y when drawing to the window, and our backend
flipped it a second time. Fixed in `to_mq_camera`, with two tests in
`crates/render/src/macroquad_backend.rs` that push world points through
macroquad's own camera matrix and check they land where
`Camera::world_to_screen` (used for picking) says they should. This also
fixes vertical picking, which was mirrored.

## Task B: real sprites

### Reference note

The original keeps its graphics in DRS archives (`graphics.drs`,
`terrain.drs`, `interfac.drs`, and others in the payload's `data/`). Each
archive holds SLP sprite files, and `interfac.drs` holds the 256-colour
palette. The formats are community-documented. Strings in `aoc.exe` name the
archives; this phase records the function that opens them in the step notes.

### B1: fix the payload extractor

`reverse/tools/extract.py` decodes every folder but never writes the files:
the name-to-stream mapping was cut off. Finish it: parse the 7z
empty-stream property so directories and empty files are skipped, then write
each file under the output directory. **Check:** the extracted `aoc.exe` has
the same SHA-256 as the copy used for the decompile, and the eight `.drs`
files exist.

### B2: DRS archive reader

New crate `crates/assets`, pure parsing with no game logic and no `sim`
dependency, so it can be tested without a window. `Drs::parse(&[u8])` reads
the header, the table list, and the file entries, and returns a file by ID.
Errors are values, never panics: these bytes come from a file on disk.

**Tests:** a small DRS built in the test reads back each file by ID; a
truncated file, a bad header, and an entry pointing past the end are errors.

### B3: palette

Parse the JASC-PAL palette (`50500` in `interfac.drs`) into 256 RGB colours.

**Tests:** a hand-written palette parses; a wrong colour count is an error.

### B4: SLP sprite decoder

Decode an SLP frame into RGBA pixels with the palette: outline table, command
table, and the drawing commands (colour list, skip, fill, player colour,
shadow, and the extended commands). Player colour takes a player number, so a
player 3 villager is drawn in player 3's colour.

**Tests:** synthetic SLP bytes for each command produce the expected pixels;
a command running past the frame is an error.

### B5: sprites in the renderer

`render::Renderer` gains textures: `load_texture(width, height, rgba) ->
TextureId` and `sprite(texture, pos, size)`. `RecordingRenderer` records sprite
calls, so drawing stays testable headlessly. The macroquad backend uploads and
draws textures. `render` still knows nothing about `sim` or game formats.

### B6: drawing with sprites

- `--assets DIR` points the binary at an extracted payload. Without it, the
  game draws shapes as today.
- Terrain: the grass terrain sprite, un-skewed from the original's diamond
  shape into a square tile, because this game's map is a top-down square
  grid.
- Units and buildings: the villager, militia, town centre, and barracks
  sprites, drawn upright, standing on their tile. Which SLP IDs these are is
  found by dumping candidate frames to PNG and looking at them; the chosen IDs
  are recorded in the step notes.
- Selection boxes, health bars, and the overlay stay as they are.

**Tests** (with `RecordingRenderer` and synthetic textures): with assets, a
unit is drawn as one sprite call at its position; without assets, as today;
a missing or broken assets folder is an error at startup, not a crash.

### Decision: top-down, not isometric

The original draws an isometric (diamond) view. This game's map, camera, and
picking are a top-down square grid. This pre-stage keeps top-down: terrain
sprites are un-skewed into squares, and unit and building sprites are drawn
upright. Switching the whole view to isometric changes the camera, picking,
and drawing order, and stays in plan Phase 12.

## Not in this pre-stage

Animation (walking and attack frames beyond one still frame), facing
direction, sound, other terrain types, and the isometric view.

## Gate

1. The map scrolls when the mouse touches a screen edge.
2. With `--assets DIR`, a screenshot shows grass, villagers, militia, a town
   centre, and a barracks drawn with the original sprites. Checked by eye.
3. Without `--assets`, the game runs as before.
4. `cargo test --workspace` and clippy pass. No asset file is committed.
