//! This module defines the *inspector* of the `graphics` feature
//! ([`GraphicsInspector`]), a service that reports the type and the size of
//! image files. It also defines the inspect stage [`INSPECT_STAGE`], which
//! calls the inspector for each image and writes a record of each image into
//! the table [`INFO`], along with the error conditions that the inspect stage
//! reports.
//!
//! # The inspector
//!
//! The inspector reports whether an image is a *vector image* (a drawing, such
//! as an SVG or a PDF file) or a *raster image* (a grid of pixels, such as a
//! PNG or a JPEG file). It also reports the size of the image: the size in
//! pixels and the resolution of a raster image, and the physical size of the
//! image, such as `5cm` by `3cm`. The program that embeds FLM can contribute
//! one inspector to the service slot [`INSPECTOR_SLOT`]
//! (`flm.graphics.inspector`). The `graphics` feature works without an
//! inspector, but the size of an image file is then unknown.
//!
//! The inspect stage calls [`GraphicsInspector::inspect`] for each image with
//! a [`GraphicsInspectRequest`]. The request contains the path of the image as
//! written in the document, and the origin of the source in which the image is
//! written, from which a relative path is resolved. When a graphics resolver
//! ([`GraphicsResolver`]) is registered, the inspect stage first calls the
//! resolver to find and read the file. The request then also contains the file
//! and its content ([`resolved`](GraphicsInspectRequest::resolved)). Without a
//! graphics resolver, the inspector must find and read the file itself.
//!
//! If the graphics resolver fails to find or to read the file, the inspect
//! stage reports the error [`ResolveFailed`] and does not call the inspector.
//! For a path that begins with a scheme, such as `https://example.org/a.png`,
//! the inspect stage calls neither the resolver nor the inspector
//! ([`names_scheme`]).
//!
//! The inspector returns the *inspected information*
//! ([`GraphicsInspectedInformation`]), which contains the type and the size of
//! the image as the file states them, or an error ([`GraphicsInspectError`]).
//!
//! The inspector can be called several times for the same image. When content
//! that contains an image is shown a second time, such as the title of a
//! section that a reference displays, the inspect stage runs again over this
//! copy of the content ([synthesized
//! values](crate::guide::concepts_overview#synthesized-values)). The
//! diagnostics about the image are then reported again ([reports over content
//! shown again](crate::guide::rendering#reports-over-content-shown-again)). An
//! inspector may cache the inspected information that it returned, provided
//! that a cached result is equal to the result of a new call.
//!
//! # The record of an image
//!
//! The inspect stage writes one [`GraphicsRecord`] for each image into the
//! table [`INFO`] (`flm.graphics.info`). The lowering rules of the image
//! element read this table to compute the size at which the image is shown
//! ([the size of an image](super#the-size-of-an-image)). The records are keyed
//! by the address of the image's node. The same path, such as `./a.png`, can
//! refer to two different files when it is written in sources of different
//! folders.
//!
//! A record contains the path, the origin of the source, the file that the
//! graphics resolver found, and the inspected information. The inspect stage
//! keeps every value of the inspected information unchanged, in its own unit,
//! and fills in what the inspected information of a raster image leaves out:
//!
//! - If the inspected information contains no resolution, a resolution of 96
//!   dots per inch ([`ASSUMED_DPI`]) is assumed, and the warning
//!   [`AssumedResolution`] is reported.
//! - If the inspected information contains no physical size, the physical size
//!   is computed in points from the size in pixels and the resolution. For
//!   instance, a width of 300 pixels at a resolution of 150 dots per inch
//!   gives a width of 144 points, that is, 2 inches ([`Length::from_dots`]).
//!
//! If the inspector returns an error, the inspect stage reports the error
//! [`InspectFailed`]. If the inspected information does not meet the
//! requirements stated in [`GraphicsInspector::inspect`], the inspect stage
//! reports one of the errors [`NoPixelSize`], [`InspectedSizeNotPhysical`] or
//! [`InspectedResolutionNotPositive`]. In each of these cases, the record
//! contains no inspected information, and the size of the image file is
//! unknown.

use std::collections::BTreeMap;
use std::fmt;

use techy::core::node::NodeRef;
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::serialize::SerialValue;

