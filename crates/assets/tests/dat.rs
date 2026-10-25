//! The `.dat` file: raw-deflate compressed, starting with a version string.
//!
//! Compressed data is built in the tests; the real file is read only by the
//! ignored test at the end, when `AOE_ASSETS` points at an install.

use assets::dat::{decompress, decompress_with_limit, version, DatError};
use miniz_oxide::deflate::compress_to_vec;

/// The Conquerors' header: the version string padded to 8 bytes.
fn header(text: &[u8; 7]) -> Vec<u8> {
    let mut data = text.to_vec();
    data.push(0);
    data
}

#[test]
fn compressed_data_decompresses_back_to_what_it_was() {
    let original = [header(b"VER 5.7"), vec![42; 1000]].concat();

    assert_eq!(decompress(&compress_to_vec(&original, 6)), Ok(original));
}

#[test]
fn the_version_string_is_read_from_the_start() {
    let data = [header(b"VER 5.7"), vec![1, 2, 3]].concat();

    assert_eq!(version(&data), Ok("VER 5.7"));
}

#[test]
fn bytes_that_are_not_compressed_are_an_error() {
    // An uncompressed header is not a valid deflate stream.
    let result = decompress(b"VER 5.7\0this is not compressed at all");

    assert_eq!(result, Err(DatError::NotCompressed));
}

#[test]
fn data_too_short_for_a_version_is_an_error() {
    assert_eq!(version(b"VER"), Err(DatError::Truncated));
}

#[test]
fn data_without_a_version_string_is_an_error() {
    assert_eq!(version(b"PK\x03\x04abcd"), Err(DatError::NotDat));
}

#[test]
fn a_version_this_reader_does_not_know_is_an_error_naming_it() {
    let data = header(b"VER 5.9");

    assert_eq!(
        version(&data),
        Err(DatError::UnsupportedVersion("VER 5.9".to_string()))
    );
}

#[test]
fn output_larger_than_the_limit_is_an_error_rather_than_a_huge_allocation() {
    // A few bytes that inflate to megabytes: the shape of a decompression
    // bomb. With a small limit it must stop, not keep allocating.
    let bomb = compress_to_vec(&vec![0u8; 1 << 20], 10);

    assert_eq!(decompress_with_limit(&bomb, 4096), Err(DatError::TooLarge));
}

