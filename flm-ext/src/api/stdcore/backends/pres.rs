//! Mirrors [`flm_core::stdcore::backends::pres::names`]: the names of the
//! standard presentation elements and of their content slots, which a rule's
//! body stages with [`Lowerer::pres`](crate::api::core::lowering::Lowerer::pres),
//! [`Lowerer::pres_slots`](crate::api::core::lowering::Lowerer::pres_slots) or
//! [`pres_tree`](crate::api::core::lowering::pres_tree), and declares with
//! [`LoweringDecl::produces`](crate::api::core::lowering::LoweringDecl::produces).
//! The module holds names only. What differs: the three groups of elements
//! and the list kinds of `flm-core`'s module are not mirrored.
//!
//! A payload is data, the element's payload as its projection writes it; an
//! optional field may be left out (or `null`), a list field may be left out
//! (empty). The shapes, by element:
//!
//! | Element | Payload |
//! |---|---|
//! | `PAR` | `{ anchor? }` |
//! | `BLOCK` | `{ role?, kinds?: [name], anchor?, doc_role? }` |
//! | `BLOCK_HEADING`, `RUNIN_HEADING` | `{ name, level, in_toc, anchor? }` |
//! | `SPAN` | `{ style?, role?, kinds?: [name], anchor?, doc_role? }` |
//! | `LIST` | `{ kinds?: [name], anchor?, entry_anchors?: [anchor or null] }` |
//! | `CELLS` | `{ rows, cols, cells: [{ row, col, row_span, col_span, align?, header?, rules?: [edge], styles?: [name] }] }` |
//! | `LINES` | `{}` |
//! | `LINE` | `{ indent_left, indent_right, align }` (lengths as `"0tab"`, `"2pt"`; `align` `"left"`, `"center"`, `"right"`) |
//! | `VERBATIM` | `{ kind, display, lang? }` (`kind` `"text"`, `"code"`, `"a"`; `display` `"inline"`, `"block"`) |
//! | `MATH_PASSTHROUGH` | `{ display, env? }` (`display` `"inline"`, `"display"`) |
//! | `MATH_TEXT` | `{ command?, anchor?, at_tag? }` |
//! | `LINK` | `{ href, kind, doc_role?, label?, shows_target? }` |
//! | `IMAGE` | `{ src, display, alt?, width?, height?, srcset?: [{ src, descriptor }], anchor? }` |
//! | `UNSUPPORTED` | `{ element, message }` |
//! | `DROPPED` | `{ condition, detail }` |
//! | `UNKNOWN` | `{ kind, name, spelling }` (`kind` `"macro"`, `"environment"`, …) |
//! | `DECO_RUNIN`, `DECO_FINAL`, `DECO_MARGIN` | `{}` |
//! | `DECO_MARK` | `{ slot }` |
//! | `SUBTREE` | `{ tree }` (a tree id as text) |
//!
//! A span's `style` is one of `"emph"`, `"bold"`, `"italic"`, `"monospace"`,
//! `"underline"`, `"small-caps"`, `"superscript"`, `"subscript"`,
//! `"unresolved-reference"`; a `doc_role` one of `"endnotes"`, `"footnote"`,
//! `"noteref"`, `"backlink"`, `"bibliography"`, `"biblioref"`, `"toc"`. A
//! payload that does not read back is the method's argument error, with the
//! payload type's own message. An element outside this list and the HTML
//! construct ([`html::TAG`](super::html::TAG)) cannot be given a payload as
//! data.

use flm_core::stdcore::backends::pres::names;

/// Mirrors [`PAR`](names::PAR): `flm.pres.par`, a laid-out paragraph; its body is an inline run.
pub const PAR: &str = names::PAR;

/// Mirrors [`BLOCK`](names::BLOCK): `flm.pres.block`, a block container with a role and kinds; its body is a run of blocks.
pub const BLOCK: &str = names::BLOCK;

/// Mirrors [`BLOCK_HEADING`](names::BLOCK_HEADING): `flm.pres.block-heading`, a heading placed on its own between blocks.
pub const BLOCK_HEADING: &str = names::BLOCK_HEADING;

/// Mirrors [`RUNIN_HEADING`](names::RUNIN_HEADING): `flm.pres.runin-heading`, the title of a run-in heading.
pub const RUNIN_HEADING: &str = names::RUNIN_HEADING;

/// Mirrors [`SPAN`](names::SPAN): `flm.pres.span`, inline content with a display style, a role and kinds.
pub const SPAN: &str = names::SPAN;

