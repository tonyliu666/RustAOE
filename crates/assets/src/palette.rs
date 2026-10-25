//! JASC-PAL palettes: the 256 colours sprite pixels index into.
//!
//! The game's palette is file 50500 in `interfac.drs`, plain text:
//!
//! ```text
//! JASC-PAL
//! 0100
//! 256
//! 0 0 0
//! 0 74 161
//! ...
//! ```
//!
//! A header, a version, the colour count, then one `red green blue` line per
//! colour, each 0 to 255. The game writes Windows line endings; both kinds
//! are accepted.

use std::fmt;

/// The number of colours an 8-bit sprite can index.
const COLOURS: usize = 256;

/// 256 colours.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Palette {
    colours: Box<[[u8; 3]; COLOURS]>,
}

/// Why bytes could not be read as a palette.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PaletteError {
    /// Not text, or the first line is not `JASC-PAL`.
    NotJascPal,
    /// The count line is not 256. Sprites index all 256 entries.
    WrongCount(usize),
    /// Fewer colour lines than 256.
    MissingColours { found: usize },
    /// Colour `index` is not three numbers from 0 to 255.
    BadColour { index: usize },
}

impl fmt::Display for PaletteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaletteError::NotJascPal => write!(f, "not a JASC-PAL palette"),
            PaletteError::WrongCount(count) => {
                write!(f, "palette has {count} colours; sprites need {COLOURS}")
            }
            PaletteError::MissingColours { found } => {
                write!(f, "palette lists only {found} of {COLOURS} colours")
            }
            PaletteError::BadColour { index } => {
                write!(
                    f,
                    "palette colour {index} is not three numbers from 0 to 255"
                )
            }
        }
    }
}

impl std::error::Error for PaletteError {}

impl Palette {
    pub fn parse(bytes: &[u8]) -> Result<Palette, PaletteError> {
        let text = std::str::from_utf8(bytes).map_err(|_| PaletteError::NotJascPal)?;
        // `lines` splits on `\n` and drops a trailing `\r`, so both line
        // endings read the same.
        let mut lines = text.lines();
        if lines.next().map(str::trim) != Some("JASC-PAL") {
            return Err(PaletteError::NotJascPal);
        }
        let _version = lines.next();
        let count = lines
            .next()
            .and_then(|line| line.trim().parse::<usize>().ok())
            .ok_or(PaletteError::NotJascPal)?;
        if count != COLOURS {
            return Err(PaletteError::WrongCount(count));
        }

        let mut colours = Box::new([[0u8; 3]; COLOURS]);
        for (index, colour) in colours.iter_mut().enumerate() {
            let line = lines
                .next()
                .ok_or(PaletteError::MissingColours { found: index })?;
            *colour = parse_colour(line).ok_or(PaletteError::BadColour { index })?;
        }
        Ok(Palette { colours })
    }

    /// The colour at `index`.
    pub fn rgb(&self, index: u8) -> [u8; 3] {
        self.colours[usize::from(index)]
    }
}

/// `"0 74 161"` as `[0, 74, 161]`; `None` unless exactly three bytes.
fn parse_colour(line: &str) -> Option<[u8; 3]> {
    let mut parts = line.split_whitespace().map(|part| part.parse::<u8>().ok());
    let colour = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(colour)
}
