//! The comparison of two builds of the live preview, and the update it gives: what an open
//! page must change to show the second build where it shows the first.
//!
//! After each build, the watch loop (`Watch`, compiled with the cargo feature `watch`)
//! compares the new output with the last one and hands the result to its caller in the
//! event `Event::Built`. The preview's server (`Server::tell`) sends it to every open page,
//! where the page script applies it. The page script is the script that the preview's page
//! carries, declared by the feature [`preview`](crate::stdpkg::preview).
//! Bringing the page up to date in place, instead of loading it anew, keeps the reader's
//! place in the page.
//!
//! **What is compared.** A build's output, as the comparison sees it, is a
//! [`BuildOutput`]: the named files the build handed over and, when the leading output
//! (below) is a page assembled by the compose feature
//! ([`flm_core::stdlib::features::compose`]), that page's table [`HTML_REGIONS`]
//! ([`ComposedRegions`]). That table holds the text of the output regions that the page
//! can bring up to date on its own (an output region is one named part of an output's
//! entry artifact: [`Regions`](flm_core::core::region::Regions)): the document's content
//! and each [piece](flm_core::core::piece::Piece) placed beside it, each as the HTML
//! inside an element with a stable identifier, and the words of the page's title. The
//! table also holds the rest of the page: everything else, including the head, the
//! stylesheets, the scripts, the language, the template's own markup, and each piece's
//! words wherever the template writes them. Since the table's regions and the rest of the
//! page together make up the whole page, two pages that differ anywhere differ in one of
//! them. A build produces every output of its workflow, and the preview shows the
//! *leading output*, the first of the outputs the preparation chose
//! ([`Prepared::outputs`](crate::session::Prepared::outputs)): its entry file is the page,
//! and the files of the other outputs are compared and served as files beside it.
//! [`BuildOutput::of_build`] makes a `BuildOutput` from a build's run, the name of its
//! leading output and the files the build handed over, wherever the build ran.
//!
//! **What the comparison gives.** [`compare`] compares two outputs by exact equality, and
//! does not look for the reason of a difference:
//!
//! - when the rest of the page changed, or a region appeared or disappeared, or one of the
//!   two builds gave a page and the other did not, or there is no earlier output, the page
//!   must be loaded anew ([`PageChange::Reload`]); so must the entry file of an output that
//!   has no table of output regions (plain text, LaTeX, a PDF file, a page assembled without
//!   the compose feature) when it changed, appeared or disappeared, since such a file can
//!   only be loaded anew;
//! - otherwise, the regions whose text (or kind of text) changed, each with its new text
//!   ([`PageChange::Regions`]), or no change ([`PageChange::Same`]);
//! - in every case, the paths of the files beside the page whose bytes changed, or that
//!   appeared or disappeared. The page's own file (the entry file: the file a reader opens
//!   first, [`BuildOutput::entry`]) is not among them: its changes are those of the
//!   regions. An output that is not a page (plain text, LaTeX) has no regions, and its
//!   entry file is one of the compared files.
//!
//! A build that did not hand over its output (it stopped at the preparation, failed as a
//! program, or its output could not be produced) gives [`PageChange::Absent`]: the page
//! stays as it is, and the next build is compared with the last output a build handed
//! over. A build of a profile that declares no output (`check`) hands over the *output
//! without files* ([`BuildOutput::default`]), which has no entry file; it is compared as
//! any output is, and it replaces the last output.
//!
//! **The update.** An [`Update`] holds the result of the comparison and the number of the
//! build. [`Update::to_json`] gives it as the fields of the JSON message the page script
//! reads; the problems of the build and what the page script reads of the document are the
//! [document snapshot](super::snapshot)'s. This module reads and writes no file, and is
//! compiled whether or not the cargo feature `watch` is enabled, so that a host can compare
//! two builds without the watch loop.

use std::collections::BTreeMap;

use flm_core::core::run::Run;
use serde_json::{Value, json};

use crate::output::NamedFile;
use flm_core::stdlib::backends::html::stages::MEDIA_TYPE as HTML_MEDIA_TYPE;
use flm_core::stdlib::features::compose::{ComposedKind, ComposedRegion, ComposedRegions, HTML_REGIONS};

