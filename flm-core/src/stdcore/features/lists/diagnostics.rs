//! The conditions the `lists` feature reports, each identified as
//! `flm.lists.<condition>`.
//!
//! Every condition is an Error except [`RefIgnored`], a Note: a `ref=` on a
//! list whose items have no number has no effect and harms nothing. Each item
//! below states the condition first, then the feature's recovery: what it
//! does to go on (see [recovery](crate::stdcore::backends::lowering::error)).
//! A diagnostic's message states the problem only.
//!
//! Every condition is reported by a lowering rule, never by the aspects of a
//! list or an item (they read what they can and report nothing), so a list
//! is reported once each time a rule lowers it:
//!
//! - the mistakes in how a list is written — [`UnknownKey`],
//!   [`MissingValue`], [`BadLabelForm`], [`ContentBeforeItem`],
//!   [`MissingTag`] — the Note [`RefIgnored`], and the problems of the label
//!   shown beside the items — [`BadLabel`], [`LabelNamesNumber`],
//!   [`NoNumbering`] — are reported by whichever rule lowers the list: the
//!   general rule ([`ListRule`](super::ListRule)) or the recomposition rule
//!   of the `latex-recomposed` backend
//!   ([`ListLatexRule`](super::latex::ListLatexRule)). A document therefore
//!   gets the same reports on every backend;
//! - [`NoEnumitemFormat`] and [`NumberReadAsCharacters`] are reported by the
//!   recomposition rule alone, the only rule that writes a list's own
//!   template as LaTeX;
//! - [`ItemOutsideList`] is reported by the rule of an `\item` that is not
//!   written directly in a list's body ([`ItemRule`](super::ItemRule)).

use techy::error::DiagnosticInfo;

/// Condition `flm.lists.unknown-key`, an Error: the optional argument of a
/// list, or of an `\item`, holds something that is neither one of its keys
/// (`label` and `ref` for a list, `tag` for an item) nor, for a list, a short
/// label form.
///
/// A Unicode bullet written as a short form (`[•]`) and a template holding a
/// dollar sign (`[(${alph})]`) are among these: a label holding any character
/// other than those of a short form is written `label={…}`.
///
/// Recovery: the key is ignored. A list then shows the label template of its
/// nesting depth, unless a `label=` in the same bracket gives one.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.unknown-key",
    message = "‘{key}’ is not an option of ‘{construct}’ (a list takes label=… or a short form \
               such as (i), and ref=…; an item takes tag=…, or its whole bracket as the tag)"
)]
pub struct UnknownKey {
    /// The list environment's name, or `item`.
    pub construct: String,
    /// The key as written.
    pub key: String,
}

/// Condition `flm.lists.missing-value`, an Error: the optional argument of a
/// list or of an `\item` writes one of its keys (`label`, `ref`, `tag`)
/// without `=` and a value.
///
/// Recovery: the key is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.missing-value",
    message = "the option ‘{key}’ of ‘{construct}’ must be given a value"
)]
pub struct MissingValue {
    /// The list environment's name, or `item`.
    pub construct: String,
    /// The key as written.
    pub key: String,
}

/// Condition `flm.lists.bad-label-form`, an Error: a list's short label form
/// holds more than one of the characters `a`, `A`, `i`, `I` and `1`, so that
/// more than one of them could be the number: `[i.a]` reads as `#{roman}.a`
/// and as `i.#{alph}`.
///
/// Recovery: the list shows the label template of its nesting depth. The
/// author writes the template meant with `label={…}`.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.bad-label-form",
    message = "the short label form ‘{written}’ of ‘{environment}’ may hold only one of the \
               characters a, A, i, I and 1, but it holds several, so it reads as ‘{first}’ or as \
               ‘{second}’; write the template meant as label=…"
)]
pub struct BadLabelForm {
    /// The list environment's name.
    pub environment: String,
    /// The short form as written.
    pub written: String,
    /// The template read with the first of those characters as the number.
    pub first: String,
    /// The template read with the second of those characters as the number.
    pub second: String,
}

