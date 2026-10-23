//! Pathfinding: A* for lone units, flow fields for groups.
//!
//! Two layers with different cost profiles. A flow field is O(map) once and
//! then free per unit, which is what makes a 200-unit order affordable; A* is
//! cheap for one unit and wasteful for a hundred. The choice between them is
//! made by the caller based on how many units share the order.

use crate::{Grid, Tile, TilePos};
use std::collections::{BinaryHeap, VecDeque};

/// One of the eight directions a flow field cell can point, plus "nowhere".
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Dir8 {
    None,
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl Dir8 {
    /// The tile offset this direction steps by.
    pub const fn offset(self) -> (i32, i32) {
        match self {
            Dir8::None => (0, 0),
            Dir8::N => (0, -1),
            Dir8::NE => (1, -1),
            Dir8::E => (1, 0),
            Dir8::SE => (1, 1),
            Dir8::S => (0, 1),
            Dir8::SW => (-1, 1),
            Dir8::W => (-1, 0),
            Dir8::NW => (-1, -1),
        }
    }

    const fn from_offset(dx: i32, dy: i32) -> Dir8 {
        match (dx, dy) {
            (0, -1) => Dir8::N,
            (1, -1) => Dir8::NE,
            (1, 0) => Dir8::E,
            (1, 1) => Dir8::SE,
            (0, 1) => Dir8::S,
            (-1, 1) => Dir8::SW,
            (-1, 0) => Dir8::W,
            (-1, -1) => Dir8::NW,
            _ => Dir8::None,
        }
    }
}

/// Step cost of a straight move. Diagonals cost `DIAGONAL_COST`.
///
/// Integers rather than 1.0 and 1.414 because the whole simulation is integral;
/// 10 and 14 are the standard approximation and their ratio is within 1% of
/// the true one.
const STRAIGHT_COST: u16 = 10;
const DIAGONAL_COST: u16 = 14;
/// Cost written into a cell no expansion has reached.
const UNREACHABLE: u16 = u16::MAX;

/// The eight neighbour offsets, in a fixed order.
///
/// The order is part of the determinism contract: it decides which of two
/// equal-cost paths wins, and changing it invalidates existing replays.
const NEIGHBOURS: [(i32, i32, u16); 8] = [
    (0, -1, STRAIGHT_COST),
    (1, -1, DIAGONAL_COST),
    (1, 0, STRAIGHT_COST),
    (1, 1, DIAGONAL_COST),
    (0, 1, STRAIGHT_COST),
    (-1, 1, DIAGONAL_COST),
    (-1, 0, STRAIGHT_COST),
    (-1, -1, DIAGONAL_COST),
];

/// Whether a diagonal step between two tiles is allowed.
///
/// Both orthogonal neighbours must be open, otherwise a unit would slip
/// through the corner between two diagonally touching buildings.
fn diagonal_open(terrain: &Grid<Tile>, from: TilePos, dx: i32, dy: i32) -> bool {
    if dx == 0 || dy == 0 {
        return true;
    }
    passable(terrain, TilePos::new(from.x + dx, from.y))
        && passable(terrain, TilePos::new(from.x, from.y + dy))
}

fn passable(terrain: &Grid<Tile>, pos: TilePos) -> bool {
    terrain.get(pos).is_some_and(|t| t.passable)
}

/// A Dijkstra field flowing towards one goal tile.
///
/// Every passable tile stores the direction of its cheapest step towards the
/// goal, so any number of units can follow it with a single array read each.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FlowField {
    goal: TilePos,
    width: i32,
    height: i32,
    dir: Vec<Dir8>,
    cost: Vec<u16>,
}

