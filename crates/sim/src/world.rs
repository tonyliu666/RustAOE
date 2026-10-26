//! The simulation state and the single tick that advances it.

use crate::entity::{
    Building, BuildingId, BuildingKind, EntityId, GatherPhase, Order, Player, PlayerId, Route,
    Site, SiteId, Unit, UnitId, UnitKind,
};
use crate::path::{astar, Dir8, FlowFieldCache, PathRequest};
use crate::spatial::SpatialHash;
use crate::{Content, Fx, FxVec2, Grid, Resource, Resources, Rng, TilePos};
use serde::{Deserialize, Serialize};
use slotmap::{Key, SlotMap};
use smallvec::SmallVec;
use std::collections::VecDeque;

/// One terrain cell.
///
/// Only passability for now. Units never write to terrain; buildings and mines
/// stamp tiles blocked when placed and unstamp when destroyed.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Tile {
    pub passable: bool,
}

impl Tile {
    pub const OPEN: Tile = Tile { passable: true };
    pub const BLOCKED: Tile = Tile { passable: false };
}

/// The map a world is built from.
///
/// Part of a replay, so it has to describe the map fully — terrain and every
/// starting entity — rather than pointing at a file that may have changed
/// since the recording.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct MapSpec {
    width: i32,
    height: i32,
    blocked: Vec<TilePos>,
    buildings: Vec<(BuildingKind, PlayerId, TilePos)>,
    sites: Vec<Site>,
    units: Vec<(UnitKind, PlayerId, TilePos)>,
    starting_resources: Resources,
    /// Players, not counting Gaia.
    players: u8,
}

/// A map that cannot be played.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MapError(pub String);

impl std::fmt::Display for MapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid map: {}", self.0)
    }
}

impl std::error::Error for MapError {}

/// How a game ended.
///
/// Derived from the players' defeated flags, so that teams (plan Phase 10)
/// change only the derivation.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    Won(PlayerId),
    /// The last players standing were all defeated on the same tick.
    Draw,
}

impl MapSpec {
    /// A map that is passable everywhere and holds nothing.
    pub fn new(width: i32, height: i32) -> MapSpec {
        assert!(width > 0 && height > 0, "map must have positive extent");
        MapSpec {
            width,
            height,
            players: 2,
            ..MapSpec::default()
        }
    }

    /// Marks tiles impassable. Positions outside the map are ignored.
    pub fn with_blocked(mut self, tiles: impl IntoIterator<Item = TilePos>) -> MapSpec {
        self.blocked.extend(tiles);
        self
    }

    pub fn with_building(mut self, kind: BuildingKind, owner: PlayerId, at: TilePos) -> MapSpec {
        self.buildings.push((kind, owner, at));
        self
    }

    /// A gold mine holding `gold`.
    pub fn with_mine(self, at: TilePos, gold: i32) -> MapSpec {
        self.with_site(Resource::Gold, at, gold)
    }

    /// A site of `resource` holding `amount`.
    pub fn with_site(mut self, resource: Resource, at: TilePos, amount: i32) -> MapSpec {
        self.sites.push(Site::new(at, resource, amount));
        self
    }

    /// A farm holding `food`, three tiles a side, which units walk over.
    pub fn with_farm(mut self, at: TilePos, food: i32) -> MapSpec {
        self.sites.push(Site::farm(at, food));
        self
    }

    pub fn with_unit(mut self, kind: UnitKind, owner: PlayerId, at: TilePos) -> MapSpec {
        self.units.push((kind, owner, at));
        self
    }

    /// How many players the map is for, not counting Gaia. Two by default.
    pub fn with_players(mut self, players: u8) -> MapSpec {
        self.players = players;
        self
    }

    pub const fn players(&self) -> u8 {
        self.players
    }

    /// Checks the map can be played: two to eight players, and no entity
    /// owned by a player the map does not have. Gaia may own entities.
    ///
    /// Called by whoever builds a map from outside input, such as a replay
    /// file, before the world is built from it.
    pub fn validate(&self) -> Result<(), MapError> {
        if !(2..=PlayerId::MAX_PLAYERS).contains(&self.players) {
            return Err(MapError(format!(
                "{} players; a map has 2 to {}",
                self.players,
                PlayerId::MAX_PLAYERS
            )));
        }
        let owners = self
            .buildings
            .iter()
            .map(|(_, owner, _)| *owner)
            .chain(self.units.iter().map(|(_, owner, _)| *owner));
        for owner in owners {
            if owner.number() > self.players {
                return Err(MapError(format!(
                    "an entity belongs to player {} on a {}-player map",
                    owner.number(),
                    self.players
                )));
            }
        }
        Ok(())
    }

    /// [`MapSpec::validate`], plus: every building and unit type the map
    /// places exists in `content`. Run on any map from outside, such as a
    /// replay's, against the content it will be played with; the world
    /// indexes content by these types.
    pub fn validate_for(&self, content: &Content) -> Result<(), MapError> {
        self.validate()?;
        if let Some((kind, _, _)) = self
            .buildings
            .iter()
            .find(|(kind, _, _)| kind.index() >= content.buildings.len())
        {
            return Err(MapError(format!(
                "building type {} is not in the content, which has {}",
                kind.index(),
                content.buildings.len()
            )));
        }
        if let Some((kind, _, _)) = self
            .units
            .iter()
            .find(|(kind, _, _)| kind.index() >= content.units.len())
        {
            return Err(MapError(format!(
                "unit type {} is not in the content, which has {}",
                kind.index(),
                content.units.len()
            )));
        }
        Ok(())
    }

    /// What every player starts with.
    pub fn with_starting_resources(mut self, resources: Resources) -> MapSpec {
        self.starting_resources = resources;
        self
    }

    /// Sets the starting gold, leaving the other resources as they are.
    pub fn with_starting_gold(mut self, gold: i32) -> MapSpec {
        self.starting_resources.gold = gold;
        self
    }

    pub const fn width(&self) -> i32 {
        self.width
    }

    pub const fn height(&self) -> i32 {
        self.height
    }

    /// Where a player's base sits, for pointing their camera at it on the
    /// first frame.
    ///
    /// The first building they own, which on every map that ships is their
    /// town centre. Returns nothing for a player with no buildings, who has
    /// already lost.
    pub fn home(&self, player: PlayerId, content: &Content) -> Option<TilePos> {
        self.buildings
            .iter()
            .find(|(_, owner, _)| *owner == player)
            .map(|(kind, _, origin)| {
                let size = content.building(*kind).size;
                TilePos::new(origin.x + size / 2, origin.y + size / 2)
            })
    }