use crate::core::data::store::StageOut;
use crate::core::data::table::{ProjectCx, SingletonTable, Table, TableName};
use crate::core::plan::PlanCx;
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::{Flm, FlmDiagnostic, FlmOrigin, FlmSpan, nowhere};
use crate::model::addr::NodeAddr;
use crate::stdlib::backends::pres::{I48F16, Length, LengthUnit};
use crate::stdlib::features::index::ELEMENT_INDEX;
use crate::techy::source::SourceOrigin;

use super::resolve::{
    GraphicsResolveRequest, GraphicsResolver, ImageFile, RESOLVER_SLOT, ResolvedImage, names_scheme,
};
use super::{GraphicsImage, INSPECTOR_SLOT, ImagePath, read_path};

/// The name of the inspect stage, `flm.graphics.inspect` ([`InspectStage`]).
pub const INSPECT_STAGE: &str = "flm.graphics.inspect";

/// The name of the table `flm.graphics.info` ([`GraphicsInfo`]). The table
/// contains the [`GraphicsRecord`] of each image, keyed by the address of the
/// image's node. The inspect stage [`INSPECT_STAGE`] writes this table, and
/// the lowering rules of the image element read it.
pub const INFO: TableName<GraphicsInfo> = TableName::new("flm.graphics.info");

/// The resolution, in dots per inch, that the inspect stage assumes for a
/// raster image whose file states no resolution.
pub const ASSUMED_DPI: i32 = 96;

/// The physical size of an image, given by its width and its height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dimensions {
    /// The width.
    pub width: Length,
    /// The height.
    pub height: Length,
}

/// The size of a raster image in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PixelSize {
    /// The width, in pixels.
    pub width: u32,
    /// The height, in pixels.
    pub height: u32,
}

/// The *inspected information* that an inspector returns about an image file:
/// the type of the image and its size, as the file states them.
///
/// Every length must be in one of the units `pt`, `mm`, `cm` or `in`. See
/// [`GraphicsInspector::inspect`] for the other requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphicsInspectedInformation {
    /// A vector image, that is, a drawing, such as an SVG or a PDF file.
    Vector {
        /// The physical size of the image, such as the size of the page of a
        /// PDF file.
        physical_dimensions: Dimensions,
    },
    /// A raster image, that is, a grid of pixels, such as a PNG or a JPEG
    /// file.
    Raster {
        /// The resolution in dots per inch, if the file states one.
        dpi: Option<I48F16>,
        /// The size of the image in pixels.
        size: PixelSize,
        /// The physical size of the image, if the file states one.
        physical_dimensions: Option<Dimensions>,
    },
}

impl GraphicsInspectedInformation {
    /// Returns the field `physical_dimensions` of either variant. For a raster
    /// image, returns `None` if the inspected information contains no physical
    /// size.
    pub fn physical_dimensions(&self) -> Option<Dimensions> {
        match self {
            GraphicsInspectedInformation::Vector { physical_dimensions } => {
                Some(*physical_dimensions)
            }
            GraphicsInspectedInformation::Raster { physical_dimensions, .. } => {
                *physical_dimensions
            }
        }
    }
}

/// The request that the inspect stage passes to the inspector for one image
/// ([`GraphicsInspector::inspect`]).
///
/// Fields may be added to this struct in later versions. Therefore, create a
/// request with [`new`](GraphicsInspectRequest::new).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GraphicsInspectRequest {
    /// The path of the image file, as written in the document.
    pub path: String,
    /// The origin of the source in which the image is written. Use its
    /// canonical path ([`FlmOrigin::canonical_path`]) to resolve a relative
    /// path. For an image written in the template of a user definition, this
    /// is the origin of the source in which the call of the definition is
    /// written.
    pub origin: FlmOrigin,
    /// The file that the graphics resolver ([`GraphicsResolver`]) found for
    /// the path, with its content. `None` if no graphics resolver is
    /// registered. In this case, the inspector must find and read the file
    /// itself.
    pub resolved: Option<ResolvedImage>,
}

impl GraphicsInspectRequest {
    /// Returns a request for the path `path`, written in the source with the
    /// origin `origin`. The request contains no resolved file.
    pub fn new(path: impl Into<String>, origin: FlmOrigin) -> GraphicsInspectRequest {
        GraphicsInspectRequest { path: path.into(), origin, resolved: None }
    }

    /// Sets the file and its content that the graphics resolver found
    /// ([`resolved`](GraphicsInspectRequest::resolved)), and returns the
    /// request.
    pub fn resolved(mut self, resolved: ResolvedImage) -> GraphicsInspectRequest {
        self.resolved = Some(resolved);
        self
    }
}

