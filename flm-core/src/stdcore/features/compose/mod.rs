//! The feature `compose`: the stages that assemble each output's entry artifact from the
//! output's regions through a **compose template**, a text with named places for those
//! regions, and the service slots of the template engines and of the compose templates.
//!
//! An output region is one named part of an output's entry artifact (the document's
//! content, each piece placed beside it, and on HTML the stylesheets, the scripts, the
//! title and the language), which the backend writes once into the table
//! `flm.output.<backend>.regions` ([`Regions`]). The entry artifact is the artifact that
//! represents the output as a whole: the HTML page, the printed text, the recomposed LaTeX
//! document ([`ArtifactSet`]). The backend's final stage assembles the entry artifact
//! from those regions in a layout of the backend's own ([`crate::core::region`]). The
//! feature `compose` is a second way to assemble them: through the compose template that
//! the settings choose, so that a program chooses what surrounds the document's content.
//!
//! # A host registers the feature
//!
//! The feature is not part of the package [`stdcore`](crate::stdcore::package). The
//! composer of an output provides the same capability as the backend's final stage, and
//! a plan chooses between two stages that provide one capability only by a stage the
//! plan request prefers or by a capability marked as the default; with neither, the plan
//! fails ([`PlanError::Contested`](crate::core::plan::PlanError::Contested)). So a host
//! that registers the feature also prefers the composer of each output it renders
//! ([`stage_name`]; [`Overrides::prefer`](crate::core::plan::Overrides::prefer)). A
//! registry without the feature has one stage per output that assembles the entry
//! artifact, the backend's final stage. Where a host registers the feature but the
//! settings switch it off, the host leaves it out of the registry, and the backend's
//! final stage assembles each output.
//!
//! # Template engines and compose templates
//!
//! The feature declares two service slots, to which any feature of any package may
//! contribute. A service slot is the declaration, by the feature that owns an
//! interaction, of a dotted name, the type of the values the slot takes and its arity
//! ([`crate::core::services`]). A feature of another package contributes to them as a
//! peer ([`FeatureBuilder::peer_slot`]), so that its registry still builds where
//! `compose` is switched off.
//!
//! - [`ENGINES_SLOT`] (`flm.compose.engines`) takes the **template engines**, each a
//!   [`TemplateEngine`] with a name. `compose` contributes `content-only`, which places
//!   the content and the pieces with nothing around them ([`engines`]); a host
//!   contributes the engines of the template languages it reads.
//! - [`TEMPLATES_SLOT`] (`flm.compose.templates`) takes the **compose templates**, each a
//!   [`Template`]: its name, the output it serves, its engine, its text, the pieces it
//!   accepts and those it requires, and its own settings with their defaults.
//!   [`Template::from_data`] makes one from a description given as data, whose key
//!   `main` gives the path of the file that holds the compose template's text, relative
//!   to the folder of the description; without `main`, the text is empty. `compose`
//!   contributes none.
//!
//! # The stage of each output
//!
//! The feature registers one stage per output made of output regions, that is, per
//! output for which a backend contributes a regions output ([`RegionsOutput`]) to the
//! service slot `flm.output.regions` ([`REGIONS_SLOT`]; [`served_output`]). A regions
//! output is a backend's statement that its output is made of output regions: which table
//! holds them and which capability its final stage provides. The stages are registered in
//! the feature's second round of registration, a [`Finalizer`] that runs once every
//! feature has registered, so every backend has contributed to that service slot by then.
//!
//! The stage of an output, `flm.compose.<output>` ([`stage_name`]), is called the
//! *composer* in this module. It writes the output's artifact set into the same table as
//! the backend's final stage and provides the same capability, so a plan can schedule
//! either one; the host prefers the composer (see [above](self#a-host-registers-the-feature)).
//!
//! The composer reads the output's regions and the files that the output's stages write
//! beside the entry artifact (the style file of the recomposed LaTeX, the copies of
//! images), applies the compose template that the render option [`ComposeOptions`]
//! chooses for the output ([`chosen_template`]), and writes the entry artifact and those
//! files as the output's artifact set ([`ArtifactSet::insert_entry_with`]). It reports a
//! problem as the Error [`TemplateFailed`]; the output is then not made.
//!
//! # What a compose template reads
//!
//! | Name | What it is |
//! |---|---|
//! | `regions` | every output region, by name, as text: `content`; each piece's name; on HTML `styles`, `scripts`, `title` (the words of the page's title, escaped for HTML: the words of the document's title piece, else the title that the render option `html.title` gives) and `lang` (the language tag, present only when the render option `html.lang` gives one); on the recomposed LaTeX `style-package`, when the output has a style file |
//! | `pieces` | the regions that are pieces, by name, in the order of the output's regions; each has `text`, `words` (its displayed characters as plain text) and `place` (`before-content` or `after-content`) |
//! | `meta` | the document's metadata ([`DocMeta`]): `title`, `author` (a list of texts), `date`, and every other entry the program gives, each value as it is given. The title, the authors and the date are the texts the author wrote, FLM source and not plain text (`A \emph{fine} paper`); a compose template that shows them uses the pieces instead: `pieces["flm.meta.title"].text` for the HTML, or `.words` for plain words |
//! | `config` | the compose template's own settings: its defaults, with the values that the render option `template_config` gives for it on the output written over them key by key |
//! | `artifacts` | the path of each file the compose template ships, by the file's name ([below](self#files-a-compose-template-ships)): `artifacts.style` gives `paper-style.css`; for a folder it ships, the folder's path, with no trailing slash: `artifacts.icons` gives `paper-icons`, and a page writes `{{ artifacts.icons }}/check.svg` |
//!
//! A compose template that refers to a value that nothing provides cannot be applied: the
//! value is not taken as an empty text.
//!
//! # Files a compose template ships
//!
//! A compose template may ship files of its own, written beside the entry artifact: a
//! stylesheet, a script, a logo ([`Template::artifacts`], [`TemplateArtifact`]; the key
//! `artifacts` of its description, [`Template::from_data`]). Each has a name, and its
//! identity is `compose:<name>`. The composer asks the program's naming of files (the
//! service slot `flm.artifacts.namer`, [`ArtifactNamer`]) for the path of each, and gives
//! the paths to the compose template as the value `artifacts`. A path is relative to the
//! folder where the program writes the output's files, which is where the entry file lies
//! under a host's usual naming. A file marked to be applied (the field `apply` of
//! [`TemplateArtifact::File`]) is applied as a template of the same template engine
//! ([`TemplateEngine::render_text`]), with the context of the entry artifact, its value
//! `html` set to whether the file's media type is `text/html`; the result is the file's
//! content. The composer then adds each file to the output's artifact set, with its media
//! type and extension, and the entry artifact links to each ([`Artifact::refs`]). A
//! compose template that ships files cannot be applied where no package in use provides a
//! naming of files. The composer takes the list from the template engine
//! ([`TemplateEngine::artifacts`]), once per application and before naming the files.
//! By default it is the compose template's own list; an engine whose compose templates
//! choose their files when they are applied gives them there.
//!
//! A compose template may also ship a whole folder as one file of this list (the key
//! `dir` of its description in place of `file`; [`TemplateArtifact::Folder`]): every
//! file under the folder, at any depth, by its path relative to the folder. The composer
//! asks the naming of files for the folder's path (a path with no extension,
//! [`ArtifactRef::folder`]), gives that path to the compose template, with no trailing
//! slash, and adds a folder artifact ([`Artifact::folder`]) to the output's artifact set;
//! the program writes each of its files under that folder with its own path. A folder is
//! never applied as a template, and its files' media types come from their extensions.
//!
//! # Files a compose template includes
//!
//! A compose template's text, and the text of a file it ships and applies, may include
//! other files of the compose template's package, as its template engine's language
//! provides: a file applied as a template of the same engine, with the same context, or
//! a file whose text is written as it is. The files are read when the compose template
//! is applied, through the handle [`Template::files`] ([`TemplateFiles`]), which the
//! package that makes the compose template attaches to it, so that the name of an
//! included file may be computed and need not be listed ahead. The path of an included
//! file is relative to the folder of the compose template's description, and the handle
//! refuses a path that leaves the package's files. This crate reads no file of its own:
//! the handle reads, and the program that provides it records each read. A compose
//! template with no handle includes nothing.
//!
//! # On HTML: one element per region, and the composer's table
//!
//! On HTML, the composer writes the content and each piece inside a `<div>` element whose
//! identifier is derived from the region's name ([`region_id`]: `flm-region-content`,
//! `flm-region-flm.meta.title`), before the compose template places it, whichever
//! compose template is used. The composer also writes the table [`HTML_REGIONS`]
//! (`flm.compose.html.regions`, a [`ComposedRegions`]), which holds:
//!
//! - the HTML that the page holds under the identifier of the content and of each piece;
//! - the words of the page's title (the region `title`);
//! - the rest of the page, everything outside those regions (the field `skeleton`): the
//!   page with exactly what the table holds left out, and everything else kept, namely the
//!   stylesheets, the scripts, the language, the compose template's own markup, and the
//!   words of each piece wherever the compose template writes them.
//!
//! The table holds what a page can bring up to date by itself: an element found by its
//! identifier, and the page's title, which has its place in every page. The words of a
//! piece have no such place, so a change of them is a change of the rest of the page.
//! The regions in the table and the rest of the page together make up the whole page, so
//! two builds of one document that differ anywhere differ either in the rest of the page
//! or in one entry of the table. A live preview can compare two builds this way, to tell
//! a change of one region, which it applies in place, from a change of the page.
//!
//! # Which pieces a render builds
//!
//! A piece is a document-level value that a backend places before or after the document's
//! content, such as the title, the list of footnotes or the bibliography ([`Piece`]). The
//! pieces that a compose template accepts are the pieces that a render request places on
//! the backend of the compose template's output: the host sets the library's selection of
//! pieces for that backend from the chosen compose template ([`accepted_pieces`],
//! [`select_pieces`]), so that a piece that the compose template does not place is never
//! built. The pieces that the compose template *requires* ([`Template::requires`]) are
//! placed although a piece may be declared off by default: [`select_pieces`] includes
//! them. When the plan is built, the composer declares that it reads each required piece;
//! a plan in which no stage provides a required piece therefore fails to build, with an
//! error that gives the piece's name.
//!
//! # Map of the items
//!
//! - [`ComposeFeature`], [`FEATURE`]: the feature.
//! - [`ENGINES_SLOT`], [`TemplateEngine`], [`engines`] ([`ContentOnlyEngine`]): the
//!   template engines.
//! - [`TEMPLATES_SLOT`], [`Template`], [`Template::from_data`], [`Accepts`],
//!   [`Delimiters`], [`TemplateArtifact`], [`TemplateFiles`]: the compose templates.
//! - [`TemplateContext`], [`PieceValue`]: what a template engine is given.
//! - [`ComposeOptions`], [`TemplateChoice`], [`DEFAULT_TEMPLATE`], [`chosen_template`]:
//!   choosing a compose template.
//! - [`served_output`], [`stage_name`], [`accepted_pieces`], [`select_pieces`]: what the
//!   host calls when it prepares the plan of an output.
//! - [`HTML_REGIONS`], [`ComposedRegions`], [`ComposedRegion`], [`ComposedKind`],
//!   [`region_id`], [`REGION_ID_PREFIX`]: the composer's table on HTML.
//! - [`TemplateFailed`]: the condition of a compose template that cannot be applied.

