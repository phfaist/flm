//! The document snapshot of the live preview: what the viewer needs of the last processed
//! document, computed from the run of a build on the watch loop's thread.
//!
//! The preview's pages show more of a document than its output: the problems of the last
//! build, the document's headings and labels, and, for a place of a source file, the
//! commands, environments and special character sequences written around it. A
//! [`Snapshot`] holds all of it, so that the server answers its requests from it and never
//! from the run, which stays on the watch loop's thread. It has two parts, which come from
//! two different builds when the last build did not hand over its output:
//!
//! - the **problems** of the last build, each as data ([`Reported`]);
//! - what the run of the build that gave the output served knows of the document
//!   ([`Processed`]): its sources, by name ([`Places`]), which turn a byte offset into a
//!   line and a column; its headings in document order ([`Heading`]); its labels, each
//!   with what it designates, the kind of its reference point and the heading it stands
//!   under ([`LabelEntry`]); and its *constructs* ([`Construct`]).
//!
//! **The constructs.** A construct is a command, an environment or a special character
//! sequence written in a file of the document: a callable node of the document's trees as
//! parsed ([`Run::doc`]) whose span lies in a source with a name. Its byte range is the
//! range of its node in that source, as the HTML backend counts the place it writes into an
//! element (`data-flm-source-span`): for a file with a front matter, in the content below
//! the front matter. A special character sequence counts only when a feature declares it,
//! so that the nodes the library makes itself (a paragraph, a paragraph break) are not
//! constructs. A use of a user definition is a construct like any other, and the content it
//! makes carries the place of the use in the page, so a place in that content answers with
//! the use.
//!
//! **What reads the tables.** The headings come from the nodes whose element declares the
//! aspect [`Sectioning`]: the level, and the title an entry of a table of contents shows
//! ([`Sectioning::toc_title`]). A heading's number comes from the counter table
//! ([`COUNTERS`]) and the identifier of its element from the anchors table ([`ANCHORS`]),
//! both through the heading's first reference point. A label's designation is what a
//! reference to it shows ([`designation`]), from the refs feature's table of targets
//! ([`REF_TARGETS`]) and the counter table; the kind of its reference point comes from the
//! table of reference points ([`REFPOINTS`]). A table that the plan of the build type does
//! not schedule leaves out what it gives, with no error: no number without the counter
//! table, no identifier without the anchors table, no kind without the table of reference
//! points, no label without the table of targets.
//!
//! This module reads and writes no file, and is compiled whether or not the cargo feature
//! `watch` is enabled, so that a host can make the snapshot of a build without the watch
//! loop.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use flm_core::core::run::Run;
use flm_core::lang::mint::{CallableKind, LANGUAGE_OWNER};
use flm_core::lang::text::{plain_text, plain_text_of};
use flm_core::lang::{Flm, FlmCallableType, FlmSource, FlmSpan};
use flm_core::model::addr::NodeAddr;
use flm_core::model::aspects::{PointId, Sectioning};
use flm_core::model::element::element_of;
use flm_core::stdlib::features::numbering::{COUNTERS, FormatOptions};
use flm_core::stdlib::features::refpoints::REFPOINTS;
use flm_core::stdlib::features::refs::{ANCHORS, REF_TARGETS, designation};
use flm_core::techy::core::node::NodeRef;
use serde_json::{Map, Value, json};

use crate::report::Reported;

/// What the viewer needs of the last processed document: see the [module
/// documentation](self). The default holds no problem and no processed document, as before
/// the first build.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// The problems of the last build, in the order reported.
    pub problems: Vec<Reported>,
    /// What the run of the build that gave the output served knows of the document; `None`
    /// until a build hands over its output. A build that did not hand over its output
    /// keeps the one before.
    pub processed: Option<Arc<Processed>>,
}