    /// The symmetric two-player map the proof of concept ships with.
    ///
    /// Mirrored so that neither side has an advantage the slice was not meant
    /// to test, and small enough that the two bases are a short walk apart.
    ///
    /// Its types are looked up by name in `content`: `town_center`,
    /// `barracks`, and `villager`. Content without one of them cannot place
    /// this map, which is an error naming the missing type.
    pub fn proof_of_concept(content: &Content) -> Result<MapSpec, MapError> {
        const SIZE: i32 = 128;
        let missing = |name: &str| {
            MapError(format!(
                "the built-in map needs a `{name}` type in the content"
            ))
        };
        let town_center = content
            .building_named("town_center")
            .ok_or_else(|| missing("town_center"))?;
        let barracks = content
            .building_named("barracks")
            .ok_or_else(|| missing("barracks"))?;
        let villager = content
            .unit_named("villager")
            .ok_or_else(|| missing("villager"))?;
        let mill = content
            .building_named("mill")
            .ok_or_else(|| missing("mill"))?;
        let lumber_camp = content
            .building_named("lumber_camp")
            .ok_or_else(|| missing("lumber_camp"))?;
        let mining_camp = content
            .building_named("mining_camp")
            .ok_or_else(|| missing("mining_camp"))?;
        // The original's standard 200 food, so villagers can be trained
        // before food can be gathered (plan Phase 2), and 200 gold for the
        // barracks.
        let mut map = MapSpec::new(SIZE, SIZE).with_starting_resources(Resources {
            food: 200,
            gold: 200,
            ..Resources::default()
        });

        // Each base, laid out from its town centre's corner (the same for
        // both, so neither side is favoured): every resource near the
        // building that takes it, in the amounts the original's sites hold.
        for (player, base) in [
            (PlayerId::ONE, TilePos::new(16, 16)),
            (PlayerId::TWO, TilePos::new(SIZE - 24, SIZE - 24)),
        ] {
            let at = |dx: i32, dy: i32| TilePos::new(base.x + dx, base.y + dy);
            map = map
                .with_building(town_center, player, base)
                .with_building(barracks, player, at(6, 0))
                .with_building(mill, player, at(8, 5))
                .with_building(lumber_camp, player, at(-6, -2))
                .with_building(mining_camp, player, at(-2, 7));
            for i in 0..3 {
                map = map.with_unit(villager, player, at(i, 5));
            }
            // Gold: seven, in two rows; stone: five in a row; both by the
            // mining camp.
            for (dx, dy) in (-6..-2).map(|x| (x, 6)).chain((-6..-3).map(|x| (x, 7))) {
                map = map.with_site(Resource::Gold, at(dx, dy), Site::STARTING_GOLD);
            }
            for dx in 0..5 {
                map = map.with_site(Resource::Stone, at(dx, 9), Site::STARTING_STONE);
            }
            // Six forage bushes and two farms by the mill.
            for (dx, dy) in [(11, 5), (12, 5), (13, 5), (11, 6), (12, 6), (13, 6)] {
                map = map.with_site(Resource::Food, at(dx, dy), Site::BUSH_FOOD);
            }
            map = map
                .with_farm(at(8, 8), Site::FARM_FOOD)
                .with_farm(at(11, 8), Site::FARM_FOOD);
            // A woodline by the lumber camp: three columns of eleven trees.
            for dx in -10..-7 {
                for dy in -6..5 {
                    map = map.with_site(Resource::Wood, at(dx, dy), Site::TREE_WOOD);
                }
            }
        }

        // A forest three trees deep across the middle, with two gaps, so
        // that pathfinding has something to route around on the way between
        // the two bases, and wood to cut a way through.
        for x in (0..SIZE).filter(|x| !(40..48).contains(x) && !(80..88).contains(x)) {
            for y in SIZE / 2 - 1..=SIZE / 2 + 1 {
                map = map.with_site(Resource::Wood, TilePos::new(x, y), Site::TREE_WOOD);
            }
        }
        Ok(map)
    }
}

/// Something a player asked the simulation to do.
///
/// This enum is simultaneously the input abstraction, the replay format, and
/// the future wire format.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Command {
    Move {
        player: PlayerId,
        units: SmallVec<[UnitId; 32]>,
        target: TilePos,
    },
    Attack {
        player: PlayerId,
        units: SmallVec<[UnitId; 32]>,
        target: EntityId,
    },
    Gather {
        player: PlayerId,
        units: SmallVec<[UnitId; 32]>,
        site: SiteId,
    },
    Train {
        player: PlayerId,
        building: BuildingId,
        unit: UnitKind,
    },
    Stop {
        player: PlayerId,
        units: SmallVec<[UnitId; 32]>,
    },
    /// Loaded villagers drop off their loads at `building` before they are
    /// full, then go back to the mine they were working.
    Deposit {
        player: PlayerId,
        units: SmallVec<[UnitId; 32]>,
        building: BuildingId,
    },
}

impl Command {
    /// The player who issued the command.
    pub const fn player(&self) -> PlayerId {
        match self {
            Command::Move { player, .. }
            | Command::Attack { player, .. }
            | Command::Gather { player, .. }
            | Command::Train { player, .. }
            | Command::Stop { player, .. }
            | Command::Deposit { player, .. } => *player,
        }
    }
}

/// The whole simulation state.
pub struct World {
    tick: u64,
    rng: Rng,
    terrain: Grid<Tile>,
    units: SlotMap<UnitId, Unit>,
    buildings: SlotMap<BuildingId, Building>,
    sites: SlotMap<SiteId, Site>,
    /// Slot 0 is Gaia, then one per player.
    players: Vec<Player>,
    /// The numbers every system plays by. Fixed for the life of the world.
    content: Content,
    outcome: Option<Outcome>,
    spatial: SpatialHash,
    flow_fields: FlowFieldCache,
    path_queue: VecDeque<PathRequest>,
    /// Scratch buffer for the movement pass, kept to avoid a per-tick
    /// allocation on the hottest loop in the game.
    scratch_ids: Vec<UnitId>,
}

impl World {
    /// How far apart two edges may be and still touch, in tiles: an eighth.
    ///
    /// Units pressing on a target push each other a little away from it, so
    /// with an exact test only the first to arrive would ever reach a melee
    /// range of 0. Provisional: the original's own tolerance is still to be
    /// found in the decompile (exe-parity plan, Phase 1.6 step 5c).
    pub const CONTACT: Fx = Fx::from_frac(1, 8);

    /// Single-unit paths solved per tick.
    ///
    /// A budget rather than solving every request immediately, so that a
    /// hundred separate orders in one tick cannot spike past the tick budget.
    pub const PATH_BUDGET_PER_TICK: usize = 16;

    /// A world playing by the default content.
    pub fn new(seed: u64, map: MapSpec) -> World {
        World::with_content(seed, map, Content::default())
    }

    /// A world playing by `content`.
    ///
    /// Neither is validated here: that is the loader's job, done once at
    /// startup, so that a bad file is an error message and not a world that
    /// misbehaves. Callers must have run [`Content::validate`] and
    /// [`MapSpec::validate_for`] with this content; the world indexes content
    /// by the map's types and would panic on one the content lacks.
    pub fn with_content(seed: u64, map: MapSpec, content: Content) -> World {
        debug_assert_eq!(
            map.validate_for(&content),
            Ok(()),
            "build worlds from maps validated against their content"
        );
        let mut terrain = Grid::new(map.width, map.height, Tile::OPEN);
        for pos in &map.blocked {
            if terrain.in_bounds(*pos) {
                terrain.set(*pos, Tile::BLOCKED);
            }
        }

        let mut world = World {
            tick: 0,
            rng: Rng::from_seed(seed),
            terrain,
            units: SlotMap::with_key(),
            buildings: SlotMap::with_key(),
            sites: SlotMap::with_key(),
            // Gaia starts with nothing; every player with the map's stock.
            players: (0..=map.players)
                .map(|slot| Player {
                    resources: if slot == 0 {
                        Resources::default()
                    } else {
                        map.starting_resources
                    },
                    defeated: false,
                })
                .collect(),
            content,
            outcome: None,
            spatial: SpatialHash::new(map.width, map.height, SpatialHash::DEFAULT_CELL_SIZE),
            flow_fields: FlowFieldCache::new(FlowFieldCache::DEFAULT_CAPACITY),
            path_queue: VecDeque::new(),
            scratch_ids: Vec::new(),
        };

        for (kind, owner, origin) in map.buildings {
            let hp = world.content.building(kind).max_hp;
            let size = world.content.building(kind).size;
            let building = Building::new(kind, owner, origin, hp, size);
            world.stamp(building.footprint(), Tile::BLOCKED);
            world.buildings.insert(building);
        }
        for site in map.sites {
            if site.blocks {
                world.stamp(site.footprint(), Tile::BLOCKED);
            }
            world.sites.insert(site);
        }
        for (kind, owner, at) in map.units {
            world.units.insert(Unit::new(
                kind,
                owner,
                FxVec2::tile_center(at),
                world.content.unit(kind).max_hp,
            ));
        }
        world
    }