pub mod engines;

use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::core::artifact::{
    Artifact, ArtifactId, ArtifactNamer, ArtifactRef, ArtifactSet, NamingScope, RelPath, media_type_of,
};
use crate::core::data::store::StageOut;
use crate::core::data::table::{PieceClass, ProjectCx, Table, TableHandle, TableName};
use crate::core::piece::{Piece, PieceDecl};
use crate::core::plan::PlanCx;
use crate::core::region::{self, Region, Regions, RegionsOutput};
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, Finalizer, FlmFeature, FlmRegistry};
use crate::core::run::StageCx;
use crate::core::services::Arity;
use crate::core::settings::SettingsDecl;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::{FlmOrigin, FlmSpan};
use crate::model::names::Name;
use crate::model::options::{Bind, OptionKey, hash_value};
use crate::stdcore::backends::pieces::{self as core_pieces, PiecesRenderOptions};
use crate::stdcore::backends::html;
use crate::stdcore::backends::html::escape::escape_attr_into;
use crate::stdcore::backends::lowering::{NAMER_SLOT, REGIONS_SLOT};
use crate::stdcore::features::meta::DocMeta;
use crate::techy::error::{Diagnostic, DiagnosticInfo};
use crate::techy::serialize::SerialValue;
use crate::techy::source::{Source, SourceSpan};
use serde_json::{Map, Value};

pub use engines::{CONTENT_ONLY, ContentOnlyEngine};

/// The name under which the feature is registered, `compose`.
pub const FEATURE: &str = "compose";

/// The name of the service slot `flm.compose.engines`, which takes any number of
/// template engines ([`TemplateEngine`]), each with a name. Where several engines have
/// the name a template gives, the engine of the feature placed last in the feature order
/// applies.
pub const ENGINES_SLOT: &str = "flm.compose.engines";

/// The name of the service slot `flm.compose.templates`, which takes any number of compose
/// templates ([`Template`]), each with a name and the output it serves.
pub const TEMPLATES_SLOT: &str = "flm.compose.templates";

/// The name of the compose template that a render uses when the settings choose none,
/// `simple`. The feature ships no compose template of that name: a host contributes the
/// compose templates it offers, and where none of them serves an output under the chosen
/// name, the composer of that output reports [`TemplateFailed`].
pub const DEFAULT_TEMPLATE: &str = "simple";

/// The prefix of the identifier of the element that holds a region on HTML, `flm-region-`:
/// see [`region_id`].
pub const REGION_ID_PREFIX: &str = "flm-region-";

/// The typed name of the composer's table on HTML, `flm.compose.html.regions`
/// ([`ComposedRegions`]): the HTML that the page holds under each region's identifier, the
/// words of the page's title, and the rest of the page. See the [module
/// documentation](self#on-html-one-element-per-region-and-the-composers-table).
pub const HTML_REGIONS: TableName<ComposedRegions> = TableName::new("flm.compose.html.regions");

/// Condition `flm.compose.template-failed` (Error): the chosen compose template cannot be
/// applied. The stage `flm.compose.<output>` reports it when the settings that choose the
/// compose template cannot be read, when no compose template of the chosen name serves
/// the output or two packages ship one each, when its template engine is not registered,
/// when the output's regions, the files written beside the entry artifact or the
/// document's metadata cannot be read, when its template engine cannot give the files it
/// ships ([`TemplateEngine::artifacts`]), when it ships files and no package in use
/// provides a naming of files, when its text or the text of a file it applies does not
/// compile or refers to a value that nothing provides, or when the output's artifact set
/// cannot be written. The output is then not made, with one exception: when a file that
/// the compose template ships cannot be added to the artifact set after the entry artifact
/// was written, the output is made without that file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateFailed {
    /// The compose template's name; `None` when the settings that choose it cannot be read.
    pub template: Option<String>,
    /// The name of the output it was applied for.
    pub output: String,
    /// What went wrong, as the template engine, the search for the compose template or the
    /// failed read states it.
    pub detail: String,
}

impl fmt::Display for TemplateFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.template {
            Some(template) => write!(
                f,
                "the compose template `{template}` for the output `{}` cannot be applied: {}",
                self.output, self.detail
            ),
            None => write!(f, "the compose template for the output `{}` cannot be chosen: {}", self.output, self.detail),
        }
    }
}

impl DiagnosticInfo for TemplateFailed {
    const IDENTIFIER: &'static str = "flm.compose.template-failed";
}

// --- the outputs the composer serves ---------------------------------------------------------

/// Returns what a backend states about the output `output` of `registry` when that output
/// is made of output regions ([`RegionsOutput`], contributed to the service slot
/// [`REGIONS_SLOT`]), and `None` otherwise. The composer serves exactly those outputs.
///
/// The output is matched by the table of its artifact set: an output of another name
/// whose artifact set is written to the table of such an output is served as that output,
/// with that output's compose templates. Where several backends state one table, the
/// statement of the feature placed last in the feature order applies.
pub fn served_output<'r>(registry: &'r FlmRegistry, output: &str) -> Option<&'r RegionsOutput> {
    let table = &registry.output(output)?.def.table;
    registry
        .services()
        .services_by_precedence_named::<RegionsOutput>(REGIONS_SLOT)
        .find(|served| &served.document == table)
}

