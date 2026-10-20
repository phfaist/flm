//! The host's in-process converter, which turns a PNG, JPEG, GIF or WebP image into a PNG
//! image.

use std::io::Cursor;

use flm_core::stdcore::features::graphics::{ConversionRequest, GraphicsConvertError, GraphicsConverter, ImageFormat};

/// The converter's name, `raster`, by which the `via` list of a conversion rule refers to
/// it.
pub const RASTER_CONVERTER: &str = "raster";

/// The in-process converter, named [`RASTER_CONVERTER`]: it turns a PNG, JPEG, GIF or WebP
/// image into a PNG image of the same size in pixels, in the program's own code.
///
/// Of an animated image, it converts the first frame. It reads no conversion option: a
/// raster image is not drawn at a resolution.
#[derive(Debug, Clone, Copy, Default)]
pub struct RasterConverter;

/// Returns the `image` crate's format for one of the four raster formats, and `None` for
/// an SVG drawing or a PDF file.
fn raster_kind(format: ImageFormat) -> Option<image::ImageFormat> {
    Some(match format {
        ImageFormat::Png => image::ImageFormat::Png,
        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
        ImageFormat::Gif => image::ImageFormat::Gif,
        ImageFormat::Webp => image::ImageFormat::WebP,
        ImageFormat::Svg | ImageFormat::Pdf => return None,
    })
}

impl GraphicsConverter for RasterConverter {
    fn name(&self) -> &str {
        RASTER_CONVERTER
    }

    fn offers(&self, from: ImageFormat, to: ImageFormat) -> bool {
        raster_kind(from).is_some() && to == ImageFormat::Png
    }

    fn convert(&self, request: &ConversionRequest<'_>) -> Result<Vec<u8>, GraphicsConvertError> {
        let (Some(from), true) = (raster_kind(request.from), request.to == ImageFormat::Png) else {
            return Err(GraphicsConvertError::new(format!(
                "this converter turns only a PNG, JPEG, GIF or WebP image into a PNG image, not {} into {}",
                request.from, request.to
            )));
        };
        let picture = image::load_from_memory_with_format(request.content, from)
            .map_err(|error| GraphicsConvertError::new(error.to_string()))?;
        let mut png = Vec::new();
        picture
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|error| GraphicsConvertError::new(error.to_string()))?;
        Ok(png)
    }
}
