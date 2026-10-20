//! The host's inspector: what an image file states about its size, read from the content
//! that the graphics resolver read.

use std::io::Cursor;

use flm_core::stdcore::backends::pres::{I48F16, Length, LengthUnit};
use flm_core::stdcore::features::graphics::{
    Dimensions, GraphicsInspectError, GraphicsInspectRequest, GraphicsInspectedInformation, GraphicsInspector,
    ImageFormat, PixelSize,
};

/// The host's inspector: it reads the size of a PNG, JPEG, GIF or WebP image, of an SVG
/// drawing and of a PDF file's first page, from the content that the graphics resolver
/// read. See the [module
/// documentation](super#what-the-inspector-reads-from-an-image-file).
///
/// It needs that content: where no graphics resolver is registered, it reports for every
/// image that the content cannot be read.
#[derive(Debug, Clone, Copy, Default)]
pub struct FileInspector;

impl GraphicsInspector for FileInspector {
    fn inspect(&self, request: &GraphicsInspectRequest) -> Result<GraphicsInspectedInformation, GraphicsInspectError> {
        let Some(resolved) = &request.resolved else {
            return Err(GraphicsInspectError::new(
                "its content must be read to measure it, and the program provides no way to read image files",
            ));
        };
        let content = &resolved.content[..];
        match resolved.file.format() {
            Some(format @ (ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::Webp)) => {
                raster(content, format)
            }
            Some(ImageFormat::Svg) => svg(content),
            Some(ImageFormat::Pdf) => pdf(content),
            None => Err(GraphicsInspectError::new(
                "it must be a PNG, JPEG, GIF or WebP image, an SVG drawing or a PDF file",
            )),
        }
    }
}

/// Returns what a raster image states: its size in pixels, read by the crate `image`, and
/// the resolution that its file states, in dots per inch.
fn raster(content: &[u8], format: ImageFormat) -> Result<GraphicsInspectedInformation, GraphicsInspectError> {
    let kind = match format {
        ImageFormat::Png => image::ImageFormat::Png,
        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
        ImageFormat::Gif => image::ImageFormat::Gif,
        _ => image::ImageFormat::WebP,
    };
    let reader = image::ImageReader::with_format(Cursor::new(content), kind);
    let (width, height) = reader.into_dimensions().map_err(|error| GraphicsInspectError::new(error.to_string()))?;
    let dpi = match format {
        ImageFormat::Png => png_resolution(content)?,
        ImageFormat::Jpeg => jpeg_resolution(content)?,
        // A GIF file cannot state a resolution; a WebP file states none the host reads.
        _ => None,
    };
    Ok(GraphicsInspectedInformation::Raster { dpi, size: PixelSize { width, height }, physical_dimensions: None })
}

/// Returns the resolution `dpi` as the inspector reports it: rounded to the nearest 1/128
/// dot per inch, so that a resolution stated per meter (3780 dots per meter, 96.012 dots
/// per inch) gives the size an author expects.
fn rounded_dpi(dpi: f64) -> I48F16 {
    // 1/128 is 2^-7; the answer's sixteen fractional bits hold it exactly.
    // See decision rationale at [§flm-dd-dr:host-graphics-inspector].
    I48F16::from_bits(((dpi * 128.0).round() as i64) << 9)
}

/// Returns one resolution from the two that a file states for its two directions, which
/// must be equal to a hundredth of a dot per inch.
fn one_resolution(x: f64, y: f64) -> Result<Option<I48F16>, GraphicsInspectError> {
    if (x - y).abs() > 1e-2 {
        return Err(GraphicsInspectError::new(format!(
            "its horizontal and vertical resolutions must be equal, and are {x} and {y} dots per inch"
        )));
    }
    Ok(Some(rounded_dpi(x)))
}

/// Returns the resolution that a PNG file states in its `pHYs` chunk, where the chunk
/// states one in dots per meter; a chunk that states only the aspect of a pixel gives
/// none.
fn png_resolution(content: &[u8]) -> Result<Option<I48F16>, GraphicsInspectError> {
    let reader = png::Decoder::new(Cursor::new(content))
        .read_info()
        .map_err(|error| GraphicsInspectError::new(error.to_string()))?;
    match reader.info().pixel_dims {
        Some(png::PixelDimensions { xppu, yppu, unit: png::Unit::Meter }) => {
            one_resolution(f64::from(xppu) * 0.0254, f64::from(yppu) * 0.0254)
        }
        _ => Ok(None),
    }
}

