//! Tile coordinates and the dense grid they index.

/// A tile coordinate.
///
/// Signed rather than unsigned so that arithmetic near the map edge can go
/// negative and be rejected by a bounds check, instead of wrapping into a
/// large positive value that looks in-bounds.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct TilePos {
    pub x: i32,
    pub y: i32,
}

impl TilePos {
    pub const fn new(x: i32, y: i32) -> TilePos {
        TilePos { x, y }
    }
}

/// A dense row-major grid of `width * height` cells.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Grid<T> {
    width: i32,
    height: i32,
    cells: Vec<T>,
}

impl<T: Clone> Grid<T> {
    /// Builds a grid with every cell set to `fill`.
    pub fn new(width: i32, height: i32, fill: T) -> Grid<T> {
        assert!(width > 0 && height > 0, "grid must have positive extent");
        let len = (width as usize) * (height as usize);
        Grid {
            width,
            height,
            cells: vec![fill; len],
        }
    }
}

impl<T> Grid<T> {
    pub const fn width(&self) -> i32 {
        self.width
    }

    pub const fn height(&self) -> i32 {
        self.height
    }

    /// Reports whether a position addresses a cell in this grid.
    pub const fn in_bounds(&self, pos: TilePos) -> bool {
        pos.x >= 0 && pos.y >= 0 && pos.x < self.width && pos.y < self.height
    }

    /// Reads a cell, or `None` if the position is outside the grid.
    pub fn get(&self, pos: TilePos) -> Option<&T> {
        self.index_of(pos).map(|i| &self.cells[i])
    }

    /// Writes a cell.
    ///
    /// Panics if the position is outside the grid. An out-of-bounds write is
    /// a caller bug, and silently clamping it would corrupt an unrelated tile.
    pub fn set(&mut self, pos: TilePos, value: T) {
        let i = self.index_of(pos).expect("grid write outside bounds");
        self.cells[i] = value;
    }

    fn index_of(&self, pos: TilePos) -> Option<usize> {
        if self.in_bounds(pos) {
            Some((pos.y as usize) * (self.width as usize) + pos.x as usize)
        } else {
            None
        }
    }
}
