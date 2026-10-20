//! The `xnotes` feature: footnotes, and the other kinds of note set apart
//! from the running text, written `\footnote{…}` by default.
//!
//! The guide to the language describes these notes for authors
//! ([xnotes](crate::guide::language_syntax#xnotes)).
//!
//! # Xnotes and xnote kinds
//!
//! An **xnote** is a piece of text set apart from the running text, marked
//! where it belongs and shown elsewhere or in place (a footnote, an endnote,
//! a side note); an **xnote kind** is one way of being an xnote, declared by
//! the feature's configuration. (The word is made up: "note" alone says too
//! little, and "footnote" or "endnote" each describe one place where the note
//! is shown; the *x* stands for *foot*, *end*, *side*.) The footnotes written
//! `\footnote{…}` are one xnote kind; a host adds others, such as endnotes or
//! side notes, through the configuration, and another feature may contribute
//! an xnote kind for the notes its own elements carry. The configuration,
//! [`XnotesConfig`], lists the xnote kinds an author writes, each an
//! [`XnoteKind`]; the default configuration lists one, `footnote`.
//!
//! # What an author writes
//!
//! ```text
//! The claim holds in every case.\footnote{Except the empty one, which we
//! exclude by convention.} The next sentence goes on.
//!
//! A note holding paragraphs.\footnote{The first paragraph of the note.
//!
//!   The second paragraph.}
//!
//! A labeled note.\footnote[\label{fn:appendix}]{See the appendix.}
//! See \ref{fn:appendix}, or \ref[noprefix]{fn:appendix}.
//! ```
//!
//! An xnote is written `\<command>[<labels>]{<content>}`, where `<command>`
//! is one of the commands of its xnote kind ([`XnoteKind::commands`]). The
//! content is inline content when it is written as one run of text, and
//! paragraphs when it holds a blank line or a block such as a list; the xnote
//! kind's [`layout`](XnoteKind::layout) may fix one of the two.
//!
//! The optional argument holds the xnote's labels, `\label{fn:…}`, as many
//! as the author wants, and nothing else. A
//! [label](crate::guide::concepts_overview#reference-points-labels-and-anchors)
//! is the text an author writes as `\label{type:label}` to refer to a place
//! from elsewhere. Anything else written in the optional argument is reported
//! ([`BadLabelArgument`]; [`labels_of`] reads the argument), and the labels
//! written beside it still label the xnote. `\label` can be written in that
//! argument because the argument's declaration refers to the references
//! feature's scoped package
//! [`flm.refs.label`](crate::stdcore::features::refs::LABEL_PACKAGE): a
//! scoped package ([`ScopedPackageDef`](crate::lang::mint::ScopedPackageDef))
//! is a set of element declarations that can be written only inside the
//! environment bodies and arguments whose declarations refer to it. A
//! `\label` written after the xnote, as in `\footnote{…}\label{fn:x}`, is
//! therefore an unknown command, as it is in running text. The label type
//! (the part before the colon) must be the xnote kind's
//! [`label_type`](XnoteKind::label_type), `fn` for footnotes; the references
//! feature reports any other label type (`flm.refs.label-type-not-accepted`).
//!
//! An xnote may be written wherever inline content may be written: in a
//! paragraph, a heading's title, a list item, a `\text{…}` inside a formula,
//! the template of a user definition. Its commands are available in text
//! mode only, so a `\footnote` written directly in a formula is not an xnote:
//! it is passed to the formula renderer with the rest of the formula. An
//! xnote may be written inside another xnote's content, of its own xnote kind
//! or of another. It is then an xnote like any other: numbered, labeled and
//! listed in its xnote kind's list in document order, its mark placed where
//! it is written, inside the outer xnote's content. The `xnotes` feature
//! refuses no position; a feature that contributes an xnote kind and does not
//! accept its xnotes in some positions (inside another xnote, for instance)
//! reports them itself.
//!
//! # Where an xnote is shown
//!
//! The [`placement`](XnoteKind::placement) of an xnote kind
//! ([`XnotePlacement`]) says where its xnotes are shown:
//!
//! - A **listed** xnote leaves its **xnote mark** where it is written: a
//!   superscript (the xnote's number, for a numbered xnote kind) linked to the
//!   xnote. Its content is shown in the xnote kind's **list**, placed after
//!   the document's content. A listed xnote is inline content attached to the
//!   word before it: a space written before `\footnote` is removed.
//! - An **in-place** xnote is shown where it is written, in brackets, with its
//!   mark at the head: as inline content where its content is inline, and as
//!   a block of its own where its content holds paragraphs, interrupting its
//!   paragraph as a displayed formula does.
//!
//! The block-structure pass ([`crate::lang::rewrites::blocks`]) decides
//! whether an in-place xnote is a block, from the xnote kind and the node
//! ([`argument_holds_blocks`]), before the document's trees are frozen (no
//! longer changed; see [frozen
//! trees](crate::guide::concepts_overview#frozen-trees-and-computed-data)). The
//! placement is therefore part of the configuration, fixed when the registry
//! is built, and no render request can change it.
//!
//! The first xnote of the example above prints, on the plain-text output:
//!
//! ```text
//! The claim holds in every case.¹ The next sentence goes on.
//!
//! Footnotes
//! ---------
//!
//! ¹ Except the empty one, which we exclude by convention.
//! ```
//!
//! The general rules ([`XnoteRule`], [`ListRule`]) state the shapes that the
//! plain-text output receives and that the HTML rules build on; [`html`] and [`latex`] state what the HTML page and the
//! recomposed LaTeX write; the rendering guide shows the printed results
//! ([Xnotes](crate::guide::rendering#xnotes)).
//!
//! # What the feature registers
//!
//! The `xnotes` feature ([`XnotesFeature`], an
//! [`FlmFeature`]) registers, from its
//! configuration:
//!
//! - **The elements.** One element value of [`XNOTE`] (`flm.xnotes.xnote`)
//!   per configured xnote kind that has commands, carrying an
//!   [`XnoteElement`]; and four synthesized elements, which no document can
//!   write: the list of each xnote kind ([`list_element_name`],
//!   `flm.xnotes.<kind>.list`), an entry of a list ([`ENTRY`]), the content
//!   pointer an entry holds ([`CONTENT_POINTER`]), and the node that
//!   represents an xnote in content shown again ([`MARK_OF`]).
//! - **The service slot of xnote kinds**, [`KINDS_SLOT`] (`flm.xnotes.kinds`,
//!   declared with
//!   [`service_slot`](crate::core::registry::FeatureBuilder::service_slot)),
//!   which holds every xnote kind of the registry, in order. A service slot
//!   is the declaration, by the feature that owns an interaction, of a dotted
//!   name, the type of the values the slot takes and its arity; any feature
//!   may contribute a value to it ([`crate::core::services`]). The feature
//!   contributes its configured xnote kinds; another feature contributes an
//!   xnote kind of its own through [`contribute_kind`], the function that
//!   makes every declaration an xnote kind needs.
//! - **The table of xnotes**, [`XNOTES`] (`flm.xnotes`, an [`Xnotes`]): every
//!   xnote of the document, by xnote kind, in document order. The **collect
//!   stage**, [`COLLECT`] (`flm.xnotes.collect`), writes it from the table of
//!   reference points. It finds xnotes by the reference kind of their points,
//!   never by element, so the xnotes of a kind another feature contributes are
//!   collected like `\footnote` (see [`XnoteSource`]).
//! - **The list of each xnote kind.** A *piece*, a document-level
//!   [synthesized value](crate::guide::concepts_overview#synthesized-values)
//!   that a backend places beside the document's content
//!   ([`Piece`]), held in the table
//!   `flm.xnotes.list.<kind>` ([`piece_table`]) and built by the stage
//!   `flm.xnotes.build-list.<kind>` ([`ListBuilder`]); it is declared to the
//!   `pieces` feature as placed after the content and on by default.
//!   The documentation of [`contribute_kind`] lists every declaration made
//!   for a list.
//! - **For a numbered xnote kind, a counter and two counter styles**, declared
//!   to the numbering feature (see [Numbers, references and
//!   anchors](#numbers-references-and-anchors) below).
//! - **The lowering rules.** A lowering rule
//!   ([`LoweringRule`]) is written for
//!   one element and stages (builds into the lowered tree; see
//!   [`Lowerer`](crate::core::lowering::Lowerer)) its presentation for the
//!   backend being rendered; its default mark says on which backends it is
//!   chosen. The general rules are marked default on every backend; a
//!   backend with a rule of its own marked default there uses that rule
//!   instead ([`SelectedRules`](crate::core::plan::SelectedRules) states the
//!   rule selection order). The general rules, which a backend uses for
//!   every element that has no rule of its own there:
//!   - [`XNOTE_RULE`] ([`XnoteRule`]): an xnote becomes its mark for a listed
//!     xnote kind, and the bracketed xnote for an in-place one;
//!   - `flm.xnotes.<kind>.list.lower.default` ([`ListRule`], one per xnote
//!     kind): the list becomes a block holding the title as a heading and a
//!     labeled list, one entry per xnote, labeled with its mark;
//!   - [`POINTER_RULE`] ([`PointerRule`]): a content pointer becomes the
//!     content it refers to;
//!   - [`MARK_OF_RULE`] ([`MarkOfRule`]): the node representing an xnote in
//!     content shown again becomes the xnote's mark;
//!   - [`ENTRY_RULE`] ([`EntryRule`]): an entry outside its list becomes
//!     nothing and is reported.
//!
//!   The plain-text backend uses the general rules only. The `html` backend
//!   has rules of its own for the xnote and the list ([`html`]), which write
//!   the same shapes with no bracket text and with a link back from each
//!   entry to its mark. The `latex-recomposed` backend has rules of its own
//!   for every element but the entry ([`latex`]): an xnote becomes the
//!   command `\flmXN<kind>`, and the list, the content pointer and the node
//!   representing an xnote in content shown again become nothing.
//! - **The stylesheet** [`STYLE`] (`flm.xnotes.style`) of the HTML page
//!   ([`html::stylesheet`]).
//! - **The settings**: the fields of [`XnotesConfig`] written as data, one
//!   entry per xnote kind under the settings key `kinds`, keyed by the xnote
//!   kind's name ([`crate::core::settings`]).
//!
//! The [standard package](crate::stdcore::package) lists the feature, with
//! the configuration
//! [`StdcoreConfig::xnotes`](crate::stdcore::StdcoreConfig::xnotes).
//!
//! # Numbers, references and anchors
//!
//! Each xnote has two reference points
//! ([`RefPoint`]): places in the document
//! that can be referred to, numbered, or given an anchor, the identifier a
//! link points at (see [reference points, labels and
//! anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)).
//! [`ref_points`] returns the two:
//!
//! - **The xnote's own point**, whose reference kind is the xnote kind's name
//!   (`footnote`), carrying the xnote's labels. The xnote of a numbered xnote
//!   kind counts in the kind's counter, `flm.xnotes.<kind>`, which the
//!   [`numbering`](crate::stdcore::features::numbering) feature counts; its
//!   anchor is built from its number (`fn-1`, or `fn-2-1` for a kind numbered
//!   within sections), a `\ref` to it prints the kind's word and the number
//!   (`Footnote 1`), and `\ref[noprefix]` prints the number alone (`1`). The
//!   xnote of an unnumbered xnote kind takes its first label as its anchor,
//!   or else an anchor built from its position (`sn--1kzmq`); a `\ref` to it
//!   prints its content where the content is inline, and otherwise prints the
//!   label and is reported
//!   ([`NothingToShow`](crate::stdcore::features::refs::NothingToShow)).
//! - **The mark's point**, of the reference kind `<kind>-mark`
//!   (`footnote-mark`), which accepts no label and always takes an anchor
//!   built from its position (`fnref--1kzmq`): the place in the running text
//!   that the entry of a listed xnote links back to.
//!
//! The references feature's anchors stage computes the anchors (the table
//! [`ANCHORS`](crate::stdcore::features::refs::ANCHORS)), and the numbering
//! feature the numbers (the counter table
//! [`COUNTERS`]), as for every
//! reference point; the `xnotes` feature computes neither and stores nothing
//! derived from them.
//!
//! A counter is a dotted name under which things are counted in document
//! order, and a counter style says how a counter value is shown (see
//! [counters and counter
//! styles](crate::guide::concepts_overview#counters-and-counter-styles)). A
//! numbered xnote kind declares its counter ([`counter_name`]) and two counter
//! styles to the numbering feature, as peer contributions to that feature's
//! service slots (a peer contribution is dropped, with one Note, when the
//! feature that owns the slot is not registered): the reference style
//! `flm.xnotes.style.<kind>` ([`style_name`]: the xnote kind's words and
//! number format), and the mark style `flm.xnotes.style.<kind>.mark`
//! ([`mark_style_name`]: the xnote kind's mark template), which a host
//! replaces like any declared counter style. In a registry without the
//! numbering feature, the xnotes of a numbered kind have no number: a
//! reference prints the label, and the collect stage reports [`NoNumbering`]
//! once per xnote kind. An unnumbered xnote kind declares nothing to the
//! numbering feature.
//!
//! # Xnotes in content shown again
//!
//! Content that is shown a second time, away from where it is written (a
//! heading's title that a reference prints, an unnumbered float's caption, a
//! defined term, a list item's tag), holds no xnote. When such content is
//! copied to be shown again, the xnote's element replaces the xnote in the
//! copy, through its [`InCopy`] aspect, by a
//! node of the element [`MARK_OF`] ([`MarkOf`], [`copied_as`]), which
//! [`MarkOfRule`] writes as the xnote's mark, linked to the xnote and with no
//! anchor of its own (and as the marks of every xnote of a node that carries
//! several). The xnote itself is shown, listed and reported only where it is
//! written.
//!
//! # Xnotes carried by another feature's element
//!
//! An element of another feature can carry xnotes (for instance an element
//! one of whose arguments is a note). Four things make a node an xnote, and
//! the author's `\footnote` goes through the same four:
//!
//! 1. its xnote kind, contributed through [`contribute_kind`] (a kind with no
//!    command, since only the `xnotes` feature declares commands);
//! 2. the element's [`Referenceable`]
//!    aspect, returning the two points of [`ref_points`] per xnote;
//! 3. the element's [`XnoteSource`] aspect, returning per xnote the region of
//!    the node that holds its content;
//! 4. the element's [`ApartFromText`]
//!    aspect, including those regions ([`regions_apart_from_text`]).
//!
//! The element's lowering rules write each xnote with the public staging
//! helpers, so that it is written as `\footnote` is: [`stage_mark`] for a
//! listed xnote kind, [`stage_in_place`] for an in-place one, [`stage_latex`]
//! on the recomposed LaTeX. Each helper reads the table of xnotes by the
//! node's address and writes one xnote, identified by its position among the
//! node's xnotes of its kind; [`XnoteSource`] states how a node's xnotes are
//! paired with their contents. The element's [`InCopy`]
//! aspect returns [`copied_as`], so that its xnotes are shown in a copy as
//! `\footnote`'s are.
//!
//! # Interactions with other features
//!
//! - **`numbering`** numbers the xnotes of numbered kinds. Without it, those
//!   xnotes have no number, and [`NoNumbering`] is reported.
//! - **`refs`** declares `\label` and computes the anchors. Without it, the
//!   optional argument accepts no `\label`: every `\label` written there is
//!   refused like any other content (the first refused item of each xnote is
//!   reported as [`BadLabelArgument`]), and the marks carry no link and the
//!   entries no link back.
//! - **`refpoints`** writes the table of reference points
//!   ([`REFPOINTS`](crate::stdcore::features::refpoints::REFPOINTS)) from
//!   which the collect stage finds the xnotes.
//! - **`pieces`** declares the service slot the lists are declared to.
//!   Without it, no backend places a list. A render leaves one xnote kind's
//!   list out through the render options
//!   [`PiecesRenderOptions`](crate::stdcore::backends::pieces::PiecesRenderOptions),
//!   with the piece's name `flm.xnotes.list.<kind>`.
//! - **`lowering`, `html`, `latex` and `latex-recomposed`** own the
//!   presentation elements, the backends and the LaTeX constructs the rules
//!   use, and `numbering` and `refs` own the tables the rules read. The
//!   feature declares each name it uses with a peer declaration
//!   ([`PeerName`](crate::core::registry::PeerName)), so that it builds in a
//!   registry without those features: a rule that produces a presentation
//!   element or is the default on a backend that nobody declares is set
//!   aside (excluded from rule selection; see
//!   [`PeerName`](crate::core::registry::PeerName)), and a rule that reads a
//!   table optionally runs and finds the table absent.
//!
//! # Diagnostics
//!
//! Every condition is reported under `flm.xnotes.*`, and the documentation
//! of each states what the feature does after reporting it:
//! [`BadLabelArgument`], [`NoNumbering`], [`Empty`], [`PointerRefused`],
//! [`EntryOutsideList`], [`Unpaired`], and [`StageFailed`] (a stage of the
//! feature could not read or write a table it declared).
//!
//! # Map of the items
//!
//! - Configuration: [`XnotesConfig`], [`XnoteKind`], [`XnotePlacement`];
//!   [`XnotesFeature`] and [`feature`].
//! - Names: [`FEATURE`], [`XNOTE`], [`LABELS_ARG`], [`XNOTE_ARG`],
//!   [`ENTRY`], [`CONTENT_POINTER`], [`MARK_OF`], [`KINDS_SLOT`],
//!   [`COLLECT`], [`STYLE`], the rule names [`XNOTE_RULE`], [`ENTRY_RULE`],
//!   [`POINTER_RULE`], [`MARK_OF_RULE`], the roles and kind tokens
//!   [`MARK_ROLE`], [`INPLACE_ROLE`], [`ENTRY_MARK_ROLE`], [`KIND_XNOTES`],
//!   [`LINK_KIND`], [`BACKLINK_KIND`]; the heading name [`TITLE_HEADING`];
//!   the names per xnote
//!   kind [`list_element_name`], [`piece_name`], [`piece_table`],
//!   [`builder_name`], [`counter_name`], [`style_name`],
//!   [`mark_style_name`], [`list_rule_name`].
//! - The element and its aspects: [`XnoteElement`], [`XnotesListElement`],
//!   [`XnoteSource`], [`XnoteOf`], [`ref_points`],
//!   [`regions_apart_from_text`], [`labels_of`] and [`Rejected`],
//!   [`MarkOf`] and [`copied_as`].
//! - Declaring an xnote kind: [`contribute_kind`].
//! - The table and the stages: [`XNOTES`], [`Xnotes`], [`XnoteRecord`],
//!   [`ListBuilder`].
//! - Lowering: the rules [`XnoteRule`], [`ListRule`], [`PointerRule`],
//!   [`MarkOfRule`], [`EntryRule`]; the staging helpers [`stage_mark`],
//!   [`stage_in_place`], [`stage_mark_text`], [`append_to_last_paragraph`],
//!   [`Brackets`]; the backends' rules in [`html`], [`latex`]
//!   ([`stage_latex`]) and [`latex_rendered`], which also holds the
//!   definition piece of the rendered LaTeX.
//! - Diagnostics: [`BadLabelArgument`], [`NoNumbering`], [`Empty`],
//!   [`PointerRefused`], [`EntryOutsideList`], [`Unpaired`],
//!   [`StageFailed`].

