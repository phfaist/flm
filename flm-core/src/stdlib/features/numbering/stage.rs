//! This module defines the numbering stage [`NUMBER`] (`flm.numbering.number`),
//! along with the diagnostic conditions that the stage reports.
//!
//! The numbering stage is the only stage of the library that advances
//! counters. It runs once per document, in two steps. First, it applies the
//! numbering options of the document ([`NumberingOptions`]) to the hierarchy of
//! counters declared in the registry, and reports each option that it cannot
//! apply. Then it reads the reference points of the document in document order
//! from the table `flm.refpoints` ([`REFPOINTS`]). For each reference point, it
//! advances the counter as the number source of the reference point
//! ([`NumberSource`](crate::model::aspects::NumberSource)) specifies, and
//! writes a [`CounterRecord`] to the counter table [`COUNTERS`].
//!
//! A counter record contains the counter value of a reference point, and not
//! the text of its number. A lowering rule writes the number from the record
//! where the number is shown.
//!
//! Every condition of this module is reported as an error, except
//! [`UnknownCounter`], which is reported as a warning.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use techy::core::node::{NodeId, NodeRef};
use techy::error::{Diagnostic, DiagnosticInfo};

use crate::model::addr::NodeAddr;
use crate::model::aspects::PointId;
use crate::model::names::Name;
use crate::core::data::store::StageOut;
use crate::core::plan::PlanCx;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::{nowhere, Flm, FlmOrigin, FlmSpan};
use crate::stdlib::features::refpoints::REFPOINTS;

use super::decl::{chains_of, CounterDecl, Parents, ParentsError};
use super::feature::{nesting_outside, written_outside, Compiled, Shared};
use super::joiner::{compile_within, joiner_text, CompiledWithinJoiner, WithinCx};
use super::options::{NumberingOptions, WithinOption};
use super::formats::NumberTemplate;
use super::style::{Ancestor, CounterStyle, StyleError};
use super::table::{is_nested, CounterRecord, Counters, COUNTERS};
use super::value::CounterValue;

/// The name of the numbering stage, `flm.numbering.number`.
///
/// The numbering stage numbers the reference points of a document. It reads the
/// reference points from the table `flm.refpoints` and the numbering options of
/// the document ([`NumberingOptions`]), and writes the counter value of each
/// reference point to the counter table `flm.counters` ([`COUNTERS`]).
pub const NUMBER: &str = "flm.numbering.number";

// --- diagnostics ------------------------------------------------------------------------------

/// The warning that is reported when the numbering options refer to a counter
/// that is not declared in any feature (id `flm.numbering.unknown-counter`).
///
/// The numbering options refer to a counter in two places: as the key of an
/// entry of [`NumberingOptions::counters`], and as a parent counter in the
/// `within` value of an entry
/// ([`CounterOptions::within`](super::CounterOptions::within)).
///
/// Reported by the numbering stage [`NUMBER`] when it applies the numbering
/// options, before it reads any reference point. An entry for an undeclared
/// counter is ignored. If the `within` value of an entry refers to an
/// undeclared parent counter, the `within` value is ignored, and the counter
/// is numbered as its declaration specifies. The `initial` value of this entry
/// still applies.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.unknown-counter",
    message = "Unknown counter ‘{counter}’ in the numbering options"
)]
pub struct UnknownCounter {
    /// The name of the undeclared counter.
    pub counter: String,
}

/// The error that is reported when a reference point is numbered with a
/// counter that is not declared in any feature (id
/// `flm.numbering.undeclared-counter`).
///
/// The counter of a reference point is given in its number source
/// ([`NumberSource::counter`](crate::model::aspects::NumberSource::counter)).
/// This error usually means that the feature in which the counter is declared
/// is not registered.
///
/// Reported by the numbering stage [`NUMBER`] once per run and per counter, at
/// the first reference point that is numbered with the counter. None of the
/// reference points that are numbered with this counter receives a number.
///
/// A reference point that has a number source requests a number, so a missing
/// counter is an error. If the constructs of a feature are numbered only in
/// some configurations, their reference points have no number source in the
/// other configurations. The math feature and the headings feature do so with
/// [`MathConfig::numbered_equations`](crate::stdlib::features::math::MathConfig::numbered_equations)
/// and
/// [`HeadingsConfig::numbering`](crate::stdlib::features::headings::HeadingsConfig::numbering).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.undeclared-counter",
    message = "Undeclared counter ‘{counter}’"
)]
pub struct UndeclaredCounter {
    /// The name of the undeclared counter.
    pub counter: String,
}

/// The error that is reported when a reference point is numbered in a counter
/// style that is not declared in any feature (id
/// `flm.numbering.undeclared-style`).
///
/// The counter style of a reference point is given in its number source
/// ([`NumberSource::style`](crate::model::aspects::NumberSource::style)). This
/// error is reported only if the counter of the reference point is declared;
/// otherwise, [`UndeclaredCounter`] is reported.
///
/// Reported by the numbering stage [`NUMBER`] at every such reference point.
/// The reference point receives no number, and its counter is not advanced.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.undeclared-style",
    message = "Undeclared counter style ‘{style}’ (counter ‘{counter}’)"
)]
pub struct UndeclaredStyle {
    /// The counter of the reference point.
    pub counter: String,
    /// The name of the undeclared counter style.
    pub style: String,
}

/// The error that is reported when the number of a reference point has a
/// sub-number that its counter style cannot write (id
/// `flm.numbering.undeclared-sub-level`).
///
/// A counter value is a number with optional sub-numbers, as in `3b`. A
/// counter style declares one sub-level for each sub-number that it can write
/// ([`CounterStyleDecl::sub_levels`](super::CounterStyleDecl::sub_levels)). The
/// number of a reference point has a sub-number at a sub-level in two cases:
/// the reference point advances this sub-level
/// ([`NumberSource::advance`](crate::model::aspects::NumberSource::advance)),
/// or an earlier reference point of the same counter, in another counter
/// style, left a sub-number at this sub-level in the counter value.
///
/// Reported by the numbering stage [`NUMBER`] at every such reference point.
/// The reference point receives no number, and the counter keeps the value
/// that it had before the reference point, so that the numbers of the later
/// reference points do not change.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.undeclared-sub-level",
    message = "Missing sub-level {level} in counter style ‘{style}’ (counter ‘{counter}’; \
               sub-levels are counted from 0)"
)]
pub struct UndeclaredSubLevel {
    /// The counter of the reference point.
    pub counter: String,
    /// The name of the counter style of the reference point.
    pub style: String,
    /// The sub-level that the counter style does not declare, counted from 0
    /// for the first sub-number.
    pub level: usize,
}

