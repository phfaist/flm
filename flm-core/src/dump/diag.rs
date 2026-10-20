//! Assertions on diagnostics for tests: select a diagnostic by its
//! **identifier**, then narrow the selection by the source text its span
//! covers, its `line:col` position, its severity, its number of traceback
//! frames or its **data**, never by its message.
//!
//! A [diagnostic](crate::guide::concepts_overview#diagnostics) reports a
//! problem with a document: a severity (Error, Warning or Note), a position,
//! and a typed condition with its data. The condition's identifier (such as
//! `flm.lang.dollar-math-disabled`) and the fields of its data are a stable
//! interface that tests may rely on; the message is written for a person and
//! may be reworded at any time. This module therefore offers no way to match
//! on message text.
//!
//! A test selects and narrows step by step. For example, on a fragment parsed
//! from `a $b`, where `$` is not accepted as a mathematics delimiter by
//! default:
//!
//! ```text
//! fragment.diags()
//!     .expect("flm.lang.dollar-math-disabled")
//!     .at("$")
//!     .data(json!({ "suggestion": DollarMathDisabled::SUGGESTION }))
//!     .once();
//! ```
//!
//! (The example is not compiled here, because it needs a fragment, which
//! [`parse_fragment`](crate::lang::parse_fragment) produces from a source text
//! with a [built language](crate::lang::BuiltLang); the condition is
//! [`DollarMathDisabled`](crate::lang::DollarMathDisabled).)
//!
//! [`expect`] (or [`ExpectDiag::expect`], the same operation as a method on a
//! diagnostics collection, a slice or a vector) selects the diagnostics that
//! carry an identifier, and fails when there is none. Each further step
//! narrows the selection: by the source text the span covers
//! ([`DiagExpect::at`]), by position ([`DiagExpect::at_line_col`]), by severity
//! ([`DiagExpect::severity`]), by the number of traceback frames
//! ([`DiagExpect::frames`]), by data ([`DiagExpect::data`]). [`DiagExpect::count`]
//! and [`DiagExpect::once`] assert how many diagnostics remain, and
//! [`expect_none`] asserts that no diagnostic carries an identifier.
//!
//! Every assertion fails by panicking, which fails the test. A step that leaves
//! nothing prints the candidates it had, each with its identifier, severity,
//! position, covered text and data. [The list of panicking
//! items](crate::guide::panics) lists these functions.
//!
//! # How data is compared
//!
//! The diagnostic's data is compared, as a techy serialized value, with an
//! expected value of any type `serde` can serialize, such as a
//! `serde_json::json!` literal.
//!
//! **Maps are compared without regard to the order of their entries.** Equality
//! of serialized values depends on the order of map entries, and the two sides
//! order their entries differently: a typed condition serializes its fields in
//! the order they are declared in Rust, while a `json!` literal keeps the order
//! in which it was written. Without this rule, a test would have to write the
//! entries of its `json!` literal in the order in which the condition's fields
//! are declared in Rust. Both sides are therefore brought to a canonical form,
//! with the entries of every map sorted by key at every level, before they are
//! compared. Lists keep their order.
//!
//! Nothing else in the library compares values this way: a [JSON golden
//! file](crate::dump::golden) compares the order of map entries too.

use serde::Serialize;
use techy::error::Severity;
use techy::serialize::SerialValue;
use techy::source::LineIndexCache;

use crate::dump::json::{from_serde, to_json_pretty};
use crate::lang::origin::{FlmDiagnostic, FlmDiagnostics, FlmOrigin};

/// A selection of diagnostics that all carry one identifier, narrowed step by
/// step.
///
/// Create one with [`expect`] or [`ExpectDiag::expect`]. Each narrowing method
/// ([`at`](DiagExpect::at), [`at_line_col`](DiagExpect::at_line_col),
/// [`severity`](DiagExpect::severity), [`frames`](DiagExpect::frames),
/// [`data`](DiagExpect::data)) consumes the selection and returns the narrowed
/// one, and panics when nothing is left. [`count`](DiagExpect::count) asserts
/// the number left and returns the selection; [`once`](DiagExpect::once)
/// asserts that exactly one is left and returns that diagnostic. See the [module documentation](self) for an example.
pub struct DiagExpect<'a> {
    identifier: String,
    candidates: Vec<&'a FlmDiagnostic>,
    /// The narrowing steps applied so far, for failure messages.
    steps: Vec<String>,
}

