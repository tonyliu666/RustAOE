//! DRS archives, the original's container format.
//!
//! Layout, all integers little-endian:
//!
//! - A 64-byte header: 40 bytes of copyright text, the version `1.00`, a
//!   12-byte archive type (`tribe`), the table count, and the offset of the
//!   first file.
//! - One 12-byte record per table: a type tag, the offset of the table's
//!   entry list, and its entry count. The tag is the file extension written
//!   backwards and padded with spaces: `" pls"` for `slp` sprites, `"anib"`
//!   for `bina` binary files such as palettes, `" vaw"` for `wav` sounds.
//! - Each table's 12-byte entries: file ID, offset of the file from the start
//!   of the archive, and its size.
//!
//! The archive is parsed over a borrowed slice: `graphics.drs` is 86 MB, and
//! copying every sprite out of it up front would double that.

use std::collections::BTreeMap;
use std::fmt;

const HEADER: usize = 64;
const RECORD: usize = 12;
const VERSION: &[u8; 4] = b"1.00";

/// A parsed DRS archive borrowing its bytes.
/// The original's archives, in the order it opens them (`FUN_0043ae90`):
/// the expansion's first, then the base game's. A file is looked up by type
/// and ID in the first archive that has it (`FUN_00543630`), so the
/// expansion's copies override the base game's.
pub const ARCHIVE_ORDER: [&str; 8] = [
    "sounds_x1.drs",
    "gamedata_x1.drs",
    "gamedata_x1_p1.drs",
    "sounds.drs",
    "graphics.drs",
    "terrain.drs",
    "interfac.drs",
    "gamedata.drs",
];

#[derive(Clone, Debug)]
pub struct Drs<'a> {
    bytes: &'a [u8],
    tables: Vec<DrsTable>,
    /// File ID to (table index, offset, size).
    files: BTreeMap<u32, (usize, usize, usize)>,
}

/// One table of an archive: every file of one type.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DrsTable {
    kind: String,
    len: usize,
}

impl DrsTable {
    /// The file type, such as `slp`, `bina`, or `wav`.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// How many files the table holds.
    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Why bytes could not be read as a DRS archive.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DrsError {
    /// Shorter than the header, the table list, or an entry list says.
    Truncated,
    /// The version field is not `1.00`.
    NotDrs,
    /// An entry's data runs past the end of the archive.
    EntryOutOfBounds { id: u32 },
    /// Two entries share an ID, so a lookup by ID would be ambiguous.
    DuplicateId { id: u32 },
}

impl fmt::Display for DrsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DrsError::Truncated => write!(f, "DRS archive is truncated"),
            DrsError::NotDrs => write!(f, "not a DRS archive (version is not 1.00)"),
            DrsError::EntryOutOfBounds { id } => {
                write!(f, "DRS file {id} runs past the end of the archive")
            }
            DrsError::DuplicateId { id } => write!(f, "DRS file ID {id} appears twice"),
        }
    }
}

impl std::error::Error for DrsError {}

/// A little-endian `u32` at `at`, or `Truncated`.
fn u32_at(bytes: &[u8], at: usize) -> Result<u32, DrsError> {
    let field = bytes.get(at..at + 4).ok_or(DrsError::Truncated)?;
    Ok(u32::from_le_bytes([field[0], field[1], field[2], field[3]]))
}

/// A table tag as its file type: `" pls"` becomes `slp`.
fn kind_of_tag(tag: &[u8]) -> String {
    tag.iter()
        .rev()
        .map(|&b| char::from(b))
        .collect::<String>()
        .trim()
        .to_string()
}

impl<'a> Drs<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Drs<'a>, DrsError> {
        if bytes.len() < HEADER {
            return Err(DrsError::Truncated);
        }
        if &bytes[40..44] != VERSION {
            return Err(DrsError::NotDrs);
        }
        let table_count = u32_at(bytes, 56)? as usize;
        // Check the whole table list fits before reading any of it, so that a
        // wild count reports truncation rather than garbage read from the
        // file data that follows.
        let records_end = RECORD
            .checked_mul(table_count)
            .and_then(|size| size.checked_add(HEADER));
        if records_end.is_none_or(|end| end > bytes.len()) {
            return Err(DrsError::Truncated);
        }

        let mut tables = Vec::new();
        let mut files = BTreeMap::new();
        for table in 0..table_count {
            let record = HEADER + RECORD * table;
            let tag = bytes.get(record..record + 4).ok_or(DrsError::Truncated)?;
            let entries_at = u32_at(bytes, record + 4)? as usize;
            let count = u32_at(bytes, record + 8)? as usize;

            for entry in 0..count {
                let at = entries_at + RECORD * entry;
                let id = u32_at(bytes, at)?;
                let offset = u32_at(bytes, at + 4)? as usize;
                let size = u32_at(bytes, at + 8)? as usize;
                if offset.checked_add(size).is_none_or(|end| end > bytes.len()) {
                    return Err(DrsError::EntryOutOfBounds { id });
                }
                if files.insert(id, (table, offset, size)).is_some() {
                    return Err(DrsError::DuplicateId { id });
                }
            }
            tables.push(DrsTable {
                kind: kind_of_tag(tag),
                len: count,
            });
        }

        Ok(Drs {
            bytes,
            tables,
            files,
        })
    }

    /// The bytes of file `id`.
    pub fn get(&self, id: u32) -> Option<&'a [u8]> {
        let (_, offset, size) = *self.files.get(&id)?;
        Some(&self.bytes[offset..offset + size])
    }

    /// Every file ID, in ascending order.
    pub fn ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.files.keys().copied()
    }

    pub fn tables(&self) -> impl Iterator<Item = &DrsTable> {
        self.tables.iter()
    }

    /// Every file as (type, ID, byte range in the archive), in ID order.
    ///
    /// For indexing an archive once and dropping the parse: the ranges stay
    /// valid for the bytes the archive was parsed from.
    pub fn entries(&self) -> impl Iterator<Item = (&str, u32, std::ops::Range<usize>)> + '_ {
        self.files.iter().map(|(id, (table, offset, size))| {
            (self.tables[*table].kind(), *id, *offset..*offset + *size)
        })
    }

    /// The file type of file `id`, such as `slp`.
    pub fn kind_of(&self, id: u32) -> Option<&str> {
        let (table, _, _) = self.files.get(&id)?;
        Some(self.tables[*table].kind())
    }
}