    /// Advances the world exactly one tick.
    ///
    /// Cannot fail and reports nothing. Invalid commands are dropped rather
    /// than returned as errors: once commands arrive from remote players,
    /// invalid traffic is expected traffic.
    ///
    /// The order of these phases is part of the contract. Changing it changes
    /// the outcome of every existing replay.
    pub fn step(&mut self, cmds: &[Command]) {
        self.apply_commands(cmds);
        self.run_production();
        self.run_gathering();
        self.drain_path_queue();
        self.run_movement();
        self.run_combat();
        self.run_cleanup();
        self.check_win();
        self.tick += 1;
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    /// The winner, once one side has lost every building.
    pub const fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    /// Read-only access to the state, for drawing and for the application's
    /// command mapping.
    pub const fn view(&self) -> WorldView<'_> {
        WorldView { world: self }
    }

    /// FNV-1a over the state that a desync would show up in.
    ///
    /// Computed on demand rather than every tick. The generator state is
    /// included because every draw is part of world state: two worlds that
    /// have consumed different numbers of draws have diverged even if nothing
    /// visible differs yet.
    pub fn checksum(&self) -> u64 {
        const OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01B3;

        let mut hash = OFFSET;
        let mut eat = |value: u64| {
            for byte in value.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(PRIME);
            }
        };

