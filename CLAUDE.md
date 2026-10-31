# CLAUDE.md

A Rust remake of Age of Empires II: The Conquerors, rebuilt rule by rule
against the original. Read this first in a new session, then the plan and
the current phase's spec.

## Where things are

- **Plan:** `docs/superpowers/plans/2026-10-24-exe-parity-plan.md`. Phases,
  gates, and the open questions list.
- **Current spec:** `docs/superpowers/specs/2026-10-26-phase3-construction-population.md`
  (Phase 3: construction and population). Each step has a "Step N result" section.
- **Progress log:** `.superpowers/sdd/2026-10-24-exe-parity-plan/progress.md`.
  Add one line per finished step.
- **Player guide:** `QUICKSTART.md`, covering how to run, the controls, and a
  walkthrough.
- **Benchmarks:** `docs/benchmarks.md`.
- **Reverse engineering:** `reverse/README.md` and `reverse/tools/`. Only
  these are tracked; decompiled output is not.

### Crates

| Crate | What it holds |
| --- | --- |
| `sim` | The deterministic simulation. Fixed-point `Fx`, never floats. |
| `render` | The renderer abstraction, with a `RecordingRenderer` for tests. |
| `app` | The binary: `session`, `draw`, `ui`, `pick`, `sprites`, `looks`, `content_file`, `replay_file`. |
| `assets` | DRS, SLP, and palette readers, plus the `.dat` reader `assets::dat`. |
| `datconv` | A dev tool that converts the `.dat` to content RON. |

### Data, presentation, and the install

- Game data lives in `crates/app/content/default.ron`. The rules go to
  `sim::Content`; the looks (sprite, parts, icon) go to `app::Looks`,
  matched by type name. Replays never carry looks.
- The original install is at `aoe2-install/` (gitignored), and is passed as
  `--assets ./aoe2-install`.
- Run the game with `cargo run --release -p app -- --assets ./aoe2-install`.

## How the user wants work done

- **Announce each step's plan** and wait for the user's go-ahead before
  doing it, unless they say to go straight on.
- **TDD for every rule.**
  1. Write the test and watch it fail (RED) for the right reason.
  2. Make it pass.
  3. Mutation-check it: break the rule on purpose with a temporary script
     edit, confirm a test catches it, and restore the code.
  4. Record the results in the spec and in `progress.md`.
- **After each phase,** run two subagents:
  - a code review;
  - an unanchored direction check against `reverse/` and the plan. Feed it
    no conclusions.
- **After player-facing work,** update `QUICKSTART.md` and tell the user
  exactly how to try it.
- **Screenshots:**
  - Never move the user's mouse (no XWarpPointer).
  - To see something, add a temporary `AOE_PROBE_*` env-var block in
    `crates/app/src/main.rs` (for example, a wider zoom or commands pushed
    into `pending`).
  - Build in release, run, capture with `xwd -name "RTS proof of concept"`,
    and convert to PNG with PIL.
  - Then restore `main.rs` from a backup and check with
    `grep -c PROBE crates/app/src/main.rs`.
- **Commit or push only when asked.**
  - Branch: `feat/rts-poc-milestone-1`.
  - Commit messages are plain English prose and end with the
    `Co-Authored-By` line.
- **Language:**
  - Reply to the user in Traditional Chinese, terse.
  - Docs, code comments, and commit messages are in normal English.
- **Follow the original's behaviour.**
  - Check the `.dat` (through `assets::dat` or `datconv`) and the install's
    archives instead of guessing.
  - When something is judged by eye or is provisional, say so in the spec.
- **Check before every finish:**
  - `cargo test -q --workspace`
  - `cargo clippy -q --workspace --all-targets -- -D warnings`
  - `cargo fmt`

## Rules and gotchas learned

- **Replay format:** `sim::Replay::FORMAT_VERSION` (now 17).
  - Bump it whenever playback or the layout changes.
  - Add a test in `crates/app/tests/replay_file.rs` that refuses the previous
    version.
- **The extracted install is untrusted.**
  - Never run its binaries.
  - Run any Python that reads it with `python3 -I`.
  - Never commit game assets (`aoe2-install/`, `/all.ron`, `/original.ron`).
- **Shell:**
  - zsh does not word-split `$v`; use `${=v}`.
  - `/tmp` gets wiped, so recreate helper scripts there.
  - Never `pkill -f` broadly; it can kill your own shell.
- **Editing:**
  - After `cargo fmt`, re-read a file before any exact-text edit, because
    the layout may have changed.
  - Python replace scripts should assert the old text occurs exactly once.
- **Tests use a fake install,** `crates/app/tests/common/mod.rs`.
  - `graphics()` lists every SLP the default content loads; add new ones
    there.
  - `terrain.drs` holds grass (15001) and farm ground (15004).
  - The fake `sprites()` builds a `Sprites` by hand.
- **Inspecting SLPs:**
  - `cargo run -p assets --example slp_dump -- INTERFAC.DRS ARCHIVE.DRS OUT_DIR PLAYER -- IDS...`
  - Write the output outside the repo.
  - For `.dat` lookups, write a throwaway example under
    `crates/assets/examples/` and delete it afterwards.