mod config;
mod diagnostics;
mod kinds;
mod labels;
mod list;
mod lower;
mod mark_of;
mod marks;
mod pointer;
mod stages;
pub mod html;
pub mod latex;
pub mod latex_rendered;

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::core::settings::SettingsDecl;
use crate::core::data::table::TableHandle;
use crate::core::lowering::{DefaultMark, LoweringDecl, LoweringRule, NodeRegion};
use crate::core::piece::Piece;
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use crate::lang::rewrites::blocks::argument_holds_blocks;
use crate::lang::Flm;
use crate::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, ApartFromText, BlockRole, BlockRoleAspect, ContentRef, FixedBlockRole,
    InCopy, InlineSpacing, NumberSource, RefPoint, Referenceable,
};
use crate::model::element::ElementSpec;
use crate::model::names::{ElementName, Name, RefKind, StageName};
use crate::stdcore::backends::pres;
use crate::stdcore::features::numbering::COUNTERS;
use crate::stdcore::features::refs::{self, LabelUse};

pub use config::{XnoteKind, XnotePlacement, XnotesConfig};
pub use diagnostics::{
    BadLabelArgument, Empty, EntryOutsideList, NoNumbering, PointerRefused, Unpaired,
};
pub use kinds::contribute_kind;
pub use labels::{labels_of, Rejected};
pub use list::{list_rule_name, ListRule, TITLE_HEADING};
pub use lower::{
    append_to_last_paragraph, stage_in_place, stage_mark, stage_mark_text, Brackets, EntryRule,
    XnoteRule,
};
pub use mark_of::{copied_as, MarkOf, MarkOfRule};
pub use pointer::PointerRule;
pub use stages::{ListBuilder, StageFailed, XnoteRecord, Xnotes, XNOTES};
pub use latex::stage_latex;