        eat(self.tick);
        for word in self.rng.state() {
            eat(word);
        }
        // Routes, stuck counters, the flow field cache, and the path queue are
        // left out: they are planning detail that follows from the order and
        // the position, both of which are hashed.
        for (_, unit) in &self.units {
            eat(unit_kind_code(unit.kind));
            eat(unit.owner.index() as u64);
            eat(unit.pos.x.to_bits() as u32 as u64);
            eat(unit.pos.y.to_bits() as u32 as u64);
            eat(unit.hp as u32 as u64);
            eat(unit.carrying as u32 as u64);
            eat(resource_code(unit.carried));
            eat(u64::from(unit.mine_ticks));
            eat(u64::from(unit.attack_cooldown));
            for word in order_words(&unit.order) {
                eat(word);
            }
        }
        for (_, building) in &self.buildings {
            eat(building_kind_code(building.kind));
            eat(building.owner.index() as u64);
            eat(building.hp as u32 as u64);
            eat(u64::from(building.train_ticks));
            eat(building.queue.len() as u64);
            for kind in &building.queue {
                eat(unit_kind_code(*kind));
            }
        }
        for (_, site) in &self.sites {
            eat(resource_code(site.resource));
            eat(site.remaining as u32 as u64);
        }
        for player in &self.players {
            eat(u64::from(player.defeated));
            for resource in Resource::ALL {
                eat(player.resources.get(resource) as u32 as u64);
            }
        }
        hash
    }

    fn stamp(&mut self, tiles: impl Iterator<Item = TilePos>, tile: Tile) {
        for pos in tiles {
            if self.terrain.in_bounds(pos) {
                self.terrain.set(pos, tile);
            }
        }
    }

    // --- Phase 1: commands -------------------------------------------------

    /// Validates and applies this tick's commands.
    ///
    /// Every command is checked against the issuing player. Wrong owner, dead
    /// or missing entity, insufficient gold, and out-of-bounds targets are all
    /// dropped silently. The simulation never trusts its caller — the same
    /// check that will keep it safe once commands arrive from the network.
    fn apply_commands(&mut self, cmds: &[Command]) {
        for cmd in cmds {
            if !self.may_command(cmd.player()) {
                continue;
            }
            match cmd {
                Command::Move {
                    player,
                    units,
                    target,
                } => self.order_move(*player, units, *target),
                Command::Attack {
                    player,
                    units,
                    target,
                } => self.order_attack(*player, units, *target),
                Command::Gather {
                    player,
                    units,
                    site: mine,
                } => self.order_gather(*player, units, *mine),
                Command::Train {
                    player,
                    building,
                    unit,
                } => self.order_train(*player, *building, *unit),
                Command::Stop { player, units } => {
                    for id in self.owned(*player, units) {
                        self.units[id].order = Order::Idle;
                    }
                }
                Command::Deposit {
                    player,
                    units,
                    building,
                } => self.order_deposit(*player, units, *building),
            }
        }
    }

    /// Filters an addressed unit list down to the ones that exist and belong
    /// to the issuing player.
    /// Gaia, a player the map does not have, and a defeated player cannot
    /// give orders. Dropped like any other invalid command.
    fn may_command(&self, player: PlayerId) -> bool {
        !player.is_gaia()
            && self
                .players
                .get(player.index())
                .is_some_and(|p| !p.defeated)
    }

    fn owned(&self, player: PlayerId, units: &[UnitId]) -> Vec<UnitId> {
        units
            .iter()
            .copied()
            .filter(|id| self.units.get(*id).is_some_and(|u| u.owner == player))
            .collect()
    }

    fn order_move(&mut self, player: PlayerId, units: &[UnitId], target: TilePos) {
        if !self.walkable(target) {
            return;
        }
        let ids = self.owned(player, units);
        let route = self.plan(&ids, target);
        for id in ids {
            self.units[id].order = Order::Move {
                goal: target,
                route: route.clone(),
            };
            self.units[id].stuck_ticks = 0;
            self.units[id].retries = 0;
        }
    }

    fn order_attack(&mut self, player: PlayerId, units: &[UnitId], target: EntityId) {
        let Some(owner) = self.owner_of(target) else {
            return;
        };
        // Friendly fire is not a mechanic here, and a mine has no owner and no
        // hit points, so neither is a legal target.
        if owner == player {
            return;
        }
        for id in self.owned(player, units) {
            if self.content.unit(self.units[id].kind).damage == 0 {
                continue;
            }
            self.units[id].order = Order::Attack {
                target,
                route: Route::Arrived,
            };
            self.units[id].stuck_ticks = 0;
        }
    }

    fn order_gather(&mut self, player: PlayerId, units: &[UnitId], mine: SiteId) {
        let Some(target) = self.sites.get(mine) else {
            return;
        };
        if target.remaining <= 0 {
            return;
        }
        let ids: Vec<UnitId> = self
            .owned(player, units)
            .into_iter()
            .filter(|id| self.content.unit(self.units[*id].kind).gathers)
            .collect();
        let resource = target.resource;
        // As in the original, a site nothing can stand beside, such as a
        // tree inside a woodline, sends the villagers to the nearest one like
        // it that can be reached; with none, they stand.
        let Some(mine) = self.reachable_site_near(mine) else {
            for id in ids {
                self.units[id].order = Order::Idle;
            }
            return;
        };
        let (goal, size) = (self.sites[mine].origin, self.sites[mine].size);
        let route = self.plan_adjacent(&ids, goal, size);
        for id in ids {
            // As in the original, a villager sent to gather something else
            // drops what it carries.
            let unit = &mut self.units[id];
            if unit.carrying > 0 && unit.carried != resource {
                unit.carrying = 0;
            }
            self.units[id].order = Order::Gather {
                site: mine,
                phase: GatherPhase::ToSite,
                route: route.clone(),
            };
            self.units[id].stuck_ticks = 0;
        }
    }

    /// `site` if a tile beside it is walkable, otherwise the site of the same
    /// resource nearest it that has one; ties go to the first in the site
    /// table, so every machine picks the same.
    fn reachable_site_near(&self, site: SiteId) -> Option<SiteId> {
        let target = &self.sites[site];
        let beside = |s: &Site| self.free_tile_beside(s.origin, s.size).is_some();
        if beside(target) {
            return Some(site);
        }
        let (resource, centre) = (target.resource, target.center());
        self.sites
            .iter()
            .filter(|(_, s)| s.resource == resource && s.remaining > 0 && beside(s))
            .min_by_key(|(_, s)| (s.center() - centre).length_squared())
            .map(|(id, _)| id)
    }

    /// Sends loaded villagers to drop off at one of their owner's drop-offs,
    /// then stand, as in the original: dropping off by hand ends gathering.
    /// Units carrying nothing, units whose load the building does not take,
    /// and other players' buildings are ignored.
    fn order_deposit(&mut self, player: PlayerId, units: &[UnitId], building: BuildingId) {
        let Some(target) = self.buildings.get(building) else {
            return;
        };
        if target.owner != player {
            return;
        }
        let stats = self.content.building(target.kind);
        let (origin, size) = (target.origin, target.size);
        let ids: Vec<UnitId> = self
            .owned(player, units)
            .into_iter()
            .filter(|id| {
                let unit = &self.units[*id];
                unit.carrying > 0 && stats.takes(unit.carried)
            })
            .collect();
        let route = self.plan_adjacent(&ids, origin, size);
        for id in ids {
            self.units[id].order = Order::Deposit {
                building,
                route: route.clone(),
            };
            self.units[id].stuck_ticks = 0;
        }
    }

    fn order_train(&mut self, player: PlayerId, building: BuildingId, unit: UnitKind) {
        let Some(target) = self.buildings.get(building) else {
            return;
        };
        if target.owner != player
            || !self.content.building(target.kind).trains.contains(&unit)
            || target.queue.len() >= Building::QUEUE_LIMIT
        {
            return;
        }
        let cost = self.content.unit(unit).cost;
        let stock = &mut self.players[player.index()].resources;
        if !stock.covers(&cost) {
            return;
        }
        // Charged on enqueue rather than on completion, so that queueing five
        // units cannot spend the same resources five times.
        *stock = stock.saturating_sub(cost);
        let target = &mut self.buildings[building];
        if target.queue.is_empty() {
            target.train_ticks = self.content.unit(unit).train_ticks;
        }
        target.queue.push(unit);
    }

    /// Chooses a route for a group heading to one tile.
    ///
    /// More than one unit shares a flow field; a lone unit is queued for A*.
    /// Building a 16 KB field for a single villager is exactly the waste the
    /// two-layer design exists to avoid.
    fn plan(&mut self, ids: &[UnitId], goal: TilePos) -> Route {
        if ids.len() > 1 {
            self.flow_fields.get_or_build(&self.terrain, goal);
            Route::Flow { goal }
        } else {
            for id in ids {
                self.path_queue.push_back(PathRequest { unit: *id, goal });
            }
            Route::Pending { goal }
        }
    }

    /// Like [`World::plan`] but for a target that occupies tiles: routes to a
    /// walkable tile beside the footprint instead of into it.
    fn plan_adjacent(&mut self, ids: &[UnitId], origin: TilePos, size: i32) -> Route {
        match self.free_tile_beside(origin, size) {
            Some(goal) => self.plan(ids, goal),
            None => Route::Arrived,
        }
    }

    /// The first walkable tile in the ring around a footprint.
    ///
    /// Deterministic scan order, because which tile a villager walks to
    /// changes where it stands and therefore the checksum.
    fn free_tile_beside(&self, origin: TilePos, size: i32) -> Option<TilePos> {
        (-1..=size)
            .flat_map(|dy| (-1..=size).map(move |dx| (dx, dy)))
            .filter(|(dx, dy)| *dx == -1 || *dy == -1 || *dx == size || *dy == size)
            .map(|(dx, dy)| TilePos::new(origin.x + dx, origin.y + dy))
            .find(|pos| self.walkable(*pos))
    }

    fn walkable(&self, pos: TilePos) -> bool {
        self.terrain.get(pos).is_some_and(|t| t.passable)
    }

    fn owner_of(&self, target: EntityId) -> Option<PlayerId> {
        match target {
            EntityId::Unit(id) => self.units.get(id).map(|u| u.owner),
            EntityId::Building(id) => self.buildings.get(id).map(|b| b.owner),
            EntityId::Site(_) => None,
        }
    }

    fn position_of(&self, target: EntityId) -> Option<FxVec2> {
        match target {
            EntityId::Unit(id) => self.units.get(id).map(|u| u.pos),
            EntityId::Building(id) => self.buildings.get(id).map(Building::center),
            EntityId::Site(id) => self.sites.get(id).map(Site::center),
        }
    }

    /// Whether `target` is within the attacker's range, measured as the
    /// original measures it: between edges. A unit's edge is its radius from
    /// its centre; a building's or mine's is its footprint, so its corners
    /// are farther than its sides. A melee unit's range is 0: it hits what it
    /// touches.
    ///
    /// Edges within [`World::CONTACT`] of each other count as touching.
    ///
    /// In raw 16.16 units squared, as `i64`, so that the comparison is exact.
    fn in_reach(&self, attacker: UnitId, target: EntityId) -> Option<bool> {
        let unit = &self.units[attacker];
        let stats = self.content.unit(unit.kind);
        let limit = i64::from((stats.range + stats.radius + World::CONTACT).to_bits());
        let pos = unit.pos;
        let footprint = |origin: TilePos, size: i32| {
            // The gap along one axis from the unit's centre to the span
            // [from, from + size), zero inside it.
            let gap = |p: Fx, from: i32| {
                let (lo, hi) = (Fx::from_int(from), Fx::from_int(from + size));
                i64::from(if p < lo {
                    (lo - p).to_bits()
                } else if p > hi {
                    (p - hi).to_bits()
                } else {
                    0
                })
            };
            let (dx, dy) = (gap(pos.x, origin.x), gap(pos.y, origin.y));
            dx * dx + dy * dy <= limit * limit
        };
        match target {
            EntityId::Unit(id) => {
                let other = self.units.get(id)?;
                let reach = limit + i64::from(self.content.unit(other.kind).radius.to_bits());
                Some((other.pos - pos).length_squared() <= reach * reach)
            }
            EntityId::Building(id) => {
                let b = self.buildings.get(id)?;
                Some(footprint(b.origin, b.size))
            }
            // Never a target: a mine belongs to no one, so an attack order on
            // one is refused (`order_attack`).
            EntityId::Site(_) => None,
        }
    }

    // --- Phase 2: production ----------------------------------------------

    fn run_production(&mut self) {
        let ids: Vec<BuildingId> = self.buildings.keys().collect();
        for id in ids {
            let building = &mut self.buildings[id];
            let Some(kind) = building.queue.first().copied() else {
                continue;
            };
            if building.train_ticks > 0 {
                building.train_ticks -= 1;
                continue;
            }
            let (origin, size, owner) = (building.origin, building.size, building.owner);
            // No room to stand means the unit waits rather than being lost:
            // deleting a paid-for unit because the base is crowded would be a
            // silent theft of the player's gold.
            let Some(spawn) = self.free_tile_beside(origin, size) else {
                continue;
            };
            self.units.insert(Unit::new(
                kind,
                owner,
                FxVec2::tile_center(spawn),
                self.content.unit(kind).max_hp,
            ));
            let building = &mut self.buildings[id];
            building.queue.remove(0);
            building.train_ticks = building
                .queue
                .first()
                .map_or(0, |next| self.content.unit(*next).train_ticks);
        }
    }

    // --- Phase 3: gathering -----------------------------------------------

    fn run_gathering(&mut self) {
        let ids: Vec<UnitId> = self.units.keys().collect();
        for id in ids {
            if let Order::Deposit { building, .. } = self.units[id].order {
                self.deposit_and_stand(id, building);
                continue;
            }
            let Order::Gather {
                site: mine, phase, ..
            } = self.units[id].order.clone()
            else {
                continue;
            };
            match phase {
                GatherPhase::ToSite => self.gather_approach(id, mine),
                GatherPhase::Gathering => self.gather_extract(id, mine),
                GatherPhase::ToDropOff { building } => self.gather_deposit(id, mine, building),
            }
        }
    }

    fn gather_approach(&mut self, id: UnitId, mine: SiteId) {
        let Some(target) = self.sites.get(mine) else {
            self.units[id].order = Order::Idle;
            return;
        };
        let remaining = target.remaining;
        if remaining <= 0 {
            // The seam ran out on the way over. Bank whatever is in hand
            // rather than stranding it.
            self.units[id].order = Order::Idle;
            self.send_to_drop_off(id, mine);
            return;
        }
        let origin = target.origin;
        if self.touches(id, origin, target.size) {
            self.units[id].order = Order::Gather {
                site: mine,
                phase: GatherPhase::Gathering,
                route: Route::Arrived,
            };
        }
    }

    fn gather_extract(&mut self, id: UnitId, mine: SiteId) {
        let Some(resource) = self.sites.get(mine).map(|s| s.resource) else {
            self.units[id].order = Order::Idle;
            return;
        };
        let gather = *self.content.gather.get(resource);
        let unit = &mut self.units[id];
        unit.mine_ticks += 1;
        if unit.mine_ticks < gather.ticks_per_unit {
            return;
        }
        unit.mine_ticks = 0;

        let seam = &mut self.sites[mine];
        if seam.remaining > 0 {
            seam.remaining -= 1;
            let unit = &mut self.units[id];
            unit.carried = resource;
            unit.carrying += 1;
        }
        let exhausted = self.sites[mine].remaining <= 0;
        if self.units[id].carrying >= gather.carry_capacity || exhausted {
            self.send_to_drop_off(id, mine);
        }
    }

    fn gather_deposit(&mut self, id: UnitId, mine: SiteId, building: BuildingId) {
        let Some(target) = self.buildings.get(building) else {
            // The drop-off was destroyed mid-walk. Look for another one.
            self.send_to_drop_off(id, mine);
            return;
        };
        let (origin, size) = (target.origin, target.size);
        if !self.touches(id, origin, size) {
            return;
        }

        self.bank(id);

        // Head back for another load, unless the seam is gone or unreachable.
        let Some(_mine_still_exists) = self.sites.get(mine) else {
            self.units[id].order = Order::Idle;
            return;
        };
        let Some(next_mine) = self.reachable_site_near(mine) else {
            self.units[id].order = Order::Idle;
            return;
        };
        let (origin, size) = (self.sites[next_mine].origin, self.sites[next_mine].size);
        let route = self.plan_adjacent(&[id], origin, size);
        self.units[id].order = Order::Gather {
            site: next_mine,
            phase: GatherPhase::ToSite,
            route,
        };
    }

    /// Banks a villager's load into its owner's stock.
    fn bank(&mut self, id: UnitId) {
        let unit = &mut self.units[id];
        let owner = unit.owner;
        let (carried, resource) = (std::mem::take(&mut unit.carrying), unit.carried);
        // Saturating for the same reason as `damage`: content decides how
        // much a load holds, so the sum must not differ between builds.
        let stock = self.players[owner.index()].resources.get_mut(resource);
        *stock = stock.saturating_add(carried);
    }

    /// A villager dropping off by hand: banks once it touches the drop-off,
    /// then stands.
    fn deposit_and_stand(&mut self, id: UnitId, building: BuildingId) {
        let Some(target) = self.buildings.get(building) else {
            // The drop-off is gone; the load stays carried.
            self.units[id].order = Order::Idle;
            return;
        };
        let (origin, size) = (target.origin, target.size);
        if self.touches(id, origin, size) {
            self.bank(id);
            self.units[id].order = Order::Idle;
        }
    }

    /// Sends a loaded villager to the nearest of its owner's buildings that
    /// takes its load and has a free adjacent tile.
    fn send_to_drop_off(&mut self, id: UnitId, mine: SiteId) {
        let unit = &self.units[id];
        if unit.carrying == 0 {
            self.units[id].order = Order::Idle;
            return;
        }
        let (pos, owner, load) = (unit.pos, unit.owner, unit.carried);
        let nearest = self
            .buildings
            .iter()
            .filter(|(_, b)| {
                b.owner == owner
                    && self.content.building(b.kind).takes(load)
                    && self.free_tile_beside(b.origin, b.size).is_some()
            })
            .min_by_key(|(_, b)| (b.center() - pos).length_squared())
            .map(|(id, b)| (id, b.origin, b.size));

        let Some((building, origin, size)) = nearest else {
            // Nowhere to bank it. The gold stays carried, which keeps the
            // conservation invariant true, and the villager idles.
            self.units[id].order = Order::Idle;
            return;
        };
        let route = self.plan_adjacent(&[id], origin, size);
        self.units[id].order = Order::Gather {
            site: mine,
            phase: GatherPhase::ToDropOff { building },
            route,
        };
    }

    /// The point of a square footprint nearest a unit's centre, and the
    /// offset to it.
    fn nearest_point(&self, id: UnitId, origin: TilePos, size: i32) -> FxVec2 {
        let pos = self.units[id].pos;
        let clamp = |p: Fx, from: i32| p.clamp(Fx::from_int(from), Fx::from_int(from + size));
        FxVec2::new(clamp(pos.x, origin.x), clamp(pos.y, origin.y)) - pos
    }

    /// Whether a unit touches a square footprint: its edge within
    /// [`World::CONTACT`] of the footprint's, as for a melee attack. A
    /// villager mines and deposits only so close.
    fn touches(&self, id: UnitId, origin: TilePos, size: i32) -> bool {
        let radius = self.content.unit(self.units[id].kind).radius;
        let limit = i64::from((radius + World::CONTACT).to_bits());
        self.nearest_point(id, origin, size).length_squared() <= limit * limit
    }

    /// The footprint a gathering villager is walking to: its mine, or the
    /// drop-off it is taking a load to. `None` while it mines.
    fn gather_footprint(&self, id: UnitId) -> Option<(TilePos, i32)> {
        match self.units[id].order {
            Order::Gather {
                site: mine,
                phase: GatherPhase::ToSite,
                ..
            } => self.sites.get(mine).map(|m| (m.origin, m.size)),
            Order::Gather {
                phase: GatherPhase::ToDropOff { building },
                ..
            }
            | Order::Deposit { building, .. } => {
                self.buildings.get(building).map(|b| (b.origin, b.size))
            }
            _ => None,
        }
    }

    // --- Phase 4: path budget ---------------------------------------------

    /// Solves at most [`World::PATH_BUDGET_PER_TICK`] queued single-unit paths.
    fn drain_path_queue(&mut self) {
        for _ in 0..World::PATH_BUDGET_PER_TICK {
            let Some(request) = self.path_queue.pop_front() else {
                break;
            };
            let Some(unit) = self.units.get(request.unit) else {
                continue;
            };
            // The order changed while the request sat in the queue, so the
            // answer is no longer wanted.
            let still_wanted = matches!(unit.order.route(), Some(Route::Pending { goal }) if *goal == request.goal);
            if !still_wanted {
                continue;
            }
            let start = unit.tile();
            let route = match astar(&self.terrain, start, request.goal) {
                Some(steps) if !steps.is_empty() => Route::Path { steps, next: 0 },
                // Unreachable, or already there. Either way, stop walking.
                _ => Route::Arrived,
            };
            self.units[request.unit].order.set_route(route);
        }
    }

    // --- Phase 5: movement -------------------------------------------------

    fn run_movement(&mut self) {
        self.spatial.clear();
        for (id, unit) in &mut self.units {
            unit.prev_pos = unit.pos;
            self.spatial.insert(id, unit.pos);
        }

        // Any flow field a unit is following has to exist before the movement
        // loop starts, because that loop only holds a shared borrow of the
        // cache. Building them up front also means units sharing a goal share
        // the one build, which is the whole point of the layer.
        self.scratch_ids.clear();
        self.scratch_ids.extend(self.units.keys());
        let goals: Vec<TilePos> = self
            .units
            .values()
            .filter_map(|u| match u.order.route() {
                Some(Route::Flow { goal }) => Some(*goal),
                _ => None,
            })
            .collect();
        for goal in goals {
            if self.flow_fields.peek(goal).is_none() {
                self.flow_fields.get_or_build(&self.terrain, goal);
            }
        }

        let ids = std::mem::take(&mut self.scratch_ids);
        for id in &ids {
            self.move_unit(*id);
        }
        self.scratch_ids = ids;
    }

    fn move_unit(&mut self, id: UnitId) {
        let unit = &self.units[id];
        let (pos, kind) = (unit.pos, unit.kind);
        let (speed, radius) = {
            let stats = self.content.unit(kind);
            (stats.speed, stats.radius)
        };

        let Some(desired) = self.desired_direction(id) else {
            self.units[id].stuck_ticks = 0;
            return;
        };

        // Separation is capped well below the desired direction, so a crowd
        // jostles while still making headway. Letting it match the goal
        // direction in strength is what produces the vibrating blob that never
        // gets through a gap.
        let crowding = self.separation(id).clamp_length(speed.div(Fx::from_int(2)));
        let steering = (desired * speed + crowding).clamp_length(speed);
        let moved = self.slide(pos, steering, radius);
        self.units[id].pos = moved;

        // A unit that has been pressed against something for a second and a
        // half is not going to get there. Giving up is crude, but it is what
        // stops a blocked crowd vibrating against a wall forever.
        let progress = (moved - pos).length_squared() * 16 >= (speed.to_bits() as i64).pow(2);
        if progress {
            self.units[id].stuck_ticks = 0;
            self.units[id].retries = 0;
            return;
        }
        self.units[id].stuck_ticks += 1;
        if self.units[id].stuck_ticks < Unit::STUCK_LIMIT {
            return;
        }
        self.units[id].stuck_ticks = 0;

        let goal = self.units[id].order.route().and_then(Route::goal);
        match goal.filter(|_| self.units[id].retries < Unit::STUCK_RETRIES) {
            Some(goal) => {
                self.units[id].retries += 1;
                self.units[id].order.set_route(Route::Pending { goal });
                self.path_queue.push_back(PathRequest { unit: id, goal });
            }
            None => {
                self.units[id].retries = 0;
                self.units[id].order.set_route(Route::Arrived);
                if matches!(self.units[id].order, Order::Move { .. }) {
                    self.units[id].order = Order::Idle;
                }
            }
        }
    }

    /// The unit direction this unit wants to travel, or `None` if it should
    /// stand still.
    fn desired_direction(&mut self, id: UnitId) -> Option<FxVec2> {
        let pos = self.units[id].pos;

        // An attacker walks at whatever it is attacking rather than at a tile,
        // because the target moves.
        if let Order::Attack { target, .. } = self.units[id].order {
            let target_pos = self.position_of(target)?;
            let offset = target_pos - pos;
            return (!self.in_reach(id, target)?).then(|| offset.with_length(Fx::ONE));
        }

        // A gathering villager's route ends on the tile beside its mine or
        // drop-off; from there it walks the last stretch until it touches,
        // since it mines and deposits only then.
        if let Some((origin, size)) = self.gather_footprint(id) {
            if self.touches(id, origin, size) {
                return None;
            }
            if matches!(self.units[id].order.route(), None | Some(Route::Arrived)) {
                let offset = self.nearest_point(id, origin, size);
                return (offset.length_squared() > 0).then(|| offset.with_length(Fx::ONE));
            }
        }

        match self.units[id].order.route()?.clone() {
            Route::Pending { .. } | Route::Arrived => None,
            Route::Flow { goal } => {
                let arrived = self.arrived_at(pos, goal);
                if arrived {
                    self.finish_route(id);
                    return None;
                }
                let field = self.flow_fields.peek(goal)?;
                let dir = field.direction(pos.tile());
                if dir == Dir8::None {
                    self.finish_route(id);
                    return None;
                }
                let (dx, dy) = dir.offset();
                Some(FxVec2::new(Fx::from_int(dx), Fx::from_int(dy)).with_length(Fx::ONE))
            }
            Route::Path { steps, next } => {
                let mut next = next;
                // Skip every waypoint already reached, so that cutting a
                // corner does not leave the unit walking backwards to a
                // waypoint it passed.
                while next < steps.len() && self.arrived_at(pos, steps[next]) {
                    next += 1;
                }
                if next >= steps.len() {
                    self.finish_route(id);
                    return None;
                }
                self.units[id].order.set_route(Route::Path {
                    steps: steps.clone(),
                    next,
                });
                Some((FxVec2::tile_center(steps[next]) - pos).with_length(Fx::ONE))
            }
        }
    }

    fn arrived_at(&self, pos: FxVec2, goal: TilePos) -> bool {
        let limit = Unit::ARRIVE_RADIUS.to_bits() as i64;
        (FxVec2::tile_center(goal) - pos).length_squared() <= limit * limit
    }

    /// Marks a route finished, and idles the unit if walking was the whole
    /// order.
    fn finish_route(&mut self, id: UnitId) {
        if matches!(self.units[id].order, Order::Move { .. }) {
            self.units[id].order = Order::Idle;
        } else {
            self.units[id].order.set_route(Route::Arrived);
        }
    }

    /// The push away from crowding neighbours.
    ///
    /// Scaled by how deeply the two overlap, so units barely touching drift
    /// apart and units on top of each other separate hard.
    fn separation(&self, id: UnitId) -> FxVec2 {
        let unit = &self.units[id];
        let (pos, radius) = (unit.pos, self.content.unit(unit.kind).radius);
        let spacing = radius + radius;
        let limit = spacing.to_bits() as i64;

        let mut push = FxVec2::ZERO;
        self.spatial.for_each_near(pos, |other_id| {
            if other_id == id {
                return;
            }
            let Some(other) = self.units.get(other_id) else {
                return;
            };
            let offset = pos - other.pos;
            let distance_squared = offset.length_squared();
            if distance_squared >= limit * limit {
                return;
            }
            if distance_squared == 0 {
                // Exactly coincident: no direction to push along. Nudging by
                // the identifier would be non-deterministic across runs that
                // allocate keys differently, so leave it to the next tick,
                // once movement has separated them.
                return;
            }
            let overlap = spacing - offset.length();
            push = push + offset.with_length(overlap);
        });
        push
    }

    /// Applies a step, sliding along whichever axis is not blocked.
    ///
    /// Trying the axes separately rather than rejecting the whole step is what
    /// lets a unit walk along a wall instead of sticking to it.
    fn slide(&self, pos: FxVec2, step: FxVec2, radius: Fx) -> FxVec2 {
        let both = pos + step;
        if self.clear(both, radius) {
            return both;
        }
        let horizontal = FxVec2::new(both.x, pos.y);
        if self.clear(horizontal, radius) {
            return horizontal;
        }
        let vertical = FxVec2::new(pos.x, both.y);
        if self.clear(vertical, radius) {
            return vertical;
        }
        pos
    }

    /// Whether a unit of `radius` centred at `pos` is clear of terrain.
    ///
    /// The four corners of the unit's bounding box are tested rather than just
    /// its centre, which is what keeps a unit from clipping into the corner of
    /// a building at speed.
    fn clear(&self, pos: FxVec2, radius: Fx) -> bool {
        for (sx, sy) in [(-1, 1), (1, 1)] {
            for sign in [-1, 1] {
                let corner = FxVec2::new(
                    pos.x + radius.mul(Fx::from_int(sx * sign)),
                    pos.y + radius.mul(Fx::from_int(sy * sign)),
                );
                if !self.walkable(corner.tile()) {
                    return false;
                }
            }
        }
        true
    }

    // --- Phase 6: combat ---------------------------------------------------

    fn run_combat(&mut self) {
        let ids: Vec<UnitId> = self.units.keys().collect();
        for id in ids {
            if self.units[id].attack_cooldown > 0 {
                self.units[id].attack_cooldown -= 1;
            }
            let Order::Attack { target, .. } = self.units[id].order else {
                continue;
            };
            let Some(in_reach) = self.in_reach(id, target) else {
                // The victim died, possibly to someone else. Stop swinging.
                self.units[id].order = Order::Idle;
                continue;
            };
            let stats = self.content.unit(self.units[id].kind);
            let (cooldown, damage) = (stats.attack_cooldown, stats.damage);
            if !in_reach {
                continue;
            }
            if self.units[id].attack_cooldown > 0 {
                continue;
            }
            self.units[id].attack_cooldown = cooldown;
            self.damage(target, damage);
        }
    }

    /// Saturating rather than plain subtraction: content may set damage as
    /// high as `i32::MAX`, and several such hits on one target in one tick
    /// would otherwise panic a debug build and wrap the target back to life
    /// in a release build, so the two would play different games.
    fn damage(&mut self, target: EntityId, amount: i32) {
        match target {
            EntityId::Unit(id) => {
                if let Some(unit) = self.units.get_mut(id) {
                    unit.hp = unit.hp.saturating_sub(amount);
                }
            }
            EntityId::Building(id) => {
                if let Some(building) = self.buildings.get_mut(id) {
                    building.hp = building.hp.saturating_sub(amount);
                }
            }
            // Mines cannot be attacked; `order_attack` rejects them, and a
            // stray one here is a no-op rather than a panic.
            EntityId::Site(_) => {}
        }
    }

    // --- Phase 7: cleanup --------------------------------------------------

    fn run_cleanup(&mut self) {
        self.units.retain(|_, unit| unit.hp > 0);

        // A site with nothing left disappears; a tree or mine frees its tile.
        let used_up: Vec<SiteId> = self
            .sites
            .iter()
            .filter(|(_, s)| s.remaining <= 0)
            .map(|(id, _)| id)
            .collect();
        let mut terrain_changed = false;
        for id in used_up {
            let site = self.sites.remove(id).expect("listed above");
            if site.blocks {
                self.stamp(site.footprint(), Tile::OPEN);
                terrain_changed = true;
            }
        }

        let dead: Vec<BuildingId> = self
            .buildings
            .iter()
            .filter(|(_, b)| b.hp <= 0)
            .map(|(id, _)| id)
            .collect();
        for id in dead {
            let footprint: Vec<TilePos> = self.buildings[id].footprint().collect();
            self.buildings.remove(id);
            self.stamp(footprint.into_iter(), Tile::OPEN);
            terrain_changed = true;
        }
        // Terrain changed, so every cached field may now route around a wall
        // that is no longer there.
        if terrain_changed {
            self.flow_fields.clear();
        }
    }

    // --- Phase 8: win check ------------------------------------------------

    /// Sets the outcome the first time a player has no buildings left.
    ///
    /// The simulation keeps ticking afterwards; announcing the result is the
    /// application's concern.
    fn check_win(&mut self) {
        // The PoC's conquest rule, provisional until the original's is
        // confirmed: a player with no buildings is out. Gaia never is.
        for (slot, player) in self.players.iter_mut().enumerate().skip(1) {
            let id = PlayerId::new(slot as u8);
            if !player.defeated && !self.buildings.values().any(|b| b.owner == id) {
                player.defeated = true;
            }
        }
        if self.outcome.is_some() {
            return;
        }
        let mut standing = (1..self.players.len())
            .filter(|slot| !self.players[*slot].defeated)
            .map(|slot| PlayerId::new(slot as u8));
        self.outcome = match (standing.next(), standing.next()) {
            (None, _) => Some(Outcome::Draw),
            (Some(winner), None) => Some(Outcome::Won(winner)),
            (Some(_), Some(_)) => None,
        };
    }
}

