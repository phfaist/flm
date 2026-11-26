//! This module defines the diagnostic conditions of the `lists` feature. Their
//! identifiers have the form `flm.lists.<condition>`. Every condition is an
//! error, except [`RefIgnored`], which is a note.
//!
//! The conditions are reported by the lowering rules of the feature:
//!
//! - The rule that lowers a list reports the mistakes in the optional
//!   arguments of the list and of its items ([`UnknownKey`],
//!   [`MissingValue`], [`BadLabelForm`], [`RefIgnored`]), the mistakes in the
//!   body of the list ([`ContentBeforeItem`], [`MissingTag`]), and the
//!   problems with the labels beside the items ([`BadLabel`],
//!   [`LabelNamesNumber`], [`NoNumbering`]). Both the general rule
//!   [`ListRule`](super::ListRule) and the rule
//!   [`ListLatexRule`](super::latex::ListLatexRule) of the `latex-recomposed`
//!   backend report these conditions, so a document gets the same reports on
//!   every backend.
//! - [`ListLatexRule`](super::latex::ListLatexRule) also reports
//!   [`NoEnumitemFormat`] and [`NumberReadAsCharacters`]. These conditions
//!   concern the way in which the recomposed LaTeX writes the label template
//!   and the reference template of a list.
//! - The rule [`ItemRule`](super::ItemRule) of `\item` reports
//!   [`ItemOutsideList`], on every backend.

use techy::error::DiagnosticInfo;

/// The error that is reported when the optional argument of a list or of an
/// `\item` contains an unknown key (`flm.lists.unknown-key`).
///
/// The optional argument of a list accepts the keys `label` and `ref`, as in
/// `\begin{enumerate}[label=(#{alph}), ref=#{alph}]`. Instead of key-value
/// pairs, it also accepts a short label form, as in `\begin{enumerate}[(a)]`.
/// A short label form consists only of the characters `[]().:-+*aAiI1`. A
/// label with other characters is given with the `label` key, as in
/// `\begin{enumerate}[label={Step~#{arabic}:}]`. An optional argument of a
/// list that contains no `=` and is not a short label form is reported as a
/// whole as an unknown key.
///
/// The optional argument of an `\item` accepts the key `tag`, as in
/// `\item[tag={a=b}]`. An argument that contains no `=` is the tag itself, as
/// in `\item[Term]`.
///
/// Reported by the lowering rule of the list, on every backend. The unknown
/// key is ignored, and the other keys of the argument are used.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.unknown-key",
    message = "Invalid ‘{key}’ in argument of ‘{construct}’"
)]
pub struct UnknownKey {
    /// The list environment's name, or `item`.
    pub construct: String,
    /// The unknown key, as written. When the whole argument is reported, this
    /// is the text of the whole argument.
    pub key: String,
}

/// The error that is reported when a key in the optional argument of a list
/// or of an `\item` is written without `=` (`flm.lists.missing-value`).
///
/// Each key must be followed by `=` and its value, as in
/// `\begin{enumerate}[label=(#{alph}), ref=#{alph}]` or `\item[tag={Term}]`.
///
/// Reported by the lowering rule of the list, on every backend. The key is
/// ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.missing-value",
    message = "Missing value for key ‘{key}’ of ‘{construct}’: use the syntax ‘{key}=…’"
)]
pub struct MissingValue {
    /// The list environment's name, or `item`.
    pub construct: String,
    /// The key as written.
    pub key: String,
}

/// The error that is reported when a short label form contains more than one
/// of the characters `a`, `A`, `i`, `I` and `1` (`flm.lists.bad-label-form`).
///
/// In a short label form, one of these characters stands for the number of the
/// item, written in the style of the character: `\begin{enumerate}[(a)]` gives
/// the labels `(a)`, `(b)`, …, and `\begin{enumerate}[i.]` gives `i.`, `ii.`,
/// … A short label form may therefore contain at most one of these
/// characters. A short label form without any of them is a fixed label, as in
/// `\begin{itemize}[-]`. A label that contains more of these characters is
/// given with the `label` key, where `#{…}` marks the number, as in
/// `\begin{enumerate}[label=#{roman}.a]`.
///
/// Reported by the lowering rule of the list, on every backend. The list then
/// uses the label template of its [list kind](super#lists-and-list-kinds) for
/// its nesting depth.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.bad-label-form",
    message = "Invalid short label form ‘{written}’ of ‘{environment}’ (short form labels must contain exactly one instance of a, A, i, I, or 1); use the ‘label={{{first}}}’ syntax instead"
)]
pub struct BadLabelForm {
    /// The list environment's name.
    pub environment: String,
    /// The short form as written.
    pub written: String,
    /// The label template that the short form stands for when the first of
    /// its characters `a`, `A`, `i`, `I` and `1` is the number.
    pub first: String,
    /// The label template that the short form stands for when the second of
    /// its characters `a`, `A`, `i`, `I` and `1` is the number.
    pub second: String,
}