impl FlowField {
    /// Expands outward from `goal` over passable tiles.
    ///
    /// Breadth-first with a bucketed relaxation rather than a binary heap: the
    /// two edge costs make a plain queue with re-relaxation cheaper than
    /// maintaining a heap over 16k tiles.
    pub fn build(terrain: &Grid<Tile>, goal: TilePos) -> FlowField {
        let width = terrain.width();
        let height = terrain.height();
        let len = (width as usize) * (height as usize);
        let mut field = FlowField {
            goal,
            width,
            height,
            dir: vec![Dir8::None; len],
            cost: vec![UNREACHABLE; len],
        };

        if !passable(terrain, goal) {
            return field;
        }

        let goal_index = field.index(goal).expect("goal was just bounds-checked");
        field.cost[goal_index] = 0;

        let mut queue = VecDeque::new();
        queue.push_back(goal);
        while let Some(from) = queue.pop_front() {
            let from_cost = field.cost[field.index(from).expect("queued tile is in bounds")];
            for (dx, dy, step) in NEIGHBOURS {
                let to = TilePos::new(from.x + dx, from.y + dy);
                if !passable(terrain, to) || !diagonal_open(terrain, to, -dx, -dy) {
                    continue;
                }
                let Some(to_index) = field.index(to) else {
                    continue;
                };
                let candidate = from_cost.saturating_add(step);
                if candidate >= field.cost[to_index] {
                    continue;
                }
                field.cost[to_index] = candidate;
                // The stored direction points back the way the expansion came,
                // which from the neighbour's side is the way towards the goal.
                field.dir[to_index] = Dir8::from_offset(-dx, -dy);
                queue.push_back(to);
            }
        }
        field
    }

    pub const fn goal(&self) -> TilePos {
        self.goal
    }

    /// The direction to step from `pos`, or [`Dir8::None`] at the goal and on
    /// tiles the goal cannot be reached from.
    pub fn direction(&self, pos: TilePos) -> Dir8 {
        self.index(pos).map_or(Dir8::None, |i| self.dir[i])
    }

    /// Distance to the goal in step-cost units, or `None` if unreachable.
    pub fn cost(&self, pos: TilePos) -> Option<u16> {
        let cost = self.index(pos).map(|i| self.cost[i])?;
        (cost != UNREACHABLE).then_some(cost)
    }

    fn index(&self, pos: TilePos) -> Option<usize> {
        (pos.x >= 0 && pos.y >= 0 && pos.x < self.width && pos.y < self.height)
            .then(|| (pos.y as usize) * (self.width as usize) + pos.x as usize)
    }
}

/// A small least-recently-used cache of flow fields, keyed by goal tile.
///
/// Repeated orders to the same area are the common case in an RTS, and a
/// rebuild is 16k tile visits. Capacity is a tuning constant: too small and
/// scattered orders thrash it, too large and stale fields survive terrain
/// changes for longer than they should.
#[derive(Clone, Debug)]
pub struct FlowFieldCache {
    /// Most recently used last.
    entries: Vec<FlowField>,
    capacity: usize,
}

impl FlowFieldCache {
    pub const DEFAULT_CAPACITY: usize = 8;