impl Snapshot {
    /// Returns the fields of the preview's messages `hello` and `update` that the snapshot
    /// gives, as the watch client's protocol (the file `npm/flm-watchclient/PROTOCOL.md`
    /// of FLM's repository) names them:
    ///
    /// - `problems`: each problem, a [`Reported`] as JSON (its record, then the byte
    ///   offsets `bytes` and the `excerpt` of its line, each left out when not known);
    /// - `references`: maps the identifier of an element to the label that refers to it
    ///   ([`Processed::references`]);
    /// - `outline`: the headings, each `{"level", "number", "title", "id"}`, the number and
    ///   the identifier `null` where the run gives none ([`Heading`]);
    /// - `labels`: the labels, each `{"label", "designation", "id", "kind", "heading"}`,
    ///   the identifier, the kind and the heading `null` where the run gives none
    ///   ([`LabelEntry`]).
    ///
    /// The last three are `null` while no build has handed over its output.
    pub fn fields(&self) -> Map<String, Value> {
        let processed = self.processed.as_deref();
        let mut fields = Map::new();
        fields.insert(String::from("problems"), Value::Array(self.problems.iter().map(problem_json).collect()));
        fields.insert(String::from("references"), json!(processed.map(Processed::references)));
        fields.insert(String::from("outline"), json!(processed.map(|processed| processed.headings.iter().map(Heading::to_json).collect::<Vec<_>>())));
        fields.insert(String::from("labels"), json!(processed.map(|processed| processed.labels.iter().map(LabelEntry::to_json).collect::<Vec<_>>())));
        fields
    }
}

/// A problem as JSON.
fn problem_json(problem: &Reported) -> Value {
    // A record is JSON whatever its data: data with no JSON form is recorded as `null`.
    serde_json::to_value(problem).unwrap_or(Value::Null)
}

/// What the run of one build knows of the document, for the viewer: see the [module
/// documentation](self). Made with [`Processed::of_run`].
#[derive(Debug, Clone, Default)]
pub struct Processed {
    places: Places,
    headings: Vec<Heading>,
    labels: Vec<LabelEntry>,
    constructs: BTreeMap<String, Vec<Construct>>,
}

/// A heading of the document, as the outline lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// The heading's level: the smaller the number, the outer the heading
    /// ([`Sectioning::level`]).
    pub level: u32,
    /// The heading's number, as plain text, without the word before it and the delimiters
    /// around it (`2.1`), as a table of contents writes it; `None` for a heading the
    /// document did not number, and where the run has no counter table.
    pub number: Option<String>,
    /// The title that a table of contents shows for the heading, as plain text.
    pub title: String,
    /// The identifier of the heading's element; `None` where its reference point received
    /// no anchor, and where the run has no anchors table.
    pub id: Option<String>,
}

impl Heading {
    fn to_json(&self) -> Value {
        json!({"level": self.level, "number": self.number, "title": self.title, "id": self.id})
    }
}

/// A label of the document and what it designates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelEntry {
    /// The label, as written (`thm:big`).
    pub label: String,
    /// What a reference to the label shows, as plain text (`Thm. 1`, `Introduction`):
    /// [`designation`]; the label itself when the designation cannot be written (a
    /// [`FormatError`](flm_core::stdlib::features::numbering::FormatError) of its number).
    pub designation: String,
    /// The identifier of the element the label refers to; `None` where the run has no
    /// anchors table.
    pub id: Option<String>,
    /// The kind of the reference point the label refers to (`section`, `equation`,
    /// `defterm`: [`RefPoint::kind`](flm_core::model::aspects::RefPoint::kind), an open
    /// set); `None` where the run has no table of reference points.
    pub kind: Option<String>,
    /// The heading the label stands under: the position, in [`Processed::headings`], of the
    /// last heading at or before the label's reference point, so that a heading's own label
    /// stands under that heading; `None` for a label before the first heading.
    pub heading: Option<usize>,
}

impl LabelEntry {
    fn to_json(&self) -> Value {
        json!({
            "label": self.label,
            "designation": self.designation,
            "id": self.id,
            "kind": self.kind,
            "heading": self.heading,
        })
    }
}

/// A command, an environment or a special character sequence written in a file of the
/// document: see the [module documentation](self), "The constructs".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Construct {
    /// The name of the file, as the page's elements name it (`data-flm-source-span`).
    pub file: String,
    /// The byte offset of the construct's start in the file, counted as in
    /// `data-flm-source-span`.
    pub start: usize,
    /// The byte offset just past the construct's end, counted in the same way.
    pub end: usize,
    /// Whether it is a macro, an environment or a special character sequence.
    pub kind: CallableKind,
    /// The name it is written with, without the escape character (`emph`, `itemize`), or
    /// the characters of a special character sequence (`~`).
    pub name: String,
    /// The feature that declares it; `None` for a command or an environment that no
    /// feature declares.
    pub feature: Option<String>,
}

