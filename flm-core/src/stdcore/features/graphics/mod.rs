//! The `graphics` feature: images, written `\includegraphics[options]{path}`.
//!
//! An **image** is an external picture file that a document shows, referred
//! to by a **path** as the author wrote it: `figures/plot.png`,
//! `https://example.org/a.svg`. The library itself never opens the file.
//!
//! # What an author writes
//!
//! | written | shows |
//! |---|---|
//! | `\includegraphics{figures/plot.png}` | the picture, at its own size |
//! | `\includegraphics[alt=A plot]{plot.png}` | the picture, with the alternative text `A plot` |
//! | `\includegraphics[width=5cm]{plot.png}` | the picture 5cm wide, its height keeping its proportions |
//!
//! ## Where the command may be written
//!
//! `\includegraphics` is declared in the scoped package
//! [`INCLUDEGRAPHICS_PACKAGE`] (`flm.graphics.includegraphics`). A scoped
//! package ([`ScopedPackageDef`]) is a set of element declarations that can
//! be written only inside the environment bodies and arguments whose
//! declarations refer to the package by name ([scoped
//! packages](crate::guide::concepts_overview#scoped-packages)). Under the
//! default configuration, `\includegraphics` can therefore be written in the
//! body of a float whose type refers to that package (a `figure` or a
//! `table`), and is an unknown command elsewhere. Under
//! [`GraphicsConfig::global`], the feature also declares the command as an
//! ordinary command of the document, so an image can be written wherever text
//! can: in a sentence, a paragraph of its own, a heading's title, a caption,
//! a list item, a footnote. The package is declared under both
//! configurations, so a float body that refers to it always finds it.
//!
//! ## Block or inline
//!
//! An image is **inline content**, one word of the text it is written in,
//! everywhere except in one position: as a direct child of a float's body, it
//! is a **block**. The element gives its block role per occurrence from that
//! position ([`GraphicsImage`]). Where the command can be written and where an
//! image is a block are two separate questions: `See
//! \includegraphics{icon.png} here.` is one sentence holding an image; an image
//! written in a paragraph of its own is a paragraph holding an image; and the
//! image of a `figure` is the figure's content, a block, with no paragraph
//! around it. An image that should be shown as a block outside the running
//! text is written as the content of a float, with `\nonumber` where no number
//! is wanted.
//!
//! ## The options
//!
//! `[options]` holds key-value pairs ([`options`](mod@options)):
//!
//! - `alt=<text>`: the image's **alternative text**, which a reader who cannot
//!   see the picture is given instead. Its value must be plain text; a value
//!   holding a formula or a command is an Error, and the image then has no
//!   alternative text.
//! - `width=<length>` and `height=<length>`: the size the image is shown at,
//!   a [`Length`](crate::stdcore::backends::pres::Length) in one of the four
//!   physical units `pt` (a point, 1/72 inch), `mm`, `cm` and `in`, greater
//!   than zero: `width=5cm`, `height=2.5in`, `width=144pt`. A length in `em`
//!   or `tab`, a value that is not a length (`300px`, `0.5\linewidth`, `50%`,
//!   `10bp`), and a zero or negative length are each an Error, and the option
//!   is ignored. Each `width=` or `height=` that takes effect is reported with
//!   a **Warning**, because an image file prepared at the size it is shown at
//!   looks better than one resized by the output.
//!
//! Any other key is an Error; a key written twice is an Error, and the first
//! is kept; a key written without a value, and a value written without a key
//! (`[=5cm]`), are each an Error. Each such option is ignored, and the image
//! is still shown. These problems are reported wherever the image is lowered,
//! on each output: where the image is written, and again in each copy of
//! content holding it that a reference shows.
//!
//! ## The path
//!
//! `{path}` is raw text up to the matching `}` ([`ArgStdKind::Verbatim`]):
//! `%`, `#`, `~`, `\` and nested braces are characters of the path. It is read
//! as characters where the image is lowered ([`chars_at_lowering`]), so a
//! [placeholder](crate::lang::rewrites::userdefs::template) of a user
//! definition in a path is replaced by the argument's
//! characters (`\includegraphics{figs/#1.png}`). A path that is not characters
//! is the Error [`PathNotCharacters`], and an empty path (`\includegraphics{}`)
//! the Error [`EmptyPath`]; neither shows an image.
//!
//! ## The size an image is shown at
//!
//! The image's width and height are decided where it is lowered
//! ([`shown_size`]), from the options and from what the inspector reported
//! ([`GraphicsRecord`]): with both options given, both as given; with one
//! given, the other scaled from the inspected size so that the image keeps
//! its proportions, in the given option's unit; with neither, the inspected
//! size; where nothing was inspected, the given dimension alone, or none.
//!
//! Every dimension so decided, including one the author gave, is then
//! multiplied by the **magnification** of the output the image is shown on
//! ([`GraphicsRenderOptions::magnification`]): the output's factor for raster
//! images where the inspector reported a raster image, its factor for vector
//! images otherwise (a vector image, or an image whose type is not known). The
//! factors are 1 unless set, so that by default an image keeps its size.
//!
//! An image whose size is not decided at all (neither option given and
//! nothing inspected: no inspector is registered, the inspection failed, or
//! the path is an address such as `https://…`, which is never inspected) is
//! reported with the Warning [`UnknownSize`], on each output, and is written
//! without a size.
//!
//! # What the feature provides to the pipeline
//!
//! The `graphics` feature ([`Graphics`], an [`FlmFeature`]) registers:
//!
//! - **The element** [`IMAGE_ELEMENT`] (`flm.graphics.image`), whose value is
//!   [`GraphicsImage`], declared as `\includegraphics` in the scoped package
//!   [`INCLUDEGRAPHICS_PACKAGE`] and, under [`GraphicsConfig::global`], as an
//!   ordinary command too.
//! - **The argument role** [`PATH_ARG_ROLE`] (`flm.graphics.path`), which
//!   states that an argument holds the path of an image file
//!   ([`ArgDef::role`](crate::lang::mint::ArgDef::role)): the role of the path
//!   argument of `\includegraphics`.
//! - **Three service slots**, for the work that needs access to files or to
//!   programs installed on the computer, which the library itself does not
//!   do. A service is a value that can be shared across threads which one
//!   feature supplies for another feature, or for the embedding program, to
//!   use; a service slot
//!   is the declaration, by the feature that owns an interaction, of a dotted
//!   name, the type of the values the slot takes and its arity
//!   ([`core::services`](crate::core::services)). A host contributes the
//!   values; the feature requires none of them. The three services divide the
//!   work:
//!   - the **graphics resolver** ([`GraphicsResolver`], slot [`RESOLVER_SLOT`],
//!     at most one) finds the file a path refers to and reads it
//!     ([`resolve`](mod@resolve));
//!   - the **inspector** ([`GraphicsInspector`], slot [`INSPECTOR_SLOT`], at
//!     most one) reports what an image file states, measuring the content
//!     that the graphics resolver read where a resolver is registered
//!     ([`inspect`](mod@inspect));
//!   - the **converters** ([`GraphicsConverter`], slot [`CONVERTERS_SLOT`], any
//!     number) turn content of one format into another
//!     ([`convert`](mod@convert)).
//!
//!   With none of them, all that is known of an image is its path, and every
//!   output writes the path as written.
//! - **The inspect stage** [`INSPECT_STAGE`] (`flm.graphics.inspect`), which
//!   asks the resolver and the inspector about every image and writes their
//!   results into the table [`INFO`] (`flm.graphics.info`), keyed by the
//!   image's node address ([`inspect`](mod@inspect)).
//! - **One publish stage per backend** with an image rule,
//!   `flm.graphics.publish.<backend>` ([`PublishStage`]), which writes the
//!   address that backend writes for each image into the table
//!   `flm.graphics.published.<backend>` ([`published_table`]), as the output's
//!   **strategy** decides it: the file where it is, a copy made part of the
//!   output, or the file's content written into the address
//!   ([`strategy`](mod@strategy), [`publish`](mod@publish)).
//! - **The lowering rules.** On the plain-text and HTML backends, the rule
//!   [`image_rule`] lowers an image to a
//!   [`flm.pres.image`](crate::stdcore::backends::pres::IMAGE) node carrying
//!   the published address, whether the image is a block or inline, the
//!   alternative text and the size
//!   ([`ImagePayload`](crate::stdcore::backends::pres::ImagePayload)). The
//!   plain-text output prints it as `[image: <alternative text or address>]`;
//!   the HTML backend writes it as an `<img>`, with the block class `flm-pres-block`
//!   before the image's own class where the image is a block. On the
//!   `latex-recomposed` backend, the rule [`latex::LATEX_RULE`] writes the
//!   LaTeX hook `\flmIncludeGraphics[<keys>]{<path>}` ([`latex`](mod@latex)).
//! - **The settings.** The configuration [`GraphicsConfig`] (the key
//!   `global`) is the plain Rust value the feature is constructed from; it is
//!   fixed when the registry is built and cannot vary per document or per
//!   render ([settings, options and
//!   configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
//!   The render options [`GraphicsRenderOptions`] are set per render request:
//!   for each output, the strategy (`strategy`: `reference`, the default,
//!   `copy` or `data-uri`), the conversion rules (`convert`,
//!   [`ConversionRule`]) and the magnification of raster and vector images
//!   (`magnification`, [`Magnification`]). Each is a map from an output's
//!   name to its value.
//!   When a host combines several sources of settings, a source that writes
//!   one output's entry leaves the other outputs' entries as the lower-ranked
//!   sources wrote them
//!   ([`SettingsDecl::merge_entries`](crate::core::settings::SettingsDecl::merge_entries)).
//!
//! The rendering guide shows what each output writes ([images and how an
//! output reaches them](crate::guide::rendering#images-and-how-an-output-reaches-them),
//! [images on the plain-text output](crate::guide::rendering#images), [images
//! in HTML](crate::guide::rendering#images-in-html), [images in the recomposed
//! LaTeX](crate::guide::rendering#images-in-the-recomposed-latex)); the
//! language guide describes images for authors
//! ([Graphics](crate::guide::language_syntax#graphics)).
//!
//! # Interactions with other features
//!
//! - **Floats.** The feature contributes the content handler `includegraphics`
//!   to the `floats` feature ([`handler`](mod@handler)): a float type that lists
//!   it (`figure` and `table` do by default) accepts a body holding exactly
//!   one image beside its caption and labels. The contribution is a peer
//!   contribution, dropped with one Note where the `floats` feature is not
//!   registered. Without the `floats` feature and under the default
//!   configuration, no body refers to the scoped package, so no document can
//!   write an image; the feature is useful with floats, or on its own under
//!   [`GraphicsConfig::global`].
//! - **The backends.** Each publish stage and rule is registered under a peer
//!   declaration ([`PeerName`](crate::core::registry::PeerName)) of its
//!   backend, which tells the registry build that another feature owns that
//!   name, so a registry without one of the backends still builds. Under the
//!   strategy `copy`, the publish stage writes the copies into the output's
//!   artifact table of the `html` or `latex-recomposed` backend, and asks the
//!   host's artifact namer
//!   ([`ArtifactNamer`](crate::core::artifact::ArtifactNamer)) for their
//!   addresses.
//! - **The element index.** The stages find the images of a tree through the
//!   element index
//!   ([`ElementIndex`](crate::stdcore::features::index::ElementIndex), table
//!   [`ELEMENT_INDEX`](crate::stdcore::features::index::ELEMENT_INDEX)).
//!
//! # The path and the options are not words of the text
//!
//! Neither the path nor the options are words of the text an image is written
//! in: the element declares its path argument and its whole options argument
//! through the aspect [`ApartFromText`], so a heading holding an image is
//! identified by its other words alone. The alternative text is left out with
//! the options: the optional argument is one run of characters, so its value
//! cannot be kept without its keys (`alt`, `width=5cm`).
//!
//! # Map of the items
//!
//! - [`Graphics`], [`feature`], [`FEATURE`], [`GraphicsConfig`]: the feature
//!   and its configuration.
//! - [`IMAGE_ELEMENT`], [`GraphicsImage`], [`INCLUDEGRAPHICS_PACKAGE`],
//!   [`COMMAND`], [`OPTIONS_ARG`], [`PATH_ARG`], [`arguments`]: the element
//!   and its command; [`PathNotCharacters`], [`EmptyPath`]: the conditions of
//!   the path.
//! - [`options`](mod@options): reading the options and their conditions.
//! - [`resolve`](mod@resolve), [`inspect`](mod@inspect),
//!   [`convert`](mod@convert): the three services, the record table [`INFO`]
//!   and the inspect stage.
//! - [`strategy`](mod@strategy), [`publish`](mod@publish): the strategies, the
//!   render options and the publish stages.
//! - [`handler`](mod@handler): the content handler for floats.
//! - [`rules`](mod@rules): the image rule of the plain-text and HTML backends;
//!   [`latex`](mod@latex): the rule of the `latex-recomposed` backend.
//!
//! [`ScopedPackageDef`]: crate::lang::mint::ScopedPackageDef
//! [`shown_size`]: crate::stdcore::features::graphics::rules::shown_size

