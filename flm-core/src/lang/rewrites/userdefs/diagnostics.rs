//! The conditions that user-definition expansion reports. Their identifiers have
//! the form `flm.userdefs.<condition>`; a
//! [condition](crate::guide::concepts_overview#diagnostics) is the typed body of
//! a diagnostic.
//!
//! The module declares four conditions. Each arises when the **registry is
//! built**, while a definition's templates are compiled, at a **use site**,
//! while the [expand pass](super::expand) runs, or in both cases:
//!
//! - [`RecursionLimit`], at a use site while the expand pass runs: expanding the
//!   use site would exceed the expansion depth limit.
//! - [`BadPlaceholder`], when the registry is built, for a placeholder that
//!   refers to something the definition does not have or that is misused (the
//!   definition is then refused); at a use site, with the problem
//!   [`Unresolvable`](PlaceholderProblem::Unresolvable), for a placeholder that
//!   has no value there.
//! - [`BadDefinition`], when the registry is built, for a problem with the
//!   definition as a whole (the definition is then refused); at a use site, with
//!   the problem [`NotCompiled`](DefinitionProblem::NotCompiled), for a
//!   definition whose template was not compiled, or with the problem
//!   [`NotInForceInMode`](DefinitionProblem::NotInForceInMode), for a use in a
//!   parsing mode where the definition's one template text did not parse.
//! - [`ExpansionFailed`], for a failure of the expand pass itself: an internal
//!   error while building the expanded tree, or a failing expansion hook. The
//!   pass then returns no tree, and this diagnostic says why.
//!
//! For each condition reported at a use site, the use site is replaced by a text
//! node holding its literal source text, and one Error diagnostic at the use
//! site says why. No problem in a document makes the pass panic.
//!
//! Diagnostic data never carries the displayed text, so that a test that checks
//! the data does not depend on the wording. A problem type
//! ([`PlaceholderProblem`], [`DefinitionProblem`]) appears in the data as a map
//! with a stable `kind` token and the problem's fields; the text of the message
//! comes from the problem's `Display` implementation alone.

use std::fmt;

use techy::error::{DiagnosticInfo, DiagnosticValue, ToDiagnosticValue};
use techy::latexlike::Mode;

/// Condition `flm.userdefs.recursion-limit`: expanding a use site would exceed
/// the expansion depth limit, the number of expansions that one chain may nest
/// inside one another ([`ExpandOptions::recursion_limit`](super::ExpandOptions::recursion_limit)).
/// This limit is separate from the [recursion guard](crate::lang::guard), which
/// limits how deeply a tree may nest.
///
/// Recovery: the use site is replaced by its literal source text.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.userdefs.recursion-limit",
    message = "definitions may be expanded inside one another at most {limit} levels deep, \
               and expanding ‘{name}’ here would exceed that limit; a definition that uses \
               itself, directly or through other definitions, expands without end"
)]
pub struct RecursionLimit {
    /// The definition whose use site could not be expanded, as an author
    /// writes it (`\foo`).
    pub name: String,
    /// The depth limit that applies.
    pub limit: usize,
}

/// What is wrong with a placeholder of a template: the detail of
/// [`BadPlaceholder`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceholderProblem {
    /// A numbered placeholder beyond the definition's arguments.
    OutOfRange {
        /// How many arguments a numbered placeholder may refer to: the number of
        /// declared arguments minus the definition's argument number offset.
        arity: usize,
    },
    /// A named placeholder that matches no argument name and no placeholder name
    /// that an expansion hook of the definition computes.
    UnknownName,
    /// A `#{body}` placeholder in the template of a definition that has no body: a
    /// macro or a special character sequence.
    NoBody,
    /// A substitution argument (`#[…]{…}`) on a placeholder that takes none: a
    /// numbered or named argument, the body, or a computed placeholder name declared
    /// without one.
    SubstitutionNotAccepted,
    /// A placeholder that has no value at expansion time: the placeholder bindings
    /// of the use site give it none (no expansion hook computes it), or it is a
    /// placeholder left in content that an expansion hook built.
    Unresolvable,
    /// A default argument value that refers to itself through other default values.
    DefaultCycle,
}

impl fmt::Display for PlaceholderProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlaceholderProblem::OutOfRange { arity } => {
                write!(
                    f,
                    "no argument has this number; the definition has {arity} numbered argument(s)"
                )
            }
            PlaceholderProblem::UnknownName => {
                f.write_str("no argument and no computed placeholder has this name")
            }
            PlaceholderProblem::NoBody => {
                f.write_str("only an environment's template can refer to its body")
            }
            PlaceholderProblem::SubstitutionNotAccepted => {
                f.write_str("this placeholder cannot take a bracketed ‘[…]’ part")
            }
            PlaceholderProblem::Unresolvable => {
                f.write_str("it has no value at this use")
            }
            PlaceholderProblem::DefaultCycle => {
                f.write_str("default argument values must not refer to each other in a cycle")
            }
        }
    }
}

