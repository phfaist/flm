//! The conditions the `floats` feature reports, each identified as
//! `flm.floats.<condition>`. A condition is the typed body of a diagnostic: a
//! value that says what happened and holds the facts describing it
//! ([diagnostics](crate::guide::concepts_overview#diagnostics)). Each
//! condition's documentation states its severity, who reports it, and the
//! recovery, what the feature does to go on after reporting it
//! ([recoveries](crate::stdcore::backends::lowering::error)).
//!
//! Every condition about a float's body (its commands, its content, its
//! number) is reported by the lowering rule that lowers the float, through
//! [`read_float`](super::read_float), which every such rule calls; a document
//! therefore gets the same diagnostics on every backend. These conditions are
//! reported while the float is lowered, so the `check` workflow
//! ([`CHECK_WORKFLOW`](crate::stdcore::CHECK_WORKFLOW)), which validates a
//! document and writes no output, does not report them. [`MissingHandler`] is reported by the
//! feature's stage [`CHECK_HANDLERS`](super::CHECK_HANDLERS), once per run,
//! on the `check` workflow too. [`StageFailed`] reports a fault of the
//! program rather than of the document.

use techy::error::DiagnosticInfo;

/// Condition `flm.floats.caption-repeated`, an Error: a float's body holds
/// more than one `\caption`. Reported at each `\caption` after the first.
/// Recovery: the first caption is the float's; the others are not shown.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.caption-repeated",
    message = "a ‘{float_type}’ must hold at most one \\caption"
)]
pub struct CaptionRepeated {
    /// The float's type.
    pub float_type: String,
}

/// Condition `flm.floats.caption-unknown-key`, an Error: the optional
/// argument of a float's caption holds something other than `alt=…`: another
/// key, or a text without `=` (`\caption[Short]{…}`), which is read as a key.
/// Reported at the caption. Recovery: that key is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.caption-unknown-key",
    message = "the only option of \\caption is alt=…, not ‘{key}’"
)]
pub struct CaptionUnknownKey {
    /// The key as written.
    pub key: String,
}

/// Condition `flm.floats.caption-missing-value`, an Error: the optional
/// argument of a float's caption holds `alt` with no `=` and no value.
/// Reported at the caption. Recovery: the key is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.caption-missing-value",
    message = "the option alt of \\caption must be given a value, as alt=…"
)]
pub struct CaptionMissingValue;

/// Condition `flm.floats.caption-repeated-key`, an Error: the optional
/// argument of a float's caption holds `alt=` more than once. Reported at the
/// caption. Recovery: the first `alt=` is kept.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.caption-repeated-key",
    message = "the option alt of \\caption must be written at most once"
)]
pub struct CaptionRepeatedKey;

/// Condition `flm.floats.nonumber-without-effect`, a Note: a `\nonumber`
/// changes nothing, because it is in a float of a type that is never
/// numbered, in a float without `\label` of a type numbered only when
/// labeled, or written a second time in the same float. Reported at the
/// `\nonumber`. Recovery: none is needed; the float is unnumbered.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.nonumber-without-effect",
    message = "\\nonumber has no effect here: {reason}"
)]
pub struct NonumberWithoutEffect {
    /// Why the `\nonumber` changes nothing, as the end of the message's
    /// sentence: `a ‘figure’ is never numbered`.
    pub reason: String,
}

/// Condition `flm.floats.label-without-refs`, a Note: a float carries a
/// `\label`, and the references feature, which resolves labels, is not
/// registered. The label still counts for a type numbered only when labeled.
/// Reported at the float's first label each time the float is lowered.
/// Recovery: none is needed; the float has no anchor.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.label-without-refs",
    message = "the \\label of this ‘{float_type}’ has no effect: the program processing this \
               document does not resolve references to labels"
)]
pub struct LabelWithoutRefs {
    /// The float's type.
    pub float_type: String,
}

/// Condition `flm.floats.no-numbering`, an Error: the float type's numbering
/// policy numbers a float, and the numbering feature, which gives the number,
/// is not registered. Reported at the float, on every backend. Recovery: the
/// float is shown unnumbered, the line under its content showing its caption
/// alone or being left out; the recomposed LaTeX, which leaves the numbering
/// to LaTeX, still writes the float as numbered ([`latex`](super::latex)).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.no-numbering",
    message = "this ‘{float_type}’ is numbered, but the program processing this document does \
               not number floats"
)]
pub struct NoNumbering {
    /// The float's type.
    pub float_type: String,
}

/// Condition `flm.floats.content-refused`, an Error: none of the content
/// handlers that the float's type lists accepts the float's content, for
/// example two images, or text, in a `figure`. The message lists what each
/// handler contributed by a registered feature would accept, in the type's
/// order; where none of the type's handlers is contributed, it says that none
/// of the kinds of content the type accepts is available ([`MissingHandler`]
/// reports which). Reported at the float. Recovery: the content is shown as
/// written, as a type that accepts any content would show it.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.floats.content-refused", message = "a ‘{float_type}’ must hold {admitted}")]
pub struct ContentRefused {
    /// The float's type.
    pub float_type: String,
    /// What the type's contributed handlers would accept, joined as an
    /// English list with `or` (`exactly one table of cells or exactly one
    /// image`); `content of one of the kinds it accepts, and the program
    /// processing this document handles none of those kinds` where none of
    /// the type's handlers is contributed.
    pub admitted: String,
}

/// Condition `flm.floats.missing-handler`, a Note: a float type used in the
/// document lists a content handler that no registered feature contributes,
/// so the kind of content that handler accepts cannot be held. The message
/// gives the kind by the handler's name (`includegraphics`, `cells`).
/// Reported by the stage [`CHECK_HANDLERS`](super::CHECK_HANDLERS), once per
/// run for each type and handler, at the first float of the type. Recovery:
/// the handler accepts nothing; the type's other handlers are tried.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.missing-handler",
    message = "a ‘{float_type}’ may hold content of the kind ‘{handler}’, but the program \
               processing this document does not handle that kind of content"
)]
pub struct MissingHandler {
    /// The float's type.
    pub float_type: String,
    /// The handler's name.
    pub handler: String,
}

/// Condition `flm.floats.stage-failed`, an Error: the feature's stage could
/// not read a table it declared, which happens only when the stage runs in a
/// plan its declarations do not describe: a fault of the program, not of the
/// document. Recovery: the stage reports nothing about the content
/// handlers.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.floats.stage-failed", message = "the stage `{stage}` failed: {detail}")]
pub struct StageFailed {
    /// The stage's name.
    pub stage: String,
    /// What failed, as the error's message.
    pub detail: String,
}
