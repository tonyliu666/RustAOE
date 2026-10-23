//! The simulation state and the single tick that advances it.

use crate::entity::{
    Building, BuildingId, BuildingKind, EntityId, GatherPhase, Mine, MineId, Order, Player,
    PlayerId, Route, Unit, UnitId, UnitKind,
};
use crate::path::{astar, Dir8, FlowFieldCache, PathRequest};
use crate::spatial::SpatialHash;
use crate::{Fx, FxVec2, Grid, Rng, TilePos};
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;
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
    mines: Vec<(TilePos, i32)>,
    units: Vec<(UnitKind, PlayerId, TilePos)>,
    starting_gold: i32,
}

impl MapSpec {
    /// A map that is passable everywhere and holds nothing.
    pub fn new(width: i32, height: i32) -> MapSpec {
        assert!(width > 0 && height > 0, "map must have positive extent");
        MapSpec {
            width,
            height,
            starting_gold: 0,
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

    pub fn with_mine(mut self, at: TilePos, gold: i32) -> MapSpec {
        self.mines.push((at, gold));
        self
    }

    pub fn with_unit(mut self, kind: UnitKind, owner: PlayerId, at: TilePos) -> MapSpec {
        self.units.push((kind, owner, at));
        self
    }

    pub fn with_starting_gold(mut self, gold: i32) -> MapSpec {
        self.starting_gold = gold;
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
    pub fn home(&self, player: PlayerId) -> Option<TilePos> {
        self.buildings
            .iter()
            .find(|(_, owner, _)| *owner == player)
            .map(|(kind, _, origin)| {
                TilePos::new(origin.x + kind.size() / 2, origin.y + kind.size() / 2)
            })
    }

    /// The symmetric two-player map the proof of concept ships with.
    ///
    /// Mirrored so that neither side has an advantage the slice was not meant
    /// to test, and small enough that the two bases are a short walk apart.
    pub fn proof_of_concept() -> MapSpec {
        const SIZE: i32 = 128;
        let mut map = MapSpec::new(SIZE, SIZE).with_starting_gold(200);

        for (player, base) in [
            (PlayerId::One, TilePos::new(16, 16)),
            (PlayerId::Two, TilePos::new(SIZE - 24, SIZE - 24)),
        ] {
            map = map
                .with_building(BuildingKind::TownCenter, player, base)
                .with_building(
                    BuildingKind::Barracks,
                    player,
                    TilePos::new(base.x + 6, base.y),
                )
                .with_mine(TilePos::new(base.x - 4, base.y + 6), Mine::STARTING_GOLD);
            for i in 0..3 {
                map = map.with_unit(
                    UnitKind::Villager,
                    player,
                    TilePos::new(base.x + i, base.y + 5),
                );
            }
        }

        // A band of rock across the middle, with two gaps, so that pathfinding
        // has something to route around on the way between the two bases.
        let wall = (0..SIZE)
            .filter(|x| !(40..48).contains(x) && !(80..88).contains(x))
            .map(|x| TilePos::new(x, SIZE / 2));
        map.with_blocked(wall)
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
        mine: MineId,
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
}

/// The whole simulation state.
pub struct World {
    tick: u64,
    rng: Rng,
    terrain: Grid<Tile>,
    units: SlotMap<UnitId, Unit>,
    buildings: SlotMap<BuildingId, Building>,
    mines: SlotMap<MineId, Mine>,
    players: [Player; 2],
    outcome: Option<PlayerId>,
    spatial: SpatialHash,
    flow_fields: FlowFieldCache,
    path_queue: VecDeque<PathRequest>,
    /// Scratch buffer for the movement pass, kept to avoid a per-tick
    /// allocation on the hottest loop in the game.
    scratch_ids: Vec<UnitId>,
}

impl World {
    /// Single-unit paths solved per tick.
    ///
    /// A budget rather than solving every request immediately, so that a
    /// hundred separate orders in one tick cannot spike past the tick budget.
    pub const PATH_BUDGET_PER_TICK: usize = 16;
    /// How close a unit must be to a target to interact with it, in tiles,
    /// measured from the target's edge.
    pub const INTERACT_RANGE: Fx = Fx::from_frac(3, 2);

    pub fn new(seed: u64, map: MapSpec) -> World {
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
            mines: SlotMap::with_key(),
            players: [
                Player {
                    gold: map.starting_gold,
                },
                Player {
                    gold: map.starting_gold,
                },
            ],
            outcome: None,
            spatial: SpatialHash::new(map.width, map.height, SpatialHash::DEFAULT_CELL_SIZE),
            flow_fields: FlowFieldCache::new(FlowFieldCache::DEFAULT_CAPACITY),
            path_queue: VecDeque::new(),
            scratch_ids: Vec::new(),
        };

        for (kind, owner, origin) in map.buildings {
            let building = Building::new(kind, owner, origin);
            world.stamp(building.footprint(), Tile::BLOCKED);
            world.buildings.insert(building);
        }
        for (origin, gold) in map.mines {
            let mine = Mine::new(origin, gold);
            world.stamp(mine.footprint(), Tile::BLOCKED);
            world.mines.insert(mine);
        }
        for (kind, owner, at) in map.units {
            world
                .units
                .insert(Unit::new(kind, owner, FxVec2::tile_center(at)));
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
    pub const fn outcome(&self) -> Option<PlayerId> {
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
        for (_, unit) in &self.units {
            eat(unit.pos.x.to_bits() as u32 as u64);
            eat(unit.pos.y.to_bits() as u32 as u64);
            eat(unit.hp as u32 as u64);
            eat(unit.carrying as u32 as u64);
            eat(unit.owner.index() as u64);
        }
        for (_, building) in &self.buildings {
            eat(building.hp as u32 as u64);
            eat(building.queue.len() as u64);
        }
        for (_, mine) in &self.mines {
            eat(mine.remaining as u32 as u64);
        }
        for player in &self.players {
            eat(player.gold as u32 as u64);
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
                    mine,
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
            }
        }
    }

    /// Filters an addressed unit list down to the ones that exist and belong
    /// to the issuing player.
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
            if self.units[id].kind.stats().damage == 0 {
                continue;
            }
            self.units[id].order = Order::Attack {
                target,
                route: Route::Arrived,
            };
            self.units[id].stuck_ticks = 0;
        }
    }

    fn order_gather(&mut self, player: PlayerId, units: &[UnitId], mine: MineId) {
        let Some(target) = self.mines.get(mine) else {
            return;
        };
        if target.remaining <= 0 {
            return;
        }
        let goal = target.origin;
        let ids: Vec<UnitId> = self
            .owned(player, units)
            .into_iter()
            .filter(|id| self.units[*id].kind == UnitKind::Villager)
            .collect();
        let route = self.plan_adjacent(&ids, goal, Mine::SIZE);
        for id in ids {
            self.units[id].order = Order::Gather {
                mine,
                phase: GatherPhase::ToMine,
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
            || !target.kind.trains(unit)
            || target.queue.len() >= Building::QUEUE_LIMIT
        {
            return;
        }
        let cost = unit.cost();
        if self.players[player.index()].gold < cost {
            return;
        }
        // Charged on enqueue rather than on completion, so that queueing five
        // units cannot spend the same gold five times.
        self.players[player.index()].gold -= cost;
        let target = &mut self.buildings[building];
        if target.queue.is_empty() {
            target.train_ticks = unit.train_ticks();
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
            EntityId::Mine(_) => None,
        }
    }

    fn position_of(&self, target: EntityId) -> Option<FxVec2> {
        match target {
            EntityId::Unit(id) => self.units.get(id).map(|u| u.pos),
            EntityId::Building(id) => self.buildings.get(id).map(Building::center),
            EntityId::Mine(id) => self.mines.get(id).map(Mine::center),
        }
    }

    /// Half the width of a target, so that reach can be measured to its edge.
    ///
    /// Without this a soldier would have to stand inside a 4x4 town centre to
    /// hit it, which the terrain it stamps makes impossible.
    fn radius_of(&self, target: EntityId) -> Option<Fx> {
        match target {
            EntityId::Unit(id) => self.units.get(id).map(|u| u.kind.stats().radius),
            EntityId::Building(id) => self
                .buildings
                .get(id)
                .map(|b| Fx::from_frac(b.kind.size(), 2)),
            EntityId::Mine(id) => self.mines.get(id).map(|_| Fx::from_frac(Mine::SIZE, 2)),
        }
    }

    /// How close an attacker has to be to `target` to land a hit.
    fn reach(&self, attacker: UnitId, target: EntityId) -> Option<Fx> {
        Some(self.units[attacker].kind.stats().range + self.radius_of(target)?)
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
            let (origin, size, owner) = (building.origin, building.kind.size(), building.owner);
            // No room to stand means the unit waits rather than being lost:
            // deleting a paid-for unit because the base is crowded would be a
            // silent theft of the player's gold.
            let Some(spawn) = self.free_tile_beside(origin, size) else {
                continue;
            };
            self.units
                .insert(Unit::new(kind, owner, FxVec2::tile_center(spawn)));
            let building = &mut self.buildings[id];
            building.queue.remove(0);
            building.train_ticks = building.queue.first().map_or(0, |next| next.train_ticks());
        }
    }

    // --- Phase 3: gathering -----------------------------------------------

    fn run_gathering(&mut self) {
        let ids: Vec<UnitId> = self.units.keys().collect();
        for id in ids {
            let Order::Gather { mine, phase, .. } = self.units[id].order.clone() else {
                continue;
            };
            match phase {
                GatherPhase::ToMine => self.gather_approach(id, mine),
                GatherPhase::Mining => self.gather_extract(id, mine),
                GatherPhase::ToDropOff { building } => self.gather_deposit(id, mine, building),
            }
        }
    }

    fn gather_approach(&mut self, id: UnitId, mine: MineId) {
        let Some(target) = self.mines.get(mine) else {
            self.units[id].order = Order::Idle;
            return;
        };
        let (center, remaining) = (target.center(), target.remaining);
        if remaining <= 0 {
            // The seam ran out on the way over. Bank whatever is in hand
            // rather than stranding it.
            self.units[id].order = Order::Idle;
            self.send_to_drop_off(id, mine);
            return;
        }
        if self.within(id, center, Mine::SIZE) {
            self.units[id].order = Order::Gather {
                mine,
                phase: GatherPhase::Mining,
                route: Route::Arrived,
            };
        }
    }

    fn gather_extract(&mut self, id: UnitId, mine: MineId) {
        let unit = &mut self.units[id];
        unit.mine_ticks += 1;
        if unit.mine_ticks < Unit::MINE_TICKS_PER_GOLD {
            return;
        }
        unit.mine_ticks = 0;

        let Some(seam) = self.mines.get_mut(mine) else {
            self.units[id].order = Order::Idle;
            return;
        };
        if seam.remaining > 0 {
            seam.remaining -= 1;
            self.units[id].carrying += 1;
        }
        let exhausted = self.mines[mine].remaining <= 0;
        if self.units[id].carrying >= Unit::CARRY_CAPACITY || exhausted {
            self.send_to_drop_off(id, mine);
        }
    }

    fn gather_deposit(&mut self, id: UnitId, mine: MineId, building: BuildingId) {
        let Some(target) = self.buildings.get(building) else {
            // The drop-off was destroyed mid-walk. Look for another one.
            self.send_to_drop_off(id, mine);
            return;
        };
        let (center, size) = (target.center(), target.kind.size());
        if !self.within(id, center, size) {
            return;
        }

        let unit = &mut self.units[id];
        let owner = unit.owner;
        let carried = std::mem::take(&mut unit.carrying);
        self.players[owner.index()].gold += carried;

        // Head back for another load, unless the seam is gone.
        let seam_alive = self.mines.get(mine).is_some_and(|m| m.remaining > 0);
        if seam_alive {
            let origin = self.mines[mine].origin;
            let route = self.plan_adjacent(&[id], origin, Mine::SIZE);
            self.units[id].order = Order::Gather {
                mine,
                phase: GatherPhase::ToMine,
                route,
            };
        } else {
            self.units[id].order = Order::Idle;
        }
    }

    /// Sends a loaded villager to the nearest drop-off it owns.
    fn send_to_drop_off(&mut self, id: UnitId, mine: MineId) {
        let unit = &self.units[id];
        if unit.carrying == 0 {
            self.units[id].order = Order::Idle;
            return;
        }
        let (pos, owner) = (unit.pos, unit.owner);
        let nearest = self
            .buildings
            .iter()
            .filter(|(_, b)| b.owner == owner && b.kind.is_drop_off())
            .min_by_key(|(_, b)| (b.center() - pos).length_squared())
            .map(|(id, b)| (id, b.origin, b.kind.size()));

        let Some((building, origin, size)) = nearest else {
            // Nowhere to bank it. The gold stays carried, which keeps the
            // conservation invariant true, and the villager idles.
            self.units[id].order = Order::Idle;
            return;
        };
        let route = self.plan_adjacent(&[id], origin, size);
        self.units[id].order = Order::Gather {
            mine,
            phase: GatherPhase::ToDropOff { building },
            route,
        };
    }

    /// Whether a unit is close enough to interact with a `size`-tile entity
    /// centred at `center`.
    fn within(&self, id: UnitId, center: FxVec2, size: i32) -> bool {
        let reach = Fx::from_frac(size, 2) + World::INTERACT_RANGE;
        let limit = reach.to_bits() as i64;
        (self.units[id].pos - center).length_squared() <= limit * limit
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
        let speed = kind.stats().speed;

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
        let moved = self.slide(pos, steering, kind.stats().radius);
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
            let reach = self.reach(id, target)?;
            let offset = target_pos - pos;
            let limit = reach.to_bits() as i64;
            return (offset.length_squared() > limit * limit).then(|| offset.with_length(Fx::ONE));
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
        let (pos, radius) = (unit.pos, unit.kind.stats().radius);
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
            let Some(target_pos) = self.position_of(target) else {
                // The victim died, possibly to someone else. Stop swinging.
                self.units[id].order = Order::Idle;
                continue;
            };
            let stats = self.units[id].kind.stats();
            let Some(reach) = self.reach(id, target) else {
                continue;
            };
            let reach = reach.to_bits() as i64;
            if (target_pos - self.units[id].pos).length_squared() > reach * reach {
                continue;
            }
            if self.units[id].attack_cooldown > 0 {
                continue;
            }
            self.units[id].attack_cooldown = stats.attack_cooldown;
            self.damage(target, stats.damage);
        }
    }

    fn damage(&mut self, target: EntityId, amount: i32) {
        match target {
            EntityId::Unit(id) => {
                if let Some(unit) = self.units.get_mut(id) {
                    unit.hp -= amount;
                }
            }
            EntityId::Building(id) => {
                if let Some(building) = self.buildings.get_mut(id) {
                    building.hp -= amount;
                }
            }
            // Mines cannot be attacked; `order_attack` rejects them, and a
            // stray one here is a no-op rather than a panic.
            EntityId::Mine(_) => {}
        }
    }

    // --- Phase 7: cleanup --------------------------------------------------

    fn run_cleanup(&mut self) {
        self.units.retain(|_, unit| unit.hp > 0);

        let dead: Vec<BuildingId> = self
            .buildings
            .iter()
            .filter(|(_, b)| b.hp <= 0)
            .map(|(id, _)| id)
            .collect();
        if dead.is_empty() {
            return;
        }
        for id in dead {
            let footprint: Vec<TilePos> = self.buildings[id].footprint().collect();
            self.buildings.remove(id);
            self.stamp(footprint.into_iter(), Tile::OPEN);
        }
        // Terrain changed, so every cached field may now route around a wall
        // that is no longer there.
        self.flow_fields.clear();
    }

    // --- Phase 8: win check ------------------------------------------------

    /// Sets the outcome the first time a player has no buildings left.
    ///
    /// The simulation keeps ticking afterwards; announcing the result is the
    /// application's concern.
    fn check_win(&mut self) {
        if self.outcome.is_some() {
            return;
        }
        for player in PlayerId::ALL {
            let alive = self.buildings.values().any(|b| b.owner == player);
            if !alive {
                self.outcome = Some(player.other());
                return;
            }
        }
    }
}

impl Order {
    fn route(&self) -> Option<&Route> {
        match self {
            Order::Idle => None,
            Order::Move { route, .. }
            | Order::Attack { route, .. }
            | Order::Gather { route, .. } => Some(route),
        }
    }

    fn set_route(&mut self, new_route: Route) {
        match self {
            Order::Idle => {}
            Order::Move { route, .. }
            | Order::Attack { route, .. }
            | Order::Gather { route, .. } => {
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

    pub const fn outcome(&self) -> Option<PlayerId> {
        self.world.outcome
    }

    /// The tile at `pos`, or `None` if it is off the map.
    pub fn tile(&self, pos: TilePos) -> Option<Tile> {
        self.world.terrain.get(pos).copied()
    }

    pub fn gold(&self, player: PlayerId) -> i32 {
        self.world.players[player.index()].gold
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

    pub fn mines(&self) -> impl Iterator<Item = (MineId, &'a Mine)> {
        self.world.mines.iter()
    }

    pub fn mine(&self, id: MineId) -> Option<&'a Mine> {
        self.world.mines.get(id)
    }

    /// Total gold still in the ground plus gold in transit plus gold banked.
    ///
    /// Exposed for the conservation property test, which is the only cheap
    /// defence against a resource duplication bug.
    pub fn total_gold(&self) -> i64 {
        let banked: i64 = self.world.players.iter().map(|p| i64::from(p.gold)).sum();
        let carried: i64 = self
            .world
            .units
            .values()
            .map(|u| i64::from(u.carrying))
            .sum();
        let buried: i64 = self
            .world
            .mines
            .values()
            .map(|m| i64::from(m.remaining))
            .sum();
        banked + carried + buried
    }

    /// Unit positions, for the renderer to interpolate between ticks.
    pub fn snapshot(&self) -> Vec<(UnitId, FxVec2)> {
        self.world.units.iter().map(|(id, u)| (id, u.pos)).collect()
    }
}
