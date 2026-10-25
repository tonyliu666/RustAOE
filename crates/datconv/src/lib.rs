//! Converting the original's `.dat` into the game's content format.
//!
//! A development tool, not part of the game: it reads the units of one
//! civilisation (see [`assets::dat`]) and writes RON in the format
//! `app::parse_content` reads. Every rule that turns one of the original's
//! numbers into one of ours is a function here with its own test; see
//! `docs/superpowers/specs/2026-10-25-phase1.6-dat-converter.md`.
//!
//! Rounding, everywhere: to the nearest representable value, halves away
//! from zero. Every value that was not already whole in our units is listed
//! in the report, so nothing is rounded silently.

use assets::dat::{Cost, Dat, Unit};
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// Ticks in a second of game time.
const TICKS_PER_SECOND: f64 = 20.0;
/// One in 16.16 fixed point.
const FX_ONE: f64 = 65536.0;
/// The fastest unit the game accepts, in tiles per tick.
const MAX_SPEED: f64 = 0.5;
/// The largest building the game accepts, in tiles a side.
const MAX_BUILDING_SIZE: i64 = 16;
/// The task action that gathers a resource.
const GATHER: i16 = 5;
/// Attack classes counted as damage until armour classes exist (Phase 6):
/// pierce and melee.
const BASE_ATTACKS: [i16; 2] = [3, 4];
/// The male gold miner: a villager while it mines gold, whose capacity and
/// work rate the game's one gather rule uses until Phase 2.
const GOLD_MINER: i16 = 579;

/// A converted value, and the value before rounding when it had to be
/// rounded.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Rounded<T> {
    pub value: T,
    /// The exact value in our units, when it was not whole.
    pub from: Option<f64>,
}

impl<T> Rounded<T> {
    pub fn was_rounded(&self) -> bool {
        self.from.is_some()
    }
}

/// Rounds `raw` to the nearest integer, halves away from zero, noting
/// whether it was whole. Values within a ten-thousandth of a whole number
/// count as whole: the original's `f32` fields cannot hold most decimals
/// exactly.
fn round(raw: f64) -> Rounded<i64> {
    let value = raw.round();
    let from = ((raw - value).abs() > 1e-4).then_some(raw);
    Rounded {
        value: value as i64,
        from,
    }
}

/// Seconds as ticks. A time that must pass, such as training, is at least
/// one tick.
pub fn ticks(seconds: f32, at_least_one: bool) -> Rounded<u32> {
    let r = round(f64::from(seconds) * TICKS_PER_SECOND);
    let floor = if at_least_one { 1 } else { 0 };
    Rounded {
        value: r.value.clamp(floor, i64::from(u32::MAX)) as u32,
        from: r.from,
    }
}

/// A distance in tiles as a `(numerator, denominator)` fraction in lowest
/// terms, at 16.16 precision.
pub fn fraction(tiles: f32) -> Rounded<(i32, i32)> {
    to_fraction(f64::from(tiles))
}

/// Tiles per second as tiles per tick.
pub fn speed_per_tick(tiles_per_second: f32) -> Rounded<(i32, i32)> {
    to_fraction(f64::from(tiles_per_second) / TICKS_PER_SECOND)
}

fn to_fraction(value: f64) -> Rounded<(i32, i32)> {
    let r = round(value * FX_ONE);
    let (mut num, mut den) = (r.value, FX_ONE as i64);
    let divisor = gcd(num.abs(), den);
    if divisor > 1 {
        num /= divisor;
        den /= divisor;
    }
    if num == 0 {
        den = 1;
    }
    Rounded {
        value: (
            num.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            den as i32,
        ),
        from: r.from.map(|raw| raw / FX_ONE),
    }
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a.max(1)
    } else {
        gcd(b, a % b)
    }
}

/// The base damage: the melee and pierce attacks. Bonus attacks against
/// armour classes wait for Phase 6.
pub fn damage(attacks: &[(i16, i16)]) -> i32 {
    attacks
        .iter()
        .filter(|(class, _)| BASE_ATTACKS.contains(class))
        .map(|(_, amount)| i32::from(*amount).max(0))
        .sum()
}

/// A cost as `[food, wood, gold, stone]`, our order. The original numbers
/// resources food 0, wood 1, stone 2, gold 3, so they map by name; its
/// population part (4) waits for Phase 3, and unused parts are not charged.
pub fn cost(costs: &[Cost]) -> [i32; 4] {
    let mut ours = [0; 4];
    for part in costs.iter().filter(|c| c.used) {
        let slot = match part.resource {
            0 => 0,
            1 => 1,
            3 => 2,
            2 => 3,
            _ => continue,
        };
        ours[slot] += i32::from(part.amount);
    }
    ours
}

