//! The `toc` feature: the document's table of contents, as a table of entries
//! a program can read and as a piece a backend places before the content.
//!
//! # What it lists
//!
//! A *heading* is any node whose element carries the sectioning aspect
//! ([`Sectioning`](crate::model::aspects::Sectioning)): the standard headings
//! (`\section` to `\subparagraph`) and any heading kind an extension declares
//! alike. The feature lists, in the run's document order, every heading that
//! the aspect says is listed ([`in_toc`](crate::model::aspects::Sectioning::in_toc):
//! not of a kind declared `skip_toc`, not written `toc=false`) and whose level
//! ([`level`](crate::model::aspects::Sectioning::level)) is at most the
//! setting [`depth`](TocOptions::depth) (3 by default: sections, subsections
//! and subsubsections). A starred heading is listed like any other. A heading
//! inside a file brought in as a heading input comes where that file is placed.
//!
//! Each entry shows what the aspect returns for the heading
//! ([`toc_title`](crate::model::aspects::Sectioning::toc_title)): for a
//! standard heading, the alternative title written with the heading key
//! `alt=`, and the heading's title otherwise. A footnote inside the title
//! shows its mark in the entry, and its note is listed once, where the heading
//! stands.
//!
//! # The table and the piece
//!
//! The stage [`BUILD`] (`flm.toc.build`, [`BuildToc`]) writes two tables:
//!
//! - [`ENTRIES`] (`flm.toc.entries`, [`TocEntries`]): one [`TocEntry`] per
//!   listed heading, in document order: the heading's address, its level, its
//!   first reference point and the content its entry shows. The table holds no
//!   number and no anchor: a reader takes both from the counters and anchors
//!   tables at the point of use, as the heading's own lowering rule does, so
//!   that every output and the table agree. A program demands the table by its
//!   name and reads it without rendering anything.
//! - [`PIECE`] (`flm.toc`, a [`Piece`]): the table of contents as content, one
//!   synthesized value of nested lists. A *piece* is a document-level
//!   synthesized value that a backend places beside the document's content
//!   (see [pieces](crate::guide::concepts_overview#pieces)).
//!
//! The piece is declared before the content and **off by default**
//! ([`PieceDecl::default_on`]): a render places it only when its pieces
//! option includes it (`pieces.include: [flm.toc]`), or when the compose
//! template of the output requires it. Among the pieces placed before the
//! content, it comes after the metadata's.
//!
//! # The shape of the piece
//!
//! The value holds three synthesized elements, which no document can write:
//! [`PIECE`] (`flm.toc`), the root, holding one [`LIST`] (`flm.toc.list`);
//! a list holds [`ENTRY`] (`flm.toc.entry`) nodes, each carrying a
//! [`TocEntryPayload`] and holding the entry's title, then, where the entry
//! has sub-entries, one list of them. The entries are nested by relative
//! level: an entry whose level is greater than the level of the entry before
//! it at the same depth goes into a new list inside that entry; a level gap (a
//! subsubsection directly under a section) nests one list deeper, not two;
//! the first entry, at any level, is a top-level entry. A heading left out
//! (too deep, or not listed) is skipped: its listed sub-headings are nested
//! among the headings kept.
//!
//! The lowering rules ([`rules`]) stage, on every backend:
//!
//! ```text
//! flm.pres.block { role: flm.toc, kinds: [toc], doc_role: Toc }
//!   └ flm.pres.list { kinds: [unnumbered, toc] }
//!       per entry:  label ⇒ nothing
//!                   item  ⇒ flm.pres.link { href: "#<anchor>", kind: toc }
//!                             ├ flm.pres.span { role: flm.toc.entry.number }  the number
//!                             ├ " "
//!                             └ the title
//!                           the sub-list, where there is one
//! ```
//!
//! The number is the heading's number alone, without the word and the
//! delimiters a reference writes. An entry without a number has no number
//! span and no space; an entry whose heading has no anchor has no link.
//!
//! On the HTML page ([`html`]), the block is a `<nav>` with the role
//! `doc-toc`, and the page carries the feature's stylesheet [`STYLE`]
//! (`flm.toc.style`). The plain-text backend prints the nested lists as it
//! prints every list whose labels are all empty: one entry per paragraph, at
//! the left margin, so the nesting shows only through the numbers. In the
//! rendered LaTeX the block is written through the backend's contract and the
//! requirement brings in the feature's definition piece
//! ([`latex_rendered`]): the title, the entries linked to the headings, and
//! each entry's page number as LaTeX gives it. In the
//! recomposed LaTeX the three elements write nothing ([`latex`]): LaTeX builds
//! its own table of contents, and the piece is not placed on that output
//! unless a render names it for that backend.
//!
//! # Settings
//!
//! The option type [`TocOptions`] (`depth`), bound on the document or on the
//! render request ([`Bind::Either`]): `flm: {features: {toc: {depth: 2}}}` in
//! a front matter, `-O toc.depth=2` on the command line.
//!
//! # Interactions with other features
//!
//! - `index` and `refpoints`: the stage reads the element index, and the table
//!   of reference points where the `refpoints` feature is registered; without
//!   it, no entry has a point, and so no number and no link.
//! - `numbering` and `refs`: the rules read the counters and anchors tables
//!   where they are computed, under peer declarations
//!   ([`PeerName`](crate::core::registry::PeerName)), so the feature builds in
//!   a registry without either.
//! - `pieces`: the piece declaration is a peer contribution to the slot
//!   [`pieces::SLOT`]; without the `pieces` feature no backend places it.
//!
//! # Items
//!
//! - The feature: [`Toc`], [`feature`], [`FEATURE`], [`TocOptions`].
//! - The table: [`ENTRIES`], [`TocEntries`], [`TocEntry`].
//! - The stage: [`BUILD`], [`BuildToc`], [`TocFailed`].
//! - The piece and its elements: [`PIECE`], [`LIST`], [`ENTRY`],
//!   [`TocEntryPayload`], [`KIND_TOC`], [`NUMBER_ROLE`].
//! - The rules: [`rules`], [`latex`]. The style identifier, the stylesheet
//!   and the rendered LaTeX's definition piece: [`STYLE`], [`html`],
//!   [`latex_rendered`].

