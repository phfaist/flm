//! The conditions that the `citations` feature reports (a condition is the
//! typed body of a diagnostic; see
//! [diagnostics](crate::guide::concepts_overview#diagnostics)). Each is an
//! Error except [`ProviderWarning`], a Warning; the document is still
//! rendered. Each condition's documentation states the feature's recovery
//! (what it does to go on; see
//! [recovery](crate::stdcore::backends::lowering::error)).
//!
//! The lowering rule that lowers a citation chunk reports the mistakes in its
//! writing ([`NoKey`], [`NoCitation`], [`UnknownFlag`], [`Uncollected`]): once
//! per output the document is rendered to, and again where the citation
//! chunk is shown in a copy. The stages report what concerns the citation
//! entries: the retrieve stage reports what it finds while asking the
//! citation providers and reading their citation entries ([`NoProvider`],
//! [`ProviderFailed`], [`ProviderWarning`], [`EntryCites`],
//! [`EntryNotInline`]), and the check stage reports, under every workflow,
//! every cited key left with no citation entry ([`NoEntry`]). The condition
//! [`StageFailed`](super::StageFailed) is defined beside the stages.

use techy::error::DiagnosticInfo;

/// Condition `flm.citations.no-key`, an Error: a call of a citation chunk
/// holds no citation key (`\cite{}`, `\cite{ , }`). Reported by the rule
/// that lowers the citation chunk, at the call. The call shows nothing; a
/// citation chunk with nothing left to show is shown as a dropped mark
/// (`flm.pres.dropped`).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.citations.no-key", message = "each \\cite must list at least one citation key")]
pub struct NoKey;

/// Condition `flm.citations.no-citation`, an Error: an `\R` has no `\cite`
/// directly after it (`\R text`, `\R{\cite{a}}`). Reported by the rule of
/// `\R`, at the `\R`. The `\R` is shown as a dropped mark
/// (`flm.pres.dropped`); a `\cite` written further on is a citation chunk of
/// its own.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.citations.no-citation", message = "\\R must be followed directly by \\cite")]
pub struct NoCitation;

/// Condition `flm.citations.unknown-flag`, an Error: the flags argument of
/// an `\R` holds something other than `S` and `I`. Reported by the rule of
/// `\R`, at the `\R`, once per unknown flag (or once for a flags argument
/// that is not plain characters). The unknown flag is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.unknown-flag",
    message = "the flags of \\R may be only S and I, not `{flag}`"
)]
pub struct UnknownFlag {
    /// The flag as written, or the plain text of a flags argument that is not
    /// plain characters.
    pub flag: String,
}

/// Condition `flm.citations.uncollected`, an Error: a citation key has no
/// number, because it is cited where the collect stage does not look: inside
/// content that a stage generated rather than the document's own trees, or
/// inside a user definition's template expanded only while the output is
/// written. Reported by [`stage_mark`](super::stage_mark), at the citation
/// chunk. The citation key is left out of the citation mark.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.uncollected",
    message = "a citation must be written in the document's own text to be numbered, and the \
               citation key `{key}` is cited outside it"
)]
pub struct Uncollected {
    /// The citation key.
    pub key: String,
}

/// Condition `flm.citations.no-provider`, an Error: the document cites works
/// and the registry holds no citation provider (the service slot
/// [`PROVIDERS_SLOT`](super::PROVIDERS_SLOT) is empty), so no cited key has a
/// citation entry. Reported once, by the retrieve stage, at the document's
/// first citation. Every cited key is then also reported as [`NoEntry`].
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.no-provider",
    message = "the document cites works, the first being `{key}`, and no source of bibliography \
               entries is set up to look them up"
)]
pub struct NoProvider {
    /// The document's first cited key.
    pub key: String,
}

/// Condition `flm.citations.provider-failed`, an Error: the citation provider
/// that claimed a citation key could not answer for it
/// ([`CitationProviderError`](super::CitationProviderError)). Reported by the
/// retrieve stage, at the first citation of the citation key. The citation
/// key has no citation entry, and no later citation provider is asked about
/// it.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.provider-failed",
    message = "the bibliography entry of the key `{key}` could not be retrieved: {detail}"
)]
pub struct ProviderFailed {
    /// The citation key.
    pub key: String,
    /// What the citation provider said.
    pub detail: String,
}

/// Condition `flm.citations.provider-warning`, a Warning: the citation
/// provider that answered a citation key has something to tell the author
/// about it ([`CitationAnswer::warnings`](super::CitationAnswer::warnings)).
/// Reported by the retrieve stage, at the citation key's first citation, with
/// the citation provider's text as the message. The citation entry is shown
/// and the numbering is unchanged.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.citations.provider-warning", message = "{detail}")]
pub struct ProviderWarning {
    /// The citation key.
    pub key: String,
    /// What the citation provider said.
    pub detail: String,
}

/// Condition `flm.citations.entry-cites`, an Error: the citation entry that a
/// citation provider returned for a citation key holds a citation (`\cite`,
/// `\R`), directly or through a user definition. Reported by the retrieve
/// stage, at the first citation of the citation key. The citation entry is
/// refused and the citation key has no citation entry.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.entry-cites",
    message = "the bibliography entry of the key `{key}` may not cite other works"
)]
pub struct EntryCites {
    /// The citation key.
    pub key: String,
}

/// Condition `flm.citations.entry-not-inline`, an Error: the citation entry
/// that a citation provider returned for a citation key holds a paragraph
/// break or a block (a list, a heading), while a citation entry must be
/// inline content (the content of one entry of the list of cited works, or of
/// a citation mark under the in-place placement). Reported by the retrieve
/// stage, at the first citation of the citation key. The citation entry is
/// refused and the citation key has no citation entry.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.entry-not-inline",
    message = "the bibliography entry of the key `{key}` may not hold a paragraph break or a \
               block such as a list or a heading"
)]
pub struct EntryNotInline {
    /// The citation key.
    pub key: String,
}

/// Condition `flm.citations.no-entry`, an Error: a cited key has no citation
/// entry, for the reason [`MissingEntry`](super::MissingEntry) gives.
/// Reported by the check stage, at the first citation of the citation key,
/// under every workflow. The citation key keeps its number and its citation
/// marks; the list of cited works, or the citation mark under the in-place
/// placement, shows a dropped mark (`flm.pres.dropped`) where its citation
/// entry would be.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.citations.no-entry",
    message = "the cited key `{key}` must have a bibliography entry; {reason}"
)]
pub struct NoEntry {
    /// The citation key.
    pub key: String,
    /// Why it has none, in a few words ([`MissingEntry::reason`](super::MissingEntry::reason)):
    /// `no source of bibliography entries knows it`.
    pub reason: String,
}
