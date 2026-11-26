//! This module reads the options of an image, which are written in the
//! optional argument of `\includegraphics[options]{path}`. It also defines
//! the diagnostic conditions that are reported for these options.
//!
//! The options are key-value pairs separated by commas, as in
//! `\includegraphics[alt=A plot, width=5cm]{plot.png}`. A value that contains
//! a comma is written in braces, as in `alt={A plot, before and after}`. Three
//! keys are valid:
//!
//! - `alt` ([`ALT_KEY`]): the alternative text of the image, which must be
//!   plain text.
//! - `width` ([`WIDTH_KEY`]) and `height` ([`HEIGHT_KEY`]): the size of the
//!   image. The value is a length greater than zero, in one of the units `pt`,
//!   `mm`, `cm` or `in`, as in `5cm` ([`Length`] describes the units).
//!
//! [`image_options`] returns the options of an image, and [`report`] reports
//! the errors and warnings for these options. Each lowering rule of
//! `\includegraphics` (element [`IMAGE_ELEMENT`](super::IMAGE_ELEMENT)) calls
//! both functions when it lowers an image: the rule
//! [`image_rule`](super::image_rule) on the plain-text, HTML and
//! `latex-rendered` backends, and the rule
//! [`LATEX_RULE`](super::latex::LATEX_RULE) on the `latex-recomposed`
//! backend.
//!
//! An invalid option is reported as an error and ignored, and the image is
//! otherwise written as usual. Each `width` or `height` that is used is
//! reported with the warning [`Resized`]. The diagnostics are reported each
//! time the image is lowered. An image is lowered again for each copy of it
//! in an output, for instance in the title of a heading that a `\ref` expands
//! to, and its diagnostics are then reported again. The `check` workflow
//! ([`CHECK_WORKFLOW`](crate::stdlib::CHECK_WORKFLOW)) lowers nothing, and
//! therefore does not report them.

use std::collections::BTreeSet;

use techy::core::node::NodeRef;
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::extract::parse_keyval_drop_annotations;

use crate::core::lowering::{LowerError, Lowerer, chars_at_lowering};
use crate::lang::Flm;
use crate::lang::text::{is_space, plain_text_of};
use crate::stdlib::backends::pres::{I48F16, Length};

use super::OPTIONS_ARG;

/// The key of the alternative text, `alt`.
pub const ALT_KEY: &str = "alt";

/// The key of the width, `width`.
pub const WIDTH_KEY: &str = "width";

/// The key of the height, `height`.
pub const HEIGHT_KEY: &str = "height";

/// The keys that are valid in the options of `\includegraphics`: `alt`,
/// `width` and `height`.
pub const KEYS: [&str; 3] = [ALT_KEY, WIDTH_KEY, HEIGHT_KEY];

/// The options of an image, as returned by [`image_options`].
///
/// A field is `None` when its key is not written in the options, or when the
/// option is invalid. [`report`] reports the errors for the invalid options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageOptions {
    /// The alternative text: the value of `alt`, with the whitespace at both
    /// ends removed.
    pub alt: Option<String>,
    /// The width of the image: the value of `width`, a length greater than
    /// zero in one of the units `pt`, `mm`, `cm` or `in`.
    pub width: Option<Length>,
    /// The height of the image: the value of `height`, a length greater than
    /// zero in one of the units `pt`, `mm`, `cm` or `in`.
    pub height: Option<Length>,
}

/// A finding of [`read`] in the options of an image: an invalid option, or a
/// `width` or `height` that is used. [`report`] reports one diagnostic for
/// each finding.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Finding {
    /// The optional argument is not a list of key-value pairs with
    /// plain-text keys.
    Unreadable,
    /// A value written with no key before its `=`.
    NoKey,
    /// A key that is not one of [`KEYS`].
    Unknown(String),
    /// A key written a second time.
    Repeated(String),
    /// A key written without `=` and a value.
    NoValue(String),
    /// An `alt=` value that is not plain text.
    AltNotText,
    /// A `width=` or `height=` value that is not a length.
    NotALength { key: String, value: String },
    /// A `width=` or `height=` length in a unit with no fixed size.
    NotPhysical { key: String, unit: String },
    /// A `width=` or `height=` length that is zero or negative.
    NotPositive { key: String, value: String },
    /// A `width=` or `height=` that takes effect.
    Resized(String),
}

