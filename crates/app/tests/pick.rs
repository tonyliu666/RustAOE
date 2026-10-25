//! Picking: a click or a drag box selects what the player sees.
//!
//! A unit is drawn as a sprite standing on its position, so most of what the
//! player sees is above that point. Picking tests against the drawn body.

use app::{entity_at, unit_at, units_in_box, Bodies, Body};
use render::Vec2;
use sim::{BuildingKind, EntityId, MapSpec, PlayerId, TilePos, UnitKind, World};

/// A villager-shaped body: half a tile wide, a tile tall, standing on its
/// feet at the bottom centre.
fn tall() -> Bodies {
    let body = Body {
        offset: Vec2::new(-0.25, -1.0),
        size: Vec2::new(0.5, 1.0),
    };
    Bodies::all(body)
}

fn map() -> MapSpec {
    MapSpec::new(16, 16)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(0, 0))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(12, 12),
        )
}

/// A world with one villager of player one standing at the centre of (6, 6),
/// which is (6.5, 6.5).
fn one_villager() -> World {
    World::new(
        1,
        map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(6, 6)),
    )
}

#[test]
fn by_default_a_click_near_the_feet_picks_the_unit() {
    let world = one_villager();

    assert!(unit_at(world.view(), Vec2::new(6.9, 6.5), &Bodies::default(), 0.0).is_some());
    assert!(unit_at(world.view(), Vec2::new(7.5, 6.5), &Bodies::default(), 0.0).is_none());
}

#[test]
fn a_click_on_the_head_picks_the_unit() {
    let world = one_villager();

    // 0.8 tiles above the feet: inside the drawn body, outside the old
    // 0.6-tile circle around the feet.
    assert!(unit_at(world.view(), Vec2::new(6.5, 5.7), &tall(), 0.0).is_some());
}

#[test]
fn a_click_beside_the_body_picks_nothing() {
    let world = one_villager();

    assert!(unit_at(world.view(), Vec2::new(6.9, 6.0), &tall(), 0.0).is_none());
    assert!(unit_at(world.view(), Vec2::new(6.5, 6.8), &tall(), 0.0).is_none());
}

#[test]
fn where_two_bodies_overlap_the_one_drawn_in_front_wins() {
    // The second villager stands half a tile lower, so it is drawn later, on
    // top, and its head covers the first one's feet.
    let world = World::new(
        1,
        map()
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(6, 6))
            .with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(6, 7)),
    );
    let (front, _) = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::TWO)
        .expect("the front villager");

    // Bodies 1.5 tiles tall: the back one covers y 5.0 to 6.5, the front
    // one 6.0 to 7.5, so (6.5, 6.2) is on both.
    let body = Body {
        offset: Vec2::new(-0.25, -1.5),
        size: Vec2::new(0.5, 1.5),
    };
    let taller = Bodies::all(body);
    assert_eq!(
        unit_at(world.view(), Vec2::new(6.5, 6.2), &taller, 0.0),
        Some(front)
    );
}

#[test]
fn a_click_on_a_units_head_over_a_building_picks_the_unit() {
    let world = World::new(
        1,
        map().with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(2, 4)),
    );

    // The villager stands at (2.5, 4.5), just below the town centre, which
    // covers tiles 0 to 3. (2.5, 3.8) is on the villager's head and on the
    // town centre's last row of tiles.
    let picked = entity_at(world.view(), Vec2::new(2.5, 3.8), &Bodies::default(), 0.0);
    assert!(
        matches!(picked, Some(EntityId::Building(_))),
        "default bodies miss the head"
    );
    let picked = entity_at(world.view(), Vec2::new(2.5, 3.8), &tall(), 0.0);
    assert!(
        matches!(picked, Some(EntityId::Unit(_))),
        "picked {picked:?}"
    );
}

#[test]
fn a_drag_box_over_the_heads_selects_the_units() {
    let world = one_villager();

    // The box covers the upper body only, not the feet at (6.5, 6.5).
    let selected = units_in_box(
        world.view(),
        PlayerId::ONE,
        Vec2::new(6.0, 5.6),
        Vec2::new(7.0, 6.0),
        &tall(),
    );

    assert_eq!(selected.len(), 1);
}

#[test]
fn a_drag_box_beside_the_body_selects_nothing() {
    let world = one_villager();

    let selected = units_in_box(
        world.view(),
        PlayerId::ONE,
        Vec2::new(7.0, 5.0),
        Vec2::new(8.0, 7.0),
        &tall(),
    );

    assert!(selected.is_empty());
}

#[test]
fn a_click_on_a_drawn_body_beats_a_nearer_units_enlarged_target() {
    // Two villagers side by side. With targets enlarged to 3 tiles, a click
    // on the left one's body is also inside the right one's target; the body
    // actually clicked wins.
    let world = World::new(
        1,
        map()
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(6, 6))
            .with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(7, 7)),
    );
    let (left, _) = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::ONE)
        .expect("the left villager");

    assert_eq!(
        unit_at(world.view(), Vec2::new(6.5, 6.0), &tall(), 3.0),
        Some(left)
    );
}

#[test]
fn off_every_body_the_nearest_enlarged_target_wins() {
    let world = World::new(
        1,
        map()
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(6, 6))
            .with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(6, 8)),
    );
    let (upper, _) = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::ONE)
        .expect("the upper villager");

    // (7.0, 6.2) touches neither body (x 6.25 to 6.75) but both enlarged
    // targets; the upper villager's body centre (6.5, 6.0) is nearer.
    assert_eq!(
        unit_at(world.view(), Vec2::new(7.0, 6.2), &tall(), 4.0),
        Some(upper)
    );
}