impl Order {
    fn route(&self) -> Option<&Route> {
        match self {
            Order::Idle => None,
            Order::Move { route, .. }
            | Order::Attack { route, .. }
            | Order::Gather { route, .. }
            | Order::Deposit { route, .. } => Some(route),
        }
    }

    fn set_route(&mut self, new_route: Route) {
        match self {
            Order::Idle => {}
            Order::Move { route, .. }
            | Order::Attack { route, .. }
            | Order::Gather { route, .. }
            | Order::Deposit { route, .. } => {
                *route = new_route;
            }
        }
    }
}

/// A read-only window onto a [`World`].
#[derive(Copy, Clone)]
pub struct WorldView<'a> {
    world: &'a World,
}

impl<'a> WorldView<'a> {
    pub const fn tick(&self) -> u64 {
        self.world.tick
    }

    pub const fn width(&self) -> i32 {
        self.world.terrain.width()
    }

    pub const fn height(&self) -> i32 {
        self.world.terrain.height()
    }

    pub const fn outcome(&self) -> Option<Outcome> {
        self.world.outcome
    }

    /// Players, not counting Gaia.
    pub fn player_count(&self) -> u8 {
        (self.world.players.len() - 1) as u8
    }

    /// Whether `player` is out of the game. Gaia never is.
    pub fn defeated(&self, player: PlayerId) -> bool {
        self.world
            .players
            .get(player.index())
            .is_some_and(|p| p.defeated)
    }