/// Error raised when a key in the options of `\includegraphics` is not plain
/// text (id `flm.graphics.unreadable-options`).
///
/// Each key must be plain text, as in
/// `\includegraphics[alt=A plot, width=5cm]{plot.png}`. An option written
/// without `=` consists of its key alone.
///
/// Reported by the lowering rule of `\includegraphics` when it is invoked.
/// The whole optional argument is ignored, and the image is written as if it
/// had no options.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.unreadable-options",
    message = "Invalid key in the options of ‘\\includegraphics’: keys must be plain text"
)]
pub struct UnreadableOptions;

/// Error raised when an option of `\includegraphics` has a value but no key
/// (id `flm.graphics.missing-key`).
///
/// An option consists of a key, `=` and a value, as in `width=5cm`.
///
/// Reported by the lowering rule of `\includegraphics` when it is invoked.
/// The value is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.missing-key",
    message = "Missing key before ‘=’ in the options of ‘\\includegraphics’"
)]
pub struct MissingKey;

/// Error raised when the options of `\includegraphics` contain an unknown key
/// (id `flm.graphics.unknown-key`).
///
/// The valid keys are `alt`, `width` and `height` ([`KEYS`]).
///
/// Reported by the lowering rule of `\includegraphics` when it is invoked.
/// The key and its value are ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.unknown-key",
    message = "Invalid key ‘{key}’ in the options of ‘\\includegraphics’: \
               valid keys are ‘alt’, ‘width’ and ‘height’"
)]
pub struct UnknownKey {
    /// The key as written.
    pub key: String,
}

/// Error raised when a key is written more than once in the options of
/// `\includegraphics` (id `flm.graphics.repeated-key`).
///
/// Reported by the lowering rule of `\includegraphics` when it is invoked,
/// once for each repetition of the key. Only the first occurrence of the key
/// is read. Each later occurrence is ignored, even when the first occurrence
/// is itself reported as an error.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.repeated-key",
    message = "Duplicate key ‘{key}’ in the options of ‘\\includegraphics’"
)]
pub struct RepeatedKey {
    /// The key as written.
    pub key: String,
}

/// Error raised when a key in the options of `\includegraphics` is written
/// without `=` (id `flm.graphics.missing-value`).
///
/// Each key must be followed by `=` and its value, as in `alt=A plot`.
///
/// Reported by the lowering rule of `\includegraphics` when it is invoked.
/// The key is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.missing-value",
    message = "Missing value for key ‘{key}’ of ‘\\includegraphics’: use the syntax ‘{key}=…’"
)]
pub struct MissingValue {
    /// The key as written.
    pub key: String,
}

/// Error raised when the value of `alt` in the options of `\includegraphics`
/// is not plain text (id `flm.graphics.alt-not-text`).
///
/// The alternative text must be plain text, that is, text without commands
/// or math, as in `alt=A plot` or `alt={A plot, before and after}`.
///
/// Reported by the lowering rule of `\includegraphics` when it is invoked.
/// The image is otherwise written as usual, without alternative text.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.alt-not-text",
    message = "The ‘alt’ text of an image must be plain text, without formulas or commands"
)]
pub struct AltNotText;

/// Error raised when the value of `width` or `height` in the options of
/// `\includegraphics` is not a length (id `flm.graphics.not-a-length`).
///
/// A length is a decimal number followed directly by a unit, as in `5cm`,
/// `2.5in` or `144pt`. The valid units are `pt`, `mm`, `cm` and `in`
/// ([`Length`] describes them). A length in the unit `em` or `tab` is
/// reported as [`RelativeUnit`].
///
/// Reported by the lowering rule of `\includegraphics` when it is invoked.
/// The key is ignored, and the size of the image is determined as if the key
/// were not written.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.not-a-length",
    message = "Invalid length ‘{value}’ for key ‘{key}’ of ‘\\includegraphics’: a length must be \
               a number followed directly by a unit, as in ‘5cm’ (supported units: ‘pt’, ‘mm’, \
               ‘cm’ and ‘in’)"
)]
pub struct NotALength {
    /// The key, `width` or `height`.
    pub key: String,
    /// The value as written, trimmed. When the value contains commands, this
    /// is its plain text ([`plain_text_of`]).
    pub value: String,
}

