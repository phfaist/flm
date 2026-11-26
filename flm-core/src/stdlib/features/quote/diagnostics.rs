//! This module defines the diagnostic conditions that the [`quote`](super)
//! feature reports (see
//! [diagnostics](crate::guide::concepts_overview#diagnostics)). All three
//! conditions are errors in the source of a quotation.
//!
//! The lowering rules of the feature report these errors on every backend.
//! The `check` workflow ([`CHECK_WORKFLOW`](crate::stdlib::CHECK_WORKFLOW))
//! lowers nothing, and therefore does not report them.

use techy::error::DiagnosticInfo;

/// The error that is reported when a quote environment such as `quote`
/// contains content outside its quotation-section commands (id
/// `flm.quote.bare-content`).
///
/// The configuration of a quote environment
/// ([`QuoteEnvironment`](super::QuoteEnvironment)) declares the
/// quotation-section commands (`\text`, `\lines`, `\attributed` and `\block`)
/// that its body accepts. It can also declare a kind of quotation section for
/// the content written outside these commands
/// ([`bare`](super::QuoteEnvironment::bare)). If it declares none, as for the
/// default environment `quote`, all content must be written inside the
/// accepted commands, as in
/// `\begin{quote}\text{Veni, vidi, vici.}\attributed{Caesar}\end{quote}`.
/// Whitespace and comments outside the commands are allowed.
///
/// Reported by the lowering rule of the quote environment
/// ([`QuoteRule`](super::QuoteRule), or
/// [`QuoteLatexRule`](super::latex::QuoteLatexRule) on the `latex-recomposed`
/// backend) when it is invoked. The error is reported once for each run of
/// content outside the commands: all the content between two
/// quotation-section commands, or before the first, or after the last. This
/// content is otherwise written as usual, at its place in the quotation and
/// outside any quotation section.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.quote.bare-content",
    message = "Content of the ‘{environment}’ environment must be written inside {sections}"
)]
pub struct BareContent {
    /// The environment's name.
    pub environment: String,
    /// The quotation-section commands that the environment accepts, quoted
    /// and listed as in an English sentence, e.g.
    /// `‘\text’, ‘\lines’ or ‘\block’`.
    pub sections: String,
}

/// Error raised when a quotation-section command is not written directly in
/// the body of a quote environment (`flm.quote.nested-section`).
///
/// A quotation-section command (`\text`, `\lines`, `\attributed` or `\block`)
/// starts a quotation section. It must be written directly in the body of a
/// quote environment, as in
/// `\begin{quote}\text{Veni, vidi, vici.}\attributed{Caesar}\end{quote}`.
///
/// Reported by the lowering rule of the quotation-section commands
/// ([`SectionRule`](super::SectionRule)) when it is invoked. The command
/// expands to the content of its argument. For a `\lines` command, the `\\`
/// and `\indent` commands in the argument are left out.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.quote.nested-section",
    message = "‘\\{section}’ must be written directly in the body of the quotation environment"
)]
pub struct NestedSection {
    /// The quotation-section command's name, without the backslash: `text`.
    pub section: String,
}

/// Error raised when an `\indent` command is not written at the start of a
/// line (`flm.quote.indent-mid-line`).
///
/// A lines section of a quotation is the argument of a `\lines` command, or
/// the body of a quote environment whose content is read as lines, such as
/// `address`. In a lines section, each `\indent` command at the start of a
/// line indents the line by one tab, as in
/// `\lines{First line\\ \indent Second line}`. The `\indent` commands of a
/// line must come before all other content of the line.
///
/// Reported by the lowering rule of the quote environment
/// ([`QuoteRule`](super::QuoteRule), or
/// [`QuoteLatexRule`](super::latex::QuoteLatexRule) on the `latex-recomposed`
/// backend) when it is invoked. The `\indent` command is left out. The line
/// is otherwise written as usual, indented by the `\indent` commands at its
/// start.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.quote.indent-mid-line",
    message = "‘\\indent’ must be written at the start of a line"
)]
pub struct IndentMidLine;
