# Phase 3: Construction and population

**Date:** 2026-10-26
**Status:** Draft
**Plan:** [`2026-10-24-exe-parity-plan.md`](../plans/2026-10-24-exe-parity-plan.md), Phase 3
**Builds on:** [`2026-10-25-phase2-four-resources.md`](2026-10-25-phase2-four-resources.md)

## Goal

Players place buildings, builders construct them over time, and houses raise the population cap.

## What the original does (from reverse engineering)

- Building placement: ghost preview follows mouse, green/red for valid/invalid
- Click places a foundation (0 HP), villagers with build order walk to it and construct
- Construction progresses in stages, each with its own graphic
- Houses ("house" in strings) increase population cap (POPULATION-CAP-25 through POPULATION-CAP-200)
- Training is refused when at population cap
- Footprint sizes come from content (each building type has a size)

## Items from Phase 2 direction check to address

1. **Hand drop-off at walled-in building never finishes** — `order_deposit` plans to a building with no free tile beside it. Automatic drop-off already passes such buildings by. Will become relevant when players place buildings.
2. **`mine_ticks` not reset on new order** — part of a unit's progress carries from one site to the next.
3. **Tick benchmark measures marching, not gathering** — each tree cut clears cached flow fields; that cost not measured.
4. **Mixed selection right-clicked on site** — sends villagers to gather, leaves soldiers without an order.

## Design

### Placement

- New command: `Command::PlaceBuilding { kind, origin }`
- Selection must be a building that trains the unit (villager) — the town centre
- Ghost preview drawn at mouse position, snapped to tile grid
- Validation: all tiles in footprint must be passable and unoccupied; must be within map bounds
- Cost paid on placement (as original), refunded if cancelled before foundation placed
- Replay format bump

### Construction

- Foundation is a new entity type: `Foundation { kind, origin, hp, builder, progress_ticks }`
- HP starts at 0, max HP from building stats
- Villagers assigned to build: right-click foundation with villagers selected issues `Command::Build { target: FoundationId }`
- Build progress: each villager adds HP per tick (`build_rate` from content, converted from .dat)
- Construction stages: foundation → partial → complete (each with own graphic from .dat pieces)
- When HP reaches max, foundation becomes the building
- If all builders die/disappear, construction pauses; can be resumed

### Population

- Each player has `population` and `population_cap` (starts at 25)
- House building type: when completed, adds 25 to cap (from .dat)
- Training a unit checks `population + unit.population_cost <= population_cap`
- Unit cost includes `population` field (villager = 1, soldier = 1)
- Replay format bump

### Content changes

- `BuildingStats` gains: `size` (footprint), `build_time` (ticks), `build_rate` (HP per tick), `population_provided`
- `UnitStats` gains: `population_cost` (default 1 for trainable units)
- Footprint sizes (`BuildingKind::size`) move from code into content
- `datconv` extracts these from .dat

## Steps

1. **Content and datconv**: `BuildingStats.size`, `build_time`, `build_rate`, `population_provided`; `UnitStats.population_cost`. `datconv` writes them. `default.ron` adopts.
2. **Placement command and validation**: `Command::PlaceBuilding`, validation logic, cost payment, ghost preview in UI.
3. **Foundation entity**: new sim entity with HP, progress, builder list. `World::foundations()`, `World::step_foundation()`.
4. **Build order**: `Command::Build`, villager walks to foundation and repairs it each tick. Multiple builders stack.
5. **Construction stages**: foundation drawn as own graphic; at thresholds (e.g. 33%, 66%) shows partial graphics from .dat pieces.
6. **Population cap**: per-player `population` and `population_cap`; house adds 25; training refused at cap.
7. **Phase 2 leftovers**: fix hand drop-off at walled-in building; reset `mine_ticks` on new order; mixed selection handling; update benchmark.
8. **QUICKSTART, screenshot, benchmarks, end-of-phase review + direction check**.

## Gate

1. Invalid placements rejected (off-map, on impassable, on another building, on resource site).
2. Build progress: foundation HP rises per builder per tick; completes at max HP; becomes building.
3. Builder death mid-construction: progress pauses, can resume with new builders.
4. Training refused at population cap; house raises cap; cap respected.
5. All Phase 2 leftovers fixed with tests.
6. Tests and clippy pass; tick benchmark under budget; no game data committed.
7. Replay format version bumped; old version refused.

