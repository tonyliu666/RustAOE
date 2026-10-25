# Phase 2: four resources

**Date:** 2026-10-25
**Status:** Approved 2026-10-25, in progress
**Plan:** [`2026-10-24-exe-parity-plan.md`](../plans/2026-10-24-exe-parity-plan.md), Phase 2
**Builds on:** [`2026-10-25-phase1.6-dat-converter.md`](2026-10-25-phase1.6-dat-converter.md) (the `.dat` reader and `datconv`)

## Goal

Food, wood, gold, and stone, each gathered as the original gathers it, from
its own kind of site, carried by a villager doing that job, and dropped off
at a building that takes it. Every number comes from the original's `.dat`
through `datconv`, not from memory. Gathering looks and handles as the
original does: the cursor shows the job, a loaded villager can be sent to
drop off by hand, and the information panel shows what a villager carries.

Players still cannot build (Phase 3), so the map places the sites and the
drop-off buildings.

## What the original does (from its `.dat`, British)

A villager (unit 83) has no gather task of its own; it turns into a job unit
of the same class (4) when it starts gathering. Each job unit carries its own
capacity, work rate (resource per second), and drop sites:

| Job | Unit | Rate per second | Carries | Drops at |
| --- | --- | --- | --- | --- |
| Forager | 120 | 0.31 food | 10 | town centre, mill |
| Hunter | 122 | 0.41 food | 35 | town centre, mill |
| Fisherman | 56 | 0.43 food | 10 | town centre, mill |
| Farmer | 259 | 0.53 food | 10 | town centre, mill |
| Lumberjack | 123 | 0.39 wood | 10 | town centre, lumber camp |
| Stone miner | 124 | 0.36 stone | 10 | town centre, mining camp |
| Gold miner | 579 | 0.38 gold | 10 | town centre, mining camp |

The sites' amounts (a gold mine's gold, a tree's wood) are in each site
unit's resource storage, which the reader does not keep yet (step 1).

## Decisions (the user, 2026-10-25)

1. **Food sources:** forage bushes, and farms placed by the map (building a
   farm waits for Phase 3). Hunting, with animals that move and must be
   killed first, waits for Phase 6; fishing, with water and boats, for
   Phase 5.
2. **Forests:** every forest tile is a tree with the original's wood. It
   blocks movement until it is cut down, then frees its tile.
3. **The villager attacks** as the original's does, with 3 melee damage.
4. **The villager's picture** stays sprite 1473, as the user chose earlier;
   its standing graphic from the `.dat` is 1479.

## Design

- **Sites:** `Mine` becomes a resource site owned by Gaia: a kind (gold,
  stone, wood, food), an amount, and a footprint. A site with nothing left
  disappears; a tree frees its tile.
- **Jobs:** content gains, per resource, what a villager gathering it is
  like: its rate (as ticks per unit, as today's `ticks_per_gold`), its
  capacity, and the original's job unit ID (for the converter and tests). A
  villager carries one resource at a time; sent to a different resource, it
  drops what it carries (as the original does).
- **Drop-off:** a building's `drop_off` becomes the resources it takes: the
  town centre all four, the mill food, the lumber camp wood, the mining
  camp gold and stone. A villager walks to the nearest one that takes its
  load.
- **Map:** each base gains forage bushes, farms, a stone mine, a mill, a
  lumber camp beside the forest, and a mining camp, placed as the original's
  standard start places them; the forest band becomes trees.
- **Numbers:** `default.ron` takes every value `datconv` converts (hit
  points, speeds, ranges, attack, reload and train times, radii, costs,
  sizes, icons, and the gathering table), with the new building types'
  looks. The soldier becomes the militia at 60 food and 20 gold.
- **Interface:** over a site with villagers selected, the cursor turns into
  the original's cursor for that job (interfac.drs cursors, found in step 6);
  a right-click on a building that takes a villager's load sends it to drop
  off and then back to its site; the information panel shows a selected
  villager's load ("7 gold") and, for any selected unit, its name and hit
  points.
- **Replay:** the rules change, so the format moves to version 9.

## Steps

1. **Read the sites.** The reader keeps each unit's resource storages, so a
   site's amount is known; the real-file test checks the gold mine, stone
   mine, a tree, and a forage bush. The job table above is read and checked.
2. **Sites in the simulation.** `Mine` becomes a site of any resource;
   gathering, carrying, and depositing over a resource kind; conservation is
   a property test for each resource.
3. **Drop-off by resource.** Buildings take the resources content says; a
   villager never delivers to a building that does not take its load (the
   plan's gate).
4. **The map.** Bushes, farms, a stone mine, trees in the forest, and the
   three drop-off buildings per base.
5. **The original's numbers.** `datconv` writes the gathering table and the
   new types; `default.ron` adopts the converted values; the replay format
   moves to 9.
6. **Gathering in the interface.** Job cursors, drop-off by hand, the
   carried load and unit details in the information panel.
7. **QUICKSTART**, a screenshot check, and the end-of-phase review and
   direction check.

## Gate

1. Each of the four resources can be gathered and dropped off, and the
   player's stock rises by exactly what was mined (conservation, per
   resource).
2. A villager never delivers a resource to a building that does not take it.
3. The numbers in `default.ron` are the converted ones, checked by a test
   against `datconv`'s output on the real file.
4. Tests and clippy pass; the tick benchmark stays under budget; no game data
   committed.

## Step 1 result (2026-10-25)

`assets::dat::Unit::storages` keeps each unit's resource storages (resource,
amount, mode), empty slots left out. Checked on the real file (Gaia's units):

| Site | Unit | Holds | Footprint (half-size) |
| --- | --- | --- | --- |
| Gold mine | 66 `GOLDM` | 800 gold | 0.5 |
| Stone mine | 102 `STONM` | 350 stone | 0.5 |
| Forage bush | 59 `FORAG` | 125 food | 0.5 |
| Forest trees | 348 to 351 (bamboo, oak, pine, palm) | 100 wood | |
| Single trees | 284, 399 to 405, ... | 125 wood | |

The job table above is checked in the same test. Sites are type 10 units of
their own classes (gold 32, stone 8, bush 7, tree 15); a gold mine is a
single tile, not today's 2x2.

**The farm is not settled:** unit 50 `FARM` stores 15 food, and its depleted
form, unit 357 `FARM_D`, stores 180 of resource 12, not food. How much food
a farm gives is decided by the executable as well as by these numbers; step 4
finds it in the decompile before placing farms, rather than guessing.
