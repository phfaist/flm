//! Where an FLM source comes from ([`FlmOrigin`]), and the type aliases for FLM
//! sources, spans and diagnostics.
//!
//! techy attaches an **origin** to every source: a small value saying where the
//! text comes from. FLM's origin is [`FlmOrigin`]. For a file, it holds the
//! file's *canonical path*: a key that the embedding program chooses, usually
//! the file's path. The canonical path has three uses: it is the label a diagnostic
//! shows for the source, the key by which the library detects a file that
//! includes itself through `\input`, and the key of the tables an embedding
//! program keeps per source. An origin records where the text comes from, not
//! what it contains, so the front matter of a file is not part of it.
//!
//! Two kinds of source have no canonical path:
//!
//! - A source with no name at all (a snippet held in memory) carries
//!   [`FlmOrigin::none`]. It has no label, and it is left out of every check
//!   keyed by source, so it cannot be confused with another unnamed source.
//! - A **synthetic** source, which the library creates rather than reads from a
//!   file (for example, the source created for one use of a [user
//!   definition](crate::lang::rewrites::userdefs::Definition)), carries
//!   [`FlmOrigin::synthetic`] with a label saying what it is (`expansion of
//!   \foo`). A diagnostic raised inside it can then say where it was raised.
//!   The label is shown, never used as a key. The source a user definition's
//!   own text is compiled from (its template, labeled `template of \foo (text
//!   mode)`, or an argument's default value) carries [`FlmOrigin::definition`]
//!   instead: a synthetic origin that also marks the text as a user
//!   definition's own text ([`FlmOrigin::is_definition`]), in which `\input`
//!   includes no file.
//!
//! The four type aliases [`FlmSource`], [`FlmSpan`], [`FlmDiagnostic`] and
//! [`FlmDiagnostics`] are techy's source, span and diagnostic types with
//! [`FlmOrigin`] as their origin type; the whole library uses them.
//!
//! Serialized form of an origin: a map with the keys `canonical_path` and
//! `label`, each a string or absent, and `definition`, `true` or absent.

use std::borrow::Cow;

use techy::error::{Diagnostic, Diagnostics};
use techy::serialize::{DeserializableValue, SerializableValue};
use techy::source::{Source, SourceOrigin, SourceSpan};

/// Where an FLM source comes from: its canonical path when it has one, or the
/// label of a source the library created.
///
/// Create an origin with [`FlmOrigin::new`] for a file, [`FlmOrigin::none`]
/// for an unnamed snippet, [`FlmOrigin::synthetic`] or
/// [`FlmOrigin::definition`] for a source the library creates, and attach it to
/// a source with `FlmSource::with_origin`. The [module documentation](self)
/// explains what the canonical path is used for.
///
/// # Examples
///
/// ```
/// use flm_core::lang::FlmOrigin;
/// use flm_core::techy::source::SourceOrigin;
///
/// let file = FlmOrigin::new("book/ch1.flm");
/// assert_eq!(file.canonical_path(), Some("book/ch1.flm"));
/// assert_eq!(file.label().as_deref(), Some("book/ch1.flm"));
///
/// // A source the library creates has a label but no path, so no check keyed
/// // by source applies to it.
/// let made = FlmOrigin::synthetic("expansion of \\foo");
/// assert_eq!(made.canonical_path(), None);
/// assert_eq!(made.label().as_deref(), Some("expansion of \\foo"));
///
/// // An unnamed snippet has neither.
/// assert_eq!(FlmOrigin::none(), FlmOrigin::default());
/// assert_eq!(FlmOrigin::none().label(), None);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(SerializableValue, DeserializableValue)]
pub struct FlmOrigin {
    #[serial(name = "canonical_path")]
    canonical_path: Option<String>,
    #[serial(name = "label")]
    label: Option<String>,
    #[serial(name = "definition")]
    definition: Option<bool>,
}

impl FlmOrigin {
    /// Returns an origin whose canonical path is `canonical_path`.
    pub fn new(canonical_path: impl Into<String>) -> FlmOrigin {
        FlmOrigin { canonical_path: Some(canonical_path.into()), label: None, definition: None }
    }

    /// Returns the origin of a source with no name at all. This is also the
    /// `Default`.
    pub fn none() -> FlmOrigin {
        FlmOrigin { canonical_path: None, label: None, definition: None }
    }

    /// Returns the origin of a source the library creates: no canonical path,
    /// so no check keyed by source applies to it, and `label` as the name that
    /// diagnostics show.
    pub fn synthetic(label: impl Into<String>) -> FlmOrigin {
        FlmOrigin { canonical_path: None, label: Some(label.into()), definition: None }
    }

