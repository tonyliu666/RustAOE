//! SLP sprites, version `2.0N`: run-length encoded frames that index the
//! palette.
//!
//! Layout, all integers little-endian:
//!
//! - A 32-byte header: version `2.0N`, frame count, 24-byte comment.
//! - One 32-byte record per frame: offset of the command table, offset of
//!   the outline table, palette offset, properties, width, height, hotspot x,
//!   hotspot y. The hotspot is the pixel that stands on the ground point.
//! - Per frame, an outline table: for each row, the transparent pixels on the
//!   left and right (`u16` each), or `0x8000` for a fully transparent row.
//! - Per frame, a command table: for each row, the offset of its commands.
//! - The commands, one byte each plus operands, ending each row with `0x0F`.
//!
//! | Low bits | Command | Count |
//! | --- | --- | --- |
//! | `..00` | copy colours | byte >> 2 |
//! | `..01` | skip | byte >> 2 |
//! | `0010` | copy colours | (byte & 0xF0) << 4 \| next |
//! | `0011` | skip | (byte & 0xF0) << 4 \| next |
//! | `0110` | copy player colours | byte >> 4, or next if 0 |
//! | `0111` | fill one colour | byte >> 4, or next if 0 |
//! | `1010` | fill one player colour | byte >> 4, or next if 0 |
//! | `1011` | shadow | byte >> 4, or next if 0 |
//! | `1110` | extended: hints, outline pixels | |
//! | `1111` | end of row | |

use crate::Palette;
use std::fmt;

const HEADER: usize = 32;
const FRAME_RECORD: usize = 32;
const VERSION: &[u8; 4] = b"2.0N";
const TRANSPARENT_ROW: u16 = 0x8000;
/// The largest frame side accepted, in pixels. The original's largest
/// sprites are a few hundred pixels; a damaged header claiming more would
/// otherwise ask for gigabytes before anything else was checked.
pub const MAX_FRAME_SIDE: i32 = 4096;
/// Palette entries between one player's colours and the next.
const PLAYER_COLOUR_STRIDE: u16 = 16;

/// The colour of a shadow pixel: black at roughly half opacity.
pub const SHADOW: [u8; 4] = [0, 0, 0, 128];

/// A parsed sprite borrowing its bytes.
#[derive(Clone, Debug)]
pub struct Slp<'a> {
    bytes: &'a [u8],
    frames: Vec<SlpFrame>,
}

/// One frame's size, hotspot, and where its tables are.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SlpFrame {
    pub width: i32,
    pub height: i32,
    /// The pixel that stands on the ground point, from the top-left.
    pub hotspot_x: i32,
    pub hotspot_y: i32,
    commands_at: usize,
    outline_at: usize,
}

/// A decoded frame: `width * height` RGBA pixels, row by row.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    fn new(width: u32, height: u32) -> Image {
        Image {
            width,
            height,
            rgba: vec![0; width as usize * height as usize * 4],
        }
    }

    /// The pixel at (`x`, `y`). Panics outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let at = (y as usize * self.width as usize + x as usize) * 4;
        [
            self.rgba[at],
            self.rgba[at + 1],
            self.rgba[at + 2],
            self.rgba[at + 3],
        ]
    }

    fn set(&mut self, x: usize, y: usize, colour: [u8; 4]) {
        let at = (y * self.width as usize + x) * 4;
        self.rgba[at..at + 4].copy_from_slice(&colour);
    }
}

/// Why bytes could not be read or decoded as a sprite.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SlpError {
    /// Shorter than a table, offset, or command says.
    Truncated,
    /// The version is not `2.0N`.
    NotSlp,
    /// A frame size that is negative.
    BadFrame(usize),
    /// A frame wider or taller than [`MAX_FRAME_SIDE`].
    FrameTooLarge(usize),
    /// A frame number the sprite does not have.
    NoSuchFrame(usize),
    /// A row's commands draw more pixels than the frame is wide.
    RowOverflow { row: usize },
    /// A command byte the format does not define.
    UnknownCommand { row: usize, byte: u8 },
}