/// Condition `flm.lists.bad-label`, an Error: a template that a list writes
/// in its optional argument — its label template (`label=` or a short form)
/// or its reference template (`ref=`) — is not valid in the template
/// language: a `#{…}` that is not a number format, or a text that is not
/// valid FLM.
///
/// One mistake is reported once. For a numbered list in a registry that holds
/// the [`numbering`](crate::stdcore::features::numbering) feature, that
/// feature's stage [`NUMBER`](crate::stdcore::features::numbering::NUMBER),
/// which records each reference point's counter value in the counter table,
/// compiles the list's own reference template
/// ([`ListOptions::ref_template`](super::ListOptions::ref_template)) and
/// reports it as `flm.numbering.bad-number-format`; the list's lowering rule
/// reports every other template it compiles, and never that one. A `ref=` on
/// an unnumbered list is not compiled ([`RefIgnored`]).
///
/// Recovery: the list uses the template of its nesting depth in place of the
/// one that is not valid.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.bad-label",
    message = "the {key}= template of ‘{environment}’ cannot be used: {detail}"
)]
pub struct BadLabel {
    /// The list environment's name.
    pub environment: String,
    /// The key the template is written under: `label` (a short form
    /// included) or `ref`.
    pub key: String,
    /// The template, in the `#{…}` language; a short form as translated.
    pub template: String,
    /// What the template compiler reported.
    pub detail: String,
}

/// Condition `flm.lists.label-names-number`, an Error: the label template
/// that an unnumbered list writes in its optional argument writes a number
/// format (`\begin{itemize}[label=(#{alph})]`, `\begin{itemize}[(i)]`), and
/// the list's items have no number to write. Whether a list is numbered
/// follows from its list kind ([`ListKind::numbered`](super::ListKind::numbered)),
/// never from its template.
///
/// Recovery: the list shows the label template of its nesting depth.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.label-names-number",
    message = "the label template ‘{template}’ must not write a number, since the items of \
               ‘{environment}’ are not numbered"
)]
pub struct LabelNamesNumber {
    /// The list environment's name.
    pub environment: String,
    /// The template, in the `#{…}` language.
    pub template: String,
}

/// Condition `flm.lists.ref-ignored`, a Note: a list whose list kind is not
/// numbered writes a reference template, `ref=`. Its items have no number, so
/// a reference to one of them shows the label beside it and never the
/// reference template, which is neither compiled nor checked.
///
/// The key is accepted, and reported as a Note rather than an Error, so that
/// a list can be switched between a numbered and an unnumbered list kind
/// without its bracket being rewritten. Reported once per list, on every
/// backend.
///
/// Recovery: the key is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.ref-ignored",
    message = "ref= has no effect on ‘{environment}’, whose items are not numbered"
)]
pub struct RefIgnored {
    /// The list environment's name.
    pub environment: String,
}

/// Condition `flm.lists.content-before-item`, an Error: a list's body holds
/// content other than whitespace and comments before its first `\item`.
///
/// Recovery: the general rule shows the content as a first item with no
/// label, which counts in nothing; the recomposed LaTeX writes the content
/// before the first `\item`, as the document wrote it.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.content-before-item",
    message = "the body of ‘{environment}’ must begin with \\item, but it holds content before \
               its first \\item"
)]
pub struct ContentBeforeItem {
    /// The list environment's name.
    pub environment: String,
}

/// Condition `flm.lists.missing-tag`, an Error: an item carries no tag in a
/// list that writes no label of its own — for example a `description` item
/// written `\item` without its term, in a list whose bracket writes no label
/// either.
///
/// Recovery: no label is shown beside the item.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.missing-tag",
    message = "this item of ‘{environment}’ must carry a tag, written \\item[…], since the list \
               writes no label of its own"
)]
pub struct MissingTag {
    /// The list environment's name.
    pub environment: String,
}

