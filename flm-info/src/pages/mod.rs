//! The pages of documentation of an environment: a tree of pages, each holding one view of
//! the description, and the generator of each page's source in FLM.
//!
//! A [`Tree`] is made from an [`Environment`] and, optionally, the information a program
//! gives about itself ([`ProgramInfo`]). Each [`Page`] has a *path* (`features/headings`,
//! `features/headings/macros/section`), a title, a summary, a *body* (the [view](crate::env)
//! the page shows, a [`Node`]) and the list of the pages directly below it (its
//! *children*). [`Tree::page`] gives the page of a path, [`Tree::lookup`] the pages of the
//! entities and commands of a given name, and [`Tree::walk`] every page in tree order.
//! [`Page::to_json`] writes a page as JSON (the [rules of the JSON form](crate::describe)
//! apply), and [`Output`] is the document that holds several pages behind a header.
//! [`source`] writes a page as an FLM document, which the documentation markup of the
//! extension `flmdoc` renders; this module does not render pages and does not depend on
//! that extension.
//!
//! # The documentation texts a page embeds
//!
//! A page's source embeds documentation texts as they are, since they are FLM: the texts of
//! the features and of what they declare, whole or as their summaries. [`texts`] lists the
//! texts a page embeds, each with whose text it is ([`TextSource`]): the feature that
//! declares it, with its package, or, for a text written as data (the documentation text of
//! a workflow given as data, that of a user definition that no other entry of the settings
//! shares), where it was written, as the settings cascade gives it (cargo feature
//! `flm-main`). The list comes from the walk that writes the source, so it holds exactly the
//! texts the source embeds. A program that renders the pages parses each text with the
//! registry that renders them; a text that does not parse (one an extension wrote, for
//! example) goes into [`SourceOptions::plain_texts`], and the page then writes it as plain
//! text, so that the page still renders.
//!
//! # The paths
//!
//! | path | the page |
//! |---|---|
//! | `/` | the environment at a glance: the profile and the math processor in view (cargo feature `flm-main`), the packages, the features, the workflows |
//! | `packages/`, `packages/<name>` | the packages; one package, with where it comes from (cargo feature `flm-main`) |
//! | `features/`, `features/<name>` | the features; one feature, with its documentation, its element types and the commands written for each, its scoped packages, its settings keys and what else it declares |
//! | `features/<name>/macros`, `…/environments`, `…/specials` | the commands of one kind that the feature declares |
//! | `features/<name>/macros/<macro>`, `…/environments/<environment>`, `…/specials/<trigger>` | one command: every declaration of the name by the feature, with its signature, its arguments, its tack-on fields, its element |
//! | `features/<name>/scopedpackages`, `…/scopedpackages/<package>` | the scoped packages of the feature; one scoped package, with where it is available |
//! | `…/scopedpackages/<package>/macros`, `…/macros/<macro>` (and `environments`, `specials`) | the commands of a scoped package, as for a feature |
//! | `settings/`, `settings/<feature>`, `settings/parsing`, `settings/front-matter` | the settings keys of a feature; the parsing settings; the keys of a front matter (cargo feature `flm-main`) |
//! | `workflows/`, `workflows/<name>` | the workflows; one workflow, with the plan in view when it was built from this workflow |
//! | `namespaces/`, `namespaces/<namespace>`, `namespaces/<namespace>/<name>` | the families of names the registry holds; every name of one family; one entity |
//! | `program/`, `program/paths`, `program/extensions` | the program's commands and environment variables; the folders it reads; the packages and extensions a document may use (only with a [`ProgramInfo`]) |
//!
//! A macro's segment is its name without the backslash; a special character sequence's
//! segment is its trigger as written; a scoped package's and an entity's segments are their
//! dotted names. The index pages of a section (`packages/`, `features/`, `settings/`,
//! `workflows/`, `namespaces/`) always exist; the index page of the commands of one kind
//! (`features/<name>/macros`, and likewise in a scoped package) and that of a feature's
//! scoped packages (`features/<name>/scopedpackages`) exist only when they list at least one
//! page. The entities of five namespaces have their page elsewhere,
//! and `namespaces/<namespace>/<name>` is no page for them: a feature's page is
//! `features/<name>`, a package's `packages/<name>`, a workflow's `workflows/<name>`, a
//! scoped package's `features/<owner>/scopedpackages/<name>`, and a settings key's the page
//! of its feature's settings, `settings/<feature>`.
//!
//! A path is matched exactly, except that the index page of a section may also be given
//! without its final slash (`features` for `features/`). A name that is no page's path is
//! looked up with [`Tree::lookup`].
//!
//! The paths `settings/parsing` and `settings/front-matter` are those of the parsing
//! settings and of the front matter's keys, whatever the features are called: the settings
//! page of a feature named `parsing` or `front-matter` has no path of its own, and the
//! lookup of one of its keys gives the fixed page. No standard feature has either name.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use flm_core::core::plan::{Overrides, Plan};
//! use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
//! use flm_core::stdcore::{self, StdcoreConfig};
//! use flm_info::Environment;
//! use flm_info::pages::{SourceOptions, Tree, source};
//!
//! let mut builder = RegistryBuilder::new();
//! builder.use_package(stdcore::package(StdcoreConfig::new()));
//! builder.use_package(flm_core::math_passthrough::package());
//! let registry = builder.build(&RegistryInputs::new())?;
//! let plan = Plan::for_workflow(&registry, "text", &Overrides::none())?;
//! let factories = vec![stdcore::factory()?, flm_core::math_passthrough::factory()?];
//! let env = Environment::new(registry, Some(Arc::new(plan)), factories);
//!
//! let tree = Tree::new(&env, None);
//! let page = tree.page("features/headings/macros/section").expect("the page of `\\section`");
//! assert_eq!(page.title, "Macro \\section");
//! assert!(source(&page, &SourceOptions::default()).contains("\\docsignature"));
//! // `\section` is no path: the lookup finds the page.
//! assert!(tree.page("\\section").is_none());
//! assert_eq!(tree.lookup("\\section")[0].path, page.path);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

