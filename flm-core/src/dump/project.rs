//! The projection: a one-way rendering of a tree, a node, a computed table or a
//! diagnostic as a techy serialized value ([`SerialValue`]), in a vocabulary
//! this library owns and keeps stable.
//!
//! The projection is what the library's tests compare, usually through a JSON
//! [golden file](crate::dump::golden), and what `flm dump --view compact`
//! prints. A parsed tree and a lowered (presentation) tree project in the same
//! shape. The projection shows what each node means: the type name of the
//! element of every callable and group node (`elem`), text runs by their text (`chars`),
//! comments and nested list nodes by their kind (`kind`), arguments by name
//! (`args`), children, the body of an environment-shaped node (`body`), a
//! presentation node's payload through [`PresPayload::project`], and, on
//! request, spans as `line:col`. It never shows a parsing state, a techy
//! specification or a node id. Because the vocabulary belongs to this library
//! rather than to techy, a change in techy's serialized layout does not change
//! a golden file. A projection cannot be read back into a tree; the wire form
//! of [`crate::dump::session`] is the form that reads back.
//!
//! Use a [`Projector`] (or the free functions [`project_tree`],
//! [`project_node`], [`project_nodes`], [`project_diagnostic`],
//! [`project_diagnostics`] and [`node_ref_value`], each of which builds one for
//! a single call) with [`ProjectOptions`] to choose what is shown beyond the
//! structure. A table is projected through [`Projector::table`]. Render the
//! result as text with [`to_json_pretty`](crate::dump::json::to_json_pretty).
//!
//! The projection descends a tree by plain recursion on the calling thread,
//! without the [recursion guard](crate::lang::guard). A tree nested deeply
//! enough can therefore overflow the calling thread's stack, which aborts the
//! process. Projecting on a [worker thread](crate::lang::guard::on_worker_thread)
//! gives a large stack but no refusal.
//!
//! # The shape of a node's projection
//!
//! Each node projects as a map whose keys come in the order listed; a key is
//! absent when it has nothing to show:
//!
//! - a text run: `{"chars": "…"}`, plus `"map": [{"len", "span"}, …]` when
//!   source maps are requested and the run carries one;
//! - a comment: `{"kind": "comment", "text": "…"}`;
//! - a group: `{"elem": "flm.group", "children": […]}` (or `flm.math`,
//!   `flm.verbatim`, `flm.paragraph`; a math group adds `"form": "inline"` or
//!   `"display"` before its children);
//! - a callable: `{"elem": "<element>", "name": "<callable name>", "args":
//!   {"<argument name or index>": […]}, "body": […], "slots": {"<slot name or
//!   index>": […]}, "payload": <projection>}`: the name only for callables
//!   written as a macro or an environment, the body for the node's body slot,
//!   every other node slot (techy's `ParsedSlot`, a part of a node's content
//!   outside its arguments) under `slots`;
//! - a nested list node: `{"kind": "list", "children": […]}`;
//!
//! plus `"span": "L:C-L:C"` on every node when spans are requested. A tree
//! projects as the list of its root's children (the root container itself is
//! not shown), or as a one-item list holding the root when the root is not a
//! list node. An optional argument that was not given has no key; an argument
//! given empty is an empty list. Arguments are keyed by their declared name,
//! else by their index.
//!
//! **The `name` key shows a callable's source spelling** (the command word of a
//! macro, the name of an environment) and nothing else. The spelling tells
//! apart two element values that share one type name: `\section` and
//! `\subsection` carry two different element values of the type name
//! `flm.heading` (see [elements and
//! aspects](crate::guide::concepts_overview#elements-and-aspects)), and the
//! projection would not distinguish them otherwise. A callable recognized by a special character sequence has as its
//! name the sequence that triggered it (for a paragraph break, the whitespace
//! it matched), and the presentation and placeholder callables carry an
//! internal key; neither is a spelling, so neither is shown, and the element's
//! type name says what those nodes are. The element's own data
//! ([`ElementSpec::value`](crate::model::element::ElementSpec::value)) is not
//! shown either.
//!
//! # Diagnostics, tables and positions
//!
//! A diagnostic projects as `{"id", "severity", "span", "file"?, "data"}`: its
//! identifier, its severity, its span as `line:col`, the canonical path of its
//! source when the source has one, and the serialized data of its condition;
//! never its message, which is not a stable interface.
//!
//! A table projects under its name: `{"<table name>": <projection>}`. The
//! table's own [`Table::project`] produces the inner projection and renders
//! the addresses it holds through [`ProjectCx`]: node references as
//! `elem@line:col` ([`node_ref_value`]) and tree references as the tree id's
//! display ([`tree_ref_value`]). [`TableProjection`] is the implementation of
//! [`ProjectCx`] the library uses, and [`TreeSource`] supplies the trees the
//! node references are resolved in.
//!
//! Positions use the source's line and column numbering (1-based unless the
//! source sets other offsets). A span is `L:C-L:C` from its start to its
//! exclusive end, or `L:C` when it is empty. A source too large for techy to
//! index by line is shown with byte offsets instead: `@start-end` for a span,
//! `b<offset>` in a node reference.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use techy::core::node::{BodySlotExt, NodeId, NodeKind, NodeRef, NodeTree};
use techy::latexlike::MathGroupForm;
use techy::serialize::SerialValue;
use techy::source::LineIndexCache;