/// The error that is reported when the number of a reference point depends on
/// a counter that has no value yet (id `flm.numbering.parent-uninitialized`).
///
/// The number of a reference point depends on the values of the counters that
/// its counter is numbered within, as in `2.1` for the first subsection of the
/// second section. A counter has no value until it is first advanced, unless
/// the numbering options give it an initial value
/// ([`CounterOptions::initial`](super::CounterOptions::initial)). This error is
/// reported in three cases:
///
/// - A counter that the counter of the reference point is numbered within has
///   no value yet, as for a subsection before the first section.
/// - The counter of the reference point has several possible parent counters
///   ([`Parents`]), and none of them has a value yet.
/// - The reference point counts in a nested scoped instance
///   ([`NumberSource::instances`](crate::model::aspects::NumberSource::instances)),
///   and the enclosing instance of the same counter has no value yet, as for
///   an item of an inner list that comes before the first item of the outer
///   list. In this case, the field `parents` contains the counter of the
///   reference point.
///
/// Reported by the numbering stage [`NUMBER`] once per run and per counter, at
/// the first reference point concerned. The reference point receives no
/// number, and its counter is not advanced.
///
/// A document whose subsections start before its first section avoids this
/// error by giving the section counter an initial value, as in
/// `NumberingOptions::new().initial("flm.headings.section", 0)`. Its first
/// subsection is then numbered `0.1`.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.numbering.parent-uninitialized")]
pub struct ParentUninitialized {
    /// The counter of the reference point.
    pub counter: String,
    /// The counter that has no value, or the possible parent counters, none of
    /// which has a value.
    pub parents: Vec<String>,
}

impl std::fmt::Display for ParentUninitialized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let quoted: Vec<String> = self.parents.iter().map(|name| format!("‘{name}’")).collect();
        let counter = &self.counter;
        match quoted.as_slice() {
            // A counter is never its own parent, so the counter itself stands
            // for the enclosing instance of a nested scoped instance.
            [one] if self.parents[0] == *counter => write!(
                f,
                "Missing value of counter {one} in the enclosing construct for the number of \
                 this construct"
            ),
            [one] => write!(
                f,
                "Missing value of counter {one} for the number of this construct (counter \
                 ‘{counter}’)"
            ),
            [first @ .., last] => write!(
                f,
                "Missing value of counter {} or {last} for the number of this construct \
                 (counter ‘{counter}’)",
                first.join(", ")
            ),
            [] => write!(
                f,
                "Missing value for the number of this construct (counter ‘{counter}’)"
            ),
        }
    }
}

/// The error that is reported when the numbering options number a counter
/// within a parent set that is not declared in any feature (id
/// `flm.numbering.unknown-parent-set`).
///
/// A parent set is a named list of counters
/// ([`ParentSetDecl`](super::ParentSetDecl)). The `within` value of an entry
/// of the numbering options refers to a parent set as in
/// `Within::parent_set("flm.headings.levels")`.
///
/// Reported by the numbering stage [`NUMBER`] when it applies the numbering
/// options, before it reads any reference point. The `within` value is
/// ignored, and the counter is numbered as its declaration specifies. The
/// `initial` value of the entry still applies.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.unknown-parent-set",
    message = "Unknown parent set ‘{set}’ for counter ‘{counter}’ in the numbering options"
)]
pub struct UnknownParentSet {
    /// The counter of the entry.
    pub counter: String,
    /// The name of the undeclared parent set.
    pub set: String,
}

/// The error that is reported when, under the numbering options, the number of
/// a counter includes the number of a counter that is not always one of its
/// ancestors (id `flm.numbering.not-an-ancestor`).
///
/// The ancestors of a counter are the counters that it is numbered within,
/// directly or through other counters. A within-joiner writes the numbers of
/// the ancestors before the number of the counter ([`Within`](super::Within)),
/// and the nesting joiner does the same for a nested scoped instance
/// ([`CounterDecl::nesting`](super::CounterDecl::nesting)). If a counter has
/// several possible parents, its ancestors depend on the parent that applies
/// at a reference point. A within-joiner or a nesting joiner must write only
/// counters that are ancestors for every possible parent. For instance, a
/// within template that writes the parent that applies, as in `#{_parent}.`,
/// meets this requirement.
///
/// Reported by the numbering stage [`NUMBER`] when it applies the numbering
/// options, before it reads any reference point. The registry build fails for
/// a declaration that does not meet the requirement, so only the numbering
/// options can cause this error. The error is reported in two cases:
///
/// - The `within` value of an entry gives the counter such a within-joiner.
///   The `within` value is then ignored, and the counter is numbered as its
///   declaration specifies.
/// - The numbering options change the ancestors of a counter, so that the
///   declared within-joiner or the nesting joiner of the counter writes a
///   counter that is not always an ancestor. No reference point of this
///   counter receives a number during the run.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.not-an-ancestor",
    message = "Invalid reference to counter ‘{named}’ in the number of counter ‘{counter}’: \
               under the numbering options, ‘{counter}’ is not always numbered within ‘{named}’"
)]
pub struct NotAnAncestor {
    /// The counter whose within-joiner or nesting joiner writes the counter
    /// `named`.
    pub counter: String,
    /// The counter that is written, and that is not always an ancestor of the
    /// counter `counter`.
    pub named: String,
}

/// The error that is reported when the numbering options number counters
/// within each other (id `flm.numbering.hierarchy-cycle`).
///
/// Counters are numbered within each other directly, as when `a` is numbered
/// within `b` and `b` within `a`, or through other counters. Every possible
/// parent of a counter ([`Parents`]) is taken into account. The registry build
/// fails for declarations that number counters within each other, so only the
/// numbering options can cause this error.
///
/// Reported by the numbering stage [`NUMBER`] once per run, when it applies the
/// numbering options. The error lists the counters that are numbered within
/// each other, and the counters that are numbered within one of them. No
/// reference point of these counters receives a number during the run.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.numbering.hierarchy-cycle")]
pub struct HierarchyCycle {
    /// The counters concerned, in name order.
    pub counters: Vec<String>,
}

