//! The host's services for image files (cargo feature `graphics`): finding the file of an
//! image, measuring the image, and converting it into another format.
//!
//! The library's `graphics` feature ([`flm_core::stdcore::features::graphics`]) handles
//! `\includegraphics` and publishes each image on each output, but opens no file itself.
//! It declares three service slots for the services that do this work; a service is a
//! value that can be shared across threads which one feature supplies for another
//! feature, or for the embedding program, to use ([`flm_core::core::services`]):
//!
//! - a **graphics resolver** ([`GraphicsResolver`]), which finds the file that an image's
//!   path refers to and reads it;
//! - an **inspector** ([`GraphicsInspector`]), which reports what an image file states
//!   about its size, from the content it is given;
//! - **converters** ([`GraphicsConverter`]), each of which turns the content of an image
//!   of one format into another format.
//!
//! This module holds the host's implementation of each service. The feature
//! `graphics-files` of the package `stdmain` contributes them to the library's service slots
//! ([`crate::stdpkg::graphics_files`]).
//!
//! # Where an image file is looked for
//!
//! [`FileResolver`] looks for the image's path, as written, in each folder of a *search
//! path*, in order; the first folder that holds a file at that path gives the file. Each
//! folder is relative to the folder of the file in which the image is written (the
//! document, or a file that the document includes with `\input`), or absolute. A document
//! given as text, on the command line or on the standard input, has the current folder as
//! its folder. The default search path is that folder alone
//! ([`GraphicsFilesConfig`](crate::stdpkg::graphics_files::GraphicsFilesConfig)). No
//! extension is added to a path that has none. Every file is read through
//! [`crate::files`], by the build under way
//! ([`ServiceFiles`](crate::files::ServiceFiles)), so every path looked for, found or
//! not, is on that build's list of files read.
//!
//! The file's **format** is determined from its first bytes, else from the extension of
//! its name ([`media_type_of`]). Its **identity** is its path from the current folder, with `.` and
//! `..` folded away, so two spellings of one path give one identity. Under the strategy
//! `reference`, where an output reaches the file in its own folder, the **address** that
//! the output writes is the file's path as seen from the folder the output is written in
//! ([`OutputFolder`](crate::output::OutputFolder)).
//!
//! The library never asks the graphics resolver about a path that begins with a scheme
//! (`https://…`; [`names_scheme`](flm_core::stdcore::features::graphics::names_scheme)):
//! such a path is not a file.
//!
//! # What the inspector reads from an image file
//!
//! [`FileInspector`] reads the size that the file's content states:
//!
//! - of a PNG, JPEG, GIF or WebP image: its size in pixels, and its resolution in dots per
//!   inch where the file states one, a PNG file in its `pHYs` chunk, a JPEG file in its
//!   JFIF header or else in its EXIF data. A GIF file cannot state a resolution, and the
//!   inspector reads none from a WebP file. The resolution is rounded to the nearest 1/128
//!   dot per inch, and the horizontal and vertical resolutions must be equal (to a
//!   hundredth of a dot per inch). Where no resolution is stated, the library assumes 96
//!   dots per inch and reports a Warning;
//! - of an SVG drawing: the `width` and `height` of its root element, each a number and a
//!   unit, `pt`, `px` (1/96 inch), `in`, `cm`, `mm` or `m`; a number with no unit is read
//!   as points;
//! - of a PDF file: the media box of its first page, scaled by the page's user unit, in
//!   points.
//!
//! # Converting an image
//!
//! The feature `graphics-files` contributes two converters, in this order:
//!
//! - [`RasterConverter`], named `raster`, turns a PNG, JPEG, GIF or WebP image into a PNG
//!   image in the program's own code; of an animated image, it converts the first frame;
//! - [`MagickConverter`], named `magick`, turns an image of any format into any other by
//!   running ImageMagick's program `magick`, which must be installed on the machine.
//!   Contributed last, it converts every pair of formats that the first converter does not
//!   offer, such as a PDF drawing into a PNG image.
//!
//! The conversion rules of each output, a setting of the library's `graphics` feature,
//! say which image is converted into which format. The library also chooses the converter
//! for a rule: the first converter that the rule's `via` list allows (every converter,
//! where the list is empty) and that offers the rule's pair of formats, the converters
//! taken with those of the feature placed last in the feature order first, each feature's
//! in the order it contributed them, and the host's last
//! ([`Services::services_by_precedence_named`](flm_core::core::services::Services::services_by_precedence_named),
//! [`choose_converter`](flm_core::stdcore::features::graphics::choose_converter)). A
//! rule selects one of these two converters by writing its name, `raster` or `magick`, in
//! `via`.
//!
//! # Map of the items
//!
//! - [`FileResolver`], [`media_type_of`], [`UNKNOWN_MEDIA_TYPE`]: the graphics resolver.
//! - [`FileInspector`]: the inspector.
//! - [`RasterConverter`], [`RASTER_CONVERTER`], [`MagickConverter`],
//!   [`MAGICK_CONVERTER`]: the two converters.
//!
//! [`GraphicsResolver`]: flm_core::stdcore::features::graphics::GraphicsResolver
//! [`GraphicsInspector`]: flm_core::stdcore::features::graphics::GraphicsInspector
//! [`GraphicsConverter`]: flm_core::stdcore::features::graphics::GraphicsConverter

mod inspect;
mod magick;
mod raster;
mod resolve;

pub use inspect::FileInspector;
pub use magick::{MAGICK_CONVERTER, MagickConverter};
pub use raster::{RASTER_CONVERTER, RasterConverter};
pub use resolve::{FileResolver, UNKNOWN_MEDIA_TYPE, media_type_of};