/// The real file, read only when `AOE_ASSETS` names an extracted install.
#[test]
#[ignore = "needs the game's data: set AOE_ASSETS and run with --ignored"]
fn the_real_file_decompresses_and_is_the_conquerors_format() {
    let dir = std::env::var("AOE_ASSETS").expect("AOE_ASSETS names an install");
    let bytes = std::fs::read(std::path::Path::new(&dir).join("data/empires2_x1_p1.dat"))
        .expect("the install has its .dat");

    let data = decompress(&bytes).expect("decompresses");

    assert_eq!(version(&data), Ok("VER 5.7"));
    // Every graphic present stores its own index as its ID; one wrong field
    // size anywhere before or inside the table would break that.
    let dat = Dat::parse(&data).expect("the sections up to the graphics read");
    let present = dat.graphics.iter().flatten().count();
    assert_eq!((dat.graphics.len(), present), (7367, 7014));
    for (index, graphic) in dat.graphics.iter().enumerate() {
        if let Some(graphic) = graphic {
            assert_eq!(graphic.id as usize, index, "graphic {index}");
        }
    }
    assert_eq!(dat.after_graphics, 917_004);
    // The terrain block: the first terrain's names sit after the block's map
    // fields (24 bytes), tile sizes (19 of 6), padding, and two flag bytes.
    assert_eq!(&data[917_004 + 142..917_004 + 147], b"Grass");
    assert_eq!(dat.random_maps_at, 917_004 + 42_208);
    assert_eq!(
        (
            dat.random_maps,
            dat.effects,
            dat.unit_headers,
            dat.units_present
        ),
        (9, 514, 866, 842)
    );
    assert_eq!((dat.civs.len(), dat.civs_at), (19, 1_092_830));
    assert_eq!(&data[dat.civs_at + 3..dat.civs_at + 7], b"Gaia");
    let names: Vec<&str> = dat.civs.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Gaia",
            "British",
            "French",
            "Goths",
            "Germans",
            "Japanese",
            "Chinese",
            "Byzantine",
            "Persians",
            "Saracens",
            "Turks",
            "Vikings",
            "Mongols",
            "Celts",
            "Spanish",
            "Aztecs",
            "Mayan",
            "Huns",
            "Koreans",
        ]
    );
    // Every civilisation has a slot for every unit ID, and the walk through
    // all of their units ends at the researches: 460 of them.
    assert!(dat.civs.iter().all(|c| c.units.len() == 866));
    assert_eq!((dat.researches_at, dat.researches), (5_587_765, 460));
    let britons = &dat.civs[1];
    let unit = |id: usize| britons.units[id].as_ref().expect("the unit exists");
    let villager = unit(83);
    assert_eq!(
        (
            villager.name.as_str(),
            villager.kind,
            villager.hit_points,
            villager.icon
        ),
        ("VMBAS", 70, 25, 15)
    );
    assert_eq!(dat.slp_of(villager.standing_graphic), Some(1479));
    assert_eq!(villager.resource_capacity, 10);
    let trained = villager.creatable.as_ref().expect("trained");
    assert_eq!(
        (trained.costs[0].resource, trained.costs[0].amount),
        (0, 50)
    );
    assert_eq!((trained.train_time, trained.train_location), (25, 109));
    let militia = unit(74);
    assert_eq!(
        (militia.name.as_str(), militia.hit_points, militia.icon),
        ("SPRMN", 40, 8)
    );
    assert_eq!(dat.slp_of(militia.standing_graphic), Some(993));
    let trained = militia.creatable.as_ref().expect("trained");
    let costs: Vec<(i16, i16)> = trained
        .costs
        .iter()
        .filter(|c| c.used)
        .map(|c| (c.resource, c.amount))
        .collect();
    assert_eq!(costs, [(0, 60), (3, 20)]);
    assert_eq!(
        (trained.train_time, trained.train_location, trained.button),
        (21, 12, 1)
    );
    assert_eq!((unit(109).hit_points, unit(109).icon), (2400, 28));
    assert_eq!((unit(12).hit_points, unit(12).icon), (1200, 2));
    assert_eq!(unit(93).creatable.as_ref().map(|c| c.button), Some(2));
    // The limit leaves plenty of room for the real file.
    assert!(
        data.len() < assets::dat::MAX_DECOMPRESSED / 4,
        "{} bytes",
        data.len()
    );
    println!(
        "empires2_x1_p1.dat: {} bytes compressed, {} decompressed",
        bytes.len(),
        data.len()
    );
}

// --- sections up to the graphics table ------------------------------------------

use assets::dat::{Dat, GraphicDelta};

/// Little-endian field writers for building a `.dat` body.
#[derive(Default)]
struct Bytes(Vec<u8>);

impl Bytes {
    fn i16(&mut self, v: i16) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn i32(&mut self, v: i32) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn f32(&mut self, v: f32) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    /// A fixed-size, zero-padded text field.
    fn text(&mut self, s: &str, size: usize) -> &mut Self {
        let mut field = s.as_bytes().to_vec();
        field.resize(size, 0);
        self.0.extend(field);
        self
    }
}

/// One graphic in The Conquerors' layout.
fn graphic(
    b: &mut Bytes,
    id: i16,
    name: &str,
    slp: i32,
    deltas: &[(i16, i16, i16)],
    angle_sounds: u8,
) {
    b.text(name, 21).text("file", 13).i32(slp);
    b.u8(0).u8(0).u8(5).i16(-1).u8(0); // loaded, colour flag, layer, player colour, selection
    for _ in 0..4 {
        b.i16(0); // coordinates
    }
    b.i16(deltas.len() as i16).i16(-1).u8(angle_sounds);
    let angles = 2;
    b.i16(10).i16(angles).f32(1.0).f32(0.1).f32(0.0); // frames, angles, speed, frame time, replay delay
    b.u8(0).i16(id).u8(0).u8(0); // sequence type, ID, mirroring, editor flag
    for (graphic_id, x, y) in deltas {
        b.i16(*graphic_id)
            .i16(0)
            .i32(0)
            .i16(*x)
            .i16(*y)
            .i16(-1)
            .i16(0);
    }
    if angle_sounds != 0 {
        for _ in 0..angles {
            for _ in 0..6 {
                b.i16(-1);
            }
        }
    }
}