/// The error that is reported when a label template or a reference template
/// in the optional argument of a list is not valid (`flm.lists.bad-label`).
///
/// These templates are written in the language of the number templates of the
/// [`numbering`](crate::stdlib::features::numbering) feature. Each `#{name}`
/// stands for the number of the item in the number format `name`, and the
/// rest of the template is FLM content, as in `label=(#{alph})` or
/// `label={\textbf{#{arabic}}.}`. A template is not valid if it contains a
/// placeholder that is not a number format, or content that is not valid FLM.
///
/// Reported by the lowering rule of the list, on every backend. If the label
/// template is not valid, the list uses the label template of its
/// [list kind](super#lists-and-list-kinds) for its nesting depth. On the
/// `latex-recomposed` backend, the key of the template is left out of the
/// recomposed LaTeX, and LaTeX uses its default instead.
///
/// Each template that is not valid is reported once. The reference template of
/// a numbered list is compiled by the numbering feature, which reports a
/// reference template that is not valid as `flm.numbering.bad-number-format`.
/// This also holds for a template given with `label=` alone, which sets both
/// the label template and the reference template. This condition therefore
/// reports a reference template only in a registry without the numbering
/// feature.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.bad-label",
    message = "Invalid ‘{key}’ value in ‘{environment}’: {detail}"
)]
pub struct BadLabel {
    /// The list environment's name.
    pub environment: String,
    /// The key of the template: `label` for the label template, also when it
    /// is given as a short label form, or `ref` for the reference template.
    pub key: String,
    /// The template. A short label form is given as the label template that it
    /// stands for, e.g. `(#{roman})` for `(i)`.
    pub template: String,
    /// The messages that were reported when the template was compiled,
    /// separated by semicolons.
    pub detail: String,
}

/// The error that is reported when the label template of an unnumbered list
/// contains a number format (`flm.lists.label-names-number`).
///
/// The [list kind](super#lists-and-list-kinds) of a list environment
/// determines whether the items of its lists are numbered
/// ([`ListKind::numbered`](super::ListKind::numbered)). The label template of
/// a list cannot change this. The items of an unnumbered list, such as
/// `itemize`, have no number. The label template of such a list must
/// therefore be a fixed label, as in `\begin{itemize}[label={--}]` or
/// `\begin{itemize}[*]`. A number format is written as `#{…}` in a label
/// template, and as one of the characters `a`, `A`, `i`, `I` and `1` in a
/// short label form.
///
/// Reported by the lowering rule of the list, on every backend. The list then
/// uses the label template of its list kind for its nesting depth. On the
/// `latex-recomposed` backend, the key `label` is left out of the recomposed
/// LaTeX, and LaTeX uses its default label.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.label-names-number",
    message = "Invalid numbering placeholder in template ‘{template}’ for unnumbered list ‘{environment}’"
)]
pub struct LabelNamesNumber {
    /// The list environment's name.
    pub environment: String,
    /// The label template. A short label form is given as the label template
    /// that it stands for, e.g. `(#{roman})` for `(i)`.
    pub template: String,
}

/// The note that is reported when the optional argument of an unnumbered list
/// contains the key `ref` (`flm.lists.ref-ignored`).
///
/// The items of an unnumbered list have no number, so the reference template
/// given with `ref` is not used. A reference to an item of such a list is
/// written as the label beside the item. The value of `ref` is ignored, and
/// it is not checked.
///
/// This condition is a note and not an error so that you can change the
/// environment of a list between a numbered and an unnumbered one, for
/// instance from `enumerate` to `itemize`, without removing the key `ref`.
///
/// Reported by the lowering rule of the list, on every backend, once for each
/// such list.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.ref-ignored",
    message = "Invalid ‘ref=’ for unnumbered list ‘{environment}’ (remove it)"
)]
pub struct RefIgnored {
    /// The list environment's name.
    pub environment: String,
}

/// The error that is reported when the body of a list contains content before
/// its first `\item` (`flm.lists.content-before-item`). Whitespace, blank
/// lines and comments do not count as content here.
///
/// Reported by the lowering rule of the list, on every backend. On every
/// backend except `latex-recomposed`, the content is written as an additional
/// first item, which has no label and no number. On the `latex-recomposed`
/// backend, the content is written before the first `\item`, as in the
/// source.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.content-before-item",
    message = "No content may appear before the first ‘\\item’ in ‘{environment}’"
)]
pub struct ContentBeforeItem {
    /// The list environment's name.
    pub environment: String,
}

/// The error that is reported when an item has no tag in a list that has no
/// label template (`flm.lists.missing-tag`).
///
/// A list has a label template if its [list kind](super#lists-and-list-kinds)
/// has one for the nesting depth of the list, or if the optional argument of
/// the list gives one. In the default configuration, the list kind of the
/// `description` environment has no label template. Each item of a
/// `description` list must therefore have a tag, as in `\item[Term]` or
/// `\item[tag={Term}]`.
///
/// Reported by the lowering rule of the list, on every backend, once for each
/// item without a tag. The item is written without a label.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.missing-tag",
    message = "An item tag is needed here since ‘{environment}’ has no default label; use the syntax ‘\\item[…]’ or ‘\\item[tag={{…}}]’"
)]
pub struct MissingTag {
    /// The list environment's name.
    pub environment: String,
}