/// The error that an inspector returns when it cannot determine the type or
/// the size of an image file. The inspect stage reports this error as the
/// error [`InspectFailed`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicsInspectError {
    /// The reason for the failure, such as `no such file`. The message of
    /// [`InspectFailed`] contains this text after a colon, as in
    /// `Failed to inspect image file ‘plot.pdf’: no such file`. Therefore,
    /// begin the text with a lowercase letter, and end it without a period.
    pub detail: String,
}

impl GraphicsInspectError {
    /// Returns the error with the text `detail`.
    pub fn new(detail: impl Into<String>) -> GraphicsInspectError {
        GraphicsInspectError { detail: detail.into() }
    }
}

impl fmt::Display for GraphicsInspectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for GraphicsInspectError {}

/// An *inspector* is a service that reports the type and the size of image
/// files. The program that embeds FLM can contribute one inspector to the
/// service slot [`INSPECTOR_SLOT`]. The inspect stage [`INSPECT_STAGE`] then
/// calls [`inspect`](GraphicsInspector::inspect) for each image of the
/// document. See the [module documentation](self) for the requests that the
/// inspector receives and for the use of the information that it returns.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
///
/// use flm_core::core::registry::RegistryBuilder;
/// use flm_core::stdlib::backends::pres::{Length, LengthUnit};
/// use flm_core::stdlib::features::graphics::{
///     Dimensions, GraphicsInspectError, GraphicsInspectRequest, GraphicsInspectedInformation,
///     GraphicsInspector, INSPECTOR_SLOT,
/// };
///
/// /// Every image is a vector picture three inches wide and two high.
/// struct Fixed;
///
/// impl GraphicsInspector for Fixed {
///     fn inspect(
///         &self,
///         _request: &GraphicsInspectRequest,
///     ) -> Result<GraphicsInspectedInformation, GraphicsInspectError> {
///         let inches = |amount: i32| Length::new(amount.into(), LengthUnit::Inches);
///         Ok(GraphicsInspectedInformation::Vector {
///             physical_dimensions: Dimensions { width: inches(3), height: inches(2) },
///         })
///     }
/// }
///
/// let mut builder = RegistryBuilder::new();
/// builder.contribute_to(INSPECTOR_SLOT, Arc::new(Fixed) as Arc<dyn GraphicsInspector>);
/// ```
pub trait GraphicsInspector: Send + Sync {
    /// Returns the type and the size of the image file that `request` refers
    /// to, as the file states them.
    ///
    /// If the file of a raster image states no resolution or no physical
    /// size, set the corresponding field to `None`. The inspect stage then
    /// assumes a resolution and computes the physical size ([the record of an
    /// image](self#the-record-of-an-image)).
    ///
    /// The returned inspected information must meet three requirements:
    ///
    /// - Every length is in one of the units `pt`, `mm`, `cm` or `in`.
    /// - The width and the height in pixels of a raster image are greater
    ///   than zero.
    /// - The resolution of a raster image, if given, is greater than zero.
    ///
    /// If one of these requirements is not met, the inspect stage reports the
    /// error [`InspectedSizeNotPhysical`], [`NoPixelSize`] or
    /// [`InspectedResolutionNotPositive`], and ignores the inspected
    /// information.
    ///
    /// # Errors
    ///
    /// Returns a [`GraphicsInspectError`] if the inspector cannot determine
    /// the type and the size of the file, for instance if the file does not
    /// exist, cannot be read, or has a format that the inspector does not
    /// support. The inspect stage reports this error as the error
    /// [`InspectFailed`].
    fn inspect(
        &self,
        request: &GraphicsInspectRequest,
    ) -> Result<GraphicsInspectedInformation, GraphicsInspectError>;
}

