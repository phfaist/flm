//! This module defines the `graphics` feature ([`Graphics`]), which provides
//! the command `\includegraphics[options]{path}`. The command inserts an
//! *image*, that is, an external picture file, into the document. For
//! instance, `\includegraphics[alt=A plot, width=5cm]{figures/plot.png}`
//! inserts the picture of the file `figures/plot.png`, 5cm wide, with the
//! alternative text `A plot`. A path can also be an address, such as
//! `https://example.org/a.svg`.
//!
//! The library never opens a file itself. The program that embeds FLM can
//! contribute *services* that find, measure and convert image files, as
//! described below; the `flm` program contributes them with its feature
//! `graphics-files`. Without these services, every output refers to an image
//! by its path as written.
//!
//! The language guide describes the command for authors
//! ([Graphics](crate::guide::language_syntax#graphics)). The rendering guide
//! shows what each output writes ([images and how an output reaches
//! them](crate::guide::rendering#images-and-how-an-output-reaches-them)).
//!
//! # Where the command can be written
//!
//! `\includegraphics` is declared in the [scoped
//! package](crate::guide::concepts_overview#scoped-packages)
//! [`INCLUDEGRAPHICS_PACKAGE`] (`flm.graphics.includegraphics`). The command
//! is therefore available only in the bodies and arguments whose declarations
//! refer to this package. Elsewhere, it is an unknown command. With the
//! `floats` feature, the float types `figure` and `table` refer to this
//! package, so that an image can be written as the content of a figure:
//!
//! ```text
//! \begin{figure}
//!   \includegraphics{figures/plot.png}
//!   \caption{A plot of the data.}
//! \end{figure}
//! ```
//!
//! With the configuration [`GraphicsConfig::global`], `\includegraphics` is
//! also declared as an ordinary command, which can be written wherever text
//! can be written.
//!
//! An image is a block when it is a direct child of the body of a float, as in
//! the figure above. Everywhere else, an image is inline content, like a word
//! of the text around it ([`GraphicsImage`]).
//!
//! # The options and the path
//!
//! The optional argument contains key-value pairs: the alternative text `alt`,
//! and the size `width` and `height`. The module [`options`](mod@options)
//! reads the options and defines the diagnostics that are reported for them.
//! An invalid option is reported as an error and ignored, and the image is
//! written as usual.
//!
//! The mandatory argument contains the path of the image file as raw text, up
//! to the matching `}`. The characters `%`, `#`, `~`, `\` and nested braces are
//! part of the path. The path is read when the image is lowered. Therefore, a
//! placeholder in the path of a user definition's template is replaced by the
//! characters of the corresponding argument of the call. For instance, if the
//! template of `\fig` is `\includegraphics{figs/#1.png}`, the call `\fig{plot}`
//! inserts the image `figs/plot.png`. A path that is not plain text is
//! reported as the error [`PathNotCharacters`], and an empty path as the error
//! [`EmptyPath`]. In both cases, no image is written.
//!
//! # The size of an image
//!
//! The size at which an image is shown is computed when the image is lowered
//! ([`shown_size`]):
//!
//! - When both `width` and `height` are given, the image has this size.
//! - When only one of them is given, the other dimension is computed from the
//!   size of the image file, so that the image keeps its proportions. If the
//!   size of the file is not known, only the given dimension is set.
//! - When neither is given, the image has the size of the image file.
//!
//! The size of the image file is known when an inspector, one of the services
//! described below, has reported it. The size is then multiplied by the
//! *magnification* of the output: one factor for raster images and another
//! for vector images ([`GraphicsRenderOptions::magnification`]). Both factors
//! are 1 by default. When no size is known, the warning [`UnknownSize`] is
//! reported, and the image is written without a size.
//!
//! # Services for image files
//!
//! A *service* is a value that a feature, or the program that embeds FLM,
//! contributes to a *service slot* declared in another feature
//! ([`core::services`](crate::core::services)). The `graphics` feature
//! declares three service slots, for the work that needs access to files or to
//! installed programs. None of these services is required.
//!
//! - The *graphics resolver* ([`GraphicsResolver`], slot [`RESOLVER_SLOT`], at
//!   most one) finds the file that a path refers to, and reads its content
//!   ([`resolve`](mod@resolve)).
//! - The *inspector* ([`GraphicsInspector`], slot [`INSPECTOR_SLOT`], at most
//!   one) reports the type and the size of an image file
//!   ([`inspect`](mod@inspect)).
//! - The *converters* ([`GraphicsConverter`], slot [`CONVERTERS_SLOT`], any
//!   number) convert image files from one format into another, for instance
//!   from PDF to PNG ([`convert`](mod@convert)).
//!
//! # Stages and lowering rules
//!
//! The feature registers two kinds of stages, which call the services:
//!
//! - The inspect stage [`INSPECT_STAGE`] (`flm.graphics.inspect`) calls the
//!   graphics resolver and the inspector for each image. It writes the results
//!   into the table [`INFO`] (`flm.graphics.info`), keyed by the address of the
//!   image's node.
//! - The publish stage `flm.graphics.publish.<backend>` ([`PublishStage`]) is
//!   registered for each of the backends `text`, `html`, `latex-recomposed`
//!   and `latex-rendered`. It computes the address at which the output of the
//!   backend refers to each image, and writes this address into the table
//!   `flm.graphics.published.<backend>` ([`published_table`]). The address
//!   depends on the *strategy* that the render options set for the output: the
//!   address of the file where it is (`reference`, the default), the address
//!   of a copy of the file that is made part of the output (`copy`), or a
//!   `data:` address that contains the content of the file (`data-uri`). Under
//!   the strategy `content`, the stage also writes the content of each image
//!   file into the table `flm.graphics.content.<backend>` ([`content_table`]),
//!   for an output that renders images itself. See the modules
//!   [`strategy`](mod@strategy) and [`publish`](mod@publish).
//!
//! On the progress sheet of a build ([`crate::progress`]), the inspect stage
//! declares no row of its own: the run counts its trees. A call of a publish
//! stage that converts images declares a row of the kind [`IMAGES_KIND`]
//! (`flm.graphics.images`), counted in the images it converts, and under it a
//! row of the kind [`CONVERT_KIND`] (`flm.graphics.convert`, "Converting
//! graphics") for each conversion, whose scope the converter receives.
//!
//! The lowering rules of the image element read these tables:
//!
//! - On the backends `text`, `html` and `latex-rendered`, the rule
//!   [`image_rule`] lowers an image to a
//!   [`flm.pres.image`](crate::stdlib::backends::pres::IMAGE) node. For
//!   instance, the plain-text output prints this node as
//!   `[image: <alternative text or address>]`, and the HTML output writes it
//!   as an `<img>` element.
//! - On the backend `latex-recomposed`, the rule
//!   [`LATEX_RULE`](latex::LATEX_RULE) writes the LaTeX hook
//!   `\flmIncludeGraphics[<keys>]{<path>}` ([`latex`](mod@latex)).
//!
//! The stages and the rules of each backend are registered with a peer
//! declaration of the backend ([`PeerName`](crate::core::registry::PeerName)).
//! A registry without one of these backends therefore still builds.
//!
//! # Settings
//!
//! The configuration [`GraphicsConfig`] is the value from which the feature is
//! constructed, and is fixed when the registry is built. The render options
//! [`GraphicsRenderOptions`] are set for each render request. For each output,
//! they contain the strategy (`strategy`), the conversion rules (`convert`,
//! [`ConversionRule`]) and the magnification (`magnification`,
//! [`Magnification`]). See [settings, options and
//! configuration](crate::guide::concepts_overview#settings-options-and-configuration).
//!
//! # Images in floats
//!
//! The feature contributes the content handler `includegraphics` to the
//! `floats` feature ([`handler`](mod@handler)). A float type that lists this
//! handler, as `figure` and `table` do by default, accepts a body that
//! contains exactly one image besides its caption and labels.
//!
//! In a registry without the `floats` feature, this contribution is dropped
//! with a note. To use images without floats, set [`GraphicsConfig::global`].
//!
//! [`shown_size`]: crate::stdlib::features::graphics::rules::shown_size

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
use crate::stdlib::backends::lowering::rules::owning_callable;
use crate::stdlib::backends::{html, text};
use crate::stdlib::features::floats::{CONTENT_SLOT, FLOAT, FloatContentHandler};

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
    CONVERT_KIND, GraphicsContent, GraphicsPublished, IMAGES_KIND, ImageContent, OutputImages,
    PublishStage, PublishedImage, content_table, output_images, published_table,
};
pub use resolve::{
    GraphicsResolveError, GraphicsResolveRequest, GraphicsResolver, ImageFile, ImageFormat,
    RESOLVER_SLOT, ResolvedImage, names_scheme,
};
pub use strategy::{Factor, FactorError, GraphicsRenderOptions, Magnification, Strategy};
pub use rules::{ImageRule, UnknownSize, image_rule};

