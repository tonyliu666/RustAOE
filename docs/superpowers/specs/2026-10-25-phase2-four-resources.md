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
4. **The villager's picture** is sprite 1479, its standing graphic in the
   `.dat`, as datconv converts it. (The user first chose to keep 1473; on
   2026-10-26 they chose to follow datconv, which the gate test requires.)

## Design

- **Sites:** `Mine` becomes a resource site owned by Gaia: a kind (gold,
  stone, wood, food), an amount, and a footprint. A site with nothing left
  disappears; a tree frees its tile.
- **Jobs:** content gains, per resource, what a villager gathering it is
  like: its rate (ticks per unit, `ticks_per_unit`), its
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

## Step 1b: a selected unit on the information panel (2026-10-25)

Asked for by the user before step 2. With units selected and no building,
the information panel shows the first unit's icon, name, and hit points, and,
while a gatherer carries a load, `carrying N gold` below them (at 800x600,
baseline (271, 522)). Only gold can be carried until step 2, which writes
the load's own resource there. Checked in a screenshot: `villager`, `40/40`,
`carrying 4 gold`.

## Step 1c: mining and depositing on touch; dropping off by hand (2026-10-25)

Found by the user: a villager mined from a distance. It mined and deposited
within a tile and a half of the target (`World::INTERACT_RANGE`), and, worse,
a villager whose route back to the mine ended on a corner tile stood there for
good, outside that range with its route marked arrived. Now a villager mines
and deposits only when it touches the mine or the drop-off, as a melee attack
needs (edges within `World::CONTACT`); when its route ends short, it walks
the last stretch to the footprint's nearest point. `INTERACT_RANGE` is gone.

Asked for by the user: `Command::Deposit` sends loaded villagers to one of
their owner's drop-offs early. One that was gathering goes back to its mine
afterwards; one that was not (`Order::Deposit`) stands. A right-click on your
own drop-off with loaded villagers selected issues it for those carrying a
load. The automatic walk back when the load is full is unchanged, as in the
original. Replay format 9: the rule and the command set changed (the version
Phase 2 was to move to anyway).

## Step 2 result (2026-10-25)

`Mine` is now `Site` (and `MineId` `SiteId`, `EntityId::Site`,
`view.sites()`): a one-tile site of any resource, as the original's mines,
bushes, and trees are; `MapSpec::with_site(resource, at, amount)`, with
`with_mine` kept for gold. A gold mine starts with the original's 800. A
villager records the resource it carries (`Unit::carried`) and banks into
that stock; sent to a site of another resource it drops its load, as in the
original. Content's `gather` is one `(carry_capacity, ticks_per_unit)` per
resource (`sim::Gathering`; RON `gather: (food: (...), wood: (...), ...)`),
all ten at one per two seconds until step 5; `datconv` writes each from its
job unit (forager 120, lumberjack 123, gold miner 579, stone miner 124).
`view.total(resource)` replaces `total_gold`, and conservation is tested for
each resource. The information panel names the load's resource. The
checksum hashes each site's and load's resource. Replay format 10.

Every site is still drawn as a gold mine; step 4, which puts the other kinds
on the map, gives each its sprite.

## Step 3 result (2026-10-25)

A building's `drop_off` is the list of resources it takes
(`BuildingStats::takes`; RON `drop_off: [food, wood, gold, stone]`). The town
centre takes all four, the barracks none. A villager walks back to the
nearest of its owner's buildings that takes its load, a hand drop-off is
refused where the load is not taken, and a right-click on such a building is
an ordinary move. A load nothing takes stays carried and the villager
stands, which keeps every resource conserved. Replay format 11.

`datconv` takes each building's resources from the job units whose drop
sites list it. On the real file: the mill (68) takes food, the lumber camp
(562) wood, the mining camp (584) gold and stone, the town centre (109) all
four, as the original's. Converting the camps found that a graphic made of
pieces is drawn by its pieces alone, its own SLP only where a piece is -1:
the lumber camp's own SLP (3503) is in no archive and is never drawn. The
converter no longer requires it, and a type's `sprite` is its own SLP when a
piece draws it and its first piece otherwise.

## Step 4 result (2026-10-25)

