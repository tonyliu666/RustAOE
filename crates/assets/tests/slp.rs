//! SLP sprites: run-length encoded frames that index the palette.
//!
//! Each test builds a sprite byte by byte with the layout of the game's
//! files, so no game data is needed or committed.

use assets::{Image, Palette, Slp, SlpError, SHADOW};

/// A palette where colour `i` is `(i, 255 - i, 7)`, so every index decodes
/// to a colour that names it.
fn palette() -> Palette {
    let mut text = String::from("JASC-PAL\n0100\n256\n");
    for i in 0..256 {
        text += &format!("{i} {} 7\n", 255 - i);
    }
    Palette::parse(text.as_bytes()).expect("valid palette")
}

/// The colour palette index `i` decodes to, fully opaque.
fn colour(i: u8) -> [u8; 4] {
    [i, 255 - i, 7, 255]
}

const CLEAR: [u8; 4] = [0, 0, 0, 0];

/// One row of a frame: transparent pixels on the left and right, and the
/// drawing commands between them. `None` is a row marked fully transparent.
type Row = Option<(u16, u16, Vec<u8>)>;

struct Frame {
    width: i32,
    height: i32,
    hotspot: (i32, i32),
    rows: Vec<Row>,
}

/// A row of commands with no padding.
fn row(commands: &[u8]) -> Row {
    Some((0, 0, commands.to_vec()))
}

/// A frame `width` wide with one row per entry in `rows`.
fn frame(width: i32, rows: Vec<Row>) -> Frame {
    Frame {
        width,
        height: rows.len() as i32,
        hotspot: (0, 0),
        rows,
    }
}

/// Builds an SLP file.
///
/// Layout: a 32-byte header (version `2.0N`, frame count, comment), one
/// 32-byte record per frame (command table offset, outline table offset,
/// palette offset, properties, width, height, hotspot x, hotspot y), then for
/// each frame its outline table (left and right padding per row, `0x8000`
/// for a transparent row), its command table (offset of each row's
/// commands), and the commands.
fn build(frames: &[Frame]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(b"2.0N");
    out.extend((frames.len() as i32).to_le_bytes());
    out.extend([0u8; 24]);

    let records_end = 32 + 32 * frames.len();
    let mut bodies = Vec::new();
    let mut records = Vec::new();
    let mut at = records_end;
    for frame in frames {
        let height = frame.rows.len();
        let outline_at = at;
        let commands_table_at = outline_at + 4 * height;
        let mut row_commands_at = commands_table_at + 4 * height;
        let mut outline = Vec::new();
        let mut table = Vec::new();
        let mut commands = Vec::new();
        for row in &frame.rows {
            match row {
                Some((left, right, cmds)) => {
                    outline.extend(left.to_le_bytes());
                    outline.extend(right.to_le_bytes());
                    table.extend((row_commands_at as u32).to_le_bytes());
                    commands.extend(cmds);
                    row_commands_at += cmds.len();
                }
                None => {
                    outline.extend(0x8000u16.to_le_bytes());
                    outline.extend(0x8000u16.to_le_bytes());
                    table.extend((row_commands_at as u32).to_le_bytes());
                }
            }
        }
        records.extend((commands_table_at as u32).to_le_bytes());
        records.extend((outline_at as u32).to_le_bytes());
        records.extend(0u32.to_le_bytes());
        records.extend(0u32.to_le_bytes());
        records.extend(frame.width.to_le_bytes());
        records.extend(frame.height.to_le_bytes());
        records.extend(frame.hotspot.0.to_le_bytes());
        records.extend(frame.hotspot.1.to_le_bytes());
        let body = [outline, table, commands].concat();
        at += body.len();
        bodies.extend(body);
    }
    out.extend(records);
    out.extend(bodies);
    out
}

/// Decodes frame 0 of a one-frame sprite as player 1.
fn decode_one(frame: Frame) -> Image {
    let bytes = build(&[frame]);
    let slp = Slp::parse(&bytes).expect("a well-formed sprite");
    slp.decode(0, &palette(), 1).expect("a decodable frame")
}

fn row_pixels(image: &Image, y: u32) -> Vec<[u8; 4]> {
    (0..image.width).map(|x| image.pixel(x, y)).collect()
}

// --- commands ---------------------------------------------------------------