impl Construct {
    /// Returns the construct as JSON: `{"kind", "name", "file", "start", "end",
    /// "feature"}`, the kind `macro`, `environment` or `specials`, the feature `null` where
    /// none declares it.
    pub fn to_json(&self) -> Value {
        let kind = match self.kind {
            CallableKind::Macro => "macro",
            CallableKind::Environment => "environment",
            CallableKind::Specials => "specials",
        };
        json!({
            "kind": kind,
            "name": self.name,
            "file": self.file,
            "start": self.start,
            "end": self.end,
            "feature": self.feature,
        })
    }
}

/// What the snapshot knows of one place of a file ([`Processed::at`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct At {
    /// The line, counted from 1, as [`Places::line_column`] gives it.
    pub line: usize,
    /// The column, counted from 1 in characters, as [`Places::line_column`] gives it.
    pub column: usize,
    /// The constructs that contain the place, from the innermost outward.
    pub constructs: Vec<Construct>,
}

/// The error of [`Processed::at`]: why the snapshot knows nothing of a place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtError {
    /// No source of the document has the name asked for.
    UnknownFile {
        /// The file, as asked.
        file: String,
    },
    /// The offset is past the end of the source.
    PastEnd {
        /// The file, as asked.
        file: String,
        /// The byte offset, as asked.
        offset: usize,
    },
}

impl fmt::Display for AtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AtError::UnknownFile { file } => write!(f, "Missing source file ‘{file}’ in the document shown"),
            AtError::PastEnd { file, offset } => write!(f, "Byte offset {offset} past the end of ‘{file}’"),
        }
    }
}

impl std::error::Error for AtError {}

impl Processed {
    /// Collects what the run `run` knows of the document: its sources ([`Places::of_run`]),
    /// its headings, its labels and its constructs, as the [module documentation](self)
    /// says.
    pub fn of_run(run: &Run) -> Processed {
        let registry = run.plan().registry();
        let counters = run.table(COUNTERS).ok().flatten();
        let anchors = run.table(ANCHORS).ok().flatten();
        let number_options = FormatOptions::new().without_word().without_delimiters();
        // The place of each node in document order, by which the labels are sorted.
        let mut order: BTreeMap<NodeAddr, usize> = BTreeMap::new();
        let mut headings = Vec::new();
        // The place of each heading in document order, by which a label finds its heading.
        let mut heading_positions = Vec::new();
        // The frozen trees, whose addresses the run's tables name.
        for (position, (addr, node)) in run.walk().enumerate() {
            order.insert(addr, position);
            if let Some(aspect) = element_of(&node).aspect::<dyn Sectioning>() {
                let point = PointId { addr, index: 0 };
                let number = counters
                    .and_then(|counters| counters.format(point, &number_options))
                    .and_then(Result::ok)
                    .map(|segments| segments.iter().map(|segment| plain_text(segment.content())).collect());
                let title = aspect
                    .toc_title(&node)
                    .and_then(|title| title.nodes(&node).map(plain_text_of))
                    .unwrap_or_default();
                let id = anchors.and_then(|anchors| anchors.id(point)).map(String::from);
                headings.push(Heading { level: aspect.level(&node), number, title, id });
                heading_positions.push(position);
            }
        }
        // The trees as parsed, which hold every construct as written, whatever a rewrite
        // made of its node.
        let mut constructs: BTreeMap<String, Vec<Construct>> = BTreeMap::new();
        for (_, node) in run.doc().content().walk_nodes() {
            if let Some(construct) = construct_of(&node, registry) {
                constructs.entry(construct.file.clone()).or_default().push(construct);
            }
        }
        let points = run.table(REFPOINTS).ok().flatten();
        let mut labels: Vec<(usize, u32, LabelEntry)> = Vec::new();
        if let Ok(Some(targets)) = run.table(REF_TARGETS) {
            for (label, target) in &targets.by_label {
                let shown = designation(target, label, counters)
                    .map(|designation| designation.plain_text())
                    .unwrap_or_else(|_| label.to_string());
                let id = anchors.and_then(|anchors| anchors.id(target.point)).map(String::from);
                let kind = points.and_then(|points| points.point(target.point)).map(|record| record.point.kind.to_string());
                let position = order.get(&target.point.addr).copied().unwrap_or(usize::MAX);
                // The headings at or before the label's point; the last of them is its heading.
                let before = heading_positions.partition_point(|heading| *heading <= position);
                let heading = before.checked_sub(1);
                let entry = LabelEntry { label: label.to_string(), designation: shown, id, kind, heading };
                labels.push((position, target.point.index, entry));
            }
        }
        // Document order; the labels of one point keep the order of the table, by label.
        labels.sort_by_key(|(position, index, _)| (*position, *index));
        Processed {
            places: Places::of_run(run),
            headings,
            labels: labels.into_iter().map(|(_, _, entry)| entry).collect(),
            constructs,
        }
    }

