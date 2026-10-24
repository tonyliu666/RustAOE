# Plan: Path from the RTS PoC to the Age of Empires II: The Conquerors Target

**Date:** 2026-10-24
**Status:** Proposed
**Builds on:** [`docs/superpowers/specs/2026-08-04-rts-poc-design.md`](../specs/2026-08-04-rts-poc-design.md)
**Reference:** `reverse/` (decompiled `aoc.exe`, function index, strings)

## Reading of the goal

The target is the feature set of the shipped game in `sample/AgeOfEmpires-TheConquerors.exe`, reached in phases. Every phase ends in something playable or measurable, the same rule as the PoC spec. Full byte-level compatibility with the original is **not** a goal. Its `.drs`, `.slp`, and `.dat` formats are read only where a phase needs them.

The PoC spec's non-goals still apply until the phase that removes them. Each removal is a new spec before code starts.

## Where we are

- PoC milestones 1–8 are in code on `feat/rts-poc-milestone-1`. `cargo test --workspace` passes.
- Unverified: the criterion benchmark for 200 units under 5 ms per tick (milestone 4). Measure it before anything else.
- Game-side reference is ready: 7,509 decompiled functions, and the random-map entry point is `FUN_0045ee10`.

## Phases

Each phase has a goal, a deliverable, a verification gate, and the reference to read first. A phase is done when its gate passes. Do not start the next phase while the gate is red.

### Phase 0: Close the PoC

- **Goal:** Confirm the PoC matches its own spec before extending it.
- **Work:** Run the 200-unit criterion benchmark (`crates/sim/benches/step.rs`). Record the flow-field build cost. Check that every proptest in the spec's testing section exists and runs.
- **Gate:** Benchmark under 5 ms per tick, recorded in `docs/` with the machine spec. Any missing property test is added.

### Phase 1: Data-driven content

- **Goal:** Stop hard-coding unit and building stats.
- **Work:** Define a RON schema for units, buildings, and mines. Load it once at startup into plain structs, as the spec's error-handling section already requires.
- **Reference:** No game format read yet. This phase uses our own schema.
- **Gate:** Changing a stat in a RON file changes gameplay with no code change. A test loads a bad file and gets an error, not a panic.

### Phase 2: Four resources

- **Goal:** Food, wood, gold, and stone, each with its own gatherer and drop-off.
- **Work:** Generalise the gather state machine over resource kind. Add a resource field to `Player`. Gold conservation becomes a per-resource property test.
- **Costs become multi-resource.** `UnitStats::cost` changes from one gold number to a cost per resource, written in RON as `cost: (food: 50)`. Match the original game: a villager costs food, not gold. The default content changes to the original's costs (villager 50 food; soldier as the militia, 60 food and 20 gold), confirmed against the game data before they become the defaults. This changes the outcome of default-content games, so it is a recorded replay break.
- **Reference:** Gather loop in `reverse/aoc/aoc_decompiled.c`. Search for the resource strings in `reverse/aoc/strings.txt` first.
- **Gate:** Conservation property test passes for all four resources. A villager never delivers a resource to a building that does not accept it.

### Phase 3: Construction

- **Goal:** Players place buildings. Builders construct them over time.
- **Work:** Placement validation, construction progress, builder assignment. The spec deferred these; this phase writes a new spec first.
- **Gate:** Unit tests for invalid placements, build progress, and builder death mid-construction.

### Phase 4: Ages and research

- **Goal:** Dark, Feudal, Castle, and Imperial ages, with research queues and age-up costs.
- **Work:** A tech table in RON, research queue on buildings, and gating of units and buildings by age.
- **Reference:** Age and tech strings in `reverse/aoc/strings.txt`. Confirm the age costs against the game's data before hard-coding anything.
- **Gate:** A replay recorded before the change still reproduces its checksums, or the change is recorded as an intentional replay break.

### Phase 5: Unit roster

- **Goal:** Add archers, cavalry, monks, and siege. This is the first phase that changes the combat model.
- **Work:** Damage types and armour. The spec ruled both out; this phase replaces that rule with a spec change. Add projectiles only if the archer design needs them.
- **Gate:** Damage unit tests per type. The tick budget benchmark is re-run with the larger roster.