/// Returns whether `output` is written by the HTML backend: a value that a compose template
/// writes is then escaped for HTML, and every region placed carries its name.
fn is_html(output: &RegionsOutput) -> bool {
    output.backend.as_str() == html::BACKEND
}

/// Returns the name of the composer's stage for the output `output`,
/// `flm.compose.<output>`.
pub fn stage_name(output: &str) -> String {
    format!("flm.compose.{output}")
}

/// Returns the identifier of the element that holds the region `name` on HTML:
/// [`REGION_ID_PREFIX`] followed by the name as it is written, dots included (a `.` is
/// allowed in an HTML identifier). Two distinct names therefore never share an
/// identifier, and the name is read back from the identifier by removing the prefix. The
/// identifier is derived from the name alone, so adding a region never changes the
/// identifier of another.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::compose::region_id;
///
/// assert_eq!(region_id("content"), "flm-region-content");
/// assert_eq!(region_id("flm.meta.title"), "flm-region-flm.meta.title");
/// assert_ne!(region_id("a.b-c"), region_id("a-b.c"));
/// ```
pub fn region_id(name: &str) -> String {
    format!("{REGION_ID_PREFIX}{name}")
}

/// Returns the element that holds a region on HTML, written around `inside`: a `<div>`
/// whose `id` is [`region_id`] of `name`, escaped as an attribute value.
fn region_element(name: &str, inside: &str) -> String {
    let mut id = String::new();
    escape_attr_into(&mut id, &region_id(name));
    if inside.is_empty() {
        format!("<div id=\"{id}\"></div>")
    } else {
        format!("<div id=\"{id}\">\n{inside}\n</div>")
    }
}

// --- templates and engines ---------------------------------------------------------------------

/// Which pieces a compose template accepts: every piece, or the pieces listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Accepts {
    /// Every piece that the registry declares.
    All,
    /// The pieces listed, and no other.
    Only(Vec<Name>),
}

/// The three pairs of delimiters of a compose template written in a Jinja-like template
/// language (such as the language of a host's `minijinja` engine), for a compose template
/// whose output itself uses the usual ones (`{% %}`, `{{ }}`, `{# #}`), such as LaTeX. A
/// template engine that reads no delimiters ignores them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delimiters {
    /// The start and the end of a statement.
    pub block: (String, String),
    /// The start and the end of a place for a value.
    pub variable: (String, String),
    /// The start and the end of a comment.
    pub comment: (String, String),
}

/// A compose template: a text with named places for an output's regions, applied by a
/// template engine (see the [module documentation](self)).
///
/// A feature contributes one to [`TEMPLATES_SLOT`]; [`Template::from_data`] makes one from
/// a description given as data and the readers of the files it gives.
///
/// Two compose templates are equal when every field is equal and their [`files`](Self::files)
/// are both absent or are the same handle; `Debug` prints the handle as present or absent.
#[derive(Clone)]
pub struct Template {
    /// The compose template's name, which the setting `compose.template` gives.
    pub name: String,
    /// The package that ships the compose template, as messages show it (for instance the
    /// name of a compiled package, or the identity of an extension's package,
    /// `<identity>#<package>`). A
    /// problem with two compose templates of the same name gives the packages that ship
    /// them.
    pub package: String,
    /// The name of the output it serves.
    pub output: String,
    /// The name of the template engine that applies it.
    pub engine: String,
    /// Its text, in the template engine's language; empty for a template engine that reads
    /// no text (`content-only`). [`Template::from_data`] reads it from the file that the key
    /// `main` of the description gives, and leaves it empty without `main`.
    pub text: String,
    /// The pieces it accepts: the pieces that a render request places on the backend of
    /// the compose template's output ([`accepted_pieces`]).
    pub accepts: Accepts,
    /// The pieces it requires: each is placed although it may be declared off by default
    /// ([`select_pieces`]), and a plan in which no stage provides one of them fails.
    pub requires: Vec<Name>,
    /// Its own settings, with their defaults, which the render option `template_config`
    /// can change ([`ComposeOptions::template_config`]).
    pub config: Map<String, Value>,
    /// The delimiters of its language, where they differ from the template engine's
    /// usual ones.
    pub delimiters: Option<Delimiters>,
    /// The files it ships, in the order of its description (see the [module
    /// documentation](self#files-a-compose-template-ships)).
    pub artifacts: Vec<TemplateArtifact>,
    /// The files its text, and the text of a file it ships and applies, may include, read
    /// when it is applied (see the [module
    /// documentation](self#files-a-compose-template-includes)); `None` for a compose
    /// template with no files of its own.
    pub files: Option<Arc<dyn TemplateFiles>>,
}

impl PartialEq for Template {
    fn eq(&self, other: &Template) -> bool {
        let same_files = match (&self.files, &other.files) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        self.name == other.name
            && self.package == other.package
            && self.output == other.output
            && self.engine == other.engine
            && self.text == other.text
            && self.accepts == other.accepts
            && self.requires == other.requires
            && self.config == other.config
            && self.delimiters == other.delimiters
            && self.artifacts == other.artifacts
            && same_files
    }
}

impl fmt::Debug for Template {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Template")
            .field("name", &self.name)
            .field("package", &self.package)
            .field("output", &self.output)
            .field("engine", &self.engine)
            .field("text", &self.text)
            .field("accepts", &self.accepts)
            .field("requires", &self.requires)
            .field("config", &self.config)
            .field("delimiters", &self.delimiters)
            .field("artifacts", &self.artifacts)
            .field("files", &format_args!("{}", if self.files.is_some() { "present" } else { "absent" }))
            .finish()
    }
}

/// The files a compose template may include, read when the compose template is applied:
/// the handle that [`Template::files`] holds (see the [module
/// documentation](self#files-a-compose-template-includes)).
///
/// The package that makes the compose template gives it; a template engine reads through
/// it the file that an include names. The handle resolves the path, from the folder of
/// the compose template's description, and refuses a path that leaves the package's
/// files.
pub trait TemplateFiles: Send + Sync {
    /// Returns the text of the file at `path`, a path relative to the folder of the
    /// compose template's description, with forward slashes; `Ok(None)` when there is no
    /// such file.
    ///
    /// # Errors
    ///
    /// Returns a complete text, for a person to read, that names the compose template and
    /// the path and says why the file cannot be included: it cannot be read, it is not
    /// UTF-8 text, or its path leaves the package's files. A template engine reports it
    /// as it is.
    fn read_text(&self, path: &str) -> Result<Option<String>, String>;
}

/// One file, or one folder, a compose template ships, written beside the entry artifact
/// (see the [module documentation](self#files-a-compose-template-ships)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateArtifact {
    /// One file (the key `file` of the description).
    File {
        /// The file's name, by which the compose template reads its path
        /// (`artifacts.<name>`); its identity is `compose:<name>`.
        name: String,
        /// The file's extension, without the dot (`css`).
        extension: String,
        /// The file's media type (`text/css`).
        media_type: String,
        /// Whether the composer applies the bytes, as text, as a template before writing
        /// them.
        apply: bool,
        /// The file's content, as the compose template's package holds it.
        bytes: Vec<u8>,
    },
    /// A folder (the key `dir` of the description), shipped as one folder artifact
    /// ([`Artifact::folder`]).
    Folder {
        /// The folder's name, by which the compose template reads its path
        /// (`artifacts.<name>`); its identity is `compose:<name>`.
        name: String,
        /// The files of the folder, each by its path relative to the folder (forward
        /// slashes, no leading slash) and its content, in the order the folder reader of
        /// [`Template::from_data`] gives them (the lexical order of the paths). It is
        /// never empty.
        files: Vec<(String, Vec<u8>)>,
    },
}

impl TemplateArtifact {
    /// Returns the name of the file or folder, by which the compose template reads its
    /// path (`artifacts.<name>`).
    pub fn name(&self) -> &str {
        match self {
            TemplateArtifact::File { name, .. } | TemplateArtifact::Folder { name, .. } => name,
        }
    }
}

/// The pieces that a description accepts, as written: `all`, or a list of piece names.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum AcceptsData {
    All(String),
    Only(Vec<String>),
}

