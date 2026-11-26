//! This module defines the diagnostic conditions of the [`floats`](super)
//! feature. Their identifiers have the form `flm.floats.<condition>`. The
//! conditions [`NonumberWithoutEffect`], [`LabelWithoutRefs`] and
//! [`MissingHandler`] are notes, and the other conditions are errors.
//!
//! Every lowering rule of a float reads the float with
//! [`read_float`](super::read_float), which reports the problems of the float.
//! A document therefore gets the same reports on every backend. These
//! conditions are reported only when a float is lowered. The `check` workflow
//! ([`CHECK_WORKFLOW`](crate::stdlib::CHECK_WORKFLOW)) lowers no float, and
//! therefore does not report them. They concern the following problems:
//!
//! - a repeated `\caption`, or a mistake in the optional argument of
//!   `\caption` ([`CaptionRepeated`], [`CaptionUnknownKey`],
//!   [`CaptionMissingValue`], [`CaptionRepeatedKey`]);
//! - a `\nonumber` that has no effect ([`NonumberWithoutEffect`]);
//! - a `\label` in a registry without the references feature
//!   ([`LabelWithoutRefs`]);
//! - a float to be numbered in a registry without the numbering feature
//!   ([`NoNumbering`]);
//! - content that the float type does not accept ([`ContentRefused`]).
//!
//! The check stage [`CHECK_HANDLERS`](super::CHECK_HANDLERS) reports
//! [`MissingHandler`], also under the `check` workflow. The check stage
//! reports [`StageFailed`] when it fails to read a table. A document cannot
//! cause this error.

use techy::error::DiagnosticInfo;

/// The error that is reported when the body of a float contains more than one
/// `\caption` (`flm.floats.caption-repeated`).
///
/// Reported by the lowering rule of the float, on every backend, at each
/// `\caption` after the first. The first `\caption` gives the caption of the
/// float. The other `\caption` commands are left out of the output.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.caption-repeated",
    message = "A ‘{float_type}’ must hold at most one ‘\\caption’"
)]
pub struct CaptionRepeated {
    /// The name of the float type, e.g. `figure`.
    pub float_type: String,
}

/// The error that is reported when the optional argument of `\caption`
/// contains a key other than `alt` (`flm.floats.caption-unknown-key`).
///
/// The optional argument of `\caption` contains key-value pairs. Its only key
/// is `alt`, which gives the alternative text of the caption, as in
/// `\caption[alt=A plot]{A plot of the data, before and after.}`. A reference
/// to an unnumbered float shows the alternative text in place of the caption.
/// Unlike in LaTeX, the optional argument does not give a short caption.
///
/// Reported by the lowering rule of the float, on every backend, at the
/// `\caption`. The unknown key is ignored. A valid `alt` key in the same
/// argument is still used.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.caption-unknown-key",
    message = "Invalid ‘{key}’ in the optional argument of ‘\\caption’: \
               this argument may contain only ‘alt=…’"
)]
pub struct CaptionUnknownKey {
    /// The unknown key, as written. When the optional argument cannot be read
    /// as key-value pairs, this is the text of the whole argument.
    pub key: String,
}

/// The error that is reported when the key `alt` in the optional argument of
/// `\caption` is written without `=` (`flm.floats.caption-missing-value`).
///
/// The key `alt` must be followed by `=` and the alternative text of the
/// caption, as in `\caption[alt=A plot]{A plot of the data, before and
/// after.}`.
///
/// Reported by the lowering rule of the float, on every backend, at the
/// `\caption`. The key is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.caption-missing-value",
    message = "Missing value for key ‘alt’ of ‘\\caption’: use the syntax ‘alt=…’"
)]
pub struct CaptionMissingValue;

/// The error that is reported when the optional argument of `\caption`
/// contains `alt=…` more than once (`flm.floats.caption-repeated-key`).
///
/// Reported by the lowering rule of the float, on every backend, at the
/// `\caption`. The first value of `alt` is used, and the others are ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.caption-repeated-key",
    message = "Duplicate key ‘alt’ in the optional argument of ‘\\caption’"
)]
pub struct CaptionRepeatedKey;

/// The note that is reported when a `\nonumber` in the body of a float has no
/// effect (`flm.floats.nonumber-without-effect`).
///
/// The numbering policy of the float type
/// ([`FloatNumbering`](super::FloatNumbering)) determines whether a float is
/// numbered. A `\nonumber` has no effect in the following cases:
///
/// - The float type is never numbered
///   ([`Unnumbered`](super::FloatNumbering::Unnumbered)).
/// - The float type is numbered only when the float has a `\label`
///   ([`AutoNumbered`](super::FloatNumbering::AutoNumbered), the default), and
///   the float has no `\label`.
/// - The float contains an earlier `\nonumber`.
///
/// Reported by the lowering rule of the float, on every backend, at each such
/// `\nonumber`. The float is not numbered.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.nonumber-without-effect",
    message = "‘\\nonumber’ has no effect here: {reason}"
)]
pub struct NonumberWithoutEffect {
    /// The reason why the `\nonumber` has no effect, as the end of the
    /// message, e.g. `a ‘figure’ is never numbered`.
    pub reason: String,
}