/// The feature's registered name, `xnotes`.
pub const FEATURE: &str = "xnotes";

/// The declaration of the feature's settings: the fields of [`XnotesConfig`]
/// are its settings keys, and each xnote kind is one entry of the map `kinds`,
/// keyed by its name. See [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<XnotesConfig>(
            "The kinds of xnote (notes set apart from the running text, such as footnotes): \
            for each kind, its commands, the words a reference writes, its label type, whether \
            it is numbered, its number and mark templates, where its notes are shown, how its \
            content is laid out, the title of its list, the text of the link back to the mark \
            and the LaTeX command it is handed to. Fixed when the registry is built: it cannot \
            vary per document or per render.",
        )
        .merge_entries("kinds", || crate::core::settings::named::to_data(&XnotesConfig::default().kinds, "name"))
        .markup("kinds.*.words")
        .markup("kinds.*.title")
        .markup("kinds.*.backlink")
        .markup("kinds.*.number_format")
        .markup("kinds.*.mark")
        .markup("kinds.*.within.joiner.joined")
        .markup("kinds.*.within.joiner.template")
}

/// The name of the element of an xnote an author writes, `flm.xnotes.xnote`:
/// one element value per configured xnote kind that has commands, each
/// carrying an [`XnoteElement`].
pub const XNOTE: &str = "flm.xnotes.xnote";

