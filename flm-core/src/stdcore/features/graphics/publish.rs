//! The address each backend writes for an image: one publish stage and one
//! table per backend with an image rule ([`PublishStage`],
//! [`published_table`]), and the copies of image files that an output holds.
//!
//! The **published address** of an image is the text a backend writes to
//! reach it. The output's **strategy** ([`Strategy`], chosen per output by the
//! render options [`GraphicsRenderOptions`]) decides how the address is found.
//! The stage `flm.graphics.publish.<backend>` ([`PublishStage`]) writes a
//! [`PublishedImage`] for each image (the address, and alternative sources,
//! of which no strategy gives any yet) into the table
//! `flm.graphics.published.<backend>` ([`published_table`]), keyed by node
//! address; that backend's image rule reads it. Each backend has its own
//! table, so the HTML page and the LaTeX of one document may publish their
//! images differently.
//!
//! # The three strategies
//!
//! - **`reference`**, the default: the address of the file where it is,
//!   which is the address the graphics resolver returned where it found the
//!   file ([`ImageFile::address`](super::ImageFile::address)), else the path
//!   as written. Nothing is read, copied or converted.
//! - **`copy`**: the file's content is read through the resolver, converted
//!   where a conversion rule applies (below), and made an artifact of the
//!   output, written into the output's artifact table
//!   `flm.output.<backend>.artifacts` ([`OutputImages::artifacts`]), which the
//!   output's final stage ([final
//!   stage](crate::guide::concepts_overview#output-regions-and-the-final-stage))
//!   adds to the output's artifact set. An artifact is
//!   one file's worth of output held as a value, identified by its producer
//!   and a name ([`ArtifactSet`]). The copy's identity is
//!   `graphics:<stem>-<digest>`: the file's stem (the last segment of the path
//!   as written, without its extension, every character other than a letter,
//!   a digit, `-` and `_` written `_`) and eight hexadecimal digits of a
//!   digest of the file's content, the copy's format and the conversion
//!   rule's options, as in `graphics:plot-3fa9c2d1`. The copy carries the
//!   short name of its format as its extension (`png`, `jpeg`;
//!   [`ImageFormat::name`]; none for a file of another kind). The address is
//!   where the host's artifact namer ([`ArtifactNamer`]) places that identity
//!   with that extension, for the outputs of the command
//!   ([`NamingScope`]), relative to where the output is written. A copy's
//!   identity and address therefore depend only on the file, its conversion
//!   and the outputs of the command, never on the rest of the document, the
//!   order of the work or an earlier run: a file shown twice, in one tree or
//!   in two, is one copy.
//! - **`data-uri`**: the file's content, converted where a conversion rule
//!   applies, is written into the address itself,
//!   `data:<media type>;base64,<content>`, and the output holds no other
//!   file. Only an HTML page can show such an address
//!   ([`OutputImages::data_address`]).
//!
//! A path that begins with a scheme (`https://…`; [`names_scheme`]) is
//! written as it is, regardless of the strategy. Where `copy` or `data-uri`
//! cannot be carried out, an Error is reported at the image and the image is
//! published by `reference`: when the output cannot use the strategy (`copy`
//! on an output that is a single file, such as the plain-text output;
//! `data-uri` on an output other than the HTML page), when no graphics
//! resolver is registered, when the file cannot be read, and, for `copy`,
//! when no artifact namer is registered.
//!
//! # Conversion
//!
//! Under `copy` and `data-uri`, the first of the output's conversion rules
//! whose `from` lists the file's format applies, and the converter chosen for
//! it ([`choose_converter`]) turns the content into the rule's `to` format;
//! the copy, or the address, then has that format. A rule that no converter
//! can perform, a converter's failure, and a `via` that lists a converter
//! that is not registered are each an Error at the image, which is then
//! published as the file is ([`convert`](super::convert)).
//!
//! # Where the stage runs
//!
//! The feature registers the stage for the backends it has a rule for:
//! `text`, `html` and `latex-recomposed`, each under a peer declaration
//! ([`PeerName`](crate::core::registry::PeerName)) of its backend, so that a
//! registry without one of them still builds. The stage runs once per tree,
//! as the inspect stage does, so that an image inside a synthesized value is
//! published when that value is brought up to date ([value
//! catch-up](crate::guide::concepts_overview#value-catch-up)). In a synthesized
//! value, the stage writes the address, the same as for the image in the
//! document's tree, and makes no copy: only the stage run over a document
//! tree writes copies into the output's artifact table. *Not yet
//! implemented*: an image that appears only in a synthesized value, never in
//! a tree of the document (on the recomposed LaTeX, the image of a user
//! definition whose path holds a placeholder, expanded at each use site), is
//! given its copy's address, and the output does not yet hold the copy.