#[test]
fn a_short_copy_draws_its_palette_colours() {
    // 0b000011_00: copy 3 pixels.
    let image = decode_one(frame(3, vec![row(&[3 << 2, 10, 20, 30, 0x0F])]));

    assert_eq!(row_pixels(&image, 0), [colour(10), colour(20), colour(30)]);
}

#[test]
fn a_short_skip_leaves_pixels_transparent() {
    // Skip 2, then copy 1.
    let image = decode_one(frame(3, vec![row(&[(2 << 2) | 1, 1 << 2, 9, 0x0F])]));

    assert_eq!(row_pixels(&image, 0), [CLEAR, CLEAR, colour(9)]);
}

/// A long copy (`0x02`) or skip (`0x03`) of `count` pixels: the high four
/// bits of the count in the command's top nibble, the low eight next.
fn long_command(kind: u8, count: u16) -> [u8; 2] {
    [(((count >> 8) as u8) << 4) | kind, (count & 0xFF) as u8]
}

#[test]
fn a_long_copy_draws_more_than_63_pixels() {
    // 0x02 with the high count bits in the top nibble, low bits next: 300.
    let mut cmds = long_command(0x02, 300).to_vec();
    cmds.extend((0..300).map(|i| (i % 256) as u8));
    cmds.push(0x0F);
    let image = decode_one(frame(300, vec![row(&cmds)]));

    assert_eq!(image.pixel(0, 0), colour(0));
    assert_eq!(image.pixel(299, 0), colour((299 % 256) as u8));
}

#[test]
fn a_long_skip_leaves_more_than_63_pixels_transparent() {
    let cmds = [long_command(0x03, 100).as_slice(), &[1 << 2, 5, 0x0F]].concat();
    let image = decode_one(frame(101, vec![row(&cmds)]));

    assert_eq!(image.pixel(99, 0), CLEAR);
    assert_eq!(image.pixel(100, 0), colour(5));
}

#[test]
fn a_fill_repeats_one_colour() {
    // 0x07 with the count in the top nibble: four pixels of colour 77.
    let image = decode_one(frame(4, vec![row(&[(4 << 4) | 0x07, 77, 0x0F])]));

    assert_eq!(row_pixels(&image, 0), [colour(77); 4]);
}

#[test]
fn a_fill_with_a_zero_nibble_reads_its_count_from_the_next_byte() {
    let image = decode_one(frame(20, vec![row(&[0x07, 20, 77, 0x0F])]));

    assert_eq!(row_pixels(&image, 0), [colour(77); 20]);
}

#[test]
fn a_shadow_draws_translucent_black() {
    let image = decode_one(frame(2, vec![row(&[(2 << 4) | 0x0B, 0x0F])]));

    assert_eq!(row_pixels(&image, 0), [SHADOW; 2]);
}

#[test]
fn outline_commands_take_space_but_draw_nothing() {
    // 0x4E: one outline pixel; 0x5E: a run of outline pixels, count next.
    let image = decode_one(frame(5, vec![row(&[0x4E, 0x5E, 3, 1 << 2, 8, 0x0F])]));

    assert_eq!(
        row_pixels(&image, 0),
        [CLEAR, CLEAR, CLEAR, CLEAR, colour(8)]
    );
}

#[test]
fn render_hints_take_no_space() {
    // 0x0E and 0x1E are rendering hints with no pixels.
    let image = decode_one(frame(1, vec![row(&[0x0E, 0x1E, 1 << 2, 8, 0x0F])]));

    assert_eq!(row_pixels(&image, 0), [colour(8)]);
}

// --- player colour ----------------------------------------------------------

/// A sprite drawing two player-colour pixels, values 2 and 5.
fn player_sprite() -> Vec<u8> {
    build(&[frame(2, vec![row(&[(2 << 4) | 0x06, 2, 5, 0x0F])])])
}

#[test]
fn player_colour_is_shifted_into_the_players_range() {
    let bytes = player_sprite();
    let slp = Slp::parse(&bytes).expect("valid");

    let one = slp.decode(0, &palette(), 1).expect("decodes");
    let three = slp.decode(0, &palette(), 3).expect("decodes");

    // Each player's colours are 16 palette entries further along.
    assert_eq!(row_pixels(&one, 0), [colour(2 + 16), colour(5 + 16)]);
    assert_eq!(row_pixels(&three, 0), [colour(2 + 48), colour(5 + 48)]);
}