impl std::fmt::Display for HierarchyCycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Circular hierarchy of counters in the numbering options (counters concerned: {})",
            self.counters.iter().map(|name| format!("‘{name}’")).collect::<Vec<_>>().join(", ")
        )
    }
}

/// The error that is reported when the `within` value that the numbering
/// options give a counter cannot be used (id
/// `flm.numbering.bad-within-joiner`).
///
/// The `within` value of an entry of the numbering options
/// ([`CounterOptions::within`](super::CounterOptions::within)) gives the parent
/// counters and the within-joiner of the counter ([`Within`](super::Within)).
/// The value cannot be used in the following cases:
///
/// - Its joining text or its within template is not valid FLM.
/// - Its within template refers to a counter that is not declared, or
///   contains a key in brackets other than `bare` and the names of the number
///   formats. A valid within template refers to declared counters or to the
///   parent that applies, as in `#{flm.headings.section}.` or
///   `#[Roman]{_parent}.`.
/// - It selects a registered within-joiner by name, and this within-joiner
///   returns an error when it is compiled.
/// - It gives no parent counter: an empty list of parent counters, or a parent
///   set that contains no counter.
///
/// Reported by the numbering stage [`NUMBER`] when it applies the numbering
/// options, before it reads any reference point. The `within` value is
/// ignored, and the counter is numbered as its declaration specifies. The
/// `initial` value of the entry still applies.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.bad-within-joiner",
    message = "Invalid value of ‘within’ for counter ‘{counter}’ in the numbering options: \
               {detail}"
)]
pub struct BadWithinJoiner {
    /// The counter of the entry.
    pub counter: String,
    /// The within-joiner of the `within` value: the joining text, the text of
    /// the within template, or the name of a registered within-joiner. The
    /// text is empty for the form that writes no ancestor
    /// ([`WithinJoinerSpec::Bare`](super::WithinJoinerSpec::Bare)).
    pub joiner: String,
    /// The description of the problem.
    pub detail: String,
}

/// The error that is reported when the numbering options select a
/// within-joiner by a name that is not registered (id
/// `flm.numbering.unknown-within-joiner`).
///
/// The `within` value of an entry of the numbering options selects a
/// within-joiner by name as in
/// `Within::parent("flm.headings.section").joiner_named("example.compact")`.
/// A within-joiner is registered under its name in the service slot
/// [`WITHIN_JOINERS_SLOT`](super::WITHIN_JOINERS_SLOT).
///
/// Reported by the numbering stage [`NUMBER`] when it applies the numbering
/// options, before it reads any reference point. The `within` value is
/// ignored, and the counter is numbered as its declaration specifies. The
/// `initial` value of the entry still applies.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.unknown-within-joiner",
    message = "Unknown within-joiner ‘{joiner}’ for counter ‘{counter}’ in the numbering options"
)]
pub struct UnknownWithinJoiner {
    /// The counter of the entry.
    pub counter: String,
    /// The parent counters that the `within` value gives, as text:
    /// `` `flm.headings.section` ``, `` any of `a`, `b` `` or
    /// `` the parent set `x` ``.
    pub parent: String,
    /// The name of the within-joiner.
    pub joiner: String,
}

/// The error that is reported when the scoped instances of a reference point
/// do not enclose the reference point one within the next (id
/// `flm.numbering.instance-not-enclosing`).
///
/// A reference point can count in scoped instances of its counter, which its
/// number source lists outermost first
/// ([`NumberSource::instances`](crate::model::aspects::NumberSource::instances)).
/// Each instance is a node that encloses the node of the reference point, and
/// each instance encloses the next instance of the list. The node of the
/// reference point is never an instance itself. For instance, the number
/// source of an item of a list that is nested in another list lists two
/// scoped instances: the outer list, then the inner list.
///
/// A document cannot cause this error: it is caused by the feature that
/// defines the element of the reference point. Reported by the numbering
/// stage [`NUMBER`] at every such reference point. The reference point
/// receives no number, and no counter is advanced.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.instance-not-enclosing",
    message = "Invalid scoped instances of counter ‘{counter}’ for this construct: \
               each instance must enclose the next one, and the last one must enclose this \
               construct"
)]
pub struct InstanceNotEnclosing {
    /// The counter of the reference point.
    pub counter: String,
}

/// The error that is reported when the number template of a reference point
/// is not valid (id `flm.numbering.bad-number-format`).
///
/// A reference point can carry a number template of its own
/// ([`NumberSource::number_format`](crate::model::aspects::NumberSource::number_format)),
/// which replaces the number format of its counter style. The author of the
/// document usually writes this template, as in the reference template
/// `ref=(#{alph})` of a list. A valid number template is FLM text in which
/// each `#{…}` contains the name of a number format
/// ([`NumberFormat`](super::NumberFormat)), as in `(#{alph})` or `#{Roman}.`.
///
/// Reported by the numbering stage [`NUMBER`] once per run for each template
/// and counter style, at the first reference point that carries the template.
/// The reference points that carry the template are numbered in their counter
/// style as declared.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.bad-number-format",
    message = "Invalid number template ‘{format}’ (counter ‘{counter}’, counter style ‘{style}’): \
               {detail}"
)]
pub struct BadNumberFormat {
    /// The counter of the reference point.
    pub counter: String,
    /// The counter style whose number format the template replaces.
    pub style: String,
    /// The number template, as the reference point carries it.
    pub format: String,
    /// The description of the problem.
    pub detail: String,
}

/// The error that is reported when the numbering stage [`NUMBER`] cannot read
/// its input or write the counter table (id `flm.numbering.stage-failed`).
///
/// The input of the stage is the table of reference points and the numbering
/// options. A document cannot cause this error: it is caused by a fault of the
/// program. The counter table is not written, so no reference point of the
/// document receives a number.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.stage-failed",
    message = "Stage ‘flm.numbering.number’ failed: {detail}"
)]
pub struct StageFailed {
    /// The error that the stage received, as text.
    pub detail: String,
}