use crate::model::addr::{NodeAddr, TreeId};
use crate::model::doc::DocContent;
use crate::core::run::Run;
use crate::core::data::table::{ProjectCx, Table};
use crate::lang::ext::{PresPayload, TextSourceMap};
use crate::lang::flm::{Flm, FlmCallableType, FlmGroupType};
use crate::lang::origin::{FlmDiagnostic, FlmOrigin, FlmSpan};

/// The options of a projection: what it shows beyond the structure it always
/// shows.
///
/// The three switches are independent. By default spans and source maps are
/// hidden and presentation payloads are shown. Build the options with
/// [`new`](ProjectOptions::new) and the builder methods, then pass them to
/// [`Projector::new`] or to one of the free functions of this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectOptions {
    /// Whether every node shows its span as `line:col` (key `"span"`); off by
    /// default.
    pub spans: bool,
    /// Whether presentation nodes show their payload (key `"payload"`); on by
    /// default.
    pub payloads: bool,
    /// Whether text runs show their text source map, when they carry one (key
    /// `"map"`); off by default.
    pub source_maps: bool,
}

impl Default for ProjectOptions {
    fn default() -> ProjectOptions {
        ProjectOptions { spans: false, payloads: true, source_maps: false }
    }
}

impl ProjectOptions {
    /// Returns the default options: no spans, payloads shown, no source maps.
    pub fn new() -> ProjectOptions {
        ProjectOptions::default()
    }

    /// Builder: shows the span of every node.
    pub fn with_spans(mut self) -> ProjectOptions {
        self.spans = true;
        self
    }

    /// Builder: hides the payloads of presentation nodes.
    pub fn without_payloads(mut self) -> ProjectOptions {
        self.payloads = false;
        self
    }

    /// Builder: shows the text source maps of text runs.
    pub fn with_source_maps(mut self) -> ProjectOptions {
        self.source_maps = true;
        self
    }
}

fn text(value: impl Into<String>) -> SerialValue {
    SerialValue::Str(value.into())
}

/// Whether a callable of this form shows its name under the `name` key: only
/// when the name is the callable's source spelling, the command word of a macro
/// or the name of an environment. A callable recognized by a special character
/// sequence has the triggering sequence as its name (for a paragraph break, the
/// matched whitespace), and the presentation and placeholder callables carry an
/// internal key; neither is a spelling, so neither is shown.
fn has_source_name(form: FlmCallableType) -> bool {
    match form {
        FlmCallableType::Macro | FlmCallableType::Environment => true,
        FlmCallableType::Specials
        | FlmCallableType::Placeholder
        | FlmCallableType::Presentation => false,
    }
}

fn map(entries: Vec<(&str, SerialValue)>) -> SerialValue {
    SerialValue::Map(entries.into_iter().map(|(key, value)| (String::from(key), value)).collect())
}

