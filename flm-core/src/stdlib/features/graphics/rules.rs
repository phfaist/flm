//! This module defines the lowering rule [`image_rule`] of the `graphics`
//! feature, along with what every lowering rule of the image element shares:
//! the computation of the size at which an image is shown ([`shown_size`]),
//! and the warning [`UnknownSize`], which is reported when this size is
//! unknown.
//!
//! The rule [`image_rule`] lowers an image, that is, a node of the element
//! [`IMAGE_ELEMENT`] of `\includegraphics`, to a
//! [`flm.pres.image`](crate::stdlib::backends::pres::IMAGE) node. The
//! `graphics` feature registers this rule for the backends `text`, `html` and
//! `latex-rendered`. On the backend `latex-recomposed`, the rule
//! [`LATEX_RULE`](super::latex::LATEX_RULE) writes an image as a LaTeX hook
//! ([`latex`](mod@super::latex)).
//!
//! The payload of the `flm.pres.image` node ([`ImagePayload`]) contains the
//! following values:
//!
//! - `src`: the *published address* of the image, that is, the address at
//!   which the output refers to the image. The publish stage of the backend
//!   computes this address according to the strategy set for the output
//!   ([`publish`](mod@super::publish)).
//! - `display`: whether the image is a block or inline content. An image is a
//!   block when it is a direct child of the body of a float, and inline
//!   content everywhere else ([`GraphicsImage`](super::GraphicsImage)).
//! - `alt`: the alternative text, given with the option `alt`.
//! - `width` and `height`: the size at which the image is shown
//!   ([`shown_size`]).
//!
//! Each backend writes this node in its own way. For instance, the plain-text
//! output prints `\includegraphics[alt=A plot]{plot.png}` as
//! `[image: A plot]`, and the HTML output writes an `<img>` element. The
//! rendering guide describes the output of each backend: [images in plain
//! text](crate::guide::rendering#images), [images in
//! HTML](crate::guide::rendering#images-in-html), and [images in the rendered
//! LaTeX](crate::guide::rendering#images-in-the-rendered-latex).

use std::sync::Arc;

use techy::core::node::NodeRef;
use techy::error::{Diagnostic, DiagnosticInfo};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::lang::Flm;
use crate::model::names::ElementName;
use crate::model::aspects::{BlockRole, block_role_of};
use crate::stdlib::backends::pres::{self, ImageDisplay, ImagePayload, Length};

use super::inspect::{GraphicsInspectedInformation, GraphicsRecord, INFO};
use super::options::{ImageOptions, image_options, report};
use super::publish::{PublishedImage, published_table};
use super::strategy::{GraphicsRenderOptions, Magnification};
use super::{EmptyPath, IMAGE_ELEMENT, ImagePath, PathNotCharacters, read_path};

/// Returns the name of the lowering rule [`image_rule`] for `backend`:
/// `flm.graphics.image.lower.<backend>`.
pub fn rule_name(backend: &str) -> String {
    format!("{IMAGE_ELEMENT}.lower.{backend}")
}

/// Returns the width and the height at which an image is shown on an output.
/// A dimension is `None` when it is not set.
///
/// `options` contains the valid options of the image, `record` is the record
/// that the inspect stage wrote for the image into the table [`INFO`], and
/// `magnification` is the magnification of the output
/// ([`GraphicsRenderOptions::magnification`]). The size is first computed from
/// the options and from the physical size of the image file:
///
/// - When both `width` and `height` are given, the image has this size.
/// - When only one of them is given, the other dimension is computed from the
///   physical size of the image file, so that the image keeps its
///   proportions. The computed dimension has the unit of the given one
///   ([`Length::scaled`]). For instance, with `width=5cm`, an image file of
///   340pt by 170pt gets a height of 2.5cm.
/// - When neither is given, the image has the physical size of the image file.
///
/// The physical size of the image file is known only when `record` contains
/// inspected information ([`GraphicsRecord::inspected`]). Otherwise, only the
/// given dimensions are set. A computed dimension is also left unset when it
/// cannot be computed, because a dimension of the image file is zero or the
/// result is too large to be represented as a [`Length`].
///
/// Each dimension, including a given one, is then multiplied by a factor of
/// `magnification` ([`Length::times`]): the factor `raster` for a raster
/// image, and the factor `vector` for a vector image or for an image without
/// inspected information. A dimension that becomes too large to be
/// represented is left unset.
///
/// Every lowering rule of the image element computes the size with this
/// function. A lowering rule of your own can call it to show images at the
/// same size.
pub fn shown_size(
    options: &ImageOptions,
    record: Option<&GraphicsRecord>,
    magnification: &Magnification,
) -> (Option<Length>, Option<Length>) {
    let inspected = record.and_then(|record| record.inspected.as_ref());
    let factor = match inspected {
        Some(GraphicsInspectedInformation::Raster { .. }) => magnification.raster,
        Some(GraphicsInspectedInformation::Vector { .. }) | None => magnification.vector,
    };
    let (width, height) = match (
        options.width,
        options.height,
        inspected.and_then(GraphicsInspectedInformation::physical_dimensions),
    ) {
        (Some(width), Some(height), _) => (Some(width), Some(height)),
        (Some(width), None, Some(size)) => {
            (Some(width), size.height.scaled(&width, &size.width, width.unit).ok())
        }
        (None, Some(height), Some(size)) => {
            (size.width.scaled(&height, &size.height, height.unit).ok(), Some(height))
        }
        (None, None, Some(size)) => (Some(size.width), Some(size.height)),
        (width, height, None) => (width, height),
    };
    // One computation for every backend, the factor keyed by output: the
    // magnification is applied once, where the size is decided, and not
    // repeated in each backend, where the copies could drift apart.
    let magnify = |length: Option<Length>| length.and_then(|length| length.times(factor.value()).ok());
    (magnify(width), magnify(height))
}