/// Returns an empty span that is not located in the document. The stage
/// reports the conditions about the numbering options, and its own failures,
/// with this span.
fn run_wide() -> FlmSpan {
    nowhere(FlmOrigin::none())
}

/// Returns the description of `error` for the field `detail` of
/// [`BadNumberFormat`]: the messages of the diagnostics of a template that is
/// not valid, or else the message of `error`. The message of
/// [`BadNumberFormat`] quotes the template itself.
fn describe(error: &StyleError) -> String {
    match error {
        StyleError::Template { diagnostics, .. } | StyleError::Parse { diagnostics, .. } => {
            diagnostics.iter().map(|diagnostic| diagnostic.message()).collect::<Vec<_>>().join("; ")
        }
        other => other.to_string(),
    }
}

// --- the stage ---------------------------------------------------------------------------------

/// This struct implements the numbering stage [`NUMBER`]. The [module
/// documentation](self) describes the stage.
pub(super) struct NumberStage {
    shared: Arc<Shared>,
}

impl NumberStage {
    /// Returns the stage. `shared` is the state of one registration of the
    /// numbering feature, in which the build hook of the feature stores the
    /// compiled counters, counter styles and within-joiners.
    pub(super) fn new(shared: Arc<Shared>) -> NumberStage {
        NumberStage { shared }
    }
}

impl Stage for NumberStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(NUMBER, Effect::Compute, Scope::Document).writes(COUNTERS).docs(
            "Numbers the reference points of the document: reads them in document order, \
             advances their counters, and records the counter value of each reference point \
             in the table `flm.counters`.",
        )
    }

    /// Declares that the stage reads the table of reference points
    /// [`REFPOINTS`] and the numbering options. The table is required: a plan
    /// that schedules this stage fails in a registry without the
    /// [`refpoints`](crate::stdlib::features::refpoints) feature. The stage
    /// reads nothing of the [`refs`](crate::stdlib::features::refs) feature,
    /// so a registry without references still numbers its constructs.
    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(REFPOINTS).option::<NumberingOptions>()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let points = match cx.get(REFPOINTS) {
            Ok(Some(points)) => points,
            Ok(None) => return,
            Err(error) => return failed(out, error),
        };
        let options = match cx.option::<NumberingOptions>() {
            Ok(options) => options.clone(),
            Err(error) => return failed(out, error),
        };
        // Before the language is built there is nothing to count with. A
        // registry that finished building has always run the hook, so this is
        // the empty answer of a feature nobody declared a counter into.
        let Some(compiled) = self.shared.compiled() else {
            return write(out, Counters::default());
        };

        let mut counters = Counters::default();
        counters.carry_styles(Arc::clone(&compiled.styles));
        let plan = Plan::build(compiled, &options, cx.registry().lang(), out);
        for (counter, entry) in &plan.counters {
            if entry.refused {
                continue;
            }
            if let Some(joiner) = &entry.joiner {
                counters.number_within(counter.clone(), Arc::clone(joiner));
            }
            if let Some(joiner) = compiled.nesting.get(counter) {
                counters.number_nested(counter.clone(), Arc::clone(joiner));
            }
        }

        let mut state = State::new(&plan, &options);
        for record in points.in_document_order(cx) {
            let id: PointId = record.id;
            let Some(source) = &record.point.number else {
                counters.insert(id, None);
                continue;
            };
            // The node is read back from its frozen tree, where the instances
            // the point names are checked to enclose it.
            let node = cx.frozen_tree(id.addr.tree).and_then(|tree| tree.get(id.addr.node));
            let Some(node) = node else {
                counters.insert(id, None);
                continue;
            };
            let record = state.advance(&plan, source, id, &node, out);
            counters.insert(id, record);
        }
        write(out, counters);
    }
}

/// Writes `counters` to the counter table [`COUNTERS`], and reports
/// [`StageFailed`] if the table cannot be written.
fn write(out: &mut StageOut<'_>, counters: Counters) {
    match out.table(COUNTERS) {
        Ok(table) => *table = counters,
        Err(error) => failed(out, error),
    }
}

/// Reports [`StageFailed`], with `detail` as the description of the failure.
fn failed(out: &mut StageOut<'_>, detail: impl ToString) {
    out.diags
        .push(Diagnostic::error(StageFailed { detail: detail.to_string() }, run_wide()));
}

// --- the hierarchy of one run --------------------------------------------------------------

/// The place of one counter in the hierarchy of counters during one run: the
/// declaration of the counter, with the changes that the numbering options
/// make.
struct Entry {
    /// The declaration of the counter.
    decl: CounterDecl,
    /// The possible parent counters. Among several, the parent that applies
    /// at a reference point is the one advanced most recently. The list is
    /// empty for a counter that is numbered within no counter.
    parents: Vec<Name>,
    /// Whether the count continues across the advances of the parent (the
    /// `no_restart` field of [`Within`](super::Within)).
    no_restart: bool,
    /// The within-joiner of the counter, or `None` for a counter that is
    /// numbered within no counter.
    joiner: Option<Arc<dyn CompiledWithinJoiner>>,
    /// Whether the counter is refused for the run: it is listed in the
    /// [`HierarchyCycle`] error, or one of its joiners writes a counter that is
    /// not always an ancestor ([`NotAnAncestor`]). No reference point of a
    /// refused counter receives a number.
    refused: bool,
    /// Whether the numbering options changed the parents of the counter. If
    /// the within-joiner of the options is then refused ([`NotAnAncestor`]),
    /// the stage restores the parents and the within-joiner of the
    /// declaration.
    from_options: bool,
}

/// The hierarchy of counters for one run: every declared counter with its
/// place in the hierarchy after the numbering options are applied, along with
/// the declarations that the feature compiled.
///
/// [`Plan::build`] reports each numbering option that cannot be applied. The
/// stage then numbers the reference points without checking the hierarchy
/// again.
struct Plan<'a> {
    compiled: &'a Compiled,
    /// The built language, which a point's own number template is compiled
    /// against.
    lang: &'a crate::lang::BuiltLang,
    counters: BTreeMap<Name, Entry>,
}