    /// The tile at `pos`, or `None` if it is off the map.
    pub fn tile(&self, pos: TilePos) -> Option<Tile> {
        self.world.terrain.get(pos).copied()
    }

    pub fn resources(&self, player: PlayerId) -> Resources {
        self.world.players[player.index()].resources
    }

    /// Shorthand for the gold part of [`WorldView::resources`].
    pub fn gold(&self, player: PlayerId) -> i32 {
        self.resources(player).gold
    }

    /// The numbers the world plays by, for anything drawn from them, such as
    /// the length of a full health bar.
    pub const fn content(&self) -> &'a Content {
        &self.world.content
    }

    pub fn units(&self) -> impl Iterator<Item = (UnitId, &'a Unit)> {
        self.world.units.iter()
    }

    pub fn unit(&self, id: UnitId) -> Option<&'a Unit> {
        self.world.units.get(id)
    }

    pub fn buildings(&self) -> impl Iterator<Item = (BuildingId, &'a Building)> {
        self.world.buildings.iter()
    }

    pub fn building(&self, id: BuildingId) -> Option<&'a Building> {
        self.world.buildings.get(id)
    }

    pub fn sites(&self) -> impl Iterator<Item = (SiteId, &'a Site)> {
        self.world.sites.iter()
    }

    pub fn site(&self, id: SiteId) -> Option<&'a Site> {
        self.world.sites.get(id)
    }

    /// Total gold still in the ground plus gold in transit plus gold banked.
    ///
    /// Exposed for the conservation property test, which is the only cheap
    /// defence against a resource duplication bug.
    /// All of one resource in the world: banked by players, carried by
    /// units, and left in sites. Only training spends any and only a load
    /// dropped for another resource loses any, so gathering never changes
    /// it.
    pub fn total(&self, resource: Resource) -> i64 {
        let banked: i64 = self
            .world
            .players
            .iter()
            .map(|p| i64::from(p.resources.get(resource)))
            .sum();
        let carried: i64 = self
            .world
            .units
            .values()
            .filter(|u| u.carried == resource)
            .map(|u| i64::from(u.carrying))
            .sum();
        let left: i64 = self
            .world
            .sites
            .values()
            .filter(|s| s.resource == resource)
            .map(|s| i64::from(s.remaining))
            .sum();
        banked + carried + left
    }

    /// Unit positions, for the renderer to interpolate between ticks.
    pub fn snapshot(&self) -> Vec<(UnitId, FxVec2)> {
        self.world.units.iter().map(|(id, u)| (id, u.pos)).collect()
    }
}

