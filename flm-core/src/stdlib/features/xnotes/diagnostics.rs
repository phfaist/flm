//! This module defines the diagnostic conditions that the `xnotes` feature
//! reports (see [diagnostics](crate::guide::concepts_overview#diagnostics)).
//!
//! The conditions [`BadLabelArgument`], [`Empty`] and [`NoNumbering`] concern
//! the xnotes written in a document. The lowering rule of an xnote reports
//! [`BadLabelArgument`] when the optional argument of the xnote contains
//! something other than `\label` commands, and [`Empty`] when the xnote has no
//! content. The collect stage ([`COLLECT`](super::COLLECT)) reports
//! [`NoNumbering`] when a document contains xnotes of a numbered xnote kind
//! and the registry does not contain the numbering feature.
//!
//! The three other conditions indicate an error in the code of a feature, and
//! a document cannot cause them. The collect stage reports [`Unpaired`] for an
//! element whose aspects return different numbers of xnotes. The lowering
//! rules of a list of xnotes and of its content pointers report
//! [`PointerRefused`] for a content pointer that does not refer to the content
//! of an xnote. The lowering rule of an entry of a list of xnotes reports
//! [`EntryOutsideList`] when the entry is lowered outside its list.
//!
//! Each of these conditions is reported as an Error, except [`Empty`], which
//! is reported as a Warning. The document is rendered in spite of these
//! conditions. The documentation of each condition states what is written in
//! place of the faulty part of the document.
//!
//! The condition [`StageFailed`](super::StageFailed), which the stages of the
//! feature report, is defined together with the stages.

use techy::error::DiagnosticInfo;

/// The error that is reported when the optional argument of an xnote contains
/// something other than `\label` commands (id `flm.xnotes.bad-label-argument`).
///
/// The optional argument of an xnote contains the labels of the xnote, as in
/// `\footnote[\label{fn:a}]{…}`. This argument may contain only `\label{…}`
/// commands, whitespace (including `~` and `\ `) and comments.
///
/// Reported by the lowering rule of the xnote when it is invoked. The error is
/// reported once for each xnote, for the first node of the argument that does
/// not meet this requirement. All content of the argument other than the
/// `\label` commands is ignored, and these `\label` commands still label the
/// xnote.
///
/// In a registry without the references feature, the `\label` command is not
/// defined, and a `\label` command in the optional argument is reported in the
/// same way.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.xnotes.bad-label-argument",
    message = "Invalid ‘{found}’ in the optional argument of this {kind}: this \
               argument may contain only ‘\\label{{…}}’ commands"
)]
pub struct BadLabelArgument {
    /// The name of the xnote kind, e.g. `footnote`.
    pub kind: String,
    /// The source text of the first node of the optional argument that is not
    /// a `\label` command, whitespace or a comment. The text is trimmed and
    /// cut after 40 characters.
    pub found: String,
}

/// Error raised when the xnotes of a numbered xnote kind cannot be numbered
/// (`flm.xnotes.no-numbering`).
///
/// The numbering feature computes the numbers of the xnotes of a numbered
/// xnote kind, such as footnotes. This error is reported when a document
/// contains xnotes of a numbered kind and the registry does not contain the
/// numbering feature.
///
/// Reported by the collect stage [`COLLECT`](super::COLLECT), once for each
/// such xnote kind, at the first xnote of the kind. The xnotes of this kind
/// have no number: their marks in the text and in the list of the kind are
/// empty. The list of the kind is still built.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.xnotes.no-numbering",
    message = "Numbering is not available for ‘{kind}’ notes"
)]
pub struct NoNumbering {
    /// The name of the xnote kind.
    pub kind: String,
}

/// The warning that is reported when an xnote has no content (id
/// `flm.xnotes.empty`). Content that consists only of whitespace (including
/// `~` and `\ `) and comments counts as empty.
///
/// Reported by the lowering rule of the xnote when it is invoked. The xnote is
/// otherwise processed as usual, with empty content: its mark is written, and
/// for an xnote kind with a list, the entry of the xnote in the list is empty.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.xnotes.empty", message = "This {kind} is empty")]
pub struct Empty {
    /// The name of the xnote's kind.
    pub kind: String,
}