/// A building's footprint in tiles a side: twice its half-width, at least
/// one.
pub fn building_size(half_width: f32) -> Rounded<i32> {
    let r = round(f64::from(half_width) * 2.0);
    Rounded {
        value: r.value.clamp(1, i64::from(i32::MAX)) as i32,
        from: r.from,
    }
}

/// A gather rate in resource per second as ticks per unit gathered.
pub fn ticks_per_gold(per_second: f32) -> Rounded<u32> {
    if per_second <= 0.0 {
        return Rounded {
            value: 0,
            from: None,
        };
    }
    let r = round(TICKS_PER_SECOND / f64::from(per_second));
    Rounded {
        value: r.value.clamp(1, i64::from(u32::MAX)) as u32,
        from: r.from,
    }
}

/// Which units to write.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Selection {
    /// The four types the game has today, under the names it uses.
    Default,
    /// Every trainable unit and building of the civilisation.
    All,
}

/// The types the game has today: original unit ID and our name.
const DEFAULT_UNITS: [(i16, &str); 2] = [(83, "villager"), (74, "soldier")];
const DEFAULT_BUILDINGS: [(i16, &str); 2] = [(109, "town_center"), (12, "barracks")];

/// Why a file could not be converted.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConvertError {
    /// The file has no civilisation at this index.
    NoCiv(usize),
    /// The civilisation lacks a unit the conversion needs.
    MissingUnit { id: i16, name: &'static str },
    /// A unit the game needs cannot be converted.
    Unconvertible { name: String, problem: String },
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConvertError::NoCiv(index) => write!(f, "the file has no civilisation {index}"),
            ConvertError::MissingUnit { id, name } => {
                write!(f, "the civilisation has no unit {id} ({name})")
            }
            ConvertError::Unconvertible { name, problem } => write!(f, "{name}: {problem}"),
        }
    }
}

impl std::error::Error for ConvertError {}

/// The converted content as RON, and what the conversion had to round,
/// leave out, or could not draw.
#[derive(Clone, PartialEq, Debug)]
pub struct Conversion {
    pub ron: String,
    pub report: Vec<String>,
}

/// A unit type, converted.
struct UnitEntry {
    name: String,
    unit: Unit,
    sprite: i32,
    gathers: bool,
    speed: (i32, i32),
    damage: i32,
    range: (i32, i32),
    attack_cooldown: u32,
    radius: (i32, i32),
    cost: [i32; 4],
    train_ticks: u32,
}

/// A building type, converted.
struct BuildingEntry {
    name: String,
    unit: Unit,
    sprite: i32,
    size: i32,
    drop_off: bool,
}

/// What the whole file says about gathering, for the per-unit rules.
struct Facts<'a> {
    dat: &'a Dat,
    /// The classes of units that have a gather task: a villager has none of
    /// its own, but the gold miner it turns into has.
    gathering_classes: BTreeSet<i16>,
    /// Units a gatherer may drop its load at.
    drop_offs: BTreeSet<i16>,
}