/// The name of an xnote's optional argument, `labels`, which holds the
/// `\label{…}` commands that label the xnote ([`labels_of`] reads them).
pub const LABELS_ARG: &str = "labels";

/// The name of an xnote's mandatory argument, `xnote`, which holds the
/// xnote's content.
pub const XNOTE_ARG: &str = "xnote";

/// The name of the element of one entry of a list of xnotes,
/// `flm.xnotes.entry`: a synthesized element, which no document can write,
/// with one element value. [`ListBuilder`] creates one entry per xnote.
pub const ENTRY: &str = "flm.xnotes.entry";

/// The name of the element of a content pointer, `flm.xnotes.content-pointer`:
/// a synthesized element, which no document can write, with one element
/// value.
///
/// A node of this element is placed in an entry of a list of xnotes, in the
/// place of the xnote's content. It carries a
/// [`ContentPointer`](crate::stdcore::backends::lowering::ContentPointer): the
/// payload of a synthesized node that shows, where the node is placed,
/// content written elsewhere in the tree without copying it, here the address
/// of the node carrying the xnote and the region of that node that holds the
/// content. The list shows the content by lowering that region where the
/// pointer is placed ([`PointerRule`], [`ListRule`]).
pub const CONTENT_POINTER: &str = "flm.xnotes.content-pointer";

/// The name of the element of the node that represents an xnote in content
/// shown again, `flm.xnotes.mark-of`: a synthesized element, which no
/// document can write, with one element value.
///
/// When content is copied to be shown again (a heading's title that a
/// reference prints), the copy holds a node of this element in the place of
/// each node carrying xnotes: the [`InCopy`] aspect of [`XnoteElement`], and
/// of any element that implements it with [`copied_as`], returns it. The node
/// has no children and carries a [`MarkOf`], which identifies the xnotes it
/// represents; [`MarkOfRule`] writes it as their marks.
pub const MARK_OF: &str = "flm.xnotes.mark-of";

/// The name of the general lowering rule of the element [`MARK_OF`],
/// `flm.xnotes.mark-of.lower.default` ([`MarkOfRule`]): the node becomes the
/// mark of each xnote it represents, linked to the xnote, with no anchor.
pub const MARK_OF_RULE: &str = "flm.xnotes.mark-of.lower.default";

/// The name of the general lowering rule of the element [`CONTENT_POINTER`],
/// `flm.xnotes.content-pointer.lower.default` ([`PointerRule`]): the pointer
/// becomes the content it refers to, lowered where the pointer is placed.
pub const POINTER_RULE: &str = "flm.xnotes.content-pointer.lower.default";

/// The name of the service slot of xnote kinds, `flm.xnotes.kinds`: it holds
/// every xnote kind of the registry, in order, each an [`XnoteKind`].
///
/// The `xnotes` feature declares the slot (any number of values, in
/// contribution order) and contributes its configured kinds;
/// [`contribute_kind`] contributes any other ([`crate::core::services`]
/// describes service slots). The collect stage reads it, and so does a build
/// hook of the feature that compiles, once the registry is built, the mark of
/// every unnumbered xnote kind (its [`mark`](XnoteKind::mark) template, which
/// holds no number and so writes the same text beside every xnote).
pub const KINDS_SLOT: &str = "flm.xnotes.kinds";

/// The name of the collect stage, `flm.xnotes.collect`, which writes the table
/// [`XNOTES`] (see [`Xnotes`] for what it records).
pub const COLLECT: &str = "flm.xnotes.collect";

/// The role of the superscript span around an xnote's mark in the running
/// text, `flm.xnotes.xnote.mark`.
pub const MARK_ROLE: &str = "flm.xnotes.xnote.mark";

/// The role of the span or the block holding an in-place xnote,
/// `flm.xnotes.xnote.inplace`.
pub const INPLACE_ROLE: &str = "flm.xnotes.xnote.inplace";