/// A compose template's description, as written.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Description {
    output: String,
    engine: String,
    #[serde(default)]
    main: Option<String>,
    accepts: AcceptsData,
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default)]
    config: Map<String, Value>,
    #[serde(default)]
    delimiters: Option<Delimiters>,
    #[serde(default)]
    artifacts: Map<String, Value>,
}

/// The description of one file or folder a compose template ships, as written.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactDescription {
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    dir: Option<String>,
    #[serde(default)]
    template: Option<bool>,
    #[serde(default)]
    media_type: Option<String>,
}

/// Returns the extension of the file `file` (a path whose parts are separated by `/`): what
/// follows the last `.` of its last part, when that `.` neither begins nor ends the part.
fn extension_of_file(file: &str) -> Option<&str> {
    let last = file.rsplit('/').next().unwrap_or(file);
    match last.rfind('.') {
        Some(dot) if dot > 0 && dot + 1 < last.len() => Some(&last[dot + 1..]),
        _ => None,
    }
}

/// The reader of the files a description lists, as [`Template::from_data`] takes it.
type FileReader<'a> = &'a dyn Fn(&str) -> Result<Option<Vec<u8>>, String>;

/// The reader of the folders a description lists, as [`Template::from_data`] takes it.
type FolderReader<'a> = &'a dyn Fn(&str) -> Result<Option<Vec<(String, Vec<u8>)>>, String>;

/// Returns the file or folder `name` that the description `description` of the compose
/// template `template` (of template engine `engine`) lists, read through `files` or
/// `folders`.
fn template_artifact(
    template: &str,
    engine: &str,
    name: &str,
    description: Value,
    files: FileReader<'_>,
    folders: FolderReader<'_>,
) -> Result<TemplateArtifact, String> {
    let description: ArtifactDescription = serde_json::from_value(description).map_err(|error| {
        format!("the artifact `{name}` of the compose template `{template}` cannot be read: {error}")
    })?;
    let file = match (&description.file, &description.dir) {
        (Some(_), Some(_)) => {
            return Err(format!(
                "the artifact `{name}` of the compose template `{template}` gives both `file` and `dir`; give one"
            ));
        }
        (None, Some(dir)) => return template_folder(template, name, dir, &description, folders),
        (Some(file), None) => file.as_str(),
        (None, None) => {
            return Err(format!("the artifact `{name}` of the compose template `{template}` must give `file` or `dir`"));
        }
    };
    let apply = description.template.unwrap_or(false);
    if apply && engine == CONTENT_ONLY {
        return Err(format!(
            "the artifact `{name}` of the compose template `{template}` sets `template` to `true`, and the \
             template engine `{CONTENT_ONLY}` applies no text"
        ));
    }
    let Some(extension) = extension_of_file(file) else {
        return Err(format!(
            "the file `{file}` of the artifact `{name}` of the compose template `{template}` must have an \
             extension"
        ));
    };
    let media_type = match description.media_type.as_deref().or_else(|| media_type_of(extension)) {
        Some(media_type) => media_type.to_string(),
        None => {
            return Err(format!(
                "the artifact `{name}` of the compose template `{template}` must give `media_type`, since \
                 the extension `{extension}` of its file `{file}` has no known media type"
            ));
        }
    };
    let bytes = match files(file) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            return Err(format!(
                "the file `{file}` of the artifact `{name}` of the compose template `{template}` is not a \
                 file of the compose template's package"
            ));
        }
        Err(error) => {
            return Err(format!(
                "the file `{file}` of the artifact `{name}` of the compose template `{template}` cannot be \
                 read: {error}"
            ));
        }
    };
    if apply && std::str::from_utf8(&bytes).is_err() {
        return Err(format!(
            "the file `{file}` of the artifact `{name}` of the compose template `{template}` must be text, \
             since `template` is `true`"
        ));
    }
    Ok(TemplateArtifact::File {
        name: name.to_string(),
        extension: extension.to_string(),
        media_type,
        apply,
        bytes,
    })
}

/// Returns the folder `dir` that the artifact `name` of the compose template `template`
/// ships (its description `description`), read through `folders`.
fn template_folder(
    template: &str,
    name: &str,
    dir: &str,
    description: &ArtifactDescription,
    folders: FolderReader<'_>,
) -> Result<TemplateArtifact, String> {
    let given: Vec<&str> = [("template", description.template.is_some()), ("media_type", description.media_type.is_some())]
        .into_iter()
        .filter_map(|(key, given)| given.then_some(key))
        .collect();
    if !given.is_empty() {
        let verb = if given.len() == 1 { "applies" } else { "apply" };
        return Err(format!(
            "the artifact `{name}` of the compose template `{template}` gives `dir` together with `{}`, which {verb} \
             to a file only",
            given.join("` and `")
        ));
    }
    match folders(dir)? {
        None => Err(format!("the shipped folder `{dir}` of the compose template `{template}` does not exist")),
        Some(files) if files.is_empty() => {
            Err(format!("the shipped folder `{dir}` of the compose template `{template}` holds no file"))
        }
        Some(files) => Ok(TemplateArtifact::Folder { name: name.to_string(), files }),
    }
}

impl Template {
    /// The compose template `name` of the package `package` (the factory's name), from
    /// its description as data, `text`, which reads the file of its text, `files`, which
    /// reads a file the description lists, `folders`, which reads a folder it lists, and
    /// `template_files`, the handle of the files it may include.
    ///
    /// The description is a map of these keys:
    ///
    /// - `output`: the output the compose template serves;
    /// - `engine`: the name of its template engine;
    /// - `main` (optional): the path of the file that holds its text, relative to the
    ///   folder of the description; without it, the compose template's text is empty;
    /// - `accepts`: the pieces it accepts, `all` or a list of piece names;
    /// - `requires` (optional): the pieces it requires, a list of piece names;
    /// - `config` (optional): its own settings, with their defaults;
    /// - `delimiters` (optional): the delimiters of its language where they differ from
    ///   the usual ones ([`Delimiters`]);
    /// - `artifacts` (optional): the files and folders it ships (see the [module
    ///   documentation](self#files-a-compose-template-ships)), a map from each one's name
    ///   to a map of these keys: `file`, the file's path, relative to the folder of the
    ///   description, whose extension is the artifact's; `template` (optional, default
    ///   `false`), whether the file's text is applied as a template of the same engine;
    ///   `media_type` (optional), the file's media type, by default the one its extension
    ///   gives ([`media_type_of`]). In place of `file`, the key `dir` gives the path of a
    ///   folder, relative to the folder of the description, shipped whole as one folder
    ///   artifact ([`TemplateArtifact::Folder`]); `template` and `media_type` are refused
    ///   with it, since a folder is never applied as a template and its files' media types
    ///   come from their extensions.
    ///
    /// `text(path)` returns the text of the file at `path` as the description's `main`
    /// writes it, `Ok(None)` when there is no such file, and `Err` with a text for a person
    /// when it cannot be read. It is called once when the description gives `main`, and not
    /// at all otherwise.
    ///
    /// `files(path)` returns the bytes of the file at `path` as the description writes it,
    /// `Ok(None)` when there is no such file, and `Err` with a text for a person when it
    /// cannot be read. It is called once per file listed.
    ///
    /// `folders(path)` returns the files of the folder at `path` as the description writes
    /// it, each by its path relative to that folder (forward slashes, no leading slash) and
    /// its bytes, listed at every depth in lexical order of the paths; `Ok(None)` when
    /// there is no such folder; and `Err` with a complete text for a person when the folder
    /// cannot be shipped (it cannot be read, or its path is refused), which `from_data`
    /// returns as it is. The reader decides which names it leaves out (an extension's
    /// reader leaves out the names beginning with `.`). It is called once per folder
    /// listed.
    ///
    /// `template_files` is the handle through which the compose template's includes are
    /// read when it is applied ([`Template::files`]), `None` for a compose template with no
    /// files of its own. `from_data` keeps it and reads nothing through it.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::features::compose::{Accepts, Template};
    /// use serde_json::json;
    ///
    /// let template = Template::from_data(
    ///     "mine",
    ///     "page",
    ///     json!({ "output": "html", "engine": "host", "main": "page.html", "accepts": "all" }),
    ///     &|path| Ok((path == "page.html").then(|| String::from("<main>{{ regions.content }}</main>"))),
    ///     &|_| Ok(None),
    ///     &|_| Ok(None),
    ///     None,
    /// )
    /// .unwrap();
    /// assert_eq!((template.output.as_str(), &template.accepts), ("html", &Accepts::All));
    /// assert_eq!(template.text, "<main>{{ regions.content }}</main>");
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a text, for a person to read, that says why the description cannot be
    /// read: it is not a map, it holds an unknown key, it lacks `output`, `engine` or
    /// `accepts`, a value has the wrong shape, or `accepts` is neither `all` nor a list of
    /// names; or why its text cannot be read: there is no file at `main`, or it cannot be
    /// read (the text of `text`'s `Err`); or why a file it lists cannot be shipped: there
    /// is no such file, it cannot be read, it has no extension, its media type is not
    /// known, or it is applied as a template while it is not text or the engine is
    /// `content-only`; or why a folder it lists cannot be shipped: the artifact gives both
    /// `file` and `dir`, or neither, or `dir` together with `template` or `media_type`,
    /// there is no such folder, it holds no file, or the text of `folders`'s `Err`.
    // The readers' types are spelled out, so that their contracts read in the signature.
    #[allow(clippy::type_complexity)]
    pub fn from_data(
        package: &str,
        name: &str,
        description: Value,
        text: &dyn Fn(&str) -> Result<Option<String>, String>,
        files: &dyn Fn(&str) -> Result<Option<Vec<u8>>, String>,
        folders: &dyn Fn(&str) -> Result<Option<Vec<(String, Vec<u8>)>>, String>,
        template_files: Option<Arc<dyn TemplateFiles>>,
    ) -> Result<Template, String> {
        let description: Description = serde_json::from_value(description)
            .map_err(|error| format!("the description of the compose template `{name}` cannot be read: {error}"))?;
        let accepts = match description.accepts {
            AcceptsData::All(all) if all == "all" => Accepts::All,
            AcceptsData::All(other) => {
                return Err(format!(
                    "in the description of the compose template `{name}`, `accepts` must be `all` or a list \
                     of piece names, not `{other}`"
                ));
            }
            AcceptsData::Only(names) => Accepts::Only(names.into_iter().map(Name::from).collect()),
        };
        let template_text = match description.main.as_deref() {
            None => String::new(),
            Some(main) => match text(main) {
                Ok(Some(read)) => read,
                Ok(None) => {
                    return Err(format!(
                        "the text `{main}` that `main` gives for the compose template `{name}` is not a file of \
                         the compose template's package"
                    ));
                }
                Err(error) => {
                    return Err(format!(
                        "the text `{main}` that `main` gives for the compose template `{name}` cannot be read: \
                         {error}"
                    ));
                }
            },
        };
        let mut artifacts = Vec::new();
        for (artifact, data) in description.artifacts {
            artifacts.push(template_artifact(name, &description.engine, &artifact, data, files, folders)?);
        }
        Ok(Template {
            name: String::from(name),
            package: String::from(package),
            output: description.output,
            engine: description.engine,
            text: template_text,
            accepts,
            requires: description.requires.into_iter().map(Name::from).collect(),
            config: description.config,
            delimiters: description.delimiters,
            artifacts,
            files: template_files,
        })
    }
}