/// The warning that is reported when the size of an image is unknown
/// (`flm.graphics.unknown-size`).
///
/// The size of an image is unknown when neither `width` nor `height` is given
/// and the size of the image file is not known. The size of the image file is
/// not known in the following cases:
///
/// - The program that embeds FLM contributes no inspector
///   ([`GraphicsInspector`](super::GraphicsInspector)), the service that
///   reports the size of image files.
/// - The image file cannot be found or inspected.
/// - The path is an address that begins with a scheme, such as
///   `https://example.org/a.svg`. Such an address is never inspected.
///
/// The size of such an image can be given with `width` or `height`, as in
/// `\includegraphics[width=5cm]{https://example.org/a.svg}`.
///
/// Reported by the lowering rule of `\includegraphics` when it is invoked.
/// The image is then written without a size.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.unknown-size",
    message = "Unknown size of image ‘{path}’; set its size with ‘width=…’ \
               or ‘height=…’"
)]
pub struct UnknownSize {
    /// The path, as written.
    pub path: String,
}

/// The values that every lowering rule of the image element needs to write an
/// image. [`read_image`] returns these values.
pub(crate) struct Image<'r> {
    /// The valid options of the image.
    pub(crate) options: ImageOptions,
    /// The entry of the image in the published table of the backend, which
    /// contains the published address of the image.
    pub(crate) published: &'r PublishedImage,
    /// The width and the height at which the image is shown ([`shown_size`]).
    pub(crate) size: (Option<Length>, Option<Length>),
}

/// Reads the image `node` for the lowering rule of `backend`, and reports the
/// diagnostics of the image. Returns the values that the rule needs to write
/// the image, or `None` when the path of the image is invalid.
///
/// The function performs the following steps:
///
/// 1. It reads the path of the image ([`read_path`]). When the path is not
///    plain text, it reports the error [`PathNotCharacters`] and returns
///    `None`. When the path is empty, it reports the error [`EmptyPath`] and
///    returns `None`. The rule then writes nothing for the image.
/// 2. It reads the options ([`image_options`]), and reports their errors and
///    warnings ([`report`]).
/// 3. It computes the size of the image with [`shown_size`], from the record
///    of the image in the table [`INFO`] and from the magnification of the
///    output of `backend` ([`GraphicsRenderOptions::magnification_for`]). When
///    neither the width nor the height is set, it reports the warning
///    [`UnknownSize`].
/// 4. It looks up the entry of the image in the published table of `backend`
///    ([`published_table`]).
///
/// The calling rule must declare that it reads the table [`INFO`], the
/// published table of `backend`, and the option type
/// [`GraphicsRenderOptions`].
///
/// # Errors
///
/// - [`LowerError::PlaceholderCharacters`] when the path or the value of an
///   option contains a placeholder of a user definition's template. This
///   happens only when the template is lowered without being expanded.
/// - [`LowerError::Access`] or [`LowerError::Option`] when the calling rule
///   did not declare one of the reads above.
/// - [`LowerError::Structure`] when the published table of `backend` has no
///   entry for the image. This does not happen when the publish stage of
///   `backend` ran before the lowering.
pub(crate) fn read_image<'r>(
    node: &NodeRef<'_, Flm>,
    backend: &str,
    lw: &'r mut dyn Lowerer,
) -> Result<Option<Image<'r>>, LowerError> {
    let path = match read_path(node)? {
        ImagePath::Read(path) => path,
        ImagePath::NotCharacters => {
            lw.diagnose(Diagnostic::error(PathNotCharacters, node.span().clone()));
            return Ok(None);
        }
        ImagePath::Empty => {
            lw.diagnose(Diagnostic::error(EmptyPath, node.span().clone()));
            return Ok(None);
        }
    };
    let options = image_options(node)?;
    report(node, lw)?;
    let addr = lw.addr();
    let size = {
        let record = lw.get(INFO)?.and_then(|info| info.get(addr));
        let magnification = lw.option::<GraphicsRenderOptions>()?.magnification_for(backend);
        shown_size(&options, record, &magnification)
    };
    if size == (None, None) {
        lw.diagnose(Diagnostic::warning(UnknownSize { path }, node.span().clone()));
    }
    let lw: &'r dyn Lowerer = lw;
    let published = lw
        .get(&published_table(backend))?
        .and_then(|table| table.get(addr))
        .ok_or_else(|| {
            LowerError::structure(format!(
                "missing entry for the image in table ‘{}’",
                published_table(backend).name()
            ))
        })?;
    Ok(Some(Image { options, published, size }))
}