/// The name under which the [`Graphics`] feature is registered: `graphics`.
pub const FEATURE: &str = "graphics";

/// Returns the declaration of the settings of the `graphics` feature. The fields of the
/// configuration [`GraphicsConfig`] and of the render options [`GraphicsRenderOptions`] are the
/// settings keys.
///
/// The values of the keys `strategy`, `convert` and `magnification` are maps from the name of an
/// output to the value for this output. These keys are merged entry by entry
/// ([`SettingsDecl::merge_entries`]), and have no default entries. Therefore, when a host
/// combines several sources of settings, a source that sets the entry of one output leaves the
/// entries of the other outputs unchanged.
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<GraphicsConfig>(
            "Whether `\\includegraphics` can be written wherever text can be written (`global`). \
            When `global` is false, which is the default, the command is available only in the \
            bodies of the float types that allow it, such as `figure` and `table`. This \
            configuration is fixed when the registry is built; it cannot vary per document or \
            per render.",
        )
        .options::<GraphicsRenderOptions>(
            Bind::Render,
            "The image settings of each output, set per render request. Each key maps the name \
            of an output to the value for this output. `strategy` selects how the output refers \
            to its images: `reference` (the default) refers to the file where it is, `copy` \
            makes a copy of the file part of the output, `data-uri` writes the content of the \
            file into a `data:` address, and `content` passes the content of the file to an \
            output that renders images itself. `convert` lists the conversion rules that the \
            output applies to image files under the strategies `copy`, `data-uri` and \
            `content`. `magnification` contains the factors by which the output multiplies the \
            size of raster images and of vector images (`raster` and `vector`, 1 by default).",
        )
        .merge_entries("strategy", || Ok(serde_json::Value::Object(serde_json::Map::new())))
        .remove_false("strategy")
        .merge_entries("convert", || Ok(serde_json::Value::Object(serde_json::Map::new())))
        .remove_false("convert")
        .merge_entries("magnification", || Ok(serde_json::Value::Object(serde_json::Map::new())))
        .remove_false("magnification")
}