use std::collections::BTreeMap;
use std::hash::Hash;
use std::sync::Arc;

use base64::Engine as _;
use techy::core::node::NodeRef;
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::serialize::SerialValue;

use crate::core::artifact::{Artifact, ArtifactId, ArtifactNamer, ArtifactRef, ArtifactSet, NamingScope};
use crate::core::data::store::StageOut;
use crate::core::data::table::{ProjectCx, Table, TableHandle, TableName};
use crate::core::plan::PlanCx;
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::{Flm, FlmDiagnostic};
use crate::model::addr::NodeAddr;
use crate::model::digest::Fnv1a64;
use crate::model::names::StageName;
use crate::stdcore::backends::lowering::NAMER_SLOT;
use crate::stdcore::backends::pres::SrcsetCandidate;
use crate::stdcore::backends::{html, latex_recomposed, latex_rendered};
use crate::stdcore::features::index::ELEMENT_INDEX;

use super::convert::{
    CONVERTERS_SLOT, ConversionOptions, ConversionRequest, ConversionRule, GraphicsConverter,
    NoConverter, choose_converter,
};
use super::inspect::{GraphicsInfo, GraphicsRecord, INFO, ResolveFailed, stage_failed};
use super::resolve::{GraphicsResolver, ImageFile, ImageFormat, RESOLVER_SLOT, names_scheme, stem_of};
use super::strategy::{GraphicsRenderOptions, Strategy};
use super::{FEATURE, GraphicsImage, ImagePath, read_path};

/// What a backend writes for one image: the published address, and
/// alternative sources.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PublishedImage {
    /// The published address, as the output's strategy decides it.
    pub src: String,
    /// Alternative sources, each with its descriptor; no strategy gives any
    /// yet, so the list is always empty.
    pub srcset: Vec<SrcsetCandidate>,
}

/// The value of a published table, `flm.graphics.published.<backend>`
/// ([`published_table`]): one [`PublishedImage`] per image, keyed by node
/// address.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphicsPublished {
    images: BTreeMap<NodeAddr, PublishedImage>,
}

impl GraphicsPublished {
    /// Returns what the backend writes for the image at `addr`, if the image
    /// was published.
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

    /// Returns `[ { node, src, srcset }, … ]` in node-address order, `srcset`
    /// only where the image has alternative sources.
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

/// Returns the published table of `backend`,
/// `flm.graphics.published.<backend>`, which that backend's publish stage
/// writes and its image rule reads.
pub fn published_table(backend: &str) -> TableHandle<GraphicsPublished> {
    TableHandle::new(format!("flm.graphics.published.{backend}"))
}

/// Returns the name of the publish stage of `backend`,
/// `flm.graphics.publish.<backend>`.
pub fn publish_stage_name(backend: &str) -> StageName {
    StageName::from(format!("flm.graphics.publish.{backend}"))
}

/// What the output of one backend can hold of an image, which decides the
/// strategies that output can use. [`output_images`] returns it for each
/// backend.
#[derive(Debug, Clone, Copy)]
pub struct OutputImages {
    /// The output's artifact table, `flm.output.<backend>.artifacts`, into
    /// which the publish stage writes the copies of image files under the
    /// strategy `copy`; `None` for an output that is a single file (the
    /// plain-text output), which holds no copy.
    pub artifacts: Option<TableName<ArtifactSet>>,
    /// Whether the output can show an image whose address begins with
    /// `data:`, as the strategy `data-uri` writes it: an HTML page can
    /// (`<img src="data:…">`); plain text and LaTeX's `\includegraphics`
    /// cannot.
    pub data_address: bool,
}

/// Returns what the output of `backend` can hold of an image
/// ([`OutputImages`]): `html` holds copies and shows data addresses,
/// `latex-recomposed` and `latex-rendered` hold copies, and `text`, like any
/// other backend, does neither.
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

/// Condition `flm.graphics.needs-resolver`, an Error: the output's strategy,
/// `copy` or `data-uri`, reads the image's file, and no graphics resolver is
/// registered to read it. Reported by the publish stage. Recovery: the image
/// is published by `reference`.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.needs-resolver",
    message = "the option `strategy` of the feature `graphics` chooses `{strategy}` for the output \
               `{output}`, which requires reading the image file `{path}`, and this program cannot \
               read image files"
)]
pub struct NeedsResolver {
    /// The output's name.
    pub output: String,
    /// The path, as written.
    pub path: String,
    /// The strategy, `copy` or `data-uri`.
    pub strategy: String,
}