/// A `GraphicsRecord` contains what the inspect stage found out about one
/// image: its path, the origin of its source, the file that the graphics
/// resolver found, and the inspected information. The table [`INFO`] contains
/// the record of each image. See [the record of an
/// image](self#the-record-of-an-image).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicsRecord {
    /// The path of the image file, as written in the document.
    pub path: String,
    /// The origin of the source in which the image is written, as passed to
    /// the graphics resolver and to the inspector
    /// ([`GraphicsInspectRequest::origin`]).
    pub origin: FlmOrigin,
    /// The file that the graphics resolver found, without its content.
    /// `None` if no graphics resolver is registered, if the resolver failed to
    /// find the file, or if the path begins with a scheme.
    pub file: Option<ImageFile>,
    /// The inspected information, with the resolution and the physical size
    /// that the inspect stage filled in. `None` in the following cases:
    ///
    /// - No inspector is registered.
    /// - The inspector was not called, because the graphics resolver failed or
    ///   because the path begins with a scheme.
    /// - The inspector returned an error.
    /// - The inspected information does not meet the requirements of
    ///   [`GraphicsInspector::inspect`].
    pub inspected: Option<GraphicsInspectedInformation>,
}

/// The content of the table [`INFO`]: the [`GraphicsRecord`] of each image,
/// keyed by the address of the image's node.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphicsInfo {
    records: BTreeMap<NodeAddr, GraphicsRecord>,
}

impl GraphicsInfo {
    /// Returns the record of the image whose node has the address `addr`, or
    /// `None` if the table contains no record for this address.
    pub fn get(&self, addr: NodeAddr) -> Option<&GraphicsRecord> {
        self.records.get(&addr)
    }

    /// Returns every record, in node-address order.
    pub fn iter(&self) -> impl Iterator<Item = (NodeAddr, &GraphicsRecord)> + '_ {
        self.records.iter().map(|(addr, record)| (*addr, record))
    }

    /// Returns the number of records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns `true` if the table contains no record.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// Returns `length` as a serialized string, such as `5cm`.
fn length_value(length: &Length) -> SerialValue {
    SerialValue::Str(length.to_string())
}

/// Returns `dimensions` as a serialized map with the keys `width` and
/// `height`.
fn dimensions_value(dimensions: &Dimensions) -> SerialValue {
    SerialValue::Map(vec![
        (String::from("width"), length_value(&dimensions.width)),
        (String::from("height"), length_value(&dimensions.height)),
    ])
}

/// Returns the inspected information `answer` as a serialized map. The key
/// `kind` has the value `vector` or `raster`. The other keys are the fields of
/// the variant, except a field whose value is `None`.
fn answer_value(answer: &GraphicsInspectedInformation) -> SerialValue {
    match answer {
        GraphicsInspectedInformation::Vector { physical_dimensions } => SerialValue::Map(vec![
            (String::from("kind"), SerialValue::Str(String::from("vector"))),
            (String::from("physical_dimensions"), dimensions_value(physical_dimensions)),
        ]),
        GraphicsInspectedInformation::Raster { dpi, size, physical_dimensions } => {
            let mut entries = vec![
                (String::from("kind"), SerialValue::Str(String::from("raster"))),
                (
                    String::from("size"),
                    SerialValue::Map(vec![
                        (String::from("width"), SerialValue::Str(size.width.to_string())),
                        (String::from("height"), SerialValue::Str(size.height.to_string())),
                    ]),
                ),
            ];
            if let Some(dpi) = dpi {
                entries.push((
                    String::from("dpi"),
                    SerialValue::Str(dpi.to_string()),
                ));
            }
            if let Some(dimensions) = physical_dimensions {
                entries.push((String::from("physical_dimensions"), dimensions_value(dimensions)));
            }
            SerialValue::Map(entries)
        }
    }
}

impl Table for GraphicsInfo {
    fn absorb(&mut self, piece: GraphicsInfo) {
        self.records.extend(piece.records);
    }

    /// Returns the records as a list of maps, in the order of the node
    /// addresses. Each map has the keys `node` and `path`. It also has the
    /// key `origin` if the origin has a label, and the keys `file` and
    /// `inspected` if the record contains a value for them.
    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.records
                .iter()
                .map(|(addr, record)| {
                    let mut entries = vec![
                        (String::from("node"), cx.node_ref(*addr)),
                        (String::from("path"), SerialValue::Str(record.path.clone())),
                    ];
                    if let Some(label) = record.origin.label() {
                        entries.push((String::from("origin"), SerialValue::Str(label.into_owned())));
                    }
                    if let Some(file) = &record.file {
                        entries.push((
                            String::from("file"),
                            SerialValue::Map(vec![
                                (String::from("identity"), SerialValue::Str(file.identity.clone())),
                                (String::from("media_type"), SerialValue::Str(file.media_type.clone())),
                                (String::from("address"), SerialValue::Str(file.address.clone())),
                            ]),
                        ));
                    }
                    if let Some(answer) = &record.inspected {
                        entries.push((String::from("inspected"), answer_value(answer)));
                    }
                    SerialValue::Map(entries)
                })
                .collect(),
        )
    }
}