/// The note that is reported when a float has a `\label` and the registry does
/// not contain the [`refs`](crate::stdlib::features::refs) feature (id
/// `flm.floats.label-without-refs`).
///
/// Without the references feature, `\ref` is not available, and nothing can
/// refer to the float. The label still counts for the numbering policy of the
/// float type: a float of a type that is numbered only when it has a `\label`
/// ([`AutoNumbered`](super::FloatNumbering::AutoNumbered)) is numbered.
///
/// Reported by the lowering rule of the float, on every backend, at the first
/// `\label` of the float. The float is otherwise lowered as usual.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.label-without-refs",
    message = "This ‘{float_type}’ cannot be referred to: references to labels are not available"
)]
pub struct LabelWithoutRefs {
    /// The name of the float type, e.g. `figure`.
    pub float_type: String,
}

/// The error that is reported when a float is to be numbered and the registry
/// does not contain the [`numbering`](crate::stdlib::features::numbering)
/// feature (id `flm.floats.no-numbering`).
///
/// The numbering policy of the float type
/// ([`FloatNumbering`](super::FloatNumbering)) determines whether a float is
/// numbered. In the default configuration, a `figure` or a `table` is
/// numbered when it has a `\label` and no `\nonumber`. Only the numbering
/// feature computes the numbers of floats.
///
/// Reported by the lowering rule of the float, on every backend, at the float.
/// On every backend except `latex-recomposed`, the float is written as an
/// unnumbered float: the line under its content contains only its caption, or
/// is left out when the float has no caption. On the `latex-recomposed`
/// backend, the float is written as numbered, and LaTeX numbers it.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.no-numbering",
    message = "Missing number for this ‘{float_type}’: numbering is not available"
)]
pub struct NoNumbering {
    /// The name of the float type, e.g. `figure`.
    pub float_type: String,
}

/// The error that is reported when the content of a float is not accepted by
/// its float type (`flm.floats.content-refused`).
///
/// A float type lists the content handlers that it accepts
/// ([`FloatType::handlers`](super::FloatType::handlers)). A content handler
/// ([`FloatContentHandler`](super::FloatContentHandler)) checks the content of
/// a float, and accepts it or refuses it. The content of a float is accepted
/// when one of the content handlers of its type accepts it. In the default
/// configuration, a `figure` must contain exactly one image, as in
/// `\begin{figure}\includegraphics{plot.png}\caption{A plot}\end{figure}`. A
/// `table` must contain exactly one table of cells or exactly one image. The
/// commands `\caption`, `\label` and `\nonumber`, whitespace and comments do
/// not count as content.
///
/// The message lists the content that each available content handler of the
/// type accepts. A content handler that is not available in the registry
/// accepts no content, and is reported as [`MissingHandler`]. When none of the
/// content handlers of the type is available, no content can be accepted, and
/// the message lists the missing content handlers instead.
///
/// Reported by the lowering rule of the float, on every backend, at the float.
/// The content is lowered as usual, as if the float type accepted any
/// content.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.content-refused",
    message = "A ‘{float_type}’ must hold {admitted}"
)]
pub struct ContentRefused {
    /// The name of the float type, e.g. `figure`.
    pub float_type: String,
    /// The content that the float type accepts, as the end of the message. It
    /// lists what each available content handler of the type accepts, joined
    /// with `or`, e.g. `exactly one table of cells or exactly one image`. When
    /// none of the content handlers of the type is available, it lists the
    /// missing content handlers instead, e.g. `content accepted by the missing
    /// content handler ‘includegraphics’`.
    pub admitted: String,
}

/// The note that is reported when a float type that is used in the document
/// lists a content handler that is not available in the registry (id
/// `flm.floats.missing-handler`).
///
/// A float type lists the content handlers that it accepts
/// ([`FloatType::handlers`](super::FloatType::handlers)). Each content handler
/// is contributed to the service slot [`CONTENT_SLOT`](super::CONTENT_SLOT) by
/// the feature that provides its kind of content: the `graphics` feature
/// contributes `includegraphics`, and the `cells` feature contributes `cells`.
/// A content handler is missing, for instance, when the registry does not
/// contain the feature that contributes it.
///
/// Reported by the check stage [`CHECK_HANDLERS`](super::CHECK_HANDLERS), also
/// under the `check` workflow, once for each float type and each missing
/// content handler of the type, at the first float of the type in the
/// document. The missing content handler accepts no content, and the other
/// content handlers of the type are tried. When none of the content handlers
/// of a float type is available, every float of the type is reported as
/// [`ContentRefused`].
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.floats.missing-handler",
    message = "Missing content handler ‘{handler}’ for float type ‘{float_type}’"
)]
pub struct MissingHandler {
    /// The name of the float type, e.g. `figure`.
    pub float_type: String,
    /// The name of the missing content handler, e.g. `includegraphics`.
    pub handler: String,
}

/// Error raised when the check stage of the floats feature fails to read a
/// table (id `flm.floats.stage-failed`).
///
/// The check stage [`CHECK_HANDLERS`](super::CHECK_HANDLERS) reads the element
/// index ([`ELEMENT_INDEX`](crate::stdlib::features::index::ELEMENT_INDEX)).
/// A table read fails only when the code of a feature is faulty, for instance
/// when the value type of the table differs from the declared one (see
/// [`AccessError`](crate::core::data::store::AccessError)). A document cannot
/// cause this error.
///
/// Reported with no position in the source. The check stage then stops, and
/// reports no [`MissingHandler`] for the document.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.floats.stage-failed", message = "Stage ‘{stage}’ failed: {detail}")]
pub struct StageFailed {
    /// The name of the stage, `flm.floats.check-handlers`.
    pub stage: String,
    /// The error message of the failed table read.
    pub detail: String,
}
