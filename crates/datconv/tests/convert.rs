//! Converting a whole `.dat`, built in the test, to the game's content.

use assets::dat::{Annex, Civ, Combat, Cost, Creatable, Dat, Graphic, GraphicDelta, Task, Unit};
use datconv::{convert, ConvertError, Install, Selection};

/// A graphic drawing `slp`, with pieces naming other graphics (-1 for this
/// graphic's own SLP).
fn graphic(id: i16, slp: i32, pieces: &[i16]) -> Option<Graphic> {
    let placed: Vec<(i16, i16, i16)> = pieces.iter().map(|&g| (g, 0, 0)).collect();
    placed_graphic(id, slp, &placed)
}

/// [`graphic`] with each piece's pixel offset: (graphic, x, y).
fn placed_graphic(id: i16, slp: i32, pieces: &[(i16, i16, i16)]) -> Option<Graphic> {
    Some(Graphic {
        id,
        name: format!("G{id}"),
        slp,
        frame_count: 1,
        angle_count: 1,
        deltas: pieces
            .iter()
            .map(|&(graphic_id, offset_x, offset_y)| GraphicDelta {
                graphic_id,
                offset_x,
                offset_y,
            })
            .collect(),
    })
}

/// The SLPs of a type's parts, all at one point, as `Part`s.
fn at_one_point(slps: &[u32]) -> Vec<app::Part> {
    slps.iter().map(|&slp| app::Part::new(slp, 0, 0)).collect()
}

/// SLPs the fake install lacks, as the real one lacks the villager's idle
/// pieces and one piece of the town centre.
const NOT_IN_THE_INSTALL: [u32; 3] = [1477, 1478, 890];

/// The fake install: every SLP the fake file names except
/// [`NOT_IN_THE_INSTALL`], and icon sprites as long as the real ones.
fn install() -> Install {
    Install {
        sprites: [1479, 993, 3596, 2683, 889, 4612, 891]
            .into_iter()
            .collect(),
        unit_icons: 134,
        building_icons: 52,
    }
}

/// [`convert`] with the fake install.
fn convert_with(dat: &Dat, selection: Selection) -> Result<datconv::Conversion, ConvertError> {
    convert(dat, 1, selection, &install())
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
        storages: Vec::new(),
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
        annexes: Vec::new(),
    }
}

