# Plan: Path from the RTS PoC to the Age of Empires II: The Conquerors Target

**Date:** 2026-10-24 (revised the same day after the Phase 1 review and direction check)
**Status:** Phases 0, 1, and 1.5 done; the pre-stage is done. Phase 1.6 steps 1, 2, and 3a done; pre-stage 2 (selecting buildings and the command panel) done, inserted before Phase 1.6 step 3b. Phase 1.6 step 3b next, and Phase 1.6 finishes before more interface work.
**Builds on:** [`docs/superpowers/specs/2026-08-04-rts-poc-design.md`](../specs/2026-08-04-rts-poc-design.md)
**Reference:** `reverse/` (decompiled `aoc.exe`, function index, strings)

## Reading of the goal

The target is the feature set of the shipped game in `sample/AgeOfEmpires-TheConquerors.exe`, reached in phases. Every phase ends in something playable or measurable, the same rule as the PoC spec. Full byte-level compatibility with the original is **not** a goal. Its `.drs`, `.slp`, and `.dat` formats are read only where a phase needs them.

The PoC spec's non-goals still apply until the phase that removes them. Each removal is a new spec before code starts.

## Where we are

- PoC milestones 1–8 are in code on `feat/rts-poc-milestone-1`.
- Phase 0 is done: 44 µs per tick for 200 units against a 5 ms budget (`docs/benchmarks.md`).
- Phase 1 is done: unit, building, and gather numbers come from RON (`docs/superpowers/specs/2026-10-24-phase1-data-driven-content.md`). Commits `c1bc3c2`, `feea851`.
- Game-side reference: 7,509 decompiled functions. World creation is `FUN_0040bdf0` (Gaia, then a player count from the game settings, up to 8; its `< 9` loops set teams and diplomacy). Random-map selection is `FUN_0045ee10`, which only picks a `.rms` script or a real-world `.scx`; the script interpreter it builds is `FUN_00534c40`.

## Revision notes

The direction check after Phase 1 found that four assumptions in `sim` would block later phases, and that the plan left out systems the binary has. Accepted changes:

- New Phase 1.5 generalises the core types before Phase 2 builds on them.
- New Phase 1.6 builds a `.dat` converter (option A: a dev tool that writes our RON from the original data), so phases take numbers from the game's data, not from memory.
- Terrain types and elevation get their own phase before the combat roster.
- Fog of war moves before the AI, so the AI is written against what a player can see.
- Maps centre on the `.rms` script format, with variable map size.
- New phases cover diplomacy and teams, walls and trade, victory conditions, save and load, and game speed.
- Performance checkpoints are added where table sizes or unit counts grow.

Old to new numbering: 2 → 2, 3 → 3, 4 → 4, new 5, 5 → 6, 6 → 7, 8 → 8, 7 → 9, new 10, new 11, 9 → 12, 10 → 13.

## Phases

Each phase has a goal, a deliverable, a verification gate, and the reference to read first. A phase is done when its gate passes. Do not start the next phase while the gate is red.

### Phase 0: Close the PoC (done)

- **Goal:** Confirm the PoC matches its own spec before extending it.
- **Gate result:** 44.35 µs per tick, recorded in `docs/benchmarks.md`. All five spec property tests exist.

### Phase 1: Data-driven content (done)

- **Goal:** Stop hard-coding unit and building stats.
- **Gate result:** see the gate table in the Phase 1 spec. Footprint sizes and mine contents stayed out of scope, as the spec records.

### Phase 1.5: Generalise the core