    /// Returns the sources of the document, by name.
    pub fn places(&self) -> &Places {
        &self.places
    }

    /// Returns the headings, in document order.
    pub fn headings(&self) -> &[Heading] {
        &self.headings
    }

    /// Returns the labels, in the document order of the points they refer to, the labels of
    /// one point in label order.
    pub fn labels(&self) -> &[LabelEntry] {
        &self.labels
    }

    /// Returns the constructs written in the file named `file`, in document order; none for
    /// a name that no source has.
    pub fn constructs(&self, file: &str) -> &[Construct] {
        self.constructs.get(file).map(Vec::as_slice).unwrap_or_default()
    }

    /// Returns the labels of the page's elements: maps the identifier of each element that
    /// a label refers to to that label, as written. Where several labels refer to one
    /// element, the alphabetically first label is kept. A label without an identifier gives
    /// no entry.
    pub fn references(&self) -> BTreeMap<String, String> {
        let mut references = BTreeMap::new();
        for entry in &self.labels {
            if let Some(id) = &entry.id {
                let kept = references.entry(id.clone()).or_insert_with(|| entry.label.clone());
                if entry.label < *kept {
                    kept.clone_from(&entry.label);
                }
            }
        }
        references
    }

    /// Returns what the snapshot knows of the byte `offset` of the file named `file`,
    /// counted as in `data-flm-source-span`: its line and column, and the constructs that
    /// contain it (whose range starts at or before it and ends after it), from the
    /// innermost outward.
    ///
    /// # Errors
    ///
    /// [`AtError::UnknownFile`] when no source has the name `file`, and
    /// [`AtError::PastEnd`] when the offset is past the end of the source.
    pub fn at(&self, file: &str, offset: usize) -> Result<At, AtError> {
        let Some((line, column)) = self.places.line_column(file, offset) else {
            return Err(if self.places.has(file) {
                AtError::PastEnd { file: file.to_owned(), offset }
            } else {
                AtError::UnknownFile { file: file.to_owned() }
            });
        };
        let mut constructs: Vec<Construct> = self
            .constructs(file)
            .iter()
            .filter(|construct| construct.start <= offset && offset < construct.end)
            .cloned()
            .collect();
        // An inner construct lies within the outer one: it is shorter, or, for two of one
        // range, comes later in document order (the sort is stable).
        constructs.reverse();
        constructs.sort_by_key(|construct| construct.end - construct.start);
        Ok(At { line, column, constructs })
    }
}