/// Error raised when the value of `width` or `height` in the options of
/// `\includegraphics` is a length in the unit `em` or `tab` (id
/// `flm.graphics.relative-unit`).
///
/// The size of an image must be given in a unit of fixed size, `pt`, `mm`,
/// `cm` or `in`, as in `width=5cm`. The units `em` and `tab` have no fixed
/// size: an em depends on the font, and each backend sets the width of a tab.
///
/// Reported by the lowering rule of `\includegraphics` when it is invoked.
/// The key is ignored, and the size of the image is determined as if the key
/// were not written.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.relative-unit",
    message = "Unsupported unit ‘{unit}’ for key ‘{key}’ of ‘\\includegraphics’ (supported units: \
               ‘pt’, ‘mm’, ‘cm’ and ‘in’)"
)]
pub struct RelativeUnit {
    /// The key, `width` or `height`.
    pub key: String,
    /// The unit as written, `em` or `tab`.
    pub unit: String,
}

/// Error raised when the value of `width` or `height` in the options of
/// `\includegraphics` is zero or negative (id `flm.graphics.not-positive`).
///
/// Reported by the lowering rule of `\includegraphics` when it is invoked.
/// The key is ignored, and the size of the image is determined as if the key
/// were not written.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.not-positive",
    message = "Invalid length ‘{value}’ for key ‘{key}’ of ‘\\includegraphics’: the length must be \
               greater than zero"
)]
pub struct NotPositive {
    /// The key, `width` or `height`.
    pub key: String,
    /// The length, as written and trimmed.
    pub value: String,
}

/// The warning that is reported when the size of an image is set with the key
/// `width` or `height` of `\includegraphics` (id `flm.graphics.resized`).
///
/// An image file that is prepared at the size at which the image is
/// displayed, with a resolution suited to that size, looks better than an
/// image file that is resized in the output.
///
/// Reported by the lowering rule of `\includegraphics` when it is invoked,
/// once for each `width` and each `height` that is used. The image is written
/// at the given size.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.resized",
    message = "Image resized with key ‘{key}’ of ‘\\includegraphics’: \
               for the best quality, prepare the image file at the displayed size"
)]
pub struct Resized {
    /// The key, `width` or `height`.
    pub key: String,
}

/// Returns the options of the image `node`, a node of `\includegraphics`.
///
/// The result contains only the valid options (see [`ImageOptions`]). This
/// function reports no diagnostic; call [`report`] to report the errors and
/// warnings for the options.
///
/// # Errors
///
/// Returns [`LowerError::PlaceholderCharacters`] when the value of `alt`,
/// `width` or `height` contains a placeholder of a user definition's
/// template, such as `#1`. This happens only when the template is lowered
/// without being expanded (see [`LowerError::PlaceholderCharacters`]).
pub fn image_options(node: &NodeRef<'_, Flm>) -> Result<ImageOptions, LowerError> {
    Ok(read(node)?.0)
}

/// Reports the errors and warnings for the options of the image `node`
/// through `lw`.
///
/// One error is reported for each invalid option, and one [`Resized`] warning
/// for each `width` and each `height` that is used, in the order in which
/// the options are written. Each diagnostic is located at `node`.
///
/// # Errors
///
/// Returns [`LowerError::PlaceholderCharacters`] in the same cases as
/// [`image_options`].
pub fn report(node: &NodeRef<'_, Flm>, lw: &mut dyn Lowerer) -> Result<(), LowerError> {
    let (_, findings) = read(node)?;
    for finding in findings {
        let span = node.span().clone();
        lw.diagnose(match finding {
            Finding::Unreadable => Diagnostic::error(UnreadableOptions, span),
            Finding::NoKey => Diagnostic::error(MissingKey, span),
            Finding::Unknown(key) => Diagnostic::error(UnknownKey { key }, span),
            Finding::Repeated(key) => Diagnostic::error(RepeatedKey { key }, span),
            Finding::NoValue(key) => Diagnostic::error(MissingValue { key }, span),
            Finding::AltNotText => Diagnostic::error(AltNotText, span),
            Finding::NotALength { key, value } => {
                Diagnostic::error(NotALength { key, value }, span)
            }
            Finding::NotPhysical { key, unit } => {
                Diagnostic::error(RelativeUnit { key, unit }, span)
            }
            Finding::NotPositive { key, value } => {
                Diagnostic::error(NotPositive { key, value }, span)
            }
            Finding::Resized(key) => Diagnostic::warning(Resized { key }, span),
        });
    }
    Ok(())
}