## Reference

- Building placement: strings `can-build`, `can-build-with-escrow`, `building-available`
- Foundation/construction: `FUN_004a8470` (`build`), `FUN_004a8550` (`build-forward`)
- Population: strings `POPULATION-CAP-25` through `POPULATION-CAP-200`, `population-cap`, `population-headroom`
- House: string `house`
- Wall/gate: `build-wall`, `build-gate` (later phase)
## Step 1 result: content and datconv (redone 2026-10-31)

The first attempt guessed: it took unit 94 (BRSRK, a berserk) for the house,
gave it a placeholder sprite and an icon frame the install lacks (startup
failed), read a "build rate" from the repair rate (0 for the house), wrote
no building cost, and set the house's population to 25 by hand. Redone
from the `.dat`:

- **The house is unit 70 HOUS**: sprite 2223, icon 34, 2x2, 900 HP.
- **Cost** is the building's `creatable.costs`, charged when the foundation
  is placed: house 30 wood, town centre 275 wood, barracks 175 wood, mill
  and camps 100 wood. (The `.dat` charges the town centre no stone.)
- **Build time** is the `creatable.train_time` at 20 ticks a second: the
  time one villager takes. House 500, town centre 2000, barracks 1000, mill
  and camps 700. A building with no `creatable` (a town centre annex) has
  none (`build_time: None`): no one builds it. How more builders speed it up
  is step 4's question.
- **No build rate.** The field is gone from `sim`, the RON, and datconv.
- **Population** is the population headroom the original keeps as resource
  4 in a unit's storages: `population_provided` is the amount held in mode 4
  (once standing): house +5, town centre +5. `population_cost` is the
  negated amount held in mode 2 (while alive): villager and militia 1.
  `datconv::headroom` reads both.
- `default.ron` and `Content::default` hold the converted numbers; the
  real-file gate passes. Validation: a build time of 0, a negative building
  cost, and a negative population provided are refused.
- Replay format 17 (refuses 16): content and commands changed shape.
- The `assets::dat` reader is back to reading only what it read before; the
  derived fields it had grown were computed by datconv instead.
- Mutations caught: 11 of 11 (house ID, cost, build time, annex without
  build time, both storage modes, the headroom resource, the three
  validations, the shipped house's population).

## Step 2 result: placing a building (2026-10-31)

- With only your villagers selected (all of them the active player's
  gatherers), the command panel shows building icons (`interfac.drs` 50705)
  in `ui::builds_on_buttons` order: house, barracks, mill, lumber camp,
  mining camp. Drawing and clicking share that one list. Provisional: the
  original first offers economic and military pages; the buildings sit
  directly on the buttons until those pages are read from the original.
- A button or its number key picks the building up
  (`PlayerUi::placing_building`). While placing, a left click on the map
  sends `Command::PlaceBuilding` with the footprint centred on the cursor
  (`pick::placement_origin`, provisional) and keeps the selection; a click on
  the interface places nothing; a right click or `Esc` cancels. `Esc` no
  longer quits the game while placing.
- The preview is the building's own sprite under the cursor, drawn through
  the new `Renderer::sprite_tinted`: see-through (`PLACEABLE`) where it may
  go, red (`UNPLACEABLE`) where it may not. The green or red square is gone.
  Both tints are judged by eye.
- Whether a building may go somewhere is decided once, by
  `WorldView::can_place`, which both the preview and the order use. It
  checks the map edge, passable ground, buildings, foundations, and blocking
  sites. Cost is checked by the order only.
- Foundations are drawn as their building, see-through (`FOUNDATION`).
  Provisional until step 5 draws construction graphics.
- Tests: draw_entities (+7), session (+8), construction (+1), render
  recording (+1). The construction test that claimed a refund but asserted
  none was removed. Mutations caught: 9 of 9 (owner check, panel icons,
  centred origin, red tint, square, can_place, foundation tint, picking up,
  recorded tint). Screenshot: house icon first on the panel; the house
  see-through on open ground, red over the town centre.
- Not done: the `PlaceBuilding` command still names the town centre rather
  than the builders, and no villager walks over to build (step 4).
