//! This module defines the diagnostic conditions that the `citations` feature
//! reports (see [diagnostics](crate::guide::concepts_overview#diagnostics)).
//!
//! The lowering rules of `\cite` and `\R` report the errors in the citations
//! of a document: [`NoKey`], [`NoCitation`], [`UnknownFlag`] and
//! [`Uncollected`]. The stages of the feature report the problems with the
//! citation entries. The retrieve stage ([`RETRIEVE`](super::RETRIEVE))
//! obtains the citation entries from the citation providers, and reports
//! [`NoProvider`], [`ProviderFailed`], [`ProviderWarning`], [`EntryCites`] and
//! [`EntryNotInline`]. The check stage ([`CHECK`](super::CHECK)) reports
//! [`NoEntry`] for each cited key that has no citation entry.
//!
//! Each of these conditions is reported as an Error, except
//! [`ProviderWarning`], which is reported as a Warning. The document is
//! rendered in spite of these conditions. The documentation of each condition
//! states what is written in place of the faulty citation or citation entry.
//!
//! The condition [`StageFailed`](super::StageFailed), which a stage of the
//! feature reports when it cannot read or write one of its tables, is defined
//! together with the stages.

use techy::error::DiagnosticInfo;

/// The error that is reported when a `\cite` call has an empty argument (id
/// `flm.citations.no-key`). An argument that contains only commas and
/// whitespace counts as empty.
///
/// Reported by the lowering rule of `\cite` when it is invoked, and by the
/// lowering rule of `\R` for the `\cite` call that follows the `\R`. If the
/// `\cite` call stands alone, it expands to a dropped mark
/// (`flm.pres.dropped`). Several `\cite` calls written directly one after
/// another, as in `\cite{a}\cite[Thm 4]{b}`, expand to one citation mark, and
/// a call with an empty argument adds no number to this citation mark.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.no-key",
    message = "Missing key in the argument of ‘\\cite’"
)]
pub struct NoKey;

/// Error raised when a `\R` is not immediately followed by `\cite`
/// (`flm.citations.no-citation`). The `\R` command must immediately be
/// followed by a valid `\cite{..}` command invocation, as in
/// `\R\cite{bib:myref}` or `\R[I]\cite{bib:myref,bib:myref2}`. Reported by the
/// lowering rule for `\R` when `\R` is invoked. In such a case, the `\R`
/// expands to a dropped mark (`flm.pres.dropped`).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.no-citation",
    message = "‘\\R’ must be followed immediately by ‘\\cite’"
)]
pub struct NoCitation;

/// The error that is reported when the flags argument of `\R` contains an
/// unknown flag (`flm.citations.unknown-flag`).
///
/// The flags argument of `\R` accepts two flags, each written as one
/// character, as in `\R[S]\cite{a}`. The flag `S` capitalizes the word that
/// `\R` writes before the citation mark (`Ref.` instead of `ref.`), and the
/// flag `I` omits this word. Whitespace in the flags argument is ignored.
///
/// Reported by the lowering rule of `\R` when it is invoked, once for each
/// unknown flag. Each unknown flag is ignored. If the flags argument contains
/// content other than characters, the error is reported once, with the plain
/// text of the whole argument, and the whole flags argument is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.unknown-flag",
    message = "Invalid flag ‘{flag}’ for ‘\\R’; expected ‘S’, ‘I’"
)]
pub struct UnknownFlag {
    /// The unknown flag. If the flags argument contains content other than
    /// characters, this field contains the plain text of the whole argument.
    pub flag: String,
}

/// Error raised when a citation key is cited only outside the trees of the
/// document, so that it has no number (condition id
/// `flm.citations.uncollected`).
///
/// The collect stage ([`COLLECT`](super::COLLECT)) records the citation keys
/// of the citations in the trees of the document, and only these citation keys
/// are numbered. A citation in other content is not recorded. This is the
/// case for a citation in a [synthesized
/// value](crate::guide::concepts_overview#synthesized-values) that a stage
/// creates, and for a citation in the template of a user definition whose use
/// sites are expanded during lowering.
///
/// Reported by the lowering rules of `\cite` and `\R` through
/// [`stage_mark`](super::stage_mark), once for each such citation key, at the
/// `\cite` or `\R`. The citation key is left out of the citation mark. If no
/// citation key is left, the citation expands to a dropped mark
/// (`flm.pres.dropped`).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.uncollected",
    message = "Citation to ‘{key}’ may not appear here (citations may only appear in document text)"
)]
pub struct Uncollected {
    /// The citation key.
    pub key: String,
}

/// The error that is reported when a document cites works and no citation
/// provider is set up (`flm.citations.no-provider`).
///
/// Citation providers supply the citation entries of the cited works. The
/// program that embeds FLM contributes them to the service slot
/// [`PROVIDERS_SLOT`](super::PROVIDERS_SLOT); see
/// [`CitationProvider`](super::CitationProvider).
///
/// Reported once by the retrieve stage ([`RETRIEVE`](super::RETRIEVE)), at the
/// first citation of the document. The citations are numbered as usual, but no
/// cited key has a citation entry. A dropped mark (`flm.pres.dropped`) is
/// written in place of each citation entry, and the check stage also reports
/// [`NoEntry`] for each cited key.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.no-provider",
    message = "No bibliography source set up to handle citation ‘{key}’"
)]
pub struct NoProvider {
    /// The document's first cited key.
    pub key: String,
}