impl SingletonTable for GraphicsInfo {
    const NAME: &'static str = "flm.graphics.info";
}

/// The warning that is reported when a raster image file does not state its
/// resolution (id `flm.graphics.assumed-resolution`).
///
/// Reported by the inspect stage [`INSPECT_STAGE`] when the inspected
/// information of a raster image contains no resolution. The inspect stage
/// then assumes a resolution of 96 dots per inch ([`ASSUMED_DPI`]). If the
/// file states no physical size either, the size of the image is computed
/// from its size in pixels at this resolution.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.assumed-resolution",
    message = "Missing resolution in image file ‘{path}’"
)]
pub struct AssumedResolution {
    /// The path of the image file, as written in the document.
    pub path: String,
}

/// The error that is reported when the inspector fails to determine the type
/// or the size of an image file (id `flm.graphics.inspect-failed`).
///
/// The message contains the reason that the inspector returned
/// ([`GraphicsInspectError`]). Reported by the inspect stage
/// [`INSPECT_STAGE`]. The image is then written with the size given in its
/// options `width` and `height`. If neither option is given, the image is
/// written without a size.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.inspect-failed",
    message = "Failed to inspect image file ‘{path}’: {detail}"
)]
pub struct InspectFailed {
    /// The path of the image file, as written in the document.
    pub path: String,
    /// The reason that the inspector returned
    /// ([`GraphicsInspectError::detail`]).
    pub detail: String,
}

/// The error that is reported when the graphics resolver fails to find or to
/// read the file of an image (id `flm.graphics.resolve-failed`).
///
/// The message contains the reason that the graphics resolver returned
/// ([`GraphicsResolveError`](super::GraphicsResolveError)). Two stages report
/// this error:
///
/// - The inspect stage [`INSPECT_STAGE`] reports it when the resolver fails to
///   find the file, or fails to read the content that the inspector needs. The
///   image is then written with the size given in its options `width` and
///   `height`. If neither option is given, the image is written without a
///   size.
/// - The publish stage of an output ([`PublishStage`](super::PublishStage))
///   reports it when the resolver fails to read the file under the strategy
///   `copy`, `data-uri` or `content`.
///
/// If the resolver fails to find the file, every output refers to the image by
/// its path as written. If a publish stage fails to read the file, its output
/// refers to the image by the address of the file that the resolver returned.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.resolve-failed",
    message = "Failed to read image file ‘{path}’: {detail}"
)]
pub struct ResolveFailed {
    /// The path of the image file, as written in the document.
    pub path: String,
    /// The reason that the graphics resolver returned
    /// ([`GraphicsResolveError::detail`](super::GraphicsResolveError::detail)).
    pub detail: String,
}

/// The error that is reported when the inspector returns a width or a height
/// of zero pixels for a raster image (id `flm.graphics.no-pixel-size`).
///
/// Reported by the inspect stage [`INSPECT_STAGE`], which then ignores the
/// inspected information of the image. The image is written with the size
/// given in its options `width` and `height`. If neither option is given, the
/// image is written without a size.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.no-pixel-size",
    message = "Invalid size of image file ‘{path}’: the width and the height in \
               pixels must be greater than zero"
)]
pub struct NoPixelSize {
    /// The path of the image file, as written in the document.
    pub path: String,
}

/// The error that is reported when the inspector returns a size of an image
/// in the unit `em` or `tab` (id `flm.graphics.inspected-size-not-physical`).
///
/// An inspector must give every length in one of the units `pt`, `mm`, `cm`
/// or `in`, which have a fixed size. A document cannot cause this error: it
/// is caused by a faulty inspector.
///
/// Reported by the inspect stage [`INSPECT_STAGE`], which then ignores the
/// inspected information of the image. The image is written with the size
/// given in its options `width` and `height`. If neither option is given, the
/// image is written without a size.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.inspected-size-not-physical",
    message = "Invalid unit ‘{unit}’ in the size reported by the inspector for image file \
               ‘{path}’: sizes must be given in ‘pt’, ‘mm’, ‘cm’ or ‘in’"
)]
pub struct InspectedSizeNotPhysical {
    /// The path of the image file, as written in the document.
    pub path: String,
    /// The unit of the size, `em` or `tab`.
    pub unit: String,
}

