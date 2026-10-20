//! The conditions the `quote` feature reports, each an Error identified as
//! `flm.quote.<condition>`. A condition is the typed body of a diagnostic
//! ([diagnostics](crate::guide::concepts_overview#diagnostics)).
//!
//! All three are reported while a quotation is lowered, on every backend.
//! [`BareContent`] and [`IndentMidLine`] are reported by the lowering rule of
//! the quote environment, which reads the body and its lines: the general
//! rule [`QuoteRule`](super::QuoteRule), and on the `latex-recomposed`
//! backend [`QuoteLatexRule`](super::latex::QuoteLatexRule), which reads the
//! body and its lines in the same way. [`NestedSection`] is reported by the
//! rule of a quotation-section command outside section position
//! ([`SectionRule`](super::SectionRule)), the same rule on every backend.
//!
//! Each condition's documentation states its recovery, what the lowering does
//! to go on after reporting it
//! ([recovery](crate::stdcore::backends::lowering::error)). Every recovery
//! keeps the content the author wrote in the output; only a misplaced
//! `\indent`, and the `\\` and `\indent` of a misplaced lines section, are
//! left out.

use techy::error::DiagnosticInfo;

/// Condition `flm.quote.bare-content`: content written in the body of a quote
/// environment that reads no bare content, such as `quote`, outside any
/// quotation-section command (text, a paragraph, a list). Reported once per
/// bare run: all the content between two quotation-section commands, or
/// before the first or after the last (a paragraph and the list after it are
/// one run); whitespace and source comments alone make no run. Recovery: the
/// content is shown in place, inside the quotation, outside any quotation
/// section.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.quote.bare-content",
    message = "content of the ‘{environment}’ environment must be written inside {sections}"
)]
pub struct BareContent {
    /// The environment's name.
    pub environment: String,
    /// The quotation-section commands the environment accepts, each written
    /// with its backslash, in the order `\text`, `\lines`, `\attributed`,
    /// `\block`, joined as an English list with `or`: `\text or \block`,
    /// `\text, \lines, \attributed or \block`.
    pub sections: String,
}

/// Condition `flm.quote.nested-section`: a quotation-section command written
/// anywhere other than directly in the body of a quote environment, for
/// example inside another quotation section's argument, as
/// in `\lines{\text{x}}`, where the quotation-section commands of the
/// enclosing body can still be written. (A command written directly in the
/// body of a quote environment that does not accept it is part of that
/// body's bare content, and is reported as such where the environment reads
/// none.) Recovery: the command is shown as its argument's content, with a
/// lines section's line ends and indentations left out.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.quote.nested-section",
    message = "\\{section} must be written directly in the body of the quotation environment"
)]
pub struct NestedSection {
    /// The quotation-section command's name, without the backslash: `text`.
    pub section: String,
}

/// Condition `flm.quote.indent-mid-line`: an `\indent` written after the
/// first content of its line in a lines section; only the `\indent` commands
/// at the start of a line indent it. Recovery: the `\indent` is left out.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.quote.indent-mid-line",
    message = "\\indent must be written at the start of a line"
)]
pub struct IndentMidLine;
