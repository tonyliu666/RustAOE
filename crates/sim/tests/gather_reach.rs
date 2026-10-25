//! A villager mines and deposits only when it touches the mine or the
//! drop-off, as it attacks only what it touches: edges within the contact
//! tolerance.

use sim::{BuildingKind, Command, Content, MapSpec, PlayerId, TilePos, UnitKind, World};
use smallvec::smallvec;

/// How far a unit's edge is from a square footprint, in tiles.
fn edge_gap(pos: (f64, f64), radius: f64, origin: TilePos, size: i32) -> f64 {
    let gap = |p: f64, from: i32| {
        let (lo, hi) = (f64::from(from), f64::from(from + size));
        if p < lo {
            lo - p
        } else if p > hi {
            p - hi
        } else {
            0.0
        }
    };
    let (dx, dy) = (gap(pos.0, origin.x), gap(pos.1, origin.y));
    (dx * dx + dy * dy).sqrt() - radius
}

fn tiles(v: sim::FxVec2) -> (f64, f64) {
    (
        f64::from(v.x.to_bits()) / 65536.0,
        f64::from(v.y.to_bits()) / 65536.0,
    )
}

/// A town centre at (2, 2), a mine at (12, 2), and a villager between them,
/// well away from both.
fn world() -> (World, sim::UnitId, sim::SiteId) {
    let map = MapSpec::new(24, 24)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(20, 20),
        )
        .with_mine(TilePos::new(12, 2), 500)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 8));
    let world = World::new(1, map);
    let (villager, _) = world.view().units().next().expect("the villager");
    let (mine, _) = world.view().sites().next().expect("the mine");
    (world, villager, mine)
}

/// The contact tolerance, plus a hair for fixed-point rounding.
fn touching(gap: f64) -> bool {
    gap <= sim::World::CONTACT.to_bits() as f64 / 65536.0 + 1e-4
}

#[test]
fn a_villager_starts_mining_only_once_it_touches_the_mine() {
    let (mut world, villager, mine) = world();
    let radius = f64::from(Content::default().unit(UnitKind::VILLAGER).radius.to_bits()) / 65536.0;
    let origin = world.view().sites().next().expect("mine").1.origin;
    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        site: mine,
    }]);

    // The first gold extracted marks the moment mining began, at most one
    // gold's worth of ticks after the villager arrived.
    let mut at = None;
    for _ in 0..600 {
        world.step(&[]);
        let unit = world.view().unit(villager).expect("the villager");
        if unit.carrying > 0 {
            at = Some(tiles(unit.pos));
            break;
        }
    }
    let at = at.expect("never mined");

    let gap = edge_gap(at, radius, origin, sim::Site::SIZE);
    assert!(touching(gap), "mining {gap:.3} tiles from the mine's edge");
}

#[test]
fn a_villager_deposits_only_once_it_touches_the_drop_off() {
    let (mut world, villager, mine) = world();
    let radius = f64::from(Content::default().unit(UnitKind::VILLAGER).radius.to_bits()) / 65536.0;
    let (_, town_centre) = world
        .view()
        .buildings()
        .find(|(_, b)| b.owner == PlayerId::ONE)
        .map(|(id, b)| (id, (b.origin, b.size)))
        .expect("the town centre");
    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        site: mine,
    }]);

    // Gathering runs before movement within a tick: the deposit is made from
    // where the villager stood when the tick began, and it walks off towards
    // the mine in the same tick. So the place that counts is the one before.
    let mut at = None;
    for _ in 0..2000 {
        let before = tiles(world.view().unit(villager).expect("the villager").pos);
        world.step(&[]);
        if world.view().resources(PlayerId::ONE).gold > 0 {
            at = Some(before);
            break;
        }
    }
    let at = at.expect("never deposited");

    let gap = edge_gap(at, radius, town_centre.0, town_centre.1);
    assert!(
        touching(gap),
        "deposited {gap:.3} tiles from the town centre's edge"
    );
}

#[test]
fn a_villager_still_mines_load_after_load() {
    // With the final approach to touch, the loop of mining, walking back,
    // and depositing still runs on its own.
    let (mut world, villager, mine) = world();
    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        site: mine,
    }]);
    for _ in 0..3000 {
        world.step(&[]);
    }

    let capacity = Content::default().gather.gold.carry_capacity;
    assert!(
        world.view().resources(PlayerId::ONE).gold >= 3 * capacity,
        "banked only {}",
        world.view().resources(PlayerId::ONE).gold
    );
}