pub mod convert;
pub mod handler;
pub mod inspect;
pub mod latex;
pub mod options;
pub mod publish;
pub mod resolve;
pub mod rules;
pub mod strategy;

use std::sync::Arc;

use techy::core::node::NodeRef;
use techy::error::DiagnosticInfo;

use crate::core::settings::SettingsDecl;
use crate::model::options::Bind;
use crate::core::lowering::chars_at_lowering;
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::core::services::Arity;
use crate::lang::mint::{ArgDef, ArgStdKind, ElementDef, ScopedPackageDef, SyntaxDef};
use crate::lang::Flm;
use crate::model::aspects::{ApartFromText, BlockRole, BlockRoleAspect, ContentRef};
use crate::model::element::{ElementSpec, element_of};
use crate::model::names::{FeatureName, Name};
use crate::stdcore::backends::lowering::rules::owning_callable;
use crate::stdcore::backends::{html, text};
use crate::stdcore::features::floats::{CONTENT_SLOT, FLOAT, FloatContentHandler};

pub use convert::{
    CONVERTERS_SLOT, ConversionOptions, ConversionRequest, ConversionRule, GraphicsConvertError,
    GraphicsConverter, NoConverter, choose_converter,
};
pub use handler::IncludeGraphicsContent;
pub use inspect::{
    Dimensions, GraphicsInfo, GraphicsInspectError, GraphicsInspectRequest,
    GraphicsInspectedInformation, GraphicsInspector, GraphicsRecord, PixelSize,
};
pub use publish::{
    GraphicsPublished, OutputImages, PublishStage, PublishedImage, output_images, published_table,
};
pub use resolve::{
    GraphicsResolveError, GraphicsResolveRequest, GraphicsResolver, ImageFile, ImageFormat,
    RESOLVER_SLOT, ResolvedImage, names_scheme,
};
pub use strategy::{Factor, FactorError, GraphicsRenderOptions, Magnification, Strategy};
pub use rules::{ImageRule, UnknownSize, image_rule};