impl<'a> Plan<'a> {
    /// Returns the hierarchy of counters for one run: the declared hierarchy,
    /// changed by the `within` values of `options`.
    ///
    /// Reports each `within` value that cannot be applied, and leaves the
    /// counter of this value as declared. Then marks as refused the counters
    /// that are numbered within each other, and the counters whose declared
    /// within-joiner or nesting joiner writes a counter that is not always an
    /// ancestor under `options`.
    fn build(
        compiled: &'a Compiled,
        options: &NumberingOptions,
        lang: &'a crate::lang::BuiltLang,
        out: &mut StageOut<'_>,
    ) -> Plan<'a> {
        // Everything the options ask for that cannot be done is reported here,
        // once, before the document is walked: each such fact is then reported
        // once rather than at every point, and an option that is wrong is
        // reported even when the document has no point it concerns.
        let mut counters: BTreeMap<Name, Entry> = BTreeMap::new();
        for (name, decl) in &compiled.counters {
            let hierarchy = compiled.hierarchy.get(name).cloned().unwrap_or_default();
            counters.insert(
                name.clone(),
                Entry {
                    decl: decl.clone(),
                    parents: hierarchy.parents,
                    no_restart: hierarchy.no_restart,
                    joiner: compiled.within.get(name).cloned(),
                    refused: false,
                    from_options: false,
                },
            );
        }

        for (name, entry) in &options.counters {
            let Some(target) = counters.get_mut(name) else {
                report_warning(out, UnknownCounter { counter: name.to_string() });
                continue;
            };
            match &entry.within {
                None => {}
                Some(WithinOption::Nothing) => {
                    target.parents.clear();
                    target.no_restart = false;
                    target.joiner = None;
                    target.from_options = true;
                }
                Some(WithinOption::Counter(within)) => {
                    let parents = match within.parents.resolve(&compiled.parent_sets) {
                        Ok(parents) => parents,
                        Err(ParentsError::UnknownSet(set)) => {
                            report_error(
                                out,
                                UnknownParentSet { counter: name.to_string(), set: set.to_string() },
                            );
                            continue;
                        }
                        Err(ParentsError::Empty) => {
                            report_error(
                                out,
                                BadWithinJoiner {
                                    counter: name.to_string(),
                                    joiner: joiner_text(within),
                                    detail: match &within.parents {
                                        Parents::Set(set) => {
                                            format!("Empty parent set ‘{set}’")
                                        }
                                        _ => String::from("Empty list of parent counters"),
                                    },
                                },
                            );
                            continue;
                        }
                    };
                    // The parents the options name are checked here and not
                    // at the walk: a counter numbered within one that does not
                    // exist would otherwise stand on its own without anything
                    // being said about it, and a misspelled name would quietly
                    // change what restarts it.
                    if let Some(unknown) =
                        parents.iter().find(|parent| !compiled.counters.contains_key(*parent))
                    {
                        report_warning(out, UnknownCounter { counter: unknown.to_string() });
                        continue;
                    }
                    let what = format!("the counter `{name}`");
                    let cx = WithinCx::new(
                        lang,
                        &compiled.language,
                        &compiled.names,
                        &compiled.joiners,
                    );
                    match compile_within(within, &what, &cx) {
                        Ok(joiner) => {
                            target.parents = parents;
                            target.no_restart = within.no_restart;
                            target.joiner = Some(joiner);
                            target.from_options = true;
                        }
                        Err(StyleError::UnknownJoiner { name: joiner }) => report_error(
                            out,
                            UnknownWithinJoiner {
                                counter: name.to_string(),
                                parent: within.parents.describe(),
                                joiner: joiner.to_string(),
                            },
                        ),
                        Err(error) => report_error(
                            out,
                            BadWithinJoiner {
                                counter: name.to_string(),
                                joiner: joiner_text(within),
                                detail: error.to_string(),
                            },
                        ),
                    }
                }
            }
        }

        // A joiner the options give a counter must write, for every chain the
        // counter may have, only counters of that chain; one that does not is
        // refused and the counter keeps the hierarchy it was declared with.
        let names: Vec<Name> = counters.keys().cloned().collect();
        for name in &names {
            let entry = counters.get(name).expect("every counter has an entry");
            if !entry.from_options {
                continue;
            }
            let Some(joiner) = &entry.joiner else { continue };
            let outside =
                written_outside(joiner.as_ref(), name, |counter| parents_of(&counters, counter));
            if let Some(named) = outside {
                report_error(
                    out,
                    NotAnAncestor { counter: name.to_string(), named: named.to_string() },
                );
                let declared = compiled.hierarchy.get(name).cloned().unwrap_or_default();
                let entry = counters.get_mut(name).expect("present");
                entry.parents = declared.parents;
                entry.no_restart = declared.no_restart;
                entry.joiner = compiled.within.get(name).cloned();
                entry.from_options = false;
            }
        }

        // The options may put two counters inside each other, through any of
        // the parents a counter may have; every counter of such a cycle, and
        // every counter whose hierarchy leads into one, is refused here, before
        // anything is walked, so that the walk cannot meet a hierarchy it has
        // to think about.
        let mut cyclic: BTreeSet<Name> = BTreeSet::new();
        for name in &names {
            if let Err(cycle) = chains_of(name, &|counter| parents_of(&counters, counter)) {
                cyclic.extend(cycle);
                cyclic.insert(name.clone());
            }
        }
        if !cyclic.is_empty() {
            report_error(
                out,
                HierarchyCycle {
                    counters: cyclic.iter().map(ToString::to_string).collect(),
                },
            );
            for name in &cyclic {
                if let Some(entry) = counters.get_mut(name) {
                    entry.refused = true;
                }
            }
        }

        // A declared joiner the options made fail, by moving the counter's
        // ancestors, cannot be undone the same way: the counter is refused.
        // So is a counter whose nesting joiner the options made fail, since
        // the options cannot change that joiner.
        for name in &names {
            let entry = counters.get(name).expect("every counter has an entry");
            if entry.refused {
                continue;
            }
            let within = entry.joiner.as_ref().and_then(|joiner| {
                written_outside(joiner.as_ref(), name, |counter| parents_of(&counters, counter))
            });
            let nesting = compiled.nesting.get(name).and_then(|joiner| {
                nesting_outside(joiner.as_ref(), name, |counter| parents_of(&counters, counter))
            });
            if let Some(named) = within.or(nesting) {
                report_error(
                    out,
                    NotAnAncestor { counter: name.to_string(), named: named.to_string() },
                );
                counters.get_mut(name).expect("present").refused = true;
            }
        }