/// The error that is reported when the items of a numbered list need a number
/// and the registry does not contain the
/// [`numbering`](crate::stdlib::features::numbering) feature (id
/// `flm.lists.no-numbering`).
///
/// Only the numbering feature computes the numbers of list items. The items of
/// a numbered list need a number if the label template of the list contains a
/// number format and at least one item has no tag. A list with a fixed label,
/// as in `\begin{enumerate}[label={*}]`, needs no number, and neither does an
/// item with a tag.
///
/// Reported by the lowering rule of the list, on every backend, once for each
/// such list. On every backend except `latex-recomposed`, the items without a
/// tag are written without a label. On the `latex-recomposed` backend, the list
/// is written as usual, and LaTeX numbers its items.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.no-numbering",
    message = "Missing feature ‘numbering’ for numbered list ‘{environment}’"
)]
pub struct NoNumbering {
    /// The list environment's name.
    pub environment: String,
}

/// The error that is reported when an `\item` inside a list is not placed
/// directly in the body of the list (`flm.lists.item-outside-list`).
///
/// An `\item` begins an item only if it is placed directly in the body of a
/// list, as in `\begin{itemize}\item First \item Second\end{itemize}`.
/// Outside of a list, `\item` is not defined at all.
///
/// Reported by the lowering rule [`ItemRule`](super::ItemRule) of `\item`, on
/// every backend. The `\item` expands to nothing. The content that follows it
/// is written as usual.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.item-outside-list",
    message = "‘\\item’ must be placed directly in the body of a list"
)]
pub struct ItemOutsideList;

/// The error that is reported on the `latex-recomposed` backend when the label
/// template or the reference template of a list contains a number format that
/// LaTeX's `enumitem` package does not support (id
/// `flm.lists.no-enumitem-format`).
///
/// The recomposed LaTeX writes the label template and the reference template
/// given in the optional argument of a list as the `label` and `ref` keys of
/// the `enumitem` package. It writes each number format as the `enumitem`
/// command for this format. Such commands exist only for the number formats
/// `#{arabic}`, `#{alph}`, `#{Alph}`, `#{roman}` and `#{Roman}`, as in
/// `\begin{enumerate}[label=(#{alph})]`. The other backends write the number
/// themselves and accept every number format of the
/// [`numbering`](crate::stdlib::features::numbering) feature.
///
/// Reported by the rule [`ListLatexRule`](super::latex::ListLatexRule), once
/// for each key. The key is left out of the recomposed LaTeX. Without
/// `label`, LaTeX uses its default label. Without `ref`, references to the
/// items use the label template.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.no-enumitem-format",
    message = "Unsupported number format ‘#{{{format}}}’ in ‘{key}’ value for backend ‘latex-recomposed’ (supported formats: ‘#{{arabic}}’, ‘#{{alph}}’, ‘#{{Alph}}’, ‘#{{roman}}’ and ‘#{{Roman}}’)"
)]
pub struct NoEnumitemFormat {
    /// The key of the template: `label` for the label template, also when it
    /// is given as a short label form, or `ref` for the reference template.
    pub key: String,
    /// The template. A short label form is given as the label template that it
    /// stands for, e.g. `(#{roman})` for `(i)`.
    pub template: String,
    /// The name of the first number format in the template that `enumitem`
    /// does not support, e.g. `fnsymbol`.
    pub format: String,
}

/// The error that is reported on the `latex-recomposed` backend when the label
/// template or the reference template of a list contains a number format
/// inside content that is read as characters, such as verbatim content (id
/// `flm.lists.number-read-as-characters`).
///
/// A command reads its content as characters when it uses the characters as
/// they are written instead of typesetting them. This is the case, for
/// instance, for verbatim content, for the address of a link and for the label
/// in a `\ref`. The recomposed LaTeX writes a number format as a command of
/// LaTeX's `enumitem` package, which LaTeX expands each time it sets a label.
/// The characters of such content, however, are needed when the recomposed
/// LaTeX is generated, before any number exists. The other backends write the
/// number themselves and report nothing.
///
/// Reported by the rule [`ListLatexRule`](super::latex::ListLatexRule), once
/// for each key. The key is left out of the recomposed LaTeX, as for
/// [`NoEnumitemFormat`].
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lists.number-read-as-characters",
    message = "Unsupported number format in ‘{key}’ value ‘{template}’ for backend ‘latex-recomposed’: number formats may not be placed inside content read as characters, such as verbatim content or a link address"
)]
pub struct NumberReadAsCharacters {
    /// The key of the template: `label` for the label template, also when it
    /// is given as a short label form, or `ref` for the reference template.
    pub key: String,
    /// The template. A short label form is given as the label template that it
    /// stands for, e.g. `(#{roman})` for `(i)`.
    pub template: String,
}