/// Projects trees, nodes, tables and diagnostics with one set of
/// [`ProjectOptions`], caching the line positions it computes.
///
/// One projector serves any number of projections, and every method takes
/// `&self`, so all the entries of a table are projected through one projector.
/// Turning a byte offset into a line and column requires the line starts of the
/// source, so the projector caches them per source. The cache is behind a lock
/// and never changes an output, so the methods need no `&mut`.
///
/// The free functions of this module ([`project_tree`], [`project_node`], and so
/// on) build a projector for a single call; use a `Projector` directly when
/// projecting more than one thing. The [module documentation](self) gives the
/// shape of every projection.
pub struct Projector {
    options: ProjectOptions,
    lines: Mutex<LineIndexCache<FlmOrigin>>,
}

impl Projector {
    /// Creates a projector with the given options.
    pub fn new(options: ProjectOptions) -> Projector {
        Projector { options, lines: Mutex::new(LineIndexCache::new()) }
    }

    /// Returns the projector's options.
    pub fn options(&self) -> &ProjectOptions {
        &self.options
    }

    /// Projects a whole tree: the list of its root's children (the root
    /// container itself is not shown), or a one-item list holding the root when
    /// the root is not a list node.
    pub fn tree(&self, tree: &NodeTree<Flm>) -> SerialValue {
        let root = tree.root();
        if root.is_list() {
            self.nodes(root.children().iter())
        } else {
            SerialValue::List(vec![self.node(&root)])
        }
    }