/// A small inflated `.dat`: version, one terrain restriction over two
/// terrains, one player colour, one sound with one file, and three graphic
/// slots of which the middle one is empty. Then `rest`, which [`after`]
/// fills with the sections that follow.
fn small_dat(rest: &[u8]) -> Vec<u8> {
    let mut b = Bytes::default();
    b.text("VER 5.7", 8);
    b.i16(1).i16(2); // terrain restrictions, terrains
    b.i32(1).i32(1); // their pointer tables
    b.f32(1.0).f32(0.0); // passable multipliers per terrain
    for _ in 0..2 {
        b.i32(-1).i32(-1).i32(-1).i32(0); // pass graphic per terrain
    }
    b.i16(1); // player colours
    for v in 0..9 {
        b.i32(v);
    }
    b.i16(1); // sounds
    b.i16(7).i16(0).i16(1).i32(300); // ID, delay, files, cache time
    b.text("sound.wav", 13).i32(5000).i16(100).i16(-1).i16(-1);
    b.i16(3); // graphic slots
    b.i32(1).i32(0).i32(1); // pointers: slot 1 is empty
    graphic(&mut b, 0, "VILLAGER", 1388, &[(5, 3, -4)], 1);
    graphic(&mut b, 2, "BARRACKS", 131, &[], 0);
    b.0.extend(rest);
    b.0
}

#[test]
fn the_graphics_table_reads_each_graphics_sprite_and_pieces() {
    let data = small_dat(&after());

    let dat = Dat::parse(&data).expect("a well-formed file");

    let villager = dat.graphics[0].as_ref().expect("slot 0 has a graphic");
    assert_eq!(
        (villager.id, villager.name.as_str(), villager.slp),
        (0, "VILLAGER", 1388)
    );
    assert_eq!(
        villager.deltas,
        [GraphicDelta {
            graphic_id: 5,
            offset_x: 3,
            offset_y: -4,
        }]
    );
    assert!(dat.graphics[1].is_none(), "slot 1 has no graphic");
    let barracks = dat.graphics[2].as_ref().expect("slot 2 has a graphic");
    assert_eq!((barracks.slp, barracks.deltas.len()), (131, 0));
}

#[test]
fn the_terrain_block_begins_exactly_where_the_graphics_table_ends() {
    let data = small_dat(&after());

    let dat = Dat::parse(&data).expect("a well-formed file");

    assert_eq!(
        &data[dat.after_graphics..dat.after_graphics + 4],
        TERRAIN_BLOCK_MARK
    );
}

#[test]
fn a_file_cut_off_inside_the_graphics_is_an_error() {
    let data = small_dat(&[]);

    let result = Dat::parse(&data[..data.len() - 10]);

    assert_eq!(result.err(), Some(DatError::Truncated));
}

#[test]
fn a_negative_count_is_an_error_rather_than_a_huge_allocation() {
    let mut data = small_dat(&after());
    // The terrain restriction count, just after the version.
    data[8..10].copy_from_slice(&(-1i16).to_le_bytes());

    assert_eq!(
        Dat::parse(&data).err(),
        Some(DatError::Corrupt("negative count"))
    );
}

// --- from the terrain block to the civilisations -----------------------------------

/// The Conquerors' terrain block has no counts: 42 terrains of 436 bytes and
/// 16 borders of 1440, inside a fixed frame of map fields. 42,208 bytes in
/// all, checked against the real file by the ignored test.
const TERRAIN_BLOCK_SIZE: usize = 42_208;
/// The first bytes of the fake terrain block, to find where it begins.
const TERRAIN_BLOCK_MARK: &[u8] = b"TERR";

