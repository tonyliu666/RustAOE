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
/// slots of which the middle one is empty. Then `rest`.
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
    let data = small_dat(&[]);

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
fn reading_stops_exactly_where_the_graphics_table_ends() {
    let marker = b"NEXT SECTION";
    let data = small_dat(marker);

    let dat = Dat::parse(&data).expect("a well-formed file");

    assert_eq!(&data[dat.after_graphics..], marker);
}

#[test]
fn a_file_cut_off_inside_the_graphics_is_an_error() {
    let data = small_dat(&[]);

    let result = Dat::parse(&data[..data.len() - 10]);

    assert_eq!(result.err(), Some(DatError::Truncated));
}

#[test]
fn a_negative_count_is_an_error_rather_than_a_huge_allocation() {
    let mut data = small_dat(&[]);
    // The terrain restriction count, just after the version.
    data[8..10].copy_from_slice(&(-1i16).to_le_bytes());

    assert_eq!(
        Dat::parse(&data).err(),
        Some(DatError::Corrupt("negative count"))
    );
}