/// The error that is reported when the inspector returns a resolution of zero
/// or less for a raster image (id
/// `flm.graphics.inspected-resolution-not-positive`).
///
/// Reported by the inspect stage [`INSPECT_STAGE`], which then ignores the
/// inspected information of the image. The image is written with the size
/// given in its options `width` and `height`. If neither option is given, the
/// image is written without a size.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.inspected-resolution-not-positive",
    message = "Invalid resolution in image file ‘{path}’: the resolution must be \
               greater than zero"
)]
pub struct InspectedResolutionNotPositive {
    /// The path of the image file, as written in the document.
    pub path: String,
}

/// The error that is reported when a stage of the [`graphics`](super) feature
/// fails because of an error in the program (id `flm.graphics.stage-failed`).
///
/// A document cannot cause this error. The inspect stage and the publish
/// stages report it when they cannot access a table or the render options
/// listed in their declarations. This means that the plan does not match the
/// declarations of the stage. Reported at no position in the document. The
/// stage then stops, and the tables that it has not written yet are absent.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.graphics.stage-failed", message = "Stage ‘{stage}’ failed: {detail}")]
pub struct StageFailed {
    /// The name of the stage.
    pub stage: String,
    /// The message of the error that the stage received when it accessed a
    /// table or the render options.
    pub detail: String,
}

/// Pushes the error [`StageFailed`] of the stage `stage` to `out`, with the
/// text `detail`, at no position in the document.
pub(super) fn stage_failed(out: &mut StageOut<'_>, stage: &str, detail: impl ToString) {
    out.diags.push(Diagnostic::error(
        StageFailed { stage: stage.to_string(), detail: detail.to_string() },
        nowhere(FlmOrigin::none()),
    ));
}

/// Returns the origin that the graphics resolver and the inspector receive for
/// the image at `span`.
///
/// This is the origin of the first source that has a canonical path, starting
/// from the source of `span` and following each expansion back to the place
/// where it was triggered. For an image written in the template of a user
/// definition, this is the source in which the call of the definition is
/// written. If no source on this chain has a canonical path, returns the
/// origin of the source of `span`.
// A source that the expansion of a user definition produces has no file of its
// own, so the host is given the file in which the author wrote the use site.
fn request_origin(span: &FlmSpan) -> FlmOrigin {
    let own = span.source().origin().clone();
    let mut span = span.clone();
    loop {
        if span.source().origin().canonical_path().is_some() {
            return span.source().origin().clone();
        }
        let Some(triggered) = span.source().provenance().triggered_at().cloned() else {
            return own;
        };
        span = triggered;
    }
}

/// The reason why the inspect stage ignores the inspected information of an
/// image. Each variant corresponds to one error condition.
enum Refusal {
    /// A raster image with a width or a height of zero pixels
    /// ([`NoPixelSize`]).
    NoPixelSize,
    /// A size in a unit that has no fixed size, `em` or `tab`
    /// ([`InspectedSizeNotPhysical`]).
    NotPhysical(LengthUnit),
    /// A resolution of zero or less ([`InspectedResolutionNotPositive`]).
    ResolutionNotPositive,
}

/// Returns the unit of the width of `dimensions` if this unit has no fixed
/// size, else the unit of the height if this unit has no fixed size, else
/// `None`.
fn non_physical(dimensions: &Dimensions) -> Option<LengthUnit> {
    [dimensions.width.unit, dimensions.height.unit].into_iter().find(|unit| !unit.is_physical())
}

