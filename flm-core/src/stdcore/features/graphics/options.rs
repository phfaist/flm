//! Reading an image's options, `alt=`, `width=` and `height=`
//! ([`image_options`]), and the conditions the reading reports ([`report`]).
//!
//! Every lowering rule of the image element reads the options where the image
//! is lowered, through techy's key-value reader: pairs separated by `,`, the
//! first `=` of a pair separating the key from the value, and a value in
//! braces read without them. Each value is read as characters
//! ([`chars_at_lowering`]) and trimmed; the values of `width=` and `height=`
//! are then read as a [`Length`] by [`Length::parse`], and must be in a
//! physical unit (`pt`, `mm`, `cm`, `in`) and greater than zero.
//!
//! An option the reading cannot take is an Error, and the option is ignored.
//! Each `width=` or `height=` that takes effect is reported with the Warning
//! [`Resized`]. The rules report these conditions, in the order written,
//! wherever the image is lowered: once per output where the image is
//! written, and again wherever a synthesized value shows it a second time,
//! such as a heading's title that a reference shows. A synthesized value is
//! a small tree that a stage builds while a run is under way, held outside
//! the document's trees ([synthesized
//! values](crate::guide::concepts_overview#synthesized-values)).

use std::collections::BTreeSet;

use techy::core::node::NodeRef;
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::extract::parse_keyval_drop_annotations;

use crate::core::lowering::{LowerError, Lowerer, chars_at_lowering};
use crate::lang::Flm;
use crate::lang::text::plain_text_of;
use crate::stdcore::backends::pres::{I48F16, Length};

use super::OPTIONS_ARG;

/// The key of the alternative text, `alt`.
pub const ALT_KEY: &str = "alt";

/// The key of the width, `width`.
pub const WIDTH_KEY: &str = "width";

/// The key of the height, `height`.
pub const HEIGHT_KEY: &str = "height";

/// The three keys an image's options accept: `alt`, `width` and `height`.
pub const KEYS: [&str; 3] = [ALT_KEY, WIDTH_KEY, HEIGHT_KEY];

/// An image's options as [`image_options`] reads them, without the options
/// it refused.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageOptions {
    /// The alternative text, the value of `alt=`, trimmed.
    pub alt: Option<String>,
    /// The width, the value of `width=`: a length in a physical unit, greater
    /// than zero.
    pub width: Option<Length>,
    /// The height, the value of `height=`: a length in a physical unit,
    /// greater than zero.
    pub height: Option<Length>,
}

/// One option the reading of an image's options could not take, or one
/// dimension that takes effect, in the order written.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Finding {
    /// The optional argument is not a list of key-value pairs with
    /// plain-text keys.
    Unreadable,
    /// A value written with no key before its `=`.
    NoKey,
    /// A key the image does not accept.
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

/// Condition `flm.graphics.unreadable-options`, an Error: an image's optional
/// argument cannot be read as key-value pairs, because a key is not plain
/// text (a command is written where a key would be). Recovery: the whole
/// optional argument is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.unreadable-options",
    message = "the options of \\includegraphics must be key-value pairs whose keys are plain text"
)]
pub struct UnreadableOptions;

/// Condition `flm.graphics.missing-key`, an Error: an image's options hold
/// `=` and a value with no key before it, as in `[=5cm]`. Recovery: the entry
/// is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.missing-key",
    message = "each option of \\includegraphics must give its key before `=`"
)]
pub struct MissingKey;

/// Condition `flm.graphics.unknown-key`, an Error: an image's options hold a
/// key other than `alt`, `width` and `height`. Recovery: the key is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.unknown-key",
    message = "\\includegraphics accepts only the options `alt`, `width` and `height`, not `{key}`"
)]
pub struct UnknownKey {
    /// The key as written.
    pub key: String,
}

/// Condition `flm.graphics.repeated-key`, an Error: an image's options hold
/// one key twice. Recovery: the first is kept and the second ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.repeated-key",
    message = "the option `{key}` of \\includegraphics may be given only once"
)]
pub struct RepeatedKey {
    /// The key as written.
    pub key: String,
}