/// Selects the diagnostics of `diagnostics` that carry `identifier`, in their
/// original order, as a [`DiagExpect`] to narrow further.
///
/// [`ExpectDiag::expect`] is the same operation as a method on the collections
/// a test usually holds.
///
/// # Panics
///
/// Panics when none of them carries `identifier`, listing the identifiers that
/// are present.
pub fn expect<'a>(
    diagnostics: impl IntoIterator<Item = &'a FlmDiagnostic>,
    identifier: &str,
) -> DiagExpect<'a> {
    let all: Vec<&'a FlmDiagnostic> = diagnostics.into_iter().collect();
    let candidates: Vec<&'a FlmDiagnostic> =
        all.iter().copied().filter(|diagnostic| diagnostic.identifier() == identifier).collect();
    if candidates.is_empty() {
        let present: Vec<&str> = all.iter().map(|diagnostic| diagnostic.identifier()).collect();
        panic!(
            "expected a diagnostic with the identifier `{identifier}`; the identifiers present \
             are {present:?}"
        );
    }
    DiagExpect { identifier: String::from(identifier), candidates, steps: Vec::new() }
}

/// [`expect`] and [`expect_none`] as methods on the diagnostics a test usually
/// holds: an [`FlmDiagnostics`] collection (such as a fragment's
/// [`diags`](crate::model::doc::Fragment::diags)), a slice, or a vector.
pub trait ExpectDiag {
    /// Selects the diagnostics carrying `identifier` ([`expect`]).
    ///
    /// # Panics
    ///
    /// Panics when none carries it, listing the identifiers present.
    fn expect(&self, identifier: &str) -> DiagExpect<'_>;

    /// Asserts that no diagnostic carries `identifier` ([`expect_none`]).
    ///
    /// # Panics
    ///
    /// Panics when one does, describing it.
    fn expect_none(&self, identifier: &str);
}

impl ExpectDiag for FlmDiagnostics {
    fn expect(&self, identifier: &str) -> DiagExpect<'_> {
        expect(self.iter(), identifier)
    }

    fn expect_none(&self, identifier: &str) {
        expect_none(self.iter(), identifier);
    }
}

impl ExpectDiag for [FlmDiagnostic] {
    fn expect(&self, identifier: &str) -> DiagExpect<'_> {
        expect(self.iter(), identifier)
    }

    fn expect_none(&self, identifier: &str) {
        expect_none(self.iter(), identifier);
    }
}

impl ExpectDiag for Vec<FlmDiagnostic> {
    fn expect(&self, identifier: &str) -> DiagExpect<'_> {
        expect(self.iter(), identifier)
    }

    fn expect_none(&self, identifier: &str) {
        expect_none(self.iter(), identifier);
    }
}

/// Asserts that no diagnostic of `diagnostics` carries `identifier`.
///
/// [`ExpectDiag::expect_none`] is the same operation as a method.
///
/// # Panics
///
/// Panics when one does, describing it.
pub fn expect_none<'a>(diagnostics: impl IntoIterator<Item = &'a FlmDiagnostic>, identifier: &str) {
    let found: Vec<&'a FlmDiagnostic> = diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.identifier() == identifier)
        .collect();
    if !found.is_empty() {
        panic!(
            "expected no diagnostic with the identifier `{identifier}`, found:\n{}",
            describe(&found)
        );
    }
}

