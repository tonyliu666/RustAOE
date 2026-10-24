//! DRS archives: the original's container for sprites, palettes, and sounds.
//!
//! Each test builds a small archive in memory with the same layout as the
//! game's files, so no game data is needed or committed.

use assets::{Drs, DrsError};

/// A table as written in the archive: its 4-byte type tag and its files.
type Table<'a> = ([u8; 4], Vec<(u32, &'a [u8])>);

/// Builds a DRS archive.
///
/// Layout: a 64-byte header (copyright text, version, archive type, table
/// count, offset of the first file), one 12-byte record per table (type tag,
/// offset of its entry list, entry count), each table's 12-byte entries (ID,
/// offset, size), then the file data.
fn build(tables: &[Table]) -> Vec<u8> {
    let mut header = Vec::new();
    let mut copyright = b"Copyright (c) 1997 Ensemble Studios.\x1a".to_vec();
    copyright.resize(40, 0);
    header.extend(copyright);
    header.extend(b"1.00");
    let mut kind = b"tribe".to_vec();
    kind.resize(12, 0);
    header.extend(kind);

    let records = 12 * tables.len();
    let entries: usize = tables.iter().map(|(_, files)| 12 * files.len()).sum();
    let first_file = 64 + records + entries;
    header.extend((tables.len() as u32).to_le_bytes());
    header.extend((first_file as u32).to_le_bytes());

    let mut table_records = Vec::new();
    let mut entry_list = Vec::new();
    let mut data = Vec::new();
    let mut entry_at = 64 + records;
    for (tag, files) in tables {
        table_records.extend(tag);
        table_records.extend((entry_at as u32).to_le_bytes());
        table_records.extend((files.len() as u32).to_le_bytes());
        for (id, bytes) in files {
            entry_list.extend(id.to_le_bytes());
            entry_list.extend(((first_file + data.len()) as u32).to_le_bytes());
            entry_list.extend((bytes.len() as u32).to_le_bytes());
            data.extend(*bytes);
        }
        entry_at += 12 * files.len();
    }

    [header, table_records, entry_list, data].concat()
}

/// The type tag the game uses for sprite tables: "slp" reversed and padded.
const SLP: [u8; 4] = *b" pls";
/// The type tag for raw binary files, such as palettes.
const BINA: [u8; 4] = *b"anib";

fn sample() -> Vec<u8> {
    build(&[
        (SLP, vec![(15001, b"grass".as_slice()), (1388, b"villager")]),
        (BINA, vec![(50500, b"JASC-PAL")]),
    ])
}

#[test]
fn every_file_comes_back_by_its_id() {
    let bytes = sample();
    let drs = Drs::parse(&bytes).expect("a well-formed archive");

    assert_eq!(drs.get(15001), Some(b"grass".as_slice()));
    assert_eq!(drs.get(1388), Some(b"villager".as_slice()));
    assert_eq!(drs.get(50500), Some(b"JASC-PAL".as_slice()));
}

#[test]
fn an_id_the_archive_does_not_hold_is_none() {
    let bytes = sample();
    let drs = Drs::parse(&bytes).expect("a well-formed archive");

    assert_eq!(drs.get(4242), None);
}

#[test]
fn the_ids_are_listed_in_order() {
    let bytes = sample();
    let drs = Drs::parse(&bytes).expect("a well-formed archive");

    assert_eq!(drs.ids().collect::<Vec<_>>(), [1388, 15001, 50500]);
}

#[test]
fn each_table_reports_its_file_type_and_count() {
    let bytes = sample();
    let drs = Drs::parse(&bytes).expect("a well-formed archive");

    let tables: Vec<(&str, usize)> = drs.tables().map(|t| (t.kind(), t.len())).collect();
    assert_eq!(tables, [("slp", 2), ("bina", 1)]);
}

#[test]
fn a_file_knows_which_table_it_came_from() {
    let bytes = sample();
    let drs = Drs::parse(&bytes).expect("a well-formed archive");

    assert_eq!(drs.kind_of(1388), Some("slp"));
    assert_eq!(drs.kind_of(50500), Some("bina"));
}

#[test]
fn an_archive_shorter_than_its_header_is_an_error() {
    let bytes = sample();

    assert_eq!(Drs::parse(&bytes[..30]).err(), Some(DrsError::Truncated));
}

#[test]
fn a_file_that_is_not_a_drs_archive_is_an_error() {
    let mut bytes = sample();
    bytes[40..44].copy_from_slice(b"9.99");

    assert_eq!(Drs::parse(&bytes).err(), Some(DrsError::NotDrs));
}

#[test]
fn a_table_list_running_past_the_end_is_an_error() {
    let mut bytes = sample();
    // Claim a thousand tables in a file that has room for two.
    bytes[56..60].copy_from_slice(&1000u32.to_le_bytes());

    assert_eq!(Drs::parse(&bytes).err(), Some(DrsError::Truncated));
}

#[test]
fn a_file_entry_pointing_past_the_end_is_an_error() {
    let mut bytes = sample();
    // The first entry's size field: make the file run far past the end.
    let size_at = 64 + 24 + 8;
    bytes[size_at..size_at + 4].copy_from_slice(&1_000_000u32.to_le_bytes());

    assert_eq!(
        Drs::parse(&bytes).err(),
        Some(DrsError::EntryOutOfBounds { id: 15001 })
    );
}

#[test]
fn an_id_listed_twice_is_an_error() {
    let bytes = build(&[(SLP, vec![(7, b"a".as_slice()), (7, b"b")])]);

    assert_eq!(
        Drs::parse(&bytes).err(),
        Some(DrsError::DuplicateId { id: 7 })
    );
}