        Plan { compiled, lang, counters }
    }
}

/// Returns the possible parent counters of `name` in the hierarchy `counters`,
/// without the counters that `counters` does not contain. The stage passes
/// this function to [`chains_of`].
fn parents_of(counters: &BTreeMap<Name, Entry>, name: &Name) -> Vec<Name> {
    let Some(entry) = counters.get(name) else { return Vec::new() };
    entry.parents.iter().filter(|parent| counters.contains_key(*parent)).cloned().collect()
}

/// Reports `condition`, a condition about the numbering options, as an error
/// that is not located in the document.
fn report_error(out: &mut StageOut<'_>, condition: impl DiagnosticInfo + 'static) {
    out.diags.push(Diagnostic::error(condition, run_wide()));
}

/// Reports `condition`, a condition about the numbering options, as a warning
/// that is not located in the document.
fn report_warning(out: &mut StageOut<'_>, condition: impl DiagnosticInfo + 'static) {
    out.diags.push(Diagnostic::warning(condition, run_wide()));
}

// --- counting ------------------------------------------------------------------------------

/// An ancestor of a reference point: a counter, its value at the reference
/// point, and the scoped instance in which this value was counted, if any.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Link {
    counter: Name,
    value: CounterValue,
    instance: Option<NodeAddr>,
}

/// The ancestors of a reference point, outermost first. The chain ends with
/// the parent that applies. For a reference point in a nested scoped instance,
/// the chain continues with the enclosing instances of the counter of the
/// reference point, outermost first.
type Chain = Vec<Link>;

/// The key of one sequence of values of a counter: the counter, the ancestors
/// under which the counter is advanced, and the scoped instance in which it is
/// counted (`None` for the single sequence of the counter through the
/// document).
type Key = (Name, Chain, Option<NodeAddr>);

/// The values of the counters while the stage reads the reference points of a
/// document.
///
/// A counter has no value until it is first advanced, unless the numbering
/// options give it an initial value
/// ([`CounterOptions::initial`](super::CounterOptions::initial)). A reference
/// point whose number includes a counter without a value receives no number.
// A counter nothing has advanced does not stand at zero: a number such as
// `2.0.1` would show a part that does not exist. With no value until the first
// advance, one walk numbers the document, and a missing ancestor is reported at
// the point that needs it.
#[derive(Default)]
struct State {
    /// The current value of each sequence of values, by [`Key`]. A sequence
    /// that is used for the first time starts at the initial value of its
    /// counter, or at zero. A sequence that is used again continues from its
    /// last value. A counter therefore restarts when the value of an ancestor
    /// in its key changes ([`key_of`]), and each scoped instance has a
    /// sequence of its own.
    values: BTreeMap<Key, CounterValue>,
    /// For each counter, the value of [`advances`](State::advances) after the
    /// last advance of the counter in its single sequence through the
    /// document. Among several possible parents, the parent that applies is
    /// the one with the highest value. Advances in a scoped instance are not
    /// recorded here.
    // A scoped instance is a run of its own, living as long as its node's
    // content, so its advances are not advances of the counter's sequence
    // through the document.
    advanced: BTreeMap<Name, u64>,
    /// The number of advances recorded so far in [`advanced`](State::advanced).
    advances: u64,
    /// The initial values that the numbering options give, by counter. Each
    /// sequence of the counter has this value before its first advance.
    initial: BTreeMap<Name, CounterValue>,
    /// For each counter and scoped instance, the counter style of the reference
    /// point that last advanced the counter. Where the counter is an ancestor
    /// of another reference point, its value is written in this counter style.
    styles: BTreeMap<(Name, Option<NodeAddr>), Arc<CounterStyle>>,
    /// The undeclared counters for which [`UndeclaredCounter`] was reported.
    /// The error is reported once per run for each counter.
    reported: BTreeSet<Name>,
    /// The counters for which [`ParentUninitialized`] was reported. The error
    /// is reported once per run for each counter.
    uninitialized: BTreeSet<Name>,
    /// The node and the scoped instances of the last successful check of
    /// [`encloses`](State::encloses). The reference points of one node have
    /// the same instances, so the ancestors of the node are walked only once.
    enclosing: Option<(NodeAddr, Vec<NodeId>)>,
    /// The counter styles derived from the number templates of reference
    /// points, by counter style name and template. A derived counter style is
    /// the declared counter style with its number format replaced by the
    /// template. The value is `None` for a template that does not compile.
    /// Each template is compiled once per run for each counter style.
    derived: BTreeMap<(Name, String), Option<Arc<CounterStyle>>>,
}

impl State {
    /// Returns the state at the start of a run. No counter has a value, except
    /// the counters that `options` give an initial value. These counters count
    /// as advanced before the document begins, the outermost counter first, so
    /// that the innermost counter counts as advanced most recently.
    fn new(plan: &Plan<'_>, options: &NumberingOptions) -> State {
        let mut state = State::default();
        let mut initialized: Vec<(usize, Name)> = Vec::new();
        for (name, entry) in &options.counters {
            let Some(initial) = entry.initial else { continue };
            // An entry naming a counter nothing declares was reported when the
            // plan was built.
            if plan.counters.get(name).is_none_or(|counter| counter.refused) {
                continue;
            }
            state.initial.insert(name.clone(), CounterValue::new(i64::from(initial)));
            let depth = chains_of(name, &|counter| parents_of(&plan.counters, counter))
                .map(|chains| chains.iter().map(Vec::len).max().unwrap_or(0))
                .unwrap_or(0);
            initialized.push((depth, name.clone()));
        }
        // The initial values count as advanced before the document, outermost
        // counter first, so that the innermost is the most recent, as a
        // document advancing them would leave them.
        initialized.sort();
        for (_, name) in &initialized {
            state.mark_advanced(name);
        }
        state
    }