const fn resource_code(resource: Resource) -> u64 {
    match resource {
        Resource::Food => 0,
        Resource::Wood => 1,
        Resource::Gold => 2,
        Resource::Stone => 3,
    }
}

const fn unit_kind_code(kind: UnitKind) -> u64 {
    kind.index() as u64
}

const fn building_kind_code(kind: BuildingKind) -> u64 {
    kind.index() as u64
}

/// An entity as two words: which table, and its key's bits.
fn entity_words(entity: EntityId) -> [u64; 2] {
    match entity {
        EntityId::Unit(id) => [0, id.data().as_ffi()],
        EntityId::Building(id) => [1, id.data().as_ffi()],
        EntityId::Site(id) => [2, id.data().as_ffi()],
    }
}

/// An order as a fixed number of words, so that two different orders can
/// never hash as the same byte stream.
fn order_words(order: &Order) -> [u64; 5] {
    match order {
        Order::Idle => [0; 5],
        Order::Move { goal, .. } => [1, goal.x as u32 as u64, goal.y as u32 as u64, 0, 0],
        Order::Attack { target, .. } => {
            let [table, key] = entity_words(*target);
            [2, table, key, 0, 0]
        }
        Order::Gather {
            site: mine, phase, ..
        } => {
            let (phase, building) = match phase {
                GatherPhase::ToSite => (0, 0),
                GatherPhase::Gathering => (1, 0),
                GatherPhase::ToDropOff { building } => (2, building.data().as_ffi()),
            };
            [3, mine.data().as_ffi(), phase, building, 0]
        }
        Order::Deposit { building, .. } => [4, building.data().as_ffi(), 0, 0, 0],
    }
}

