# Reverse-engineering output: Age of Empires II: The Conquerors

Source: `sample/AgeOfEmpires-TheConquerors.exe` (97.7 MB installer).

The `.exe` is a PE stub with a 7z payload appended at offset `0x27c00`. The
payload holds the real game. The game binary is `aoc.exe` (2.7 MB, version
string "Age of Empires II Expansion"), the 7z folder 1 BCJ2 stream.

## Features

Inferred from `aoc/strings.txt`, the PE import table, and the decompiled
code. Nothing here was played or tested. Strings show the feature exists in
the binary; they do not prove it works in every mode.

### Gameplay

- **Random maps.** Map generation from `.rms` scripts. Strings name 14+ maps
  (Arabia, Archipelago, Baltic, Black Forest, and others). `FUN_0045ee10`
  builds a seed string and picks a map script.
- **Scenarios and campaigns.** Scenario play (`Scenario` appears 25 times),
  campaign selection and campaign game screens, and `campaign\*.cpn` /
  `*.cpx` campaign files. Campaign editor screen also present.
- **Resource gathering.** Gold, food, wood, and stone resource strings. Hunt
  and fish targets appear (boar, sheep, fish trap, fishing ship).
- **Relics and wonders.** Relic gathering and wonder-race strings, with
  Wonder appearing 9 times.
- **Population control.** Population caps at 25, 50, 75, 100, 125, 150, 175,
  and 200, plus civilian and military population counts.
- **Unit roster.** Archers, cavalry, scorpions, mangonels, a trebuchet,
  hussars, petards, cannons, galleys, and fishing boats. Exact unit names
  come from the game data, not the binary.
- **Unit commands.** Stance, formation, patrol, guard, garrison, repair,
  tribute to a player, and build-wall (palisade, fortified wall, guard
  tower, bombard tower).
- **Diplomacy.** A "Diplomacy AI" and a "Diplomacy Dialog" are in the
  binary. Ally, neutral, and enemy stances appear in taunt and stance
  strings.
- **Victory and defeat.** Resign, Defeated, Score, Achievements Screen, and
  mission dialogs.
- **Tutorial.** One tutorial string.

### Systems

- **Save and load.** Load appears 63 times and Save 36 times, so save/load
  screens and files are present.
- **Replay.** Replay strings ("Replaying Started"). Recording is probably in
  the game.
- **AI.** AI script files (`ai\*.ai`) and AI-driven behaviour strings
  (`ability-to-maintain-distance`, `can-build-wall`, `population-headroom`).
- **Multiplayer.** Lobby screens, lobby create and launch, `LOBBY: System
  Message`, a zone-message system, and player-drop handling
  (`DropDeadPlayer`). Chat appears 32 times.
- **Networking.** DirectPlay (`DPLAYX.dll`) and WinSock (`WSOCK32.dll`).
- **Rendering and sound.** DirectDraw (`DDRAW.dll`), DirectSound
  (`DSOUND.dll`), GDI, WinMM, and MSVFW32 (video playback).
- **Input.** `IMM32.dll` (input method editor) is imported, which points to
  text entry for chat and names. Hotkey support is not confirmed from the
  strings.

### Not in the binary

Unit stats, tech tree, civilization bonuses, and map layouts are not in
`aoc.exe`. They live in the data files in the payload (`data/*.dat`,
`data/*.drs`, `random/*.rms`). `Town Center` does not appear in the binary
for that reason.

## Layout

| Path | What |
| --- | --- |
| `aoc/aoc_decompiled.c` | Ghidra 12.1.4 decompilation of all 7,509 functions in `aoc.exe`, 0 failures (10 MB) |
| `aoc/function_index.txt` | `name @ address` for every decompiled function |
| `aoc/strings.txt` | Printable strings from `aoc.exe` (`strings -n 5`) |
| `tools/list7z.py` | Parses the 7z container headers (LZMA, LZMA2, multi-coder folders) |
| `tools/extract.py` | Decodes the folders, including BCJ2, and splits out files |
| `tools/scripts/DumpAllC.java` | Ghidra headless script that writes the full C dump |

## Caveats

- Ghidra found no symbols, so most names are `FUN_<address>`. Renaming is a
  later step.
- Decompiled C is a reading aid, not a source-equivalent reconstruction. Types
  are guessed (`undefined4`, `param_1`), and `DAT_` and `s_` labels point to
  addresses, not names.
- Not decompiled: the installer stub `.text`, `config.dll`, `wndmode.dll`,
  `language.dll`, and the 4 MB `data/*.dat` and `*.drs` game data. The data
  files are not code.
- Parts of the 7z decoder are specific to this archive: it assumes a
  one-output-per-coder BCJ2 folder. Other archives may need more work.

## Reproduce

```sh
# 1. decode the installer payload into a fresh directory
python3 -I reverse/tools/extract.py sample/AgeOfEmpires-TheConquerors.exe <empty-dir>
# 2. export all decompiled functions (after importing aoc.exe into a Ghidra project)
analyzeHeadless <proj-dir> aoc -import aoc.exe \
  -scriptPath reverse/tools/scripts -postScript DumpAllC.java <out.c> -deleteProject
```

Set `GHIDRA_INSTALL_DIR` to the Ghidra 12.1.x install (REA verifies 12.1.4 only).

The extracted payload is untrusted. Keep it in its own directory outside the
repo, and do not run the extracted binaries.