/// Checks the inspected information `answer` and fills in what it leaves out,
/// as described in [the record of an image](self#the-record-of-an-image).
///
/// Returns the completed inspected information, and `true` if the resolution
/// [`ASSUMED_DPI`] was assumed. If a physical size computed from the size in
/// pixels is too large to represent, the completed inspected information
/// contains no physical size. Returns the reason for ignoring `answer` if it
/// does not meet the requirements of [`GraphicsInspector::inspect`].
fn complete(
    answer: GraphicsInspectedInformation,
) -> Result<(GraphicsInspectedInformation, bool), Refusal> {
    match answer {
        GraphicsInspectedInformation::Vector { physical_dimensions } => {
            if let Some(unit) = non_physical(&physical_dimensions) {
                return Err(Refusal::NotPhysical(unit));
            }
            Ok((answer, false))
        }
        GraphicsInspectedInformation::Raster { dpi, size, physical_dimensions } => {
            if size.width == 0 || size.height == 0 {
                return Err(Refusal::NoPixelSize);
            }
            if let Some(unit) = physical_dimensions.as_ref().and_then(non_physical) {
                return Err(Refusal::NotPhysical(unit));
            }
            if dpi.is_some_and(|dpi| dpi <= I48F16::ZERO) {
                return Err(Refusal::ResolutionNotPositive);
            }
            let assumed = dpi.is_none();
            let dpi = dpi.unwrap_or(I48F16::from_num(ASSUMED_DPI));
            let physical_dimensions = match physical_dimensions {
                Some(stated) => Some(stated),
                // A size too large to hold is left out rather than wrapped.
                None => Length::from_dots(u64::from(size.width), dpi)
                    .and_then(|width| {
                        Length::from_dots(u64::from(size.height), dpi)
                            .map(|height| Dimensions { width, height })
                    })
                    .ok(),
            };
            Ok((
                GraphicsInspectedInformation::Raster { dpi: Some(dpi), size, physical_dimensions },
                assumed,
            ))
        }
    }
}

/// The services that the inspect stage calls: the graphics resolver and the
/// inspector registered in the registry. A field is `None` if no service was
/// contributed to its service slot.
struct Services<'r> {
    resolver: Option<&'r dyn GraphicsResolver>,
    inspector: Option<&'r dyn GraphicsInspector>,
}

/// Returns the record of the image `node` with the path `path`. Calls the
/// graphics resolver and the inspector of `services`, if they are registered,
/// and pushes the resulting diagnostics to `diags`.
fn record_of(
    node: &NodeRef<'_, Flm>,
    path: String,
    services: &Services<'_>,
    diags: &mut Vec<FlmDiagnostic>,
) -> GraphicsRecord {
    let origin = request_origin(node.span());
    let mut record = GraphicsRecord { path, origin, file: None, inspected: None };
    if names_scheme(&record.path) {
        return record;
    }
    let span = node.span().clone();
    let mut request = GraphicsInspectRequest::new(record.path.clone(), record.origin.clone());
    if let Some(resolver) = services.resolver {
        let asked = GraphicsResolveRequest::new(record.path.clone(), record.origin.clone());
        let file = match resolver.resolve(&asked) {
            Ok(file) => file,
            Err(error) => {
                let failed = ResolveFailed { path: record.path.clone(), detail: error.detail };
                diags.push(Diagnostic::error(failed, span));
                return record;
            }
        };
        record.file = Some(file.clone());
        if services.inspector.is_none() {
            return record;
        }
        match resolver.read(&file) {
            Ok(content) => request = request.resolved(ResolvedImage { file, content: content.into() }),
            Err(error) => {
                let failed = ResolveFailed { path: record.path.clone(), detail: error.detail };
                diags.push(Diagnostic::error(failed, span));
                return record;
            }
        }
    }
    let Some(inspector) = services.inspector else { return record };
    let path = record.path.clone();
    match inspector.inspect(&request).map_err(|error| error.detail) {
        Err(detail) => diags.push(Diagnostic::error(InspectFailed { path, detail }, span)),
        Ok(answer) => match complete(answer) {
            Ok((completed, assumed)) => {
                if assumed {
                    diags.push(Diagnostic::warning(AssumedResolution { path }, span));
                }
                record.inspected = Some(completed);
            }
            Err(Refusal::NoPixelSize) => {
                diags.push(Diagnostic::error(NoPixelSize { path }, span));
            }
            Err(Refusal::NotPhysical(unit)) => diags.push(Diagnostic::error(
                InspectedSizeNotPhysical { path, unit: unit.to_string() },
                span,
            )),
            Err(Refusal::ResolutionNotPositive) => {
                diags.push(Diagnostic::error(InspectedResolutionNotPositive { path }, span));
            }
        },
    }
    record
}

/// The inspect stage of the `graphics` feature, registered under the name
/// [`INSPECT_STAGE`].
///
/// For each image, the stage calls the graphics resolver and the inspector, if
/// they are registered, and writes the [`GraphicsRecord`] of the image into
/// the table [`INFO`]. The stage finds the images in the element index
/// ([`ElementIndex`](crate::stdlib::features::index::ElementIndex), table
/// [`ELEMENT_INDEX`]). It runs once for each tree, including each copy of
/// content that is shown a second time. See the [module documentation](self).
#[derive(Debug, Default, Clone, Copy)]
pub struct InspectStage;