/// Error representing a content pointer
/// ([`CONTENT_POINTER`](super::CONTENT_POINTER)) that does not refer to the
/// content of an xnote (id `flm.xnotes.pointer-refused`).
///
/// In a list of xnotes, the content of each xnote is written in an entry of
/// the list. Each entry contains a content pointer: a synthesized node that
/// refers to the region of a node that contains the content of the xnote, such
/// as the mandatory argument of a `\footnote` command. The region must contain
/// the content of an xnote of a kind declared in the registry, as the
/// [`XnoteSource`](super::XnoteSource) aspect of the node's element returns
/// it. This requirement ensures that no content is written twice: any other
/// content is already written where its node is placed.
///
/// A document cannot cause this error: it is caused by the faulty stage that
/// created the content pointer. Reported by the lowering rule of the content
/// pointer ([`PointerRule`](super::PointerRule)), and by the lowering rule of
/// a list of xnotes ([`ListRule`](super::ListRule)), which lowers the content
/// pointers of its entries itself. The content pointer expands to nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.xnotes.pointer-refused",
    message = "Invalid content pointer to {region} of a node of ‘{element}’: the \
               pointer must refer to the content of an xnote"
)]
pub struct PointerRefused {
    /// The name of the element of the node that the content pointer refers to.
    pub element: String,
    /// The region of this node that the content pointer refers to, as text,
    /// e.g. `argument 1`.
    pub region: String,
}

/// Error representing an entry of a list of xnotes ([`ENTRY`](super::ENTRY))
/// that is lowered by its own lowering rule (id
/// `flm.xnotes.entry-outside-list`).
///
/// The lowering rule of a list of xnotes lowers the content of each entry
/// itself, and never invokes the lowering rule of the entry. An entry is
/// therefore lowered by its own rule only when a stage placed it outside a
/// list, or when the lowering rule of its list lowers the children of the list
/// one by one.
///
/// A document cannot cause this error: it is caused by a faulty stage or
/// lowering rule. Reported by [`EntryRule`](super::EntryRule). The entry
/// expands to nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.xnotes.entry-outside-list",
    message = "Entries of a list of xnotes must be lowered by the lowering rule of their list"
)]
pub struct EntryOutsideList;

/// Error raised when the two aspects of an element that describe its xnotes
/// return different numbers of xnotes of one kind (id `flm.xnotes.unpaired`).
///
/// Each xnote that an element carries is described in two aspects of the
/// element. The [`Referenceable`](crate::model::aspects::Referenceable) aspect
/// returns two reference points for each xnote, created with
/// [`ref_points`](super::ref_points). The [`XnoteSource`](super::XnoteSource)
/// aspect returns the region of the node that contains the content of each
/// xnote. For each xnote kind, both aspects must return the same number of
/// xnotes.
///
/// A document cannot cause this error: it indicates an error in the
/// definition of the element. Reported by the collect stage
/// [`COLLECT`](super::COLLECT) once for each such node and xnote kind. The
/// xnotes of the node are paired with the content regions in order. An xnote
/// without a content region has empty content. A content region without an
/// xnote is not written, and the lowering rule of the element reports
/// `flm.run.missing-entry` for it.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.xnotes.unpaired",
    message = "Misconfigured element ‘{element}’: different numbers of xnotes of \
               kind ‘{kind}’ in its reference points ({points}) and in its \
               ‘XnoteSource’ aspect ({contents})"
)]
pub struct Unpaired {
    /// The name of the element of the node.
    pub element: String,
    /// The name of the xnote kind.
    pub kind: String,
    /// The number of xnotes of this kind among the reference points of the
    /// node.
    pub points: usize,
    /// The number of content regions of xnotes of this kind that the
    /// [`XnoteSource`](super::XnoteSource) aspect of the element returns for
    /// the node.
    pub contents: usize,
}
