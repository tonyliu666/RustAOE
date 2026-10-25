//! Converting a whole `.dat`, built in the test, to the game's content.

use assets::dat::{Civ, Combat, Cost, Creatable, Dat, Graphic, GraphicDelta, Task, Unit};
use datconv::{convert, ConvertError, Selection};

fn graphic(id: i16, slp: i32, deltas: usize) -> Option<Graphic> {
    Some(Graphic {
        id,
        name: format!("G{id}"),
        slp,
        frame_count: 1,
        angle_count: 1,
        deltas: (0..deltas)
            .map(|_| GraphicDelta {
                graphic_id: 0,
                offset_x: 0,
                offset_y: 0,
            })
            .collect(),
    })
}

fn part(resource: i16, amount: i16, used: bool) -> Cost {
    Cost {
        resource,
        amount,
        used,
    }
}

fn unit(kind: u8, id: i16, name: &str, class: i16) -> Unit {
    Unit {
        kind,
        id,
        name: name.to_string(),
        class,
        standing_graphic: 1,
        hit_points: 25,
        collision: [0.25, 0.25, 2.0],
        clearance: [0.25, 0.25],
        icon: 15,
        resource_capacity: 10,
        speed: Some(2.5),
        work_rate: Some(1.0),
        drop_sites: Some([-1, -1]),
        combat: Some(Combat {
            attacks: vec![(4, 3)],
            max_range: 0.0,
            reload_time: 2.0,
        }),
        creatable: Some(Creatable {
            costs: [part(0, 50, true), part(-1, 0, false), part(4, 1, false)],
            train_time: 25,
            train_location: 109,
            button: 1,
        }),
    }
}

/// A file with one civilisation (after Gaia) holding the four types the
/// game has, a gold miner, a spearman, and a scout with no sprite.
fn dat() -> Dat {
    let mut units: Vec<Option<Unit>> = vec![None; 600];
    let mut villager = unit(70, 83, "VMBAS", 4);
    villager.drop_sites = Some([109, -1]);
    units[83] = Some(villager);
    let mut militia = unit(70, 74, "SPRMN", 6);
    militia.hit_points = 40;
    militia.standing_graphic = 2;
    militia.icon = 8;
    militia.speed = Some(1.0);
    militia.combat = Some(Combat {
        attacks: vec![(4, 4), (29, 0)],
        max_range: 0.0,
        reload_time: 2.0,
    });
    militia.creatable = Some(Creatable {
        costs: [part(0, 60, true), part(3, 20, true), part(4, 1, false)],
        train_time: 21,
        train_location: 12,
        button: 2,
    });
    // A unit that does not gather lists a drop site: that alone must not
    // make the barracks one.
    militia.drop_sites = Some([12, -1]);
    units[74] = Some(militia.clone());
    let mut spearman = militia.clone();
    spearman.id = 93;
    spearman.name = "PKEMN".to_string();
    // The spearman's button comes first although its ID is higher.
    spearman.creatable.as_mut().expect("trained").button = 1;
    units[93] = Some(spearman);
    let mut scout = militia.clone();
    scout.id = 448;
    scout.name = "SCOUT".to_string();
    scout.standing_graphic = 9; // no such graphic
    units[448] = Some(scout);
    let mut miner = unit(70, 579, "VMGLD", 4);
    miner.work_rate = Some(0.379);
    miner.drop_sites = Some([109, -1]);
    units[579] = Some(miner);
    let mut town_centre = unit(80, 109, "RTWC", 3);
    town_centre.hit_points = 2400;
    town_centre.standing_graphic = 3;
    town_centre.icon = 28;
    town_centre.collision = [2.0, 2.0, 0.0];
    town_centre.speed = Some(0.0);
    units[109] = Some(town_centre);
    let mut barracks = unit(80, 12, "BRKS", 3);
    barracks.hit_points = 1200;
    barracks.standing_graphic = 4;
    barracks.icon = 2;
    barracks.collision = [1.5, 1.5, 0.0];
    barracks.speed = Some(0.0);
    units[12] = Some(barracks);

    let mut unit_tasks: Vec<Option<Vec<Task>>> = vec![None; 600];
    unit_tasks[579] = Some(vec![Task {
        action: 5,
        class: -1,
        unit: 66,
        resource_in: 3,
        resource_out: -1,
        work_value: 1.0,
        work_range: 0.0,
    }]);
    Dat {
        graphics: vec![
            None,
            graphic(1, 1479, 0),
            graphic(2, 993, 0),
            graphic(3, 3596, 4),
            graphic(4, 2683, 0),
        ],
        after_graphics: 0,
        random_maps_at: 0,
        random_maps: 0,
        effects_at: 0,
        effects: 0,
        unit_headers_at: 0,
        unit_headers: 600,
        units_present: 0,
        unit_tasks,
        civs_at: 0,
        civs: vec![
            Civ {
                name: "Gaia".to_string(),
                units: vec![None; 600],
            },
            Civ {
                name: "British".to_string(),
                units,
            },
        ],
        researches_at: 0,
        researches: 0,
    }
}