/// One random map's lists, as (lands, terrains, units, elevations).
type MapLists = (usize, usize, usize, usize);

/// The random maps: a count and a pointer, a header per map (its ID, nine
/// fields, and each list's count and pointer), then each map again without
/// its ID and with its lists' entries.
fn random_maps(b: &mut Bytes, maps: &[MapLists]) {
    b.i32(maps.len() as i32).i32(0x1234);
    let lists = |b: &mut Bytes, (lands, terrains, units, elevations): MapLists, entries: bool| {
        for (count, size) in [(lands, 44), (terrains, 24), (units, 44), (elevations, 24)] {
            b.i32(count as i32).i32(0);
            if entries {
                b.0.extend(std::iter::repeat_n(7u8, count * size));
            }
        }
    };
    for (id, map) in maps.iter().enumerate() {
        b.i32(id as i32);
        for _ in 0..9 {
            b.i32(1);
        }
        lists(b, *map, false);
    }
    for map in maps {
        for _ in 0..9 {
            b.i32(1);
        }
        lists(b, *map, true);
    }
}

/// Effects: an `i32` count, then per effect a 31-byte name and its
/// commands, 11 bytes each.
fn effects(b: &mut Bytes, commands: &[usize]) {
    b.i32(commands.len() as i32);
    for count in commands {
        b.text("CHUN4", 31).i16(*count as i16);
        for _ in 0..*count {
            b.u8(1).i16(2).i16(3).i16(4).f32(5.0);
        }
    }
}

/// Unit headers: an `i32` count, then per unit a flag and, when it exists,
/// its tasks, 59 bytes each. `None` is a unit that does not exist.
fn unit_headers(b: &mut Bytes, tasks: &[Option<usize>]) {
    b.i32(tasks.len() as i32);
    for unit in tasks {
        match unit {
            None => {
                b.u8(0);
            }
            Some(count) => {
                b.u8(1).i16(*count as i16);
                b.0.extend(std::iter::repeat_n(3u8, count * 59));
            }
        }
    }
}

/// The sections after the graphics table: a terrain block, two random maps,
/// three effects, three unit headers, two civilisations without units, and
/// a research count of 7.
fn after() -> Vec<u8> {
    let mut b = Bytes::default();
    let mut block = TERRAIN_BLOCK_MARK.to_vec();
    block.resize(TERRAIN_BLOCK_SIZE, 0);
    b.0.extend(block);
    random_maps(&mut b, &[(1, 0, 2, 1), (0, 3, 0, 0)]);
    effects(&mut b, &[0, 3, 1]);
    unit_headers(&mut b, &[Some(2), None, Some(0)]);
    b.i16(2);
    civ(&mut b, "Gaia", &[]);
    civ(&mut b, "British", &[]);
    b.i16(7);
    b.0
}

#[test]
fn the_sections_after_the_graphics_are_walked_to_the_civilisations() {
    let data = small_dat(&after());

    let dat = Dat::parse(&data).expect("a well-formed file");

    assert_eq!(dat.random_maps_at, dat.after_graphics + TERRAIN_BLOCK_SIZE);
    assert_eq!(
        (
            dat.random_maps,
            dat.effects,
            dat.unit_headers,
            dat.units_present
        ),
        (2, 3, 3, 2)
    );
    assert_eq!(dat.civs.len(), 2);
    // The first civilisation follows its count: player type, then name.
    assert_eq!(&data[dat.civs_at + 3..dat.civs_at + 7], b"Gaia");
}

#[test]
fn each_section_begins_where_the_one_before_it_ends() {
    let data = small_dat(&after());

    let dat = Dat::parse(&data).expect("a well-formed file");

    // Each count is the first field of its section.
    let i32_at = |at: usize| i32::from_le_bytes(data[at..at + 4].try_into().expect("4 bytes"));
    assert_eq!(i32_at(dat.random_maps_at), 2);
    assert_eq!(i32_at(dat.effects_at), 3);
    assert_eq!(i32_at(dat.unit_headers_at), 3);
    assert_eq!(&data[dat.civs_at..dat.civs_at + 2], &2i16.to_le_bytes());
}

