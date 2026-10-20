//! What is known about an image file: the inspector that a host supplies
//! ([`GraphicsInspector`]), what it reports
//! ([`GraphicsInspectedInformation`]), the record kept for each image
//! ([`GraphicsRecord`], in the table [`INFO`]), and the inspect stage that
//! writes the records ([`INSPECT_STAGE`]).
//!
//! # The inspector
//!
//! The inspector ([`GraphicsInspector`]) is the service that reports what an
//! image file itself states: whether it is a vector or a raster image, its
//! size in pixels, its resolution and its physical size. A host that can
//! examine image files contributes one to the service slot [`INSPECTOR_SLOT`]
//! (`flm.graphics.inspector`, one value at most) when it builds its registry;
//! the feature never requires one. The inspector is built with the registry
//! and receives no document: it is given one [`GraphicsInspectRequest`] per
//! image, which holds the path as the author wrote it, the origin of the
//! source the image is written in (from which a relative path is found), and,
//! where a graphics resolver ([`GraphicsResolver`]) is registered, the file the
//! resolver found and its content
//! ([`resolved`](GraphicsInspectRequest::resolved)); the inspector measures that
//! content. Without a graphics resolver, the request holds only the
//! path and the origin, and the inspector finds and reads the file itself: the
//! library opens no file.
//!
//! The inspect stage asks the resolver first. Where the resolver cannot find
//! or read the file, the stage reports the Error [`ResolveFailed`] and does
//! not ask the inspector. A path that begins with a scheme (`https://…`;
//! [`names_scheme`]) is not a file: neither the resolver nor the inspector is
//! asked about it, and nothing is reported.
//!
//! The inspector reports what the file itself states
//! ([`GraphicsInspectedInformation`]): for a vector image, its physical size;
//! for a raster image, its size in pixels and, where the file states them,
//! its resolution and its physical size. Every length is in one of the four
//! physical units `pt`, `mm`, `cm` and `in`. The inspector may instead return
//! an error ([`GraphicsInspectError`]).
//!
//! The inspector is asked about each image of each tree. It is asked again
//! wherever a synthesized value shows content holding an image a second time,
//! such as a heading's title that a reference shows: the records that a
//! value's lowering rules read are computed over the value itself, and what
//! the inspector reports there is reported again, as everything reported over
//! such a value is. A synthesized value is a small tree that a stage builds
//! while a run is under way, held outside the document's trees ([synthesized
//! values](crate::guide::concepts_overview#synthesized-values)). An inspector
//! may keep the results it has already returned, provided that reusing one
//! cannot be told from computing it again.
//!
//! # The record
//!
//! The stage [`INSPECT_STAGE`] (`flm.graphics.inspect`) writes one
//! [`GraphicsRecord`] per image into the table [`INFO`]
//! (`flm.graphics.info`), keyed by the image's node address and never by its
//! path, since two images written `./a.png` in sources of different folders
//! may be two files. The record holds the path, the origin, the file the
//! resolver found (without its content), and the inspector's result **as
//! returned and completed**: nothing the inspector returned is changed or
//! converted, and what a raster image's result leaves out is filled in:
//!
//! - no resolution: 96 dots per inch is assumed ([`ASSUMED_DPI`]), and one
//!   Warning is reported per image ([`AssumedResolution`]);
//! - no physical size: it is computed from the size in pixels and the
//!   resolution, stated or assumed, in points (1 pt = 1/72 inch): pixels × 72
//!   ÷ resolution, computed exactly and rounded once to the nearest 1/65536 pt
//!   ([`Length::from_dots`]).
//!
//! A stated physical size, and a vector image's size, are kept in their own
//! units. Without an inspector, the record holds the path, the origin, the
//! file where a resolver found one, and no result; the only thing reported is
//! a resolver's failure to find the file ([`ResolveFailed`]).
//! An inspector's error ([`InspectFailed`]), a raster image's size in pixels
//! of zero ([`NoPixelSize`]), a physical size in `em` or `tab`
//! ([`InspectedSizeNotPhysical`]), and a resolution that is not greater than
//! zero ([`InspectedResolutionNotPositive`]) are each one Error per image,
//! identifying the path, and the record then holds no result.
//!
//! The stage runs once per tree, so that an image inside a synthesized value
//! (such as a use site of a user definition expanded on the recomposed
//! LaTeX) is recorded when that value is brought up to date for its lowering
//! ([value catch-up](crate::guide::concepts_overview#value-catch-up)).

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
use crate::stdcore::backends::pres::{I48F16, Length, LengthUnit};
use crate::stdcore::features::index::ELEMENT_INDEX;
use crate::techy::source::SourceOrigin;