/// The construct that the node `node` is, when it is one: see the [module
/// documentation](self), "The constructs". The feature that declares it is the owner of the
/// node's element in `registry`, found by the element's identity, which holds for an
/// element of a scoped package too (`\item` of the lists feature); else the feature of the
/// first declaration of its name. A structural element, which no feature declares, has the
/// language's own owner, `flm`, and no feature.
fn construct_of(node: &NodeRef<'_, Flm>, registry: &flm_core::core::registry::FlmRegistry) -> Option<Construct> {
    let kind = match node.callable_type()? {
        FlmCallableType::Macro => CallableKind::Macro,
        FlmCallableType::Environment => CallableKind::Environment,
        FlmCallableType::Specials => CallableKind::Specials,
        FlmCallableType::Placeholder | FlmCallableType::Presentation => return None,
    };
    let span = node.span();
    let file = span.source().origin().canonical_path()?;
    let name = node.name()?;
    let element = element_of(node);
    let owner = registry.elem_idx(element).and_then(|idx| registry.element(idx)).map(|entry| entry.owner.to_string());
    let feature = match owner {
        Some(owner) if owner == LANGUAGE_OWNER => None,
        Some(owner) => Some(owner),
        None => {
            let declarations = registry.declarations().named(kind, name);
            // The declaration whose element the node has, else the first one of the name.
            let declaration = declarations
                .iter()
                .find(|declaration| Arc::ptr_eq(&declaration.element, element))
                .or(declarations.first());
            declaration.map(|declaration| declaration.owner.to_string())
        }
    };
    if kind == CallableKind::Specials && feature.is_none() {
        return None;
    }
    let range = span.range();
    Some(Construct {
        file: file.to_owned(),
        start: range.start,
        end: range.end,
        kind,
        name: name.to_owned(),
        feature,
    })
}

/// The sources of one build, by name, which turn a place written in the page (a source's
/// name and a byte offset into it) into the line and the column an editor counts.
///
/// Made from a run with [`Places::of_run`]; [`Places::line_column`] does the conversion.
#[derive(Debug, Clone, Default)]
pub struct Places {
    sources: BTreeMap<String, Arc<FlmSource>>,
}

impl Places {
    /// Collects the sources of the document that `run` processed, each under its name
    /// (its canonical path): first the source of each tree of the document (the primary
    /// file, and every file brought in with a heading of its own); then, for a name that no
    /// tree has, the source of the first node that comes from it, in document order (a file
    /// brought in without a heading).
    ///
    /// A node that the library made (the expansion of a macro, text that a feature built
    /// during lowering) counts for the source it was made from, which is the source the
    /// HTML backend writes into the page for it.
    pub fn of_run(run: &Run) -> Places {
        // The sources are the build's own, never the files read again: a file may have
        // changed since the build, whose output is the page the places come from.
        let content = run.doc().content();
        let mut sources = BTreeMap::new();
        for (_, fragment) in content.trees() {
            if let Some(name) = fragment.source().origin().canonical_path() {
                sources.entry(name.to_owned()).or_insert_with(|| Arc::clone(fragment.source()));
            }
        }
        for (_, node) in content.walk_nodes() {
            if let Some(named) = named_source(node.span())
                && let Some(name) = named.origin().canonical_path()
                && !sources.contains_key(name)
            {
                sources.insert(name.to_owned(), named);
            }
        }
        Places { sources }
    }

    /// Whether a source has the name `name`.
    pub fn has(&self, name: &str) -> bool {
        self.sources.contains_key(name)
    }

    /// Returns the line and the column, both counted from 1, of the byte offset `offset` in
    /// the source named `name`: the line as the file numbers it (the front matter's lines
    /// counted), the column in characters from the start of the line. An offset inside a
    /// character counts as that character's offset. Sources of any length are handled.
    /// Returns `None` when no source has that name, or when the offset is past the end of
    /// the source.
    pub fn line_column(&self, name: &str, offset: usize) -> Option<(usize, usize)> {
        let source = self.sources.get(name)?;
        let content = source.content();
        if offset > content.len() {
            return None;
        }
        let offset = (0..=offset).rev().find(|at| content.is_char_boundary(*at)).unwrap_or(0);
        // Every source is indexed, whatever its length: the index's default limit, past
        // which it gives no line, is for displaying a diagnostic, where a byte position
        // stands in; an editor needs the line.
        let mut index = source.line_index();
        index.set_max_scan_len(content.len());
        let (line, range) = index.line_of(offset)?;
        let column = content[range.start..offset].chars().count() + source.column_number_offset();
        Some((line, column))
    }
}

/// The first source with a name on the chain of sources back from `span`: the span's own
/// source, or, for content the library made, the source it was made from.
fn named_source(span: &FlmSpan) -> Option<Arc<FlmSource>> {
    let mut span = span.clone();
    loop {
        if span.source().origin().canonical_path().is_some() {
            return Some(Arc::clone(span.source()));
        }
        let triggered = span.source().provenance().triggered_at()?.clone();
        span = triggered;
    }
}