    pub fn new(capacity: usize) -> FlowFieldCache {
        assert!(
            capacity > 0,
            "a zero-capacity cache would rebuild every use"
        );
        FlowFieldCache {
            entries: Vec::new(),
            capacity,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the field for `goal`, building it if it is not cached.
    pub fn get_or_build(&mut self, terrain: &Grid<Tile>, goal: TilePos) -> &FlowField {
        let found = self.entries.iter().position(|f| f.goal == goal);
        let index = match found {
            Some(index) => index,
            None => {
                if self.entries.len() == self.capacity {
                    self.entries.remove(0);
                }
                self.entries.push(FlowField::build(terrain, goal));
                self.entries.len() - 1
            }
        };
        // Move to the back so the front stays the least recently used.
        let field = self.entries.remove(index);
        self.entries.push(field);
        self.entries.last().expect("just pushed")
    }

    /// Looks a field up without building it.
    pub fn peek(&self, goal: TilePos) -> Option<&FlowField> {
        self.entries.iter().find(|f| f.goal == goal)
    }

    /// Drops every cached field.
    ///
    /// Called when terrain changes. Invalidating only the fields whose region
    /// the change touches would be cheaper, but deciding that requires the
    /// same expansion the rebuild does, and a building falls rarely.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// A queued request for a single-unit path.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PathRequest {
    pub unit: crate::UnitId,
    pub goal: TilePos,
}

/// A node waiting in the A* frontier.
///
/// `Ord` is reversed on cost so that [`BinaryHeap`], which is a max-heap, pops
/// the cheapest node. Ties break on tile position rather than being left
/// unspecified: two equal-cost nodes must pop in the same order on every
/// machine or the resulting paths diverge.
#[derive(Copy, Clone, PartialEq, Eq)]
struct Frontier {
    estimate: u32,
    cost: u32,
    pos: TilePos,
}

impl Ord for Frontier {
    fn cmp(&self, other: &Frontier) -> std::cmp::Ordering {
        other
            .estimate
            .cmp(&self.estimate)
            .then_with(|| other.cost.cmp(&self.cost))
            .then_with(|| other.pos.cmp(&self.pos))
    }
}

impl PartialOrd for Frontier {
    fn partial_cmp(&self, other: &Frontier) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Octile distance, the exact cost of an unobstructed diagonal-capable walk.
///
/// Admissible and tight, so A* expands close to the minimum number of nodes.
fn heuristic(from: TilePos, to: TilePos) -> u32 {
    let dx = (from.x - to.x).unsigned_abs();
    let dy = (from.y - to.y).unsigned_abs();
    let (long, short) = if dx > dy { (dx, dy) } else { (dy, dx) };
    (long - short) * u32::from(STRAIGHT_COST) + short * u32::from(DIAGONAL_COST)
}

/// Finds a path from `start` to `goal`, exclusive of `start`.
///
/// Returns `None` if the goal is blocked or unreachable. Never returns a path
/// through a blocked tile.
pub fn astar(terrain: &Grid<Tile>, start: TilePos, goal: TilePos) -> Option<Vec<TilePos>> {
    if !passable(terrain, start) || !passable(terrain, goal) {
        return None;
    }
    if start == goal {
        return Some(Vec::new());
    }

    let width = terrain.width() as usize;
    let len = width * terrain.height() as usize;
    let index = |pos: TilePos| (pos.y as usize) * width + pos.x as usize;

    let mut best = vec![u32::MAX; len];
    let mut came_from = vec![None; len];
    let mut open = BinaryHeap::new();

    best[index(start)] = 0;
    open.push(Frontier {
        estimate: heuristic(start, goal),
        cost: 0,
        pos: start,
    });

    while let Some(node) = open.pop() {
        if node.pos == goal {
            return Some(reconstruct(&came_from, index, start, goal));
        }
        // A stale heap entry: this tile was already reached more cheaply.
        if node.cost > best[index(node.pos)] {
            continue;
        }
        for (dx, dy, step) in NEIGHBOURS {
            let next = TilePos::new(node.pos.x + dx, node.pos.y + dy);
            if !passable(terrain, next) || !diagonal_open(terrain, node.pos, dx, dy) {
                continue;
            }
            let cost = node.cost + u32::from(step);
            let next_index = index(next);
            if cost >= best[next_index] {
                continue;
            }
            best[next_index] = cost;
            came_from[next_index] = Some(node.pos);
            open.push(Frontier {
                estimate: cost + heuristic(next, goal),
                cost,
                pos: next,
            });
        }
    }
    None
}

fn reconstruct(
    came_from: &[Option<TilePos>],
    index: impl Fn(TilePos) -> usize,
    start: TilePos,
    goal: TilePos,
) -> Vec<TilePos> {
    let mut steps = vec![goal];
    let mut cursor = goal;
    while let Some(previous) = came_from[index(cursor)] {
        if previous == start {
            break;
        }
        steps.push(previous);
        cursor = previous;
    }
    steps.reverse();
    steps
}