/// The role of the mark beside an entry of a list of xnotes,
/// `flm.xnotes.entry.mark`.
pub const ENTRY_MARK_ROLE: &str = "flm.xnotes.entry.mark";

/// The kind token `xnotes`, carried by the block holding a list of xnotes, by
/// the presentation list inside it and by an in-place xnote; the xnote kind's
/// name is the kind token after it. (A kind token is a feature-owned name on a
/// presentation payload that a stylesheet or a lowering rule selects on; see
/// [`pres::payload`].)
pub const KIND_XNOTES: &str = "xnotes";

/// The kind token of the link from a mark to its xnote, `xnote`.
pub const LINK_KIND: &str = "xnote";

/// The kind token of the link from the entry of a listed xnote back to its
/// mark, `xnote-back` (written on the HTML page only; see [`html`]).
pub const BACKLINK_KIND: &str = "xnote-back";

/// The feature's style identifier, `flm.xnotes.style`, which the lowering of
/// an xnote and of a list of xnotes requires on every backend but the
/// recomposed LaTeX: the stylesheet [`html::stylesheet`] on the HTML page.
pub const STYLE: &str = "flm.xnotes.style";

/// The name of the general lowering rule of the element [`XNOTE`],
/// `flm.xnotes.xnote.lower.default` ([`XnoteRule`]): an xnote becomes its
/// mark for a listed xnote kind, and the xnote in place for an in-place one.
pub const XNOTE_RULE: &str = "flm.xnotes.xnote.lower.default";

/// The name of the lowering rule of the element [`ENTRY`],
/// `flm.xnotes.entry.lower.default` ([`EntryRule`]): an entry lowered outside
/// its list becomes nothing and is reported.
pub const ENTRY_RULE: &str = "flm.xnotes.entry.lower.default";

/// The name of the list element of the xnote kind `kind`,
/// `flm.xnotes.<kind>.list`: a synthesized element, which no document can
/// write, with one element value per xnote kind, carrying an
/// [`XnotesListElement`].
pub fn list_element_name(kind: &str) -> ElementName {
    ElementName::from(format!("flm.xnotes.{kind}.list"))
}

/// The name of the piece holding the list of the xnote kind `kind`, which is
/// also the name of the table holding it: `flm.xnotes.list.<kind>`.
pub fn piece_name(kind: &str) -> Name {
    Name::from(format!("flm.xnotes.list.{kind}"))
}

/// The table of the list of the xnote kind `kind`, `flm.xnotes.list.<kind>`
/// ([`piece_name`]): the table that the kind's builder ([`ListBuilder`])
/// writes, and from which the backends place the list after the content.
///
/// The rest of the pipeline reads the list from this table only, so the
/// stage that writes the table decides the list. To build one xnote kind's
/// list differently, register a document-scope stage of your own that writes
/// this table (from the table [`XNOTES`], the document's trees, or any other
/// table it declares that it reads), and prefer it in a plan request
/// ([`Overrides::prefer`](crate::core::plan::Overrides::prefer)) or in a
/// workflow of your own
/// ([`WorkflowDef::prefer`](crate::core::registry::WorkflowDef::prefer)).
/// Without a preference, the plan refuses to choose between the two stages
/// and reports [`PlanError::Contested`](crate::core::plan::PlanError::Contested),
/// which lists both. The lists of the other xnote kinds are not affected.
pub fn piece_table(kind: &str) -> TableHandle<Piece> {
    TableHandle::new(piece_name(kind))
}

/// The name of the stage that builds the list of the xnote kind `kind`,
/// `flm.xnotes.build-list.<kind>` ([`ListBuilder`]).
pub fn builder_name(kind: &str) -> StageName {
    StageName::from(format!("flm.xnotes.build-list.{kind}"))
}

/// The name of the counter in which the xnotes of the numbered xnote kind
/// `kind` are counted, `flm.xnotes.<kind>` (see
/// [`CounterDecl`](crate::stdcore::features::numbering::CounterDecl)).
pub fn counter_name(kind: &str) -> Name {
    Name::from(format!("flm.xnotes.{kind}"))
}

/// The name of the reference style of the numbered xnote kind `kind`,
/// `flm.xnotes.style.<kind>`: the counter style
/// ([`CounterStyleDecl`](crate::stdcore::features::numbering::CounterStyleDecl))
/// in which a `\ref` writes an xnote's number, with the kind's words
/// (`Footnote~1`).
pub fn style_name(kind: &str) -> Name {
    Name::from(format!("flm.xnotes.style.{kind}"))
}

/// The name of the mark style of the numbered xnote kind `kind`,
/// `flm.xnotes.style.<kind>.mark`: the counter style in which an xnote's mark
/// writes its number (`1`), made from the kind's
/// [`mark`](XnoteKind::mark) template.
pub fn mark_style_name(kind: &str) -> Name {
    Name::from(format!("flm.xnotes.style.{kind}.mark"))
}

/// The reference kind of the mark's point of an xnote of the xnote kind
/// `kind`, `<kind>-mark`.
fn mark_point_kind(kind: &str) -> RefKind {
    RefKind::from(format!("{kind}-mark"))
}

/// The per-value data of the [`XNOTE`] element: the xnote kind that a command
/// writes.
///
/// The value also implements the element's five aspects: its block role
/// ([`BlockRoleAspect`]: inline content for a listed xnote kind; inline or a
/// block of its own for an in-place one), its reference points
/// ([`Referenceable`], through [`ref_points`]), the content of its xnote
/// ([`XnoteSource`]: the argument [`XNOTE_ARG`]), the content it sets apart
/// from the text around it ([`ApartFromText`]), and what replaces it in
/// content shown again ([`InCopy`]: a node of the element [`MARK_OF`]).
#[derive(Debug, Clone)]
pub struct XnoteElement {
    /// The xnote kind.
    pub kind: Arc<XnoteKind>,
}

impl PartialEq for XnoteElement {
    /// Two values are equal when their xnote kinds are equal.
    fn eq(&self, other: &XnoteElement) -> bool {
        self.kind == other.kind
    }
}

impl Eq for XnoteElement {}

/// The per-value data of the list element of an xnote kind
/// ([`list_element_name`]): the xnote kind whose xnotes the list holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XnotesListElement {
    /// The xnote kind.
    pub kind: Arc<XnoteKind>,
}

