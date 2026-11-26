//! This module defines the publish stages of the `graphics` feature
//! ([`PublishStage`]), along with the tables that these stages write and the
//! error conditions that they report. A publish stage computes the address at
//! which an output refers to each image.
//!
//! The *published address* of an image is the address that an output writes
//! to refer to the image, for instance in the `src` attribute of an HTML
//! `<img>` element. The `graphics` feature registers one publish stage for
//! each of the backends `text`, `html`, `latex-recomposed` and
//! `latex-rendered`, named `flm.graphics.publish.<backend>`
//! ([`publish_stage_name`]). The stage writes a [`PublishedImage`] for each
//! image into the *published table* of the backend,
//! `flm.graphics.published.<backend>` ([`published_table`]), keyed by the
//! address of the image's node. The lowering rule of images on this backend
//! reads the published table. Since each backend has its own published table,
//! the outputs of one document can refer to the same image in different ways.
//!
//! # Strategies
//!
//! The published address depends on the *strategy* that the render options
//! set for the output ([`GraphicsRenderOptions::strategy`], [`Strategy`]). The
//! render options select the strategy by the name of the output, which is the
//! name of its backend. There are four strategies:
//!
//! - `reference` (the default): The address is the address of the file that
//!   the graphics resolver returned ([`ImageFile::address`]), or the path as
//!   written in the document if no graphics resolver found the file. The file
//!   is not read.
//! - `copy`: The stage reads the file, converts its content if a conversion
//!   rule applies, and adds a copy of the file to the output. The address is
//!   the path of the copy (see [copies of image
//!   files](self#copies-of-image-files)). The plain-text output `text` cannot
//!   use this strategy.
//! - `data-uri`: The stage reads the file, converts its content if a
//!   conversion rule applies, and writes the content into the address itself,
//!   as in `data:image/png;base64,iVBORw0…`. Only the `html` output can use
//!   this strategy.
//! - `content`: The address is the same as under `reference`. The stage also
//!   reads the file, converts its content if a conversion rule applies, and
//!   writes the content into the *content table* of the backend,
//!   `flm.graphics.content.<backend>` ([`content_table`]). This strategy is
//!   meant for an output that renders images itself, for instance by drawing
//!   each image with characters on a terminal. A stage of this output reads
//!   the content table. Every output can use this strategy.
//!
//! The stage reads files through the graphics resolver ([`GraphicsResolver`]),
//! a service that the program which embeds FLM can contribute. A path that
//! begins with a scheme, such as `https://example.org/a.png`, is written
//! unchanged under every strategy, and is never read ([`names_scheme`]).
//!
//! If the strategy `copy`, `data-uri` or `content` cannot be carried out for an
//! image, an error is reported at the image. The output then refers to the
//! image as under `reference`, and the content table contains no entry for the
//! image. This happens in the following cases:
//!
//! - The output cannot use the strategy ([`CopyInOneFile`],
//!   [`DataUriNotShown`]).
//! - No graphics resolver is registered ([`NeedsResolver`]).
//! - The graphics resolver fails to find or to read the file
//!   ([`ResolveFailed`]).
//! - The strategy is `copy`, and no artifact namer is registered
//!   ([`NeedsNamer`]).
//!
//! # Conversion
//!
//! Under the strategies `copy`, `data-uri` and `content`, the stage applies the
//! first conversion rule of the output whose `from` lists the format of the
//! file ([`GraphicsRenderOptions::convert`]). The converter that performs the
//! rule is selected by [`choose_converter`]. The copy, the `data:` address or
//! the entry of the content table then has the format `to` of the rule. If the
//! conversion cannot be carried out, an error is reported at the image
//! ([`UnknownConverter`], [`NoConverterOffers`], [`ConversionFailed`]), and the
//! stage uses the content of the file unconverted. See the module
//! [`convert`](super::convert).
//!
//! # Copies of image files
//!
//! Under the strategy `copy`, the stage writes each copy as an artifact into
//! the artifact table of the output, `flm.output.<backend>.artifacts`
//! ([`OutputImages::artifacts`]). An *artifact* is one file of an output, held
//! in memory ([`Artifact`]). The final stage of the output adds these
//! artifacts to the artifact set of the output ([`ArtifactSet`]).
//!
//! The identity of a copy has the form `graphics:<stem>-<digest>`, as in
//! `graphics:plot-3fa9c2d1`. The stem is the file name without its extension.
//! The digest consists of eight hexadecimal digits, computed from the content
//! of the file, the format of the copy and the options of the conversion rule.
//! The identity therefore depends only on the file and on its conversion. An
//! image file that appears several times in a document has a single copy.
//!
//! The address of a copy is the path, relative to the output, that the
//! artifact namer ([`ArtifactNamer`]) returns for the identity and the file
//! extension of the copy. The artifact namer is a service that the program
//! which embeds FLM contributes to the service slot [`NAMER_SLOT`]. The file
//! extension is the short name of the format of the copy, such as `png`
//! ([`ImageFormat::name`]). A copy in a format that [`ImageFormat`] does not
//! list has no file extension.
//!
//! # Synthesized values
//!
//! A publish stage runs once for each tree, including the trees of
//! [synthesized values](crate::guide::concepts_overview#synthesized-values),
//! such as the title of a section that a reference displays again. In a
//! synthesized value, an image has the same published address as in the
//! document. Under the strategy `content`, the image also has its own entry
//! in the content table. Copies are written only by the runs over the trees of
//! the document.
//!
//! The output does not contain a copy of an image that appears only in a
//! synthesized value, and in no tree of the document. The output nevertheless
//! refers to such an image by the address of its copy.