impl PlaceholderProblem {
    /// The stable token for the problem in diagnostic data, which a test or a
    /// tool matches on; the wording belongs to the message alone.
    pub fn kind(&self) -> &'static str {
        match self {
            PlaceholderProblem::OutOfRange { .. } => "out-of-range",
            PlaceholderProblem::UnknownName => "unknown-name",
            PlaceholderProblem::NoBody => "no-body",
            PlaceholderProblem::SubstitutionNotAccepted => "substitution-not-accepted",
            PlaceholderProblem::Unresolvable => "unresolvable",
            PlaceholderProblem::DefaultCycle => "default-cycle",
        }
    }
}

/// Diagnostic data: a map holding the [`kind`](PlaceholderProblem::kind) token
/// and the fields (`arity` for an out-of-range placeholder), never the
/// displayed text, so that a test that checks the data does not depend on the
/// wording.
impl ToDiagnosticValue for PlaceholderProblem {
    fn to_diagnostic_value(&self) -> DiagnosticValue {
        let mut fields = vec![(String::from("kind"), self.kind().to_diagnostic_value())];
        if let PlaceholderProblem::OutOfRange { arity } = self {
            fields.push((String::from("arity"), arity.to_diagnostic_value()));
        }
        DiagnosticValue::Map(fields)
    }
}

/// Condition `flm.userdefs.bad-placeholder`: a placeholder of a template that is
/// invalid (found when the registry is built) or that has no value at a use site
/// (found at expansion); [`PlaceholderProblem`] says which.
///
/// Recovery: when the registry is built, where templates are compiled, the
/// definition is refused; at expansion, the use site is replaced by its literal
/// source text.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.userdefs.bad-placeholder",
    message = "the placeholder ‘#{reference}’ in the template of ‘{name}’ is invalid: {reason}"
)]
pub struct BadPlaceholder {
    /// The definition whose template holds the placeholder, as an author
    /// writes it (`\foo`).
    pub name: String,
    /// The placeholder as written between the `#` and the end of the
    /// placeholder (`1`, `body`, `title`).
    pub reference: String,
    /// What is wrong with the placeholder.
    pub reason: PlaceholderProblem,
}

/// What is wrong with a definition as a whole: the detail of [`BadDefinition`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionProblem {
    /// The definition declares no template at all (its `content` is `None`).
    NoTemplate,
    /// The definition was refused when the registry was built, so a use site has
    /// nothing to expand into; the reason was reported then, among the registry's
    /// build diagnostics. A use site that the expand pass reaches before the
    /// templates are compiled has this problem too.
    NotCompiled,
    /// The use site is in a parsing mode where the definition does not apply. The
    /// definition was declared for both modes with one template text, and that
    /// text parses only in the other mode, which is then the only mode where the
    /// definition applies (running text, or a formula).
    NotInForceInMode {
        /// The parsing mode of the use site.
        mode: Mode,
    },
    /// A default value is declared for an argument the definition does not have.
    UnknownDefault {
        /// The key the default was declared under.
        key: String,
    },
    /// The template language could not be built: [minting](crate::lang::mint) its
    /// declarations, which turns them into the techy packages a parse looks
    /// commands up in, failed. Those declarations are constants of the
    /// [`template`](super::template) module, so this is not expected to happen.
    Minting {
        /// The minting error, as text.
        detail: String,
    },
    /// The template parse returned no tree: the template's parsing state could not
    /// be prepared, the [recursion guard](crate::lang::guard) refused deeply nested
    /// content, or the parse was aborted.
    ParseFailed {
        /// The parse failure, as text.
        detail: String,
    },
    /// The template refers to a *tack-on argument*: the trailing argument that
    /// absorbs the field macros written after a call, such as the `\label{…}` of
    /// `\thm{Euclid}\label{thm:euclid}` (see
    /// [fields](crate::guide::concepts_overview#fields-data-a-construct-reads)).
    ///
    /// A template cannot use what a tack-on argument absorbs, on any backend: what
    /// was absorbed is a run of whole macros, and those macros have no meaning as
    /// the content of a placeholder (a `\label` has no meaning on its own in the
    /// content a template produces). A definition may declare a tack-on argument and
    /// leave it unused; referring to it is this problem. Reading the absorbed
    /// information through a placeholder that queries it is intended and not
    /// implemented.
    TackOnPlaceholder {
        /// The placeholder as written between the `#` and the end of the
        /// placeholder (`2`, `fields`).
        reference: String,
    },
}

impl fmt::Display for DefinitionProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DefinitionProblem::NoTemplate => f.write_str("it declares no template"),
            DefinitionProblem::NotCompiled => f.write_str(
                "its template could not be compiled, for a reason reported earlier",
            ),
            DefinitionProblem::NotInForceInMode { mode } => {
                write!(
                    f,
                    "it is used here in {}, and its template cannot be parsed there",
                    mode_phrase(*mode)
                )
            }
            DefinitionProblem::UnknownDefault { key } => {
                write!(f, "a default value is declared for ‘{key}’, which is not an argument")
            }
            DefinitionProblem::Minting { detail } => {
                write!(f, "the template language could not be prepared: {detail}")
            }
            DefinitionProblem::ParseFailed { detail } => {
                write!(f, "the template could not be parsed: {detail}")
            }
            DefinitionProblem::TackOnPlaceholder { reference } => write!(
                f,
                "a template cannot refer to ‘#{reference}’: that argument collects the \
                 commands written after a call, such as ‘\\label’"
            ),
        }
    }
}