    /// Returns the current value of the sequence `key`: its value after its
    /// last advance, or else the initial value of its counter, or else `None`.
    fn value_of(&self, key: &Key) -> Option<CounterValue> {
        self.values.get(key).or_else(|| self.initial.get(&key.0)).cloned()
    }

    /// Records an advance of `counter` in its single sequence through the
    /// document.
    fn mark_advanced(&mut self, counter: &Name) {
        self.advances += 1;
        self.advanced.insert(counter.clone(), self.advances);
    }

    /// Advances the counter of the reference point `point`, carried by `node`,
    /// as its number source `source` specifies. Returns the counter record of
    /// the reference point, or `None` if the reference point receives no
    /// number. Reports the conditions found at the reference point to `out`.
    fn advance(
        &mut self,
        plan: &Plan<'_>,
        source: &crate::model::aspects::NumberSource,
        point: PointId,
        node: &NodeRef<'_, Flm>,
        out: &mut StageOut<'_>,
    ) -> Option<CounterRecord> {
        let span = node.span();
        let Some(entry) = plan.counters.get(&source.counter) else {
            // One report per run and per counter, not one per point: the fact
            // is the same at every point that names the counter.
            if self.reported.insert(source.counter.clone()) {
                out.diags.push(Diagnostic::error(
                    UndeclaredCounter { counter: source.counter.to_string() },
                    span.clone(),
                ));
            }
            return None;
        };
        if entry.refused {
            return None;
        }
        let Some(declared) = plan.compiled.styles.get(&source.style) else {
            out.diags.push(Diagnostic::error(
                UndeclaredStyle {
                    counter: source.counter.to_string(),
                    style: source.style.to_string(),
                },
                span.clone(),
            ));
            return None;
        };
        // A point's own number template replaces the declared style's number
        // format; the record holds the derived style in the field that holds
        // the style a number is written in, so everything that writes the
        // point's number writes it the same way, with no second way to write it.
        let style = match &source.number_format {
            None => Arc::clone(declared),
            Some(text) => self.derived(plan, source, declared, text, span, out),
        };

        // The levels the point advances are checked before anything is
        // advanced: a level the style declares no format for could never be
        // shown, and creating it would make the value as deep as the number
        // asked for. The counter stays where it stood, so that one mistake does
        // not shift every number after it.
        let depth = style.sub_level_count();
        for level in &source.advance {
            if *level as usize > depth {
                out.diags.push(Diagnostic::error(
                    UndeclaredSubLevel {
                        counter: source.counter.to_string(),
                        style: source.style.to_string(),
                        level: *level as usize - 1,
                    },
                    span.clone(),
                ));
                return None;
            }
        }

        // The instances must enclose the point and each other, or the point
        // is not numbered: a chain that is not one would key a count by nodes
        // the point does not stand in.
        if !self.encloses(point.addr, node, &source.instances) {
            out.diags.push(Diagnostic::error(
                InstanceNotEnclosing { counter: source.counter.to_string() },
                span.clone(),
            ));
            return None;
        }
        let instances: Vec<NodeAddr> =
            source.instances.iter().map(|id| NodeAddr::new(point.addr.tree, *id)).collect();

        // Every ancestor of the chain must have a value; the point is otherwise
        // not numbered and the counter is not advanced.
        let plain = match self.chain_of(plan, &source.counter) {
            Ok(chain) => chain,
            Err(missing) => {
                self.report_uninitialized(&source.counter, missing, span, out);
                return None;
            }
        };
        // The outermost instance is counted under what the counter is numbered
        // within; each instance inside it is counted within the value the
        // enclosing instance stands at here, as an ancestor, so that a nested
        // number is written the way every number within a parent is.
        let mut chain = plain.clone();
        let mut key_chain = key_of(entry, &plain);
        let own = instances.last().copied();
        for enclosing in instances.iter().rev().skip(1).rev() {
            let key = (source.counter.clone(), key_chain.clone(), Some(*enclosing));
            let Some(value) = self.value_of(&key) else {
                self.report_uninitialized(
                    &source.counter,
                    vec![source.counter.clone()],
                    span,
                    out,
                );
                return None;
            };
            let link =
                Link { counter: source.counter.clone(), value, instance: Some(*enclosing) };
            chain.push(link.clone());
            key_chain.push(link);
        }
        let slot: Key = (source.counter.clone(), key_chain, own);
        let mut value = self.value_of(&slot).unwrap_or_else(|| CounterValue::new(0));
        for level in &source.advance {
            value = value.advanced(*level as usize);
        }
        // Two kinds of point may count in one counter under styles of
        // different depths; a value that carries a sub-number this style
        // cannot write is refused here, and the counter is left where it
        // stood, so that the mistake moves no other number.
        for (level, sub_number) in value.sub_numbers().iter().enumerate() {
            if *sub_number != 0 && level >= depth {
                out.diags.push(Diagnostic::error(
                    UndeclaredSubLevel {
                        counter: source.counter.to_string(),
                        style: source.style.to_string(),
                        level,
                    },
                    span.clone(),
                ));
                return None;
            }
        }
        if !source.advance.is_empty() {
            self.values.insert(slot, value.clone());
            self.styles.insert((source.counter.clone(), own), Arc::clone(&style));
            if own.is_none() {
                self.mark_advanced(&source.counter);
            }
        }
        Some(CounterRecord {
            counter: source.counter.clone(),
            value,
            ancestors: self.ancestors(plan, chain),
            style,
            instance: own,
        })
    }

    /// Returns the counter style `declared` with its number format replaced by
    /// `text`, the number template of a reference point with the number source
    /// `source`. The template is compiled the first time the run meets it for
    /// this counter style, and the warnings of the compilation are reported
    /// then. If the template does not compile, returns `declared` and reports
    /// [`BadNumberFormat`] once for this template and counter style.
    fn derived(
        &mut self,
        plan: &Plan<'_>,
        source: &crate::model::aspects::NumberSource,
        declared: &Arc<CounterStyle>,
        text: &str,
        span: &FlmSpan,
        out: &mut StageOut<'_>,
    ) -> Arc<CounterStyle> {
        let key = (source.style.clone(), text.to_string());
        let derived = self.derived.entry(key).or_insert_with(|| {
            let template = NumberTemplate::formats(text);
            match declared.with_number_format(&template, plan.lang, &plan.compiled.language) {
                Ok(style) => {
                    for warning in &style.warnings()[declared.warnings().len()..] {
                        out.diags.push(warning.clone());
                    }
                    Some(Arc::new(style))
                }
                Err(error) => {
                    out.diags.push(Diagnostic::error(
                        BadNumberFormat {
                            counter: source.counter.to_string(),
                            style: source.style.to_string(),
                            format: text.to_string(),
                            detail: describe(&error),
                        },
                        span.clone(),
                    ));
                    None
                }
            }
        });
        Arc::clone(derived.as_ref().unwrap_or(declared))
    }