use std::collections::BTreeMap;
use std::hash::Hash;
use std::sync::Arc;

use base64::Engine as _;
use techy::core::node::NodeRef;
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::serialize::SerialValue;

use crate::core::artifact::{Artifact, ArtifactId, ArtifactNamer, ArtifactRef, ArtifactSet, NamingScope, digest_name};
use crate::core::data::store::StageOut;
use crate::core::data::table::{ProjectCx, Table, TableHandle, TableName};
use crate::core::plan::PlanCx;
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::{Flm, FlmDiagnostic, FlmSpan};
use crate::model::addr::NodeAddr;
use crate::model::digest::Fnv1a64;
use crate::model::names::StageName;
use crate::progress::{self, Counted, Kind, Unit};
use crate::stdlib::backends::lowering::NAMER_SLOT;
use crate::stdlib::backends::pres::SrcsetCandidate;
use crate::stdlib::backends::{html, latex_recomposed, latex_rendered};
use crate::stdlib::features::index::ELEMENT_INDEX;

use super::convert::{
    CONVERTERS_SLOT, ConversionOptions, ConversionRequest, ConversionRule, GraphicsConverter,
    NoConverter, choose_converter,
};
use super::inspect::{GraphicsInfo, GraphicsRecord, INFO, ResolveFailed, stage_failed};
use super::resolve::{GraphicsResolver, ImageFile, ImageFormat, RESOLVER_SLOT, names_scheme, stem_of};
use super::strategy::{GraphicsRenderOptions, Strategy};
use super::{FEATURE, GraphicsImage, ImagePath, read_path};

/// A `PublishedImage` contains the published address of one image, which the
/// lowering rule of images on a backend writes to refer to the image. See the
/// [module documentation](self).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PublishedImage {
    /// The published address of the image, computed according to the strategy
    /// of the output.
    pub src: String,
    /// Alternative sources of the image, each with its descriptor, as in the
    /// `srcset` attribute of an HTML `<img>` element. No strategy produces
    /// alternative sources, so this list is always empty.
    pub srcset: Vec<SrcsetCandidate>,
}

/// The value of the published table of a backend,
/// `flm.graphics.published.<backend>` ([`published_table`]).
///
/// The table contains one [`PublishedImage`] for each image of the document,
/// keyed by the address of the image's node.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphicsPublished {
    images: BTreeMap<NodeAddr, PublishedImage>,
}

impl GraphicsPublished {
    /// Returns the published image at the node address `addr`, or `None` if
    /// no image was published at this address.
    pub fn get(&self, addr: NodeAddr) -> Option<&PublishedImage> {
        self.images.get(&addr)
    }

    /// Returns every published image, in node-address order.
    pub fn iter(&self) -> impl Iterator<Item = (NodeAddr, &PublishedImage)> + '_ {
        self.images.iter().map(|(addr, image)| (*addr, image))
    }

    /// Returns the number of published images.
    pub fn len(&self) -> usize {
        self.images.len()
    }

    /// Returns whether no image was published.
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}

impl Table for GraphicsPublished {
    fn absorb(&mut self, piece: GraphicsPublished) {
        self.images.extend(piece.images);
    }

    /// Returns the list of published images, in the order of their node
    /// addresses. Each entry is a map with the keys `node` and `src`, and also
    /// `srcset` if the image has alternative sources.
    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.images
                .iter()
                .map(|(addr, image)| {
                    let mut entries = vec![
                        (String::from("node"), cx.node_ref(*addr)),
                        (String::from("src"), SerialValue::Str(image.src.clone())),
                    ];
                    if !image.srcset.is_empty() {
                        entries.push((
                            String::from("srcset"),
                            SerialValue::List(
                                image
                                    .srcset
                                    .iter()
                                    .map(|candidate| {
                                        SerialValue::Str(format!(
                                            "{} {}",
                                            candidate.src, candidate.descriptor
                                        ))
                                    })
                                    .collect(),
                            ),
                        ));
                    }
                    SerialValue::Map(entries)
                })
                .collect(),
        )
    }
}

/// Returns the handle of the published table of `backend`,
/// `flm.graphics.published.<backend>`. The publish stage of the backend writes
/// this table, and the lowering rule of images on the backend reads it.
pub fn published_table(backend: &str) -> TableHandle<GraphicsPublished> {
    TableHandle::new(format!("flm.graphics.published.{backend}"))
}

/// An `ImageContent` contains the content of one image file, as the publish
/// stage writes it into the content table under the strategy `content`. An
/// output that renders images itself reads this content. See the [module
/// documentation](self).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageContent {
    /// The published address of the image, which is the same as under the
    /// strategy `reference`.
    pub src: String,
    /// The media type of the content, such as `image/png`. If a conversion
    /// rule converted the content, this is the media type of the format `to`
    /// of the rule.
    pub media_type: String,
    /// The content of the image file, or the result of its conversion.
    pub content: Vec<u8>,
}

/// The value of the content table of a backend,
/// `flm.graphics.content.<backend>` ([`content_table`]).
///
/// Under the strategy `content`, the table contains one [`ImageContent`] for
/// each image whose file the publish stage read, keyed by the address of the
/// image's node. Under the other strategies, the table is empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphicsContent {
    images: BTreeMap<NodeAddr, ImageContent>,
}