/// The feature's registered name, `graphics`.
pub const FEATURE: &str = "graphics";

/// Returns the declaration of the feature's settings: the fields of [`GraphicsConfig`] and of
/// [`GraphicsRenderOptions`] are its keys; `strategy`, `convert` and `magnification` are each
/// combined entry by entry, one entry per output, with no default entry. See [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<GraphicsConfig>(
            "Whether `\\includegraphics` is also an ordinary command of the document (`global`, \
            false by default), besides a command of the scoped package \
            `flm.graphics.includegraphics`, which a float type can make available in its body \
            (the `figure` and `table` types do). Fixed when the registry is built: it cannot \
            vary per document or per render.",
        )
        .options::<GraphicsRenderOptions>(
            Bind::Render,
            "How each output publishes its images (`strategy`: `reference`, `copy` or \
            `data-uri`), the conversion rules it applies (`convert`), and the factors by which \
            it multiplies the size of raster and of vector images (`magnification`: `raster` \
            and `vector`, 1 by default), each a map from an output's name to its value. Set \
            per render request.",
        )
        .merge_entries("strategy", || Ok(serde_json::Value::Object(serde_json::Map::new())))
        .merge_entries("convert", || Ok(serde_json::Value::Object(serde_json::Map::new())))
        .merge_entries("magnification", || Ok(serde_json::Value::Object(serde_json::Map::new())))
}