/// Condition `flm.graphics.missing-value`, an Error: an image's options hold
/// a key with no `=` and no value. Recovery: the key is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.missing-value",
    message = "the option `{key}` of \\includegraphics must be given a value"
)]
pub struct MissingValue {
    /// The key as written.
    pub key: String,
}

/// Condition `flm.graphics.alt-not-text`, an Error: an image's `alt=` value
/// holds something that is not plain text, such as a formula or a command.
/// Recovery: the image has no alternative text.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.alt-not-text",
    message = "the `alt` text of an image must be plain text, without formulas or commands"
)]
pub struct AltNotText;

/// Condition `flm.graphics.not-a-length`, an Error: an image's `width=` or
/// `height=` value is not a length that [`Length::parse`] reads, such as
/// `300px`, `0.5\linewidth`, `50%`, `10bp` or an empty value. Recovery: the
/// option is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.not-a-length",
    message = "the `{key}` of an image must be a number followed directly by one of the units pt, \
               mm, cm or in, such as `5cm`, not `{value}`"
)]
pub struct NotALength {
    /// The key, `width` or `height`.
    pub key: String,
    /// The value, as written and trimmed.
    pub value: String,
}

/// Condition `flm.graphics.relative-unit`, an Error: an image's `width=` or
/// `height=` is a length in `em` or `tab`, units without a fixed size.
/// Recovery: the option is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.relative-unit",
    message = "the `{key}` of an image must be given in pt, mm, cm or in, not in `{unit}`"
)]
pub struct RelativeUnit {
    /// The key, `width` or `height`.
    pub key: String,
    /// The unit as written, `em` or `tab`.
    pub unit: String,
}

/// Condition `flm.graphics.not-positive`, an Error: an image's `width=` or
/// `height=` is zero or negative. Recovery: the option is ignored, and the
/// image keeps the size it would have had without it.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.not-positive",
    message = "the `{key}` of an image must be greater than zero, not `{value}`"
)]
pub struct NotPositive {
    /// The key, `width` or `height`.
    pub key: String,
    /// The length, as written and trimmed.
    pub value: String,
}

/// Condition `flm.graphics.resized`, a Warning: an image is given a size by
/// `width=` or `height=`, one Warning per such option that takes effect. An
/// image file prepared at the size it is shown at, with a resolution suited to
/// that size, looks better than one resized by the output. Recovery: none is
/// needed; the image is shown at the size given.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.graphics.resized",
    message = "an image file should be prepared at the size it is shown at, rather than resized \
               with `{key}=`"
)]
pub struct Resized {
    /// The key, `width` or `height`.
    pub key: String,
}

/// Returns the options of the image `node`, without the options the reading
/// refused (see the [module documentation](self)). Reports nothing; call
/// [`report`] to report the problems.
///
/// # Errors
///
/// [`LowerError::PlaceholderCharacters`] where a value holds a placeholder of
/// a template that is lowered as written (the template of a kept
/// definition), whose characters only a use site supplies.
pub fn image_options(node: &NodeRef<'_, Flm>) -> Result<ImageOptions, LowerError> {
    Ok(read(node)?.0)
}

/// Reports through `lw`, at `node`, what reading the options of the image
/// `node` found: one Error per option the reading refused and one Warning per
/// dimension that takes effect, in the order written.
///
/// # Errors
///
/// [`LowerError::PlaceholderCharacters`] as [`image_options`].
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

/// Reads the options of the image `node`: returns the options taken and the
/// findings in the order written. [`image_options`] and [`report`] call it.
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
        let text = chars_at_lowering(value.iter())?.map(|text| text.trim().to_string());
        if key == ALT_KEY {
            match text {
                Ok(text) => options.alt = Some(text),
                Err(_) => findings.push(Finding::AltNotText),
            }
            continue;
        }
        let written = match text {
            Ok(text) => text,
            Err(_) => plain_text_of(value.iter()).trim().to_string(),
        };
        match dimension(key, &written) {
            Ok(length) => {
                // A Warning for a dimension taken, none for one refused.
                // See decision rationale at [§flm-dd-dr:std-graphics-options].
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

/// Returns the length that the `width=` or `height=` value `written` gives,
/// or the problem that prevents it.
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