impl GraphicsContent {
    /// Returns the content of the image at the node address `addr`, or `None`
    /// if the table contains no entry for this address.
    pub fn get(&self, addr: NodeAddr) -> Option<&ImageContent> {
        self.images.get(&addr)
    }

    /// Returns the content of every image with its node address, in the order
    /// of the node addresses.
    pub fn iter(&self) -> impl Iterator<Item = (NodeAddr, &ImageContent)> + '_ {
        self.images.iter().map(|(addr, image)| (*addr, image))
    }

    /// Returns the number of entries in the table.
    pub fn len(&self) -> usize {
        self.images.len()
    }

    /// Returns whether the table contains no entry.
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}

impl Table for GraphicsContent {
    fn absorb(&mut self, piece: GraphicsContent) {
        self.images.extend(piece.images);
    }

    /// Returns the list of entries, in the order of their node addresses. Each
    /// entry is a map with the keys `node`, `src`, `media_type` and `content`.
    ///
    /// The value of `content` is `{ text }` if the bytes are valid UTF-8 text,
    /// and `{ base64 }` otherwise, with the bytes encoded in base64 (standard
    /// alphabet, with padding). An extension receives the content of an
    /// artifact in the same form.
    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.images
                .iter()
                .map(|(addr, image)| {
                    SerialValue::Map(vec![
                        (String::from("node"), cx.node_ref(*addr)),
                        (String::from("src"), SerialValue::Str(image.src.clone())),
                        (String::from("media_type"), SerialValue::Str(image.media_type.clone())),
                        (String::from("content"), content_value(&image.content)),
                    ])
                })
                .collect(),
        )
    }
}

/// Returns `bytes` as `{ text }` if they are valid UTF-8 text, and as
/// `{ base64 }` otherwise.
fn content_value(bytes: &[u8]) -> SerialValue {
    let entry = match std::str::from_utf8(bytes) {
        Ok(text) => (String::from("text"), SerialValue::Str(text.to_string())),
        Err(_) => {
            (String::from("base64"), SerialValue::Str(base64::engine::general_purpose::STANDARD.encode(bytes)))
        }
    };
    SerialValue::Map(vec![entry])
}

/// Returns the handle of the content table of `backend`,
/// `flm.graphics.content.<backend>`. Under the strategy `content`, the publish
/// stage of the backend writes the content of each image file into this
/// table. A stage of an output that renders images itself reads the table.
pub fn content_table(backend: &str) -> TableHandle<GraphicsContent> {
    TableHandle::new(format!("flm.graphics.content.{backend}"))
}

/// Returns the name of the publish stage of `backend`,
/// `flm.graphics.publish.<backend>`.
pub fn publish_stage_name(backend: &str) -> StageName {
    StageName::from(format!("flm.graphics.publish.{backend}"))
}

/// The kind of the row, `flm.graphics.images`, that a call of a publish stage
/// declares on the progress sheet of the build for the images of its tree
/// that it converts ([`crate::progress`]).
///
/// The row is counted in items: its total is the number of images of the tree
/// to which a conversion rule applies, as far as the records of the inspect
/// stage tell, and it advances as each of them is published. It is labeled
/// with the name of the tree's source file, where the tree has one. A call
/// that converts no image declares no row; an image that is only copied is
/// not counted. The row has no title: a display shows the conversions under
/// it ([`CONVERT_KIND`]).
pub const IMAGES_KIND: Kind = Kind::new("flm.graphics.images");

/// The kind of the row, `flm.graphics.convert`, with the title "Converting
/// graphics", that a publish stage declares on the progress sheet of the
/// build for each conversion of an image, under its row [`IMAGES_KIND`]
/// ([`crate::progress`]).
///
/// The row is labeled with the path of the image as written in the
/// document. It runs while the converter converts, and the converter receives
/// its scope ([`GraphicsConverter::convert`]). It ends as finished when the
/// converter returns the converted content, and as failed when the converter
/// fails.
pub const CONVERT_KIND: Kind = Kind::titled("flm.graphics.convert", "Converting graphics");

/// An `OutputImages` describes whether the output of one backend can contain
/// copies of image files, and whether it can use `data:` addresses. This
/// determines whether the output can use the strategies `copy` and
/// `data-uri`. [`output_images`] returns this value for a backend.
#[derive(Debug, Clone, Copy)]
pub struct OutputImages {
    /// The artifact table of the output, `flm.output.<backend>.artifacts`,
    /// into which the publish stage writes the copies of image files under
    /// the strategy `copy`. `None` if the output consists of a single file,
    /// such as the plain-text output; such an output cannot use the strategy
    /// `copy`.
    pub artifacts: Option<TableName<ArtifactSet>>,
    /// Whether the output can refer to an image by an address that begins
    /// with `data:`, as the strategy `data-uri` writes it. An HTML page can,
    /// as in `<img src="data:…">`.
    pub data_address: bool,
}