/// The name of the element of `\includegraphics`, `flm.graphics.image`. Its
/// element value carries a [`GraphicsImage`].
pub const IMAGE_ELEMENT: &str = "flm.graphics.image";

/// The name of the scoped package holding `\includegraphics`,
/// `flm.graphics.includegraphics`: `\includegraphics` can be written in a
/// body whose declaration refers to it. A float type lists it among its
/// scoped packages
/// ([`FloatType::scoped_packages`](crate::stdcore::features::floats::FloatType::scoped_packages));
/// the feature declares the package under every configuration.
pub const INCLUDEGRAPHICS_PACKAGE: &str = "flm.graphics.includegraphics";

/// The name of the command, without the backslash, `includegraphics`.
pub const COMMAND: &str = "includegraphics";

/// The name of the command's optional argument, `options`, which holds
/// key-value pairs.
pub const OPTIONS_ARG: &str = "options";

/// The name of the command's mandatory argument, `path`, which holds the
/// image's path as raw text.
pub const PATH_ARG: &str = "path";

/// The argument role of the path of an image file, `flm.graphics.path`: the
/// role of the path argument ([`PATH_ARG`]) of `\includegraphics`.
pub const PATH_ARG_ROLE: &str = "flm.graphics.path";

/// The name of the inspector's service slot, `flm.graphics.inspector`: it
/// takes one [`GraphicsInspector`] at most ([`Arity::Single`]), which a host
/// contributes and the feature never requires. See [`inspect`](mod@inspect).
pub const INSPECTOR_SLOT: &str = "flm.graphics.inspector";