use super::resolve::{
    GraphicsResolveRequest, GraphicsResolver, ImageFile, RESOLVER_SLOT, ResolvedImage, names_scheme,
};
use super::{GraphicsImage, INSPECTOR_SLOT, ImagePath, read_path};

/// The name of the inspect stage, `flm.graphics.inspect` ([`InspectStage`]).
pub const INSPECT_STAGE: &str = "flm.graphics.inspect";

/// The table of records, `flm.graphics.info` ([`GraphicsInfo`]): one
/// [`GraphicsRecord`] per image, keyed by node address. The inspect stage
/// writes it, and every lowering rule of the image element reads it.
pub const INFO: TableName<GraphicsInfo> = TableName::new("flm.graphics.info");

/// The resolution assumed for a raster image whose file states none, in dots
/// per inch: 96.
pub const ASSUMED_DPI: i32 = 96;

/// A physical size: a width and a height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dimensions {
    /// The width.
    pub width: Length,
    /// The height.
    pub height: Length,
}

/// A size in whole pixels, width then height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PixelSize {
    /// The width, in pixels.
    pub width: u32,
    /// The height, in pixels.
    pub height: u32,
}

/// What an inspector reports about an image file: what the file itself
/// states, every length in a physical unit (`pt`, `mm`, `cm`, `in`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphicsInspectedInformation {
    /// A vector image (a drawing), which states its physical size.
    Vector {
        /// The size the image is meant to be shown at.
        physical_dimensions: Dimensions,
    },
    /// A raster image: a grid of pixels.
    Raster {
        /// The resolution, in dots per inch, where the file states one.
        dpi: Option<I48F16>,
        /// The size in pixels, which the inspector must always report for a
        /// raster image.
        size: PixelSize,
        /// The size the image is meant to be shown at, where the file states
        /// one.
        physical_dimensions: Option<Dimensions>,
    },
}

impl GraphicsInspectedInformation {
    /// Returns the physical size the result states, if any: a vector image's,
    /// or a raster image's where the file states one or the record was
    /// completed with one.
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

/// What the inspect stage asks an inspector about one image.
///
/// New fields may be added in later versions: an inspector reads the fields
/// it needs, and a request is built with
/// [`new`](GraphicsInspectRequest::new).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GraphicsInspectRequest {
    /// The path, as the author wrote it.
    pub path: String,
    /// The origin of the source the image is written in, from which a
    /// relative path is found: the source's canonical path, in the host's own
    /// form. For an image that the expansion of a user definition put in
    /// place, the origin of the source in which the use site is written.
    pub origin: FlmOrigin,
    /// The file the path refers to and its content, where a graphics
    /// resolver ([`GraphicsResolver`]) is registered and found and read the
    /// file; `None` where no resolver is registered.
    pub resolved: Option<ResolvedImage>,
}

impl GraphicsInspectRequest {
    /// Returns the request about the path `path`, written in the source of
    /// origin `origin`, with no resolved file.
    pub fn new(path: impl Into<String>, origin: FlmOrigin) -> GraphicsInspectRequest {
        GraphicsInspectRequest { path: path.into(), origin, resolved: None }
    }

    /// Builder: sets [`resolved`](GraphicsInspectRequest::resolved).
    pub fn resolved(mut self, resolved: ResolvedImage) -> GraphicsInspectRequest {
        self.resolved = Some(resolved);
        self
    }
}

/// Why an inspector could not examine a file, in the inspector's own words,
/// which the Error the stage reports shows ([`InspectFailed`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicsInspectError {
    /// What went wrong, as the inspector says it, such as `no such file`.
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

/// The **inspector**: a service that examines image files, which a host
/// contributes to the service slot [`INSPECTOR_SLOT`]. See the [module
/// documentation](self).
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
///
/// use flm_core::core::registry::RegistryBuilder;
/// use flm_core::stdcore::backends::pres::{Length, LengthUnit};
/// use flm_core::stdcore::features::graphics::{
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
    /// Returns what the file of `request.path` itself states.
    ///
    /// # Errors
    ///
    /// A [`GraphicsInspectError`] in the inspector's own words when the file
    /// is missing, cannot be read, or is of a kind the inspector cannot
    /// examine.
    fn inspect(
        &self,
        request: &GraphicsInspectRequest,
    ) -> Result<GraphicsInspectedInformation, GraphicsInspectError>;
}