// The page tree: commands prefixed by kind under their feature, scoped packages likewise,
// `namespaces/` for the families of names. See decision rationale at
// [§flm-dd-dr:info-page-tree].

mod build;
mod paths;
mod source;
mod texts;

use std::sync::Arc;

use flm_core::lang::mint::CallableKind;
use serde::Serialize;
use serde_json::Value;

pub use crate::describe::Meta;
use crate::env::{
    CommandView, EntityView, Environment, FeatureView, NamespaceView, PackageView, ParsingView,
    ScopedPackageView, SettingsView, WorkflowView, is_empty, is_false,
};
pub use source::{SourceOptions, source};
#[cfg(feature = "flm-main")]
pub use texts::TextHolder;
pub use texts::{EmbeddedText, TextSource, texts};

/// The kind of document [`Output`] is, as its header ([`Meta::kind`]) gives it.
pub const OUTPUT_KIND: &str = "info";

/// The version of the format of [`Output`], as its header ([`Meta::format`]) gives it.
pub const OUTPUT_FORMAT: u32 = 1;

/// The tree of the pages of documentation of an environment.
///
/// A tree holds a copy of the environment it was made from; each page is computed when it
/// is asked for ([`page`](Tree::page), [`lookup`](Tree::lookup), [`walk`](Tree::walk)).
/// Cloning a tree is cheap: the clones share the environment.
#[derive(Clone)]
pub struct Tree {
    inner: Arc<TreeInner>,
}

struct TreeInner {
    env: Environment,
    program: Option<ProgramInfo>,
}

impl Tree {
    /// Makes the tree of the pages of `env`. With `program`, the information a program
    /// gives about itself, the tree has the section `program/`; without it, the section is
    /// absent.
    pub fn new(env: &Environment, program: Option<ProgramInfo>) -> Tree {
        Tree { inner: Arc::new(TreeInner { env: env.clone(), program }) }
    }

    /// The environment the tree describes.
    pub fn environment(&self) -> &Environment {
        &self.inner.env
    }

    /// The information about the program, when the tree was made with it.
    pub fn program(&self) -> Option<&ProgramInfo> {
        self.inner.program.as_ref()
    }

    /// The page whose path is `path`, or `None` when no page has that path (see the [module
    /// documentation](self) for the paths). The index page of a section may also be given
    /// without its final slash.
    pub fn page(&self, path: &str) -> Option<Page> {
        let location = paths::parse(path)?;
        build::page(self, &location)
    }

    /// The pages of every entity of any namespace whose name is `name`, and of every command
    /// written as `name` (with or without the backslash, as
    /// [`Environment::lookup`] finds commands), each once, in tree order. Empty when nothing
    /// has that name.
    pub fn lookup(&self, name: &str) -> Vec<Page> {
        build::lookup(self, name)
    }

    /// Every page of the tree, each once, in tree order: a page, then the pages below it,
    /// in the order of its [`children`](Page::children), starting from `/`.
    pub fn walk(&self) -> Walk {
        Walk { tree: self.clone(), pending: vec![String::from(paths::ROOT)] }
    }
}