/// One piece as a compose template reads it: see the [module
/// documentation](self#what-a-compose-template-reads).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PieceValue {
    /// The piece's name.
    pub name: String,
    /// Where the backend places it: `before-content` or `after-content`.
    pub place: String,
    /// Its text in the output's format; on HTML, inside the element that carries the
    /// piece's name ([`region_id`]).
    pub text: String,
    /// Its displayed characters as plain text.
    pub words: String,
}

/// What a compose template reads: see the [module
/// documentation](self#what-a-compose-template-reads). The stage hands it to the template
/// engine together with the compose template.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateContext {
    /// Whether the output is HTML. A template engine then escapes for HTML every value
    /// that the compose template writes, except a region and a piece's text, which are
    /// already HTML.
    pub html: bool,
    /// Every output region, by name, as text, in the backend's order; on HTML, the content
    /// and each piece inside the element that carries its name.
    pub regions: Vec<(String, String)>,
    /// The regions that are pieces, in the backend's order.
    pub pieces: Vec<PieceValue>,
    /// The document's metadata, as data.
    pub meta: Value,
    /// The compose template's own settings: its defaults, with the values of the render
    /// option `template_config` written over them.
    pub config: Map<String, Value>,
    /// The path of each file the compose template ships, as (name, path), in the order
    /// in which its template engine gives them ([`TemplateEngine::artifacts`]; by default
    /// the order of its description, [`Template::artifacts`]). A path is relative to the
    /// folder where the program writes the output's files, where the entry file lies
    /// under a host's usual naming of files. A template engine exposes them as the value
    /// `artifacts`, a map from name to path.
    pub artifacts: Vec<(String, String)>,
}

/// A template engine: the code that applies a compose template written in its language to
/// a [`TemplateContext`].
///
/// A feature contributes one to [`ENGINES_SLOT`]; a compose template refers to its
/// template engine by name ([`Template::engine`]).
pub trait TemplateEngine: Send + Sync {
    /// Returns the engine's name, by which a compose template refers to it.
    fn name(&self) -> &str;

    /// Returns the text of the compose template `template` applied to `context`.
    ///
    /// # Errors
    ///
    /// Returns a text, for a person to read, that says why the compose template cannot be
    /// applied: for instance its text does not compile, or it refers to a value that
    /// `context` does not provide. The stage reports it in [`TemplateFailed`].
    ///
    /// An engine whose language includes other files reads them through
    /// [`Template::files`], and reports the text of its `Err` as it is.
    fn render(&self, template: &Template, context: &TemplateContext) -> Result<String, String>;

    /// Returns the files the compose template `template` ships when it is applied to
    /// `context`, which the composer names and writes beside the entry artifact, in this
    /// order, and whose paths it then gives to [`render`](Self::render) in
    /// [`TemplateContext::artifacts`]. The composer calls it once per application, before
    /// it names the files, so `context` holds no path yet: its `artifacts` is empty.
    ///
    /// The default returns the compose template's own list ([`Template::artifacts`]). An
    /// engine whose compose templates choose their files when they are applied returns
    /// them here.
    ///
    /// # Errors
    ///
    /// Returns a text, for a person to read, that says why the files cannot be given. The
    /// stage reports it in [`TemplateFailed`], as for [`render`](Self::render).
    fn artifacts(&self, template: &Template, context: &TemplateContext) -> Result<Vec<TemplateArtifact>, String> {
        let _ = context;
        Ok(template.artifacts.clone())
    }

    /// Applies `text`, written in this engine's language under the settings of `template`
    /// (its name, its delimiters), to `context`, and returns the result. The composer
    /// calls it for a file the compose template ships and applies
    /// (the field `apply` of [`TemplateArtifact::File`]).
    ///
    /// The default applies [`render`](Self::render) to a copy of `template` whose text is
    /// `text`, which serves an engine that reads the compose template's text.
    ///
    /// # Errors
    ///
    /// As [`render`](Self::render).
    fn render_text(&self, template: &Template, text: &str, context: &TemplateContext) -> Result<String, String> {
        self.render(&Template { text: text.to_string(), ..template.clone() }, context)
    }
}

// --- the settings --------------------------------------------------------------------------

/// Which compose template a render uses: one name for every output, or a name per output.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum TemplateChoice {
    /// The compose template of this name, for whichever output a render request makes.
    Name(String),
    /// A compose template's name per output's name; an output not listed takes
    /// [`DEFAULT_TEMPLATE`].
    ByOutput(BTreeMap<String, String>),
}

impl TemplateChoice {
    /// Returns the name of the compose template chosen for the output `output`.
    pub fn for_output(&self, output: &str) -> &str {
        match self {
            TemplateChoice::Name(name) => name,
            TemplateChoice::ByOutput(names) => {
                names.get(output).map(String::as_str).unwrap_or(DEFAULT_TEMPLATE)
            }
        }
    }
}