/// Condition `flm.graphics.needs-namer`, an Error: the output's strategy is
/// `copy`, and no artifact namer is registered to place the copy
/// ([`NAMER_SLOT`]). Reported by the publish stage. Recovery: the image is
/// published by `reference`.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.needs-namer",
    message = "the option `strategy` of the feature `graphics` chooses `copy` for the output \
               `{output}`, which requires choosing a file name for the copy of the image `{path}`, \
               and this program does not choose names for the files of an output"
)]
pub struct NeedsNamer {
    /// The output's name.
    pub output: String,
    /// The path, as written.
    pub path: String,
}

/// Condition `flm.graphics.copy-in-one-file`, an Error: the output's strategy
/// is `copy`, and the output is a single file, which cannot hold a copy (the
/// plain-text output). Reported by the publish stage. Recovery: the image is
/// published by `reference`.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.copy-in-one-file",
    message = "the option `strategy` of the feature `graphics` may not choose `copy` for the output \
               `{output}`, which is a single file and so cannot hold a copy of the image `{path}`"
)]
pub struct CopyInOneFile {
    /// The output's name.
    pub output: String,
    /// The path, as written.
    pub path: String,
}

/// Condition `flm.graphics.data-uri-not-shown`, an Error: the output's
/// strategy is `data-uri`, and the output cannot show an address that holds
/// the file's content (the plain-text output, either LaTeX output). Reported
/// by the publish stage. Recovery: the image is published by `reference`.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.data-uri-not-shown",
    message = "the option `strategy` of the feature `graphics` may not choose `data-uri` for the \
               output `{output}`, because that output cannot show an image whose content is written \
               into its own address (a `data:` address), as the image `{path}` would be"
)]
pub struct DataUriNotShown {
    /// The output's name.
    pub output: String,
    /// The path, as written.
    pub path: String,
}

/// Condition `flm.graphics.unknown-converter`, an Error: the conversion rule
/// that applies to an image lists, in its `via`, a converter that is not
/// registered. Reported by the publish stage. Recovery: the image is published
/// as the file is, unconverted.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.unknown-converter",
    message = "the conversion rule from {from} to {to} for the output `{output}` (option `convert` \
               of the feature `graphics`) lists the converter `{converter}` in `via`, and this \
               program has no converter of that name"
)]
pub struct UnknownConverter {
    /// The output's name.
    pub output: String,
    /// The image's format, by its short name.
    pub from: String,
    /// The rule's target format, by its short name.
    pub to: String,
    /// The converter's name, as the rule's `via` lists it.
    pub converter: String,
}

/// Condition `flm.graphics.no-converter`, an Error: no converter that the
/// conversion rule allows can turn the image's format into the rule's `to`
/// format. Reported by the publish stage. Recovery: the image is published as
/// the file is, unconverted.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.no-converter",
    message = "the conversion rule from {from} to {to} for the output `{output}` (option `convert` \
               of the feature `graphics`) applies to the image `{path}`, and none of the converters \
               that the rule allows can convert {from} to {to}"
)]
pub struct NoConverterOffers {
    /// The output's name.
    pub output: String,
    /// The path, as written.
    pub path: String,
    /// The image's format, by its short name.
    pub from: String,
    /// The rule's target format, by its short name.
    pub to: String,
}

/// Condition `flm.graphics.conversion-failed`, an Error: the converter chosen
/// for the image could not convert it; the message gives the converter's own
/// words. Reported by the publish stage. Recovery: the image is published as
/// the file is, unconverted.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.conversion-failed",
    message = "the image `{path}` could not be converted from {from} to {to} by `{converter}`: \
               {detail}"
)]
pub struct ConversionFailed {
    /// The path, as written.
    pub path: String,
    /// The image's format, by its short name.
    pub from: String,
    /// The rule's target format, by its short name.
    pub to: String,
    /// The converter's name.
    pub converter: String,
    /// The converter's own words.
    pub detail: String,
}