/// One line per diagnostic: identifier, severity, position, covered text, data.
fn describe(diagnostics: &[&FlmDiagnostic]) -> String {
    let mut lines = LineIndexCache::<FlmOrigin>::new();
    diagnostics
        .iter()
        .map(|diagnostic| {
            let span = diagnostic.span();
            let position = match lines.line_col(span.source(), span.start()) {
                Some((line, col)) => format!("{line}:{col}"),
                None => format!("byte {}", span.start()),
            };
            let data = SerialValue::from(diagnostic.data().serializable_data());
            format!(
                "  {} ({}) at {position} covering {:?}, {} traceback frame(s), data {}",
                diagnostic.identifier(),
                diagnostic.severity(),
                span.content(),
                diagnostic.frames().len(),
                to_json_pretty(&data).unwrap_or_else(|_| format!("{data:?}"))
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl<'a> DiagExpect<'a> {
    /// Returns the identifier every diagnostic in this selection carries.
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// Returns the diagnostics still in the selection, in their original order.
    pub fn diagnostics(&self) -> &[&'a FlmDiagnostic] {
        &self.candidates
    }

    /// Narrows the selection to the diagnostics satisfying `keep`, failing with
    /// the candidates when none does.
    fn narrow(
        mut self,
        step: String,
        mut keep: impl FnMut(&FlmDiagnostic) -> bool,
    ) -> DiagExpect<'a> {
        let before = std::mem::take(&mut self.candidates);
        self.candidates = before.iter().copied().filter(|diagnostic| keep(diagnostic)).collect();
        if self.candidates.is_empty() {
            panic!(
                "no diagnostic with the identifier `{}` {}{}; the candidates were:\n{}",
                self.identifier,
                self.steps.iter().map(|step| format!("{step}, ")).collect::<String>(),
                step,
                describe(&before)
            );
        }
        self.steps.push(step);
        self
    }

    /// Keeps the diagnostics whose span covers exactly the source text `text`.
    ///
    /// # Panics
    ///
    /// Panics when none does, listing the candidates it had.
    pub fn at(self, text: &str) -> DiagExpect<'a> {
        let expected = String::from(text);
        self.narrow(format!("covering {text:?}"), move |diagnostic| {
            diagnostic.span().content() == expected
        })
    }

    /// Keeps the diagnostics whose span starts at `line:col`, in the source's
    /// own numbering (1-based unless the source sets other offsets).
    ///
    /// # Panics
    ///
    /// Panics when none does, listing the candidates it had.
    pub fn at_line_col(self, line: usize, col: usize) -> DiagExpect<'a> {
        let mut lines = LineIndexCache::<FlmOrigin>::new();
        self.narrow(format!("at {line}:{col}"), move |diagnostic| {
            let span = diagnostic.span();
            lines.line_col(span.source(), span.start()) == Some((line, col))
        })
    }

    /// Keeps the diagnostics of the given severity.
    ///
    /// # Panics
    ///
    /// Panics when none is of that severity, listing the candidates it had.
    pub fn severity(self, severity: Severity) -> DiagExpect<'a> {
        self.narrow(format!("of severity {severity}"), move |diagnostic| {
            diagnostic.severity() == severity
        })
    }

    /// Keeps the diagnostics carrying exactly `count` traceback frames.
    ///
    /// A diagnostic's traceback frames are the parse frames (the constructs
    /// being parsed) that were open when it was recorded, innermost first,
    /// followed by any outer frames added to it afterwards.
    ///
    /// Only the number of frames can be asserted: a frame's title is text
    /// written for a person and is not a stable interface, so no step matches
    /// on it.
    ///
    /// # Panics
    ///
    /// Panics when none carries that many, listing the candidates it had.
    pub fn frames(self, count: usize) -> DiagExpect<'a> {
        self.narrow(format!("with {count} traceback frame(s)"), move |diagnostic| {
            diagnostic.frames().len() == count
        })
    }

    /// Keeps the diagnostics whose data equals `expected`, a value of any type
    /// `serde` can serialize, such as a `serde_json::json!` literal.
    ///
    /// Maps are compared without regard to the order of their entries; see the
    /// [module documentation](self) for why.
    ///
    /// # Panics
    ///
    /// Panics when `expected` cannot be converted to a serialized value (the
    /// cases [`from_serde`] lists, such as a floating-point number); and when no diagnostic in
    /// the selection carries that data, listing the candidates it had.
    pub fn data<V: Serialize>(self, expected: V) -> DiagExpect<'a> {
        let expected = from_serde(&expected).unwrap_or_else(|error| {
            panic!("the expected data is not a serializable value: {error}")
        });
        let shown = to_json_pretty(&expected).unwrap_or_else(|_| format!("{expected:?}"));
        let expected = canonical(expected);
        self.narrow(format!("with data {shown}"), move |diagnostic| {
            canonical(SerialValue::from(diagnostic.data().serializable_data())) == expected
        })
    }

    /// Asserts that exactly `count` diagnostics remain in the selection, and
    /// returns the selection unchanged.
    ///
    /// # Panics
    ///
    /// Panics when a different number remains, listing them.
    pub fn count(self, count: usize) -> DiagExpect<'a> {
        if self.candidates.len() != count {
            panic!(
                "expected {count} diagnostic(s) with the identifier `{}` {}, found {}:\n{}",
                self.identifier,
                self.steps.join(", "),
                self.candidates.len(),
                describe(&self.candidates)
            );
        }
        self
    }

    /// Asserts that exactly one diagnostic remains in the selection, and
    /// returns it.
    ///
    /// # Panics
    ///
    /// Panics when a different number remains, listing them.
    pub fn once(self) -> &'a FlmDiagnostic {
        self.count(1).candidates[0]
    }
}