/// The iterator over every page of a tree, in tree order, that [`Tree::walk`] returns.
pub struct Walk {
    tree: Tree,
    pending: Vec<String>,
}

impl Iterator for Walk {
    type Item = Page;

    fn next(&mut self) -> Option<Page> {
        while let Some(path) = self.pending.pop() {
            let Some(page) = self.tree.page(&path) else { continue };
            self.pending.extend(page.children.iter().rev().map(|entry| entry.path.clone()));
            return Some(page);
        }
        None
    }
}

/// One page of documentation: its path, its kind, its title and summary, the view it shows
/// and the pages directly below it.
///
/// A page serializes to JSON ([`Page::to_json`]) with its fields as keys; the body is
/// absent for an index page, whose content is its list of children.
#[derive(Clone, Serialize, schemars::JsonSchema)]
pub struct Page {
    /// The page's path (see the [module documentation](self)).
    pub path: String,
    /// What the page shows.
    pub kind: PageKind,
    /// The page's title, as plain text: `Feature headings`, `Macro \section`.
    pub title: String,
    /// A short text that says what the page shows, in FLM with the backtick, as the
    /// summaries of documentation texts are written; for the page of a declaration, the
    /// summary of its documentation text.
    #[serde(skip_serializing_if = "is_empty")]
    pub summary: String,
    /// The view the page shows; [`Node::Index`] for an index page.
    #[serde(skip_serializing_if = "Node::is_index")]
    pub body: Node,
    /// The pages directly below this one, in order. [`Page::to_json`] with `recursive`
    /// writes each as a full page.
    #[schemars(with = "Vec<Child>")]
    pub children: Vec<Entry>,
    #[serde(skip)]
    #[schemars(skip)]
    tree: Tree,
}

impl std::fmt::Debug for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Page")
            .field("path", &self.path)
            .field("kind", &self.kind)
            .field("title", &self.title)
            .field("summary", &self.summary)
            .field("body", &self.body)
            .field("children", &self.children)
            .finish()
    }
}

impl Page {
    /// The page as JSON: an object with the keys `path`, `kind`, `title`, `summary` (absent
    /// when empty), `body` (absent for an index page) and `children`. Without `recursive`,
    /// each child is an object with its `path`, `title` and `summary`; with `recursive`,
    /// each child is the full page, written the same way, so that the result holds every
    /// page below this one.
    pub fn to_json(&self, recursive: bool) -> Value {
        let mut value = serde_json::to_value(self).unwrap_or(Value::Null);
        if recursive {
            let children: Vec<Value> = self
                .children
                .iter()
                .filter_map(|entry| self.tree.page(&entry.path))
                .map(|page| page.to_json(true))
                .collect();
            if let Value::Object(map) = &mut value {
                map.insert(String::from("children"), Value::Array(children));
            }
        }
        value
    }
}

/// The JSON form of a child of a page: an [`Entry`], or the full page when the page is
/// written with its children in full. For the JSON Schema of a page only.
#[derive(schemars::JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)]
enum Child {
    /// The child's path, title and summary.
    Entry(Entry),
    /// The child in full.
    Page(Box<Page>),
}

/// A reference to a page, as a list of children holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Entry {
    /// The page's path.
    pub path: String,
    /// The page's title, as plain text.
    pub title: String,
    /// The page's summary, in FLM with the backtick.
    #[serde(skip_serializing_if = "is_empty")]
    pub summary: String,
    /// Whose text the summary is, which the page's source records when it embeds the
    /// summary ([`texts`]).
    #[serde(skip)]
    #[schemars(skip)]
    source: TextSource,
}