/// What a publish stage reads once per run of the stage, and uses for every
/// image.
struct Publishing<'r> {
    /// The output's name, which is the backend's name.
    output: &'r str,
    strategy: Strategy,
    rules: &'r [ConversionRule],
    resolver: Option<&'r dyn GraphicsResolver>,
    converters: Vec<&'r dyn GraphicsConverter>,
    namer: Option<&'r dyn ArtifactNamer>,
    /// The outputs of the command, which the namer receives with each copy.
    scope: &'r NamingScope,
    /// What the output can hold of an image.
    holds: OutputImages,
}

/// The content an image is published with: the file's content, or its
/// conversion.
struct Content {
    bytes: Vec<u8>,
    media_type: String,
    /// The options of the rule that converted it, where one did.
    options: Option<ConversionOptions>,
}

impl Publishing<'_> {
    /// Returns the published address of the image `node`, whose path is
    /// `path` and whose record is `record`, and the copy the output holds for
    /// it, where the strategy makes one. Pushes the diagnostics to `diags`.
    fn publish(
        &self,
        node: &NodeRef<'_, Flm>,
        path: &str,
        record: Option<&GraphicsRecord>,
        diags: &mut Vec<FlmDiagnostic>,
    ) -> (String, Option<Artifact>) {
        let file = record.and_then(|record| record.file.as_ref());
        let reference = file.map_or_else(|| path.to_string(), |file| file.address.clone());
        if names_scheme(path) || !self.strategy.needs_content() {
            return (reference, None);
        }
        let span = node.span().clone();
        match self.strategy {
            Strategy::Copy if self.holds.artifacts.is_none() => {
                let output = self.output.to_string();
                diags.push(Diagnostic::error(CopyInOneFile { output, path: path.to_string() }, span));
                return (reference, None);
            }
            Strategy::DataUri if !self.holds.data_address => {
                let output = self.output.to_string();
                diags.push(Diagnostic::error(DataUriNotShown { output, path: path.to_string() }, span));
                return (reference, None);
            }
            _ => {}
        }
        let Some(resolver) = self.resolver else {
            let strategy = self.strategy.name().to_string();
            let output = self.output.to_string();
            diags.push(Diagnostic::error(NeedsResolver { output, path: path.to_string(), strategy }, span));
            return (reference, None);
        };
        // The resolver's failure to find the file was reported where the
        // record was made.
        let Some(file) = file else { return (reference, None) };
        if self.strategy == Strategy::Copy && self.namer.is_none() {
            let output = self.output.to_string();
            diags.push(Diagnostic::error(NeedsNamer { output, path: path.to_string() }, span));
            return (reference, None);
        }
        let original = match resolver.read(file) {
            Ok(bytes) => bytes,
            Err(failed) => {
                let failed = ResolveFailed { path: path.to_string(), detail: failed.detail };
                diags.push(Diagnostic::error(failed, span));
                return (reference, None);
            }
        };
        let content = self.convert(node, path, file, &original, diags);
        match self.strategy {
            Strategy::Reference => (reference, None),
            Strategy::DataUri => {
                let encoded = base64::engine::general_purpose::STANDARD.encode(&content.bytes);
                (format!("data:{};base64,{encoded}", content.media_type), None)
            }
            Strategy::Copy => {
                let Some(namer) = self.namer else { return (reference, None) };
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
                (address.as_str().to_string(), Some(copy))
            }
        }
    }

    /// Returns the content `original` of `file`, converted by the first of
    /// the output's conversion rules that applies to its format; unconverted
    /// where no rule applies, and where the conversion cannot be made, which
    /// is reported to `diags`.
    fn convert(
        &self,
        node: &NodeRef<'_, Flm>,
        path: &str,
        file: &ImageFile,
        original: &[u8],
        diags: &mut Vec<FlmDiagnostic>,
    ) -> Content {
        let unconverted = || Content {
            bytes: original.to_vec(),
            media_type: file.media_type.clone(),
            options: None,
        };
        let Some(from) = file.format() else { return unconverted() };
        let Some(rule) = self.rules.iter().find(|rule| rule.applies_to(from)) else {
            return unconverted();
        };
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
        match converter.convert(&request) {
            Ok(bytes) => Content {
                bytes,
                media_type: rule.to.media_type().to_string(),
                options: Some(rule.options),
            },
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

/// Returns the name of the copy of a file whose path is `path` and whose
/// content is `original`, published as `content`: the file's stem and eight
/// hexadecimal digits of a digest of the original content, the copy's media
/// type and the options of the rule that converted it. See the [module
/// documentation](self#the-three-strategies).
fn copy_name(path: &str, original: &[u8], content: &Content) -> String {
    // See decision rationale at [§flm-dd-dr:std-graphics-copy-identity]
    let mut digest = Fnv1a64::new();
    digest.write_bytes(original);
    digest.write_str(&content.media_type);
    content.options.hash(&mut digest);
    format!("{}-{:08x}", stem_of(path), std::hash::Hasher::finish(&digest) >> 32)
}

/// The publish stage of one backend, `flm.graphics.publish.<backend>`: a
/// stage that computes, once per tree, the backend's published table and,
/// under the strategy `copy`, writes the copies into the output's artifact
/// table. It reads the table of records [`INFO`] and the render options
/// [`GraphicsRenderOptions`]. See the [module documentation](self).
///
/// The feature registers one for each of its backends. A host whose own
/// backend shows images as the plain-text and HTML backends do registers one
/// for that backend, declares that backend's published table
/// ([`published_table`], a [`GraphicsPublished`]) and registers
/// [`image_rule`](super::image_rule) for it; the output of such a backend
/// holds no copy and shows no data address ([`output_images`]).
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
            .writes(published_table(&self.backend));
        let decl = match output_images(&self.backend).artifacts {
            Some(table) => decl.writes(table),
            None => decl,
        };
        decl.docs(format!(
            "Writes the address that the `{}` backend writes for each image, as the output's \
             strategy decides it, into `flm.graphics.published.{}`{}.",
            self.backend,
            self.backend,
            match output_images(&self.backend).artifacts {
                Some(table) => format!(", and the copies of image files into `{}`", table.name()),
                None => String::new(),
            }
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
        let mut published = GraphicsPublished::default();
        let mut copies = ArtifactSet::new();
        let mut diags = Vec::new();
        for (addr, node, _) in index.values_of::<GraphicsImage>(id, &tree) {
            // As the inspect stage: a path whose characters are not known here
            // is the rules' to handle.
            let Ok(ImagePath::Read(path)) = read_path(&node) else { continue };
            let record = info.and_then(|info| info.get(addr));
            let (src, copy) = publishing.publish(&node, &path, record, &mut diags);
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
        for diagnostic in diags {
            out.diags.push(diagnostic);
        }
        match out.table(&published_table(&self.backend)) {
            Ok(table) => *table = published,
            Err(error) => return stage_failed(out, stage.as_str(), error),
        }
        // A value states no fact about the document: its images' copies are
        // the document's own trees'.
        // See decision rationale at [§flm-dd-dr:std-graphics-copy-identity]
        if let (Some(table), false) = (publishing.holds.artifacts, id.is_value()) {
            match out.table(table) {
                Ok(written) => *written = copies,
                Err(error) => stage_failed(out, stage.as_str(), error),
            }
        }
    }
}

/// Declares, for `backend`, the published table and the publish stage, under
/// a peer declaration of the backend: a registry without the backend still
/// builds, and no plan schedules the stage, since only that backend's image
/// rule reads its table. For a backend whose output holds copies (`html`,
/// `latex-recomposed`), the output's artifact table is declared as a peer
/// table, since the backend owns it; where the backend is not registered, the
/// stage, which writes that table, is not registered either.
pub(super) fn register(b: &mut FeatureBuilder<'_>, backend: &str) -> Result<(), BuildError> {
    b.peer_backend(
        backend,
        format!("the `{backend}` backend, for which the publish stage writes the image addresses"),
    )?;
    b.table_named::<GraphicsPublished>(
        published_table(backend).name().clone(),
        format!("the address the `{backend}` backend writes for each image, by node address"),
    )?;
    if let Some(table) = output_images(backend).artifacts {
        b.peer_table::<ArtifactSet>(
            table.name(),
            format!(
                "the `{backend}` output's artifact table, into which the copies of image files go"
            ),
        )?;
    }
    b.stage(Arc::new(PublishStage::new(backend)))
}
