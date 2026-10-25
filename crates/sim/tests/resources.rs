//! Four resources: a site holds one, a villager carries one at a time, and
//! banking adds it to the matching stock.

use sim::{
    BuildingKind, Command, Content, MapSpec, PlayerId, Resource, Site, TilePos, UnitId, UnitKind,
    World,
};
use smallvec::smallvec;

/// A town centre for each player, a site of `resource` beside player one's,
/// and a villager next to the site.
fn world_with(resource: Resource, content: Content) -> (World, UnitId, sim::SiteId) {
    let map = MapSpec::new(24, 24)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(18, 18),
        )
        .with_site(resource, TilePos::new(9, 3), 300)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 3));
    let world = World::with_content(1, map, content);
    let (villager, _) = world.view().units().next().expect("the villager");
    let (site, _) = world.view().sites().next().expect("the site");
    (world, villager, site)
}

fn gather(world: &mut World, villager: UnitId, site: sim::SiteId) {
    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        site,
    }]);
}

#[test]
fn a_site_holds_the_resource_and_amount_the_map_gives_it() {
    let (world, _, _) = world_with(Resource::Stone, Content::default());

    let (_, site) = world.view().sites().next().expect("the site");

    assert_eq!((site.resource, site.remaining), (Resource::Stone, 300));
}

#[test]
fn a_site_is_one_tile_as_the_originals_mines_bushes_and_trees_are() {
    assert_eq!(Site::SIZE, 1);
}

#[test]
fn each_resource_is_gathered_and_banked_into_its_own_stock() {
    for resource in Resource::ALL {
        let (mut world, villager, site) = world_with(resource, Content::default());
        gather(&mut world, villager, site);
        for _ in 0..3000 {
            world.step(&[]);
        }

        let stock = world.view().resources(PlayerId::ONE);
        assert!(stock.get(resource) > 0, "no {resource:?} banked");
        for other in Resource::ALL.into_iter().filter(|r| *r != resource) {
            // This map starts every player with nothing.
            assert_eq!(
                stock.get(other),
                0,
                "{other:?} banked from a {resource:?} site"
            );
        }
    }
}

#[test]
fn a_villager_carries_the_resource_of_the_site_it_works() {
    let (mut world, villager, site) = world_with(Resource::Wood, Content::default());
    gather(&mut world, villager, site);
    for _ in 0..400 {
        if world.view().unit(villager).is_some_and(|u| u.carrying > 0) {
            break;
        }
        world.step(&[]);
    }

    let unit = world.view().unit(villager).expect("the villager");
    assert!(unit.carrying > 0);
    assert_eq!(unit.carried, Resource::Wood);
}

#[test]
fn each_resource_is_gathered_at_its_own_rate_from_content() {
    // Stone at one unit a tick, everything else at the default: the stone
    // villager fills up long before a gold one would.
    let mut content = Content::default();
    content.gather.get_mut(Resource::Stone).ticks_per_unit = 1;
    let (mut world, villager, site) = world_with(Resource::Stone, content);
    gather(&mut world, villager, site);
    for _ in 0..40 {
        world.step(&[]);
    }

    let carried = world.view().unit(villager).expect("the villager").carrying;
    let default = Content::default()
        .gather
        .get(Resource::Stone)
        .ticks_per_unit;
    assert!(
        carried >= 10 || world.view().resources(PlayerId::ONE).stone > 0,
        "carried {carried} at one a tick (the default is one per {default})"
    );
}

#[test]
fn each_resource_has_its_own_capacity_from_content() {
    let mut content = Content::default();
    content.gather.get_mut(Resource::Food).carry_capacity = 3;
    let (mut world, villager, site) = world_with(Resource::Food, content);
    gather(&mut world, villager, site);

    let mut most = 0;
    for _ in 0..2000 {
        world.step(&[]);
        most = most.max(world.view().unit(villager).expect("the villager").carrying);
    }

    assert_eq!(most, 3);
}

#[test]
fn a_villager_sent_to_another_resource_drops_what_it_carries() {
    // As in the original: a villager carrying gold that is sent to chop wood
    // loses the gold.
    let map = MapSpec::new(24, 24)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(18, 18),
        )
        .with_site(Resource::Gold, TilePos::new(9, 3), 300)
        .with_site(Resource::Wood, TilePos::new(9, 8), 300)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 3));
    let mut world = World::new(1, map);
    let (villager, _) = world.view().units().next().expect("the villager");
    let site = |resource| {
        world
            .view()
            .sites()
            .find(|(_, s)| s.resource == resource)
            .map(|(id, _)| id)
            .expect("the site")
    };
    let (gold, wood) = (site(Resource::Gold), site(Resource::Wood));
    gather(&mut world, villager, gold);
    while world.view().unit(villager).expect("the villager").carrying < 3 {
        world.step(&[]);
    }

    gather(&mut world, villager, wood);

    let unit = world.view().unit(villager).expect("the villager");
    assert_eq!(unit.carrying, 0, "still carrying gold");
}

#[test]
fn a_villager_sent_to_more_of_the_same_resource_keeps_its_load() {
    let map = MapSpec::new(24, 24)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(18, 18),
        )
        .with_site(Resource::Gold, TilePos::new(9, 3), 300)
        .with_site(Resource::Gold, TilePos::new(9, 8), 300)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 3));
    let mut world = World::new(1, map);
    let (villager, _) = world.view().units().next().expect("the villager");
    let sites: Vec<sim::SiteId> = world.view().sites().map(|(id, _)| id).collect();
    gather(&mut world, villager, sites[0]);
    while world.view().unit(villager).expect("the villager").carrying < 3 {
        world.step(&[]);
    }

    gather(&mut world, villager, sites[1]);

    assert_eq!(
        world.view().unit(villager).expect("the villager").carrying,
        3
    );
}

#[test]
fn no_resource_is_ever_created_or_destroyed_by_gathering() {
    // For each resource: what is banked, carried, and left in the site
    // always adds up to what the site started with.
    for resource in Resource::ALL {
        let (mut world, villager, site) = world_with(resource, Content::default());
        let start = world.view().total(resource);
        gather(&mut world, villager, site);
        for tick in 0..3000 {
            world.step(&[]);
            assert_eq!(
                world.view().total(resource),
                start,
                "{resource:?} appeared or vanished by tick {tick}"
            );
        }
    }
}

#[test]
fn the_checksum_tells_a_gold_site_from_a_stone_one() {
    // A replay that drifted on what a site holds must not verify.
    let (gold, _, _) = world_with(Resource::Gold, Content::default());
    let (stone, _, _) = world_with(Resource::Stone, Content::default());

    assert_ne!(gold.checksum(), stone.checksum());
}