#[test]
fn the_default_conversion_writes_the_four_types_the_game_has_under_its_names() {
    let converted = convert(&dat(), 1, Selection::Default).expect("converts");

    let content = app::parse_content(&converted.ron).expect("loads and validates");

    let units: Vec<&str> = content.units.iter().map(|u| u.name.as_str()).collect();
    let buildings: Vec<&str> = content.buildings.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(units, ["villager", "soldier"]);
    assert_eq!(buildings, ["town_center", "barracks"]);
}

#[test]
fn each_field_is_converted_by_its_rule() {
    let converted = convert(&dat(), 1, Selection::Default).expect("converts");
    let content = app::parse_content(&converted.ron).expect("loads and validates");
    let villager = &content.units[0];
    let soldier = &content.units[1];

    assert_eq!(
        (villager.max_hp, villager.sprite, villager.icon),
        (25, 1479, 15)
    );
    assert_eq!(villager.train_ticks, 500);
    assert_eq!(villager.cost.food, 50);
    assert_eq!(villager.speed, sim::Fx::from_frac(1, 8));
    assert_eq!((villager.damage, villager.attack_cooldown), (3, 40));
    assert_eq!(villager.radius, sim::Fx::from_frac(1, 4));
    assert_eq!((soldier.max_hp, soldier.sprite, soldier.icon), (40, 993, 8));
    assert_eq!((soldier.cost.food, soldier.cost.gold), (60, 20));
    assert_eq!((soldier.damage, soldier.train_ticks), (4, 420));
    let town_centre = &content.buildings[0];
    assert_eq!(
        (
            town_centre.max_hp,
            town_centre.sprite,
            town_centre.icon,
            town_centre.size
        ),
        (2400, 3596, 28, 4)
    );
    assert_eq!(content.buildings[1].size, 3);
}

#[test]
fn only_the_units_class_that_gathers_gathers() {
    // The villager has no gather task of its own; the gold miner, a unit of
    // the same class it turns into to work, does.
    let content = app::parse_content(
        &convert(&dat(), 1, Selection::Default)
            .expect("converts")
            .ron,
    )
    .expect("loads");

    assert!(content.units[0].gathers, "villager");
    assert!(!content.units[1].gathers, "soldier");
}

#[test]
fn a_building_a_gatherer_drops_at_is_a_drop_off() {
    let content = app::parse_content(
        &convert(&dat(), 1, Selection::Default)
            .expect("converts")
            .ron,
    )
    .expect("loads");

    assert!(content.buildings[0].drop_off, "town centre");
    assert!(!content.buildings[1].drop_off, "barracks");
}