/// The aspect an element implements when its nodes carry xnotes: for one
/// node, which xnote kinds it carries and which region of the node holds the
/// content of each.
///
/// The content of an xnote is always a region of the node itself, one of its
/// arguments or one of its bodies (a slot of the node; [`NodeRegion`]), in
/// whichever tree the node
/// is placed. The list of the xnote kind shows the content by lowering that
/// region where the entry is placed ([`CONTENT_POINTER`]); nothing of it is
/// copied. The [`XNOTE`] element returns one xnote, whose content is its
/// argument [`XNOTE_ARG`]. An element of another feature returns the regions
/// of its own node that hold the contents of its xnotes; the collect stage
/// records the node where it is placed in the document.
///
/// This aspect is one of the four things that make a node an xnote; the
/// others are the xnote kind in the service slot [`KINDS_SLOT`]
/// ([`contribute_kind`]), the two reference points of each xnote
/// ([`ref_points`], returned through [`Referenceable`]), and the element's
/// [`ApartFromText`] aspect, which includes the regions the xnotes are made
/// from ([`regions_apart_from_text`]) so that their words are not read as
/// part of the text around them (for instance by the anchor a heading builds
/// from its title). The author's `\footnote` goes through the same four.
///
/// The content of an xnote must be a region of a node of the document. An
/// xnote whose content a feature builds itself, as a synthesized value rather
/// than as a region of a node, is not implemented: the list can point only
/// into the document's trees and into the synthesized values that the
/// lowering of the list holds.
///
/// # Several xnotes on one node
///
/// A node may carry several xnotes, of one xnote kind or of several. Its
/// element returns one [`XnoteOf`] per xnote here, and one pair of points per
/// xnote through its [`Referenceable`] aspect, calling [`ref_points`] once
/// per xnote. **The two lists are paired by order, xnote kind by xnote
/// kind**: the node's n-th pair of points of a kind (the n-th record of that
/// kind that the table [`XNOTES`] holds for the node,
/// [`Xnotes::records_at`]) is the xnote whose content is the n-th [`XnoteOf`]
/// of that kind returned here. Every reader of a node's xnotes follows this
/// pairing: the collect stage records the region of each xnote's content with
/// it
/// ([`XnoteRecord::content`]); the staging helpers take the position of the
/// xnote they write among the node's xnotes of its kind ([`stage_mark`],
/// [`stage_in_place`], [`stage_latex`]); and the node that represents the
/// node in content shown again represents every one of its xnotes
/// ([`MarkOf`]). A node whose two lists hold different numbers of xnotes of
/// a kind is reported ([`Unpaired`]). Each [`XnoteOf`] of a node must refer
/// to a region of its own: the list of an xnote kind finds an entry's xnote
/// by the node and the region that the entry's content pointer refers to
/// ([`ContentPointer`](crate::stdcore::backends::lowering::ContentPointer)).
///
/// The element decides which labels label which xnote, through the labels it
/// passes to each [`ref_points`] call: the labels of
/// `\footnote[\label{…}]{…}` label the single xnote of its node.
pub trait XnoteSource: Send + Sync {
    /// The xnotes that `node` carries, in order: for each, its xnote kind
    /// and the region of `node` that holds its content.
    fn xnotes(&self, node: &NodeRef<'_, Flm>) -> Vec<XnoteOf>;
}

/// The regions holding the contents of the xnotes of the xnote kind `kind`
/// that `node` carries, in the order the node's [`XnoteSource`] returns them:
/// the n-th is the content of the node's n-th xnote of that kind, under the
/// pairing that aspect states. Empty for a node whose element does not
/// implement the aspect.
fn contents_of_kind(node: &NodeRef<'_, Flm>, kind: &str) -> Vec<NodeRegion> {
    let Some(source) = node.ext().element.aspect::<dyn XnoteSource>() else { return Vec::new() };
    source.xnotes(node).into_iter().filter(|xnote| xnote.kind.as_str() == kind).map(|xnote| xnote.content).collect()
}

/// One xnote that a node carries, as [`XnoteSource`] returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XnoteOf {
    /// The name of the xnote kind.
    pub kind: Name,
    /// The region of the node that holds the xnote's content: the argument
    /// `xnote` for `\footnote` ([`NodeRegion::argument_named`] finds an
    /// argument's position from its name).
    pub content: NodeRegion,
}

/// The two reference points of one xnote of the xnote kind `kind`, labeled
/// by `labels` and showing `content` where a reference shows content.
///
/// Every element that carries xnotes implements its [`Referenceable`] aspect
/// with this function, called once per xnote, so that the collect stage finds
/// its xnotes: the stage recognizes an xnote by these two points. `labels`
/// are the labels the author wrote for the xnote, as [`labels_of`] or the
/// references feature's readers
/// ([`label_uses_in`](crate::stdcore::features::refs::label_uses_in)) read
/// them.
///
/// The function returns the two points in this order:
///
/// 1. **The xnote's own point**, whose reference kind is the xnote kind's
///    name. It carries the labels and accepts the kind's
///    [`label_type`](XnoteKind::label_type) alone. For a numbered xnote kind,
///    the point counts in the kind's counter ([`counter_name`]), advancing it
///    by one, and is shown in the reference style ([`style_name`]); its anchor
///    is built from its number, `fn-1` (from its first label where it has no
///    number), and it shows no content. For an unnumbered xnote kind, the
///    point counts in no counter, its anchor is its first label or else is
///    built from its position (`fn--3kzmq`), and a reference to it shows
///    `content`.
/// 2. **The mark's point**, of the reference kind `<kind>-mark`: no label,
///    accepting none, no number, no content; its anchor is built from its
///    position, with the prefix `<label type>ref` (`fnref--1kzmq`).
///
/// `content` is the xnote's content where a reference can show it: inline
/// content only, since a reference is itself inline. Pass `None` for content
/// that holds blocks; a reference to such an unnumbered xnote then shows the
/// label it was written with.
///
/// The points are returned wherever the node is placed, inside another
/// xnote's content too: an xnote inside an xnote is an xnote like any other.
pub fn ref_points(
    kind: &XnoteKind,
    labels: &[LabelUse],
    content: Option<ContentRef>,
) -> Vec<RefPoint> {
    let name = kind.name.as_str();
    let (labels, malformed) = refs::point_labels(labels);
    let xnote = RefPoint {
        kind: RefKind::from(name),
        labels,
        malformed,
        computed: 0,
        anchor: if kind.numbered {
            AnchorPolicy::FromValue { prefix: kind.label_type.clone() }
        } else {
            AnchorPolicy::Auto { prefix: kind.label_type.clone() }
        },
        number: kind.numbered.then(|| NumberSource {
            counter: counter_name(name),
            instances: Vec::new(),
            style: style_name(name),
            advance: vec![0],
            number_format: None,
        }),
        content: if kind.numbered { None } else { content },
        accepts: AcceptedRefTypes::these([kind.label_type.as_str()]),
    };
    let mark = RefPoint {
        kind: mark_point_kind(name),
        labels: Vec::new(),
        malformed: Vec::new(),
        computed: 0,
        anchor: AnchorPolicy::Auto { prefix: format!("{}ref", kind.label_type) },
        number: None,
        content: None,
        accepts: AcceptedRefTypes::these(Vec::<String>::new()),
    };
    vec![xnote, mark]
}

