# Pre-stage 2: selecting buildings and the command panel

**Date:** 2026-10-25
**Status:** Approved 2026-10-25, in progress on branch `feat/command-panel` (steps 1 to 3 done)
**Plan:** [`2026-10-24-exe-parity-plan.md`](../plans/2026-10-24-exe-parity-plan.md), inserted before Phase 1.6 step 3b
**Pulls forward:** part of plan Phase 8 (user interface)

## Goal

Train units the way the original does. The user asked for it: "press the
barracks icon and then the soldier icon is under the left panel so that you
can click it and have a progress bar to show what is the training progress
right now... The same logic applies to the town center."

1. Left-click one of your buildings to select it.
2. The bottom-left **command panel** shows a button with the real AoE2 icon
   for each unit type the building trains. Click a button to queue that unit.
3. The bottom-middle **information panel** shows the building's name and hit
   points and, while it trains, a **progress bar** for the unit in training
   and the icons of the units queued behind it.
4. A trained unit appears beside the building (already so).

User decisions (2026-10-25): the original's interface art, read from the
install; with a building selected, the number keys press its buttons by
position, and with nothing selected they do nothing.

## The original's art

Found by decoding every sprite in `interfac.drs` and looking at them:

| What | `interfac.drs` | Notes |
| --- | --- | --- |
| In-game interface | 51101 to 51118 (800x600), 51121 to 51138 (1024x768), 51141 to 51160 (1280x1024) | One per civilisation. Top bar with the wood, food, gold, stone, and population slots; bottom row of command panel, parchment information panel, and minimap frame. The middle is transparent: the map shows through |
| Unit icons | 50730, 134 frames of 36x36 | Frame 15 is the male villager, frame 8 a sword-and-shield infantryman (the militia). Confirmed against the `.dat`'s icon IDs in Phase 1.6 step 4 |
| Building icons | 50705 to 50708, 52 frames of 36x36 each | One set per architecture. In 50705, frame 2 (crossed swords) is the barracks and frame 28 (a man and a woman) the town centre, picked by eye; other frames include the archery range (0), blacksmith (4), castle (7), stable (23), and house (34) |
| Progress bar | 50764, 100 frames of 100x12 | Frame *n* is *n* percent full |

The interface image for the window's size is drawn over the map, scaled to the
window; player one's civilisation image (51101) is used for every player until
civilisations exist (plan Phase 4). The minimap frame stays empty until a
minimap exists.

## Design

- **Content:** each unit and building type gains an `icon`, a frame in its
  icon sprite. This changes `Content`, which every replay embeds, so the
  replay format becomes **version 4**, a recorded replay break.
- **Screen layout** (`app::ui`): the rectangles of the top bar, command panel,
  button grid, information panel, and progress bar, in screen pixels for the
  current window, measured on the 800x600 image and scaled. Pure functions of
  the window size, so they are tested without a window.
- **Drawing in screen space:** the renderer draws in world units, so a screen
  rectangle is turned into world units through the camera, as the overlay
  text already is. No change to `render`.
- **Selection:** the session's selection becomes either units or one building.
  A left click on a building with no unit on top selects it, and its outline
  is drawn like a unit's. A drag box still selects units only.
- **Clicks on the interface** never reach the map: a click inside the top bar
  or the bottom panels does not select, deselect, or order anything there.
- **Buttons:** one per entry in the building's `trains` list, in order, in the
  original's grid of five columns. Clicking one issues the same
  `Command::Train` as before; the simulation's checks (owner, cost, queue
  limit) are unchanged. Buttons show only for your own buildings.
- **Keys:** with a building selected, `1` to `9` press its buttons by position;
  with nothing selected they do nothing. This replaces the global rule from
  Phase 1.5.
- **Information panel:** name, hit points, the progress bar frame for the unit
  in training (elapsed over total training ticks), and small icons of the
  queue.
- **Top bar:** the active player's food, wood, gold, and stone are written in
  their slots. The debug overlay shrinks to one line (tick, zoom, live or
  replay) and moves out of the interface's way.

## Steps

1. **Content `icon` and replay version 4.** Tests: icons load from RON; a
   version-3 replay is refused.
2. **Screen layout and the interface image.** Tests: the layout's rectangles at
   800x600 and scaled to another size; the interface sprite is drawn covering
   the window; the map's clicks are blocked inside the panels.
3. **Selecting buildings.** Tests: clicking your building selects it and
   clears unit selection; clicking ground clears it; a unit standing over a
   building is picked first; the outline is drawn.
4. **Command panel and training.** Tests: a button per trainable type with its
   icon; clicking a button trains at the selected building; the keys press
   buttons by position; with nothing selected, the keys do nothing; no
   buttons for another player's building.
5. **Information panel.** Tests: name and hit points; the progress bar frame
   follows training progress; queued units' icons.
6. **Top bar and overlay.** Tests: resources in their slots; the debug line
   moved.
7. **QUICKSTART** (how to play it) and a screenshot check by eye.

## Gate

1. Clicking the town centre or barracks shows its panel with the original's
   icons; clicking a button or pressing its key trains the unit, the progress
   bar fills, and the unit appears beside the building. Checked by eye on a
   screenshot.
2. Clicks on the interface never act on the map.
3. Tests and clippy pass; no asset committed.

## Step 1 result (2026-10-25)

`UnitStats::icon` and `BuildingStats::icon` (frames in 50730 and 50705), read
from RON and required there. Defaults: villager 15, soldier 8, town centre 28,
barracks 2. Replay format version 4; versions 1 to 3 are refused by version.
Breaking the loader on purpose (ignoring the file's `icon`) fails the test.

## Step 2 result (2026-10-25)

`app::ui::Layout` gives the interface's rectangles for a window size,
measured on 51101 at 800x600 and stretched with the image: top bar
(0, 0, 800, 25); bottom row (0, 455, 800, 145); command panel
(4, 461, 206, 127), with fifteen 36-pixel buttons from (7, 465) every 41
pixels; information panel (219, 467, 280, 128); progress bar slot
(271, 499, 100, 12); minimap frame (507, 461, 290, 136). The image is loaded
as `Sprites::interface` (required) and drawn last over the whole window, the
drag box under it. A press on the top bar or the bottom row neither starts a
drag nor orders anything. Breaking each rule on purpose fails a test.

Seen on the screenshot: the map's last rows lie under the bottom panel at the
camera's southern limit, because the camera keeps the window, not the open
middle, inside the map.

## Step 2b: the camera keeps the open middle on the map (2026-10-25)

Asked for by the user after the step 2 screenshot. `CameraController` now
treats the open middle (between the top bar and the bottom row) as the view:
zooming out stops when the map fills it, scrolling stops when its edge meets
the map's, and `look_at` centres it, not the window, on the point. Beyond the
map shows only under the interface. Sideways nothing changes. Breaking each
rule on purpose fails a test.

## Step 3 result (2026-10-25)

`PlayerUi::building` holds the selected building; while it is set no units
are selected, and the other way round. A left click picks a unit first (your
own only), then a building (anyone's, to look at), else clears both; a box
selects units only and clears the building; a destroyed building falls out.
Buildings are picked on their footprint or their drawn body
(`Bodies::buildings`, from the footprint's centre, built by
`Sprites::bodies(content)` with the same scale as drawing), the front-most
winning where bodies overlap; right-click targeting uses the same test. The
selected building gets the white outline around its sprite. Breaking each
rule on purpose fails a test.