    /// Returns the origin of the source a user definition's own text is
    /// compiled from (its template, or the default value of one of its
    /// arguments): a [synthetic](FlmOrigin::synthetic) origin labeled `label`,
    /// marked as a user definition's own text
    /// ([`is_definition`](Self::is_definition)).
    pub fn definition(label: impl Into<String>) -> FlmOrigin {
        FlmOrigin { canonical_path: None, label: Some(label.into()), definition: Some(true) }
    }

    /// Returns whether this is the origin of a user definition's own text (its
    /// template or an argument's default value), created with
    /// [`FlmOrigin::definition`].
    ///
    /// The parser of `\input` reads it: an `\input` written in such text
    /// includes no file and is reported
    /// ([`InputInDefinition`](crate::lang::input::InputInDefinition)).
    pub fn is_definition(&self) -> bool {
        self.definition == Some(true)
    }

    /// Returns the canonical path, when there is one. It is the key of every
    /// check and table keyed by source.
    pub fn canonical_path(&self) -> Option<&str> {
        self.canonical_path.as_deref()
    }

    /// Returns the label of a source the library created, when there is one.
    /// The label a diagnostic shows is [`SourceOrigin::label`], which prefers
    /// the canonical path.
    pub fn synthetic_label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}

impl SourceOrigin for FlmOrigin {
    /// Returns what a diagnostic shows for this source: the canonical path, or
    /// the label when there is no path, or nothing when there is neither.
    fn label(&self) -> Option<Cow<'_, str>> {
        self.canonical_path.as_deref().or(self.label.as_deref()).map(Cow::Borrowed)
    }
}

/// An FLM source: a text with its [`FlmOrigin`]. techy's [`Source`] with
/// [`FlmOrigin`] as its origin type.
pub type FlmSource = Source<FlmOrigin>;

/// A range of bytes in an FLM source, carrying the source it belongs to.
/// techy's [`SourceSpan`] with [`FlmOrigin`] as its origin type.
pub type FlmSpan = SourceSpan<FlmOrigin>;

/// One problem reported about FLM content: a condition, a severity and a span
/// (see [diagnostics](crate::guide::concepts_overview#diagnostics)). techy's
/// [`Diagnostic`] with [`FlmOrigin`] as its origin type.
pub type FlmDiagnostic = Diagnostic<FlmOrigin>;

/// A collection of [`FlmDiagnostic`]s, with the cap on how many are kept and the
/// count of those dropped past the cap. techy's [`Diagnostics`] with
/// [`FlmOrigin`] as its origin type.
pub type FlmDiagnostics = Diagnostics<FlmOrigin>;

/// Returns an empty span over an empty source with `origin`, for a condition
/// that has no position in any document (a stage that failed before it read a
/// node, a backend that could not start a worker thread). A diagnostic needs a
/// span, and this span points at no text; the reader of the diagnostic sees the
/// origin's label instead, so each caller passes its own origin
/// ([`FlmOrigin::synthetic`] labeled with the part of the library reporting, or
/// [`FlmOrigin::none`] when there is nothing to show).
pub(crate) fn nowhere(origin: FlmOrigin) -> FlmSpan {
    let source = std::sync::Arc::new(Source::new("").with_origin(origin));
    SourceSpan::new(&source, 0..0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_label_is_the_canonical_path() {
        assert_eq!(FlmOrigin::new("book/ch1.flm").label().as_deref(), Some("book/ch1.flm"));
        assert_eq!(FlmOrigin::none().label(), None);
        assert_eq!(FlmOrigin::default(), FlmOrigin::none());
    }

    #[test]
    fn a_synthetic_origin_is_labeled_but_has_no_path() {
        let origin = FlmOrigin::synthetic("expansion of \\foo");
        assert_eq!(origin.label().as_deref(), Some("expansion of \\foo"));
        assert_eq!(origin.canonical_path(), None);
        assert_eq!(origin.synthetic_label(), Some("expansion of \\foo"));
        assert!(!origin.is_definition());
    }

    #[test]
    fn a_definition_origin_is_a_marked_synthetic_origin() {
        let origin = FlmOrigin::definition("template of \\foo (text mode)");
        assert_eq!(origin.label().as_deref(), Some("template of \\foo (text mode)"));
        assert_eq!(origin.canonical_path(), None);
        assert!(origin.is_definition());
        assert!(!FlmOrigin::new("a.flm").is_definition() && !FlmOrigin::none().is_definition());
    }
}
