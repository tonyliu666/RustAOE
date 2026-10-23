//! A uniform grid for neighbour queries.
//!
//! Local avoidance asks "who is near me" once per unit per tick. Answering
//! that by scanning every other unit is the O(n^2) that stops an RTS dead at a
//! few hundred units; a bucketed grid answers it by inspecting nine cells.

use crate::{FxVec2, UnitId};
use smallvec::SmallVec;

/// Units bucketed by position, rebuilt from scratch every tick.
///
/// Rebuilding is cheaper and far simpler than incremental updates, because
/// every unit moves every tick anyway. The cell vectors are cleared rather
/// than reallocated, so a steady-state tick does no allocation at all.
#[derive(Clone, Debug)]
pub struct SpatialHash {
    /// Cell edge length in tiles.
    cell_size: i32,
    columns: i32,
    rows: i32,
    cells: Vec<SmallVec<[UnitId; 8]>>,
}

impl SpatialHash {
    /// Two tiles: wide enough that a query radius under one tile only ever
    /// needs the immediate ring of cells, narrow enough that a cell holds a
    /// handful of units rather than a crowd.
    pub const DEFAULT_CELL_SIZE: i32 = 2;

    pub fn new(map_width: i32, map_height: i32, cell_size: i32) -> SpatialHash {
        assert!(cell_size > 0, "cell size must be positive");
        let columns = ((map_width + cell_size - 1) / cell_size).max(1);
        let rows = ((map_height + cell_size - 1) / cell_size).max(1);
        SpatialHash {
            cell_size,
            columns,
            rows,
            cells: vec![SmallVec::new(); (columns as usize) * (rows as usize)],
        }
    }

    /// Empties every bucket, keeping the allocations.
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            cell.clear();
        }
    }

    /// Files a unit under the cell containing `pos`.
    ///
    /// Positions outside the map are clamped in rather than dropped: a unit
    /// that has somehow left the map still needs to collide with its
    /// neighbours, and silently losing it from the hash would hide the bug.
    pub fn insert(&mut self, id: UnitId, pos: FxVec2) {
        let index = self.cell_index(self.cell_of(pos));
        self.cells[index].push(id);
    }

    /// Calls `visit` for every unit in the nine cells around `pos`.
    ///
    /// Nine cells rather than an exact radius test: the caller filters by
    /// actual distance anyway, and the extra candidates are cheaper to hand
    /// over than to exclude here.
    pub fn for_each_near(&self, pos: FxVec2, mut visit: impl FnMut(UnitId)) {
        let (cx, cy) = self.cell_of(pos);
        for y in (cy - 1).max(0)..=(cy + 1).min(self.rows - 1) {
            for x in (cx - 1).max(0)..=(cx + 1).min(self.columns - 1) {
                for id in &self.cells[(y as usize) * (self.columns as usize) + x as usize] {
                    visit(*id);
                }
            }
        }
    }

    fn cell_of(&self, pos: FxVec2) -> (i32, i32) {
        let tile = pos.tile();
        (
            tile.x.div_euclid(self.cell_size).clamp(0, self.columns - 1),
            tile.y.div_euclid(self.cell_size).clamp(0, self.rows - 1),
        )
    }

    fn cell_index(&self, (x, y): (i32, i32)) -> usize {
        (y as usize) * (self.columns as usize) + x as usize
    }
}