/// What a page shows. Each kind of page has one kind of body ([`Node`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PageKind {
    /// `/`: the environment at a glance; body [`Node::Root`].
    Root,
    /// `packages/`: the packages; an index page.
    PackageIndex,
    /// `packages/<name>`: one package; body [`Node::Package`].
    Package,
    /// `features/`: the features; an index page.
    FeatureIndex,
    /// `features/<name>`: one feature; body [`Node::Feature`].
    Feature,
    /// `features/<name>/macros` and the other kinds, also inside a scoped package: the
    /// commands of one kind; an index page.
    CommandIndex,
    /// The page of one command; body [`Node::Command`].
    Command,
    /// `features/<name>/scopedpackages`: the scoped packages of a feature; an index page.
    ScopedPackageIndex,
    /// `features/<name>/scopedpackages/<package>`: one scoped package; body
    /// [`Node::ScopedPackage`].
    ScopedPackage,
    /// `settings/`: the pages of settings; an index page.
    SettingsIndex,
    /// `settings/<feature>`: the settings of one feature; body [`Node::Settings`].
    Settings,
    /// `settings/parsing`: the parsing settings; body [`Node::Parsing`].
    ParsingSettings,
    /// `settings/front-matter`: the keys of a front matter (cargo feature `flm-main`); body
    /// `Node::FrontMatter`.
    FrontMatter,
    /// `workflows/`: the workflows; an index page.
    WorkflowIndex,
    /// `workflows/<name>`: one workflow; body [`Node::Workflow`].
    Workflow,
    /// `namespaces/`: the families of names; an index page, whose children's summaries say
    /// what each family's names are.
    NamespaceIndex,
    /// `namespaces/<namespace>`: every name of one family; body [`Node::Namespace`].
    Namespace,
    /// `namespaces/<namespace>/<name>`: one entity; body [`Node::Entity`].
    Entity,
    /// `program/`: the program's commands and environment variables; body
    /// [`Node::Program`].
    Program,
    /// `program/paths`: the folders the program reads; body [`Node::ProgramPaths`].
    ProgramPaths,
    /// `program/extensions`: the packages and extensions a document may use; body
    /// [`Node::ProgramExtensions`].
    ProgramExtensions,
}

/// The body of a page: the view it shows, written as that view's JSON form.
// A page holds one node and pages are made one at a time, so the size of the largest
// variant costs nothing; boxing it would only complicate the variants.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum Node {
    /// The body of an index page, which shows nothing beyond its list of children; absent
    /// from the page's JSON form.
    Index,
    /// The body of `/`.
    Root(RootView),
    /// The body of a package's page.
    Package(PackageView),
    /// The body of a feature's page.
    Feature(FeatureView),
    /// The body of a command's page.
    Command(CommandPageView),
    /// The body of a scoped package's page.
    ScopedPackage(ScopedPackageView),
    /// The body of a feature's settings page.
    Settings(SettingsView),
    /// The body of `settings/parsing`.
    Parsing(ParsingView),
    /// The body of `settings/front-matter` (cargo feature `flm-main`).
    #[cfg(feature = "flm-main")]
    FrontMatter(crate::host::FrontMatterView),
    /// The body of a workflow's page.
    Workflow(WorkflowView),
    /// The body of a namespace's page.
    Namespace(NamespaceView),
    /// The body of an entity's page.
    Entity(EntityView),
    /// The body of `program/`.
    Program(ProgramView),
    /// The body of `program/paths`.
    ProgramPaths(ProgramPaths),
    /// The body of `program/extensions`.
    ProgramExtensions(ProgramExtensions),
}

impl Node {
    /// Whether this is the body of an index page ([`Node::Index`]).
    pub fn is_index(&self) -> bool {
        matches!(self, Node::Index)
    }
}

/// The environment at a glance: the body of `/`.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct RootView {
    /// The workflow the plan in view was built from; absent without a plan in view, or for
    /// a plan built from a request made by hand.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
    /// The profile in view; present for an environment made from a prepared document
    /// (cargo feature `flm-main`).
    #[cfg(feature = "flm-main")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<crate::host::ProfileView>,
    /// The math processor in use; present for an environment made from a prepared document
    /// when a package processes the document's mathematics (cargo feature `flm-main`).
    #[cfg(feature = "flm-main")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub math: Option<crate::host::MathProcessorView>,
    /// The packages, as [`Environment::packages`] lists them.
    pub packages: Vec<PackageView>,
    /// The features, in the registry's order.
    pub features: Vec<FeatureEntryView>,
    /// The workflows, in name order.
    pub workflows: Vec<WorkflowEntryView>,
}

/// One feature, as `/` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct FeatureEntryView {
    /// The feature's name.
    pub name: String,
    /// Its title.
    pub title: String,
    /// The summary of its documentation text.
    #[serde(skip_serializing_if = "is_empty")]
    pub summary: String,
}

/// One workflow, as `/` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct WorkflowEntryView {
    /// The workflow's name, which is also the name of a profile.
    pub name: String,
    /// The outputs it produces, those it inherits included.
    pub outputs: Vec<String>,
    /// The summary of its documentation text.
    #[serde(skip_serializing_if = "is_empty")]
    pub summary: String,
}

