//! The real file, when `AOE_ASSETS` names an install: run with `--ignored`.

use datconv::{convert, Selection};

/// What the real install holds.
fn install() -> datconv::Install {
    let dir = std::env::var("AOE_ASSETS").expect("AOE_ASSETS names an install");
    datconv::Install::read(std::path::Path::new(&dir)).expect("the archives read")
}

fn real_dat() -> assets::dat::Dat {
    let dir = std::env::var("AOE_ASSETS").expect("AOE_ASSETS names an install");
    let bytes = std::fs::read(std::path::Path::new(&dir).join("data/empires2_x1_p1.dat"))
        .expect("the install has its .dat");
    let data = assets::dat::decompress(&bytes).expect("decompresses");
    assets::dat::Dat::parse(&data).expect("reads")
}

#[test]
#[ignore = "needs the game's data: set AOE_ASSETS and run with --ignored"]
fn the_four_types_convert_to_content_the_game_loads_with_the_originals_numbers() {
    let converted = convert(&real_dat(), 1, Selection::Default, &install()).expect("converts");

    let content = app::parse_content(&converted.ron).expect("loads and validates");
    let villager = &content.units[0];
    assert_eq!(
        (villager.max_hp, villager.cost.food, villager.train_ticks),
        (25, 50, 500)
    );
    assert!(villager.gathers);
    let soldier = &content.units[1];
    assert_eq!(
        (soldier.max_hp, soldier.cost.food, soldier.cost.gold),
        (40, 60, 20)
    );
    assert!(!soldier.gathers);
    assert!(content.buildings[0].drop_off, "the town centre takes gold");
    // Drawn as the original draws them: the town centre from its pieces and
    // annexes, the barracks and the units from their own sprites.
    let looks = app::parse_looks(&converted.ron).expect("its looks load");
    let town_centre = looks.building("town_center").expect("a look");
    let slps: Vec<u32> = town_centre.parts.iter().map(|p| p.slp).collect();
    assert_eq!(slps, [889, 3596, 4612, 891, 3595, 4611, 3594, 4610]);
    assert!(town_centre.parts.iter().all(|p| p.offset == [0, 0]));
    let barracks = looks.building("barracks").expect("a look");
    assert_eq!((barracks.sprite, barracks.parts.len()), (2683, 0));
    assert!(looks.units.values().all(|l| l.parts.is_empty()));
    // The game can draw it: every sprite and icon it names is in the install.
    let dir = std::env::var("AOE_ASSETS").expect("AOE_ASSETS names an install");
    let looks = app::parse_looks(&converted.ron).expect("its looks load");
    app::load_images(std::path::Path::new(&dir), &content, &looks).expect("the install draws it");
    println!("{}", converted.ron);
    for line in &converted.report {
        println!("report: {line}");
    }
}

#[test]
#[ignore = "needs the game's data: set AOE_ASSETS and run with --ignored"]
fn every_unit_and_building_of_a_civilisation_converts_to_content_the_game_loads() {
    let converted = convert(&real_dat(), 1, Selection::All, &install()).expect("converts");

    let content = app::parse_content(&converted.ron).expect("loads and validates");
    let left_out = converted
        .report
        .iter()
        .filter(|l| l.contains("left out"))
        .count();
    println!(
        "{} unit and {} building types; {left_out} left out",
        content.units.len(),
        content.buildings.len()
    );
    assert!(content.units.len() > 50, "{}", content.units.len());
    assert!(content.buildings.len() > 20, "{}", content.buildings.len());
    // And the game can draw every one of them: sprites, parts, and icons.
    let dir = std::env::var("AOE_ASSETS").expect("AOE_ASSETS names an install");
    let looks = app::parse_looks(&converted.ron).expect("its looks load");
    app::load_images(std::path::Path::new(&dir), &content, &looks).expect("the install draws it");
}