/// The render options of the feature `compose`, set per render request: the compose
/// template each output is assembled with, and each compose template's own settings.
///
/// It is an option type bound to the render request ([`Bind::Render`];
/// [`crate::model::options`]). A host combines its value from its sources of settings.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::compose::{ComposeOptions, DEFAULT_TEMPLATE};
///
/// assert_eq!(ComposeOptions::default().template.for_output("html"), DEFAULT_TEMPLATE);
/// ```
#[derive(Debug, Clone, PartialEq, serde::Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ComposeOptions {
    /// The compose template: a name, or a map from an output's name to a compose
    /// template's name. The default is [`DEFAULT_TEMPLATE`], `simple`, for every output.
    pub template: TemplateChoice,
    /// A map from an output's name to a map from a compose template's name to that
    /// compose template's own settings for that output, written over its defaults key by
    /// key: `{latex-rendered: {simple: {documentclass: report}}}`. Two compose templates of
    /// the same name for two outputs are two compose templates, so their settings are
    /// given apart ([`ComposeOptions::config_for`]). An output's name that names no output
    /// a render makes is never read, as in [`TemplateChoice::ByOutput`].
    pub template_config: BTreeMap<String, BTreeMap<String, Map<String, Value>>>,
}

impl ComposeOptions {
    /// Returns the settings that [`template_config`](Self::template_config) gives for the
    /// compose template `template` of the output `output`, if it gives any.
    pub fn config_for(&self, output: &str, template: &str) -> Option<&Map<String, Value>> {
        self.template_config.get(output)?.get(template)
    }
}

impl Default for ComposeOptions {
    fn default() -> ComposeOptions {
        ComposeOptions {
            template: TemplateChoice::Name(String::from(DEFAULT_TEMPLATE)),
            template_config: BTreeMap::new(),
        }
    }
}

// An option type is part of a plan's key, so it hashes, and two values it holds equal hash
// equal: the settings of a template are data, whose maps are equal whatever the order of
// their entries, so they are hashed with every map's entries in the order of their keys.
impl Hash for ComposeOptions {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.template.hash(state);
        for (output, templates) in &self.template_config {
            output.hash(state);
            for (name, config) in templates {
                name.hash(state);
                hash_value(&Value::Object(config.clone()), state);
            }
        }
    }
}

/// Returns the declaration of the feature's settings: the fields of [`ComposeOptions`]
/// are its keys, set per render request.
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE).options::<ComposeOptions>(
        Bind::Render,
        "The compose template each output is assembled with (`template`: a name, or a map \
         from an output's name to a compose template's name), and each compose template's \
         own settings (`template_config`, a map from an output's name to a map from a \
         compose template's name to its settings for that output). Set per render request.",
    )
}

// --- choosing a template -----------------------------------------------------------------------

/// Returns the compose template of the registry `registry` that `options` chooses for the
/// output `output`.
///
/// # Errors
///
/// Returns a text, for a person to read, when no compose template of the chosen name
/// serves the output (the text lists those that do, or says that none does), or when several do because two
/// packages ship a compose template of the same name for that output.
pub fn chosen_template<'r>(
    registry: &'r FlmRegistry,
    options: &ComposeOptions,
    output: &str,
) -> Result<&'r Template, String> {
    let name = options.template.for_output(output);
    let mut matching = registry
        .services()
        .services_named::<Template>(TEMPLATES_SLOT)
        .filter(|template| template.name == name && template.output == output);
    let first = matching.next();
    if let Some(template) = first {
        let others: Vec<&Template> = matching.collect();
        if !others.is_empty() {
            let packages: Vec<&str> =
                std::iter::once(template).chain(others).map(|template| template.package.as_str()).collect();
            return Err(format!(
                "only one package in use may ship a compose template named `{name}` for the output `{output}`, \
                 and the packages `{}` each ship one",
                packages.join("` and `")
            ));
        }
    }
    first.ok_or_else(|| {
            let known: Vec<&str> = registry
                .services()
                .services_named::<Template>(TEMPLATES_SLOT)
                .filter(|template| template.output == output)
                .map(|template| template.name.as_str())
                .collect();
            if known.is_empty() {
                format!(
                    "the compose template `{name}` must be one of the compose templates for the output \
                     `{output}`, and no package in use ships one for that output"
                )
            } else {
                format!(
                    "the compose template `{name}` must be one of the compose templates for the output \
                     `{output}`: `{}`",
                    known.join("`, `")
                )
            }
        })
}

/// Returns the pieces that the compose template `template` accepts among those that
/// `registry` declares, in the order of their declarations: the selection of pieces that
/// a host writes for the output's backend into [`PiecesRenderOptions::backends`].
pub fn accepted_pieces(registry: &FlmRegistry, template: &Template) -> Vec<Name> {
    core_pieces::declared_pieces(registry)
        .filter(|decl| match &template.accepts {
            Accepts::All => true,
            Accepts::Only(names) => names.contains(&decl.name),
        })
        .map(|decl| decl.name.clone())
        .collect()
}

/// Sets, in `pieces`, the selection of pieces of the backend of `output` to the pieces
/// that the compose template `template` accepts ([`accepted_pieces`]), and adds the
/// pieces it requires ([`Template::requires`]) to the pieces placed although declared off
/// by default ([`PiecesRenderOptions::include`]), so that what the compose template
/// requires is placed.
///
/// The host calls it when it prepares the plan of an output that the composer serves.
pub fn select_pieces(
    registry: &FlmRegistry,
    template: &Template,
    output: &RegionsOutput,
    pieces: &mut PiecesRenderOptions,
) {
    pieces.backends.insert(
        output.backend.clone(),
        accepted_pieces(registry, template).into_iter().collect(),
    );
    pieces.include.extend(template.requires.iter().cloned());
}

// --- the composer's table on HTML ----------------------------------------------------------

/// What one region's place in the page holds: HTML, or plain words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposedKind {
    /// HTML, placed inside the element that carries the region's identifier.
    Html,
    /// Plain words, placed where no element is allowed: the words of the page's title
    /// (the region [`title`](region::TITLE), placed in `<title>`), kept under that
    /// region's identifier. A piece's words are not in the table but in the rest of the
    /// page (see the [module
    /// documentation](self#on-html-one-element-per-region-and-the-composers-table)).
    Words,
}

/// One region as the page holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedRegion {
    /// The identifier: [`region_id`] of the region's name.
    pub id: String,
    /// The region's name.
    pub name: String,
    /// Whether the text is HTML or words.
    pub kind: ComposedKind,
    /// The text: the HTML inside the element, or the words.
    pub text: String,
}

/// The composer's table on HTML, [`HTML_REGIONS`]: each region that the composer placed,
/// and the rest of the page (see the [module
/// documentation](self#on-html-one-element-per-region-and-the-composers-table)).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComposedRegions {
    /// The rest of the page, everything outside its regions: the page with exactly what
    /// [`regions`](Self::regions) holds left out, that is with every element that carries
    /// a region's identifier empty and the title's words empty, and everything else as in
    /// the page (the stylesheets, the scripts, the language, the compose template's own
    /// markup, each piece's words). The regions and the rest of the page together make up
    /// the page, so two builds that differ anywhere differ in the rest of the page or in
    /// an entry of [`regions`](Self::regions).
    pub skeleton: String,
    /// Each region that the composer placed (the content and the pieces), and the words of
    /// the page's title.
    pub regions: Vec<ComposedRegion>,
}

impl ComposedRegions {
    /// Returns the region with the identifier `id`, if there is one.
    pub fn get(&self, id: &str) -> Option<&ComposedRegion> {
        self.regions.iter().find(|region| region.id == id)
    }
}

impl Table for ComposedRegions {
    /// The table has one writer per run: a second write replaces the first.
    fn absorb(&mut self, piece: ComposedRegions) {
        *self = piece;
    }

    /// `{skeleton, regions: [{id, name, kind, text}]}`.
    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        let regions = self
            .regions
            .iter()
            .map(|region| {
                SerialValue::Map(vec![
                    (String::from("id"), SerialValue::Str(region.id.clone())),
                    (String::from("name"), SerialValue::Str(region.name.clone())),
                    (
                        String::from("kind"),
                        SerialValue::Str(String::from(match region.kind {
                            ComposedKind::Html => "html",
                            ComposedKind::Words => "words",
                        })),
                    ),
                    (String::from("text"), SerialValue::Str(region.text.clone())),
                ])
            })
            .collect();
        SerialValue::Map(vec![
            (String::from("skeleton"), SerialValue::Str(self.skeleton.clone())),
            (String::from("regions"), SerialValue::List(regions)),
        ])
    }

    fn piece_class(&self) -> PieceClass {
        PieceClass::DocumentFact
    }
}

// --- the feature ---------------------------------------------------------------------------