/// Returns the resolution that a JPEG file states: in its JFIF header, in dots per inch or
/// per centimeter; else in its EXIF data, where both the resolution and its unit are
/// given.
fn jpeg_resolution(content: &[u8]) -> Result<Option<I48F16>, GraphicsInspectError> {
    if let Some((unit, x, y)) = jfif_density(content) {
        let per_inch = match unit {
            1 => Some(1.0),
            2 => Some(2.54),
            // 0: the numbers give the aspect of a pixel only.
            _ => None,
        };
        if let Some(factor) = per_inch {
            return one_resolution(f64::from(x) * factor, f64::from(y) * factor);
        }
    }
    Ok(exif_resolution(content).map(rounded_dpi))
}

/// Returns the density that a JPEG file's JFIF segment (`APP0`, `JFIF\0`) states: its
/// unit, and the horizontal and vertical densities. None of the crates that the host uses
/// gives this density to the host, and it is a fixed header at the start of the file.
fn jfif_density(content: &[u8]) -> Option<(u8, u16, u16)> {
    // The start-of-image marker, then segments: a marker `FF xx` and a length in two
    // bytes that counts itself. The JFIF segment comes first when there is one.
    let rest = content.strip_prefix(&[0xFF, 0xD8])?;
    let (marker, rest) = rest.split_at_checked(2)?;
    if marker != [0xFF, 0xE0] {
        return None;
    }
    let length = usize::from(u16::from_be_bytes([*rest.first()?, *rest.get(1)?]));
    let segment = rest.get(2..length)?;
    let fields = segment.strip_prefix(b"JFIF\0")?;
    // Version (two bytes), unit, horizontal density, vertical density.
    let unit = *fields.get(2)?;
    let x = u16::from_be_bytes([*fields.get(3)?, *fields.get(4)?]);
    let y = u16::from_be_bytes([*fields.get(5)?, *fields.get(6)?]);
    Some((unit, x, y))
}

/// Returns the horizontal resolution that a JPEG file's EXIF data states, in dots per
/// inch, where the data gives both the resolution and its unit (inches, or centimeters).
fn exif_resolution(content: &[u8]) -> Option<f64> {
    let exif = exif::Reader::new().read_from_container(&mut Cursor::new(content)).ok()?;
    let unit = exif.get_field(exif::Tag::ResolutionUnit, exif::In::PRIMARY)?.value.get_uint(0)?;
    let exif::Value::Rational(ref resolution) = exif.get_field(exif::Tag::XResolution, exif::In::PRIMARY)?.value
    else {
        return None;
    };
    let resolution = resolution.first()?;
    if resolution.denom == 0 {
        return None;
    }
    let dpi = resolution.to_f64();
    Some(if unit == 3 { dpi * 2.54 } else { dpi })
}

/// Returns what an SVG drawing states: the size that its root element's `width` and
/// `height` give, each a number followed by one of the units `pt`, `px` (1/96 inch), `in`,
/// `cm`, `mm` or `m`, or by no unit, which is read as points.
fn svg(content: &[u8]) -> Result<GraphicsInspectedInformation, GraphicsInspectError> {
    let text = std::str::from_utf8(content)
        .map_err(|_| GraphicsInspectError::new("the SVG drawing must be text encoded as UTF-8"))?;
    let document = roxmltree::Document::parse(text).map_err(|error| GraphicsInspectError::new(error.to_string()))?;
    let root = document.root_element();
    let length = |name: &str| {
        let written = root.attribute(name).ok_or_else(|| {
            GraphicsInspectError::new(format!("the SVG drawing's root element must state its `{name}`"))
        })?;
        svg_length(written).ok_or_else(|| {
            GraphicsInspectError::new(format!(
                "the `{name}` of the SVG drawing must be a number followed by pt, px, in, cm, mm, m or no \
                 unit, not `{written}`"
            ))
        })
    };
    let physical_dimensions = Dimensions { width: length("width")?, height: length("height")? };
    Ok(GraphicsInspectedInformation::Vector { physical_dimensions })
}