pub use inspect::{INFO, INSPECT_STAGE};

/// The configuration of the `graphics` feature: whether `\includegraphics`
/// is also an ordinary command of the document.
///
/// Under the default, `global: false`, the command can be written only inside
/// the bodies whose declarations refer to the scoped package
/// [`INCLUDEGRAPHICS_PACKAGE`], such as a `figure`'s body when the `floats`
/// feature is registered. Under `global: true`, it is also an ordinary
/// command, which can be written wherever text can. In both cases an image is
/// inline content, except as a direct child of a float's body, where it is a
/// block.
///
/// A feature's configuration is the plain Rust value the feature is
/// constructed from; it is fixed when the registry is built and cannot vary
/// per document or per render ([settings, options and
/// configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
/// This one decides which commands the language has, so the feature adds
/// [`digest_bytes`](GraphicsConfig::digest_bytes) to the registry digest. The
/// standard package takes it through
/// [`StdcoreConfig::graphics`](crate::stdcore::StdcoreConfig::graphics).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::graphics::GraphicsConfig;
///
/// assert!(!GraphicsConfig::new().global);
/// assert!(GraphicsConfig::new().global(true).global);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct GraphicsConfig {
    /// Whether `\includegraphics` is also an ordinary command of the
    /// document, which can be written wherever text can, besides a command of
    /// its scoped package. `false` by default.
    pub global: bool,
}