#[test]
fn a_random_map_whose_two_copies_disagree_is_an_error() {
    let mut data = small_dat(&after());
    let dat = Dat::parse(&data).expect("a well-formed file");
    // The second copy of map 0 starts after the count, pointer, and two
    // 72-byte headers; its land count is 36 bytes in.
    let at = dat.random_maps_at + 8 + 2 * 72 + 36;
    data[at..at + 4].copy_from_slice(&5i32.to_le_bytes());

    assert_eq!(
        Dat::parse(&data).err(),
        Some(DatError::Corrupt("random map lists differ between copies"))
    );
}

#[test]
fn a_negative_effect_count_is_an_error_rather_than_a_huge_allocation() {
    let mut data = small_dat(&after());
    let at = Dat::parse(&data).expect("a well-formed file").effects_at;
    data[at..at + 4].copy_from_slice(&(-1i32).to_le_bytes());

    assert_eq!(
        Dat::parse(&data).err(),
        Some(DatError::Corrupt("negative count"))
    );
}

#[test]
fn a_random_map_count_too_large_for_the_file_is_truncated_rather_than_allocated() {
    let mut data = small_dat(&after());
    let at = Dat::parse(&data)
        .expect("a well-formed file")
        .random_maps_at;
    data[at..at + 4].copy_from_slice(&i32::MAX.to_le_bytes());

    assert_eq!(Dat::parse(&data).err(), Some(DatError::Truncated));
}

#[test]
fn a_file_cut_off_inside_the_unit_headers_is_an_error() {
    let data = small_dat(&after());
    let at = Dat::parse(&data)
        .expect("a well-formed file")
        .unit_headers_at;

    assert_eq!(
        Dat::parse(&data[..at + 10]).err(),
        Some(DatError::Truncated)
    );
}

// --- civilisations and their units -----------------------------------------------

use assets::dat::{Cost, Unit};

/// A unit to write: its type and the fields the reader keeps. Fields a type
/// does not have are ignored when writing it.
#[derive(Clone)]
struct UnitSpec {
    kind: u8,
    id: i16,
    name: &'static str,
    hit_points: i16,
    standing: i16,
    icon: i16,
    collision: f32,
    capacity: i16,
    speed: f32,
    work_rate: f32,
    drop_sites: [i16; 2],
    attacks: Vec<(i16, i16)>,
    max_range: f32,
    reload: f32,
    costs: [(i16, i16, i16); 3],
    train_time: i16,
    train_location: i16,
    button: u8,
    damage_graphics: usize,
}

fn villager_spec() -> UnitSpec {
    UnitSpec {
        kind: 70,
        id: 83,
        name: "VMDL",
        hit_points: 25,
        standing: 1284,
        icon: 15,
        collision: 0.2,
        capacity: 10,
        speed: 0.8,
        work_rate: 1.0,
        drop_sites: [109, -1],
        attacks: vec![(4, 3)],
        max_range: 0.0,
        reload: 2.0,
        costs: [(0, 50, 1), (-1, 0, 0), (-1, 0, 0)],
        train_time: 25,
        train_location: 109,
        button: 1,
        damage_graphics: 1,
    }
}