impl<'a> Facts<'a> {
    fn of(dat: &'a Dat, units: &[Option<Unit>]) -> Facts<'a> {
        let gathering_classes: BTreeSet<i16> = units
            .iter()
            .flatten()
            .filter(|u| {
                usize::try_from(u.id)
                    .ok()
                    .and_then(|id| dat.unit_tasks.get(id))
                    .and_then(|t| t.as_ref())
                    .is_some_and(|tasks| tasks.iter().any(|t| t.action == GATHER))
            })
            .map(|u| u.class)
            .collect();
        let drop_offs = units
            .iter()
            .flatten()
            .filter(|u| gathering_classes.contains(&u.class))
            .filter_map(|u| u.drop_sites)
            .flatten()
            .filter(|id| *id >= 0)
            .collect();
        Facts {
            dat,
            gathering_classes,
            drop_offs,
        }
    }
}

/// Records each rounding in the report.
fn note<T: std::fmt::Debug>(report: &mut Vec<String>, field: &str, value: Rounded<T>) -> T {
    if let Some(from) = value.from {
        report.push(format!("{field}: {from} rounded to {:?}", value.value));
    }
    value.value
}

fn sprite_of(
    facts: &Facts<'_>,
    unit: &Unit,
    name: &str,
    report: &mut Vec<String>,
) -> Result<i32, String> {
    let sprite = facts.dat.slp_of(unit.standing_graphic).ok_or_else(|| {
        format!(
            "its standing graphic {} has no sprite",
            unit.standing_graphic
        )
    })?;
    let pieces = usize::try_from(unit.standing_graphic)
        .ok()
        .and_then(|g| facts.dat.graphics.get(g))
        .and_then(|g| g.as_ref())
        .map_or(0, |g| g.deltas.len());
    if pieces > 0 {
        report.push(format!(
            "{name}: graphic {} has {pieces} more pieces, not drawn yet",
            unit.standing_graphic
        ));
    }
    Ok(sprite)
}

fn convert_unit(
    facts: &Facts<'_>,
    unit: &Unit,
    name: String,
    report: &mut Vec<String>,
) -> Result<UnitEntry, String> {
    let mut notes = Vec::new();
    let sprite = sprite_of(facts, unit, &name, &mut notes)?;
    if unit.icon < 0 {
        return Err("it has no icon".to_string());
    }
    let creatable = unit.creatable.as_ref().ok_or("it is not trained")?;
    let speed_per_second = unit.speed.unwrap_or(0.0);
    if speed_per_second <= 0.0 || f64::from(speed_per_second) / TICKS_PER_SECOND > MAX_SPEED {
        return Err(format!(
            "its speed {speed_per_second} is not one the game can move"
        ));
    }
    if unit.hit_points <= 0 {
        return Err("it has no hit points".to_string());
    }
    let field = |f: &str| format!("{name}.{f}");
    let speed = note(
        &mut notes,
        &field("speed"),
        speed_per_tick(speed_per_second),
    );
    let (attacks, max_range, reload) = unit.combat.as_ref().map_or((&[][..], 0.0, 0.0), |c| {
        (&c.attacks[..], c.max_range, c.reload_time)
    });
    let damage = damage(attacks);
    let range = note(&mut notes, &field("range"), fraction(max_range));
    let attack_cooldown = note(
        &mut notes,
        &field("attack_cooldown"),
        ticks(reload, damage > 0),
    );
    let radius = note(&mut notes, &field("radius"), fraction(unit.collision[0]));
    if radius.0 <= 0 {
        return Err("it has no size".to_string());
    }
    let train_ticks = note(
        &mut notes,
        &field("train_ticks"),
        ticks(f32::from(creatable.train_time), true),
    );
    report.extend(notes);
    Ok(UnitEntry {
        gathers: facts.gathering_classes.contains(&unit.class),
        cost: cost(&creatable.costs),
        name,
        unit: unit.clone(),
        sprite,
        speed,
        damage,
        range,
        attack_cooldown,
        radius,
        train_ticks,
    })
}

fn convert_building(
    facts: &Facts<'_>,
    unit: &Unit,
    name: String,
    report: &mut Vec<String>,
) -> Result<BuildingEntry, String> {
    let mut notes = Vec::new();
    let sprite = sprite_of(facts, unit, &name, &mut notes)?;
    if unit.icon < 0 {
        return Err("it has no icon".to_string());
    }
    if unit.hit_points <= 0 {
        return Err("it has no hit points".to_string());
    }
    let size = note(
        &mut notes,
        &format!("{name}.size"),
        building_size(unit.collision[0]),
    );
    if i64::from(size) > MAX_BUILDING_SIZE {
        return Err(format!("its size {size} is larger than the game allows"));
    }
    report.extend(notes);
    Ok(BuildingEntry {
        drop_off: facts.drop_offs.contains(&unit.id),
        name,
        unit: unit.clone(),
        sprite,
        size,
    })
}

/// Converts civilisation `civ` of `dat`.
pub fn convert(dat: &Dat, civ: usize, selection: Selection) -> Result<Conversion, ConvertError> {
    let civilisation = dat.civs.get(civ).ok_or(ConvertError::NoCiv(civ))?;
    let units = &civilisation.units;
    let facts = Facts::of(dat, units);
    let mut report = Vec::new();
    let find = |id: i16, name: &'static str| {
        usize::try_from(id)
            .ok()
            .and_then(|i| units.get(i))
            .and_then(|u| u.as_ref())
            .ok_or(ConvertError::MissingUnit { id, name })
    };