/// What is known about one image: the path as written, the origin of the
/// source it is written in, the file the graphics resolver found, and the
/// inspector's result as returned and completed. See the [module
/// documentation](self#the-record).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicsRecord {
    /// The path, as the author wrote it.
    pub path: String,
    /// The origin given to the resolver and the inspector
    /// ([`GraphicsInspectRequest::origin`]).
    pub origin: FlmOrigin,
    /// The file the resolver found, without its content; `None` where no
    /// resolver is registered, where it returned an error, and for a path
    /// that begins with a scheme.
    pub file: Option<ImageFile>,
    /// The inspector's result, completed; `None` where no inspector is
    /// registered, where it was not asked, and where its result was an error
    /// or could not be recorded.
    pub inspected: Option<GraphicsInspectedInformation>,
}

/// The value of the table [`INFO`]: one [`GraphicsRecord`] per image, keyed
/// by node address.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphicsInfo {
    records: BTreeMap<NodeAddr, GraphicsRecord>,
}

impl GraphicsInfo {
    /// Returns the record of the image at `addr`, if the stage recorded one.
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

    /// Returns whether the table holds no record.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// Returns a length as a serialized value, written as text, such as `5cm`.
fn length_value(length: &Length) -> SerialValue {
    SerialValue::Str(length.to_string())
}

/// Returns a physical size as a serialized value, `{ width, height }`.
fn dimensions_value(dimensions: &Dimensions) -> SerialValue {
    SerialValue::Map(vec![
        (String::from("width"), length_value(&dimensions.width)),
        (String::from("height"), length_value(&dimensions.height)),
    ])
}

/// Returns an inspector's result as a serialized value: a map whose `kind` is
/// `vector` or `raster`.
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

    /// Returns `[ { node, path, origin, file, inspected }, … ]` in
    /// node-address order, `origin`, `file` and `inspected` only where
    /// known.
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

/// Condition `flm.graphics.assumed-resolution`, a Warning: the inspector's
/// result about a raster image states no resolution. Reported by the inspect
/// stage for each image, and again in each copy of content holding it that a
/// reference shows. Recovery: 96 dots per inch is assumed.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.assumed-resolution",
    message = "the image file `{path}` should state its resolution in dots per inch"
)]
pub struct AssumedResolution {
    /// The path, as written.
    pub path: String,
}

/// Condition `flm.graphics.inspect-failed`, an Error: the inspector could not
/// examine the image file; the message gives the inspector's own words.
/// Reported by the inspect stage. Recovery: the record holds no inspector
/// result, and the image is shown at the size its options give, or at no set
/// size.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.inspect-failed",
    message = "the image file `{path}` could not be examined: {detail}"
)]
pub struct InspectFailed {
    /// The path, as written.
    pub path: String,
    /// The inspector's own words.
    pub detail: String,
}

/// Condition `flm.graphics.resolve-failed`, an Error: the graphics resolver
/// could not find or read the file of an image; the message gives the
/// resolver's own words. Reported by the stage that asked: the inspect stage,
/// or a publish stage whose strategy reads the file. Recovery: the image is
/// published by the strategy `reference`. Where the inspect stage reports the
/// failure, the record holds no inspector result (and no file, where the
/// resolver could not find one), and the image is shown at the size its
/// options give, or at no set size.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.resolve-failed",
    message = "the image file `{path}` could not be read: {detail}"
)]
pub struct ResolveFailed {
    /// The path, as written.
    pub path: String,
    /// The resolver's own words.
    pub detail: String,
}

/// Condition `flm.graphics.no-pixel-size`, an Error: the inspector's result
/// about a raster image gives a width or a height of zero pixels. Reported by
/// the inspect stage. Recovery: the record holds no inspector result, and the
/// image is shown at the size its options give, or at no set size.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.no-pixel-size",
    message = "the size in pixels reported for the image file `{path}` must be greater than zero"
)]
pub struct NoPixelSize {
    /// The path, as written.
    pub path: String,
}