impl GraphicsConfig {
    /// Returns the default configuration: the command is declared in its
    /// scoped package only.
    pub fn new() -> GraphicsConfig {
        GraphicsConfig::default()
    }

    /// Builder: sets [`global`](GraphicsConfig::global).
    pub fn global(mut self, global: bool) -> GraphicsConfig {
        self.global = global;
        self
    }

    /// Returns the bytes the feature adds to the registry digest: one byte,
    /// `1` under `global` and `0` otherwise.
    pub fn digest_bytes(&self) -> Vec<u8> {
        // Taken apart by name, so a field added and not written here fails
        // to compile.
        let GraphicsConfig { global } = self;
        vec![u8::from(*global)]
    }
}

/// The data of the element value of [`IMAGE_ELEMENT`], which implements the
/// element's two aspects: its block role, given per occurrence by the image's
/// position ([`BlockRoleAspect`]), and the parts of the image that are not
/// words of the text it is written in ([`ApartFromText`]). An aspect is a
/// piece of behavior attached to an element when the feature registers it
/// ([elements and aspects](crate::guide::concepts_overview#elements-and-aspects)).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsImage;

/// An image is a block where it is a direct child of a float's body (the
/// body of a node whose element is the `floats` feature's [`FLOAT`],
/// `flm.floats.float`, regardless of the float type), and inline content
/// anywhere else: in a sentence, a paragraph of its own, a heading's title, a
/// caption, a list item, a footnote, and the body of any other environment.
/// An image inside a paragraph of a float's body, or inside the float's
/// caption, is not a direct child of the body. Without the `floats` feature
/// no node is a float, and every image is inline. The block role is computed
/// from the tree at each call; nothing is stored.
impl BlockRoleAspect for GraphicsImage {
    fn block_role(&self, node: &NodeRef<'_, Flm>) -> BlockRole {
        let in_float_body = owning_callable(node).is_some_and(|owner| {
            element_of(&owner).name.as_str() == FLOAT
                && owner.body().is_some_and(|body| body.iter().any(|child| child.id() == node.id()))
        });
        if in_float_body { BlockRole::Block } else { BlockRole::DEFAULT }
    }
}

/// Neither the options nor the path of an image are words of the text it is
/// written in: returns the options argument, [`OPTIONS_ARG`], and the path
/// argument, [`PATH_ARG`].
impl ApartFromText for GraphicsImage {
    fn apart_from_text(&self, _node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        vec![
            ContentRef::ArgumentNamed(OPTIONS_ARG.to_string()),
            ContentRef::ArgumentNamed(PATH_ARG.to_string()),
        ]
    }
}

/// Condition `flm.graphics.path-not-characters`, an Error: the path of an
/// image holds content that is not characters, such as a command that a use
/// site of a user definition passes into the definition's path
/// (`\fig{\emph{x}}` for a definition whose template is
/// `\includegraphics{#1.png}`). Reported by the image's lowering rule, at the
/// command, wherever the image is lowered. Recovery: no image is shown.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.path-not-characters",
    message = "the path of an image must be plain text, without commands"
)]
pub struct PathNotCharacters;

/// Condition `flm.graphics.empty-path`, an Error: the path of an image is
/// empty, as in `\includegraphics{}`. Reported by the image's lowering rule,
/// at the command, wherever the image is lowered. Recovery: no image is
/// shown.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.graphics.empty-path", message = "the path of an image must not be empty")]
pub struct EmptyPath;

/// What the path argument of an image node holds, as the rules and the
/// stages read it ([`read_path`]).
pub(crate) enum ImagePath {
    /// Characters, at least one: the path as written.
    Read(String),
    /// No character.
    Empty,
    /// Content that is not characters.
    NotCharacters,
}