#[test]
fn a_player_colour_fill_repeats_the_shifted_colour() {
    let bytes = build(&[frame(3, vec![row(&[(3 << 4) | 0x0A, 4, 0x0F])])]);
    let slp = Slp::parse(&bytes).expect("valid");

    let two = slp.decode(0, &palette(), 2).expect("decodes");

    assert_eq!(row_pixels(&two, 0), [colour(4 + 32); 3]);
}

// --- rows and frames --------------------------------------------------------

#[test]
fn left_and_right_padding_are_transparent() {
    let image = decode_one(frame(5, vec![Some((2, 1, vec![(2 << 2), 40, 41, 0x0F]))]));

    assert_eq!(
        row_pixels(&image, 0),
        [CLEAR, CLEAR, colour(40), colour(41), CLEAR]
    );
}

#[test]
fn a_row_marked_transparent_stays_transparent() {
    let image = decode_one(frame(2, vec![None, row(&[2 << 2, 1, 2, 0x0F])]));

    assert_eq!(row_pixels(&image, 0), [CLEAR, CLEAR]);
    assert_eq!(row_pixels(&image, 1), [colour(1), colour(2)]);
}

#[test]
fn a_sprite_lists_each_frames_size_and_hotspot() {
    let mut first = frame(2, vec![row(&[2 << 2, 1, 2, 0x0F])]);
    first.hotspot = (1, 0);
    let mut second = frame(3, vec![row(&[3 << 2, 1, 2, 3, 0x0F]); 2]);
    second.hotspot = (-4, 7);
    let bytes = build(&[first, second]);

    let slp = Slp::parse(&bytes).expect("valid");

    let frames: Vec<_> = slp
        .frames()
        .map(|f| (f.width, f.height, f.hotspot_x, f.hotspot_y))
        .collect();
    assert_eq!(frames, [(2, 1, 1, 0), (3, 2, -4, 7)]);
    assert_eq!(
        slp.decode(1, &palette(), 1).expect("decodes").pixel(2, 1),
        colour(3)
    );
}

// --- errors -----------------------------------------------------------------

#[test]
fn a_file_with_another_version_is_an_error() {
    let mut bytes = build(&[frame(1, vec![row(&[1 << 2, 1, 0x0F])])]);
    bytes[0..4].copy_from_slice(b"3.0N");

    assert_eq!(Slp::parse(&bytes).err(), Some(SlpError::NotSlp));
}

#[test]
fn a_file_shorter_than_its_frame_list_is_an_error() {
    let bytes = build(&[frame(1, vec![row(&[1 << 2, 1, 0x0F])])]);

    assert_eq!(Slp::parse(&bytes[..40]).err(), Some(SlpError::Truncated));
}

#[test]
fn asking_for_a_frame_the_sprite_does_not_have_is_an_error() {
    let bytes = build(&[frame(1, vec![row(&[1 << 2, 1, 0x0F])])]);
    let slp = Slp::parse(&bytes).expect("valid");

    assert_eq!(
        slp.decode(1, &palette(), 1).err(),
        Some(SlpError::NoSuchFrame(1))
    );
}

#[test]
fn commands_running_past_the_row_are_an_error() {
    let bytes = build(&[frame(2, vec![row(&[3 << 2, 1, 2, 3, 0x0F])])]);
    let slp = Slp::parse(&bytes).expect("valid");

    assert_eq!(
        slp.decode(0, &palette(), 1).err(),
        Some(SlpError::RowOverflow { row: 0 })
    );
}

#[test]
fn a_row_with_no_end_marker_is_an_error() {
    // The last row's commands run off the end of the file.
    let bytes = build(&[frame(4, vec![row(&[1 << 2, 1])])]);
    let slp = Slp::parse(&bytes).expect("valid");

    assert_eq!(
        slp.decode(0, &palette(), 1).err(),
        Some(SlpError::Truncated)
    );
}

#[test]
fn a_command_byte_the_format_does_not_define_is_an_error() {
    // 0x8E is an extended command with no meaning.
    let bytes = build(&[frame(1, vec![row(&[0x8E, 0x0F])])]);
    let slp = Slp::parse(&bytes).expect("valid");

    assert_eq!(
        slp.decode(0, &palette(), 1).err(),
        Some(SlpError::UnknownCommand { row: 0, byte: 0x8E })
    );
}