- **Goal:** Remove the four assumptions that every later phase would otherwise build on: a replay format with no version, one resource, two players, and a fixed list of entity kinds.
- **Spec:** [`2026-10-24-phase1.5-generalise-core.md`](../specs/2026-10-24-phase1.5-generalise-core.md).
- **Work, in order:**
  1. Replay format version.
  2. Resources as a per-player array of food, wood, gold, and stone. Costs become a cost per resource. Only gold can be gathered until Phase 2.
  2b. Widen the checksum to unit and building kinds, owners, orders, and production and gather timers, inside the same replay break.
  3. Up to 8 players plus Gaia, with a per-player defeated flag from which the outcome is derived.
  4. Content keyed by ID, so a new unit or building type is a RON entry, not a code change.
     Each entry also names its sprite (a graphic ID, as the original's `.dat` does), because drawing is required since the pre-stage: a unit type added only in RON must have something to draw. The app's sprite and picking tables become keyed by kind ID.
  - **Replay format:** commits were made partway through this phase, so version 2 exists in two layouts. Step 4 moves to **version 3**, final when the phase ends; version-2 files are refused by version.
- **Reference:** `FUN_0040bdf0` (world creation: player count at `param_2 + 0x294`; teams and diplomacy in its `< 9` loops), `FUN_00555380`/`FUN_00555440`/`FUN_00555510` (per-player attribute table; resources are entries 0 to 3).
- **Gate:** A 4-player map plays to a single winner. A unit type added only in RON can be trained and fights. An old-format replay is refused with a version error that names both versions. The tick benchmark stays under budget.

### Pre-stage: edge scrolling and real sprites (done, inserted before Phase 1.5 step 4)

- **Goal:** Pan the map with the mouse at the screen edge, as in the original, and draw everything with the original sprites read from the user's own installer payload.
- **Spec:** [`2026-10-24-prestage-edge-scroll-and-real-sprites.md`](../specs/2026-10-24-prestage-edge-scroll-and-real-sprites.md), including its follow-up and review-fix sections.
- **Pulls forward** the DRS, SLP, and palette readers from Phase 12. The isometric view stays in Phase 12.
- **Gate (as revised by the user):** only the mouse pans (no W, A, S, D); the mouse is free by default and F10 locks it to the window; the view never shows beyond the map; every map object is a sprite, with health bars the only filled rectangles; the game requires an install (`--assets DIR` or `AOE_ASSETS`) and says how to give it one; clicks select what is drawn. No asset file is committed.
- **Reference:** the original opens its archives with `FUN_00543370` (eight calls from `FUN_0043ae90`), finds a file by walking the open archives in order and taking the first match (`FUN_00543630`, list head `DAT_006b59e4`), and closes them with `FUN_00543580`.

### Pre-stage 2: selecting buildings and the command panel (done, inserted before Phase 1.6 step 3b)

- **Goal:** Train units the way the original does: select a building, press its unit's button on the command panel, watch the progress bar in the information panel.
- **Spec:** [`2026-10-25-prestage2-building-selection-and-command-panel.md`](../specs/2026-10-25-prestage2-building-selection-and-command-panel.md), including its review-fix section.
- **Pulls forward** part of Phase 8: the original's interface image, the command and information panels, resources in the top bar. Content gains each type's `icon`, so the replay format moved to **version 4**.
- **Gate:** clicking the town centre or barracks shows its panel with the original's icons; a button or its number key trains the unit, the progress bar fills, and the unit appears beside the building; clicks on the interface never act on the map.
- **Found on the way:** the soldier's sprite was the two-handed swordsman. Read from the `.dat`'s unit entries, the militia (unit 74) stands with `SPRMN_FN` (SLP 993) and has icon 8; every icon chosen by eye matched (spec, step 4b).

### Phase 1.6: `.dat` converter

- **Goal:** Take game numbers from the original's data instead of from memory.
- **Archive order:** the original searches its open archives in order and takes the first match (`FUN_00543630`). Some sprite IDs appear in more than one archive (`gamedata_x1_p1.drs` duplicates a few in `graphics.drs`), so the game's sprite loader must search an ordered list of archives, by type and ID, rather than one named archive per ID. The evidence says the **expansion archives come first**: `FUN_0043ae90` calls `FUN_00543370` three times, loads `language_x1_p1.dll`, then calls it five more times (`aoc_decompiled.c` around line 33782); `FUN_00543370` appends to the end of the list at `DAT_006b59e4`, and the lookup walks from the head; the strings sit in the same two groups (`gamedata_x1_p1.drs`, `gamedata_x1.drs`, `sounds_x1.drs` beside `language_x1_p1.dll`; `gamedata`, `interfac`, `terrain`, `graphics`, `sounds` together). The file-name arguments were lost in the decompile, so confirm the order from the disassembly's pushes before relying on it.
- **Sprites from the `.dat`:** content's `sprite` field is an SLP ID in `graphics.drs`. The `.dat` gives each unit a graphic, which is an index into its own graphics table; an entry names an SLP and may add extra pieces ("deltas", which is why the town centre is drawn in pieces). The converter resolves a graphic to an SLP ID; drawing the extra pieces is its own step.
- **Type IDs:** our `UnitKind` and `BuildingKind` are positions in our compact lists; the original's unit IDs have gaps and per-civilisation tables. The spec must choose: keep compact IDs and record the original ID beside each entry (for tracing and tests), or index by the original ID.
- **Sprite loading at scale:** today every archive is re-parsed on each lookup and every type is decoded nine times, once per player colour, at startup. Before loading a full table: parse each archive once, search the ordered archive list, and decode only the types a game uses, or decode lazily. Add sprite load time to this phase's performance checkpoint.
- **Fields added by pre-stage 2:** each unit's icon (`IconID`; the step 4b probe found it 47 bytes after the unit ID), its button position on the command panel (the original places each trainable unit at a fixed button, not in list order as the panel does today), and the train queue's limit (today 5; the original's is to be read, not assumed). The probe's offsets are a check for the reader, not a replacement for walking every section.
- **Fractional values:** the original stores resources, gather rates, speeds, and reload times as `float`. The spec for this phase must fix how each converts: amounts to `i32`, rates to ticks per unit, speeds and ranges to `Fx`, times to ticks, with the rounding rule for each.
- **Work:** A dev-only Rust tool, a workspace member so its tests run under `cargo test --workspace`, reads `data/empires2_x1_p1.dat` from a user-supplied install and writes the fields our schema has into RON. Resources map by name, not position (the original orders them food, wood, stone, gold), and float amounts round to integers; see the Phase 1.5 spec. The game itself reads only our RON; it never opens a `.dat`. Track `reverse/README.md` and `reverse/tools/` in git; keep decompiled output and extracted data ignored.
- **Reference:** `FUN_00586b20` names the `.dat` path. The file is zlib-compressed and community-documented.
- **Gate:** The tool reproduces known values (villager 25 HP and 50 food in the original). A test compares the tool's output for a few units against values recorded in the phase spec.
- **Performance checkpoint:** load the full converted table (every unit type the `.dat` holds) and record load time and per-lookup cost in `docs/benchmarks.md`. Lookups must stay a direct index, not a search.

### Phase 2: Four resources

- **Goal:** Food, wood, gold, and stone, each with its own gatherer and drop-off.
- **Work:** Generalise the gather state machine over resource kind, and turn `Mine` into a generic resource site owned by Gaia. Food sources: forage bushes, hunt, fish, and farms placed by the map, since players cannot build until Phase 3. Drop-off buildings (mill, lumber camp, mining camp) are placed by the map for the same reason. Conservation becomes a per-resource property test.
- **Every number matches the original,** not only costs: hit points, speeds, ranges, reload and train times, carry capacity and gather rates, and each unit's standing graphic (the villager is drawn in its attack pose until then). A villager costs 50 food, not gold; the soldier is the militia at 60 food and 20 gold. Values come from the Phase 1.6 converter. This is a recorded replay break: bump the replay format version.
- **Gathering as the original plays it** (asked for by the user during pre-stage 2): a pickaxe cursor over a resource with villagers selected; a right-click on a drop-off building sends loaded villagers to deposit early and then back to work; the information panel shows a selected villager's load.
- **Reference:** Gather loop in `reverse/aoc/aoc_decompiled.c`. Search for the resource strings in `reverse/aoc/strings.txt` first.
- **Gate:** Conservation property test passes for all four resources. A villager never delivers a resource to a building that does not accept it.

### Phase 3: Construction and population

- **Goal:** Players place buildings, builders construct them over time, and houses raise the population cap.
- **Work:** Placement validation, construction progress, builder assignment. Footprint sizes move into content here. Population cap and houses ("POPULATION-CAP-25" to "200" in the strings).
- **Gate:** Unit tests for invalid placements, build progress, builder death mid-construction, and training refused at the population cap.

### Phase 4: Ages, research, and civilizations

- **Goal:** Dark, Feudal, Castle, and Imperial ages, research queues, and civilization bonuses.
- **Work:** A per-player modifier layer over the shared content table, since a technology or a civilization bonus changes stats for one player only ("aqquire %d of attribute %d", "TechInfo" in the strings). Tech tree and civilizations come from the converter.
- **Gate:** A researched technology changes one player's stats and not another's. The replay format version is bumped, or the change is shown not to alter old replays.

### Phase 5: Terrain types and elevation

- **Goal:** Terrain beyond "passable or not": water, shallows, forest, and height.
- **Work:** Terrain type per tile, passability per unit class (land, water), and elevation. Naval units, docks, and the hill bonus depend on this.
- **Reference:** "Random Map elevation initilization" and `DATA\blendomatic.dat` (blending is a Phase 12 asset concern).
- **Gate:** Land units never stand on water and ships never stand on land, as a property test.

### Phase 6: Unit roster and combat

- **Goal:** Archers, cavalry, monks, siege, and ships. The first phase that changes the combat model.
- **Work:** Damage types and armour, replacing the PoC spec's rule against them. Projectiles if the archer design needs them. Stances, formations, and garrison.
- **Gate:** Damage unit tests per type. **Performance checkpoint:** the tick benchmark runs at the original's scale, 8 players at 200 population plus Gaia, and the budget is re-set from that measurement.

### Phase 7: Maps and scenarios

- **Goal:** Generate maps the way the original does.
- **Work:** Interpret the `.rms` random-map script format, with variable map size. Load `.scx` scenarios, which the real-world maps use.
- **Reference:** `FUN_0045ee10` (seeds and picks the script), `FUN_00534c40` (script interpreter), `FUN_0040df00` (scenario load). "ERROR: Syntax error. no matching start_random command" shows the script grammar.
- **Gate:** Map generation is deterministic for a seed. Every generated map passes the no-tunnelling property test. At least one shipped `.rms` script generates a playable map.

### Phase 8: Fog of war and UI

- **Goal:** Players see only what their units see. Proper selection, hotkeys, and a resource bar.
- **Work:** Visibility as sim state, so it stays in the checksum. The UI reads only the view.
- **Gate:** A property test that no hidden enemy unit appears in a player's view.

### Phase 9: AI opponent

- **Goal:** A scripted opponent, so one player can play alone.
- **Work:** A rule-based build order that sees only its own player's visible view. AI issues ordinary `Command` values, so it goes through the same validation as a player. The original's AI uses `.ai` script files; reading them is optional.
- **Gate:** The AI wins or loses a fixed-seed match deterministically. The sim has no AI-only code path.

### Phase 10: Diplomacy, walls, and trade

- **Goal:** Teams and stances, walls and gates, markets, trade, and tribute.
- **Reference:** "Diplomacy", "TEAMS-LOCKED", "LandTrade", "BoatTrade", `tribute.wav`.
- **Gate:** Allied units never attack each other. Tribute conserves resources.

### Phase 11: Victory conditions, save and load, and game speed

- **Goal:** Conquest, relics, wonder, regicide, and king of the hill; mid-game save and load; game speed.
- **Reference:** "VICTORY-CONQUEST", "REGICIDE", "KING-OF-THE-HILL", "AUTOSAVE", "Game Speed".
- **Gate:** Each victory condition fires exactly once. A saved game reloads to the same checksum.

### Phase 12: Assets

- **Goal:** Real graphics and sound. This phase is out of the PoC architecture's scope and needs its own spec.
- **Work:** Read `.drs` and `.slp` for sprites and terrain. Read `.wav` and `.mp3` for sound. Keep all asset loading in `app` and `render`.
- **Gate:** A rendered screenshot matches a reference frame, checked by eye. Assets are not committed; they are read from a user-supplied install directory.

### Phase 13: Multiplayer

- **Goal:** Lockstep networking over the existing command stream.
- **Work:** Transport, input delay, and desync reporting, in a new spec. Peers exchange a hash of their content and replay format version in the handshake. Widen the checksum to cover orders, gather timers, and production progress before this phase, so desyncs show up early.
- **Reference:** "Starting Turn#%d", "Too many commands in one comm turn", "IsPlayerOutOfSync".
- **Gate:** Two processes stay in sync for a 10-minute recorded match.

## Controls

Decided by the user on 2026-10-25, following the original:

- **The mouse alone moves units and the view.** The screen edge scrolls, the
  wheel zooms, left click selects, right click orders. No key pans, zooms,
  or moves a unit (`camera_controller.rs::no_key_moves_or_zooms_the_camera`
  holds this for every key).
- **The keyboard is for game shortcuts.** Today: `1` to `9` train, `Tab` hands
  over in hotseat, `F10` locks the mouse, `Esc` quits. Later phases add the
  original's shortcuts as their features arrive: the villager's build menu
  (houses, castle, and so on) in Phase 3, stances and formations in Phase 6,
  and the full hotkey scheme with the user interface in Phase 8.

## Guidelines

These apply to every phase.

1. **Read the reference first.** Search `reverse/aoc/aoc_decompiled.c` and `reverse/aoc/strings.txt` before designing anything. Write the finding into the phase's spec as a short note with the function address.
2. **Write a spec before code for every phase.**
3. **Test first in `sim`.** Write the failing test, watch it fail for the right reason, then write the code. `sim` is pure, so each test builds a small world and steps it.
4. **Keep the boundaries.** `sim` has no floats and no I/O. `render` draws primitives only. `app` is the only crate that sees both. Enforce this with `cargo clippy` and the existing float lint.
5. **Keep replays honest.** Any change that alters tick order or the checksum is a replay break. So is any change to the shape of `Content` or `MapSpec`, because a replay embeds both. Record it in the phase notes and bump the replay format version. A version is final only when its phase ends: do not keep replays or commit partway through a phase that changes the format.
6. **Do not commit game assets.** Read them from a user-supplied install. The repo holds code, specs, and short reference notes only.
7. **Verify before claiming done.** Run `cargo test --workspace` and the phase's gate. Report the output, not a summary of it.
8. **Narrate code work.** Before each step, say which files change, which tests are written, and why, and wait for the go-ahead. After, say what changed and which test shows it works.
9. **Commit only when asked.** Use the attribution line from the session instructions.
10. **Check the direction after every phase.** When a phase's gate passes, run two independent subagent checks before the next phase starts:
    - **Code review** of the phase's diff against its spec, for bugs, determinism risks, and weak tests.
    - **Direction check.** A fresh subagent gets only the repository path and one question: compared with the reverse-engineering output in `reverse/`, is the project's current state on the right path toward this plan, and if not, what is missing? Do not give it your own conclusions or a summary of the work, so that its answer is not anchored on yours.

    Report both results to the user. Fold any accepted findings into this plan before the next phase.

## Deferred findings

From the pre-stage review, accepted as minor:

- One draw call per grass tile when zoomed out (textures are not batched); measure at `MIN_ZOOM` and try `build_textures_atlas`.
- A `HashSet` and per-building `Vec`s are allocated every frame to keep trees off footprints.
- Trees, buildings, and units are not depth-sorted together; a unit just north of a forest edge draws over trees in front of it.
- No test fixes the draw order (terrain before entities).
- `Archive::get` re-parses the whole archive on every call at startup.
- Install errors show only after the window has opened.
- Test scratch directories under `temp_dir()` use fixed names in some tests.
- The `ground()` test helper treats any 1x1 sprite as ground.

From the pre-stage direction check:

- The "no buildings" defeat rule is provisional; research the original's conquest rule before Phases 10 and 11 build on it.
- The top-down view means picking, scrolling, and drawing will need rework for the isometric view in Phase 12.

From the pre-stage 2 review and direction check, recorded rather than fixed:

- The interface is always the 800x600 image (51101), stretched to the window. The original picks the 1024x768 or 1280x1024 image by resolution and shows more map instead of scaling. Pick by resolution with Phase 8.
- Only your own units can be selected; the original also lets you select another player's units to look at them. Phase 8.
- Wheel zoom is not in the original (its view has a fixed scale); it is kept, and it works over the interface too.
- Content files written before pre-stage 2 no longer load: every type needs an `icon`. The error names the missing field.
- The information panel shows content names (`town_center`); the original's names come from its language DLL. Phase 8 or Phase 12.
- Each pre-stage adds features on the top-down projection (the camera's open middle, picking by drawn body, screen-space panels), so the Phase 12 isometric rework grows. Decide whether to pull the isometric projection forward before Phase 3 adds placement.
- Presentation-only fields (`sprite`, `icon`) sit in the content every replay embeds, so each new one is a replay break. Consider splitting them out before more are added.
- Buildings and units are now each drawn back to front, but trees, buildings, and units are still not sorted together.

From the Phase 1 review, accepted as minor and not yet fixed:

- `radius` has no upper bound; above about 1/2 tile a unit cannot pass a one-tile gap. Fix with Phase 3 or Phase 6.
- A bad `--content` file blocks `--play`, which ignores it. Fix with Phase 1.5's replay work.
- The old-format replay test proves parsing only. Phase 1.5's version check replaces it.
- Content is not in the checksum. Phase 13 handshake.

## Open questions

- ~~Is the Phase 12 asset read in scope for this repository?~~ Yes: decided by the pre-stage. The game reads the original's sprites from the user's own install at runtime and requires one; nothing from the game is committed.
- Is the scenario editor in scope at all?