/// The implementation of the lowering rule [`image_rule`] for one backend. It
/// lowers each image to a
/// [`flm.pres.image`](crate::stdlib::backends::pres::IMAGE) node, as the
/// [module documentation](self) describes.
#[derive(Debug, Clone)]
pub struct ImageRule {
    backend: String,
}

impl ImageRule {
    /// Returns the implementation for `backend`. It reads the published table
    /// and the magnification of `backend`.
    pub fn new(backend: impl Into<String>) -> ImageRule {
        ImageRule { backend: backend.into() }
    }
}

impl Lower for ImageRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let payload = {
            let Some(image) = read_image(&node, &self.backend, lw)? else {
                return Ok(Lowering::Emit(Vec::new()));
            };
            let (width, height) = image.size;
            ImagePayload {
                src: image.published.src.clone(),
                display: image_display(&node),
                alt: image.options.alt,
                width,
                height,
                srcset: image.published.srcset.clone(),
                anchor: None,
            }
        };
        let image = lw.pres_with(cx, &ElementName::from(pres::IMAGE), payload, Vec::new())?;
        Ok(Lowering::Emit(vec![image]))
    }
}

/// Returns whether the image `node` is shown as a block or inline, according
/// to its block role ([`block_role_of`]). An image is a block when it is a
/// direct child of the body of a float ([`GraphicsImage`](super::GraphicsImage)).
fn image_display(node: &NodeRef<'_, Flm>) -> ImageDisplay {
    if block_role_of(node) == BlockRole::Block { ImageDisplay::Block } else { ImageDisplay::Inline }
}