impl std::fmt::Debug for DiagExpect<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiagExpect")
            .field("identifier", &self.identifier)
            .field("candidates", &self.candidates.len())
            .field("steps", &self.steps)
            .finish()
    }
}

/// The canonical form of a value for [`DiagExpect::data`]: every map's entries
/// sorted by key, recursively through lists and maps; every other value as it
/// is. The sort is stable, so entries sharing one key keep their order.
fn canonical(value: SerialValue) -> SerialValue {
    match value {
        SerialValue::List(items) => SerialValue::List(items.into_iter().map(canonical).collect()),
        SerialValue::Map(entries) => {
            let mut entries: Vec<(String, SerialValue)> =
                entries.into_iter().map(|(key, value)| (key, canonical(value))).collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            SerialValue::Map(entries)
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use techy::error::{Diagnostic, DiagnosticInfo};
    use techy::source::{Source, SourceSpan};

    /// A condition whose fields are declared out of key order.
    #[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
    #[diagnostic(id = "test.two-fields", message = "{zeta} {alpha}")]
    struct TwoFields {
        zeta: String,
        alpha: String,
    }

    fn diagnostic() -> FlmDiagnostic {
        let source = std::sync::Arc::new(Source::new("x").with_origin(FlmOrigin::none()));
        let condition = TwoFields { zeta: "z".into(), alpha: "a".into() };
        Diagnostic::error(condition, SourceSpan::new(&source, 0..1))
    }

    #[test]
    fn data_matches_a_map_whose_entries_are_in_another_order() {
        let diagnostic = diagnostic();
        // `json!` keeps the order written (`alpha` before `zeta`); the
        // condition's data comes out in declaration order (`zeta` first).
        expect([&diagnostic], "test.two-fields")
            .data(serde_json::json!({ "alpha": "a", "zeta": "z" }))
            .once();
    }

    #[test]
    fn canonical_sorts_map_entries_recursively_and_keeps_list_order() {
        let value = SerialValue::List(vec![
            SerialValue::Map(vec![
                ("b".into(), SerialValue::Int(2)),
                ("a".into(), SerialValue::Map(vec![("y".into(), SerialValue::Null)])),
            ]),
            SerialValue::Int(1),
        ]);
        let expected = SerialValue::List(vec![
            SerialValue::Map(vec![
                ("a".into(), SerialValue::Map(vec![("y".into(), SerialValue::Null)])),
                ("b".into(), SerialValue::Int(2)),
            ]),
            SerialValue::Int(1),
        ]);
        assert_eq!(canonical(value), expected);
    }
}