impl DefinitionProblem {
    /// The stable token for the problem in diagnostic data.
    pub fn kind(&self) -> &'static str {
        match self {
            DefinitionProblem::NoTemplate => "no-template",
            DefinitionProblem::NotCompiled => "not-compiled",
            DefinitionProblem::NotInForceInMode { .. } => "not-in-force-in-mode",
            DefinitionProblem::UnknownDefault { .. } => "unknown-default",
            DefinitionProblem::Minting { .. } => "minting",
            DefinitionProblem::ParseFailed { .. } => "parse-failed",
            DefinitionProblem::TackOnPlaceholder { .. } => "tack-on-placeholder",
        }
    }
}

/// Diagnostic data: a map holding the [`kind`](DefinitionProblem::kind) token
/// and the fields: the `mode` of a use site where the definition does not
/// apply (`text` or `math`), the `key` of an unknown default value, the
/// `detail` of a minting or parse failure (the failure's own text, not this
/// module's wording), or the `reference` of a placeholder that refers to a
/// tack-on argument.
impl ToDiagnosticValue for DefinitionProblem {
    fn to_diagnostic_value(&self) -> DiagnosticValue {
        let mut fields = vec![(String::from("kind"), self.kind().to_diagnostic_value())];
        match self {
            DefinitionProblem::UnknownDefault { key } => {
                fields.push((String::from("key"), key.to_diagnostic_value()));
            }
            DefinitionProblem::Minting { detail } | DefinitionProblem::ParseFailed { detail } => {
                fields.push((String::from("detail"), detail.to_diagnostic_value()));
            }
            DefinitionProblem::TackOnPlaceholder { reference } => {
                fields.push((String::from("reference"), reference.to_diagnostic_value()));
            }
            DefinitionProblem::NotInForceInMode { mode } => {
                fields.push((String::from("mode"), mode_token(*mode).to_diagnostic_value()));
            }
            DefinitionProblem::NoTemplate | DefinitionProblem::NotCompiled => {}
        }
        DiagnosticValue::Map(fields)
    }
}

/// How a message refers to a parsing mode: `a formula` or `running text`.
fn mode_phrase(mode: Mode) -> &'static str {
    match mode {
        Mode::Math => "a formula",
        _ => "running text",
    }
}

/// The stable token for a parsing mode in diagnostic data: `text` or `math`, the
/// tokens with which a definition's modes are written
/// ([`DefinitionModes::token`](super::DefinitionModes::token)).
fn mode_token(mode: Mode) -> &'static str {
    match mode {
        Mode::Math => "math",
        _ => "text",
    }
}

/// Condition `flm.userdefs.bad-definition`: a definition cannot be compiled as
/// declared, found when the registry is built. With the problem
/// [`NotCompiled`](DefinitionProblem::NotCompiled), the condition is reported at
/// a use site of a definition that was refused then; with the problem
/// [`NotInForceInMode`](DefinitionProblem::NotInForceInMode), at a use site in a
/// parsing mode where the definition's template did not parse.
///
/// Recovery: when the registry is built, the definition is refused and the rest
/// of the registry is built; at a use site, the use site is replaced by its
/// literal source text.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.userdefs.bad-definition",
    message = "the definition of ‘{name}’ cannot be used: {problem}"
)]
pub struct BadDefinition {
    /// The definition, as an author writes it (`\foo`).
    pub name: String,
    /// What is wrong with the definition.
    pub problem: DefinitionProblem,
}

/// Condition `flm.userdefs.expansion-failed`: the expand pass failed for a reason
/// that no recovery covers. Either an internal error occurred while building the
/// expanded tree (an invariant of a tree transformation was broken, or a region
/// of a node could not be attached to its new nodes), or an expansion hook
/// failed.
///
/// Recovery: none; the pass returns no tree, and this diagnostic says why.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.userdefs.expansion-failed",
    message = "the user definitions could not be expanded: {detail}"
)]
pub struct ExpansionFailed {
    /// The failure, as text.
    pub detail: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_placeholder_problem_projects_as_a_token_with_its_payload() {
        let problem = PlaceholderProblem::OutOfRange { arity: 2 };
        assert_eq!(
            problem.to_diagnostic_value(),
            DiagnosticValue::Map(vec![
                (String::from("kind"), DiagnosticValue::Str(String::from("out-of-range"))),
                (String::from("arity"), DiagnosticValue::Int(2)),
            ])
        );
    }

    #[test]
    fn a_definition_problem_projects_as_a_token_with_its_payload() {
        let problem = DefinitionProblem::UnknownDefault { key: String::from("zz") };
        assert_eq!(
            problem.to_diagnostic_value(),
            DiagnosticValue::Map(vec![
                (String::from("kind"), DiagnosticValue::Str(String::from("unknown-default"))),
                (String::from("key"), DiagnosticValue::Str(String::from("zz"))),
            ])
        );
    }
}