/// Whether the content argument of the xnote `node` holds blocks, as the
/// block-structure pass laid it out.
fn holds_blocks(node: &NodeRef<'_, Flm>) -> bool {
    matches!(
        NodeRegion::argument_named(node, XNOTE_ARG),
        Some(NodeRegion::Argument(index)) if argument_holds_blocks(node, index)
    )
}

/// A listed xnote is inline content that attaches to the word before it (a
/// space before it is removed); an in-place xnote is inline content where its
/// content is inline, and a block where the block-structure pass laid its
/// content out as blocks.
impl BlockRoleAspect for XnoteElement {
    fn block_role(&self, node: &NodeRef<'_, Flm>) -> BlockRole {
        match self.kind.placement {
            XnotePlacement::List => {
                BlockRole::Inline(InlineSpacing { absorb_before: true, absorb_after: false })
            }
            XnotePlacement::InPlace if holds_blocks(node) => BlockRole::Block,
            XnotePlacement::InPlace => BlockRole::Inline(InlineSpacing::CONTENT),
        }
    }
}

/// The two points of [`ref_points`], labeled by the `\label` commands of the
/// optional argument ([`labels_of`]; anything else the argument holds is
/// ignored here and reported by the xnote's lowering rule), and showing the
/// content argument where it is inline.
impl Referenceable for XnoteElement {
    fn points(&self, node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        let (labels, _) = labels_of(node);
        let content = (!holds_blocks(node)).then(|| ContentRef::ArgumentNamed(XNOTE_ARG.to_string()));
        ref_points(&self.kind, &labels, content)
    }
}

/// One xnote of the element's xnote kind, whose content is the argument
/// [`XNOTE_ARG`].
impl XnoteSource for XnoteElement {
    fn xnotes(&self, node: &NodeRef<'_, Flm>) -> Vec<XnoteOf> {
        NodeRegion::argument_named(node, XNOTE_ARG)
            .map(|content| XnoteOf { kind: self.kind.name.clone(), content })
            .into_iter()
            .collect()
    }
}

/// The regions that `node`'s xnotes are made from, as the node's
/// [`XnoteSource`] returns them, written as [content references](ContentRef).
///
/// An element that carries xnotes returns these from its [`ApartFromText`]
/// aspect, together with anything else of its own that is apart from the
/// text (such as the labels of `\footnote`). The element thus states its
/// xnotes' regions once, in its [`XnoteSource`] implementation, and the two
/// aspects cannot disagree.
///
/// An argument region becomes [`ContentRef::Argument`] at the same position;
/// a slot region becomes each of the slot's content nodes, as
/// [`ContentRef::Node`]. The result is empty for a node whose element does
/// not implement [`XnoteSource`].
pub fn regions_apart_from_text(node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
    let Some(source) = node.ext().element.aspect::<dyn XnoteSource>() else { return Vec::new() };
    let mut apart = Vec::new();
    for xnote in source.xnotes(node) {
        match xnote.content {
            NodeRegion::Argument(index) => apart.push(ContentRef::Argument(index)),
            NodeRegion::Slot(_) => apart.extend(
                xnote.content.content_nodes(node).unwrap_or_default().iter().map(|n| ContentRef::Node(n.id())),
            ),
        }
    }
    apart
}

/// Nothing of an xnote is part of the text around it: its content, the
/// argument [`XNOTE_ARG`], is shown apart from the words it is attached to (at
/// the foot of the page, in a list, beside them), and its labels, the
/// argument [`LABELS_ARG`], identify it. A heading whose title holds an xnote
/// therefore builds its anchor from the title's own words. The content
/// regions are the ones the element's [`XnoteSource`] implementation returns
/// ([`regions_apart_from_text`]).
impl ApartFromText for XnoteElement {
    fn apart_from_text(&self, node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        let mut apart = vec![ContentRef::ArgumentNamed(LABELS_ARG.to_string())];
        apart.extend(regions_apart_from_text(node));
        apart
    }
}

/// What every rule of the element [`XNOTE`] reads and produces, except the
/// recomposition rule: the table of xnotes, the counter and anchors tables
/// optionally, and the presentation elements that [`stage_mark`] and
/// [`stage_in_place`] stage.
fn xnote_decl() -> LoweringDecl {
    LoweringDecl::new()
        .reads(XNOTES)
        .reads_opt(COUNTERS)
        .reads_opt(refs::ANCHORS)
        .produces(pres::SPAN)
        .produces(pres::LINK)
        .produces(pres::BLOCK)
        .produces(pres::PAR)
}

/// The element value of the commands of the xnote kind `kind`.
fn xnote_element(kind: Arc<XnoteKind>) -> Arc<ElementSpec> {
    let value = Arc::new(XnoteElement { kind });
    Arc::new(
        ElementSpec::new(XNOTE)
            .docs(format!("`\\{}`: a note of the xnote kind `{}`.", value.kind.commands.join("`, `\\"), value.kind.name))
            .with::<dyn BlockRoleAspect>(Arc::clone(&value) as Arc<dyn BlockRoleAspect>)
            .with::<dyn Referenceable>(Arc::clone(&value) as Arc<dyn Referenceable>)
            .with::<dyn XnoteSource>(Arc::clone(&value) as Arc<dyn XnoteSource>)
            .with::<dyn ApartFromText>(Arc::clone(&value) as Arc<dyn ApartFromText>)
            .with::<dyn InCopy>(Arc::clone(&value) as Arc<dyn InCopy>)
            .with_value(value),
    )
}

/// The element value of an entry of a list of xnotes: a block of its own.
fn entry_element() -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(ENTRY)
            .docs("One entry of a list of xnotes, holding a pointer to the xnote's content.")
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Block))),
    )
}

/// The `xnotes` feature, an [`FlmFeature`]: footnotes and the other xnote
/// kinds.
///
/// From its [`XnotesConfig`], it declares the service slot of xnote kinds
/// [`KINDS_SLOT`], every configured xnote kind through [`contribute_kind`],
/// one element value of [`XNOTE`] per xnote kind that has commands, the
/// synthesized elements [`ENTRY`], [`MARK_OF`] and [`CONTENT_POINTER`], the
/// table [`XNOTES`] and the collect stage that writes it, the lowering rules
/// of those elements on every backend, and the stylesheet [`STYLE`]. The
/// [module documentation](self) describes what each does.
///
/// Registering the feature fails with [`BuildError::Feature`] when the
/// configuration fails [`XnotesConfig::validate`].
#[derive(Debug, Clone, Default)]
pub struct XnotesFeature {
    config: XnotesConfig,
}