/// A file with one civilisation (after Gaia) holding the four types the
/// game has, a gold miner, a spearman, and a scout with no sprite.
fn dat() -> Dat {
    let mut units: Vec<Option<Unit>> = vec![None; 700];
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
    units[579] = Some(miner.clone());
    // The other jobs a villager turns into: forager, lumberjack, stone
    // miner, with the original's rates; the lumberjack carries 12 here.
    for (id, name, rate, capacity) in [
        (120, "VMFOR", 0.31, 10),
        (123, "VMLUM", 0.39, 12),
        (124, "VMMIN", 0.36, 10),
    ] {
        let mut job = miner.clone();
        job.id = id;
        job.name = name.to_string();
        job.work_rate = Some(rate);
        job.resource_capacity = capacity;
        units[usize::from(id as u16)] = Some(job);
    }
    let mut town_centre = unit(80, 109, "RTWC", 3);
    town_centre.hit_points = 2400;
    town_centre.standing_graphic = 3;
    town_centre.icon = 28;
    town_centre.collision = [2.0, 2.0, 0.0];
    town_centre.speed = Some(0.0);
    town_centre.annexes = vec![Annex {
        unit: 618,
        offset: [1.0, -1.0],
    }];
    units[109] = Some(town_centre);
    let mut annex = unit(80, 618, "RTWC1A", 3);
    annex.standing_graphic = 15;
    annex.creatable = None;
    units[618] = Some(annex);
    let mut barracks = unit(80, 12, "BRKS", 3);
    barracks.hit_points = 1200;
    barracks.standing_graphic = 4;
    barracks.icon = 2;
    barracks.collision = [1.5, 1.5, 0.0];
    barracks.speed = Some(0.0);
    units[12] = Some(barracks);

    let mut unit_tasks: Vec<Option<Vec<Task>>> = vec![None; 700];
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
            // The villager: two idle pieces the install lacks, then itself.
            graphic(1, 1479, &[10, -1, 11]),
            graphic(2, 993, &[]),
            // The town centre: a shadow, a missing piece, itself, a post.
            graphic(3, 3596, &[12, 13, -1, 14]),
            // The barracks names itself three times.
            graphic(4, 2683, &[-1, -1, -1]),
            None,
            None,
            None,
            None,
            None,
            graphic(10, 1477, &[]),
            graphic(11, 1478, &[]),
            graphic(12, 889, &[]),
            graphic(13, 890, &[]),
            graphic(14, 4612, &[]),
            // The town centre's annex.
            graphic(15, 891, &[]),
        ],
        after_graphics: 0,
        random_maps_at: 0,
        random_maps: 0,
        effects_at: 0,
        effects: 0,
        unit_headers_at: 0,
        unit_headers: 700,
        units_present: 0,
        unit_tasks,
        civs_at: 0,
        civs: vec![
            Civ {
                name: "Gaia".to_string(),
                units: vec![None; 700],
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
    let converted = convert_with(&dat(), Selection::Default).expect("converts");

    let content = app::parse_content(&converted.ron).expect("loads and validates");

    let units: Vec<&str> = content.units.iter().map(|u| u.name.as_str()).collect();
    let buildings: Vec<&str> = content.buildings.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(units, ["villager", "soldier"]);
    assert_eq!(buildings, ["town_center", "barracks"]);
}

#[test]
fn each_field_is_converted_by_its_rule() {
    let converted = convert_with(&dat(), Selection::Default).expect("converts");
    let content = app::parse_content(&converted.ron).expect("loads and validates");
    let looks = app::parse_looks(&converted.ron).expect("loads");
    let villager = &content.units[0];
    let soldier = &content.units[1];
    let unit_look = |name| looks.unit(name).expect("a look");
    let building_look = |name| looks.building(name).expect("a look");

    assert_eq!(villager.max_hp, 25);
    assert_eq!(
        (unit_look("villager").sprite, unit_look("villager").icon),
        (1479, 15)
    );
    assert_eq!(villager.train_ticks, 500);
    assert_eq!(villager.cost.food, 50);
    assert_eq!(villager.speed, sim::Fx::from_frac(1, 8));
    assert_eq!((villager.damage, villager.attack_cooldown), (3, 40));
    assert_eq!(villager.radius, sim::Fx::from_frac(1, 4));
    assert_eq!(soldier.max_hp, 40);
    assert_eq!(
        (unit_look("soldier").sprite, unit_look("soldier").icon),
        (993, 8)
    );
    assert_eq!((soldier.cost.food, soldier.cost.gold), (60, 20));
    assert_eq!((soldier.damage, soldier.train_ticks), (4, 420));
    let town_centre = &content.buildings[0];
    assert_eq!((town_centre.max_hp, town_centre.size), (2400, 4));
    let look = building_look("town_center");
    assert_eq!((look.sprite, look.icon), (3596, 28));
    assert_eq!(content.buildings[1].size, 3);
}

#[test]
fn only_the_units_class_that_gathers_gathers() {
    // The villager has no gather task of its own; the gold miner, a unit of
    // the same class it turns into to work, does.
    let content = app::parse_content(
        &convert_with(&dat(), Selection::Default)
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
        &convert_with(&dat(), Selection::Default)
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
        &convert_with(&dat(), Selection::Default)
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
fn each_resources_gathering_comes_from_its_job_unit() {
    let content = app::parse_content(
        &convert_with(&dat(), Selection::Default)
            .expect("converts")
            .ron,
    )
    .expect("loads");

    // 20 ticks a second over each job's rate per second, rounded.
    let numbers = |r| {
        let g = content.gather.get(r);
        (g.carry_capacity, g.ticks_per_unit)
    };
    assert_eq!(
        numbers(sim::Resource::Food),
        (10, 65),
        "forager, 0.31 a second"
    );
    assert_eq!(numbers(sim::Resource::Wood), (12, 51), "lumberjack, 0.39");
    assert_eq!(numbers(sim::Resource::Gold), (10, 53), "gold miner, 0.379");
    assert_eq!(numbers(sim::Resource::Stone), (10, 56), "stone miner, 0.36");
}

#[test]
fn each_entry_names_its_original_unit_id() {
    let converted = convert_with(&dat(), Selection::Default).expect("converts");

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
    let converted = convert_with(&dat(), Selection::Default).expect("converts");

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
        .any(|l| l.contains("gather.gold.ticks_per_unit")));
    assert!(!converted
        .report
        .iter()
        .any(|l| l.contains("villager.speed")));
}

#[test]
fn a_type_drawn_in_pieces_gets_its_pieces_and_annexes_as_parts() {
    // The town centre: its graphic's pieces in order (the piece the install
    // lacks left out, -1 its own SLP), then its annex's.
    let looks = app::parse_looks(
        &convert_with(&dat(), Selection::Default)
            .expect("converts")
            .ron,
    )
    .expect("loads");

    assert_eq!(
        looks.building("town_center").expect("a look").parts,
        at_one_point(&[889, 3596, 4612, 891])
    );
}

#[test]
fn a_type_that_is_only_its_own_sprite_has_no_parts() {
    // The villager's other pieces are in no archive, and the barracks names
    // only itself: each is drawn with its sprite alone.
    let looks = app::parse_looks(
        &convert_with(&dat(), Selection::Default)
            .expect("converts")
            .ron,
    )
    .expect("loads");

    let villager = &looks.unit("villager").expect("a look").parts;
    assert!(villager.is_empty(), "{villager:?}");
    assert!(looks.building("barracks").expect("a look").parts.is_empty());
}

#[test]
fn a_piece_the_install_lacks_is_left_out_and_reported() {
    let converted = convert_with(&dat(), Selection::Default).expect("converts");

    for slp in NOT_IN_THE_INSTALL {
        assert!(
            converted
                .report
                .iter()
                .any(|l| l.contains(&format!("SLP {slp}"))),
            "{slp}: {:#?}",
            converted.report
        );
    }
}

#[test]
fn a_type_whose_own_sprite_the_install_lacks_cannot_be_converted() {
    let mut install = install();
    install.sprites.remove(&993);
    let converted = convert(&dat(), 1, Selection::All, &install).expect("converts");

    assert!(!converted.ron.contains("sprmn_74"));
    assert!(converted
        .report
        .iter()
        .any(|l| l.contains("sprmn_74") && l.contains("left out")));
}

#[test]
fn every_unit_and_building_can_be_converted_with_unique_names() {
    let converted = convert_with(&dat(), Selection::All).expect("converts");

    let content = app::parse_content(&converted.ron).expect("loads and validates");
    let units: Vec<&str> = content.units.iter().map(|u| u.name.as_str()).collect();
    assert_eq!(
        units,
        [
            "sprmn_74",
            "vmbas_83",
            "pkemn_93",
            "vmfor_120",
            "vmlum_123",
            "vmmin_124",
            "vmgld_579",
        ]
    );
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
    let converted = convert_with(&dat(), Selection::All).expect("converts");

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
        convert_with(&dat, Selection::Default).err(),
        Some(ConvertError::MissingUnit {
            id: 74,
            name: "soldier"
        })
    );
}

#[test]
fn a_civilisation_the_file_does_not_have_is_an_error() {
    assert_eq!(
        convert(&dat(), 7, Selection::Default, &install()).err(),
        Some(ConvertError::NoCiv(7))
    );
}

#[test]
fn a_type_whose_icon_the_install_lacks_is_left_out_and_reported() {
    // The building icon sprite has 52 frames: frame 60 is not one of them,
    // and the game could not draw the button.
    let mut dat = dat();
    dat.civs[1].units[12].as_mut().expect("the barracks").icon = 60;

    let converted = convert_with(&dat, Selection::All).expect("converts");

    assert!(!converted.ron.contains("brks_12"));
    assert!(
        converted
            .report
            .iter()
            .any(|l| l.contains("brks_12") && l.contains("icon 60")),
        "{:#?}",
        converted.report
    );
}

#[test]
fn a_pieces_pixel_offset_is_kept_and_a_nested_piece_followed() {
    // The barracks drawn as itself, then a piece 2 pixels left and 5 down
    // which itself has a piece 1 further right: the offsets add up.
    let mut dat = dat();
    dat.graphics[4] = placed_graphic(4, 2683, &[(-1, 0, 0), (16, -2, 5)]);
    dat.graphics
        .push(placed_graphic(16, 889, &[(-1, 0, 0), (17, 1, 0)]));
    dat.graphics.push(placed_graphic(17, 4612, &[]));

    let looks = app::parse_looks(
        &convert_with(&dat, Selection::Default)
            .expect("converts")
            .ron,
    )
    .expect("loads");

    assert_eq!(
        looks.building("barracks").expect("a look").parts,
        [
            app::Part::new(2683, 0, 0),
            app::Part::new(889, -2, 5),
            app::Part::new(4612, -1, 5),
        ]
    );
}

#[test]
fn pieces_that_name_each_other_in_a_loop_stop_rather_than_running_forever() {
    let mut dat = dat();
    dat.graphics[4] = placed_graphic(4, 2683, &[(-1, 0, 0), (16, 0, 0)]);
    dat.graphics
        .push(placed_graphic(16, 889, &[(-1, 0, 0), (4, 0, 0)]));

    let converted = convert_with(&dat, Selection::Default).expect("converts");

    assert!(
        converted
            .report
            .iter()
            .any(|l| l.contains("barracks") && l.contains("nested")),
        "{:#?}",
        converted.report
    );
}

/// Converts [`dat`] with the spearman changed by `change`, and says whether
/// it was left out and named in the report.
fn spearman_left_out(change: impl FnOnce(&mut Unit)) -> bool {
    let mut dat = dat();
    change(dat.civs[1].units[93].as_mut().expect("the spearman"));
    let converted = convert_with(&dat, Selection::All).expect("converts");
    app::parse_content(&converted.ron).expect("the rest still loads");
    !converted.ron.contains("pkemn_93")
        && converted
            .report
            .iter()
            .any(|l| l.contains("pkemn_93") && l.contains("left out"))
}

#[test]
fn a_speed_that_is_not_a_number_is_left_out_rather_than_written_as_zero() {
    assert!(spearman_left_out(|u| u.speed = Some(f32::NAN)));
}

#[test]
fn a_negative_range_is_left_out_rather_than_failing_the_whole_file() {
    assert!(spearman_left_out(|u| {
        u.combat.as_mut().expect("fights").max_range = -1.0;
    }));
}

#[test]
fn a_range_longer_than_the_game_allows_is_left_out_rather_than_clamped() {
    assert!(spearman_left_out(|u| {
        u.combat.as_mut().expect("fights").max_range = 1.0e6;
    }));
}

#[test]
fn a_radius_wider_than_the_game_allows_is_left_out() {
    assert!(spearman_left_out(|u| u.collision[0] = 20.0));
}
