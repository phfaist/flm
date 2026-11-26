//! The images of the output `tty`, drawn as half-block characters: the size of an image in
//! terminal cells, and the rendering of its content at that size.
//!
//! An image is drawn as a grid of characters, one per terminal column and two pixel rows per
//! character: `▀` (upper half block), `▄` (lower half block), `█` (full block) and the
//! space. The glyphs alone show the image in two tones, so the grid means something as
//! plain text; the colors of each cell, which the stage `tty.paint` writes, show it in full.
//!
//! The size ([`grid`]). The library decides the physical size an image is shown at, the
//! image's `width` and `height` as the text backend receives them (each a decimal and a
//! unit, read in points by [`points`]). With both, they are the shown width and height;
//! with one, the other follows the decoded image's pixel aspect ratio; with neither, the
//! pixel size at 96 dots per inch. A terminal cell measures [`CellSize`]: by default 6
//! points wide (the text backend gives a column the width of 6 points) and 12 points high
//! (a terminal line is about twice as tall as a column is wide). The grid has the columns
//! and the lines that cover the shown size, at least one of each.
//!
//! The rendering ([`render`]). The decoded image ([`decode`]: PNG, JPEG, GIF or WebP, the
//! first frame of an animation) is laid over black, which shows through its transparent
//! pixels, and resampled with an averaging filter to `columns × 2·rows` pixels: to the
//! shown size, stretched when the given width and height have another ratio than the
//! image's. Each cell covers a top and a bottom pixel; a pixel is light when its relative
//! luminance (from its linear red, green and blue values, with the weights of sRGB) is at
//! least one half. The glyph shows which of the two pixels are light: both `█`, neither the
//! space, the top `▀`, the bottom `▄`. The colors complete it: under `▀` the foreground is
//! the top pixel and the background the bottom one, under `▄` the reverse, under `█` the
//! foreground is the average of the two and under the space the background is.

use base64::Engine as _;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, Rgb as Pixel, RgbImage};
use serde_json::Value;

/// The resolution at which an image of no known physical size is shown, in dots per inch:
/// the library's own assumption for an image whose file states none.
const ASSUMED_DPI: f64 = 96.0;

/// Points per inch.
const POINTS_PER_INCH: f64 = 72.0;

/// The physical size of one terminal cell, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellSize {
    /// The width of a cell: 6 points by default.
    pub width_pt: f64,
    /// The height of a cell: 12 points by default.
    pub height_pt: f64,
}

impl CellSize {
    /// The default width of a cell, in points: the text backend's column, a sixth of a
    /// twelve-point em.
    pub const DEFAULT_WIDTH_PT: f64 = 6.0;

    /// The default height of a cell, in points: twice its default width.
    pub const DEFAULT_HEIGHT_PT: f64 = 12.0;
}

impl Default for CellSize {
    fn default() -> CellSize {
        CellSize { width_pt: CellSize::DEFAULT_WIDTH_PT, height_pt: CellSize::DEFAULT_HEIGHT_PT }
    }
}

/// A color given by its red, green and blue values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// The colors of one cell of a [`Graphics`] grid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CellColors {
    /// The foreground color, the color of the glyph; `None` for the space.
    pub fg: Option<Rgb>,
    /// The background color; `None` for `█`.
    pub bg: Option<Rgb>,
}

/// An image rendered as half-block characters ([`render`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graphics {
    /// The rows of glyphs, top to bottom, each of as many characters as the grid has
    /// columns.
    pub glyphs: Vec<String>,
    /// The colors of each cell, row by row, in the order of [`glyphs`](Graphics::glyphs).
    pub colors: Vec<Vec<CellColors>>,
}

/// Why an image's content cannot be drawn ([`decode`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotDecoded {
    /// The media type is none of PNG, JPEG, GIF and WebP.
    Format,
    /// The content is not a valid image of its media type: the decoder's detail.
    Invalid(String),
}