/// Mirrors [`LIST`](names::LIST): `flm.pres.list`, a list: one node for the whole list, with a label and an item slot per entry.
pub const LIST: &str = names::LIST;

/// Mirrors [`CELLS`](names::CELLS): `flm.pres.cells`, a table of cells: one node for the whole table, with a cell slot per cell.
pub const CELLS: &str = names::CELLS;

/// Mirrors [`LINES`](names::LINES): `flm.pres.lines`, a sequence of line nodes, each printed on its own line.
pub const LINES: &str = names::LINES;

/// Mirrors [`LINE`](names::LINE): `flm.pres.line`, one line of a lines node, with its indentation and alignment.
pub const LINE: &str = names::LINE;

/// Mirrors [`VERBATIM`](names::VERBATIM): `flm.pres.verbatim`, byte-exact content of a verbatim kind.
pub const VERBATIM: &str = names::VERBATIM;

/// Mirrors [`MATH_PASSTHROUGH`](names::MATH_PASSTHROUGH): `flm.pres.math-passthrough`, mathematics passed through as source text.
pub const MATH_PASSTHROUGH: &str = names::MATH_PASSTHROUGH;

/// Mirrors [`MATH_TEXT`](names::MATH_TEXT): `flm.pres.math-text`, a run of ordinary inline content inside a formula.
pub const MATH_TEXT: &str = names::MATH_TEXT;

/// Mirrors [`LINK`](names::LINK): `flm.pres.link`, a link around inline display content.
pub const LINK: &str = names::LINK;

/// Mirrors [`IMAGE`](names::IMAGE): `flm.pres.image`, an image; no content.
pub const IMAGE: &str = names::IMAGE;

/// Mirrors [`UNSUPPORTED`](names::UNSUPPORTED): `flm.pres.unsupported`, the childless stand-in for content a backend cannot write.
pub const UNSUPPORTED: &str = names::UNSUPPORTED;

/// Mirrors [`DROPPED`](names::DROPPED): `flm.pres.dropped`, the childless mark a lowering rule stages where it drops content.
pub const DROPPED: &str = names::DROPPED;

/// Mirrors [`UNKNOWN`](names::UNKNOWN): `flm.pres.unknown`, a macro or an environment the parse could not attribute.
pub const UNKNOWN: &str = names::UNKNOWN;

/// Mirrors [`DECO_RUNIN`](names::DECO_RUNIN): `flm.pres.deco.runin`, a decoration at the start of its block.
pub const DECO_RUNIN: &str = names::DECO_RUNIN;

/// Mirrors [`DECO_FINAL`](names::DECO_FINAL): `flm.pres.deco.final`, a decoration closing its block.
pub const DECO_FINAL: &str = names::DECO_FINAL;

/// Mirrors [`DECO_MARGIN`](names::DECO_MARGIN): `flm.pres.deco.margin`, a decoration set beside the text.
pub const DECO_MARGIN: &str = names::DECO_MARGIN;

/// Mirrors [`DECO_MARK`](names::DECO_MARK): `flm.pres.deco-mark`, the decoration mark, the childless node that holds a decoration's place.
pub const DECO_MARK: &str = names::DECO_MARK;

/// Mirrors [`SUBTREE`](names::SUBTREE): `flm.pres.subtree`, the position at which another document tree is placed.
pub const SUBTREE: &str = names::SUBTREE;

/// Mirrors [`ALL`](names::ALL): every presentation element above, in the
/// order the standard package declares them.
pub const ALL: [&str; 22] = names::ALL;

/// Mirrors [`BODY_SLOT`](names::BODY_SLOT): `body`, the name of the slot that
/// holds a node's body, for [`Lowerer::pres_slots`](crate::api::core::lowering::Lowerer::pres_slots).
pub const BODY_SLOT: &str = names::BODY_SLOT;

/// Mirrors [`LABEL_SLOT`](names::LABEL_SLOT): `label`, the name of the slot
/// that holds a list entry's label.
pub const LABEL_SLOT: &str = names::LABEL_SLOT;

/// Mirrors [`ITEM_SLOT`](names::ITEM_SLOT): `item`, the name of the slot that
/// holds a list entry's item.
pub const ITEM_SLOT: &str = names::ITEM_SLOT;

/// Mirrors [`CELL_SLOT`](names::CELL_SLOT): `cell`, the name of the slot that
/// holds a table cell's content.
pub const CELL_SLOT: &str = names::CELL_SLOT;