/// The name of the element of `\includegraphics`: `flm.graphics.image`. The
/// data of its element value is [`GraphicsImage`].
pub const IMAGE_ELEMENT: &str = "flm.graphics.image";

/// The name of the scoped package that contains the declaration of
/// `\includegraphics`: `flm.graphics.includegraphics`.
///
/// `\includegraphics` is available in the bodies and arguments whose
/// declarations refer to this package. For instance, a float type refers to
/// the package by listing it in
/// [`FloatType::scoped_packages`](crate::stdlib::features::floats::FloatType::scoped_packages),
/// as the types `figure` and `table` do. The package is declared under every
/// configuration of the feature, also when [`GraphicsConfig::global`] makes
/// `\includegraphics` an ordinary command.
pub const INCLUDEGRAPHICS_PACKAGE: &str = "flm.graphics.includegraphics";

/// The name of the command, without the backslash, `includegraphics`.
pub const COMMAND: &str = "includegraphics";

/// The name of the optional argument of `\includegraphics`: `options`. The
/// argument contains the options of the image as key-value pairs (see the
/// module [`options`](mod@options)).
pub const OPTIONS_ARG: &str = "options";

/// The name of the mandatory argument of `\includegraphics`: `path`. The
/// argument contains the path of the image file as raw text, up to the
/// matching `}`.
pub const PATH_ARG: &str = "path";

/// The name of the argument role `flm.graphics.path`, which marks an argument
/// that contains the path of an image file. The path argument [`PATH_ARG`] of
/// `\includegraphics` has this role
/// ([`ArgDef::role`](crate::lang::mint::ArgDef::role)).
pub const PATH_ARG_ROLE: &str = "flm.graphics.path";