/// The error that is reported when a citation provider fails to retrieve the
/// citation entry for a citation key (`flm.citations.provider-failed`).
///
/// A citation provider reports this failure by returning a
/// [`CitationProviderError`](super::CitationProviderError) for the citation
/// key. The retrieve stage also reports this error for every citation key of a
/// call of [`CitationProvider::answer_all`](super::CitationProvider::answer_all)
/// that returns a different number of results than it received requests.
///
/// Reported by the retrieve stage ([`RETRIEVE`](super::RETRIEVE)), at the first
/// citation of the citation key. No later citation provider is called for this
/// citation key, and the citation key has no citation entry. A dropped mark
/// (`flm.pres.dropped`) is written in place of its citation entry, and the
/// check stage also reports [`NoEntry`] for it.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.provider-failed",
    message = "Failed to retrieve bibliography entry for ‘{key}’: {detail}"
)]
pub struct ProviderFailed {
    /// The citation key.
    pub key: String,
    /// The error message that the
    /// [`CitationProviderError`](super::CitationProviderError) contains.
    pub detail: String,
}

/// The warning that is reported for each warning text that a citation
/// provider adds to its answer for a citation key
/// (`flm.citations.provider-warning`).
///
/// A citation provider adds a warning text to an answer with
/// [`CitationAnswer::with_warning`](super::CitationAnswer::with_warning). The
/// message of this diagnostic is the warning text.
///
/// Reported by the retrieve stage ([`RETRIEVE`](super::RETRIEVE)), at the first
/// citation of the citation key, in the order in which the warning texts were
/// added. The answer is processed as usual.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.citations.provider-warning", message = "{detail}")]
pub struct ProviderWarning {
    /// The citation key.
    pub key: String,
    /// The warning text.
    pub detail: String,
}

/// The error that is reported when the citation entry for a citation key
/// contains a citation (`flm.citations.entry-cites`).
///
/// A citation entry may not contain `\cite` or `\R`. The citation entry is
/// checked after the user definitions that it uses have been expanded, so a
/// citation that a user definition writes is also found.
///
/// Reported by the retrieve stage ([`RETRIEVE`](super::RETRIEVE)), at the first
/// citation of the citation key. The retrieve stage refuses the citation
/// entry, so the citation key has no citation entry. A dropped mark
/// (`flm.pres.dropped`) is written in place of the citation entry, and the
/// check stage also reports [`NoEntry`] for the citation key.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.entry-cites",
    message = "The bibliography entry for ‘{key}’ may not use ‘\\cite’ recursively"
)]
pub struct EntryCites {
    /// The citation key.
    pub key: String,
}

/// The error that is reported when the citation entry for a citation key is
/// not inline content (`flm.citations.entry-not-inline`).
///
/// A citation entry must be inline content, that is, content without
/// paragraph breaks and blocks, as in `D. Knuth, \emph{The TeXbook}, 1984`. A
/// citation entry is written inside an entry of the list of cited works, or
/// inside a citation mark under the in-place placement
/// ([`CitationPlacement::InPlace`](super::CitationPlacement::InPlace)). The
/// citation entry is checked after the user definitions that it uses have
/// been expanded.
///
/// Reported by the retrieve stage ([`RETRIEVE`](super::RETRIEVE)), at the first
/// citation of the citation key. The retrieve stage refuses the citation
/// entry, so the citation key has no citation entry. A dropped mark
/// (`flm.pres.dropped`) is written in place of the citation entry, and the
/// check stage also reports [`NoEntry`] for the citation key.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.entry-not-inline",
    message = "The bibliography entry for ‘{key}’ must hold inline content only (no paragraph breaks or blocks)"
)]
pub struct EntryNotInline {
    /// The citation key.
    pub key: String,
}

/// The error that is reported when a cited key has no citation entry
/// (`flm.citations.no-entry`).
///
/// The field `reason` states why the citation key has no citation entry
/// ([`MissingEntry`](super::MissingEntry)). For instance, no citation provider
/// handles the citation key, or the citation provider failed to retrieve the
/// citation entry.
///
/// Reported by the check stage ([`CHECK`](super::CHECK)), at the first citation
/// of the citation key. The citation key keeps its number, and its citations
/// are written as usual. A dropped mark (`flm.pres.dropped`) is written in
/// place of the citation entry: in the list of cited works, or, under the
/// in-place placement
/// ([`CitationPlacement::InPlace`](super::CitationPlacement::InPlace)), in the
/// citation marks.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.no-entry",
    message = "No entry found for citation key ‘{key}’: {reason}"
)]
pub struct NoEntry {
    /// The citation key.
    pub key: String,
    /// The reason why the citation key has no citation entry, as returned by
    /// [`MissingEntry::reason`](super::MissingEntry::reason). For instance,
    /// `no appropriate citation provider found`.
    pub reason: String,
}