/// The checksum covers every piece of state a desync could hide in.
///
/// Inside the crate so that each test can change exactly one private field:
/// through the public interface an order also moves the unit, so position
/// would differ too and the test would not show the order is hashed.
#[cfg(test)]
mod checksum_tests {
    use super::*;

    /// One villager, one town centre each, and a mine.
    fn world() -> World {
        let map = MapSpec::new(16, 16)
            .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(1, 1))
            .with_building(
                BuildingKind::TOWN_CENTER,
                PlayerId::TWO,
                TilePos::new(10, 10),
            )
            .with_mine(TilePos::new(6, 1), 100)
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(6, 6));
        World::new(1, map)
    }

    fn unit(world: &World) -> UnitId {
        world.units.keys().next().expect("a unit")
    }

    fn building(world: &World) -> BuildingId {
        world.buildings.keys().next().expect("a building")
    }

    fn mine(world: &World) -> SiteId {
        world.sites.keys().next().expect("a mine")
    }

    /// Whether two otherwise identical worlds, one changed by `a` and one by
    /// `b`, have different checksums.
    fn differs(a: impl FnOnce(&mut World), b: impl FnOnce(&mut World)) -> bool {
        let (mut left, mut right) = (world(), world());
        a(&mut left);
        b(&mut right);
        left.checksum() != right.checksum()
    }

    fn untouched(_: &mut World) {}

    #[test]
    fn unit_kind_is_hashed() {
        assert!(differs(untouched, |w| {
            let id = unit(w);
            w.units[id].kind = UnitKind::SOLDIER;
        }));
    }

    #[test]
    fn unit_order_is_hashed() {
        assert!(differs(untouched, |w| {
            let id = unit(w);
            w.units[id].order = Order::Move {
                goal: TilePos::new(2, 2),
                route: Route::Arrived,
            };
        }));
    }

    #[test]
    fn the_goal_of_an_order_is_hashed() {
        let move_to = |goal| {
            move |w: &mut World| {
                let id = unit(w);
                w.units[id].order = Order::Move {
                    goal,
                    route: Route::Arrived,
                };
            }
        };
        assert!(differs(
            move_to(TilePos::new(2, 2)),
            move_to(TilePos::new(3, 2))
        ));
    }

    #[test]
    fn the_target_of_an_attack_is_hashed() {
        let attack = |pick: fn(&World) -> EntityId| {
            move |w: &mut World| {
                let (id, target) = (unit(w), pick(w));
                w.units[id].order = Order::Attack {
                    target,
                    route: Route::Arrived,
                };
            }
        };
        assert!(differs(
            attack(|w| EntityId::Building(building(w))),
            attack(|w| EntityId::Site(mine(w))),
        ));
    }

    #[test]
    fn the_gather_phase_is_hashed() {
        let gather = |phase| {
            move |w: &mut World| {
                let (id, mine) = (unit(w), mine(w));
                w.units[id].order = Order::Gather {
                    site: mine,
                    phase,
                    route: Route::Arrived,
                };
            }
        };
        assert!(differs(
            gather(GatherPhase::ToSite),
            gather(GatherPhase::Gathering)
        ));
    }

    #[test]
    fn mine_ticks_are_hashed() {
        assert!(differs(untouched, |w| {
            let id = unit(w);
            w.units[id].mine_ticks = 7;
        }));
    }

    #[test]
    fn attack_cooldown_is_hashed() {
        assert!(differs(untouched, |w| {
            let id = unit(w);
            w.units[id].attack_cooldown = 3;
        }));
    }

    #[test]
    fn building_kind_is_hashed() {
        assert!(differs(untouched, |w| {
            let id = building(w);
            w.buildings[id].kind = BuildingKind::BARRACKS;
        }));
    }

    #[test]
    fn building_owner_is_hashed() {
        assert!(differs(untouched, |w| {
            let id = building(w);
            w.buildings[id].owner = PlayerId::TWO;
        }));
    }

    #[test]
    fn the_contents_of_a_training_queue_are_hashed() {
        let queue = |kind| {
            move |w: &mut World| {
                let id = building(w);
                w.buildings[id].queue = vec![kind];
            }
        };
        assert!(differs(queue(UnitKind::VILLAGER), queue(UnitKind::SOLDIER)));
    }

    #[test]
    fn training_progress_is_hashed() {
        assert!(differs(untouched, |w| {
            let id = building(w);
            w.buildings[id].train_ticks = 9;
        }));
    }
}