/// The name of the service slot of the inspector: `flm.graphics.inspector`.
/// A feature or the program that embeds FLM can contribute at most one
/// [`GraphicsInspector`] to this slot ([`Arity::Single`]). The `graphics`
/// feature works without an inspector. See the module [`inspect`](mod@inspect).
pub const INSPECTOR_SLOT: &str = "flm.graphics.inspector";

pub use inspect::{INFO, INSPECT_STAGE};

/// The configuration of the `graphics` feature, which selects where
/// `\includegraphics` can be written.
///
/// By default (`global` is `false`), `\includegraphics` is available only in
/// the bodies whose declarations refer to the scoped package
/// [`flm.graphics.includegraphics`](INCLUDEGRAPHICS_PACKAGE), for instance the
/// body of a `figure` when the `floats` feature is registered. When `global`
/// is `true`, `\includegraphics` is also an ordinary command, which can be
/// written wherever text can be written.
///
/// The configuration is the value from which the feature is constructed. It
/// is fixed when the registry is built, and cannot vary per document or per
/// render ([settings, options and
/// configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
/// The configuration changes which commands exist in the language, so
/// [`digest_bytes`](GraphicsConfig::digest_bytes) is an input of the registry
/// digest. To configure the feature among the standard features, use
/// [`StdlibConfig::graphics`](crate::stdlib::StdlibConfig::graphics).
///
/// # Examples
///
/// ```
/// use flm_core::stdlib::features::graphics::GraphicsConfig;
///
/// assert!(!GraphicsConfig::new().global);
/// assert!(GraphicsConfig::new().global(true).global);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct GraphicsConfig {
    /// Whether `\includegraphics` can be written wherever text can be written.
    /// When `false` (the default), the command is available only in the
    /// bodies that refer to its scoped package
    /// [`flm.graphics.includegraphics`](INCLUDEGRAPHICS_PACKAGE), such as the
    /// bodies of the float types `figure` and `table`.
    pub global: bool,
}

impl GraphicsConfig {
    /// Returns the default configuration, in which `global` is `false`.
    pub fn new() -> GraphicsConfig {
        GraphicsConfig::default()
    }

    /// Sets [`global`](GraphicsConfig::global) and returns the configuration.
    pub fn global(mut self, global: bool) -> GraphicsConfig {
        self.global = global;
        self
    }

    /// Returns the bytes that represent this configuration in the registry
    /// digest: one byte, `1` when `global` is `true` and `0` otherwise.
    pub fn digest_bytes(&self) -> Vec<u8> {
        // Taken apart by name, so a field added and not written here fails
        // to compile.
        let GraphicsConfig { global } = self;
        vec![u8::from(*global)]
    }
}

/// The data of the element value of `\includegraphics` ([`IMAGE_ELEMENT`]).
///
/// This type implements the two aspects of the element. An aspect is a piece
/// of behavior attached to an element ([elements and
/// aspects](crate::guide::concepts_overview#elements-and-aspects)).
///
/// - [`BlockRoleAspect`]: an image is a block when it is a direct child of
///   the body of a float, and inline content everywhere else.
/// - [`ApartFromText`]: the options and the path of an image are not part of
///   the text in which the image is written.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsImage;

/// An image is a block when it is a direct child of the body of a float, that
/// is, of a node of the element [`FLOAT`] (`flm.floats.float`) of the `floats`
/// feature, whatever its float type. Everywhere else, an image is inline
/// content, for instance in a sentence, in the title of a heading, in a
/// caption, or in a paragraph of the body of a float. Without the `floats`
/// feature, every image is inline content.
impl BlockRoleAspect for GraphicsImage {
    fn block_role(&self, node: &NodeRef<'_, Flm>) -> BlockRole {
        let in_float_body = owning_callable(node).is_some_and(|owner| {
            element_of(&owner).name.as_str() == FLOAT
                && owner.body().is_some_and(|body| body.iter().any(|child| child.id() == node.id()))
        });
        if in_float_body { BlockRole::Block } else { BlockRole::DEFAULT }
    }
}

/// Returns the options argument [`OPTIONS_ARG`] and the path argument
/// [`PATH_ARG`]: neither is part of the text in which the image is written.
/// For instance, the identifier of a heading whose title contains an image is
/// computed from the other words of the title. The alternative text is part
/// of the options argument, and is therefore left out as well.
impl ApartFromText for GraphicsImage {
    fn apart_from_text(&self, _node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        vec![
            ContentRef::ArgumentNamed(OPTIONS_ARG.to_string()),
            ContentRef::ArgumentNamed(PATH_ARG.to_string()),
        ]
    }
}