/// Reads the path argument of `node` through [`chars_at_lowering`], the only
/// reading of characters at lowering. An argument that is absent (after a
/// parse error) reads as the empty path.
///
/// # Errors
///
/// [`LowerError::PlaceholderCharacters`](crate::core::lowering::LowerError::PlaceholderCharacters)
/// for a path holding a placeholder of a template that is lowered as
/// written (the template of a kept definition).
pub(crate) fn read_path(
    node: &NodeRef<'_, Flm>,
) -> Result<ImagePath, crate::core::lowering::LowerError> {
    let Ok(Some(content)) = node.argument_content_nodes_named(PATH_ARG) else {
        return Ok(ImagePath::Empty);
    };
    Ok(match chars_at_lowering(content.iter())? {
        Ok(text) if text.is_empty() => ImagePath::Empty,
        Ok(text) => ImagePath::Read(text),
        Err(_) => ImagePath::NotCharacters,
    })
}

/// Returns the element value of [`IMAGE_ELEMENT`], shared by the command's
/// two declarations: [`GraphicsImage`] as its data, its block role given by
/// position, and its [`ApartFromText`] aspect.
fn image_spec() -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(IMAGE_ELEMENT)
            .docs("`\\includegraphics[options]{path}`: an image the document shows.")
            .with::<dyn BlockRoleAspect>(Arc::new(GraphicsImage))
            .with::<dyn ApartFromText>(Arc::new(GraphicsImage))
            .with_value(Arc::new(GraphicsImage)),
    )
}

/// Returns the declarations of the command's two arguments: the optional
/// argument [`OPTIONS_ARG`], key-value pairs, then the mandatory argument
/// [`PATH_ARG`], raw text up to the matching `}`.
pub fn arguments() -> Vec<ArgDef> {
    vec![
        ArgDef::named(OPTIONS_ARG, ArgStdKind::KeyVals)
            .docs("Key-values: `alt=<text>`, `width=<length>`, `height=<length>`."),
        ArgDef::named(PATH_ARG, ArgStdKind::Verbatim { delimiters: Some(('{', '}')) })
            .docs(
                "The image's path, raw text up to the matching `}`: `%`, `#`, `\\`, `~` and \
                 nested braces are characters of the path.",
            )
            .role(PATH_ARG_ROLE),
    ]
}

/// Returns one declaration of the command, in text mode, whose element value
/// is `spec`.
fn command_def(owner: &FeatureName, spec: &Arc<ElementSpec>) -> ElementDef {
    ElementDef::new(owner.clone(), [COMMAND], SyntaxDef::Macro { args: arguments() }, Arc::clone(spec))
        .docs("`\\includegraphics[options]{path}`: the image whose file is at `path`.")
}

/// Returns the scoped package [`INCLUDEGRAPHICS_PACKAGE`], declared by `owner`,
/// holding the command whose element value is `spec`.
fn includegraphics_package(owner: &FeatureName, spec: &Arc<ElementSpec>) -> ScopedPackageDef {
    ScopedPackageDef {
        owner: owner.clone(),
        name: Name::from(INCLUDEGRAPHICS_PACKAGE),
        elements: vec![command_def(owner, spec)],
        docs: String::from(
            "`\\includegraphics[options]{path}`, in the bodies that refer to this package (those \
             of a `figure` and a `table` by default).",
        ),
    }
}

/// The `graphics` feature: the image element, its scoped package and, under
/// [`GraphicsConfig::global`], its ordinary declaration; the three service
/// slots; the inspect stage and its table; one publish stage and table per
/// backend with an image rule; the lowering rules. See the [module
/// documentation](self).
#[derive(Debug, Default, Clone, Copy)]
pub struct Graphics {
    config: GraphicsConfig,
}

impl Graphics {
    /// Returns the feature with the configuration `config`, which is fixed for
    /// the life of the registry it is built into.
    pub fn new(config: GraphicsConfig) -> Graphics {
        Graphics { config }
    }

    /// Returns the configuration this feature was constructed with.
    pub fn config(&self) -> &GraphicsConfig {
        &self.config
    }
}

