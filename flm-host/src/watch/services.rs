//! The program's services: what only the program that runs the preview knows, which it
//! gives the preview's server when it starts it
//! ([`Server::start`](super::server::Server::start)).
//!
//! The server transports; the knowledge of the help pages belongs to the program that runs
//! the preview (the `flm` program, which owns `flm info`). The program gives it to the
//! server through one trait, [`Services`], each service of which a program may give or
//! leave out. A program that gives none passes [`NoServices`].
//!
//! **The preparation the services answer for.** Each service is asked about the
//! environment of one preparation of the document
//! ([`Prepared`]): the last one that succeeded, which the watch loop handed the server in
//! [`Preparation::Prepared`](super::Preparation::Prepared). The server keeps it, and
//! passes it to each call; a service keeps what it computes from it by the digest of its registry
//! ([`FlmRegistry::digest`](flm_core::core::registry::FlmRegistry::digest)), which changes
//! when the packages, the features or their settings change.
//!
//! **Threads.** The server calls the services on the threads on which it answers
//! requests, never on the watch loop's: a service may take its time to compute what it
//! gives the first time it is asked.
//!
//! **The help pages** ([`Help`]) are the pages of `flm info` for the document's
//! environment: an index of every page with the names it is looked up by, one page as
//! HTML, and the page of a construct written in the document. The paths of the pages are
//! the program's: the server and the browser parts read them, and never make one.
//!
//! **The command line of an export** ([`Services::export_command`]) is the `flm render`
//! command that writes what an export of the preview writes ([`super::export`]), composed
//! by the program from its own options: the server and the browser parts show it, and never
//! compose one.
//!
//! **Opening a file** ([`Services::open_file`]) with the system's default application for
//! it, as `flm render --view` opens the file it wrote: the server asks it for the main file
//! of the last export saved.
//!
//! **The home folder** ([`Services::home_folder`]), in which the server keeps the viewer
//! settings ([`super::viewer`]): where the program finds the user's own folders.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use super::snapshot::Construct;
use crate::session::Prepared;

/// The services that the program running the preview gives the server: see the [module
/// documentation](self). Each service has a default that gives nothing.
pub trait Services: Send + Sync {
    /// The help pages, when the program gives them; `None` otherwise (the default), and the
    /// server then answers the requests of the help pages with `501`.
    fn help(&self) -> Option<&dyn Help> {
        None
    }

    /// The command line of `flm render` that writes the export of the build type `profile`
    /// named `name` (`STEM.EXT`) beside the document, with `--continue-with-errors` when
    /// `continue_with_errors` is set, and the options of the program running the preview
    /// that change the build; `None` (the default) when the program composes none.
    fn export_command(&self, profile: &str, name: &str, continue_with_errors: bool) -> Option<String> {
        let _ = (profile, name, continue_with_errors);
        None
    }

    /// Opens the file at `path`, in the system's form, with the system's default
    /// application for it, and returns once the application has started.
    ///
    /// # Errors
    ///
    /// [`OpenFileError::NotOffered`] (the default) when the program opens no file, and
    /// [`OpenFileError::Failed`] when no application can be started for the file.
    fn open_file(&self, path: &Path) -> Result<(), OpenFileError> {
        let _ = path;
        Err(OpenFileError::NotOffered)
    }

    /// The user's home folder, as the system writes paths, in whose folder `.flm` the server
    /// reads and writes the viewer settings ([`super::viewer`]); `None` (the default) when
    /// there is none, and the viewer settings then live in memory.
    fn home_folder(&self) -> Option<PathBuf> {
        None
    }
}

/// Why a file was not opened ([`Services::open_file`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenFileError {
    /// The program running the preview opens no file.
    NotOffered,
    /// No application could be started for the file: the text says why, in one line.
    Failed(String),
}