/// Returns whether the output of `backend` can contain copies of image files
/// and use `data:` addresses ([`OutputImages`]).
///
/// - The `html` output can contain copies and use `data:` addresses.
/// - The `latex-recomposed` and `latex-rendered` outputs can contain copies.
/// - The `text` output, and the output of every other backend, can do
///   neither.
pub fn output_images(backend: &str) -> OutputImages {
    match backend {
        html::BACKEND => OutputImages { artifacts: Some(html::ARTIFACTS), data_address: true },
        latex_recomposed::BACKEND => {
            OutputImages { artifacts: Some(latex_recomposed::ARTIFACTS), data_address: false }
        }
        latex_rendered::BACKEND => {
            OutputImages { artifacts: Some(latex_rendered::ARTIFACTS), data_address: false }
        }
        _ => OutputImages { artifacts: None, data_address: false },
    }
}

/// The error that is reported when the strategy of an output requires reading
/// an image file, and no graphics resolver is registered (id
/// `flm.graphics.needs-resolver`).
///
/// The strategies `copy`, `data-uri` and `content` read the content of each
/// image file through the graphics resolver ([`GraphicsResolver`]). The program
/// that embeds FLM can contribute a graphics resolver to the service slot
/// [`RESOLVER_SLOT`].
///
/// Reported by the publish stage of the output ([`PublishStage`]) for each
/// image. The output then refers to the image by its path as written in the
/// document. Under the strategy `content`, the content table contains no entry
/// for the image.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.needs-resolver",
    message = "Unsupported strategy ‘{strategy}’ for output ‘{output}’ (image file ‘{path}’): \
               reading image files is not available"
)]
pub struct NeedsResolver {
    /// The name of the output, which is the name of its backend.
    pub output: String,
    /// The path of the image file, as written in the document.
    pub path: String,
    /// The name of the strategy: `copy`, `data-uri` or `content`.
    pub strategy: String,
}

/// The error that is reported when the strategy of an output is `copy`, and no
/// artifact namer is registered (id `flm.graphics.needs-namer`).
///
/// The address of a copy of an image file is the path that the artifact namer
/// ([`ArtifactNamer`]) returns for the copy. The program that embeds FLM can
/// contribute an artifact namer to the service slot [`NAMER_SLOT`].
///
/// Reported by the publish stage of the output ([`PublishStage`]) for each
/// image whose file the graphics resolver found. The output then refers to the
/// image by the address of the file that the graphics resolver returned, and
/// contains no copy of the file.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.needs-namer",
    message = "Unsupported strategy ‘copy’ for output ‘{output}’ (image file ‘{path}’): \
               choosing file names for copies is not available"
)]
pub struct NeedsNamer {
    /// The name of the output, which is the name of its backend.
    pub output: String,
    /// The path of the image file, as written in the document.
    pub path: String,
}

/// The error that is reported when the strategy `copy` is set for an output
/// that consists of a single file (id `flm.graphics.copy-in-one-file`).
///
/// The strategy `copy` adds a copy of each image file to the output. It
/// requires an output that can contain several files, such as the `html`
/// output. The plain-text output `text` consists of a single file
/// ([`OutputImages::artifacts`]).
///
/// Reported by the publish stage of the output ([`PublishStage`]) for each
/// image. The output then refers to the image as under the strategy
/// `reference`: by the address of the file that the graphics resolver
/// returned, or by its path as written in the document if no graphics resolver
/// found the file.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.copy-in-one-file",
    message = "Unsupported strategy ‘copy’ for output ‘{output}’ (image file ‘{path}’): \
               this output is a single file"
)]
pub struct CopyInOneFile {
    /// The name of the output, which is the name of its backend.
    pub output: String,
    /// The path of the image file, as written in the document.
    pub path: String,
}

/// The error that is reported when the strategy `data-uri` is set for an
/// output that cannot use `data:` addresses (id
/// `flm.graphics.data-uri-not-shown`).
///
/// The strategy `data-uri` writes the content of each image file into the
/// address of the image, as in `<img src="data:image/png;base64,iVBORw0…">`.
/// Only the `html` output can use such addresses
/// ([`OutputImages::data_address`]).
///
/// Reported by the publish stage of the output ([`PublishStage`]) for each
/// image. The output then refers to the image as under the strategy
/// `reference`: by the address of the file that the graphics resolver
/// returned, or by its path as written in the document if no graphics resolver
/// found the file.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.data-uri-not-shown",
    message = "Unsupported strategy ‘data-uri’ for output ‘{output}’ (image file ‘{path}’): \
               this output cannot use ‘data:’ addresses"
)]
pub struct DataUriNotShown {
    /// The name of the output, which is the name of its backend.
    pub output: String,
    /// The path of the image file, as written in the document.
    pub path: String,
}

/// The error that is reported when the conversion rule that applies to an
/// image lists a converter that is not registered (id
/// `flm.graphics.unknown-converter`).
///
/// The `via` of a conversion rule lists the names of the converters that may
/// perform the rule ([`ConversionRule::via`]), as in `via: [raster]`. Each of
/// these names must be the name of a converter registered in the service slot
/// [`CONVERTERS_SLOT`] ([`GraphicsConverter::name`]).
///
/// Reported by the publish stage of the output ([`PublishStage`]) for each
/// image to which the rule applies. The image is then published with the
/// unconverted content of its file, as if no conversion rule applied.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.unknown-converter",
    message = "Unknown converter ‘{converter}’ in ‘via’ of the conversion rule from ‘{from}’ \
               to ‘{to}’ for output ‘{output}’"
)]
pub struct UnknownConverter {
    /// The name of the output, which is the name of its backend.
    pub output: String,
    /// The short name of the format of the image file, such as `pdf`.
    pub from: String,
    /// The short name of the format `to` of the conversion rule, such as
    /// `png`.
    pub to: String,
    /// The name of the converter, as listed in the `via` of the rule.
    pub converter: String,
}