/// One unit in The Conquerors' layout.
fn unit(b: &mut Bytes, u: &UnitSpec) {
    b.u8(u.kind).i16(u.name.len() as i16).i16(u.id);
    b.i16(5000).i16(6000).i16(4); // language strings, class
    b.i16(u.standing).i16(-1).i16(-1).i16(-1).u8(0); // standing pair, dying, undead, undead mode
    b.i16(u.hit_points).f32(4.0).u8(0); // line of sight, garrison
    b.f32(u.collision).f32(u.collision).f32(2.0);
    b.i16(-1).i16(-1).i16(-1).u8(0).u8(0); // train and damage sounds, dead unit, sort, built on
    b.i16(u.icon).u8(0).i16(-1).u8(1).u8(0); // icon, hidden, portrait, enabled, disabled
    b.i16(-1).i16(-1).i16(-1).i16(-1); // placement terrains
    b.f32(u.collision).f32(u.collision).u8(0).u8(0).i16(7).u8(0); // clearance, hill, fog, restriction, fly
    b.i16(u.capacity)
        .f32(0.0)
        .u8(0)
        .u8(0)
        .u8(0)
        .u8(0)
        .u8(0)
        .f32(0.0)
        .u8(0);
    b.i32(0).i32(0).i32(0); // help, hotkey text, hotkey
    for _ in 0..4 + 5 {
        b.u8(0); // recyclable .. civilisation
    }
    b.i16(0).u8(0).u8(0).f32(1.0).f32(1.0).f32(1.0); // nothing, selection, outline
    for _ in 0..3 {
        b.i16(-1).f32(0.0).u8(0); // resource storages
    }
    b.u8(u.damage_graphics as u8);
    for _ in 0..u.damage_graphics {
        b.i16(10).i16(50).u8(0);
    }
    b.i16(-1).i16(-1).u8(0).u8(0); // selection and dying sounds, attack reaction, convert
    b.0.extend(u.name.as_bytes());
    b.i16(u.id).i16(u.id); // copy, base
    if u.kind == 90 || u.kind < 20 {
        return;
    }
    b.f32(u.speed);
    if u.kind < 30 {
        return;
    }
    b.i16(-1)
        .i16(-1)
        .f32(1.0)
        .u8(0)
        .i16(-1)
        .u8(0)
        .f32(0.0)
        .u8(0);
    for _ in 0..5 {
        b.f32(0.0);
    }
    if u.kind < 40 {
        return;
    }
    b.i16(-1)
        .f32(5.0)
        .f32(u.work_rate)
        .i16(u.drop_sites[0])
        .i16(u.drop_sites[1]);
    b.u8(0).i16(-1).i16(-1).u8(0);
    if u.kind < 50 {
        return;
    }
    b.i16(0).i16(u.attacks.len() as i16);
    for (class, amount) in &u.attacks {
        b.i16(*class).i16(*amount);
    }
    b.i16(1).i16(4).i16(0).i16(-1); // one armour, terrain bonus
    b.f32(u.max_range)
        .f32(0.0)
        .f32(u.reload)
        .i16(-1)
        .i16(100)
        .u8(0)
        .i16(0);
    b.f32(0.0).f32(0.0).f32(0.0).u8(0).f32(0.0).f32(0.0).i16(-1);
    b.i16(0).i16(3).f32(u.max_range).f32(u.reload);
    if u.kind == 60 {
        b.u8(0).u8(0).u8(0).u8(0).u8(0).f32(0.0);
    }
    if u.kind < 70 {
        return;
    }
    for (resource, amount, used) in u.costs {
        b.i16(resource).i16(amount).i16(used);
    }
    b.i16(u.train_time).i16(u.train_location).u8(u.button);
    b.f32(0.0).f32(0.0).u8(0).u8(0).i32(-1).f32(1.0).u8(1);
    b.f32(0.0).f32(0.0).f32(0.0).i32(-1).i32(-1).u8(0).i16(0);
    if u.kind == 80 {
        b.i16(-1)
            .i16(-1)
            .u8(0)
            .i16(0)
            .u8(0)
            .i16(-1)
            .i16(-1)
            .i16(-1)
            .i16(-1)
            .u8(0);
        for _ in 0..4 {
            b.i16(-1).f32(0.0).f32(0.0);
        }
        b.i16(-1)
            .i16(-1)
            .i16(-1)
            .i16(-1)
            .u8(0)
            .f32(0.0)
            .f32(0.0)
            .i16(-1);
        for _ in 0..6 {
            b.u8(0);
        }
    }
}

/// One civilisation: player type, 20-byte name, resources, icon set, then a
/// pointer per unit slot and each present unit.
fn civ(b: &mut Bytes, name: &str, units: &[Option<UnitSpec>]) {
    b.u8(1)
        .text(name, 20)
        .i16(2)
        .i16(-1)
        .i16(-1)
        .f32(200.0)
        .f32(100.0)
        .u8(1);
    b.i16(units.len() as i16);
    for u in units {
        b.i32(if u.is_some() { 1 } else { 0 });
    }
    for u in units.iter().flatten() {
        unit(b, u);
    }
}