/// The feature `compose`: see the [module documentation](self). A host registers it; it
/// is not part of the package [`stdcore`](crate::stdcore::package).
#[derive(Debug, Default, Clone, Copy)]
pub struct ComposeFeature;

impl ComposeFeature {
    /// The feature `compose`: the two service slots, the template engine `content-only`,
    /// the table `flm.compose.html.regions`, a composer per regions output. It contributes
    /// no compose template.
    pub fn new() -> ComposeFeature {
        ComposeFeature
    }
}

impl FlmFeature for ComposeFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Templates").docs(
            "the template engines, the compose templates, and for each output a stage that \
             assembles the output from its content and its other parts through the chosen \
             compose template",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.service_slot::<dyn TemplateEngine>(
            ENGINES_SLOT,
            Arity::OrderedMany,
            "the template engines, each with a name",
        )?;
        b.service_slot::<Template>(
            TEMPLATES_SLOT,
            Arity::OrderedMany,
            "the compose templates, each with a name and the output it serves",
        )?;
        b.contribute_to::<dyn TemplateEngine>(ENGINES_SLOT, Arc::new(ContentOnlyEngine))?;
        b.table(
            HTML_REGIONS,
            "the HTML that the page holds under each region's identifier, the words of the page's \
             title, and the rest of the page outside those regions",
        )?;
        b.finalizer(Arc::new(RegisterStages))
    }
}

/// The second round of the feature's registration ([`Finalizer`]): registers one stage
/// per output that a backend of the registry states is made of output regions, once every
/// feature has contributed to [`REGIONS_SLOT`].
struct RegisterStages;

impl Finalizer for RegisterStages {
    fn finalize(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let outputs: Vec<RegionsOutput> = b
            .services()
            .map(|services| services.services_named::<RegionsOutput>(REGIONS_SLOT).cloned().collect())
            .unwrap_or_default();
        for output in outputs {
            b.stage(Arc::new(ComposeStage { output }))?;
        }
        Ok(())
    }
}

// --- the stage -----------------------------------------------------------------------------

/// The composer's stage for one output, `flm.compose.<output>`: see the [module
/// documentation](self).
struct ComposeStage {
    output: RegionsOutput,
}

/// Returns an empty span whose origin is the stage `stage`, for a problem that has no
/// place in the document.
fn nowhere(stage: &str) -> FlmSpan {
    let source = Arc::new(Source::new("").with_origin(FlmOrigin::synthetic(stage)));
    SourceSpan::new(&source, 0..0)
}

impl ComposeStage {
    fn regions_table(&self) -> TableHandle<Regions> {
        self.output.regions.clone()
    }

    fn document_table(&self) -> TableHandle<ArtifactSet> {
        TableHandle::new(self.output.document.clone())
    }

    fn fail(&self, out: &mut StageOut<'_>, template: Option<&str>, detail: impl ToString) {
        out.diags.push(Diagnostic::error(
            TemplateFailed {
                template: template.map(String::from),
                output: self.output.output.to_string(),
                detail: detail.to_string(),
            },
            nowhere(&stage_name(self.output.output.as_str())),
        ));
    }
}

impl Stage for ComposeStage {
    fn decl(&self) -> StageDecl {
        let mut decl =
            StageDecl::new(stage_name(self.output.output.as_str()), Effect::Compute, Scope::Document)
                .writes(self.document_table());
        if self.output.capability != self.output.document {
            decl = decl.provides(self.output.capability.clone());
        }
        if is_html(&self.output) {
            decl = decl.writes(HTML_REGIONS);
        }
        decl.docs(format!(
            "assembles the `{}` output's entry artifact from its output regions through the \
             chosen compose template",
            self.output.output
        ))
    }

    fn needs(&self, cx: &PlanCx<'_>) -> Needs {
        let mut needs = Needs::new().reads(self.regions_table()).option::<ComposeOptions>();
        for artifacts in &self.output.artifacts {
            needs = needs.reads_opt(artifacts.clone());
        }
        // The metadata is read where a feature of the registry declares it.
        if cx.registry().option_decl(&OptionKey::of::<DocMeta>()).is_some() {
            needs = needs.option::<DocMeta>();
        }
        // A required piece is read, so that a plan in which nobody provides it fails.
        let default = ComposeOptions::default();
        let options = cx.render_option::<ComposeOptions>().unwrap_or(&default);
        if let Ok(template) = chosen_template(cx.registry(), options, self.output.output.as_str()) {
            let declared: Vec<&PieceDecl> = core_pieces::declared_pieces(cx.registry()).collect();
            for name in &template.requires {
                // A piece nobody declares is read under its table's usual name, which is
                // the piece's, so that the plan fails naming it.
                let table = match declared.iter().find(|decl| &decl.name == name) {
                    Some(decl) => decl.table.clone(),
                    None => TableHandle::<Piece>::new(name.clone()),
                };
                needs = needs.reads(table);
            }
        }
        needs
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let options = match cx.option::<ComposeOptions>() {
            Ok(options) => options,
            Err(error) => return self.fail(out, None, error),
        };
        let name = options.template.for_output(self.output.output.as_str());
        let template = match chosen_template(cx.registry(), options, self.output.output.as_str()) {
            Ok(template) => template,
            Err(detail) => return self.fail(out, Some(name), detail),
        };
        let Some(engine) = cx
            .registry()
            .services()
            .services_by_precedence_named::<dyn TemplateEngine>(ENGINES_SLOT)
            .find(|engine| engine.name() == template.engine)
        else {
            return self.fail(out, Some(name), format!("the template engine `{}`, in which the template is written, must be provided by a package in use, and none provides it", template.engine));
        };
        let regions = match cx.get(&self.regions_table()) {
            Ok(Some(regions)) => regions,
            Ok(None) => return,
            Err(error) => return self.fail(out, Some(name), error),
        };
        let mut beside = Vec::new();
        for artifacts in &self.output.artifacts {
            match cx.get(artifacts) {
                Ok(set) => beside.extend(set),
                Err(error) => return self.fail(out, Some(name), error),
            }
        }
        // The metadata is read where a feature of the registry declares it, as the plan
        // does; a failed read is reported as the stage's other failed reads are.
        let meta = if cx.registry().option_decl(&OptionKey::of::<DocMeta>()).is_some() {
            match cx.option::<DocMeta>() {
                Ok(meta) => meta_value(meta),
                Err(error) => return self.fail(out, Some(name), error),
            }
        } else {
            Value::Object(Map::new())
        };
        let mut config = template.config.clone();
        if let Some(given) = options.config_for(&template.output, &template.name) {
            for (key, value) in given {
                config.insert(key.clone(), value.clone());
            }
        }
        // The files the compose template ships, as its template engine answers them for
        // this context, which holds no path yet; then their paths, from the naming of
        // files, which the compose template reads.
        let mut context = context(&self.output, regions, meta, config, Vec::new());
        let artifacts = match engine.artifacts(template, &context) {
            Ok(artifacts) => artifacts,
            Err(detail) => return self.fail(out, Some(name), detail),
        };
        let mut paths = Vec::new();
        if !artifacts.is_empty() {
            let Some(namer) = cx.services_all().service_named::<dyn ArtifactNamer>(NAMER_SLOT) else {
                let names: Vec<&str> = artifacts.iter().map(TemplateArtifact::name).collect();
                return self.fail(
                    out,
                    Some(name),
                    format!(
                        "the compose template ships the files `{}`, and naming them needs the program's naming \
                         of files (the service slot `{NAMER_SLOT}`), which no package in use provides",
                        names.join("`, `")
                    ),
                );
            };
            let scope = NamingScope::of(cx.registry(), cx.plan().demands());
            for artifact in &artifacts {
                let id = artifact_id(artifact);
                let named = match artifact {
                    TemplateArtifact::File { media_type, extension, .. } => ArtifactRef {
                        id: &id,
                        entry: false,
                        media_type,
                        extension: Some(extension),
                        output: None,
                        folder: false,
                    },
                    TemplateArtifact::Folder { .. } => {
                        ArtifactRef { id: &id, entry: false, media_type: "", extension: None, output: None, folder: true }
                    }
                };
                paths.push((artifact.name().to_string(), namer.url_of(&named, &scope).as_str().to_string()));
            }
        }
        context.artifacts = paths;
        let mut shipped = Vec::new();
        for artifact in &artifacts {
            let (artifact_name, extension, media_type, apply, bytes) = match artifact {
                TemplateArtifact::File { name, extension, media_type, apply, bytes } => {
                    (name, extension, media_type, *apply, bytes)
                }
                TemplateArtifact::Folder { files, .. } => {
                    let files = files.iter().map(|(path, bytes)| (RelPath::new(path.as_str()), bytes.clone())).collect();
                    shipped.push(Artifact::folder(artifact_id(artifact), files));
                    continue;
                }
            };
            let bytes = if apply {
                let Ok(text) = std::str::from_utf8(bytes) else {
                    let detail = format!(
                        "in the artifact `{artifact_name}`: its content must be text, since it is applied as a template"
                    );
                    return self.fail(out, Some(name), detail);
                };
                let mut applied = context.clone();
                applied.html = media_type == "text/html";
                match engine.render_text(template, text, &applied) {
                    Ok(text) => text.into_bytes(),
                    Err(detail) => {
                        return self.fail(out, Some(name), format!("in the artifact `{artifact_name}`: {detail}"));
                    }
                }
            } else {
                bytes.clone()
            };
            shipped.push(Artifact::bytes(artifact_id(artifact), media_type.clone(), bytes).with_extension(extension.clone()));
        }
        let text = match engine.render(template, &context) {
            Ok(text) => text,
            Err(detail) => return self.fail(out, Some(name), detail),
        };
        let composed = if is_html(&self.output) {
            let empty = context_with_regions_left_out(regions, &context);
            match engine.render(template, &empty) {
                Ok(skeleton) => Some(composed_regions(regions, skeleton)),
                Err(detail) => return self.fail(out, Some(name), detail),
            }
        } else {
            None
        };
        let mut entry =
            Artifact::bytes(regions.entry.clone(), regions.media_type.clone(), text.into_bytes())
                .as_entry();
        entry.refs.extend(regions.refs.iter().cloned());
        entry.refs.extend(shipped.iter().map(|artifact| artifact.id.clone()));
        let set = match out.table(&self.document_table()) {
            Ok(set) => set,
            Err(error) => return self.fail(out, Some(name), error),
        };
        let reports = match set.insert_entry_with(entry, &beside) {
            Ok(reports) => reports,
            Err(violation) => return self.fail(out, Some(name), violation),
        };
        let refused: Vec<_> = shipped.into_iter().filter_map(|artifact| set.insert(artifact).err()).collect();
        reports.into_iter().for_each(|report| out.diags.push(report));
        for violation in refused {
            self.fail(out, Some(name), violation);
        }
        if let Some(composed) = composed {
            match out.table(HTML_REGIONS) {
                Ok(table) => *table = composed,
                Err(error) => self.fail(out, Some(name), error),
            }
        }
    }
}