    /// Projects the given nodes, as a list of their projections.
    pub fn nodes<'t>(&self, nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>) -> SerialValue {
        SerialValue::List(nodes.into_iter().map(|node| self.node(&node)).collect())
    }

    /// Projects one node, in the shape the [module documentation](self) gives.
    pub fn node(&self, node: &NodeRef<'_, Flm>) -> SerialValue {
        let mut entries: Vec<(&str, SerialValue)> = Vec::new();
        match node.kind() {
            NodeKind::Chars { content } => {
                entries.push(("chars", text(content.resolve(node.span().source()))));
                if self.options.source_maps {
                    if let Some(source_map) = &node.ext().source_map {
                        entries.push(("map", self.source_map(source_map)));
                    }
                }
            }
            NodeKind::Comment(comment) => {
                entries.push(("kind", text("comment")));
                entries.push(("text", text(comment.content.resolve(node.span().source()))));
            }
            NodeKind::List => {
                entries.push(("kind", text("list")));
                entries.push(("children", self.nodes(node.children().iter())));
            }
            NodeKind::Group(group) => {
                entries.push(("elem", text(node.ext().element.name.as_str())));
                if let Some(FlmGroupType::Math(form)) = group.group_type {
                    let form = match form {
                        MathGroupForm::Inline => "inline",
                        MathGroupForm::Display => "display",
                    };
                    entries.push(("form", text(form)));
                }
                entries.push(("children", self.nodes(node.children().iter())));
            }
            NodeKind::Callable(data) => {
                entries.push(("elem", text(node.ext().element.name.as_str())));
                if has_source_name(data.callable_type) {
                    entries.push(("name", text(&*data.name)));
                }
                let mut args: Vec<(String, SerialValue)> = Vec::new();
                for (index, argument) in data.arguments.iter().enumerate() {
                    if let Some(content) = node.argument_content_nodes(index) {
                        let key = argument.name().map_or_else(|| index.to_string(), String::from);
                        args.push((key, self.nodes(content.iter())));
                    }
                }
                if !args.is_empty() {
                    entries.push(("args", SerialValue::Map(args)));
                }
                let mut body: Option<SerialValue> = None;
                let mut slots: Vec<(String, SerialValue)> = Vec::new();
                for (index, slot) in data.slots.iter().enumerate() {
                    let Some(content) = node.slot_content_nodes(index) else {
                        continue;
                    };
                    let projection = self.nodes(content.iter());
                    if body.is_none() && slot.ext.is_body() {
                        body = Some(projection);
                    } else {
                        let key = slot.name().map_or_else(|| index.to_string(), String::from);
                        slots.push((key, projection));
                    }
                }
                if let Some(body) = body {
                    entries.push(("body", body));
                }
                if !slots.is_empty() {
                    entries.push(("slots", SerialValue::Map(slots)));
                }
                if self.options.payloads {
                    if let Some(payload) = &node.ext().payload {
                        entries.push(("payload", payload.project()));
                    }
                }
            }
        }
        if self.options.spans {
            entries.push(("span", text(self.span(node.span()))));
        }
        map(entries)
    }

    /// Projects a text source map as `[{"len", "span"}, …]`: for each part of
    /// the text run, its length in bytes and the span of source text it comes
    /// from, as `line:col`.
    pub fn source_map(&self, source_map: &TextSourceMap) -> SerialValue {
        SerialValue::List(
            source_map
                .pieces()
                .iter()
                .map(|piece| {
                    map(vec![
                        ("len", SerialValue::Int(i64::from(piece.out_len))),
                        ("span", text(self.span(&piece.span))),
                    ])
                })
                .collect(),
        )
    }

    /// Renders a reference to a node of `tree` as `elem@line:col`: the node's
    /// element and the position where its span starts. This is how a table entry
    /// that points at a node is shown. A node id that `tree` does not hold
    /// renders as `?@<the id's numeric index>`.
    pub fn node_ref(&self, tree: &NodeTree<Flm>, node: NodeId) -> SerialValue {
        match tree.get(node) {
            Some(node) => {
                text(format!("{}@{}", node.ext().element.name.as_str(), self.position(node.span())))
            }
            None => text(format!("?@{}", node.index())),
        }
    }

    /// Renders a reference to a tree as the tree id's display form (`tree#0`
    /// for a document tree, `value#3` for a synthesized value).
    pub fn tree_ref(&self, tree: TreeId) -> SerialValue {
        text(tree.to_string())
    }

    /// Projects one diagnostic as `{"id", "severity", "span", "file"?, "data"}`
    /// (see the [module documentation](self)); the message is never shown.
    pub fn diagnostic(&self, diagnostic: &FlmDiagnostic) -> SerialValue {
        let mut entries: Vec<(&str, SerialValue)> = vec![
            ("id", text(diagnostic.identifier())),
            ("severity", text(diagnostic.severity().to_string())),
            ("span", text(self.span(diagnostic.span()))),
        ];
        if let Some(path) = diagnostic.span().source().origin().canonical_path() {
            entries.push(("file", text(path)));
        }
        entries.push(("data", SerialValue::from(diagnostic.data().serializable_data())));
        map(entries)
    }

    /// Projects the given diagnostics, as a list of their projections.
    pub fn diagnostics<'a>(
        &self,
        diagnostics: impl IntoIterator<Item = &'a FlmDiagnostic>,
    ) -> SerialValue {
        SerialValue::List(
            diagnostics.into_iter().map(|diagnostic| self.diagnostic(diagnostic)).collect(),
        )
    }

    /// Renders a span as `L:C-L:C` (start, exclusive end), or `L:C` when the
    /// span is empty; as byte offsets (`@start-end`, or `@offset` when empty)
    /// when the source is too large for techy to index by line.
    pub fn span(&self, span: &FlmSpan) -> String {
        let mut lines = self.lines.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match lines.line_col_span(span.source(), span.range()) {
            Some(((line, col), (end_line, end_col))) => {
                if span.is_empty() {
                    format!("{line}:{col}")
                } else {
                    format!("{line}:{col}-{end_line}:{end_col}")
                }
            }
            None if span.is_empty() => format!("@{}", span.start()),
            None => format!("@{}-{}", span.start(), span.end()),
        }
    }

    /// Renders the start of a span as `L:C`, or as `b<offset>` when the source is
    /// too large for techy to index by line. This is the position part of a node
    /// reference ([`node_ref`](Projector::node_ref)).
    pub fn position(&self, span: &FlmSpan) -> String {
        let mut lines = self.lines.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match lines.line_col(span.source(), span.start()) {
            Some((line, col)) => format!("{line}:{col}"),
            None => format!("b{}", span.start()),
        }
    }
}