/// [`after`] up to the civilisation count, with `civs` written in full, then
/// a research count of 7.
fn with_civs(civs: &[(&str, Vec<Option<UnitSpec>>)]) -> Vec<u8> {
    let mut b = Bytes::default();
    let mut block = TERRAIN_BLOCK_MARK.to_vec();
    block.resize(TERRAIN_BLOCK_SIZE, 0);
    b.0.extend(block);
    random_maps(&mut b, &[(1, 0, 2, 1)]);
    effects(&mut b, &[1]);
    unit_headers(&mut b, &[Some(2), None]);
    b.i16(civs.len() as i16);
    for (name, units) in civs {
        civ(&mut b, name, units);
    }
    b.i16(7);
    small_dat(&b.0)
}

/// A unit of every type The Conquerors' file holds.
fn one_of_each() -> Vec<Option<UnitSpec>> {
    let v = villager_spec();
    let of = |kind, id| {
        Some(UnitSpec {
            kind,
            id,
            name: "X",
            ..v.clone()
        })
    };
    vec![
        of(10, 0),
        None,
        of(15, 2),
        of(20, 3),
        of(25, 4),
        of(30, 5),
        of(40, 6),
        of(50, 7),
        of(60, 8),
        Some(v.clone()),
        of(80, 10),
        of(90, 11),
    ]
}

fn read_civ_units(data: &[u8]) -> Vec<Vec<Option<Unit>>> {
    Dat::parse(data)
        .expect("a well-formed file")
        .civs
        .into_iter()
        .map(|c| c.units)
        .collect()
}

#[test]
fn each_civilisation_is_read_with_its_name_and_unit_slots() {
    let data = with_civs(&[("Gaia", one_of_each()), ("British", vec![None, None])]);

    let dat = Dat::parse(&data).expect("a well-formed file");

    let names: Vec<&str> = dat.civs.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Gaia", "British"]);
    assert_eq!(dat.civs[0].units.len(), 12);
    assert!(dat.civs[0].units[1].is_none());
    assert_eq!(dat.civs[1].units, [None, None]);
}

#[test]
fn reading_ends_exactly_where_the_researches_begin() {
    let data = with_civs(&[("Gaia", one_of_each()), ("British", vec![None])]);

    let dat = Dat::parse(&data).expect("a well-formed file");

    assert_eq!(&data[dat.researches_at..], &7i16.to_le_bytes());
    assert_eq!(dat.researches, 7);
}

#[test]
fn a_trainable_units_fields_are_read() {
    let data = with_civs(&[("Gaia", one_of_each())]);
    let units = read_civ_units(&data);

    let villager = units[0][9].as_ref().expect("slot 9 holds the villager");

    assert_eq!(
        (
            villager.kind,
            villager.id,
            villager.name.as_str(),
            villager.class
        ),
        (70, 83, "VMDL", 4)
    );
    assert_eq!(
        (
            villager.hit_points,
            villager.standing_graphic,
            villager.icon
        ),
        (25, 1284, 15)
    );
    assert_eq!(
        (villager.collision, villager.clearance),
        ([0.2, 0.2, 2.0], [0.2, 0.2])
    );
    assert_eq!(villager.resource_capacity, 10);
    assert_eq!(villager.speed, Some(0.8));
    assert_eq!(villager.work_rate, Some(1.0));
    assert_eq!(villager.drop_sites, Some([109, -1]));
    let combat = villager.combat.as_ref().expect("type 70 fights");
    assert_eq!(combat.attacks, [(4, 3)]);
    assert_eq!((combat.max_range, combat.reload_time), (0.0, 2.0));
    let creatable = villager.creatable.as_ref().expect("type 70 is trained");
    assert_eq!(
        creatable.costs,
        [
            Cost {
                resource: 0,
                amount: 50,
                used: true
            },
            Cost {
                resource: -1,
                amount: 0,
                used: false
            },
            Cost {
                resource: -1,
                amount: 0,
                used: false
            },
        ]
    );
    assert_eq!(
        (
            creatable.train_time,
            creatable.train_location,
            creatable.button
        ),
        (25, 109, 1)
    );
}