### Sim rules

- **Range** is measured edge to edge (`in_reach`); melee is 0.
  `World::CONTACT` = 1/8 tile, provisional.
- **Gathering happens on touch** (`touches`, `nearest_point`).
  - A villager gathers and deposits only when its edge touches the footprint.
  - When a load is full, it returns automatically to the nearest building
    that takes it (`BuildingStats::drop_off`), then goes back.
  - A drop-off by hand (`Command::Deposit`) banks the load, then the
    villager stands idle, as the original does.
  - A load goes past a drop-off no villager can stand beside.
  - Each resource has its own gather numbers, and a farm has the farmer's
    (`Gathering::farm`), all converted by datconv from the job units.
  - When a site is used up (`run_cleanup`, `go_on_from`), its villagers go
    on to the nearest reachable site of the same resource with their
    loads; with none they bank and stand. Farmers do not go on.
- **Sites:** `Site { origin, resource, remaining, size, blocks }`.
  - Mines, bushes, and trees are 1 tile and block it.
  - Farms are 3×3 and don't block.
  - A used-up site is removed.
  - A gather order on a site with no walkable tile beside it goes to the
    nearest reachable site of the same resource; with none, the villagers
    stand.
- **Cursor:** the game hides the system cursor and draws its own, frames of
  `mcursors` (51000): the arrow, the axe (8) over trees with a gatherer
  selected, the hand (3, provisional) over other sites. It picks as a
  right-click does, and is the arrow over the interface.
- **Drawing:**
  - Sites are drawn from the original graphics' pieces: gold [4479, 2561],
    stone [4482, 1034], bush [2560], forest oak [2296, 4652].
  - Farms are drawn as terrain 15004, tile by tile.
  - Sites are drawn back to front and picked by their drawn rectangle; farms
    are picked by their tiles.

### Buildings' numbers (Phase 3 step 1, 2026-10-31)

- **The house is unit 70 HOUS** (sprite 2223, icon 34, 900 HP). Unit 94 is
  a berserk; an earlier guess took it for the house.
- **Cost** is `creatable.costs`, charged when the foundation is placed
  (house 30 wood).
- **`build_time`** is `creatable.train_time` in ticks: one villager's time
  (house 500). `None` for a building no one builds, such as an annex. There
  is no build rate.
- **Population** is resource 4 (population headroom) in a unit's storages:
  mode 4 once standing gives `population_provided` (house and town centre
  +5); mode 2 while alive, negated, gives `population_cost` (villager 1).
  `datconv::headroom` reads it.
- `default.ron` and `Content::default` match the conversion; the
  real-file gate checks it.

### Placing a building (Phase 3 step 2, 2026-10-31)

- **Panel:** with only your villagers selected, the command panel shows
  building icons (`interfac.drs` 50705) from `ui::builds_on_buttons`:
  house, barracks, mill, lumber camp, mining camp. Drawing and clicking use
  that one list. A flat list is provisional; the original opens economic
  and military pages.
- **Placing:** a button or number key sets `PlayerUi::placing_building`.
  A left click on the map sends `Command::PlaceBuilding`, footprint centred
  on the cursor (`pick::placement_origin`), selection kept. A click on the
  interface places nothing. A right click or `Esc` cancels; `Esc` quits the
  game only when nothing is being placed (`main.rs`).
- **Preview:** the building's sprite under the cursor, through
  `Renderer::sprite_tinted`: `PLACEABLE` (see-through) or `UNPLACEABLE`
  (red), both judged by eye. `RecordingRenderer::tinted_sprites()` records
  them apart from `sprites()`.
- **One rule:** `WorldView::can_place` (map edge, passable ground,
  buildings, foundations, blocking sites) is what both the preview and the
  order use. Never check placement a second way in `app`.
- **Foundations** are drawn as their building, tinted `FOUNDATION`, until
  step 5 reads the construction graphics.
- **Mutation scripts:** restoring a file from a copy can leave it older
  than the last build, so cargo runs the stale mutant. `touch` the restored
  sources before trusting the next test run.

## Where we are (2026-10-31)

**Phase 2 (four resources) is complete.** The spec has a result for each
step, the direction check's fixes, and a "Left for Phase 3" list. The gate
test that holds `default.ron` to the conversion needs the install:
`AOE_ASSETS=$PWD/aoe2-install cargo test -p datconv --test real_file -- --ignored`.

**Phase 3** (construction and population) is under way; its spec is
`docs/superpowers/specs/2026-10-26-phase3-construction-population.md`.
Steps 1 (content and datconv) and 2 (placing a building) are done.
**Next: step 3** (the foundation entity) and step 4 (villagers build it).

Known provisional items:
- Annex offsets are not added.
- The train queue limit is not in the `.dat`.
- Button slots.
- The contact tolerance.
- Picking uses whole rectangles, not pixel outlines.
- "Reachable" means a walkable tile beside the site, not a found path.
- Going on from a used-up site has no distance limit.
- The food, gold, and stone job cursors are the hand.
- Farm crops don't run down yet; the used-up farm (15005) is not drawn.
- The terrain table in the `.dat` is skipped, not read.