/// The body of a command's page: every declaration of one command by one feature.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct CommandPageView {
    /// The command's kind.
    pub kind: CallableKind,
    /// The name the page is about, without the backslash; for specials, the trigger.
    pub name: String,
    /// The feature that declares the command.
    pub feature: String,
    /// The scoped package that defines it, for a command defined only inside certain
    /// environments or arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub within: Option<String>,
    /// Every declaration of the name by the feature, in the order declared: one, or one per
    /// set of parsing modes when the feature declares the name in several.
    pub declarations: Vec<CommandView>,
}

/// What a program says about itself, for the section `program/` of a [`Tree`].
///
/// A program fills it: the `flm` program walks its command-line definition for the
/// commands and their options, and states the environment variables it reads and the
/// folders it uses.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, schemars::JsonSchema)]
pub struct ProgramInfo {
    /// The program's version.
    pub version: String,
    /// Its commands, in the order the program lists them.
    pub commands: Vec<ProgramCommand>,
    /// The environment variables it reads.
    pub environment_variables: Vec<EnvironmentVariable>,
    /// The folders it reads.
    pub paths: ProgramPaths,
    /// The packages and extensions a document may use.
    pub extensions: ProgramExtensions,
}

/// One command of a program.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, schemars::JsonSchema)]
pub struct ProgramCommand {
    /// The command's name, as typed after the program's name.
    pub name: String,
    /// What the command does, as plain text.
    #[serde(skip_serializing_if = "is_empty")]
    pub help: String,
    /// Its options and positional arguments, in the order the program lists them.
    pub options: Vec<ProgramOption>,
}

/// One option or positional argument of a program's command.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, schemars::JsonSchema)]
pub struct ProgramOption {
    /// The option's spellings (`-r`, `--recursive`); empty for a positional argument.
    pub names: Vec<String>,
    /// The name of the value it takes (`PATH`), when it takes one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_name: Option<String>,
    /// What it does, as plain text.
    #[serde(skip_serializing_if = "is_empty")]
    pub help: String,
    /// Its default value, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// One environment variable a program reads.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, schemars::JsonSchema)]
pub struct EnvironmentVariable {
    /// The variable's name.
    pub name: String,
    /// What the program reads it for, as plain text.
    #[serde(skip_serializing_if = "is_empty")]
    pub help: String,
}

/// The folders a program reads: the body of `program/paths`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, schemars::JsonSchema)]
pub struct ProgramPaths {
    /// The folders in which an extension is looked for, in the order searched.
    pub search_path: Vec<ProgramFolder>,
    /// The folder of the citation cache, when the program has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_folder: Option<ProgramFolder>,
    /// The folders in which an image's file is looked for, when the program knows them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_folders: Option<ImageFolders>,
}

/// One folder a program reads.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, schemars::JsonSchema)]
pub struct ProgramFolder {
    /// The folder, as the program resolves it.
    pub path: String,
    /// Where the folder comes from, as plain text (an option, an environment variable, the
    /// document's folder, …).
    #[serde(skip_serializing_if = "is_empty")]
    pub origin: String,
    /// Whether the folder exists.
    #[serde(skip_serializing_if = "is_false")]
    pub exists: bool,
}

/// The folders in which an image's file is looked for, with the rule that gives each
/// folder's meaning.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, schemars::JsonSchema)]
pub struct ImageFolders {
    /// The folders, as written.
    pub folders: Vec<String>,
    /// How each folder is read, as plain text (relative to which folder, for example).
    pub rule: String,
}

/// The packages and extensions a document may use, in three groups: the body of
/// `program/extensions`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, schemars::JsonSchema)]
pub struct ProgramExtensions {
    /// The packages compiled into the program, in name order.
    pub compiled: Vec<String>,
    /// The extensions found in the folders of the search path, by their locators.
    pub found: Vec<String>,
    /// The extensions bundled into the program, by their stems.
    pub bundled: Vec<String>,
}

/// The program's commands and environment variables: the body of `program/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ProgramView {
    /// The program's version.
    pub version: String,
    /// Its commands.
    pub commands: Vec<ProgramCommand>,
    /// The environment variables it reads.
    pub environment_variables: Vec<EnvironmentVariable>,
}

/// The document that holds several pages as JSON: a header, then the pages.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct Output {
    /// The header: the kind [`OUTPUT_KIND`], the program's version and the format
    /// [`OUTPUT_FORMAT`].
    pub meta: Meta,
    /// The pages, each as [`Page::to_json`] writes it.
    pub pages: Vec<Value>,
}

impl Output {
    /// The document holding `pages` (each as [`Page::to_json`] writes it) behind the header
    /// of this kind and format.
    pub fn new(pages: Vec<Value>) -> Output {
        Output { meta: Meta::new(OUTPUT_KIND, OUTPUT_FORMAT), pages }
    }
}