/// The error that is reported when none of the converters allowed by a
/// conversion rule can convert an image file into the format `to` of the rule
/// (id `flm.graphics.no-converter`).
///
/// The converters allowed by a conversion rule are the converters that its
/// `via` lists, or every registered converter if `via` is empty. One of these
/// converters must offer the conversion from the format of the image file into
/// the format `to` of the rule ([`GraphicsConverter::offers`]). The publish
/// stage does not chain conversions: a single converter must perform the whole
/// conversion.
///
/// Reported by the publish stage of the output ([`PublishStage`]) for each
/// image to which the rule applies. The image is then published with the
/// unconverted content of its file, as if no conversion rule applied.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.no-converter",
    message = "No converter available to convert image file ‘{path}’ from ‘{from}’ to ‘{to}’ \
               (conversion rule for output ‘{output}’)"
)]
pub struct NoConverterOffers {
    /// The name of the output, which is the name of its backend.
    pub output: String,
    /// The path of the image file, as written in the document.
    pub path: String,
    /// The short name of the format of the image file, such as `pdf`.
    pub from: String,
    /// The short name of the format `to` of the conversion rule, such as
    /// `png`.
    pub to: String,
}

/// The error that is reported when a converter fails to convert an image file
/// (id `flm.graphics.conversion-failed`).
///
/// The message contains the reason that the converter returned
/// ([`GraphicsConvertError`](super::GraphicsConvertError)). Reported by the
/// publish stage of the output ([`PublishStage`]). The image is then published
/// with the unconverted content of its file, as if no conversion rule applied.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.conversion-failed",
    message = "Failed to convert image file ‘{path}’ from ‘{from}’ to ‘{to}’ with converter \
               ‘{converter}’: {detail}"
)]
pub struct ConversionFailed {
    /// The path of the image file, as written in the document.
    pub path: String,
    /// The short name of the format of the image file, such as `pdf`.
    pub from: String,
    /// The short name of the format `to` of the conversion rule, such as
    /// `png`.
    pub to: String,
    /// The name of the converter ([`GraphicsConverter::name`]).
    pub converter: String,
    /// The reason that the converter returned
    /// ([`GraphicsConvertError::detail`](super::GraphicsConvertError::detail)).
    pub detail: String,
}

/// The settings and services that a publish stage reads once for each run,
/// and uses for every image of the tree.
struct Publishing<'r> {
    /// The name of the output, which is the name of the backend.
    output: &'r str,
    strategy: Strategy,
    rules: &'r [ConversionRule],
    resolver: Option<&'r dyn GraphicsResolver>,
    converters: Vec<&'r dyn GraphicsConverter>,
    namer: Option<&'r dyn ArtifactNamer>,
    /// The outputs of the command, which the artifact namer receives with
    /// each copy.
    scope: &'r NamingScope,
    /// Whether the output can contain copies and use `data:` addresses.
    holds: OutputImages,
}

/// The content with which an image is published: the content of the file, or
/// the result of its conversion.
struct Content {
    bytes: Vec<u8>,
    media_type: String,
    /// The options of the conversion rule that converted the content, or
    /// `None` if the content is unconverted.
    options: Option<ConversionOptions>,
}

/// The result of publishing one image: the published address, and the copy or
/// the content that the strategy produces.
struct Publication {
    /// The published address.
    src: String,
    /// The copy of the image file, under the strategy `copy`.
    copy: Option<Artifact>,
    /// The content to write into the content table, under the strategy
    /// `content`.
    content: Option<Content>,
}

impl Publication {
    /// Returns the publication of an image at the address `src`, with no copy
    /// and no content.
    fn at(src: String) -> Publication {
        Publication { src, copy: None, content: None }
    }
}

/// Why the stage does not read the file of an image, which is then published
/// at its reference.
enum Unread {
    /// Nothing to report: the path names a scheme, the strategy needs no
    /// content, or the graphics resolver did not find the file, which was
    /// reported where the record was made.
    Quietly,
    /// The strategy is `copy`, and the output is a single file.
    CopyInOneFile,
    /// The strategy is `data-uri`, and the output cannot show such an address.
    DataUriNotShown,
    /// No graphics resolver is registered.
    NeedsResolver,
    /// The strategy is `copy`, and no artifact namer is registered.
    NeedsNamer,
}

