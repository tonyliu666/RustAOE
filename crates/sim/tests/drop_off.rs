//! A building takes only the resources content says it does, and a
//! villager never delivers a load to one that does not take it.

use sim::{
    BuildingKind, BuildingStats, Command, Content, MapSpec, PlayerId, Resource, TilePos, UnitId,
    UnitKind, World,
};
use smallvec::smallvec;

/// The default content plus a lumber camp, a 2x2 building that takes wood.
fn with_lumber_camp() -> (Content, BuildingKind) {
    let mut content = Content::default();
    content.buildings.push(BuildingStats {
        name: "lumber_camp".to_string(),
        size: 2,
        max_hp: 600,
        drop_off: vec![Resource::Wood],
        trains: Vec::new(),
    });
    let camp = content.building_named("lumber_camp").expect("added");
    (content, camp)
}

/// Player one's town centre at (2, 2), a lumber camp right beside the
/// villager, a site of `resource` beside the camp, and the villager.
fn world(resource: Resource) -> (World, UnitId, sim::SiteId) {
    let (content, camp) = with_lumber_camp();
    let map = MapSpec::new(32, 32)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(28, 28),
        )
        .with_building(camp, PlayerId::ONE, TilePos::new(18, 3))
        .with_site(resource, TilePos::new(21, 3), 300)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(21, 5));
    let world = World::with_content(1, map, content);
    let (villager, _) = world.view().units().next().expect("the villager");
    let (site, _) = world.view().sites().next().expect("the site");
    (world, villager, site)
}

fn camp(world: &World) -> sim::BuildingId {
    world
        .view()
        .buildings()
        .find(|(_, b)| world.view().content().building(b.kind).name == "lumber_camp")
        .map(|(id, _)| id)
        .expect("the lumber camp")
}

fn gather_for(world: &mut World, villager: UnitId, site: sim::SiteId, ticks: u32) {
    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        site,
    }]);
    for _ in 0..ticks {
        world.step(&[]);
    }
}

/// Whether the villager was ever walking to the lumber camp with a load.
fn ever_headed_to_the_camp(world: &mut World, villager: UnitId, ticks: u32) -> bool {
    let camp = camp(world);
    (0..ticks).any(|_| {
        world.step(&[]);
        matches!(
            world.view().unit(villager).expect("the villager").order,
            sim::Order::Gather {
                phase: sim::GatherPhase::ToDropOff { building },
                ..
            } if building == camp
        )
    })
}

#[test]
fn wood_goes_to_the_nearer_lumber_camp() {
    let (mut world, villager, site) = world(Resource::Wood);
    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        site,
    }]);

    assert!(ever_headed_to_the_camp(&mut world, villager, 1500));
    for _ in 0..300 {
        world.step(&[]);
    }
    assert!(world.view().resources(PlayerId::ONE).wood > 0);
}

#[test]
fn gold_walks_past_the_lumber_camp_to_the_town_centre() {
    let (mut world, villager, site) = world(Resource::Gold);
    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        site,
    }]);

    assert!(!ever_headed_to_the_camp(&mut world, villager, 3000));
    assert!(
        world.view().resources(PlayerId::ONE).gold > 0,
        "banked at the town centre"
    );
}

#[test]
fn a_load_cannot_be_dropped_off_by_hand_where_it_is_not_taken() {
    let (mut world, villager, site) = world(Resource::Gold);
    gather_for(&mut world, villager, site, 200);
    assert!(world.view().unit(villager).expect("the villager").carrying > 0);
    let camp = camp(&world);

    world.step(&[Command::Deposit {
        player: PlayerId::ONE,
        units: smallvec![villager],
        building: camp,
    }]);

    assert!(!matches!(
        world.view().unit(villager).expect("the villager").order,
        sim::Order::Gather {
            phase: sim::GatherPhase::ToDropOff { building },
            ..
        } if building == camp
    ));
}

#[test]
fn a_load_nothing_takes_stays_carried_and_the_villager_stands() {
    // Stone, with only a lumber camp and no town centre to take it.
    let (content, camp) = with_lumber_camp();
    let map = MapSpec::new(32, 32)
        .with_building(camp, PlayerId::ONE, TilePos::new(18, 3))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(28, 28),
        )
        .with_site(Resource::Stone, TilePos::new(21, 3), 300)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(21, 5));
    let mut world = World::with_content(1, map, content);
    let (villager, _) = world.view().units().next().expect("the villager");
    let (site, _) = world.view().sites().next().expect("the site");
    let start = world.view().total(Resource::Stone);

    gather_for(&mut world, villager, site, 2000);

    let unit = world.view().unit(villager).expect("the villager");
    assert_eq!(world.view().resources(PlayerId::ONE).stone, 0);
    assert!(unit.carrying > 0, "the load was not kept");
    assert!(matches!(unit.order, sim::Order::Idle));
    assert_eq!(world.view().total(Resource::Stone), start);
}

#[test]
fn the_town_centre_takes_every_resource_and_the_barracks_none() {
    let content = Content::default();

    assert_eq!(
        content.building(BuildingKind::TOWN_CENTER).drop_off,
        Resource::ALL
    );
    assert!(content.building(BuildingKind::BARRACKS).drop_off.is_empty());
}
