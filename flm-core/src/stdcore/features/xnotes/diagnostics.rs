//! The conditions that the `xnotes` feature reports, each under the
//! identifier `flm.xnotes.<condition>`.
//!
//! A condition is the typed body of a diagnostic: a value that says what
//! happened and holds the facts describing it (see
//! [diagnostics](crate::guide::concepts_overview#diagnostics)). The
//! documentation of each condition states who reports it, where, and what the
//! feature does to go on after reporting it, its recovery (see
//! [`crate::stdcore::backends::lowering::error`]); the message itself states
//! the problem only.
//!
//! Every condition is reported as an Error, except [`Empty`], a Warning: an
//! xnote with no content is still written. The conditions are reported by:
//!
//! - the collect stage ([`COLLECT`](super::COLLECT)): [`NoNumbering`], once
//!   per numbered xnote kind that has an xnote, in a run of a registry
//!   without the numbering feature; and [`Unpaired`], once per node and
//!   xnote kind;
//! - the lowering rules of an xnote: [`BadLabelArgument`] and [`Empty`];
//! - the lowering rule of a list entry: [`EntryOutsideList`];
//! - the rules that lower a content pointer (the pointer's own rule and the
//!   list's rule): [`PointerRefused`].
//!
//! The condition [`StageFailed`](super::StageFailed), which any stage of the
//! feature may report, is defined with the stages.

use techy::error::DiagnosticInfo;

/// Condition `flm.xnotes.bad-label-argument`: the optional argument of an
/// xnote holds something other than `\label{…}` commands, whitespace and
/// comments, such as text, a group, `\textbf{…}`, or a command that no
/// feature defines there.
///
/// The lowering rule of the xnote reports it once per xnote, at the xnote,
/// with the first such thing ([`labels_of`](super::labels_of) finds them).
/// Recovery: what was refused is ignored, and the labels written beside it
/// still label the xnote.
///
/// In a registry without the references feature, the optional argument
/// accepts no `\label` at all: a `\label` written there is refused like
/// anything else, and is the item reported when it comes first.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.xnotes.bad-label-argument",
    message = "the optional argument of this {kind} may hold only \\label{{…}} commands, but it \
               holds ‘{found}’"
)]
pub struct BadLabelArgument {
    /// The name of the xnote's kind, `footnote`.
    pub kind: String,
    /// The first refused thing, as written.
    pub found: String,
}

/// Condition `flm.xnotes.no-numbering`: a document holds xnotes of a
/// numbered xnote kind, and the registry holds no numbering feature to number
/// them.
///
/// The collect stage reports it once per such xnote kind and run, at the
/// kind's first xnote. Recovery: the marks, in the running text and beside
/// the list's entries, are written empty, a reference to such an xnote shows its label,
/// and the list is still built.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.xnotes.no-numbering",
    message = "notes of the kind `{kind}` are numbered, but the program processing this \
               document does not number them"
)]
pub struct NoNumbering {
    /// The name of the xnote kind.
    pub kind: String,
}

/// Condition `flm.xnotes.empty`: an xnote with no content (nothing but
/// whitespace and comments). Reported as a Warning, at the xnote, by the
/// lowering rule that writes it. Recovery: the mark is written, and the xnote
/// is listed with an empty entry.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.xnotes.empty", message = "this {kind} is empty")]
pub struct Empty {
    /// The name of the xnote's kind.
    pub kind: String,
}

/// Condition `flm.xnotes.pointer-refused`: a content pointer
/// ([`CONTENT_POINTER`](super::CONTENT_POINTER)) refers to a region of a node
/// that is not the content of an xnote of a kind the table of xnotes records
/// (where the table of xnotes is absent, an xnote of any kind is accepted).
///
/// No document can cause it: it is the mistake of a stage that built a list.
/// The list would otherwise show content that is no xnote's, which the reader
/// would then see twice. Reported at the pointer's source position, which is
/// the xnote's own position when a list builder created the pointer.
/// Recovery: nothing is written
/// for the pointer.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.xnotes.pointer-refused",
    message = "a content pointer must refer to the content of an xnote, but this one refers to \
               the {region} of a node of `{element}`"
)]
pub struct PointerRefused {
    /// The element of the node the pointer refers to.
    pub element: String,
    /// The region the pointer refers to, such as `argument 1`.
    pub region: String,
}

/// Condition `flm.xnotes.entry-outside-list`: an entry of a list of xnotes
/// ([`ENTRY`](super::ENTRY)) reaches a lowering of its own. The rule of a
/// list writes the content of its entries itself and never lowers an entry,
/// so an entry that is lowered was not written by its list (it was placed
/// outside a list, or its list's rule lowers its children one by one).
///
/// No document can cause it: it is the mistake of a stage that built a list.
/// Reported by the entry's rule ([`EntryRule`](super::EntryRule)), at the
/// entry. Recovery: nothing is written for the entry.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.xnotes.entry-outside-list",
    message = "an entry of a list of xnotes must be written by the list that holds it, not on its \
               own"
)]
pub struct EntryOutsideList;

/// Condition `flm.xnotes.unpaired`: for one xnote kind, a node's element
/// declares, through its reference points ([`ref_points`](super::ref_points),
/// one call per xnote), a number of xnotes that differs from the number of
/// regions holding an xnote's content
/// ([`NodeRegion`](crate::core::lowering::NodeRegion)) that it returns
/// through its [`XnoteSource`](super::XnoteSource) aspect.
///
/// No document can cause it: it is the mistake of the feature that owns the
/// element. The collect stage reports it once per such node and xnote kind,
/// at the node. Recovery: the xnotes are paired with those regions by order,
/// as that aspect states; an xnote with no region is written with no content,
/// and a region with no xnote is written by no rule (the element's rule,
/// asked to write it, reports `flm.run.missing-entry`: the table of xnotes
/// holds no record for it).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.xnotes.unpaired",
    message = "the element `{element}` must give the content of each note of the kind `{kind}` \
               that it declares: it declares {points} and gives {contents}"
)]
pub struct Unpaired {
    /// The element of the node.
    pub element: String,
    /// The name of the xnote kind.
    pub kind: String,
    /// The number of the node's xnotes of the kind that its reference points
    /// declare.
    pub points: usize,
    /// The number of regions of the kind, holding an xnote's content, that
    /// its [`XnoteSource`](super::XnoteSource) aspect returns.
    pub contents: usize,
}
