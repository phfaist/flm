//! The image rule of the plain-text and HTML backends ([`image_rule`]), and
//! what every lowering rule of the image element shares: reading an image's
//! path, options, record and published address when it is lowered, and
//! deciding the size it is shown at ([`shown_size`]), which the output's
//! magnification multiplies, and reporting an image whose size is not decided
//! ([`UnknownSize`]).
//!
//! [`image_rule`] stages (builds into the lowered tree; see [`Lowerer`]) one
//! [`flm.pres.image`](crate::stdcore::backends::pres::IMAGE) node per image,
//! whose payload ([`ImagePayload`]) holds: as `src`, the address the
//! backend's published table holds for the image; as `display`, block or
//! inline, as the image's position makes it; as `alt`, the value of `alt=`;
//! and as `width` and `height`, the size [`shown_size`] decides. The
//! plain-text output prints the image as `[image: <alternative text or
//! address>]`, on a line of its own for a block image and within its text for
//! an inline one ([images on the plain-text
//! output](crate::guide::rendering#images)). The HTML backend writes an `<img>` whose
//! size is given by custom properties of its `style` attribute ([images in
//! HTML](crate::guide::rendering#images-in-html)).

use std::sync::Arc;

use techy::core::node::NodeRef;
use techy::error::{Diagnostic, DiagnosticInfo};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::lang::Flm;
use crate::model::names::ElementName;
use crate::model::aspects::{BlockRole, block_role_of};
use crate::stdcore::backends::pres::{self, ImageDisplay, ImagePayload, Length};

use super::inspect::{GraphicsInspectedInformation, GraphicsRecord, INFO};
use super::options::{ImageOptions, image_options, report};
use super::publish::{PublishedImage, published_table};
use super::strategy::{GraphicsRenderOptions, Magnification};
use super::{EmptyPath, IMAGE_ELEMENT, ImagePath, PathNotCharacters, read_path};

/// Returns the name of the image rule on `backend`,
/// `flm.graphics.image.lower.<backend>`.
pub fn rule_name(backend: &str) -> String {
    format!("{IMAGE_ELEMENT}.lower.{backend}")
}

/// Returns the size an image is shown at, width then height, from its
/// options, its record and the magnification of the output it is shown on.
///
/// First the size before magnification:
///
/// - Both options given: both as given.
/// - One given: the other is the inspected size in that direction, multiplied
///   by the given dimension and divided by the inspected size in the given
///   direction, so that the image keeps its proportions, in the given
///   option's unit ([`Length::scaled`], computed exactly and rounded once).
/// - Neither given: the inspected physical size, as the record holds it.
///
/// Where nothing was inspected (no inspector is registered, or its result was
/// an error), the result is the given dimension alone, or none; the same
/// applies where the scaling cannot be computed (an inspected dimension of
/// zero, or a result too large to hold). The record is never changed.
///
/// Then each dimension, including one the options gave, is multiplied by the
/// factor of the image's type in `magnification` ([`Length::times`], rounded
/// once): `raster` where the record holds a raster image, `vector` otherwise
/// (a vector image, or an image that was not inspected). A dimension too large
/// to hold once multiplied is dropped, as above.
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
    // One computation for every backend, the factor keyed by output.
    // See decision rationale at [§flm-dd-dr:std-graphics-magnification].
    let magnify = |length: Option<Length>| length.and_then(|length| length.times(factor.value()).ok());
    (magnify(width), magnify(height))
}

/// Condition `flm.graphics.unknown-size`, a Warning: the size an image is
/// shown at is not decided ([`shown_size`] gives neither a width nor a
/// height), because neither `width=` nor `height=` is given and the file's
/// size is not known: no inspector is registered, the inspection failed, or
/// the path is an address such as `https://…`, which is never inspected. It is
/// reported wherever the image is lowered, as the options' conditions are
/// ([`report`]). Recovery: the image is written without a size.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.unknown-size",
    message = "the size of the image `{path}` must be known from its file or given with `width` \
               or `height`"
)]
pub struct UnknownSize {
    /// The path, as written.
    pub path: String,
}

/// What every lowering rule of the image element reads of an image before
/// writing it ([`read_image`]).
pub(crate) struct Image<'r> {
    /// The options, without the options the reading refused.
    pub(crate) options: ImageOptions,
    /// What the backend writes to reach the image.
    pub(crate) published: &'r PublishedImage,
    /// The size it is shown at ([`shown_size`]).
    pub(crate) size: (Option<Length>, Option<Length>),
}

/// Reads the image `node` for the rule of `backend`: its path, its options
/// (reporting their problems through [`report`]), its record, the size it is
/// shown at with the magnification of the output named after `backend`
/// ([`shown_size`]; [`UnknownSize`] reported where no size is decided) and
/// its published address. Returns `None` where the path is not characters
/// ([`PathNotCharacters`]) or is empty ([`EmptyPath`]); both are reported, and
/// no image is shown.
///
/// # Errors
///
/// [`LowerError::PlaceholderCharacters`] where the path or an option holds a
/// placeholder of a template that is lowered as written (the template of a
/// kept definition); the error of a read the rule did not declare; and a
/// structure error for an image that the backend's publish stage did not
/// publish, which does not happen when the plan scheduled that stage.
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
        .ok_or_else(|| LowerError::structure("an image its backend's publish stage did not publish"))?;
    Ok(Some(Image { options, published, size }))
}

/// The body of the image rule of one backend ([`image_rule`]): it stages a
/// `flm.pres.image` node carrying the published address and the alternative
/// sources, whether the image is a block or inline, the alternative text and
/// the size the image is shown at. See the [module documentation](self).
#[derive(Debug, Clone)]
pub struct ImageRule {
    backend: String,
}

impl ImageRule {
    /// Returns the rule's body for `backend`, which reads that backend's
    /// published table.
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

/// Returns whether `node`, an image, is shown as a block or inline: its block
/// role ([`block_role_of`]), which the element gives by position
/// ([`GraphicsImage`](super::GraphicsImage)).
fn image_display(node: &NodeRef<'_, Flm>) -> ImageDisplay {
    if block_role_of(node) == BlockRole::Block { ImageDisplay::Block } else { ImageDisplay::Inline }
}

/// Returns the image rule of `backend`, `flm.graphics.image.lower.<backend>`
/// ([`rule_name`]), the default on that backend only. It reads the table of
/// records [`INFO`], that backend's published table ([`published_table`]) and
/// the render options [`GraphicsRenderOptions`] (for the magnification), and
/// produces `flm.pres.image` nodes. The feature registers it for the
/// plain-text and HTML backends; a host whose own backend shows images in the
/// same way registers it for that backend, with a
/// [`PublishStage`](super::PublishStage) of its own.
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
        "an image lowers to `flm.pres.image`: the address its backend publishes, a block or inline \
         as its position makes it, its alternative text, and its width and height, one scaled \
         from the inspected size where only the other is given, both multiplied by the output's \
         magnification",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::FlmOrigin;
    use crate::stdcore::backends::pres::I48F16;
    use crate::stdcore::features::graphics::{
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