/// The error that is reported when the path of `\includegraphics` is not
/// plain text (`flm.graphics.path-not-characters`).
///
/// The path that an author writes in `\includegraphics{path}` is always plain
/// text, because the argument is read as raw text. Other content gets into
/// the path through a user definition whose template places one of its
/// arguments in the path. A call of such a definition must pass plain text in
/// this argument, that is, characters only. For instance, if the template of
/// `\fig` is `\includegraphics{figs/#1.png}`, the call can be `\fig{plot}`.
///
/// Reported by the lowering rule of `\includegraphics` when it is invoked.
/// The `\includegraphics` command then expands to nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.path-not-characters",
    message = "Invalid path in ‘\\includegraphics’: the path must be plain text"
)]
pub struct PathNotCharacters;

/// The error that is reported when the path of `\includegraphics` is empty
/// (`flm.graphics.empty-path`).
///
/// The path must contain at least one character, as in
/// `\includegraphics{figures/plot.png}`.
///
/// Reported by the lowering rule of `\includegraphics` when it is invoked.
/// The `\includegraphics` command then expands to nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.graphics.empty-path", message = "Missing path in ‘\\includegraphics’")]
pub struct EmptyPath;

/// The path of an image, as returned by [`read_path`].
pub(crate) enum ImagePath {
    /// The path as written, which contains at least one character.
    Read(String),
    /// The path is empty.
    Empty,
    /// The path contains content other than characters, such as a command.
    NotCharacters,
}

/// Reads the path of the image `node` with [`chars_at_lowering`]. A path
/// argument that is missing after a parse error is read as an empty path.
///
/// # Errors
///
/// Returns
/// [`LowerError::PlaceholderCharacters`](crate::core::lowering::LowerError::PlaceholderCharacters)
/// when the path contains a placeholder of a user definition's template. This
/// happens only when a template is lowered without being expanded, such as
/// the template of a user definition that the recomposed LaTeX keeps as a
/// LaTeX definition.
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

/// Returns the element value of [`IMAGE_ELEMENT`]. Its data is
/// [`GraphicsImage`], which also implements the two aspects of the element,
/// [`BlockRoleAspect`] and [`ApartFromText`]. The declaration of
/// `\includegraphics` in the scoped package and the ordinary declaration under
/// [`GraphicsConfig::global`] share this element value.
fn image_spec() -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(IMAGE_ELEMENT)
            .docs("An image, inserted with `\\includegraphics[options]{path}`.")
            .with::<dyn BlockRoleAspect>(Arc::new(GraphicsImage))
            .with::<dyn ApartFromText>(Arc::new(GraphicsImage))
            .with_value(Arc::new(GraphicsImage)),
    )
}

/// Returns the declarations of the two arguments of `\includegraphics`, in
/// order: the optional argument [`OPTIONS_ARG`], which contains key-value
/// pairs, and the mandatory argument [`PATH_ARG`], which contains the path as
/// raw text up to the matching `}`.
pub fn arguments() -> Vec<ArgDef> {
    vec![
        ArgDef::named(OPTIONS_ARG, ArgStdKind::KeyVals).docs(
            "The options of the image, as key-value pairs, as in `alt=A plot, width=5cm`. The \
             keys are `alt` for the alternative text, and `width` and `height` for the size.",
        ),
        ArgDef::named(PATH_ARG, ArgStdKind::Verbatim { delimiters: Some(('{', '}')) })
            .docs(
                "The path of the image file, as in `figures/plot.png`. The path is raw text up to \
                 the matching `}`: the characters `%`, `#`, `\\`, `~` and nested braces are part \
                 of the path.",
            )
            .role(PATH_ARG_ROLE),
    ]
}

/// Returns a declaration of `\includegraphics` in text mode, owned by the
/// feature `owner`, with the element value `spec`.
fn command_def(owner: &FeatureName, spec: &Arc<ElementSpec>) -> ElementDef {
    ElementDef::new(owner.clone(), [COMMAND], SyntaxDef::Macro { args: arguments() }, Arc::clone(spec))
        .docs("Inserts the image whose file is at `path`.")
}