/// The backends the feature publishes images for and lowers them on: the
/// plain-text, HTML and `latex-rendered` backends through [`image_rule`], and
/// the `latex-recomposed` backend through its own rule.
const BACKENDS: [&str; 4] = [
    text::BACKEND,
    html::BACKEND,
    crate::stdcore::backends::latex_recomposed::BACKEND,
    crate::stdcore::backends::latex_rendered::BACKEND,
];

impl FlmFeature for Graphics {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Graphics").docs(
            "Images, written `\\includegraphics[options]{path}` inside the body of a float type \
             that makes the command available (the `figure` and `table` types do) or, under \
             `global`, wherever text can be written: inline content, except as a direct child of \
             a float's body, where an image is a block. The file that a host's graphics resolver \
             finds and what its inspector reports about it are recorded; each output publishes \
             the image by reference, as a copy beside the output or written into a `data:` \
             address, converted where a conversion rule says so. On the plain-text, HTML and \
             rendered LaTeX outputs the image lowers to `flm.pres.image` with the published address, on the \
             recomposed LaTeX to the LaTeX hook `\\flmIncludeGraphics`.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner: FeatureName = b.feature().clone();
        b.argument_role(PATH_ARG_ROLE, "The path of an image file.")?;
        let spec = image_spec();
        b.scoped_package(includegraphics_package(&owner, &spec))?;
        if self.config.global {
            // The same element, as an ordinary command: which declaration
            // answers inside a body naming the package is immaterial.
            // See decision rationale at [§flm-dd-dr:std-graphics-global].
            b.element(command_def(&owner, &spec))?;
        }
        // Where the floats feature is absent, nobody declares the slot and
        // the contribution is dropped with a Note.
        b.peer_slot::<dyn FloatContentHandler>(CONTENT_SLOT, Arc::new(IncludeGraphicsContent))?;
        b.service_slot::<dyn GraphicsResolver>(
            RESOLVER_SLOT,
            Arity::Single,
            "the graphics resolver a host supplies, which finds the file for an image's path (its \
             identity, its format, the address that reaches it) and reads its content when asked",
        )?;
        b.service_slot::<dyn GraphicsInspector>(
            INSPECTOR_SLOT,
            Arity::Single,
            "the inspector a host supplies, which reports what an image file states: whether it \
             is a vector or a raster image, its size in pixels, its resolution and its physical \
             size",
        )?;
        b.service_slot::<dyn GraphicsConverter>(
            CONVERTERS_SLOT,
            Arity::OrderedMany,
            "the converters that turn an image of one format into another, asked in the order \
             contributed for each conversion rule an output applies",
        )?;
        inspect::register(b)?;
        for backend in BACKENDS {
            publish::register(b, backend)?;
        }
        for backend in [text::BACKEND, html::BACKEND, crate::stdcore::backends::latex_rendered::BACKEND] {
            b.peer_backend(backend, "a backend on which the image rule is the default")?;
            b.lowering_rule(image_rule(backend))?;
        }
        latex::register(b)?;
        b.digest_input(&self.config.digest_bytes())?;
        Ok(())
    }
}

/// Returns the [`Graphics`] feature with the default configuration, ready to
/// be listed in a package entry.
///
/// Use `Arc::new(Graphics::new(config))` to list the feature with a
/// [`GraphicsConfig`] of your own.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Graphics::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_takes_key_values_then_a_braced_verbatim_path() {
        let args = arguments();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].name.as_deref(), Some(OPTIONS_ARG));
        assert_eq!(args[0].kind.identity(), ArgDef::new(ArgStdKind::KeyVals).kind.identity());
        assert_eq!(args[1].name.as_deref(), Some(PATH_ARG));
        let braced = ArgDef::new(ArgStdKind::Verbatim { delimiters: Some(('{', '}')) });
        assert_eq!(args[1].kind.identity(), braced.kind.identity());
    }

    #[test]
    fn the_configuration_digests_its_one_switch() {
        assert_eq!(GraphicsConfig::new().digest_bytes(), [0]);
        assert_eq!(GraphicsConfig::new().global(true).digest_bytes(), [1]);
    }
}