impl Stage for InspectStage {
    fn decl(&self) -> StageDecl {
        // Once per tree, so that value catch-up, which re-runs only per-tree
        // stages, re-enters it and an image a value shows is inspected too; an
        // image needs nothing outside the tree it stands in.
        StageDecl::new(INSPECT_STAGE, Effect::Compute, Scope::PerTree).writes(INFO).docs(
            "Calls the graphics resolver and the inspector, if they are registered, for each \
             image, and writes the record of each image to `flm.graphics.info`. Reports each \
             image file that cannot be found, read or inspected, and each raster image file \
             that does not state its resolution.",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(ELEMENT_INDEX)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let (Some(id), Some(tree)) = (cx.tree_id(), cx.my_tree()) else { return };
        let index = match cx.get(ELEMENT_INDEX) {
            Ok(Some(index)) => index,
            Ok(None) => return,
            Err(error) => return stage_failed(out, INSPECT_STAGE, error),
        };
        let registered = cx.registry().services();
        let services = Services {
            resolver: registered.service_named::<dyn GraphicsResolver>(RESOLVER_SLOT),
            inspector: registered.service_named::<dyn GraphicsInspector>(INSPECTOR_SLOT),
        };
        let mut info = GraphicsInfo::default();
        let mut diags = Vec::new();
        for (addr, node, _) in index.values_of::<GraphicsImage>(id, &tree) {
            // A path holding a template's placeholder is read where each use
            // site is lowered; a path that is not characters, or is empty, is
            // reported by the rules, which show no image for it.
            let Ok(ImagePath::Read(path)) = read_path(&node) else { continue };
            let record = record_of(&node, path, &services, &mut diags);
            info.records.insert(addr, record);
        }
        for diagnostic in diags {
            out.diags.push(diagnostic);
        }
        match out.table(INFO) {
            Ok(table) => *table = info,
            Err(error) => stage_failed(out, INSPECT_STAGE, error),
        }
    }
}

/// Declares the table [`INFO`] and the inspect stage.
pub(super) fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.table(INFO, "The record of each image, keyed by the address of the image's node: the path, \
                   the file that the graphics resolver found, and the type and the size of the \
                   image that the inspector returned.")?;
    b.stage(std::sync::Arc::new(InspectStage))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn length(text: &str) -> Length {
        Length::parse(text).expect("a length")
    }

    #[test]
    fn a_raster_answer_is_completed_and_nothing_stated_changes() {
        let size = PixelSize { width: 300, height: 150 };
        let bare = GraphicsInspectedInformation::Raster { dpi: None, size, physical_dimensions: None };
        let Ok((completed, assumed)) = complete(bare) else { panic!("completed") };
        assert!(assumed);
        assert_eq!(
            completed,
            GraphicsInspectedInformation::Raster {
                dpi: Some(I48F16::from_num(96)),
                size,
                physical_dimensions: Some(Dimensions {
                    width: length("225pt"),
                    height: length("112.5pt")
                }),
            }
        );
        let stated = Dimensions { width: length("3cm"), height: length("1.5cm") };
        let full = GraphicsInspectedInformation::Raster {
            dpi: Some(I48F16::from_num(300)),
            size,
            physical_dimensions: Some(stated),
        };
        assert_eq!(complete(full).ok(), Some((full, false)));
    }

    #[test]
    fn an_answer_that_cannot_be_recorded_is_refused() {
        let size = PixelSize { width: 0, height: 10 };
        let zero = GraphicsInspectedInformation::Raster { dpi: None, size, physical_dimensions: None };
        assert!(matches!(complete(zero), Err(Refusal::NoPixelSize)));
        let ems = Dimensions { width: length("2em"), height: length("1in") };
        let vector = GraphicsInspectedInformation::Vector { physical_dimensions: ems };
        assert!(matches!(complete(vector), Err(Refusal::NotPhysical(LengthUnit::Ems))));
        let flat = GraphicsInspectedInformation::Raster {
            dpi: Some(I48F16::ZERO),
            size: PixelSize { width: 1, height: 1 },
            physical_dimensions: None,
        };
        assert!(matches!(complete(flat), Err(Refusal::ResolutionNotPositive)));
    }
}