/// Returns the scoped package [`INCLUDEGRAPHICS_PACKAGE`], owned by the
/// feature `owner`. The package contains a declaration of `\includegraphics`
/// with the element value `spec`.
fn includegraphics_package(owner: &FeatureName, spec: &Arc<ElementSpec>) -> ScopedPackageDef {
    ScopedPackageDef {
        owner: owner.clone(),
        name: Name::from(INCLUDEGRAPHICS_PACKAGE),
        elements: vec![command_def(owner, spec)],
        docs: String::from(
            "Provides `\\includegraphics[options]{path}` in the bodies that refer to this \
             package, such as the bodies of the float types `figure` and `table`.",
        ),
    }
}

/// The `graphics` feature, which provides the command `\includegraphics` for
/// images. See the [module documentation](self).
///
/// The feature registers the element of `\includegraphics` with its scoped
/// package [`INCLUDEGRAPHICS_PACKAGE`], the three service slots for image
/// files, the inspect stage, one publish stage for each backend, and the
/// lowering rules of images. Construct the feature with [`Graphics::new`], or
/// use [`feature`] for the default configuration.
#[derive(Debug, Default, Clone, Copy)]
pub struct Graphics {
    config: GraphicsConfig,
}

impl Graphics {
    /// Returns the feature constructed from the configuration `config`. The
    /// configuration stays fixed in the registry that the feature is built
    /// into.
    pub fn new(config: GraphicsConfig) -> Graphics {
        Graphics { config }
    }

    /// Returns the configuration this feature was constructed with.
    pub fn config(&self) -> &GraphicsConfig {
        &self.config
    }
}

/// The backends for which the feature registers a publish stage: `text`,
/// `html`, `latex-recomposed` and `latex-rendered`. Each of these backends
/// also has a lowering rule for images: [`image_rule`] on `text`, `html` and
/// `latex-rendered`, and [`latex::LATEX_RULE`] on `latex-recomposed`.
const BACKENDS: [&str; 4] = [
    text::BACKEND,
    html::BACKEND,
    crate::stdlib::backends::latex_recomposed::BACKEND,
    crate::stdlib::backends::latex_rendered::BACKEND,
];

impl FlmFeature for Graphics {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Graphics").docs(
            "Images, inserted with `\\includegraphics[options]{path}`. By default, the command \
             is available in the bodies of the float types that allow it, such as `figure` and \
             `table`. With the configuration `global`, it can be written wherever text can be \
             written. The program can provide services that find, inspect and convert image \
             files. The render options select how each output refers to its images, for \
             instance by the path of the file or with a copy of the file. On the plain-text, \
             HTML and rendered LaTeX outputs, an image is lowered to `flm.pres.image`; on the \
             recomposed LaTeX, it is written as the LaTeX hook `\\flmIncludeGraphics`.",
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
        b.element_type_text(
            IMAGE_ELEMENT,
            "An image, given by the path of an image file, with an optional alternative text and \
             an optional size.",
        )?;
        if self.config.global {
            // The same element, as an ordinary command: which declaration
            // answers inside a body naming the package is immaterial.
            b.element(command_def(&owner, &spec))?;
        }
        // Where the floats feature is absent, nobody declares the slot and
        // the contribution is dropped with a Note.
        b.peer_slot::<dyn FloatContentHandler>(CONTENT_SLOT, Arc::new(IncludeGraphicsContent))?;
        b.service_slot::<dyn GraphicsResolver>(
            RESOLVER_SLOT,
            Arity::Single,
            "the graphics resolver, which finds the file that the path of an image refers to and \
             reads the content of this file",
        )?;
        b.service_slot::<dyn GraphicsInspector>(
            INSPECTOR_SLOT,
            Arity::Single,
            "the inspector, which reports the type of an image file (vector or raster image), its \
             size in pixels, its resolution and its physical size",
        )?;
        b.service_slot::<dyn GraphicsConverter>(
            CONVERTERS_SLOT,
            Arity::OrderedMany,
            "the converters, which convert an image file from one format into another; for a \
             conversion rule, the first converter in the order of contribution that can perform \
             the rule is used",
        )?;
        inspect::register(b)?;
        for backend in BACKENDS {
            publish::register(b, backend)?;
        }
        for backend in [text::BACKEND, html::BACKEND, crate::stdlib::backends::latex_rendered::BACKEND] {
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