impl<'r> Publishing<'r> {
    /// Returns the graphics resolver and the file through which the stage
    /// reads the content of an image, or why it does not read it. `path` is
    /// the path of the image as written in the document, and `file` the file
    /// that the graphics resolver found for it.
    fn readable<'f>(
        &self,
        path: &str,
        file: Option<&'f ImageFile>,
    ) -> Result<(&'r dyn GraphicsResolver, &'f ImageFile), Unread> {
        if names_scheme(path) || !self.strategy.needs_content() {
            return Err(Unread::Quietly);
        }
        match self.strategy {
            Strategy::Copy if self.holds.artifacts.is_none() => return Err(Unread::CopyInOneFile),
            Strategy::DataUri if !self.holds.data_address => return Err(Unread::DataUriNotShown),
            _ => {}
        }
        let resolver = self.resolver.ok_or(Unread::NeedsResolver)?;
        let file = file.ok_or(Unread::Quietly)?;
        if self.strategy == Strategy::Copy && self.namer.is_none() {
            return Err(Unread::NeedsNamer);
        }
        Ok((resolver, file))
    }

    /// Pushes to `diags` the error, if any, that says why the stage does not
    /// read the file of the image at `path`, written at `span`.
    fn report_unread(&self, unread: Unread, path: &str, span: &FlmSpan, diags: &mut Vec<FlmDiagnostic>) {
        if matches!(unread, Unread::Quietly) {
            return;
        }
        let (output, path, span) = (self.output.to_string(), path.to_string(), span.clone());
        match unread {
            Unread::Quietly => {}
            Unread::CopyInOneFile => diags.push(Diagnostic::error(CopyInOneFile { output, path }, span)),
            Unread::DataUriNotShown => diags.push(Diagnostic::error(DataUriNotShown { output, path }, span)),
            Unread::NeedsResolver => {
                let strategy = self.strategy.name().to_string();
                diags.push(Diagnostic::error(NeedsResolver { output, path, strategy }, span));
            }
            Unread::NeedsNamer => diags.push(Diagnostic::error(NeedsNamer { output, path }, span)),
        }
    }

    /// Returns the format of `file` and the first conversion rule of the
    /// output that applies to it, or `None` if the format is not known or no
    /// rule applies.
    fn rule_for(&self, file: &ImageFile) -> Option<(ImageFormat, &'r ConversionRule)> {
        let from = file.format()?;
        self.rules.iter().find(|rule| rule.applies_to(from)).map(|rule| (from, rule))
    }

    /// Returns whether a conversion rule applies to the image at `path`, whose
    /// record in the table [`INFO`] is `record`: whether the stage reads its
    /// file and a rule applies to the file's format. Reads no file.
    fn converts(&self, path: &str, record: Option<&GraphicsRecord>) -> bool {
        let file = record.and_then(|record| record.file.as_ref());
        self.readable(path, file).is_ok_and(|(_, file)| self.rule_for(file).is_some())
    }

    /// Publishes the image `node` under the strategy of the output, and
    /// returns the result. `path` is the path of the image as written in the
    /// document, and `record` is the record of the image in the table
    /// [`INFO`]. A conversion is reported on the progress sheet as a row under
    /// `images`. Pushes the errors to `diags`.
    fn publish(
        &self,
        node: &NodeRef<'_, Flm>,
        path: &str,
        record: Option<&GraphicsRecord>,
        images: &Counted,
        diags: &mut Vec<FlmDiagnostic>,
    ) -> Publication {
        let file = record.and_then(|record| record.file.as_ref());
        let reference = file.map_or_else(|| path.to_string(), |file| file.address.clone());
        let (resolver, file) = match self.readable(path, file) {
            Ok(readable) => readable,
            Err(unread) => {
                self.report_unread(unread, path, node.span(), diags);
                return Publication::at(reference);
            }
        };
        let original = match resolver.read(file) {
            Ok(bytes) => bytes,
            Err(failed) => {
                let failed = ResolveFailed { path: path.to_string(), detail: failed.detail };
                diags.push(Diagnostic::error(failed, node.span().clone()));
                return Publication::at(reference);
            }
        };
        let content = self.convert(node, path, file, &original, images, diags);
        match self.strategy {
            Strategy::Reference => Publication::at(reference),
            Strategy::Content => Publication { src: reference, copy: None, content: Some(content) },
            Strategy::DataUri => {
                let encoded = base64::engine::general_purpose::STANDARD.encode(&content.bytes);
                Publication::at(format!("data:{};base64,{encoded}", content.media_type))
            }
            Strategy::Copy => {
                let Some(namer) = self.namer else { return Publication::at(reference) };
                let id = ArtifactId::new(FEATURE, copy_name(path, &original, &content));
                let extension = ImageFormat::from_media_type(&content.media_type).map(ImageFormat::name);
                let artifact = ArtifactRef {
                    id: &id,
                    entry: false,
                    media_type: &content.media_type,
                    extension,
                    output: None,
                    folder: false,
                };
                let address = namer.url_of(&artifact, self.scope);
                let mut copy = Artifact::bytes(id, content.media_type, content.bytes);
                copy.extension = extension.map(String::from);
                Publication { src: address.as_str().to_string(), copy: Some(copy), content: None }
            }
        }
    }

    /// Returns the content `original` of `file`, converted by the first
    /// conversion rule of the output that applies to the format of the file.
    ///
    /// Returns the content unconverted if no rule applies, or if the
    /// conversion cannot be carried out. In the second case, pushes the error
    /// to `diags`. The conversion is a row of the kind [`CONVERT_KIND`] under
    /// `images`, whose scope the converter receives.
    fn convert(
        &self,
        node: &NodeRef<'_, Flm>,
        path: &str,
        file: &ImageFile,
        original: &[u8],
        images: &Counted,
        diags: &mut Vec<FlmDiagnostic>,
    ) -> Content {
        let unconverted = || Content {
            bytes: original.to_vec(),
            media_type: file.media_type.clone(),
            options: None,
        };
        let Some((from, rule)) = self.rule_for(file) else { return unconverted() };
        let span = node.span().clone();
        let (from_name, to_name) = (from.name().to_string(), rule.to.name().to_string());
        let converter = match choose_converter(rule, from, &self.converters) {
            Ok(converter) => converter,
            Err(NoConverter::Unknown(converter)) => {
                let output = self.output.to_string();
                let unknown = UnknownConverter { output, from: from_name, to: to_name, converter };
                diags.push(Diagnostic::error(unknown, span));
                return unconverted();
            }
            Err(NoConverter::NoneOffers) => {
                let none = NoConverterOffers {
                    output: self.output.to_string(),
                    path: path.to_string(),
                    from: from_name,
                    to: to_name,
                };
                diags.push(Diagnostic::error(none, span));
                return unconverted();
            }
        };
        let request = ConversionRequest::new(original, from, rule.to, &rule.options);
        // A conversion that fails ends its row as failed, when the row is
        // dropped.
        let row = images.scope().child(&CONVERT_KIND, Some(path)).start();
        match converter.convert(&request, &row.scope()) {
            Ok(bytes) => {
                row.finish();
                Content { bytes, media_type: rule.to.media_type().to_string(), options: Some(rule.options) }
            }
            Err(failed) => {
                let failed = ConversionFailed {
                    path: path.to_string(),
                    from: from_name,
                    to: to_name,
                    converter: converter.name().to_string(),
                    detail: failed.detail,
                };
                diags.push(Diagnostic::error(failed, span));
                unconverted()
            }
        }
    }
}

