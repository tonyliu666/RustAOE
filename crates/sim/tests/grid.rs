use sim::{Grid, TilePos};

fn pos(x: i32, y: i32) -> TilePos {
    TilePos::new(x, y)
}

#[test]
fn a_new_grid_is_filled_with_the_given_value() {
    let grid = Grid::new(4, 3, 7u8);
    assert_eq!(grid.width(), 4);
    assert_eq!(grid.height(), 3);
    for y in 0..3 {
        for x in 0..4 {
            assert_eq!(grid.get(pos(x, y)), Some(&7));
        }
    }
}

#[test]
fn a_value_written_to_a_tile_can_be_read_back() {
    let mut grid = Grid::new(4, 3, 0u8);
    grid.set(pos(2, 1), 9);
    assert_eq!(grid.get(pos(2, 1)), Some(&9));
}

#[test]
fn writing_one_tile_leaves_every_other_tile_untouched() {
    // A width/height swap in the index calculation passes on square grids and
    // aliases on rectangular ones, so this grid is deliberately not square.
    let mut grid = Grid::new(5, 2, 0u8);
    grid.set(pos(1, 0), 1);
    grid.set(pos(0, 1), 2);
    assert_eq!(grid.get(pos(1, 0)), Some(&1));
    assert_eq!(grid.get(pos(0, 1)), Some(&2));

    let written = [pos(1, 0), pos(0, 1)];
    for y in 0..2 {
        for x in 0..5 {
            if !written.contains(&pos(x, y)) {
                assert_eq!(grid.get(pos(x, y)), Some(&0), "at {x},{y}");
            }
        }
    }
}

#[test]
fn positions_outside_the_grid_read_as_none() {
    let grid = Grid::new(4, 3, 0u8);
    assert_eq!(grid.get(pos(4, 0)), None);
    assert_eq!(grid.get(pos(0, 3)), None);
    assert_eq!(grid.get(pos(-1, 0)), None);
    assert_eq!(grid.get(pos(0, -1)), None);
    assert_eq!(grid.get(pos(i32::MAX, i32::MAX)), None);
}

#[test]
fn in_bounds_agrees_with_get() {
    let grid = Grid::new(4, 3, 0u8);
    for y in -2..6 {
        for x in -2..7 {
            assert_eq!(
                grid.in_bounds(pos(x, y)),
                grid.get(pos(x, y)).is_some(),
                "disagreed at {x},{y}"
            );
        }
    }
}

#[test]
#[should_panic]
fn writing_outside_the_grid_panics_rather_than_corrupting_a_neighbour() {
    let mut grid = Grid::new(4, 3, 0u8);
    grid.set(pos(9, 9), 1);
}