pub mod html;
pub mod latex;
pub mod latex_rendered;
pub mod rules;
pub mod stage;

use std::sync::Arc;

use techy::core::node::NodeRef;
use techy::serialize::SerialValue;

use crate::core::data::table::{PieceClass, ProjectCx, SingletonTable, Table, TableName};
use crate::core::piece::{Piece, PieceDecl, PiecePlace};
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::core::settings::SettingsDecl;
use crate::lang::{Flm, PresPayload};
use crate::model::addr::NodeAddr;
use crate::model::aspects::{ContentRef, PointId};
use crate::model::options::Bind;
use crate::stdcore::backends::pieces;
use crate::stdcore::features::refpoints::{content_text, point_key};

pub use stage::{BuildToc, TocFailed};

/// The feature's registered name, `toc`; also its settings key.
pub const FEATURE: &str = "toc";

/// The typed name of the table of entries, `flm.toc.entries` ([`TocEntries`]);
/// like every table name, it is also the capability a caller demands to have
/// it computed.
pub const ENTRIES: TableName<TocEntries> = TableName::new("flm.toc.entries");

/// The name of the piece `flm.toc`: the name of its table (of type
/// [`Piece`]), of its declaration, and of the synthesized element at the root
/// of its content.
pub const PIECE: &str = "flm.toc";

/// The name of the synthesized element of a list of entries, `flm.toc.list`.
pub const LIST: &str = "flm.toc.list";

/// The name of the synthesized element of one entry, `flm.toc.entry`, which
/// carries a [`TocEntryPayload`].
pub const ENTRY: &str = "flm.toc.entry";

/// The name of the stage that writes the table of entries and the piece,
/// `flm.toc.build` ([`BuildToc`]).
pub const BUILD: &str = "flm.toc.build";

/// The feature's style identifier, `flm.toc.style`, which the general rule
/// of the root ([`RootRule`](rules::RootRule)) requires: the stylesheet on the
/// HTML page ([`html::stylesheet`]).
pub const STYLE: &str = "flm.toc.style";

/// The kind token of the block and of the lists the rules stage, `toc`: a
/// stylesheet or a program's lowering rule can select on it.
pub const KIND_TOC: &str = "toc";

/// The role of the span holding an entry's number, `flm.toc.entry.number`.
pub const NUMBER_ROLE: &str = "flm.toc.entry.number";

/// The default of [`TocOptions::depth`], 3.
pub const DEFAULT_DEPTH: u32 = 3;

/// The options of the `toc` feature, bound on the document or on the render
/// request ([`Bind::Either`]); in data, the settings of the feature `toc`.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::toc::TocOptions;
///
/// assert_eq!(TocOptions::default().depth, 3);
/// ```
#[derive(Debug, Clone, Hash, serde::Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct TocOptions {
    /// The deepest heading level listed: a heading is listed when its level is at
    /// most this number. 3 by default (sections, subsections, subsubsections).
    pub depth: u32,
}

impl Default for TocOptions {
    fn default() -> TocOptions {
        TocOptions { depth: DEFAULT_DEPTH }
    }
}

/// One entry of the table of contents: a listed heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocEntry {
    /// The heading.
    pub addr: NodeAddr,
    /// Its level, as the sectioning aspect returns it.
    pub level: u32,
    /// Its first reference point, where the reference points table holds one.
    pub point: Option<PointId>,
    /// What an entry shows: the content the sectioning aspect returns.
    pub title: Option<ContentRef>,
}

