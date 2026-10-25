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
    // The limit leaves plenty of room for the real file.
    assert!(data.len() < assets::dat::MAX_DECOMPRESSED / 4, "{} bytes", data.len());
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
    // The limit leaves plenty of room for the real file.
    assert!(data.len() < assets::dat::MAX_DECOMPRESSED / 4, "{} bytes", data.len());
    println!(
        "empires2_x1_p1.dat: {} bytes compressed, {} decompressed",
        bytes.len(),
        data.len()
    );
}