/// Reads the options of the image `node`. Returns the valid options, and the
/// findings in the order in which the options are written.
fn read(node: &NodeRef<'_, Flm>) -> Result<(ImageOptions, Vec<Finding>), LowerError> {
    let mut options = ImageOptions::default();
    let mut findings = Vec::new();
    let Ok(Some(content)) = node.argument_content_nodes_named(OPTIONS_ARG) else {
        return Ok((options, findings));
    };
    let Ok(keyvals) = parse_keyval_drop_annotations(content) else {
        findings.push(Finding::Unreadable);
        return Ok((options, findings));
    };
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for entry in keyvals.iter() {
        let key = entry.key();
        if key.is_empty() {
            // An empty entry, as between the commas of `[a=1,,b=2]`, says
            // nothing; a value with no key is refused.
            if entry.value_content().is_some() {
                findings.push(Finding::NoKey);
            }
            continue;
        }
        if !KEYS.contains(&key) {
            findings.push(Finding::Unknown(key.to_string()));
            continue;
        }
        if !seen.insert(key) {
            findings.push(Finding::Repeated(key.to_string()));
            continue;
        }
        let Some(value) = entry.value_content() else {
            findings.push(Finding::NoValue(key.to_string()));
            continue;
        };
        let text = chars_at_lowering(value.iter())?.map(|text| text.trim_matches(is_space).to_string());
        if key == ALT_KEY {
            match text {
                Ok(text) => options.alt = Some(text),
                Err(_) => findings.push(Finding::AltNotText),
            }
            continue;
        }
        let written = match text {
            Ok(text) => text,
            Err(_) => plain_text_of(value.iter()).trim_matches(is_space).to_string(),
        };
        match dimension(key, &written) {
            Ok(length) => {
                // A Warning for a dimension taken, none for one refused: a
                // refused dimension resizes nothing, and its Error says why.
                if key == WIDTH_KEY {
                    options.width = Some(length);
                } else {
                    options.height = Some(length);
                }
                findings.push(Finding::Resized(key.to_string()));
            }
            Err(finding) => findings.push(finding),
        }
    }
    Ok((options, findings))
}

/// Parses `written`, the value of the key `key` (`width` or `height`), as a
/// length. Returns the finding for the error when the value is not a length,
/// when its unit is `em` or `tab`, or when it is not greater than zero.
fn dimension(key: &str, written: &str) -> Result<Length, Finding> {
    let length = Length::parse(written).map_err(|_| Finding::NotALength {
        key: key.to_string(),
        value: written.to_string(),
    })?;
    if !length.unit.is_physical() {
        return Err(Finding::NotPhysical { key: key.to_string(), unit: length.unit.to_string() });
    }
    if length.amount <= I48F16::ZERO {
        return Err(Finding::NotPositive { key: key.to_string(), value: written.to_string() });
    }
    Ok(length)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dimension_is_a_positive_physical_length() {
        assert_eq!(dimension("width", "5cm"), Ok(Length::parse("5cm").unwrap()));
        assert_eq!(dimension("width", ".5in"), Ok(Length::parse("0.5in").unwrap()));
        for text in ["300px", "0.5\\linewidth", "50%", "10bp", "", "5"] {
            assert!(matches!(dimension("width", text), Err(Finding::NotALength { .. })), "{text}");
        }
        assert!(matches!(dimension("width", "1.5em"), Err(Finding::NotPhysical { .. })));
        assert!(matches!(dimension("height", "2tab"), Err(Finding::NotPhysical { .. })));
        assert!(matches!(dimension("width", "0cm"), Err(Finding::NotPositive { .. })));
        assert!(matches!(dimension("width", "-1pt"), Err(Finding::NotPositive { .. })));
    }
}