/// The output of one build, as [`compare`] sees it: the compose feature's table of the
/// page, the name of the leading output, and the named files of every output. See the
/// [module documentation](self), "What is compared".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildOutput {
    /// The compose feature's table of the page ([`HTML_REGIONS`]), when the leading
    /// output's entry file is a page that the compose feature assembled; `None` otherwise.
    pub page: Option<ComposedRegions>,
    /// The name of the leading output, the one the preview shows: the first of the outputs
    /// the preparation chose ([`Prepared::outputs`](crate::session::Prepared::outputs));
    /// empty for the output of a profile that declares no output.
    pub leading: String,
    /// The named files of every output the build produced, by path: the entry file of each
    /// output and every file beside them. A file two outputs share is held once.
    pub files: BTreeMap<String, NamedFile>,
}

impl BuildOutput {
    /// Makes the output of a build from its run `run` (the run that
    /// [`crate::session::build`] returns, in [`Built::run`](crate::session::Built::run)),
    /// the name of its leading output `leading`, and the named files `files` it handed
    /// over, as its sink received them.
    ///
    /// The result holds the files by path (the first file of each path, when two outputs
    /// share one), and the compose feature's table [`HTML_REGIONS`] when the run holds it
    /// and the leading output's entry file is an HTML page (media type `text/html`). The
    /// table describes the page the compose feature assembled, so it is kept only when that
    /// page is the one the preview shows: for a workflow whose leading output is not a page
    /// (`[text, html]`), [`page`](Self::page) is `None` and the leading output's entry file
    /// is compared as a file. A run without the compose feature's table (an output that is
    /// not a page, or a page assembled without the compose feature) also gives an output
    /// whose `page` is `None`.
    pub fn of_build(run: &Run, leading: &str, files: impl IntoIterator<Item = NamedFile>) -> BuildOutput {
        let mut by_path: BTreeMap<String, NamedFile> = BTreeMap::new();
        for file in files {
            by_path.entry(file.path.clone()).or_insert(file);
        }
        let mut output = BuildOutput { page: None, leading: leading.to_string(), files: by_path };
        if output.entry().is_some_and(|entry| entry.media_type == HTML_MEDIA_TYPE) {
            output.page = run.table(HTML_REGIONS).ok().flatten().cloned();
        }
        output
    }

    /// Returns the leading output's entry file (the file a reader opens first), if it has
    /// one.
    pub fn entry(&self) -> Option<&NamedFile> {
        self.files.values().find(|file| file.entry && file.output == self.leading)
    }

    /// The files [`compare`] compares: every file, less the leading output's entry file
    /// when that output is a page.
    fn compared(&self) -> impl Iterator<Item = &NamedFile> {
        let entry = self.entry().map(|file| file.path.as_str());
        self.files.values().filter(move |file| self.page.is_none() || Some(file.path.as_str()) != entry)
    }
}

/// What an open page must do to show a new build: see the [module documentation](self),
/// "What the comparison gives".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageChange {
    /// Nothing: no output region changed.
    Same,
    /// Bring these output regions up to date, each with its new text; the rest of the page
    /// stays as it is.
    Regions(Vec<ComposedRegion>),
    /// Load the page anew.
    Reload,
    /// The build did not hand over its output (it stopped at the preparation, failed as a
    /// program, or its output could not be produced): the page stays as it is.
    Absent,
}

/// Compares the output of a build, `now`, with the last output before it, `before` (`None`
/// when there was none), and returns what the page must do and the paths of the files that
/// changed, appeared or disappeared, in path order. `now` is `None` for a build that did not
/// hand over its output, which gives [`PageChange::Absent`] and no paths. With `before` `None` (no earlier
/// output), the page must be loaded anew ([`PageChange::Reload`]). See the [module
/// documentation](self), "What the comparison gives".
pub fn compare(before: Option<&BuildOutput>, now: Option<&BuildOutput>) -> (PageChange, Vec<String>) {
    let Some(now) = now else { return (PageChange::Absent, Vec::new()) };
    let empty = BuildOutput::default();
    let before_files = before.unwrap_or(&empty);
    let before_map: BTreeMap<&str, &NamedFile> =
        before_files.compared().map(|file| (file.path.as_str(), file)).collect();
    let now_map: BTreeMap<&str, &NamedFile> = now.compared().map(|file| (file.path.as_str(), file)).collect();
    let mut files: Vec<String> = now_map
        .iter()
        .filter(|(path, file)| before_map.get(*path).is_none_or(|old| old.bytes != file.bytes || old.media_type != file.media_type))
        .map(|(path, _)| path.to_string())
        .collect();
    files.extend(before_map.keys().filter(|path| !now_map.contains_key(*path)).map(|path| path.to_string()));
    files.sort();
    let page = match (before.map(|output| &output.page), &now.page) {
        (None, _) => PageChange::Reload,
        (Some(None), None) if before_files.entry() != now.entry() => PageChange::Reload,
        (Some(None), None) => PageChange::Same,
        (Some(Some(old)), Some(new)) => compare_pages(old, new),
        (Some(_), _) => PageChange::Reload,
    };
    (page, files)
}