/// Returns the length `written` as an SVG root element writes it, in the unit written
/// where the library has that unit; a length in pixels (1/96 inch) is given in points, and
/// a length in meters in centimeters.
fn svg_length(written: &str) -> Option<Length> {
    let written = written.trim();
    let split = written.find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-')));
    let (number, unit) = written.split_at(split.unwrap_or(written.len()));
    let number: f64 = number.parse().ok()?;
    let (amount, unit) = match unit.trim() {
        "" | "pt" => (number, LengthUnit::Points),
        "px" => (number * 0.75, LengthUnit::Points),
        "in" => (number, LengthUnit::Inches),
        "cm" => (number, LengthUnit::Centimeters),
        "mm" => (number, LengthUnit::Millimeters),
        "m" => (number * 100.0, LengthUnit::Centimeters),
        _ => return None,
    };
    Some(Length::new(I48F16::checked_from_num(amount)?, unit))
}

/// Returns what a PDF file states: the size of its first page, the page's media box
/// scaled by its user unit, in points.
fn pdf(content: &[u8]) -> Result<GraphicsInspectedInformation, GraphicsInspectError> {
    let document = lopdf::Document::load_mem(content).map_err(|error| GraphicsInspectError::new(error.to_string()))?;
    let page = *document
        .get_pages()
        .values()
        .next()
        .ok_or_else(|| GraphicsInspectError::new("the PDF file must have a page"))?;
    let unreadable = |error: lopdf::Error| GraphicsInspectError::new(format!("its first page cannot be read: {error}"));
    let page_dictionary = document.get_dictionary(page).map_err(unreadable)?;
    let user_unit = match page_dictionary.get(b"UserUnit") {
        Ok(unit) => number(unit).ok_or_else(|| GraphicsInspectError::new("the first page's user unit must be a number"))?,
        Err(_) => 1.0,
    };
    // The media box may be stated by the page, or inherited from the page tree above it. The
    // climb stops at a dictionary already visited, so that a page tree whose parents loop is
    // answered like a page with no media box rather than climbed forever.
    let no_media_box = || GraphicsInspectError::new("the PDF file's first page must state its media box");
    let mut visited = std::collections::HashSet::from([page]);
    let mut dictionary = page_dictionary;
    let media_box = loop {
        if let Ok(object) = dictionary.get(b"MediaBox") {
            break document.dereference(object).map_err(unreadable)?.1.as_array().map_err(unreadable)?;
        }
        let parent = dictionary.get(b"Parent").and_then(lopdf::Object::as_reference).map_err(|_| no_media_box())?;
        if !visited.insert(parent) {
            return Err(no_media_box());
        }
        dictionary = document.get_dictionary(parent).map_err(unreadable)?;
    };
    let corners: Vec<f64> = media_box.iter().filter_map(number).collect();
    let [left, bottom, right, top] = corners[..] else {
        return Err(GraphicsInspectError::new("the first page's media box must be four numbers"));
    };
    let points = |amount: f64| {
        I48F16::checked_from_num(amount * user_unit)
            .map(|amount| Length::new(amount, LengthUnit::Points))
            .ok_or_else(|| GraphicsInspectError::new("the first page's size is too large"))
    };
    let physical_dimensions = Dimensions { width: points((right - left).abs())?, height: points((top - bottom).abs())? };
    Ok(GraphicsInspectedInformation::Vector { physical_dimensions })
}

/// Returns the value of a PDF number, whole or real.
fn number(object: &lopdf::Object) -> Option<f64> {
    match object {
        lopdf::Object::Integer(value) => Some(*value as f64),
        lopdf::Object::Real(value) => Some(f64::from(*value)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_svg_length_is_a_number_and_one_of_the_units() {
        let pt = |amount: f64| Some(Length::new(I48F16::from_num(amount), LengthUnit::Points));
        assert_eq!(svg_length("12"), pt(12.0));
        assert_eq!(svg_length(" 96px "), pt(72.0));
        assert_eq!(svg_length("1.5e1pt"), pt(15.0));
        assert_eq!(svg_length("2m"), Some(Length::new(I48F16::from_num(200), LengthUnit::Centimeters)));
        assert_eq!(svg_length("10%"), None);
        assert_eq!(svg_length("em"), None);
    }

    #[test]
    fn a_resolution_is_rounded_to_a_128th() {
        assert_eq!(rounded_dpi(3780.0 * 0.0254), I48F16::from_num(96.015625));
        assert!(one_resolution(72.0, 96.0).is_err());
    }
}