impl fmt::Display for SlpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlpError::Truncated => write!(f, "SLP sprite is truncated"),
            SlpError::NotSlp => write!(f, "not an SLP 2.0N sprite"),
            SlpError::BadFrame(frame) => write!(f, "SLP frame {frame} has a negative size"),
            SlpError::FrameTooLarge(frame) => {
                write!(
                    f,
                    "SLP frame {frame} is larger than {MAX_FRAME_SIDE} pixels a side"
                )
            }
            SlpError::NoSuchFrame(frame) => write!(f, "SLP sprite has no frame {frame}"),
            SlpError::RowOverflow { row } => {
                write!(f, "SLP row {row} draws past the frame's width")
            }
            SlpError::UnknownCommand { row, byte } => {
                write!(f, "SLP row {row} has unknown command {byte:#04x}")
            }
        }
    }
}

impl std::error::Error for SlpError {}

/// Reads little-endian integers and command bytes, failing with `Truncated`.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn u8(&mut self) -> Result<u8, SlpError> {
        let byte = *self.bytes.get(self.at).ok_or(SlpError::Truncated)?;
        self.at += 1;
        Ok(byte)
    }
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, SlpError> {
    let field = bytes.get(at..at + 2).ok_or(SlpError::Truncated)?;
    Ok(u16::from_le_bytes([field[0], field[1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, SlpError> {
    let field = bytes.get(at..at + 4).ok_or(SlpError::Truncated)?;
    Ok(u32::from_le_bytes([field[0], field[1], field[2], field[3]]))
}

fn i32_at(bytes: &[u8], at: usize) -> Result<i32, SlpError> {
    Ok(u32_at(bytes, at)? as i32)
}

/// What one command asks for.
enum Command {
    Copy(usize),
    Skip(usize),
    CopyPlayer(usize),
    Fill(usize),
    FillPlayer(usize),
    Shadow(usize),
    /// Outline pixels: shown in the original when a unit is behind a
    /// building. Drawn transparent here; they take up space all the same.
    Outline(usize),
    /// A rendering hint with no pixels.
    Hint,
    EndOfRow,
}

impl<'a> Slp<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Slp<'a>, SlpError> {
        if bytes.len() < HEADER {
            return Err(SlpError::Truncated);
        }
        if &bytes[0..4] != VERSION {
            return Err(SlpError::NotSlp);
        }
        let count = i32_at(bytes, 4)?.max(0) as usize;
        let records_end = FRAME_RECORD
            .checked_mul(count)
            .and_then(|size| size.checked_add(HEADER));
        if records_end.is_none_or(|end| end > bytes.len()) {
            return Err(SlpError::Truncated);
        }

        let mut frames = Vec::with_capacity(count);
        for index in 0..count {
            let at = HEADER + FRAME_RECORD * index;
            let frame = SlpFrame {
                commands_at: u32_at(bytes, at)? as usize,
                outline_at: u32_at(bytes, at + 4)? as usize,
                width: i32_at(bytes, at + 16)?,
                height: i32_at(bytes, at + 20)?,
                hotspot_x: i32_at(bytes, at + 24)?,
                hotspot_y: i32_at(bytes, at + 28)?,
            };
            if frame.width < 0 || frame.height < 0 {
                return Err(SlpError::BadFrame(index));
            }
            if frame.width > MAX_FRAME_SIDE || frame.height > MAX_FRAME_SIDE {
                return Err(SlpError::FrameTooLarge(index));
            }
            // Both per-row tables must fit, so decoding never reads past the
            // end and a short file is reported before any pixels are made.
            let tables = 4 * frame.height as usize;
            for table_at in [frame.outline_at, frame.commands_at] {
                if table_at
                    .checked_add(tables)
                    .is_none_or(|end| end > bytes.len())
                {
                    return Err(SlpError::Truncated);
                }
            }
            frames.push(frame);
        }
        Ok(Slp { bytes, frames })
    }

    pub fn frames(&self) -> impl Iterator<Item = &SlpFrame> {
        self.frames.iter()
    }

    /// Decodes frame `index` to RGBA, with player colours for `player`
    /// (1 to 8; 0 for Gaia).
    pub fn decode(&self, index: usize, palette: &Palette, player: u8) -> Result<Image, SlpError> {
        let frame = self.frames.get(index).ok_or(SlpError::NoSuchFrame(index))?;
        let (width, height) = (frame.width as usize, frame.height as usize);
        let mut image = Image::new(frame.width as u32, frame.height as u32);
        let player_offset = PLAYER_COLOUR_STRIDE * u16::from(player);
        let shift = |value: u8| (u16::from(value) + player_offset).min(255) as u8;
        let opaque = |index: u8| {
            let [r, g, b] = palette.rgb(index);
            [r, g, b, 255]
        };

        for y in 0..height {
            let left = u16_at(self.bytes, frame.outline_at + 4 * y)?;
            let right = u16_at(self.bytes, frame.outline_at + 4 * y + 2)?;
            if left == TRANSPARENT_ROW || right == TRANSPARENT_ROW {
                continue;
            }
            let mut reader = Reader {
                bytes: self.bytes,
                at: u32_at(self.bytes, frame.commands_at + 4 * y)? as usize,
            };
            let mut x = usize::from(left);
            loop {
                let command = read_command(&mut reader, y)?;
                let run = match command {
                    Command::EndOfRow => break,
                    Command::Hint => 0,
                    Command::Copy(n)
                    | Command::Skip(n)
                    | Command::CopyPlayer(n)
                    | Command::Fill(n)
                    | Command::FillPlayer(n)
                    | Command::Shadow(n)
                    | Command::Outline(n) => n,
                };
                if x + run > width {
                    return Err(SlpError::RowOverflow { row: y });
                }
                match command {
                    Command::Copy(n) => {
                        for dx in 0..n {
                            image.set(x + dx, y, opaque(reader.u8()?));
                        }
                    }
                    Command::CopyPlayer(n) => {
                        for dx in 0..n {
                            image.set(x + dx, y, opaque(shift(reader.u8()?)));
                        }
                    }
                    Command::Fill(n) => {
                        let colour = opaque(reader.u8()?);
                        (0..n).for_each(|dx| image.set(x + dx, y, colour));
                    }
                    Command::FillPlayer(n) => {
                        let colour = opaque(shift(reader.u8()?));
                        (0..n).for_each(|dx| image.set(x + dx, y, colour));
                    }
                    Command::Shadow(n) => (0..n).for_each(|dx| image.set(x + dx, y, SHADOW)),
                    Command::Skip(_) | Command::Outline(_) | Command::Hint | Command::EndOfRow => {}
                }
                x += run;
            }
        }
        Ok(image)
    }
}

/// Reads one command and its count, leaving the reader on its operands.
fn read_command(reader: &mut Reader<'_>, row: usize) -> Result<Command, SlpError> {
    let byte = reader.u8()?;
    // Counts in the top nibble, or in the next byte when the nibble is zero.
    let mut nibble_or_next = || -> Result<usize, SlpError> {
        match byte >> 4 {
            0 => Ok(usize::from(reader.u8()?)),
            n => Ok(usize::from(n)),
        }
    };
    let command = match byte & 0x03 {
        0x00 => Command::Copy(usize::from(byte >> 2)),
        0x01 => Command::Skip(usize::from(byte >> 2)),
        _ => match byte & 0x0F {
            0x02 => Command::Copy((usize::from(byte & 0xF0) << 4) | usize::from(reader.u8()?)),
            0x03 => Command::Skip((usize::from(byte & 0xF0) << 4) | usize::from(reader.u8()?)),
            0x06 => Command::CopyPlayer(nibble_or_next()?),
            0x07 => Command::Fill(nibble_or_next()?),
            0x0A => Command::FillPlayer(nibble_or_next()?),
            0x0B => Command::Shadow(nibble_or_next()?),
            0x0E => match byte >> 4 {
                0x0..=0x3 => Command::Hint,
                0x4 | 0x6 => Command::Outline(1),
                0x5 | 0x7 => Command::Outline(usize::from(reader.u8()?)),
                _ => return Err(SlpError::UnknownCommand { row, byte }),
            },
            0x0F => Command::EndOfRow,
            _ => return Err(SlpError::UnknownCommand { row, byte }),
        },
    };
    Ok(command)
}