    let (unit_entries, building_entries) = match selection {
        Selection::Default => {
            let mut unit_entries = Vec::new();
            for (id, name) in DEFAULT_UNITS {
                let entry = convert_unit(&facts, find(id, name)?, name.to_string(), &mut report)
                    .map_err(|problem| ConvertError::Unconvertible {
                        name: name.to_string(),
                        problem,
                    })?;
                unit_entries.push(entry);
            }
            let mut building_entries = Vec::new();
            for (id, name) in DEFAULT_BUILDINGS {
                let entry =
                    convert_building(&facts, find(id, name)?, name.to_string(), &mut report)
                        .map_err(|problem| ConvertError::Unconvertible {
                            name: name.to_string(),
                            problem,
                        })?;
                building_entries.push(entry);
            }
            (unit_entries, building_entries)
        }
        Selection::All => {
            let name_of =
                |u: &Unit| format!("{}_{}", u.name.to_lowercase().replace(' ', "_"), u.id);
            let mut unit_entries = Vec::new();
            let mut building_entries = Vec::new();
            for unit in units.iter().flatten() {
                let name = name_of(unit);
                let converted = match unit.kind {
                    70 => convert_unit(&facts, unit, name.clone(), &mut report)
                        .map(|e| unit_entries.push(e)),
                    80 => convert_building(&facts, unit, name.clone(), &mut report)
                        .map(|e| building_entries.push(e)),
                    _ => Ok(()),
                };
                if let Err(problem) = converted {
                    report.push(format!("{name}: left out, {problem}"));
                }
            }
            (unit_entries, building_entries)
        }
    };

    let miner = find(GOLD_MINER, "gold miner")?;
    let carry_capacity = i32::from(miner.resource_capacity);
    let ticks_per_gold = note(
        &mut report,
        "gather.ticks_per_gold",
        ticks_per_gold(miner.work_rate.unwrap_or(0.0)),
    );

    let ron = write_ron(
        &civilisation.name,
        &unit_entries,
        &building_entries,
        carry_capacity,
        ticks_per_gold,
    );
    Ok(Conversion { ron, report })
}

fn write_ron(
    civ: &str,
    units: &[UnitEntry],
    buildings: &[BuildingEntry],
    carry_capacity: i32,
    ticks_per_gold: u32,
) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// Converted by datconv from data/empires2_x1_p1.dat, civilisation {civ}."
    );
    let _ = writeln!(
        out,
        "// Each entry names the original unit ID it came from."
    );
    out.push_str("(\n    units: [\n");
    for u in units {
        let [food, wood, gold, stone] = u.cost;
        let parts: Vec<String> = [
            ("food", food),
            ("wood", wood),
            ("gold", gold),
            ("stone", stone),
        ]
        .into_iter()
        .filter(|(_, amount)| *amount != 0)
        .map(|(name, amount)| format!("{name}: {amount}"))
        .collect();
        let cost = if parts.is_empty() {
            "food: 0".to_string()
        } else {
            parts.join(", ")
        };
        let _ = write!(
            out,
            "        (\n            name: \"{}\", // unit {} {}\n            sprite: {},\n            icon: {},\n            gathers: {},\n            max_hp: {},\n            speed: {:?},\n            damage: {},\n            range: {:?},\n            attack_cooldown: {},\n            radius: {:?},\n            cost: ({cost}),\n            train_ticks: {},\n        ),\n",
            u.name,
            u.unit.id,
            u.unit.name,
            u.sprite,
            u.unit.icon,
            u.gathers,
            u.unit.hit_points,
            u.speed,
            u.damage,
            u.range,
            u.attack_cooldown,
            u.radius,
            u.train_ticks,
        );
    }
    out.push_str("    ],\n    buildings: [\n");
    for b in buildings {
        let mut trained: Vec<&UnitEntry> = units
            .iter()
            .filter(|u| {
                u.unit
                    .creatable
                    .as_ref()
                    .is_some_and(|c| c.train_location == b.unit.id)
            })
            .collect();
        trained.sort_by_key(|u| (u.unit.creatable.as_ref().map_or(0, |c| c.button), u.unit.id));
        let trains: Vec<String> = trained.iter().map(|u| format!("\"{}\"", u.name)).collect();
        let _ = write!(
            out,
            "        (\n            name: \"{}\", // unit {} {}\n            sprite: {},\n            icon: {},\n            size: {},\n            max_hp: {},\n            drop_off: {},\n            trains: [{}],\n        ),\n",
            b.name,
            b.unit.id,
            b.unit.name,
            b.sprite,
            b.unit.icon,
            b.size,
            b.unit.hit_points,
            b.drop_off,
            trains.join(", "),
        );
    }
    let _ = write!(
        out,
        "    ],\n    gather: (\n        carry_capacity: {carry_capacity},\n        ticks_per_gold: {ticks_per_gold},\n    ),\n)\n"
    );
    out
}