impl std::fmt::Debug for Projector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Projector").field("options", &self.options).finish_non_exhaustive()
    }
}

/// Projects a whole tree with a projector built for this call
/// ([`Projector::tree`]).
pub fn project_tree(tree: &NodeTree<Flm>, options: &ProjectOptions) -> SerialValue {
    Projector::new(*options).tree(tree)
}

/// Projects one node with a projector built for this call
/// ([`Projector::node`]).
pub fn project_node(node: &NodeRef<'_, Flm>, options: &ProjectOptions) -> SerialValue {
    Projector::new(*options).node(node)
}

/// Projects the given nodes, as a list, with a projector built for this call
/// ([`Projector::nodes`]).
pub fn project_nodes<'t>(
    nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>,
    options: &ProjectOptions,
) -> SerialValue {
    Projector::new(*options).nodes(nodes)
}

/// Projects one diagnostic with a projector built for this call
/// ([`Projector::diagnostic`]).
pub fn project_diagnostic(diagnostic: &FlmDiagnostic, options: &ProjectOptions) -> SerialValue {
    Projector::new(*options).diagnostic(diagnostic)
}

/// Projects the given diagnostics, as a list, with a projector built for this
/// call ([`Projector::diagnostics`]).
pub fn project_diagnostics<'a>(
    diagnostics: impl IntoIterator<Item = &'a FlmDiagnostic>,
    options: &ProjectOptions,
) -> SerialValue {
    Projector::new(*options).diagnostics(diagnostics)
}

// --- tables: the projection context ------------------------------------------------------

/// A set of trees looked up by [`TreeId`], in which the node references of a
/// table are resolved while the table is projected.
///
/// Three implementations are provided: a document's parsed trees
/// ([`DocContent`]), a run's frozen trees ([`Run`], the trees its tables were
/// computed over), and a map from tree id to tree that a test assembles.
///
/// The id of a [synthesized
/// value](crate::guide::concepts_overview#synthesized-values) (a small tree a
/// stage builds) is found in none of them, because such a tree exists only
/// inside the stage invocation that built it (see [`crate::model::addr`]). A
/// table holding one therefore projects the tree **inline**, through
/// [`ProjectCx::tree`], and never by its id.
pub trait TreeSource {
    /// Returns the tree with this id, or `None` when this set does not hold it.
    fn tree_of(&self, id: TreeId) -> Option<&NodeTree<Flm>>;
}

impl TreeSource for DocContent {
    fn tree_of(&self, id: TreeId) -> Option<&NodeTree<Flm>> {
        self.tree(id).map(|tree| &**tree)
    }
}

impl TreeSource for Run {
    /// Looks the id up among the run's frozen trees, the trees its tables were
    /// computed over.
    fn tree_of(&self, id: TreeId) -> Option<&NodeTree<Flm>> {
        self.frozen_tree(id).map(|tree| &**tree)
    }
}

impl TreeSource for BTreeMap<TreeId, Arc<NodeTree<Flm>>> {
    fn tree_of(&self, id: TreeId) -> Option<&NodeTree<Flm>> {
        self.get(&id).map(|tree| &**tree)
    }
}

/// The context through which a table projects itself: a [`Projector`] and the
/// trees ([`TreeSource`]) in which the table's node references are resolved.
///
/// This is the implementation of [`ProjectCx`] the library uses, so that every
/// table's [`project`](Table::project) renders node addresses and tree ids in
/// the projection's vocabulary. Create one with
/// [`Projector::table_projection`], or let [`Projector::table`] create it.
pub struct TableProjection<'a> {
    projector: &'a Projector,
    trees: &'a dyn TreeSource,
}