/// The table of entries, `flm.toc.entries` ([`ENTRIES`]): one [`TocEntry`] per
/// listed heading, in the run's document order. See the [module
/// documentation](self).
///
/// The table is a document fact ([`PieceClass::DocumentFact`]) when it holds
/// an entry: it describes the document's own headings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TocEntries {
    /// The entries, in document order.
    pub entries: Vec<TocEntry>,
}

impl Table for TocEntries {
    /// Appends the piece's entries after this table's.
    fn absorb(&mut self, piece: TocEntries) {
        self.entries.extend(piece.entries);
    }

    /// A list, one map per entry: `node` (the heading), `level`, `point`
    /// (where there is one, keyed `<node>#<index>` as the table of reference
    /// points keys a point) and `title` (where there is one: the reference as
    /// text, or the plain text of a tree the aspect built).
    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.entries
                .iter()
                .map(|entry| {
                    let mut map = vec![
                        ("node".to_string(), cx.node_ref(entry.addr)),
                        ("level".to_string(), SerialValue::Int(i64::from(entry.level))),
                    ];
                    if let Some(point) = entry.point {
                        map.push(("point".to_string(), SerialValue::Str(point_key(cx, point))));
                    }
                    if let Some(title) = &entry.title {
                        map.push(("title".to_string(), SerialValue::Str(content_text(cx, entry.addr.tree, title))));
                    }
                    SerialValue::Map(map)
                })
                .collect(),
        )
    }

    fn piece_class(&self) -> PieceClass {
        if self.entries.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

impl SingletonTable for TocEntries {
    const NAME: &'static str = "flm.toc.entries";
}

/// The payload of an [`ENTRY`] node: the heading's first reference point,
/// where it has one, and its level. The entry's rule reads the number and the
/// anchor of the point from the counters and anchors tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TocEntryPayload {
    /// The heading's first reference point, where the reference points table
    /// holds one.
    pub point: Option<PointId>,
    /// The heading's level.
    pub level: u32,
}

impl PresPayload for TocEntryPayload {
    /// `{point?, level}`: the point as `<tree>/<node>#<index>`, then the level.
    fn project(&self) -> SerialValue {
        let mut map = Vec::with_capacity(2);
        if let Some(point) = self.point {
            map.push((
                "point".to_string(),
                SerialValue::Str(format!("{}/{}#{}", point.addr.tree, point.addr.node.index(), point.index)),
            ));
        }
        map.push(("level".to_string(), SerialValue::Int(i64::from(self.level))));
        SerialValue::Map(map)
    }
}

impl TocEntryPayload {
    /// The payload of the entry node `node`, or `None` for a node that carries
    /// none.
    pub fn of<'n>(node: &'n NodeRef<'_, Flm>) -> Option<&'n TocEntryPayload> {
        node.ext().payload.as_deref()?.downcast_ref::<TocEntryPayload>()
    }
}

/// The declaration of the feature's settings: the field of [`TocOptions`]
/// (`depth`) is its settings key.
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE).options::<TocOptions>(
        Bind::Either,
        "The deepest heading level the table of contents lists (`depth`, 3 by default). Set on \
         the document; a render request may set it again, and its value then replaces the \
         document's for that render.",
    )
}

/// The `toc` feature: the option type [`TocOptions`], the table [`ENTRIES`],
/// the stage [`BUILD`], the piece [`PIECE`] with its elements, lowering rules
/// and stylesheet (see the [module documentation](self)).
///
/// Use [`feature()`] to obtain it as a value a package entry can list.
#[derive(Debug, Default, Clone, Copy)]
pub struct Toc;

impl FlmFeature for Toc {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Table of contents").docs(
            "The document's table of contents: the table `flm.toc.entries` of the listed \
             headings, and the piece `flm.toc` (off by default) that shows them as nested lists \
             before the content.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.table(
            ENTRIES,
            "The document's listed headings, in document order: each heading's address, level, \
             first reference point and the content its entry shows.",
        )?;
        b.table_named::<Piece>(PIECE, "The table of contents as nested lists, or nothing to place.")?;
        b.stage(Arc::new(BuildToc))?;
        rules::declare(b)?;
        html::register(b)?;
        latex_rendered::register(b)?;
        latex::register(b)?;
        // The pieces feature owns the slot, and may be absent: the table of
        // contents is then placed by no backend.
        b.peer_slot(
            pieces::SLOT,
            Arc::new(
                PieceDecl::new(PIECE, PiecePlace::BeforeContent, "the table of contents, before the content")
                    .default_on(false),
            ),
        )?;
        Ok(())
    }
}

/// Returns the [`Toc`] feature, ready to be listed in a package entry.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::toc;
///
/// assert_eq!(toc::feature().info().name.as_str(), toc::FEATURE);
/// ```
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Toc)
}