/// The change from the page `old` to the page `new`.
fn compare_pages(old: &ComposedRegions, new: &ComposedRegions) -> PageChange {
    let ids = |page: &ComposedRegions| page.regions.iter().map(|region| region.id.clone()).collect::<Vec<_>>();
    if old.skeleton != new.skeleton || ids(old) != ids(new) {
        return PageChange::Reload;
    }
    let changed: Vec<ComposedRegion> = new
        .regions
        .iter()
        .zip(&old.regions)
        .filter(|(new, old)| new.text != old.text || new.kind != old.kind)
        .map(|(new, _)| new.clone())
        .collect();
    if changed.is_empty() { PageChange::Same } else { PageChange::Regions(changed) }
}

/// What changed from one build to the next: see the [module documentation](self), "The
/// update". Made by [`Update::new`].
#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    /// The number of the build, counted from 1 for the first build.
    pub build: u64,
    /// What the page must do.
    pub page: PageChange,
    /// The paths of the files beside the page that changed, appeared or disappeared.
    pub files: Vec<String>,
}

impl Update {
    /// Makes the update of the build numbered `build`, whose output is `now` (`None` when it
    /// did not hand it over), compared with the last output before it, `before`
    /// ([`compare`]).
    pub fn new(build: u64, before: Option<&BuildOutput>, now: Option<&BuildOutput>) -> Update {
        let (page, files) = compare(before, now);
        Update { build, page, files }
    }

    /// Whether the update asks nothing of a page: the build handed over its output, and no
    /// output region and no file changed. The build's problems may still differ from the
    /// last build's.
    pub fn changes_nothing(&self) -> bool {
        self.page == PageChange::Same && self.files.is_empty()
    }

    /// Returns the update as the fields of the JSON message the page script reads, the
    /// message `update` of the watch client's protocol (the file
    /// `npm/flm-watchclient/PROTOCOL.md` of FLM's repository), that the comparison gives:
    ///
    /// ```json
    /// {
    ///   "build": 3,
    ///   "page": "regions",
    ///   "regions": [{"id": "flm-region-content", "name": "content", "kind": "html", "text": "…"}],
    ///   "files": ["paper.figure.png"]
    /// }
    /// ```
    ///
    /// `page` is `same`, `regions`, `reload` or `absent` ([`PageChange`]); `regions` is empty
    /// unless `page` is `regions`; a region's `kind` is `html` (HTML content of the element
    /// `id`) or `words` (plain words, such as the page's title). The message's other fields,
    /// the problems among them, are added by the server, from the [document
    /// snapshot](super::snapshot) and from its own state.
    pub fn to_json(&self) -> Value {
        let (page, regions) = match &self.page {
            PageChange::Same => ("same", Vec::new()),
            PageChange::Regions(regions) => ("regions", regions.iter().map(region_json).collect()),
            PageChange::Reload => ("reload", Vec::new()),
            PageChange::Absent => ("absent", Vec::new()),
        };
        json!({
            "build": self.build,
            "page": page,
            "regions": regions,
            "files": self.files,
        })
    }
}

/// An output region as JSON.
fn region_json(region: &ComposedRegion) -> Value {
    let kind = match region.kind {
        ComposedKind::Html => "html",
        ComposedKind::Words => "words",
    };
    json!({"id": region.id, "name": region.name, "kind": kind, "text": region.text})
}