/// Condition `flm.lists.no-numbering`, an Error: a numbered list is lowered
/// with a registry that holds no
/// [`numbering`](crate::stdcore::features::numbering) feature, and the label
/// its items would show writes a number. Only the numbering feature numbers
/// items, so there is no number to show. Reported once per list, on every
/// backend.
///
/// Recovery: the general rule shows no label beside the list's untagged
/// items; an item carrying a tag still shows it, and a list whose label is
/// fixed (`label={*}`) still shows it. The recomposed LaTeX still writes the
/// list, and LaTeX numbers its items.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.no-numbering",
    message = "the items of ‘{environment}’ are numbered, but the program processing this \
               document does not number list items"
)]
pub struct NoNumbering {
    /// The list environment's name.
    pub environment: String,
}

/// Condition `flm.lists.item-outside-list`, an Error: an `\item` is inside a
/// list but not directly in its body — inside a group, an argument, or
/// another command or environment — so it begins no item.
///
/// Recovery: the `\item` is left out, and the content after it stays where
/// it is.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.item-outside-list",
    message = "\\item must be written directly in a list's body, not inside a group, an \
               argument, or another command or environment"
)]
pub struct ItemOutsideList;

/// Condition `flm.lists.no-enumitem-format`, an Error, reported on the
/// `latex-recomposed` backend only: a list's own label template (`label=` or
/// a short form) or reference template (`ref=`) writes a number format for
/// which LaTeX's `enumitem` package has no command. The package has commands
/// for `#{arabic}`, `#{alph}`, `#{Alph}`, `#{roman}` and `#{Roman}`, written
/// `\arabic*`, `\alph*`, `\Alph*`, `\roman*` and `\Roman*`.
///
/// The recomposed LaTeX writes a list's own template as the package's key of
/// the same name: the template's content is recomposed as any content is, and
/// each number format becomes the package's command for it. Every other
/// backend writes the number itself and reports nothing. Reported once per
/// key: a list whose `label=` and `ref=` templates both write such a format
/// is reported twice, each report giving its key.
///
/// Recovery: the key is not written, as for a template that is not valid.
/// Without `label=`, LaTeX shows its own label beside the items; without a
/// separate `ref=`, references use the label template.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.no-enumitem-format",
    message = "on LaTeX output, a list's {key}= template may write only the number formats \
               arabic, alph, Alph, roman and Roman (those of LaTeX's enumitem package), but \
               ‘{template}’ writes ‘{format}’"
)]
pub struct NoEnumitemFormat {
    /// The key the template is written under: `label` (a short form
    /// included) or `ref`.
    pub key: String,
    /// The template, in the `#{…}` language; a short form as translated.
    pub template: String,
    /// The first number format the template writes that the package has no
    /// command for, as the template writes its name.
    pub format: String,
}

/// Condition `flm.lists.number-read-as-characters`, an Error, reported on
/// the `latex-recomposed` backend only: a list's own label template or
/// reference template writes a number format inside content whose characters
/// are read as written rather than typeset, for example inside verbatim
/// content (`label={\verbtext|x#{arabic}|}`).
///
/// The recomposed LaTeX writes a number format as the `enumitem` package's
/// command for it, which LaTeX runs as it sets each label. The characters of
/// verbatim content, of an address or of the label in a `\ref` are read when the
/// recomposed LaTeX is written, before any number exists, so no LaTeX
/// command can be written there for the number. Every other backend writes
/// the number itself and reports nothing. Reported once per key.
///
/// Recovery: the key is not written, as for [`NoEnumitemFormat`].
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.number-read-as-characters",
    message = "on LaTeX output, the {key}= template ‘{template}’ must not write a number inside \
               content whose characters are read as written, such as verbatim content"
)]
pub struct NumberReadAsCharacters {
    /// The key the template is written under: `label` (a short form
    /// included) or `ref`.
    pub key: String,
    /// The template, in the `#{…}` language; a short form as translated.
    pub template: String,
}