/// Returns the metadata as a compose template reads it: see the [module
/// documentation](self#what-a-compose-template-reads).
fn meta_value(meta: &DocMeta) -> Value {
    let mut map = Map::new();
    if let Some(title) = &meta.title {
        map.insert(String::from("title"), Value::String(title.text.clone()));
    }
    if !meta.authors.is_empty() {
        map.insert(
            String::from("author"),
            Value::Array(meta.authors.iter().map(|author| Value::String(author.text.clone())).collect()),
        );
    }
    if let Some(date) = &meta.date {
        map.insert(String::from("date"), Value::String(date.text.clone()));
    }
    for (key, value) in &meta.other {
        map.insert(key.clone(), value.clone());
    }
    Value::Object(map)
}

/// Returns the text of `region` as a compose template places it: on HTML, the content and
/// each piece inside the element that carries its name.
fn placed(output: &RegionsOutput, region: &Region) -> String {
    let name = region.name.as_str();
    let text = region.text();
    if is_html(output) && composed_kind(region) == Some(ComposedKind::Html) {
        region_element(name, &text)
    } else {
        text
    }
}

/// Returns the identity of the file `artifact` that a compose template ships,
/// `compose:<name>`.
fn artifact_id(artifact: &TemplateArtifact) -> ArtifactId {
    ArtifactId::new(FEATURE, artifact.name())
}

/// Returns what a compose template reads, from the output's regions, the metadata, the
/// compose template's settings and the paths of the files it ships.
fn context(
    output: &RegionsOutput,
    regions: &Regions,
    meta: Value,
    config: Map<String, Value>,
    artifacts: Vec<(String, String)>,
) -> TemplateContext {
    let mut values = Vec::new();
    let mut pieces = Vec::new();
    for region in &regions.regions {
        let text = placed(output, region);
        if let Some(place) = region.piece {
            pieces.push(PieceValue {
                name: region.name.to_string(),
                place: String::from(place.as_str()),
                text: text.clone(),
                words: region.words.clone().unwrap_or_default(),
            });
        }
        values.push((region.name.to_string(), text));
    }
    TemplateContext { html: is_html(output), regions: values, pieces, meta, config, artifacts }
}

/// Returns how the composer's table on HTML holds `region`: the content and each piece as
/// HTML, the words of the page's title as words, and every other region not at all (it is
/// then part of the rest of the page, as each piece's words are). The regions in the table
/// and the rest of the page together make up the page, so this one rule decides both
/// ([`composed_regions`], [`context_with_regions_left_out`]).
fn composed_kind(region: &Region) -> Option<ComposedKind> {
    if region.piece.is_some() || region.name.as_str() == region::CONTENT {
        Some(ComposedKind::Html)
    } else if region.name.as_str() == region::TITLE {
        Some(ComposedKind::Words)
    } else {
        None
    }
}

/// Returns `context` with exactly the regions of the composer's table left out
/// ([`composed_kind`]): the content and each piece placed in its element with nothing
/// inside, the title's words empty, and every other region (the stylesheets, the scripts,
/// the language) and each piece's words as in the page. Applying the compose template to
/// it gives the rest of the page.
fn context_with_regions_left_out(regions: &Regions, context: &TemplateContext) -> TemplateContext {
    let mut empty = context.clone();
    for (name, text) in &mut empty.regions {
        let Some(region) = regions.get(name) else { continue };
        match composed_kind(region) {
            Some(ComposedKind::Html) => *text = region_element(name, ""),
            Some(ComposedKind::Words) => text.clear(),
            None => {}
        }
    }
    for piece in &mut empty.pieces {
        piece.text = region_element(&piece.name, "");
    }
    empty
}

/// Returns the composer's table on HTML, from the output's regions and the rest of the page
/// `skeleton`.
fn composed_regions(regions: &Regions, skeleton: String) -> ComposedRegions {
    let mut composed = Vec::new();
    for region in &regions.regions {
        let Some(kind) = composed_kind(region) else { continue };
        let text = match kind {
            ComposedKind::Html => region.text(),
            ComposedKind::Words => region.words.clone().unwrap_or_default(),
        };
        let name = region.name.as_str();
        composed.push(ComposedRegion { id: region_id(name), name: String::from(name), kind, text });
    }
    ComposedRegions { skeleton, regions: composed }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    fn hash_of(options: &ComposeOptions) -> u64 {
        let mut hasher = DefaultHasher::new();
        options.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn two_equal_options_hash_equal_whatever_the_order_of_their_entries() {
        let with = |entries: &[(&str, Value)]| {
            let mut config = Map::new();
            for (key, value) in entries {
                config.insert(String::from(*key), value.clone());
            }
            let mut options = ComposeOptions::default();
            options.template_config.entry(String::from("html")).or_default().insert(String::from("simple"), config);
            options
        };
        let inner = |a: i64, b: i64| {
            let mut map = Map::new();
            map.insert(String::from("a"), Value::from(a));
            map.insert(String::from("b"), Value::from(b));
            map
        };
        let mut reversed = Map::new();
        reversed.insert(String::from("b"), Value::from(2));
        reversed.insert(String::from("a"), Value::from(1));
        let first = with(&[("x", Value::from("one")), ("y", Value::Object(inner(1, 2)))]);
        let second = with(&[("y", Value::Object(reversed)), ("x", Value::from("one"))]);
        assert_eq!(first, second);
        assert_eq!(hash_of(&first), hash_of(&second));
        let other = with(&[("x", Value::from("one")), ("y", Value::Object(inner(1, 3)))]);
        assert_ne!(hash_of(&first), hash_of(&other));
    }
}