/// Returns the name of the copy of an image file. `path` is the path of the
/// file as written in the document, `original` is the content of the file,
/// and `content` is the content of the copy.
///
/// The name consists of the stem of the file and eight hexadecimal digits of a
/// digest. The digest is computed from `original`, the media type of
/// `content` and the options of the conversion rule that produced `content`.
/// See the [module documentation](self#copies-of-image-files).
fn copy_name(path: &str, original: &[u8], content: &Content) -> String {
    // The name is computed from the file and its conversion alone, with no
    // counter, so that a file shown twice, in one tree or two, on one thread or
    // several, is one copy.
    let mut digest = Fnv1a64::new();
    digest.write_bytes(original);
    digest.write_str(&content.media_type);
    content.options.hash(&mut digest);
    digest_name(&stem_of(path), &digest)
}

/// The publish stage of one backend, named `flm.graphics.publish.<backend>`.
/// See the [module documentation](self).
///
/// For each image of a tree, the stage computes the published address under
/// the strategy that the render options set for the output of the backend
/// ([`GraphicsRenderOptions`]). The stage writes the published addresses into
/// the published table of the backend ([`published_table`]). Under the
/// strategy `content`, it also writes the content of each image file into the
/// content table of the backend ([`content_table`]). Under the strategy
/// `copy`, it writes the copies of the image files into the artifact table of
/// the output ([`OutputImages::artifacts`]). The stage reads the records that
/// the inspect stage writes into the table [`INFO`].
///
/// On the progress sheet of the build, a call that converts images declares a
/// row of the kind [`IMAGES_KIND`], and under it a row of the kind
/// [`CONVERT_KIND`] for each conversion. Between two conversions, the call
/// asks whether the build was cancelled, and returns when it was.
///
/// The `graphics` feature registers a publish stage for each of the backends
/// `text`, `html`, `latex-recomposed` and `latex-rendered`. To use the
/// lowering rule [`image_rule`](super::image_rule) on a backend of your own,
/// register a publish stage for this backend with [`PublishStage::new`].
/// Also declare the published table of the backend (a [`GraphicsPublished`])
/// and its content table (a [`GraphicsContent`]), for instance with
/// [`FeatureBuilder::table_named`]. The output of such a backend cannot
/// contain copies or use `data:` addresses ([`output_images`]), so it can use
/// only the strategies `reference` and `content`.
#[derive(Debug, Clone)]
pub struct PublishStage {
    backend: String,
}

impl PublishStage {
    /// Returns the publish stage of `backend`.
    pub fn new(backend: impl Into<String>) -> PublishStage {
        PublishStage { backend: backend.into() }
    }
}