#[test]
fn each_unit_type_has_the_parts_its_type_has() {
    let data = with_civs(&[("Gaia", one_of_each())]);
    let units = read_civ_units(&data);

    let parts = |slot: usize| {
        let u = units[0][slot].as_ref().expect("present");
        (
            u.kind,
            u.speed.is_some(),
            u.work_rate.is_some(),
            u.combat.is_some(),
            u.creatable.is_some(),
        )
    };
    assert_eq!(parts(0), (10, false, false, false, false));
    assert_eq!(parts(2), (15, false, false, false, false));
    assert_eq!(parts(3), (20, true, false, false, false));
    assert_eq!(parts(4), (25, true, false, false, false));
    assert_eq!(parts(5), (30, true, false, false, false));
    assert_eq!(parts(6), (40, true, true, false, false));
    assert_eq!(parts(7), (50, true, true, true, false));
    assert_eq!(parts(8), (60, true, true, true, false));
    assert_eq!(parts(10), (80, true, true, true, true));
    assert_eq!(parts(11), (90, false, false, false, false));
}

#[test]
fn a_unit_of_a_type_the_reader_does_not_know_is_an_error() {
    let strange = UnitSpec {
        kind: 45,
        ..villager_spec()
    };
    let data = with_civs(&[("Gaia", vec![Some(strange)])]);

    assert_eq!(
        Dat::parse(&data).err(),
        Some(DatError::Corrupt("unknown unit type"))
    );
}

#[test]
fn a_file_cut_off_inside_a_unit_is_an_error() {
    let data = with_civs(&[("Gaia", one_of_each())]);
    let researches_at = Dat::parse(&data).expect("well-formed").researches_at;

    assert_eq!(
        Dat::parse(&data[..researches_at - 30]).err(),
        Some(DatError::Truncated)
    );
}

#[test]
fn unit_headers_keep_each_tasks_action() {
    let data = with_civs(&[("Gaia", vec![])]);

    let dat = Dat::parse(&data).expect("a well-formed file");

    // The fake headers: unit 0 has two tasks, unit 1 does not exist. Each
    // fake task's bytes are all 3, so its action type reads 0x0303.
    let actions: Vec<Option<Vec<i16>>> = dat
        .unit_tasks
        .iter()
        .map(|tasks| tasks.as_ref().map(|t| t.iter().map(|t| t.action).collect()))
        .collect();
    assert_eq!(actions, [Some(vec![0x0303, 0x0303]), None]);
    let rate = dat.unit_tasks[0].as_ref().expect("unit 0")[0].work_value;
    assert_eq!(rate, f32::from_le_bytes([3; 4]));
}

#[test]
fn a_graphic_resolves_to_the_sprite_it_names() {
    let data = with_civs(&[("Gaia", vec![])]);

    let dat = Dat::parse(&data).expect("a well-formed file");

    assert_eq!(dat.slp_of(0), Some(1388));
    assert_eq!(dat.slp_of(1), None, "an empty slot");
    assert_eq!(dat.slp_of(-1), None, "no graphic");
    assert_eq!(dat.slp_of(99), None, "past the table");
}

#[test]
fn a_graphic_that_names_no_sprite_resolves_to_none() {
    let mut data = with_civs(&[("Gaia", vec![])]);
    // The barracks graphic's SLP field follows its 21-byte name and 13-byte
    // file name; -1 means the graphic has no sprite of its own.
    let at = data
        .windows(8)
        .position(|w| w == b"BARRACKS")
        .expect("the barracks graphic")
        + 21
        + 13;
    data[at..at + 4].copy_from_slice(&(-1i32).to_le_bytes());

    let dat = Dat::parse(&data).expect("a well-formed file");

    assert_eq!(dat.slp_of(2), None);
}
