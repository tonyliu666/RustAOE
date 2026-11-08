# Age of Empires II: The Conquerors — Rust Remake

A deterministic, rule-by-rule remake of **Age of Empires II: The Conquerors** written in Rust.  
Built by reverse-engineering the original executable and matching its behaviour tick-for-tick.

---

## 🎮 What This Is

- **Genre**: Real-time strategy (RTS) — faithful recreation of AoE2: Conquerors
- **Simulation**: Deterministic, fixed-tick (20 Hz), fixed-point arithmetic (`Fx` = 16.16)
- **Replay system**: Binary format with full content checksums; old versions refused
- **Graphics**: Original `.slp` sprites decoded at runtime from your game install
- **No game assets committed**: You provide your own extracted install

---

## 🛠 Tech Stack

| Layer | Technology |
|-------|------------|
| **Language** | Rust 2021 edition |
| **Graphics** | `macroquad` (OpenGL backend) |
| **Math** | Custom fixed-point `Fx` (i32, 16.16), `FxVec2` |
| **Serialization** | `serde` + `ron` (content), custom binary (replays) |
| **Asset parsing** | Custom `.drs` (archive), `.slp` (sprites), `.dat` (data) readers |
| **Compression** | `miniz_oxide` (raw deflate for `.dat`) |
| **Testing** | `cargo test` — unit, integration, property, mutation-checked |
| **Benchmarks** | `criterion` — sim tick, flow field, lookup |
| **Workspace** | 4 crates: `sim`, `render`, `app`, `assets` + `datconv` tool |

### Crate Layout

```
├── sim/           # Deterministic simulation (no I/O, no graphics)
├── render/        # Renderer abstraction + RecordingRenderer for tests
├── app/           # Binary: session, draw, UI, input, sprites, replay
├── assets/        # DRS/SLP/palette/DAT readers
├── datconv/       # Dev tool: .dat → content RON
├── docs/          # Specs, plans, benchmarks
└── reverse/       # RE tools + docs (gitignored decompiled output)
```

---

## 🚀 Quick Start

### 1. Requirements

- **Rust** 1.80+ (`rustup update`)
- **AoE2: Conquerors install** (original CD or Steam/GOG)
- **Linux** (primary) — macOS/Windows work but untested

### 2. Extract Game Assets

```bash
# Put the extracted install at ./aoe2-install (gitignored)
# Structure expected:
# aoe2-install/
#   data/
#     empires2.dat
#     empires2_x1_p1.dat
#     graphics.drs
#     terrain.drs
#     interfac.drs
#     sound.drs
#     ...
```

Use the provided extraction script (reads original CDs/Steam):

```bash
cd reverse/tools
python3 -I extract.py /path/to/aoe2/install ./aoe2-install
```

### 3. Build & Run

```bash
# Release build (required for playable speed)
cargo run --release -p app -- --assets ./aoe2-install
```

Optional env var (same as `--assets`):
```bash
export AOE_ASSETS=./aoe2-install
cargo run --release -p app
```

### 4. Controls

| Key / Action | Effect |
|--------------|--------|
| **Left click** | Select units / click UI buttons |
| **Right click** | Move / attack / gather / build (context) |
| **Drag left** | Selection rectangle |
| **W / A / S / D** | Pan camera (edge scroll also works) |
| **Mouse wheel** | Zoom |
| **1–9** | Train units at selected building |
| **H** | Place house (with villagers selected) |
| **B / M / L / I** | Barracks / Mill / Lumber Camp / Mining Camp |
| **Esc** | Cancel placement / deselect |
| **F10** | Toggle mouse grab (edge scroll off when free) |
| **Tab** | Cycle players (in replay/debug) |

---

## 🧪 Development

### Run Tests

```bash
cargo test -q --workspace          # All tests
cargo test -q -p sim               # Sim only
cargo test -q -p app --test draw_entities  # Draw tests
```

### Lint & Format

```bash
cargo clippy -q --workspace --all-targets -- -D warnings
cargo fmt --check
```

### Benchmarks

```bash
cargo bench -p sim  # tick, flow field, lookup
```

### Convert `.dat` to Content RON (dev)

```bash
cargo run -p datconv -- --assets ./aoe2-install --output content.ron
```

### Replay Verification

```bash
cargo run --release -p app -- --play replay.bin --verify
```

---

## 📖 Documentation

| File | Purpose |
|------|---------|
| `QUICKSTART.md` | Player guide — run, controls, walkthrough |
| `docs/superpowers/plans/2026-10-24-exe-parity-plan.md` | Master plan & phases |
| `docs/superpowers/specs/` | Phase specs with step results |
| `.superpowers/sdd/.../progress.md` | Step-by-step ledger |
| `docs/benchmarks.md` | Performance numbers |
| `reverse/README.md` | RE tools & methods |

---

## 🧭 Project Status (2026-11-08)

| Phase | Status | Description |
|-------|--------|-------------|
| 0 | ✅ | Baseline benchmarks |
| 1 | ✅ | Content-driven sim, replay v3, sprites, edge scroll |
| 1.5 | ✅ | 4 resources, N players, data-driven IDs, datconv |
| 1.6 | ✅ | Command panel, training, info panel, civ data |
| 2 | ✅ | Four resources, sites, gathering, drop-off, cursors |
| **3** | **In progress** | **Construction & population** |
| 3.1–3.4 | ✅ | Building stats, placement, foundations, build order |
| **3.5** | ✅ | **Construction graphics from .dat** |
| 3.6 | ⏳ | Population cap (next) |
| 4+ | ⏳ | Combat, tech tree, walls, multiplayer, AI... |

---

## 🏗 Architecture Highlights

- **Simulation ≠ Presentation**: `sim::Content` (rules) vs `app::Looks` (sprites)
- **Deterministic replay**: Same binary + same content = identical playback
- **Fixed-point everywhere**: No floating-point in sim — reproducible across platforms
- **Generational IDs**: `UnitId`, `BuildingId`, `FoundationId`, `SiteId` — no Rust references
- **Mutation-checked tests**: Every rule has a test that catches intentional breakage
- **No game data in repo**: Assets loaded from user's install at runtime

---

## 🤝 Contributing

1. Read `CLAUDE.md` (project instructions) and the current phase spec
2. Write a failing test (RED) for the rule you're implementing
3. Make it pass (GREEN)
4. Mutation-check: break the rule, confirm test catches it, restore
5. Run `cargo test -q --workspace && cargo clippy -q --workspace --all-targets -- -D warnings && cargo fmt --check`
6. Update spec + `progress.md`

---

## ⚠️ Legal

- **No game assets included** — you must own AoE2: Conquerors
- **Reverse engineering** for interoperability (EU Directive 2009/24/EC, US 17 USC §1201(f))
- **Not affiliated** with Microsoft, Ensemble Studios, or World's Edge
- This is a **fan project** for educational / preservation purposes

---

## 📜 License

MIT — see `LICENSE` file.  
Game assets remain property of their respective owners.