impl Stage for PublishStage {
    fn decl(&self) -> StageDecl {
        let decl = StageDecl::new(publish_stage_name(&self.backend), Effect::Compute, Scope::PerTree)
            .writes(published_table(&self.backend))
            .writes(content_table(&self.backend));
        let decl = match output_images(&self.backend).artifacts {
            Some(table) => decl.writes(table),
            None => decl,
        };
        let backend = &self.backend;
        let copies = match output_images(backend).artifacts {
            Some(table) => format!(
                " Under the strategy `copy`, writes the copies of the image files into `{}`.",
                table.name()
            ),
            None => String::new(),
        };
        decl.docs(format!(
            "Writes the address at which the `{backend}` output refers to each image into \
             `flm.graphics.published.{backend}`, according to the strategy set for this output. \
             Under the strategy `content`, writes the content of each image file into \
             `flm.graphics.content.{backend}`.{copies} Reports an error for each image whose \
             strategy or conversion cannot be carried out."
        ))
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(ELEMENT_INDEX).reads(INFO).option::<GraphicsRenderOptions>()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let (Some(id), Some(tree)) = (cx.tree_id(), cx.my_tree()) else { return };
        let stage = publish_stage_name(&self.backend);
        let index = match cx.get(ELEMENT_INDEX) {
            Ok(Some(index)) => index,
            Ok(None) => return,
            Err(error) => return stage_failed(out, stage.as_str(), error),
        };
        let info: Option<&GraphicsInfo> = match cx.get(INFO) {
            Ok(info) => info,
            Err(error) => return stage_failed(out, stage.as_str(), error),
        };
        let options = match cx.option::<GraphicsRenderOptions>() {
            Ok(options) => options,
            Err(error) => return stage_failed(out, stage.as_str(), error),
        };
        let services = cx.registry().services();
        let scope = NamingScope::of(cx.registry(), cx.plan().demands());
        let publishing = Publishing {
            output: &self.backend,
            strategy: options.strategy_for(&self.backend),
            rules: options.rules_for(&self.backend),
            resolver: services.service_named::<dyn GraphicsResolver>(RESOLVER_SLOT),
            converters: services.services_by_precedence_named::<dyn GraphicsConverter>(CONVERTERS_SLOT).collect(),
            namer: services.service_named::<dyn ArtifactNamer>(NAMER_SLOT),
            scope: &scope,
            holds: output_images(&self.backend),
        };
        // As the inspect stage: a path whose characters are not known here is
        // the rules' to handle.
        let images: Vec<_> = index
            .values_of::<GraphicsImage>(id, &tree)
            .filter_map(|(addr, node, _)| match read_path(&node) {
                Ok(ImagePath::Read(path)) => {
                    let record = info.and_then(|info| info.get(addr));
                    let converts = publishing.converts(&path, record);
                    Some((addr, node, path, record, converts))
                }
                _ => None,
            })
            .collect();
        let total = images.iter().filter(|(.., converts)| *converts).count() as u64;
        // A call with nothing to convert declares no row.
        let no_row = progress::Scope::none();
        let parent = if total == 0 { &no_row } else { cx.progress() };
        let label = cx.fragment(id).and_then(|fragment| fragment.source().origin().canonical_path());
        let converting = parent.child(&IMAGES_KIND, label).start_counted(Unit::Items, total);
        let mut published = GraphicsPublished::default();
        let mut contents = GraphicsContent::default();
        let mut copies = ArtifactSet::new();
        let mut diags = Vec::new();
        let mut first = true;
        for (addr, node, path, record, converts) in images {
            if converts {
                // What a cancelled step produced is discarded whole.
                if !first && cx.progress().cancelled() {
                    return;
                }
                first = false;
            }
            let Publication { src, copy, content } =
                publishing.publish(&node, &path, record, &converting, &mut diags);
            if converts {
                converting.advance(1);
            }
            if let Some(content) = content {
                let image = ImageContent { src: src.clone(), media_type: content.media_type, content: content.bytes };
                contents.images.insert(addr, image);
            }
            if let Some(copy) = copy {
                // A set is refused only a second entry, and a copy is no
                // entry; two copies under one identity are combined by
                // `absorb`, which keeps the first.
                let mut one = ArtifactSet::new();
                one.insert(copy)
                    .expect("a new artifact set accepts one artifact that is not an entry artifact");
                copies.absorb(one);
            }
            published.images.insert(addr, PublishedImage { src, srcset: Vec::new() });
        }
        converting.finish();
        for diagnostic in diags {
            out.diags.push(diagnostic);
        }
        match out.table(&published_table(&self.backend)) {
            Ok(table) => *table = published,
            Err(error) => return stage_failed(out, stage.as_str(), error),
        }
        // A row is no file of the output: a value's rows are written as a
        // tree's are.
        match out.table(&content_table(&self.backend)) {
            Ok(table) => *table = contents,
            Err(error) => return stage_failed(out, stage.as_str(), error),
        }
        // A value states no fact about the document: its images' copies are
        // the document's own trees'.
        if let (Some(table), false) = (publishing.holds.artifacts, id.is_value()) {
            match out.table(table) {
                Ok(written) => *written = copies,
                Err(error) => stage_failed(out, stage.as_str(), error),
            }
        }
    }
}

/// Declares the published table, the content table and the publish stage of
/// `backend`.
///
/// The backend is declared as a peer, so that a registry without the backend
/// still builds. If the output of the backend can contain copies, the artifact
/// table of the output is also declared as a peer table, since the backend
/// declares this table. In a registry without the backend, this peer table is
/// unresolved, and the publish stage, which writes the table, is not
/// registered.
pub(super) fn register(b: &mut FeatureBuilder<'_>, backend: &str) -> Result<(), BuildError> {
    b.peer_backend(
        backend,
        format!(
            "the `{backend}` backend, for which the publish stage computes the address of each image"
        ),
    )?;
    b.table_named::<GraphicsPublished>(
        published_table(backend).name().clone(),
        format!(
            "The address at which the `{backend}` output refers to each image, keyed by the \
             address of the image's node."
        ),
    )?;
    b.table_named::<GraphicsContent>(
        content_table(backend).name().clone(),
        format!(
            "The content of each image file under the strategy `content`, for a stage of the \
             `{backend}` output that renders images itself, keyed by the address of the image's \
             node. Empty under the other strategies."
        ),
    )?;
    if let Some(table) = output_images(backend).artifacts {
        b.peer_table::<ArtifactSet>(
            table.name(),
            format!(
                "the artifact table of the `{backend}` output, into which the publish stage writes \
                 the copies of image files"
            ),
        )?;
    }
    b.stage(Arc::new(PublishStage::new(backend)))
}