impl fmt::Display for OpenFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenFileError::NotOffered => f.write_str("Missing way to open a file in this preview"),
            OpenFileError::Failed(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for OpenFileError {}

/// The program's services when it gives none.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoServices;

impl Services for NoServices {}

/// The help pages of the environment of a preparation: see the [module
/// documentation](self).
pub trait Help: Send + Sync {
    /// The index of the pages of `prepared`'s environment: every page, in the order of the
    /// tree of the pages ([`HelpEntry`]).
    ///
    /// # Errors
    ///
    /// A [`HelpError::Failed`] when the index cannot be made.
    fn index(&self, prepared: &Prepared) -> Result<Arc<[HelpEntry]>, HelpError>;

    /// The page `path` of `prepared`'s environment, its content as HTML.
    ///
    /// # Errors
    ///
    /// [`HelpError::NoPage`], with the nearest paths, when no page has the path `path`;
    /// [`HelpError::Failed`] when the page cannot be rendered.
    fn page(&self, prepared: &Prepared, path: &str) -> Result<HelpPage, HelpError>;

    /// The path of the page of `construct`, a construct written in the document, in
    /// `prepared`'s environment; `None` when it has none (for example a command that no
    /// feature declares).
    fn construct_page(&self, prepared: &Prepared, construct: &Construct) -> Option<String>;
}

/// One page of the index of the help pages ([`Help::index`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpEntry {
    /// The page's path.
    pub path: String,
    /// What the page shows, a name of `flm info`'s kinds of pages (`command`, `settings`,
    /// `entity`, …).
    pub kind: String,
    /// The page's title, as plain text.
    pub title: String,
    /// The page's summary, as plain text; empty when it has none.
    pub summary: String,
    /// The names the page is looked up by: a command with and without its backslash, an
    /// environment's name, each key of a settings page as `FEATURE.KEY`, the name of the
    /// entity a page describes.
    pub names: Vec<String>,
}

impl HelpEntry {
    /// Returns the entry as JSON: `{"path", "kind", "title", "summary", "names"}`.
    pub fn to_json(&self) -> Value {
        json!({
            "path": self.path,
            "kind": self.kind,
            "title": self.title,
            "summary": self.summary,
            "names": self.names,
        })
    }
}

/// One help page, rendered ([`Help::page`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpPage {
    /// The page's path.
    pub path: String,
    /// What the page shows, as [`HelpEntry::kind`] names it.
    pub kind: String,
    /// The page's title, as plain text.
    pub title: String,
    /// The page's content as HTML: an element of HTML's body, without a document around it.
    /// A link to another page is an element `a` whose attribute `data-flm-kind` is
    /// `flmdoc.page` and whose `href` is the page's path followed by `.html`.
    pub content: String,
    /// The feature whose settings the page concerns, when there is one: the tools page then
    /// offers a link to that feature in its *Settings* tab.
    pub settings_feature: Option<String>,
    /// The page's path line: the levels from the page's section down to the page itself,
    /// each a page; empty for the root page `/`. The tools page shows it above the page,
    /// each segment but the last a link to its page.
    pub path_line: Vec<HelpSegment>,
}

/// One level of a help page's path line ([`HelpPage::path_line`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpSegment {
    /// What the level adds to the path above it, as the line shows it (`features`, `refs`,
    /// `\ref`).
    pub text: String,
    /// The path of the page at that level.
    pub path: String,
}

impl HelpPage {
    /// Returns the page as JSON: `{"path", "kind", "title", "content", "settings_feature",
    /// "path_line"}`, `settings_feature` `null` when the page concerns no feature's
    /// settings, and `path_line` a list of `{"text", "path"}`.
    pub fn to_json(&self) -> Value {
        let path_line: Vec<Value> =
            self.path_line.iter().map(|segment| json!({ "text": segment.text, "path": segment.path })).collect();
        json!({
            "path": self.path,
            "kind": self.kind,
            "title": self.title,
            "content": self.content,
            "settings_feature": self.settings_feature,
            "path_line": path_line,
        })
    }
}

/// Why a help service gives nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelpError {
    /// No page has the path asked for.
    NoPage {
        /// The path asked for.
        path: String,
        /// The paths of the pages nearest to it, nearest first, each with the page's title.
        nearest: Vec<(String, String)>,
    },
    /// What was asked for cannot be made: the text says why, in one line.
    Failed(String),
}

impl fmt::Display for HelpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HelpError::NoPage { path, .. } => write!(f, "Missing help page ‘{path}’"),
            HelpError::Failed(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for HelpError {}