/// Decodes `content`, of the media type `media_type`: a PNG, JPEG, GIF or WebP image (the
/// first frame of an animation).
///
/// # Errors
///
/// [`NotDecoded::Format`] for another media type; [`NotDecoded::Invalid`] when the content
/// is not a valid image of its type.
pub fn decode(media_type: &str, content: &[u8]) -> Result<DynamicImage, NotDecoded> {
    let format = match media_type {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::WebP,
        _ => return Err(NotDecoded::Format),
    };
    image::load_from_memory_with_format(content, format).map_err(|error| NotDecoded::Invalid(error.to_string()))
}

/// Reads `length`, a decimal followed by one of the units `pt`, `mm`, `cm` and `in`, in
/// points; `None` for another text, and for a length that is not positive.
pub fn points(length: &str) -> Option<f64> {
    let split = length.find(|c: char| c.is_ascii_alphabetic())?;
    let (amount, unit) = length.split_at(split);
    let per_unit = match unit {
        "pt" => 1.0,
        "mm" => POINTS_PER_INCH / 25.4,
        "cm" => POINTS_PER_INCH / 2.54,
        "in" => POINTS_PER_INCH,
        _ => return None,
    };
    let amount: f64 = amount.parse().ok()?;
    let points = amount * per_unit;
    (points.is_finite() && points > 0.0).then_some(points)
}

/// The grid an image is drawn on, `(columns, rows)`: the shown width `width` and height
/// `height` in points, when given, completed from the image's size in pixels `pixels`
/// (`(width, height)`), divided by the cell's size `cell` and rounded, at least one of
/// each.
pub fn grid(width: Option<f64>, height: Option<f64>, pixels: (u32, u32), cell: CellSize) -> (u32, u32) {
    let pixel_width = f64::from(pixels.0.max(1));
    let pixel_height = f64::from(pixels.1.max(1));
    let (width, height) = match (width, height) {
        (Some(width), Some(height)) => (width, height),
        (Some(width), None) => (width, width * pixel_height / pixel_width),
        (None, Some(height)) => (height * pixel_width / pixel_height, height),
        (None, None) => {
            let scale = POINTS_PER_INCH / ASSUMED_DPI;
            (pixel_width * scale, pixel_height * scale)
        }
    };
    (cells(width, cell.width_pt), cells(height, cell.height_pt))
}

/// The number of cells of `size` points that `length` points covers, rounded, at least one.
fn cells(length: f64, size: f64) -> u32 {
    // The conversion saturates at the bounds of `u32`.
    (length / size).round().max(1.0) as u32
}

/// Renders `image` on a grid of `columns` by `rows` cells (see the
/// [module documentation](self)).
pub fn render(image: &DynamicImage, columns: u32, rows: u32) -> Graphics {
    let columns = columns.max(1);
    let rows = rows.max(1);
    let opaque = over_black(image);
    let pixels = image::imageops::resize(&opaque, columns, 2 * rows, FilterType::Triangle);
    let mut glyphs = Vec::new();
    let mut colors = Vec::new();
    for row in 0..rows {
        let mut line = String::new();
        let mut line_colors = Vec::new();
        for column in 0..columns {
            let top = pixels.get_pixel(column, 2 * row);
            let bottom = pixels.get_pixel(column, 2 * row + 1);
            let (glyph, cell) = cell(top, bottom);
            line.push(glyph);
            line_colors.push(cell);
        }
        glyphs.push(line);
        colors.push(line_colors);
    }
    Graphics { glyphs, colors }
}

/// `image` laid over black: each pixel's color scaled by its opacity.
fn over_black(image: &DynamicImage) -> RgbImage {
    let rgba = image.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let scale = |c: u8| ((u16::from(c) * u16::from(a) + 127) / 255) as u8;
        Pixel([scale(r), scale(g), scale(b)])
    })
}

/// The glyph and the colors of a cell whose top pixel is `top` and whose bottom pixel is
/// `bottom`.
fn cell(top: &Pixel<u8>, bottom: &Pixel<u8>) -> (char, CellColors) {
    let t = Rgb(top.0[0], top.0[1], top.0[2]);
    let b = Rgb(bottom.0[0], bottom.0[1], bottom.0[2]);
    match (is_light(t), is_light(b)) {
        (true, true) => ('█', CellColors { fg: Some(average(t, b)), bg: None }),
        (false, false) => (' ', CellColors { fg: None, bg: Some(average(t, b)) }),
        (true, false) => ('▀', CellColors { fg: Some(t), bg: Some(b) }),
        (false, true) => ('▄', CellColors { fg: Some(b), bg: Some(t) }),
    }
}