#[test]
fn a_building_trains_the_converted_units_whose_train_location_it_is() {
    let content = app::parse_content(
        &convert(&dat(), 1, Selection::Default)
            .expect("converts")
            .ron,
    )
    .expect("loads");

    let trains = |b: usize| -> Vec<&str> {
        content.buildings[b]
            .trains
            .iter()
            .map(|k| content.unit(*k).name.as_str())
            .collect()
    };
    assert_eq!(trains(0), ["villager"]);
    assert_eq!(trains(1), ["soldier"]);
}

#[test]
fn gathering_comes_from_the_gold_miner() {
    let content = app::parse_content(
        &convert(&dat(), 1, Selection::Default)
            .expect("converts")
            .ron,
    )
    .expect("loads");

    assert_eq!(content.gather.carry_capacity, 10);
    assert_eq!(content.gather.ticks_per_gold, 53);
}

#[test]
fn each_entry_names_its_original_unit_id() {
    let converted = convert(&dat(), 1, Selection::Default).expect("converts");

    for (name, id) in [
        ("villager", 83),
        ("soldier", 74),
        ("town_center", 109),
        ("barracks", 12),
    ] {
        let line = converted
            .ron
            .lines()
            .find(|l| l.contains(&format!("name: \"{name}\"")))
            .unwrap_or_else(|| panic!("{name} is written"));
        assert!(line.contains(&format!("// unit {id}")), "{line}");
    }
}

#[test]
fn every_value_that_was_rounded_is_reported() {
    let converted = convert(&dat(), 1, Selection::Default).expect("converts");

    // The soldier's 1 tile a second is 3276.8 in 65536ths of a tile a tick;
    // the gold miner's rate gives 52.77 ticks a gold.
    assert!(
        converted.report.iter().any(|l| l.contains("soldier.speed")),
        "{:#?}",
        converted.report
    );
    assert!(converted
        .report
        .iter()
        .any(|l| l.contains("gather.ticks_per_gold")));
    assert!(!converted
        .report
        .iter()
        .any(|l| l.contains("villager.speed")));
}

#[test]
fn a_graphic_drawn_in_pieces_is_reported() {
    let converted = convert(&dat(), 1, Selection::Default).expect("converts");

    assert!(
        converted
            .report
            .iter()
            .any(|l| l.contains("town_center") && l.contains("4 more pieces")),
        "{:#?}",
        converted.report
    );
}

#[test]
fn every_unit_and_building_can_be_converted_with_unique_names() {
    let converted = convert(&dat(), 1, Selection::All).expect("converts");

    let content = app::parse_content(&converted.ron).expect("loads and validates");
    let units: Vec<&str> = content.units.iter().map(|u| u.name.as_str()).collect();
    assert_eq!(units, ["sprmn_74", "vmbas_83", "pkemn_93", "vmgld_579"]);
    let barracks = content
        .buildings
        .iter()
        .find(|b| b.name == "brks_12")
        .expect("the barracks");
    // In button order.
    let trains: Vec<&str> = barracks
        .trains
        .iter()
        .map(|k| content.unit(*k).name.as_str())
        .collect();
    assert_eq!(trains, ["pkemn_93", "sprmn_74"]);
}

#[test]
fn a_unit_that_cannot_be_converted_is_left_out_and_reported() {
    let converted = convert(&dat(), 1, Selection::All).expect("converts");

    assert!(!converted.ron.contains("scout_448"));
    assert!(
        converted
            .report
            .iter()
            .any(|l| l.contains("scout_448") && l.contains("left out")),
        "{:#?}",
        converted.report
    );
}

#[test]
fn a_missing_unit_the_game_needs_is_an_error_naming_it() {
    let mut dat = dat();
    dat.civs[1].units[74] = None;

    assert_eq!(
        convert(&dat, 1, Selection::Default).err(),
        Some(ConvertError::MissingUnit {
            id: 74,
            name: "soldier"
        })
    );
}

#[test]
fn a_civilisation_the_file_does_not_have_is_an_error() {
    assert_eq!(
        convert(&dat(), 7, Selection::Default).err(),
        Some(ConvertError::NoCiv(7))
    );
}