impl ProjectCx for TableProjection<'_> {
    /// Renders the address as `elem@line:col`, looking the node up in the tree
    /// the address designates. An address whose tree is not in the
    /// [`TreeSource`] renders as `?@<tree>/<node index>`. This happens when a
    /// table stores the address of a node in a synthesized value, which
    /// [`crate::model::addr`] forbids, or of a tree the source does not hold;
    /// the projection then shows the error instead of hiding it.
    fn node_ref(&self, addr: NodeAddr) -> SerialValue {
        match self.trees.tree_of(addr.tree) {
            Some(tree) => self.projector.node_ref(tree, addr.node),
            None => text(format!("?@{}/{}", addr.tree, addr.node.index())),
        }
    }

    fn tree_ref(&self, id: TreeId) -> SerialValue {
        self.projector.tree_ref(id)
    }

    /// Projects the tree inline, as [`Projector::tree`] does.
    fn tree(&self, tree: &NodeTree<Flm>) -> SerialValue {
        self.projector.tree(tree)
    }
}

impl Projector {
    /// Creates the context through which a table projects itself, resolving node
    /// references in `trees`.
    pub fn table_projection<'a>(&'a self, trees: &'a dyn TreeSource) -> TableProjection<'a> {
        TableProjection { projector: self, trees }
    }

    /// Projects a table under its name, `{"<name>": <projection>}`, resolving
    /// its node references in `trees`.
    ///
    /// The inner projection is the table's own [`Table::project`], called
    /// through a [`table_projection`](Projector::table_projection) over `trees`;
    /// [`project_table`] then puts it under `name`. Pass the table's registered
    /// name: a table is identified by its name, never by its Rust type.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeMap;
    /// use std::sync::Arc;
    ///
    /// use flm_core::model::addr::TreeId;
    /// use flm_core::core::data::table::{ProjectCx, Table};
    /// use flm_core::dump::{ProjectOptions, Projector, to_json};
    /// use flm_core::lang::flm::Flm;
    /// use flm_core::techy::core::node::NodeTree;
    /// use flm_core::techy::serialize::SerialValue;
    ///
    /// #[derive(Default)]
    /// struct Count(i64);
    ///
    /// impl Table for Count {
    ///     fn absorb(&mut self, piece: Count) {
    ///         self.0 += piece.0;
    ///     }
    ///     fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
    ///         SerialValue::Int(self.0)
    ///     }
    /// }
    ///
    /// // This table holds no node address, so an empty set of trees will do.
    /// let trees: BTreeMap<TreeId, Arc<NodeTree<Flm>>> = BTreeMap::new();
    /// let projector = Projector::new(ProjectOptions::new());
    /// let value = projector.table("example.count", &Count(3), &trees);
    /// assert_eq!(to_json(&value)?, r#"{"example.count":3}"#);
    /// # Ok::<(), flm_core::dump::JsonError>(())
    /// ```
    pub fn table<T: Table>(&self, name: &str, table: &T, trees: &dyn TreeSource) -> SerialValue {
        project_table(name, table.project(&self.table_projection(trees)))
    }
}

/// Puts a table's projection under the table's name: `{"<name>": <projection>}`.
///
/// A table is identified by its registered name rather than by its Rust type,
/// so the name is what `flm dump` and a golden file show. `projection` is what
/// the table's [`Table::project`] returned; [`Projector::table`] computes it and
/// calls this function in one step.
pub fn project_table(name: &str, projection: SerialValue) -> SerialValue {
    map(vec![(name, projection)])
}

/// Renders a reference to a node of `tree` as `elem@line:col`, with a projector
/// built for this call ([`Projector::node_ref`]).
pub fn node_ref_value(tree: &NodeTree<Flm>, node: NodeId, options: &ProjectOptions) -> SerialValue {
    Projector::new(*options).node_ref(tree, node)
}

/// Renders a reference to a tree as the tree id's display form, `tree#0` or
/// `value#3` ([`Projector::tree_ref`]).
pub fn tree_ref_value(tree: TreeId) -> SerialValue {
    text(tree.to_string())
}

/// Projects a presentation payload on its own ([`PresPayload::project`]): the
/// same value a node's projection shows under `"payload"`.
pub fn project_payload(payload: &dyn PresPayload) -> SerialValue {
    payload.project()
}