/// Whether `color`'s relative luminance is at least one half.
fn is_light(color: Rgb) -> bool {
    let Rgb(r, g, b) = color;
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b) >= 0.5
}

/// The linear value of the sRGB component `c`, from 0 to 1.
fn linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// The average of two colors, each component rounded.
fn average(a: Rgb, b: Rgb) -> Rgb {
    let mean = |x: u8, y: u8| (u16::from(x) + u16::from(y)).div_ceil(2) as u8;
    Rgb(mean(a.0, b.0), mean(a.1, b.1), mean(a.2, b.2))
}

/// The bytes of `content`, a file's content in the data form an extension receives it in:
/// `{"text": …}`, the bytes of the text, or `{"base64": …}`, the bytes in base64 (the
/// standard alphabet, with padding); `None` for another value.
pub fn content_bytes(content: &Value) -> Option<Vec<u8>> {
    if let Some(text) = content.get("text").and_then(Value::as_str) {
        return Some(text.as_bytes().to_vec());
    }
    base64::engine::general_purpose::STANDARD.decode(content.get("base64")?.as_str()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, Luma, RgbaImage};
    use serde_json::json;

    const WHITE: Rgb = Rgb(255, 255, 255);
    const BLACK: Rgb = Rgb(0, 0, 0);

    fn rgb_image(width: u32, height: u32, pixels: &[Rgb]) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
            let Rgb(r, g, b) = pixels[(y * width + x) as usize];
            Pixel([r, g, b])
        }))
    }

    #[test]
    fn a_checkerboard_gives_a_half_block_of_each_kind_with_its_colors() {
        let image = rgb_image(2, 2, &[WHITE, BLACK, BLACK, WHITE]);
        let graphics = render(&image, 2, 1);
        assert_eq!(graphics.glyphs, ["▀▄"]);
        assert_eq!(
            graphics.colors,
            [[CellColors { fg: Some(WHITE), bg: Some(BLACK) }, CellColors { fg: Some(WHITE), bg: Some(BLACK) }]]
        );
    }

    #[test]
    fn all_light_gives_full_blocks_and_all_dark_gives_spaces() {
        let light = rgb_image(2, 2, &[WHITE, Rgb(200, 200, 200), WHITE, Rgb(200, 220, 240)]);
        let graphics = render(&light, 2, 1);
        assert_eq!(graphics.glyphs, ["██"]);
        assert_eq!(graphics.colors[0][0], CellColors { fg: Some(WHITE), bg: None });
        assert_eq!(graphics.colors[0][1], CellColors { fg: Some(Rgb(200, 210, 220)), bg: None });

        let dark = rgb_image(2, 2, &[BLACK, Rgb(10, 20, 30), Rgb(20, 0, 0), Rgb(90, 90, 90)]);
        let graphics = render(&dark, 2, 1);
        assert_eq!(graphics.glyphs, ["  "]);
        assert_eq!(graphics.colors[0][0], CellColors { fg: None, bg: Some(Rgb(10, 0, 0)) });
        assert_eq!(graphics.colors[0][1], CellColors { fg: None, bg: Some(Rgb(50, 55, 60)) });
    }

    #[test]
    fn a_gradient_turns_light_where_the_luminance_reaches_one_half() {
        // A vertical gradient of grays: sRGB 188 is the first gray of luminance one half
        // or more.
        let grays = [0u8, 100, 187, 188, 220, 255];
        let image = DynamicImage::ImageLuma8(GrayImage::from_fn(1, 6, |_, y| Luma([grays[y as usize]])));
        let graphics = render(&image, 1, 3);
        assert_eq!(graphics.glyphs, [" ", "▄", "█"]);
        assert_eq!(graphics.colors[1][0], CellColors { fg: Some(Rgb(188, 188, 188)), bg: Some(Rgb(187, 187, 187)) });
        assert!(!is_light(Rgb(187, 187, 187)) && is_light(Rgb(188, 188, 188)));
    }

    #[test]
    fn an_odd_height_is_resampled_to_two_pixel_rows_per_line() {
        let image = rgb_image(1, 3, &[WHITE, WHITE, WHITE]);
        let graphics = render(&image, 1, 2);
        assert_eq!(graphics.glyphs, ["█", "█"]);
        assert_eq!(graphics.colors.len(), 2);
    }

    #[test]
    fn a_transparent_pixel_shows_black() {
        let image = DynamicImage::ImageRgba8(RgbaImage::from_fn(1, 2, |_, y| {
            image::Rgba(if y == 0 { [255, 255, 255, 255] } else { [255, 255, 255, 0] })
        }));
        let graphics = render(&image, 1, 1);
        assert_eq!(graphics.glyphs, ["▀"]);
        assert_eq!(graphics.colors[0][0], CellColors { fg: Some(WHITE), bg: Some(BLACK) });
    }

    #[test]
    fn a_length_is_read_in_points() {
        assert_eq!(points("72pt"), Some(72.0));
        assert_eq!(points("1in"), Some(72.0));
        assert!((points("2.54cm").unwrap() - 72.0).abs() < 1e-9);
        assert!((points("25.4mm").unwrap() - 72.0).abs() < 1e-9);
        assert_eq!(points("0.5in"), Some(36.0));
        for not_read in ["1em", "2tab", "pt", "1", "", "0pt", "-1pt", "1 pt"] {
            assert_eq!(points(not_read), None, "{not_read}");
        }
    }

    #[test]
    fn the_grid_follows_the_width_and_the_height_given() {
        let cell = CellSize::default();
        assert_eq!(grid(points("1in"), points("0.5in"), (10, 10), cell), (12, 3));
        assert_eq!(grid(points("72pt"), points("144pt"), (10, 10), cell), (12, 12));
    }

    #[test]
    fn with_one_of_them_the_other_follows_the_pixel_aspect_ratio() {
        let cell = CellSize::default();
        // 1in wide, 200 by 100 pixels: half an inch high, 36 points, 3 lines.
        assert_eq!(grid(points("1in"), None, (200, 100), cell), (12, 3));
        // 1in high, 100 by 200 pixels: half an inch wide, 36 points, 6 columns.
        assert_eq!(grid(None, points("1in"), (100, 200), cell), (6, 6));
    }

    #[test]
    fn with_neither_the_pixels_are_read_at_96_dots_per_inch() {
        // 96 by 48 pixels: an inch by half an inch.
        assert_eq!(grid(None, None, (96, 48), CellSize::default()), (12, 3));
        // A tiny image takes one cell.
        assert_eq!(grid(None, None, (1, 1), CellSize::default()), (1, 1));
    }

    #[test]
    fn the_cell_size_divides_the_shown_size() {
        assert_eq!(CellSize::default(), CellSize { width_pt: 6.0, height_pt: 12.0 });
        let cell = CellSize { width_pt: 9.0, height_pt: 18.0 };
        assert_eq!(grid(points("1in"), points("1in"), (1, 1), cell), (8, 4));
    }

    #[test]
    fn a_media_type_outside_the_four_formats_is_not_decoded() {
        assert_eq!(decode("image/svg+xml", b"<svg/>").unwrap_err(), NotDecoded::Format);
        assert!(matches!(decode("image/png", b"not a png"), Err(NotDecoded::Invalid(_))));
        let mut png = Vec::new();
        rgb_image(3, 2, &[WHITE; 6]).write_to(&mut std::io::Cursor::new(&mut png), ImageFormat::Png).unwrap();
        let decoded = decode("image/png", &png).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (3, 2));
    }

    #[test]
    fn content_is_read_from_text_or_base64() {
        assert_eq!(content_bytes(&json!({"text": "<svg/>"})).unwrap(), b"<svg/>");
        assert_eq!(content_bytes(&json!({"base64": "AAEC/w=="})).unwrap(), [0, 1, 2, 255]);
        assert_eq!(content_bytes(&json!("text")), None);
    }
}