/// Condition `flm.graphics.inspected-size-not-physical`, an Error: the
/// inspector's result gives a physical size in `em` or `tab`, units without a
/// fixed size. Reported by the inspect stage. Recovery: the record holds no
/// inspector result, and the image is shown at the size its options give, or
/// at no set size.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.inspected-size-not-physical",
    message = "the size reported for the image file `{path}` must be in pt, mm, cm or in, not in \
               `{unit}`"
)]
pub struct InspectedSizeNotPhysical {
    /// The path, as written.
    pub path: String,
    /// The unit reported, `em` or `tab`.
    pub unit: String,
}

/// Condition `flm.graphics.inspected-resolution-not-positive`, an Error: the
/// inspector's result gives a resolution that is zero or negative. Reported
/// by the inspect stage. Recovery: the record holds no inspector result, and
/// the image is shown at the size its options give, or at no set size.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.inspected-resolution-not-positive",
    message = "the resolution reported for the image file `{path}` must be greater than zero"
)]
pub struct InspectedResolutionNotPositive {
    /// The path, as written.
    pub path: String,
}

/// Condition `flm.graphics.stage-failed`, an Error: a stage of the feature
/// could not read or write a table it declared, or read its options, which
/// happens only when the stage runs in a plan its declarations do not
/// describe: a fault of the program, not of the document. Recovery: the
/// stage's table is absent.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.graphics.stage-failed", message = "the stage `{stage}` failed: {detail}")]
pub struct StageFailed {
    /// The stage's name.
    pub stage: String,
    /// What failed, as the error's message.
    pub detail: String,
}

/// Reports [`StageFailed`] for `stage`, with no position, over a document
/// tree and over a synthesized value alike.
pub(super) fn stage_failed(out: &mut StageOut<'_>, stage: &str, detail: impl ToString) {
    out.diags.push(Diagnostic::error(
        StageFailed { stage: stage.to_string(), detail: detail.to_string() },
        nowhere(FlmOrigin::none()),
    ));
}

/// Returns the origin the resolver and the inspector are given for `span`: the
/// first source with a canonical path on the chain of sources back from the
/// span (for an image that the expansion of a user definition put in place,
/// the document source in which the use site is written), else the origin of
/// the span's own source.
// See decision rationale at [§flm-dd-dr:std-graphics-record].
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

/// Why an inspector's result cannot be recorded: the condition to report.
enum Refusal {
    /// A zero size in pixels.
    NoPixelSize,
    /// A physical size in a unit with no fixed size.
    NotPhysical(LengthUnit),
    /// A resolution that is not greater than zero.
    ResolutionNotPositive,
}

/// The first unit of `dimensions` that is not physical, if any.
fn non_physical(dimensions: &Dimensions) -> Option<LengthUnit> {
    [dimensions.width.unit, dimensions.height.unit].into_iter().find(|unit| !unit.is_physical())
}

/// Returns `answer` completed as the [module documentation](self#the-record)
/// says, with whether a resolution was assumed, or why it cannot be
/// recorded.
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

/// The services the inspect stage asks, as the registry holds them: each
/// `None` where no host contributed one.
struct Services<'r> {
    resolver: Option<&'r dyn GraphicsResolver>,
    inspector: Option<&'r dyn GraphicsInspector>,
}

/// Returns the record of the image `node`, whose path is `path`, asking the
/// resolver and the inspector of `services` where they are registered, and
/// pushes to `diags` the diagnostics their results cause.
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

/// The inspect stage, [`INSPECT_STAGE`]: a stage that computes, once per
/// tree, the table [`INFO`]. It finds the images through the element index
/// ([`ElementIndex`](crate::stdcore::features::index::ElementIndex), table
/// [`ELEMENT_INDEX`]). See the [module documentation](self).
#[derive(Debug, Default, Clone, Copy)]
pub struct InspectStage;

impl Stage for InspectStage {
    fn decl(&self) -> StageDecl {
        // Once per tree, so that value catch-up re-enters it.
        // See decision rationale at [§flm-dd-dr:std-graphics-record].
        StageDecl::new(INSPECT_STAGE, Effect::Compute, Scope::PerTree).writes(INFO).docs(
            "Asks the host's graphics resolver and inspector, where they are registered, about \
             every image, and records the file found and the inspector's result, completed, by \
             node address into `flm.graphics.info`.",
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
    b.table(INFO, "what is known about each image, by node address: its path, its origin, the \
                   file the resolver found and the inspector's result, completed")?;
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