impl XnotesFeature {
    /// The feature with the configuration `config`. A feature's configuration
    /// is the plain Rust value the feature is constructed from; it is fixed
    /// when the registry is built and cannot vary per document or per render
    /// (see [settings, options and
    /// configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
    pub fn new(config: XnotesConfig) -> XnotesFeature {
        XnotesFeature { config }
    }

    /// The configuration this feature was constructed with.
    pub fn config(&self) -> &XnotesConfig {
        &self.config
    }
}

impl FlmFeature for XnotesFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Xnotes").docs(
            "Footnotes and the other kinds of xnote (notes set apart from the running text): \
             the configured kinds and their commands, the two reference points of every xnote, \
             the service slot `flm.xnotes.kinds` of the kinds, and the table `flm.xnotes` of \
             every xnote of a document by kind.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        self.config
            .validate()
            .map_err(|detail| BuildError::Feature { feature: owner.clone(), detail })?;
        b.service_slot::<XnoteKind>(
            KINDS_SLOT,
            crate::core::services::Arity::OrderedMany,
            "the kinds of xnote, in order: the ones the `xnotes` feature's configuration \
             declares and the ones other features contribute",
        )?;
        // The label package belongs to the references feature, which may be
        // absent: the optional argument then accepts no `\label`. Declared
        // once here, not per kind.
        // See decision rationale at [§flm-dd-dr:std-xnote-label-peer].
        b.peer_scoped_package(
            refs::LABEL_PACKAGE,
            "the `\\label` an xnote's optional argument accepts",
        )?;
        for kind in &self.config.kinds {
            let kind = Arc::new(kind.clone());
            contribute_kind(b, Arc::clone(&kind))?;
            if kind.commands.is_empty() {
                continue;
            }
            b.element(
                ElementDef::new(
                    owner.clone(),
                    kind.commands.iter().map(String::as_str),
                    SyntaxDef::Macro {
                        args: vec![
                            ArgDef::optional(LABELS_ARG)
                                .scoped(refs::LABEL_PACKAGE)
                                .docs("The xnote's labels, `\\label{…}`, and nothing else."),
                            ArgDef::mandatory(XNOTE_ARG)
                                .layout(kind.layout)
                                .docs("The xnote's content."),
                        ],
                    },
                    xnote_element(Arc::clone(&kind)),
                )
                .docs(format!(
                    "`\\{}[\\label{{{}:…}}]{{…}}`: a note of the xnote kind `{}`.",
                    kind.commands[0], kind.label_type, kind.name
                )),
            )?;
        }
        b.element(
            ElementDef::new(owner.clone(), Vec::<&str>::new(), SyntaxDef::Synthesized, entry_element())
                .docs(
                    "One entry of a list of xnotes. It is never written in a document: a kind's \
                     list builder creates it.",
                ),
        )?;
        b.element(
            ElementDef::new(
                owner.clone(),
                Vec::<&str>::new(),
                SyntaxDef::Synthesized,
                Arc::clone(mark_of::mark_of_spec().element()),
            )
            .docs(
                "The mark of an xnote in content shown again, such as a heading's title that a \
                 reference prints: it refers to the xnote and is written as its mark. It is never \
                 written in a document: a copy of the content creates it in place of the xnote.",
            ),
        )?;
        b.element(
            ElementDef::new(
                owner.clone(),
                Vec::<&str>::new(),
                SyntaxDef::Synthesized,
                pointer::pointer_element(),
            )
            .docs(
                "The content of an xnote, shown where this node is placed: it refers to the node \
                 carrying the xnote and the region of that node that holds the content. It is \
                 never written in a document: a kind's list builder creates it.",
            ),
        )?;
        // The presentation elements belong to the lowering feature, which a
        // registry that renders nothing may leave out: the rules are then set
        // aside. The counter table belongs to the numbering feature and the
        // anchors table to the references feature, either of which may be
        // absent. `contribute_kind` declares the same names again for each
        // kind's list rule, which the registry merges.
        // See decision rationale at [§flm-dd-dr:std-xnote-pres-peers].
        list::declare_peers(b)?;
        // The element `flm.xnotes.xnote` exists only where a kind has a
        // command, and a rule names a registered element.
        if self.config.kinds.iter().any(|kind| !kind.commands.is_empty()) {
            b.lowering_rule(
                LoweringRule::new(
                    XNOTE_RULE,
                    XNOTE,
                    xnote_decl(),
                    Arc::new(XnoteRule),
                    DefaultMark::Everywhere,
                )
                .docs(
                    "an xnote lowers to its mark, a superscript `flm.pres.span` holding a link to \
                     the xnote, for a listed xnote kind; to a `flm.pres.span` or a \
                     `flm.pres.block` holding the mark and the content, bracketed, for an in-place \
                     xnote kind",
                ),
            )?;
        }
        b.lowering_rule(
            LoweringRule::new(ENTRY_RULE, ENTRY, LoweringDecl::new(), Arc::new(EntryRule), DefaultMark::Everywhere)
                .docs("an entry of a list of xnotes outside its list lowers to nothing and is reported"),
        )?;
        b.lowering_rule(pointer::pointer_rule())?;
        b.lowering_rule(mark_of::mark_of_rule())?;
        b.table(
            XNOTES,
            "Every xnote of the document, by xnote kind, in document order: its reference point \
             and the reference point of its mark.",
        )?;
        // An unnumbered kind's fixed mark is compiled once the kinds slot is
        // assembled and the language built, and carried by the collect stage
        // into the table.
        let fixed_marks = Arc::new(marks::FixedMarks::default());
        b.finalizer(Arc::new(marks::DeclareFixedMarks { marks: Arc::clone(&fixed_marks) }))?;
        b.stage(Arc::new(stages::Collect { marks: fixed_marks }))?;
        let with_element = self.config.kinds.iter().any(|kind| !kind.commands.is_empty());
        html::register(b, with_element)?;
        latex_rendered::register(b, with_element)?;
        latex::register(b, with_element)?;
        b.digest_input(&self.config.digest_bytes())?;
        Ok(())
    }
}

/// The [`XnotesFeature`] with the default configuration (the single xnote kind
/// `footnote`), ready to be listed in a package entry.
///
/// Use `Arc::new(XnotesFeature::new(config))` to list the feature under an
/// [`XnotesConfig`] of your own.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(XnotesFeature::default())
}