A site has a size and says whether it blocks: a mine, bush, or tree is one
tile and blocks it (`Site::new`); a farm is three tiles a side and does not,
so a villager works it standing on it and other units walk over it
(`Site::farm`, `MapSpec::with_farm`). A site that is used up disappears and
frees its tile; the flow fields are rebuilt when that changes the ground.
The default content has the mill, lumber camp, and mining camp (size 2, 1000
hit points, as the original's), and the map needs them.

Each base on the built-in map is laid out from its town centre: a barracks,
a mill with six forage bushes and two farms by it, a lumber camp by a
woodline three trees deep, a mining camp by seven gold mines, and five stone
mines. Amounts are the original's: gold 800, stone 350, bush 125, tree 100,
farm 175. The forest across the middle is trees, three rows deep with two
gaps of eight tiles. Replay format 12.

Each site is drawn as the original draws its unit, its graphic's pieces
composed in order: the gold mine (66) 4479 then 2561, the stone mine (102)
4482 then 1034, the forage bush (59) 2560, the forest oak (349) 2296 then
4652, which is now also the tree on rock tiles (it replaces 1270). The farm's
own SLPs (417 to 419) are in no archive of the install: the original draws a
farm as ground, and terrain 15004 is farmland with crops (checked by eye, as
the terrain table is not read yet), so a farm is drawn tile by tile in its
variations, as grass is, over the grass and under everything else. A farm's
crops running down (15005 is a farm used up) is left for later.

Tests: `sim/tests/sites.rs` (5), `sim/tests/map.rs` (6), three drawing tests
in `app/tests/sprites.rs`, three loading tests in `app/tests/install.rs`, one
replay version test. Mutations caught: each site kind drawn as gold, the farm
drawn after the buildings, a farm tile left out, the pieces drawn in reverse,
the oak's pieces swapped, grass loaded for the farm, a missing farm ground
quietly replaced by grass. Workspace 582 passed, clippy clean. Screenshot:
the base's woodline, gold, stone, bushes, farms, and three camps all drawn.

### Trees inside a woodline (user report, 2026-10-25)

The user found villagers would not cut trees. Trees on a woodline's edge
were cut; a tree walled in by others (the middle of the three columns by the
lumber camp, the middle row of the forest) has no walkable tile beside it, so
the villager was given no route and stood forever, waiting to touch it. The
tree pictures overlap heavily, and a click was resolved by tile alone, so
clicks landed on such trees often.

As in the original, a gather order for a site nothing can stand beside goes
to the site of the same resource nearest it that has a walkable tile beside
it (ties to the first in the site table); with none, the villagers stand.
"Can stand beside" is a walkable tile in the ring around the site, not a
proof the villager can walk there: provisional. A click now picks a site by
its drawn picture as well as its tiles, the front one winning where pictures
overlap, and sites are drawn back to front; a farm, flat ground, is picked
by its tiles alone. A picture's body is its whole rectangle, transparent
corners included, so a click on grass just beside a tree can pick it; the
original tests the drawn outline.

Tests: three in `sim/tests/sites.rs`, three in `app/tests/pick.rs`, two in
`app/tests/sprites.rs`. Mutations caught: the retarget skipped, the farthest
site chosen, another resource accepted, the stand order dropped, the
reachability check dropped; bodies ignored, farms given a body, the back one
winning, sites drawn unsorted, a body's scale, the stone mine's picture.
Workspace 590 passed, clippy clean. In the game, player one's villagers sent
to a crown in the woodline's middle column banked 60 wood within 50 seconds.

### A drop-off by hand ends gathering (user report, 2026-10-25)

Step 1c had a villager dropped off by hand go back to the site it was
working. The user pointed out that in the original it stands idle after
the drop-off, and asked for that. `Command::Deposit` now always ends in
`Order::Deposit`: bank on touch, then stand. The automatic return when a
load is full is unchanged. Replay format 13. The drop-off test that checked
a refused hand drop-off now also rejects a `Deposit` order at the wrong
building, which the old check could not see; mutation caught.

## Step 5 result (2026-10-26)

`datconv` writes the gathering table, one set of numbers per resource from
its job unit (forager 120, lumberjack 123, gold miner 579, stone miner 124),
and `default.ron` adopts the converted values:

| Number | Was | Now (British, from the `.dat`) |
| --- | --- | --- |
| Villager | 40 HP, 2.5 tiles/s, 0 damage, 5 s | 25 HP, 0.8 tiles/s, 3 damage every 2 s, 25 s |
| Soldier (militia) | 60 HP, 8 damage, 60 gold, 6 s | 40 HP, 0.9 tiles/s, 4 damage every 2 s, 60 food + 20 gold, 21 s |
| Unit radius | 1/4 tile | 0.2 tile |
| Town centre, barracks | 1000, 600 HP | 2400, 1200 HP |
| Gather, ticks per unit | one rate | food 65, wood 51, gold 53, stone 56 |
| Carry | one capacity | 10 of each |

The rates are the `.dat`'s per-second work rates turned into ticks at 20 Hz
and rounded (food 64.5 to 65, wood 51.3 to 51, gold 52.6 to 53, stone 55.6
to 56); `datconv` reports each rounding. The villager's sprite became 1479,
the one the `.dat` names as its standing graphic. Replay format 14.

The step's report claimed mutation checks for this step; none are recorded.
The gate test below now holds `default.ron` to the conversion instead.

## Step 6 result (2026-10-26)

The information panel showing a selected villager and its load came early,
in step 1b; the drop-off by hand in step 1c. This step added the job
cursors. As first written they were drawn at the wrong place from the wrong
sprite, with no test; the user's report below has the fix.

### Job cursors drawn at the mouse (user report, 2026-10-26)

The user saw that with a villager selected the cursor stayed the arrow over
a tree. Three faults:

- `draw_cursor` took the mouse's world position for a screen position and
  turned it into world units a second time, so the cursor was drawn far
  from the mouse, usually off screen. Its hotspot was ignored, and the
  system's arrow stayed on top.
- 51001 to 51004 are small plain arrows, not job cursors. The exe loads
  its cursors from one sprite, `mcursors` (51000), of 19 frames
  (`FUN_00576a10(..., 51000, 0x13)`). Frame 0 is the arrow, 3 the hand, 4
  the sword, 7 the hammer, and 8 the axe. There is no pickaxe or sickle.
- Nothing tested the cursor, so turning it off failed no test.

Now the game hides the system cursor and draws its own over everything:
the arrow, or the job's cursor when the selection includes a unit that
gathers (by content, not by type index) and the mouse is over a site. It
is drawn at its own pixel size at any zoom, its hotspot from the sprite on
the mouse. Wood is frame 8, the axe. **Provisional:** food, gold, and stone
take frame 3, the hand; which frame the original shows for them is not yet
read from the exe, whose cursor choice goes through calls Ghidra could not
follow.

Tests: `crates/app/tests/cursor.rs` (4) and two in `install.rs`; RED first
(drawn at (-11.4, -6.9), no arrow without a selection; loader asked for
51001). Mutations caught (7): wood not the axe, hotspot dropped at load,
hotspot ignored at draw, size not divided by zoom, any unit counted as a
gatherer, never the job cursor, no cursor drawn. Screenshot: the axe on a
tree and the hand on gold, with a villager selected. Workspace 603 passed.

### Review fixes (2026-10-26)

The end-of-phase review (commit 5160fd1) changed the simulation without
tests or a replay bump. Each change was checked:

- **A load goes past a drop-off no villager can stand beside,** as a gather
  order goes past a walled-in site. Kept, now with a test
  (`a_load_goes_past_a_drop_off_walled_in_to_one_it_can_reach`, RED without
  the rule) and replay format 15 with a refusal test for 14.
  **Provisional:** "can stand beside" is a walkable tile beside the
  building, not a found path.
- **After a drop-off, retargeting if the site had become unreachable.**
  Reverted. Nothing in the game today can wall a site in after a gather
  order reached it (sites only disappear, and no buildings are placed in
  play), so no test could make it run.
- **The cursor picked with a fixed 0.125 tile,** described as
  `World::CONTACT`, which is a simulation distance, not a click size. The
  cursor now picks as a right-click picks, with
  `Session::MIN_PICK_PIXELS / zoom`, so the cursor shows what the click
  would order. Test: over a grid round a tree with a villager in front of
  it, the axe shows exactly where a right-click picks the tree (RED with
  0.125).

Gate fixes:

- **Gate 1.** Only gold had a conservation test. Added
  `each_resource_is_conserved_and_banked_in_full_once_its_site_is_used_up`:
  four villagers on a tree, a gold mine, a stone mine, and a farm, every
  total checked every tick until every site is used up and removed.
- **Gate 3.** No test held `default.ron` to the conversion. Added
  `the_shipped_content_plays_by_the_numbers_converted_from_the_original` in
  `datconv`'s real-file tests, which compares the whole rule content. It
  failed at first: datconv's default selection held only the town centre
  and the barracks, while `default.ron` had a mill and two camps written by
  hand. The default selection now converts the mill (68), lumber camp (562),
  and mining camp (584); their numbers were already the original's.

Mutations caught: the drop-off rule removed (sim), the cursor picking at
0.125 and at 0, the mill left out of the conversion, the replay version left
at 14, a gather rate in `default.ron` changed, a load banked as gold, one
unit lost per load. Workspace 607 passed, plus the three real-file tests.

### Direction check fixes (2026-10-26)

An unanchored direction check found four departures, fixed before Phase 2
closes (replay format 16, refusing 15):

1. **Farms were worked at the forager's rate.** datconv took food's numbers
   from the forager (unit 120, 0.31 a second) alone, though the job table
   above lists the farmer (unit 259, 0.53 a second). The gather numbers now
   have a `farm` entry, converted from unit 259 (38 ticks per unit, carry
   10), and a farm, the site that does not block, is worked at it. Test:
   `a_farm_is_worked_at_the_farmers_rate_and_a_bush_at_the_foragers`.
2. **A used-up site left its villagers standing.** In the original a
   lumberjack whose tree falls moves on to the next with what it holds. Now
   when a site is removed (`run_cleanup`, then `go_on_from`), each villager
   working it goes on to the nearest reachable site of the same resource,
   keeping its load; one on its way to a drop-off comes back to that site
   instead. With none, a loaded villager banks and stands, an empty one
   stands. A farmer does not go on, neither to another farm nor to a bush,
   since the original reseeds farms rather than leaving them. A load is no
   longer banked early just because the site ran out. **Provisional:** the
   original searches within some distance of the used-up site; here any
   distance will do, and "reachable" is still a walkable tile beside.
3. **A villager whose site another used up stood holding its load.** It
   now goes on with the others, or banks it.
4. **The job cursor showed over the interface,** where a right-click
   orders nothing on the map. Over the top bar or the bottom panel the
   cursor is the arrow.

Tests (RED first, each for the reason above): the farm rate; going on to the
nearest tree and not a farther one; a miner going on to gold and not a
nearer tree; the stranded villager banking; a villager whose last load used
its tree up coming back to the next; a farmer going on to neither a farm nor
a bush; the cursor over the panel; the replay refusal. Mutations caught
(11): farm at the forager's rate, the farm going on, any resource taken,
the farthest site taken, standing with a load when nothing is near, a
villager on its way to bank not sent back, no going on at all, the cursor
ignoring the interface, the farm converted from the forager, the replay
left at 15, and (earlier) the drop-off rule.

Also brought into line: the plan's hand drop-off now stands, as the user
asked; the plan's food sources say hunt and fish moved to Phases 6 and 5;
decision 4 now follows datconv's sprite, 1479, as the user chose on
2026-10-26; `Session`'s comment on dropping off.

### Left for Phase 3 (from the direction check)

- **A hand drop-off at a walled-in building never finishes.**
  `order_deposit` plans to a building with no free tile beside it and the
  villager keeps its order forever. It cannot happen on today's maps; it
  will once players place buildings, which Phase 3 brings. The automatic
  drop-off already passes such buildings by.
- **`mine_ticks` is not reset on a new order,** so part of a unit's
  progress carries from one site to the next.
- **The tick benchmark measures marching, not gathering.** Each tree cut
  clears every cached flow field; that cost is not measured.
- **A mixed selection right-clicked on a site** sends the villagers to
  gather and leaves the soldiers without an order.

## Step 7 result (2026-10-26)

- **QUICKSTART:** the gather walkthrough has the job cursor, the original's
  rates, farms faster than bushes, and villagers going on from a used-up
  site; the "only gold can be gathered" limit is gone.
- **Screenshot** (release build, a probe in `main.rs`, removed after): the
  axe cursor over the woodline with a villager selected, the information
  panel reading `carrying 7 wood`, and the stock bar rising for food, wood,
  and gold.
- **Benchmarks:** `docs/benchmarks.md`, end of Phase 2: 43.09 µs a step for
  200 units, under budget.
- **Code review and direction check:** both run; their findings are fixed
  above or listed for Phase 3.
- **Gate:**
  1. Per-resource conservation:
     `each_resource_is_conserved_and_banked_in_full_once_its_site_is_used_up`.
  2. No delivery to a building that does not take the load: the tests in
     `crates/sim/tests/drop_off.rs`.
  3. `default.ron` is the conversion:
     `the_shipped_content_plays_by_the_numbers_converted_from_the_original`
     (needs the install; run with `--ignored`).
  4. Workspace 615 passed, the three real-file tests pass, clippy clean, no
     game data committed.

Phase 2 is complete.
