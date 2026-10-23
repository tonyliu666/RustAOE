//! Pathfinding: A*, flow fields, and the cache in front of them.

use sim::{astar, Dir8, FlowField, FlowFieldCache, Grid, Tile, TilePos};

/// An open grid with the given tiles walled off.
fn grid(width: i32, height: i32, blocked: &[(i32, i32)]) -> Grid<Tile> {
    let mut grid = Grid::new(width, height, Tile::OPEN);
    for (x, y) in blocked {
        grid.set(TilePos::new(*x, *y), Tile::BLOCKED);
    }
    grid
}

/// A full-height wall with one gap, which forces a detour.
fn wall(width: i32, height: i32, at_x: i32, gap_y: i32) -> Grid<Tile> {
    let blocked: Vec<(i32, i32)> = (0..height)
        .filter(|y| *y != gap_y)
        .map(|y| (at_x, y))
        .collect();
    grid(width, height, &blocked)
}

#[test]
fn a_path_to_the_tile_you_are_standing_on_is_empty() {
    let terrain = grid(8, 8, &[]);
    let here = TilePos::new(3, 3);

    assert_eq!(astar(&terrain, here, here), Some(Vec::new()));
}

#[test]
fn a_path_across_open_ground_walks_the_diagonal() {
    let terrain = grid(8, 8, &[]);

    let path = astar(&terrain, TilePos::new(0, 0), TilePos::new(3, 3)).expect("open ground");

    assert_eq!(
        path,
        vec![TilePos::new(1, 1), TilePos::new(2, 2), TilePos::new(3, 3)]
    );
}

#[test]
fn a_path_never_crosses_a_blocked_tile() {
    let terrain = wall(16, 16, 8, 12);

    let path = astar(&terrain, TilePos::new(2, 2), TilePos::new(14, 2)).expect("the gap is open");

    for step in path {
        assert!(
            terrain.get(step).is_some_and(|t| t.passable),
            "path stepped onto the blocked tile {step:?}"
        );
    }
}

#[test]
fn an_unreachable_goal_has_no_path() {
    // A wall with no gap at all: the two halves are separate worlds.
    let terrain = wall(16, 16, 8, -1);

    assert_eq!(
        astar(&terrain, TilePos::new(2, 2), TilePos::new(14, 2)),
        None
    );
}

#[test]
fn a_blocked_goal_has_no_path() {
    let terrain = grid(8, 8, &[(4, 4)]);

    assert_eq!(
        astar(&terrain, TilePos::new(0, 0), TilePos::new(4, 4)),
        None
    );
}

#[test]
fn a_unit_cannot_squeeze_through_a_diagonal_gap() {
    // Two blocks touching at a corner. Stepping between them would be moving
    // through a seam that has no width.
    let terrain = grid(4, 4, &[(1, 0), (0, 1)]);

    assert_eq!(
        astar(&terrain, TilePos::new(0, 0), TilePos::new(1, 1)),
        None
    );
}

#[test]
fn a_flow_field_points_every_tile_towards_the_goal() {
    let terrain = grid(8, 8, &[]);
    let goal = TilePos::new(7, 7);
    let field = FlowField::build(&terrain, goal);

    // Following the field from any tile has to arrive, and it cannot take
    // longer than the number of tiles on the map.
    for y in 0..8 {
        for x in 0..8 {
            let mut cursor = TilePos::new(x, y);
            let mut steps = 0;
            while cursor != goal {
                let (dx, dy) = field.direction(cursor).offset();
                assert!((dx, dy) != (0, 0), "field dead-ends at {cursor:?}");
                cursor = TilePos::new(cursor.x + dx, cursor.y + dy);
                steps += 1;
                assert!(steps < 64, "field loops starting from ({x}, {y})");
            }
        }
    }
}

#[test]
fn a_flow_field_never_points_into_a_blocked_tile() {
    let terrain = wall(16, 16, 8, 12);
    let field = FlowField::build(&terrain, TilePos::new(14, 2));

    for y in 0..16 {
        for x in 0..16 {
            let from = TilePos::new(x, y);
            let (dx, dy) = field.direction(from).offset();
            if (dx, dy) == (0, 0) {
                continue;
            }
            let to = TilePos::new(x + dx, y + dy);
            assert!(
                terrain.get(to).is_some_and(|t| t.passable),
                "{from:?} points into the blocked tile {to:?}"
            );
        }
    }
}

#[test]
fn a_tile_walled_off_from_the_goal_has_no_cost_and_no_direction() {
    let terrain = wall(16, 16, 8, -1);
    let field = FlowField::build(&terrain, TilePos::new(2, 2));

    let far_side = TilePos::new(14, 2);
    assert_eq!(field.cost(far_side), None);
    assert_eq!(field.direction(far_side), Dir8::None);
}

#[test]
fn a_field_for_a_blocked_goal_is_empty_rather_than_a_panic() {
    let terrain = grid(8, 8, &[(4, 4)]);
    let field = FlowField::build(&terrain, TilePos::new(4, 4));

    assert_eq!(field.cost(TilePos::new(0, 0)), None);
}

#[test]
fn the_cache_reuses_a_field_for_a_goal_it_already_built() {
    let terrain = grid(8, 8, &[]);
    let mut cache = FlowFieldCache::new(4);
    let goal = TilePos::new(1, 1);

    cache.get_or_build(&terrain, goal);
    cache.get_or_build(&terrain, goal);

    assert_eq!(cache.len(), 1);
}

#[test]
fn the_cache_evicts_the_least_recently_used_field() {
    let terrain = grid(8, 8, &[]);
    let mut cache = FlowFieldCache::new(2);
    let (first, second, third) = (TilePos::new(0, 0), TilePos::new(1, 1), TilePos::new(2, 2));

    cache.get_or_build(&terrain, first);
    cache.get_or_build(&terrain, second);
    // Touching the first one makes the second the stale one.
    cache.get_or_build(&terrain, first);
    cache.get_or_build(&terrain, third);

    assert!(cache.peek(first).is_some());
    assert!(cache.peek(third).is_some());
    assert!(
        cache.peek(second).is_none(),
        "the least recently used field survived eviction"
    );
}

#[test]
fn clearing_the_cache_drops_every_field() {
    let terrain = grid(8, 8, &[]);
    let mut cache = FlowFieldCache::new(4);
    cache.get_or_build(&terrain, TilePos::new(1, 1));

    cache.clear();

    assert!(cache.is_empty());
}