    /// Returns whether the scoped instances `instances`, outermost first,
    /// enclose the node `node` at `addr` one within the next. That is, each
    /// instance is an ancestor of `node`, and each instance is an ancestor of
    /// the next instance. `node` itself is never one of its instances. An
    /// empty list encloses every node.
    ///
    /// The result of the last successful check is kept, so that the reference
    /// points of one node are checked once.
    fn encloses(&mut self, addr: NodeAddr, node: &NodeRef<'_, Flm>, instances: &[NodeId]) -> bool {
        if instances.is_empty() {
            return true;
        }
        if self
            .enclosing
            .as_ref()
            .is_some_and(|(seen, chain)| *seen == addr && chain.as_slice() == instances)
        {
            return true;
        }
        let mut wanted = instances.iter().rev().peekable();
        for ancestor in std::iter::successors(node.parent(), |up| up.parent()) {
            if wanted.next_if(|id| **id == ancestor.id()).is_some() && wanted.peek().is_none() {
                break;
            }
        }
        let found = wanted.peek().is_none();
        if found {
            self.enclosing = Some((addr, instances.to_vec()));
        }
        found
    }

    /// Reports [`ParentUninitialized`] for a reference point that counts in
    /// `counter`, unless the error was already reported for `counter` in this
    /// run. `missing` is the counter without a value, or the possible parent
    /// counters, none of which has a value.
    fn report_uninitialized(
        &mut self,
        counter: &Name,
        missing: Vec<Name>,
        span: &FlmSpan,
        out: &mut StageOut<'_>,
    ) {
        if self.uninitialized.insert(counter.clone()) {
            out.diags.push(Diagnostic::error(
                ParentUninitialized {
                    counter: counter.to_string(),
                    parents: missing.iter().map(ToString::to_string).collect(),
                },
                span.clone(),
            ));
        }
    }

    /// Returns the ancestors of a reference point that counts in `counter`,
    /// outermost first, with their current values in their single sequences
    /// through the document.
    ///
    /// Among the possible parents of `counter`, the parent that applies is the
    /// one advanced most recently. The ancestors of this parent are found in
    /// the same way. A refused counter is never a parent.
    ///
    /// # Errors
    ///
    /// Returns the possible parents of a counter if none of them has been
    /// advanced, or the parent that applies if it has no value under the
    /// current values of its own ancestors.
    fn chain_of(&self, plan: &Plan<'_>, counter: &Name) -> Result<Chain, Vec<Name>> {
        let Some(entry) = plan.counters.get(counter) else { return Ok(Vec::new()) };
        if entry.parents.is_empty() {
            return Ok(Vec::new());
        }
        // A refused counter is never a parent: it is in a cycle or writes what
        // it may not, and the walk would otherwise go round.
        let parent = entry
            .parents
            .iter()
            .filter(|parent| plan.counters.get(*parent).is_some_and(|parent| !parent.refused))
            .filter_map(|parent| self.advanced.get(parent).map(|when| (*when, parent)))
            .max_by_key(|(when, _)| *when)
            .map(|(_, parent)| parent.clone())
            .ok_or_else(|| entry.parents.clone())?;
        let mut chain = self.chain_of(plan, &parent)?;
        let parent_entry = plan.counters.get(&parent).expect("a parent is a declared counter");
        let key = (parent.clone(), key_of(parent_entry, &chain), None);
        let Some(value) = self.value_of(&key) else {
            return Err(vec![parent]);
        };
        chain.push(Link { counter: parent, value, instance: None });
        Ok(chain)
    }

    /// Returns the ancestors in `chain` as a counter record stores them
    /// ([`Ancestor`]). Each ancestor has the counter style in which its value
    /// is written, the scoped instance in which it was counted, and the joiner
    /// that writes its own ancestors: the nesting joiner of the counter for an
    /// enclosing instance that is itself nested, or else the within-joiner of
    /// the counter.
    fn ancestors(&self, plan: &Plan<'_>, chain: Chain) -> Vec<Ancestor> {
        let mut ancestors: Vec<Ancestor> = Vec::with_capacity(chain.len());
        for Link { counter, value, instance } in chain {
            let entry = plan.counters.get(&counter);
            let style = self
                .styles
                .get(&(counter.clone(), instance))
                .cloned()
                .or_else(|| {
                    let decl = entry.map(|entry| &entry.decl)?;
                    plan.compiled.styles.get(&decl.default_style).cloned()
                })
                .expect("every counter's default style is declared");
            let joiner = if is_nested(&counter, &ancestors) {
                plan.compiled.nesting.get(&counter).cloned()
            } else {
                entry.and_then(|entry| entry.joiner.clone())
            };
            let mut ancestor = Ancestor::new(counter, value, style);
            if let Some(joiner) = joiner {
                ancestor = ancestor.within(joiner);
            }
            if let Some(instance) = instance {
                ancestor = ancestor.in_instance(instance);
            }
            ancestors.push(ancestor);
        }
        ancestors
    }
}

/// Returns the ancestors in the key of the sequence of the counter `entry`
/// when its ancestors are `chain`: all of `chain`, or, for a counter that does
/// not restart when its parent advances (`no_restart`), `chain` without the
/// parent.
// A count that continues across its parent's advances is stated in the
// hierarchy, the parent being left out of the key: no command of the document
// sets a counter's value, as LaTeX's `\setcounter` does.
fn key_of(entry: &Entry, chain: &Chain) -> Chain {
    match chain.split_last() {
        Some((_, above)) if entry.no_restart => above.to_vec(),
        _ => chain.clone(),
    }
}