/// Returns the lowering rule `flm.graphics.image.lower.<backend>`
/// ([`rule_name`]), which lowers each image to a
/// [`flm.pres.image`](crate::stdlib::backends::pres::IMAGE) node on
/// `backend`, as the [module documentation](self) describes.
///
/// The rule is the default rule for images on `backend`, and is never chosen
/// on another backend ([`DefaultMark::OnBackends`]). The rule reads the table
/// [`INFO`], the published table of `backend` ([`published_table`]) and the
/// render options [`GraphicsRenderOptions`].
///
/// The `graphics` feature registers this rule for the backends `text`, `html`
/// and `latex-rendered`. You can register it for a backend of your own that
/// writes `flm.pres.image` nodes. This backend then also needs a publish
/// stage, which writes the published table of the backend
/// ([`PublishStage`](super::PublishStage)).
pub fn image_rule(backend: &str) -> LoweringRule {
    LoweringRule::new(
        rule_name(backend),
        IMAGE_ELEMENT,
        LoweringDecl::new()
            .reads(INFO)
            .reads(published_table(backend))
            .option::<GraphicsRenderOptions>()
            .produces(pres::IMAGE),
        Arc::new(ImageRule::new(backend)),
        DefaultMark::OnBackends(vec![backend.into()]),
    )
    .docs(
        "Lowers an image to a `flm.pres.image` node. The node contains the address at which the \
         output refers to the image, whether the image is a block or inline content, the \
         alternative text, and the size at which the image is shown. The size is computed from \
         `width`, `height` and the size of the image file, and is multiplied by the \
         magnification of the output.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::FlmOrigin;
    use crate::stdlib::backends::pres::I48F16;
    use crate::stdlib::features::graphics::{
        Dimensions, Factor, GraphicsInspectedInformation, PixelSize,
    };

    fn length(text: &str) -> Length {
        Length::parse(text).expect("a length")
    }

    fn record(width: &str, height: &str) -> GraphicsRecord {
        GraphicsRecord {
            file: None,
            path: String::from("a.svg"),
            origin: FlmOrigin::none(),
            inspected: Some(GraphicsInspectedInformation::Vector {
                physical_dimensions: Dimensions { width: length(width), height: length(height) },
            }),
        }
    }

    #[test]
    fn one_dimension_given_scales_the_other_in_its_unit() {
        let one = Magnification::default();
        let inspected = record("340pt", "170pt");
        let width = ImageOptions { width: Some(length("5cm")), ..ImageOptions::default() };
        assert_eq!(shown_size(&width, Some(&inspected), &one), (Some(length("5cm")), Some(length("2.5cm"))));
        let height = ImageOptions { height: Some(length("2.5cm")), ..ImageOptions::default() };
        assert_eq!(shown_size(&height, Some(&inspected), &one), (Some(length("5cm")), Some(length("2.5cm"))));
        let both = ImageOptions {
            width: Some(length("1in")),
            height: Some(length("3in")),
            ..ImageOptions::default()
        };
        assert_eq!(shown_size(&both, Some(&inspected), &one), (Some(length("1in")), Some(length("3in"))));
        assert_eq!(
            shown_size(&ImageOptions::default(), Some(&inspected), &one),
            (Some(length("340pt")), Some(length("170pt")))
        );
    }

    #[test]
    fn with_nothing_inspected_the_given_dimension_stands_alone() {
        let one = Magnification::default();
        let width = ImageOptions { width: Some(length("5cm")), ..ImageOptions::default() };
        assert_eq!(shown_size(&width, None, &one), (Some(length("5cm")), None));
        assert_eq!(shown_size(&ImageOptions::default(), None, &one), (None, None));
        let flat = record("0pt", "10pt");
        assert_eq!(shown_size(&width, Some(&flat), &one), (Some(length("5cm")), None));
    }

    fn magnification(raster: f64, vector: f64) -> Magnification {
        Magnification {
            raster: Factor::try_from(raster).expect("a factor"),
            vector: Factor::try_from(vector).expect("a factor"),
        }
    }

    fn raster(physical: Option<(&str, &str)>) -> GraphicsRecord {
        GraphicsRecord {
            file: None,
            path: String::from("a.png"),
            origin: FlmOrigin::none(),
            inspected: Some(GraphicsInspectedInformation::Raster {
                dpi: Some(I48F16::from_num(72)),
                size: PixelSize { width: 200, height: 100 },
                physical_dimensions: physical.map(|(width, height)| Dimensions {
                    width: length(width),
                    height: length(height),
                }),
            }),
        }
    }

    #[test]
    fn a_raster_image_takes_the_raster_factor_and_a_vector_image_the_vector_factor() {
        let factors = magnification(1.5, 2.0);
        let none = ImageOptions::default();
        assert_eq!(
            shown_size(&none, Some(&raster(Some(("200pt", "100pt")))), &factors),
            (Some(length("300pt")), Some(length("150pt")))
        );
        assert_eq!(
            shown_size(&none, Some(&record("200pt", "100pt")), &factors),
            (Some(length("400pt")), Some(length("200pt")))
        );
    }

    #[test]
    fn the_given_dimensions_are_magnified_too() {
        let factors = magnification(1.5, 2.0);
        let both = ImageOptions {
            width: Some(length("2cm")),
            height: Some(length("1in")),
            ..ImageOptions::default()
        };
        assert_eq!(
            shown_size(&both, Some(&raster(None)), &factors),
            (Some(length("3cm")), Some(length("1.5in")))
        );
        // One given, the other scaled from the inspected size, then both
        // magnified.
        let width = ImageOptions { width: Some(length("5cm")), ..ImageOptions::default() };
        assert_eq!(
            shown_size(&width, Some(&record("340pt", "170pt")), &factors),
            (Some(length("10cm")), Some(length("5cm")))
        );
    }

    #[test]
    fn an_image_of_unknown_type_takes_the_vector_factor() {
        let factors = magnification(1.5, 2.0);
        let width = ImageOptions { width: Some(length("5cm")), ..ImageOptions::default() };
        assert_eq!(shown_size(&width, None, &factors), (Some(length("10cm")), None));
        let mut uninspected = record("1pt", "1pt");
        uninspected.inspected = None;
        assert_eq!(shown_size(&width, Some(&uninspected), &factors), (Some(length("10cm")), None));
        assert_eq!(shown_size(&ImageOptions::default(), None, &factors), (None, None));
    }

    #[test]
    fn the_default_magnification_keeps_every_size() {
        let one = Magnification::default();
        let none = ImageOptions::default();
        assert_eq!(
            shown_size(&none, Some(&raster(Some(("7.25pt", "3mm")))), &one),
            (Some(length("7.25pt")), Some(length("3mm")))
        );
    }
}