### Phase 6: Maps and scenarios

- **Goal:** Load a real map instead of the hard-coded PoC map.
- **Work:** Start with a fixed-seed generator that produces a valid 128×128 map from terrain and a few placement rules. Read the `.rms` script format and the `.scx` scenario format only if the generator is not enough.
- **Reference:** `FUN_0045ee10` and its callees for the random-map path. `random/*.rms` is in the game's data (in the 7z payload, not in the repo).
- **Gate:** Map generation is deterministic for a seed (same seed, same checksum). Every generated map passes the no-tunnelling property test.

### Phase 7: AI opponent

- **Goal:** A scripted opponent, so one player can play alone.
- **Work:** Start with a rule-based build order. AI issues ordinary `Command` values, so it goes through the same validation as a player.
- **Gate:** The AI wins or loses a fixed-seed match deterministically. The sim has no AI-only code path.

### Phase 8: Fog of war and UI

- **Goal:** Players see only what their units see. Proper selection, hotkeys, and a resource bar.
- **Work:** Visibility as sim state, so it stays in the checksum. The UI reads only the view.
- **Gate:** A property test that no hidden enemy unit appears in a player's view.

### Phase 9: Assets

- **Goal:** Real graphics and sound. This phase is out of the PoC architecture's scope and needs its own spec.
- **Work:** Read `.drs` and `.slp` for sprites and terrain. Read `.wav` and `.mp3` for sound. Keep all asset loading in `app` and `render`.
- **Gate:** A rendered screenshot matches a reference frame, checked by eye. Assets are not committed; they are read from a user-supplied install directory.

### Phase 10: Multiplayer

- **Goal:** Lockstep networking over the existing command stream.
- **Work:** The determinism design already supports this. The new work is transport, input delay, and desync reporting, all in a new spec.
- **Gate:** Two processes stay in sync for a 10-minute recorded match.

## Guidelines

These apply to every phase.

1. **Read the reference first.** Search `reverse/aoc/aoc_decompiled.c` and `reverse/aoc/strings.txt` before designing anything. Write the finding into the phase's spec as a short note with the function address.
2. **Write a spec before code for any phase that changes the design.** Phases 3, 5, 6, 9, and 10 each need their own spec in `docs/superpowers/specs/`.
3. **Test first in `sim`.** Write the failing test, watch it fail for the right reason, then write the code. `sim` is pure, so each test builds a small world and steps it.
4. **Keep the boundaries.** `sim` has no floats and no I/O. `render` draws primitives only. `app` is the only crate that sees both. Enforce this with `cargo clippy` and the existing float lint.
5. **Keep replays honest.** Any change that alters tick order or the checksum is a replay break. Record it in the phase notes and bump the replay format version.
6. **Do not commit game assets.** Read them from a user-supplied install. The repo holds code, specs, and short reference notes only.
7. **Verify before claiming done.** Run `cargo test --workspace` and the phase's gate. Report the output, not a summary of it.
8. **Narrate code work.** Before writing code, say which file changes and why. After, say what changed and which test shows it works.
9. **Commit only when asked.** Use the attribution line from the session instructions.
10. **Check the direction after every phase.** When a phase's gate passes, run two independent subagent checks before the next phase starts:
    - **Code review** of the phase's diff against its spec, for bugs, determinism risks, and weak tests.
    - **Direction check.** A fresh subagent gets only the repository path and one question: compared with the reverse-engineering output in `reverse/`, is the project's current state on the right path toward this plan, and if not, what is missing? Do not give it your own conclusions or a summary of the work, so that its answer is not anchored on yours.
    
    Report both results to the user. Fold any accepted findings into this plan before the next phase.

## Immediate next steps

1. Run Phase 0's benchmark and record the result.
2. Write the Phase 1 spec (RON schema) and its failing tests.
3. Note the four resource strings and their gather code paths in `reverse/` for Phase 2.

## Open questions

- Is the Phase 9 asset read in scope for this repository, or a separate project?
- Should the game's data files be read directly (`empires2_x1_p1.dat`), or should the RON schema be our own?
