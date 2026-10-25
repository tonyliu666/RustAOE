//! Palettes: the 256 colours that sprite pixels index into.
//!
//! The game's palette is a JASC-PAL text file inside `interfac.drs`. These
//! tests write palettes by hand.

use assets::{Palette, PaletteError};

/// A JASC-PAL palette of 256 colours, colour `i` being `(i, 255 - i, 7)`
/// except where `overrides` says otherwise, with `newline` between lines.
fn palette_text(newline: &str, overrides: &[(usize, &str)]) -> String {
    let mut lines = vec![
        "JASC-PAL".to_string(),
        "0100".to_string(),
        "256".to_string(),
    ];
    for i in 0..256 {
        let line = overrides
            .iter()
            .find(|(at, _)| *at == i)
            .map_or(format!("{i} {} 7", 255 - i), |(_, text)| text.to_string());
        lines.push(line);
    }
    lines.join(newline) + newline
}

#[test]
fn a_palette_parses_into_256_colours() {
    let palette = Palette::parse(palette_text("\r\n", &[]).as_bytes()).expect("valid");

    assert_eq!(palette.rgb(0), [0, 255, 7]);
    assert_eq!(palette.rgb(1), [1, 254, 7]);
    assert_eq!(palette.rgb(255), [255, 0, 7]);
}

#[test]
fn the_games_second_colour_reads_back() {
    // The first two colours of the real palette, 50500 in interfac.drs.
    let text = palette_text("\r\n", &[(0, "0 0 0"), (1, "0 74 161")]);
    let palette = Palette::parse(text.as_bytes()).expect("valid");

    assert_eq!(palette.rgb(1), [0, 74, 161]);
}

#[test]
fn unix_line_endings_parse_too() {
    let palette = Palette::parse(palette_text("\n", &[]).as_bytes()).expect("valid");

    assert_eq!(palette.rgb(200), [200, 55, 7]);
}

#[test]
fn a_file_without_the_jasc_header_is_an_error() {
    let text = palette_text("\r\n", &[]).replacen("JASC-PAL", "NOT-PAL", 1);

    assert_eq!(
        Palette::parse(text.as_bytes()).err(),
        Some(PaletteError::NotJascPal)
    );
}

#[test]
fn a_count_other_than_256_is_an_error() {
    let text = palette_text("\r\n", &[]).replacen("\r\n256\r\n", "\r\n16\r\n", 1);

    assert_eq!(
        Palette::parse(text.as_bytes()).err(),
        Some(PaletteError::WrongCount(16))
    );
}

#[test]
fn fewer_colour_lines_than_the_count_is_an_error() {
    let text = palette_text("\r\n", &[]);
    let cut: String = text.lines().take(3 + 100).collect::<Vec<_>>().join("\r\n");

    assert_eq!(
        Palette::parse(cut.as_bytes()).err(),
        Some(PaletteError::MissingColours { found: 100 })
    );
}

#[test]
fn a_value_above_255_is_an_error() {
    let text = palette_text("\r\n", &[(9, "1 2 300")]);

    assert_eq!(
        Palette::parse(text.as_bytes()).err(),
        Some(PaletteError::BadColour { index: 9 })
    );
}

#[test]
fn a_line_that_is_not_three_numbers_is_an_error() {
    for bad in ["1 2", "1 2 3 4", "red green blue", ""] {
        let text = palette_text("\r\n", &[(42, bad)]);

        assert_eq!(
            Palette::parse(text.as_bytes()).err(),
            Some(PaletteError::BadColour { index: 42 }),
            "line {bad:?} was accepted"
        );
    }
}
