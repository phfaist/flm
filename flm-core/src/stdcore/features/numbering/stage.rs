//! The numbering stage, `flm.numbering.number` ([`NUMBER`]), the only stage of
//! the library that advances a counter, and the diagnostic conditions it
//! reports.
//!
//! The stage runs once per document. It first applies the document's
//! numbering options ([`NumberingOptions`]) to the declared hierarchy of
//! counters, and reports every option it cannot apply. It then reads every
//! reference point of the document in document order from the table
//! `flm.refpoints`, which the [`refpoints`](crate::stdcore::features::refpoints)
//! feature writes, advances the levels of the counter that each point's number
//! source ([`NumberSource`](crate::model::aspects::NumberSource)) lists, and
//! writes a record of each point's counter value into the counter table
//! [`COUNTERS`](super::COUNTERS). The stage writes no text: a record holds a
//! value, and the number a reader sees is written from the record where it is
//! shown.
//!
//! Each condition below says when it is reported and what the stage does
//! next (its recovery; see [`lowering::error`](crate::stdcore::backends::lowering::error)
//! for the term). Every condition is reported as an Error, except
//! [`UnknownCounter`], which is a Warning.

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
use crate::stdcore::features::refpoints::REFPOINTS;

use super::decl::{chains_of, CounterDecl, ParentsError};
use super::feature::{nesting_outside, written_outside, Compiled, Shared};
use super::joiner::{compile_within, joiner_text, CompiledWithinJoiner, WithinCx};
use super::options::{NumberingOptions, WithinOption};
use super::formats::NumberTemplate;
use super::style::{Ancestor, CounterStyle, StyleError};
use super::table::{is_nested, CounterRecord, Counters, COUNTERS};
use super::value::CounterValue;

/// The name of the numbering stage, `flm.numbering.number`. The stage reads
/// the table of reference points (`flm.refpoints`) and the numbering options,
/// and writes the counter table (`flm.counters`).
pub const NUMBER: &str = "flm.numbering.number";

// --- diagnostics ------------------------------------------------------------------------------

/// Condition (a Warning): the document's numbering options refer to a counter
/// that no feature declares, either as the counter an entry is about or as a
/// counter within which another is numbered.
///
/// Recovery: an entry for an undeclared counter changes nothing; an entry
/// that gives an undeclared parent leaves the counter in the place in the
/// hierarchy it was declared with, and its initial value still applies.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.unknown-counter",
    message = "the numbering options refer to the counter `{counter}`, which no feature declares"
)]
pub struct UnknownCounter {
    /// The counter the options refer to.
    pub counter: String,
}

/// Condition: a reference point's number source gives a counter that no
/// feature declares, usually because the feature that would declare it is not
/// registered. The condition is reported once per run and per counter name,
/// at the first point that gives the counter.
///
/// Recovery: no point that gives the counter is numbered; each shows what it
/// would show unnumbered (its content, else its label as written).
///
/// A point that gives a counter asks to be numbered, so a registry that
/// cannot number it is in error. A feature whose points may go unnumbered
/// gives its configuration a switch under which those points give no counter
/// at all: the mathematics feature does so with
/// [`MathConfig::numbered_equations`](crate::stdcore::features::math::MathConfig::numbered_equations),
/// and the headings feature with its numbering depth
/// ([`HeadingsConfig::numbering`](crate::stdcore::features::headings::HeadingsConfig::numbering)).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.undeclared-counter",
    message = "this construct is numbered by the counter `{counter}`, which no feature declares"
)]
pub struct UndeclaredCounter {
    /// The counter the point gives.
    pub counter: String,
}

/// Condition: a reference point's number source gives a counter style that no
/// feature declares, while its counter is declared. The condition is reported
/// at every such point.
///
/// Recovery: the point is not numbered.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.undeclared-style",
    message = "the number of this construct (counter `{counter}`) is written in the counter \
               style `{style}`, which no feature declares"
)]
pub struct UndeclaredStyle {
    /// The counter the point counts in.
    pub counter: String,
    /// The counter style the point gives.
    pub style: String,
}

/// Condition: a reference point's number would carry a sub-number at a level
/// for which its counter style declares no format, for example when two kinds
/// of point count in one counter under counter styles with different numbers
/// of sub-levels. The condition is reported at every such point.
///
/// Recovery: the point is not numbered, and the counter keeps the value it had
/// before the point.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.undeclared-sub-level",
    message = "the counter style `{style}` must declare a format for sub-level {level} \
               (counting the first sub-level as 0), since the number of this construct (counter \
               `{counter}`) has a sub-number there"
)]
pub struct UndeclaredSubLevel {
    /// The counter the point counts in.
    pub counter: String,
    /// The counter style the point's number is shown in.
    pub style: String,
    /// The sub-level, counted from zero for the first sub-number.
    pub level: usize,
}

/// Condition: a reference point counts in a counter whose number needs an
/// ancestor that has no value at the point: a parent that nothing has
/// advanced yet, or several possible parents none of which has been advanced
/// yet, or, for a point of a nested scoped instance, the enclosing instance of
/// the point's own counter, in which that counter has not been advanced yet
/// (the condition then gives the point's own counter as the one with no
/// value). A counter has no value until it is first advanced, unless the
/// numbering options give it an [`initial`](super::CounterOptions::initial)
/// value. The condition is reported once per counter and run, at the first
/// such point.
///
/// Recovery: the point is not numbered, and its counter is not advanced.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.numbering.parent-uninitialized")]
pub struct ParentUninitialized {
    /// The counter the point counts in.
    pub counter: String,
    /// The ancestor that has no value, or the possible parents none of which
    /// has one.
    pub parents: Vec<String>,
}

impl std::fmt::Display for ParentUninitialized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let quoted: Vec<String> = self.parents.iter().map(|name| format!("`{name}`")).collect();
        match quoted.as_slice() {
            [one] => write!(
                f,
                "the number of this construct (counter `{}`) includes the number of the \
                 counter {one}, which has no value at this place (a counter has no value until \
                 it is first advanced, unless the numbering options give it an initial value)",
                self.counter
            ),
            _ => write!(
                f,
                "the number of this construct (counter `{}`) includes the number of one of the \
                 counters {}, and none of them has a value at this place (a counter has no value \
                 until it is first advanced, unless the numbering options give it an initial \
                 value)",
                self.counter,
                quoted.join(", ")
            ),
        }
    }
}

/// Condition: the document's numbering options number a counter within a
/// parent set that no feature registers.
///
/// Recovery: the counter keeps the place in the hierarchy it was declared
/// with.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.unknown-parent-set",
    message = "the numbering options number the counter `{counter}` within the parent set \
               `{set}`, which no feature registers"
)]
pub struct UnknownParentSet {
    /// The counter the options are about.
    pub counter: String,
    /// The parent set the options give.
    pub set: String,
}

/// Condition: a counter's within-joiner or nesting joiner writes a counter
/// that, under one of the chains of counters the counter may be numbered
/// within, is not in that chain. The registry build refuses such a joiner in
/// a declaration, so only the document's numbering options can cause this
/// condition: by giving the counter such a joiner, or by moving the counter's
/// ancestors so that its declared joiner writes a counter outside a chain.
///
/// Recovery: a counter whose options gave it that joiner keeps the place in
/// the hierarchy it was declared with; a counter whose declared joiner the
/// options made fail is not numbered in that run.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.not-an-ancestor",
    message = "the number of the counter `{counter}` shows the number of the counter \
               `{named}`, but under the numbering options `{named}` is not always one of the \
               counters `{counter}` is numbered within"
)]
pub struct NotAnAncestor {
    /// The counter whose joiner writes the other counter.
    pub counter: String,
    /// The counter written, which is not one of its ancestors under every
    /// chain.
    pub named: String,
}

/// Condition: the document's numbering options make counters numbered within
/// each other, directly or through other counters. Every counter of such a
/// cycle, and every counter whose hierarchy leads into one, is listed.
///
/// Recovery: no point that counts in a listed counter is numbered in that
/// run, and no listed counter is used as a parent.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.numbering.hierarchy-cycle")]
pub struct HierarchyCycle {
    /// The counters that are numbered within each other, in name order.
    pub counters: Vec<String>,
}

impl std::fmt::Display for HierarchyCycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the numbering options make counters numbered within each other, directly or \
             through other counters; the counters in such a cycle or numbered within one are {}",
            self.counters.iter().map(|name| format!("`{name}`")).collect::<Vec<_>>().join(", ")
        )
    }
}

/// Condition: a within-joiner that the document's numbering options give a
/// counter cannot be used: a joining text or a within template that is not
/// valid FLM, a within template that refers to a counter no feature declares
/// or carries an unknown key in brackets, a registered within-joiner that does
/// not compile, or parents that hold no counter (an empty list of parents, or
/// a parent set that holds no counter).
///
/// Recovery: the counter keeps the place in the hierarchy it was declared
/// with.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.bad-within-joiner",
    message = "the numbering options cannot number the counter `{counter}` within other \
               counters through `{joiner}`: {detail}"
)]
pub struct BadWithinJoiner {
    /// The counter the options are about.
    pub counter: String,
    /// The within-joiner as the options give it: a joining text, a within
    /// template, or the name of a registered within-joiner.
    pub joiner: String,
    /// What the compilation reported.
    pub detail: String,
}

/// Condition: the document's numbering options number a counter within
/// another through a within-joiner name that no feature registers.
///
/// Recovery: the counter keeps the place in the hierarchy it was declared
/// with.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.unknown-within-joiner",
    message = "the numbering options number the counter `{counter}` within {parent} through \
               the within-joiner `{joiner}`, which no feature registers"
)]
pub struct UnknownWithinJoiner {
    /// The counter the options are about.
    pub counter: String,
    /// The counters it would be numbered within, as a message writes them:
    /// `` `flm.headings.section` ``, `` any of `a`, `b` `` or
    /// `` the parent set `x` ``.
    pub parent: String,
    /// The within-joiner name the options give.
    pub joiner: String,
}

/// Condition: a reference point counts in scoped instances of its counter
/// that do not enclose it one within the next: an instance that does not
/// contain the point's node, the node that carries the point itself, two
/// instances given in the wrong order or twice, or a node of another tree.
/// Each instance a point gives must be a node that contains the point's node,
/// and must contain the instance after it, the last one being the point's
/// own ([`NumberSource::instances`](crate::model::aspects::NumberSource::instances)).
/// The instances are given by the element of the point, so this condition
/// reveals a mistake in the feature that owns the element. It is reported at
/// every such point.
///
/// Recovery: the point is not numbered and advances nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.instance-not-enclosing",
    message = "this construct (counter `{counter}`) is counted separately within a list of \
               parts of the document, each of which must contain the construct and every part \
               listed after it, but one of them does not"
)]
pub struct InstanceNotEnclosing {
    /// The counter the point counts in.
    pub counter: String,
}

/// Condition: a reference point carries a number template of its own
/// ([`NumberSource::number_format`](crate::model::aspects::NumberSource::number_format))
/// that does not compile: text that is not valid FLM, or a `#{…}` that is not
/// one of the number formats. The condition is reported once per run for
/// each template text and counter style, at the first point that carries it.
///
/// Recovery: every point that carries the template is numbered in its
/// counter style as declared.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.bad-number-format",
    message = "the number template of this construct (counter `{counter}`, counter style \
               `{style}`) cannot be used: {detail}"
)]
pub struct BadNumberFormat {
    /// The counter the point counts in.
    pub counter: String,
    /// The counter style whose number format the template was to replace.
    pub style: String,
    /// The template as the point carries it.
    pub format: String,
    /// What the compilation reported.
    pub detail: String,
}

/// Condition: the numbering stage [`NUMBER`] could not read or write a table it
/// declared, or could not read the numbering options. This reveals a fault in
/// the program, not in a document.
///
/// Recovery: the counter table is not written, so nothing is numbered.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.numbering.stage-failed",
    message = "the stage `flm.numbering.number` failed: {detail}"
)]
pub struct StageFailed {
    /// What failed, rendered.
    pub detail: String,
}

/// Returns an empty span that belongs to no place in the document, for a
/// condition about the run as a whole.
fn run_wide() -> FlmSpan {
    nowhere(FlmOrigin::none())
}

// --- the stage ---------------------------------------------------------------------------------

/// The numbering stage, `flm.numbering.number`. The [module
/// documentation](self) describes what it does.
pub(super) struct NumberStage {
    shared: Arc<Shared>,
}

impl NumberStage {
    /// Returns the stage, which reads what this registration of the feature
    /// compiled.
    pub(super) fn new(shared: Arc<Shared>) -> NumberStage {
        NumberStage { shared }
    }
}

impl Stage for NumberStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(NUMBER, Effect::Compute, Scope::Document).writes(COUNTERS).docs(
            "Advances the declared counters over the reference points of the document, in \
             document order, and records each point's counter value in `flm.counters`.",
        )
    }

    /// Declares a read of the table of reference points, which the
    /// [`refpoints`](crate::stdcore::features::refpoints) feature writes, and
    /// a read of the numbering options. The read of the table is required, so
    /// a plan that schedules this stage in a registry without that feature
    /// fails and gives the table's name. The stage reads nothing of the
    /// [`refs`](crate::stdcore::features::refs) feature, so a registry without
    /// that feature still numbers.
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

/// Writes the counter table, and reports a failure to reach it.
fn write(out: &mut StageOut<'_>, counters: Counters) {
    match out.table(COUNTERS) {
        Ok(table) => *table = counters,
        Err(error) => failed(out, error),
    }
}

/// Reports that the stage could not reach a table or an option it declared.
fn failed(out: &mut StageOut<'_>, detail: impl ToString) {
    out.diags
        .push(Diagnostic::error(StageFailed { detail: detail.to_string() }, run_wide()));
}

// --- the hierarchy of one run --------------------------------------------------------------

/// One counter as this run numbers it.
struct Entry {
    /// The counter as it was declared.
    decl: CounterDecl,
    /// The counters it may be numbered within, after the options: one, or
    /// several, of which the one advanced most recently applies. Empty for a
    /// counter numbered within nothing.
    parents: Vec<Name>,
    /// Whether the parent that applies is left out of what its count is kept
    /// under, after the options.
    no_restart: bool,
    /// The joiner writing its ancestors' numbers before its own, after the
    /// options.
    joiner: Option<Arc<dyn CompiledWithinJoiner>>,
    /// Whether the start of the run refused its place in the hierarchy (a
    /// cycle, or a joiner that writes a counter outside one of its chains), so
    /// that no point counting in it is numbered in this run.
    refused: bool,
    /// Whether the options changed its place in the hierarchy; a refusal of
    /// the options' joiner then restores the declared place.
    from_options: bool,
}

/// What this run numbers and how: every declared counter with the place in
/// the hierarchy that the document's options leave it, and the compiled
/// counter styles.
///
/// Building the plan finds everything the options ask for that cannot be done
/// (a counter or a parent set no feature declares, a joiner that does not
/// compile, a cycle, a joiner that writes a counter it may not be numbered
/// within), so that the walk over the points only chooses parents, advances
/// counters and writes records.
struct Plan<'a> {
    compiled: &'a Compiled,
    /// The built language, which a point's own number template is compiled
    /// against.
    lang: &'a crate::lang::BuiltLang,
    counters: BTreeMap<Name, Entry>,
}

impl<'a> Plan<'a> {
    /// Returns the plan of one run, and reports everything the options ask
    /// for that cannot be done.
    fn build(
        compiled: &'a Compiled,
        options: &NumberingOptions,
        lang: &'a crate::lang::BuiltLang,
        out: &mut StageOut<'_>,
    ) -> Plan<'a> {
        // Everything the options ask for that cannot be done is reported here,
        // once, before the document is walked.
        // See decision rationale at [§flm-dd-dr:std-numbering-options-checked].
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
                                    detail: format!(
                                        "no counter is given to number it within (the options \
                                         give {})",
                                        within.parents.describe()
                                    ),
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

/// Returns the counters that `name` may be numbered within in this run, for
/// [`chains_of`]: the stage counts by the hierarchy the document's options
/// leave.
fn parents_of(counters: &BTreeMap<Name, Entry>, name: &Name) -> Vec<Name> {
    let Some(entry) = counters.get(name) else { return Vec::new() };
    entry.parents.iter().filter(|parent| counters.contains_key(*parent)).cloned().collect()
}

/// Reports `condition` as an Error about the run as a whole: what the
/// document's options ask for cannot be done.
fn report_error(out: &mut StageOut<'_>, condition: impl DiagnosticInfo + 'static) {
    out.diags.push(Diagnostic::error(condition, run_wide()));
}

/// Reports `condition` as a Warning about the run as a whole: what the
/// document's options ask for changes nothing, and everything else is
/// numbered as it would have been.
fn report_warning(out: &mut StageOut<'_>, condition: impl DiagnosticInfo + 'static) {
    out.diags.push(Diagnostic::warning(condition, run_wide()));
}

// --- counting ------------------------------------------------------------------------------

/// One ancestor of a point as the stage keeps it: the counter, its value at
/// the point, and the scoped instance in which that value was counted, if any.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Link {
    counter: Name,
    value: CounterValue,
    instance: Option<NodeAddr>,
}

/// The ancestors of a point, outermost first: the parent that applies last,
/// and after it, for a point of a nested scoped instance, the enclosing
/// instances of the point's own counter, outermost first.
type Chain = Vec<Link>;

/// The key under which one count is kept: the counter, the list of ancestors
/// it is advanced under, and the scoped instance it is counted in, or none for
/// the counter's single sequence through the document.
type Key = (Name, Chain, Option<NodeAddr>);

/// The value of every count while the document is read.
///
/// A counter has no value until it is first advanced, or until the options
/// give it one ([`initial`](super::CounterOptions::initial)), and a point
/// whose number would need a counter with no value is not numbered.
// See decision rationale at [§flm-dd-dr:std-numbering-uninitialized].
#[derive(Default)]
struct State {
    /// Per counter, per list of ancestors (each by name, value and instance)
    /// and per scoped instance, the current value. A key used for the first
    /// time starts at zero, and a key used again resumes where it stopped:
    /// restarting and resuming follow from this single rule, and two sibling
    /// instances, being two keys, each start again.
    values: BTreeMap<Key, CounterValue>,
    /// Per counter that has a value in its single sequence through the
    /// document, when it was last advanced there, counted in advances from the
    /// start of the run: this tells which of several parents was advanced most
    /// recently. An advance in a scoped instance is local to that instance's
    /// node and is not counted here.
    // See decision rationale at [§flm-dd-dr:std-numbering-instances].
    advanced: BTreeMap<Name, u64>,
    /// How many advances the run has made so far.
    advances: u64,
    /// Per counter that the options give an initial value, that value: the
    /// counter's value, under every list of ancestors and in every scoped
    /// instance, before its first advance there.
    initial: BTreeMap<Name, CounterValue>,
    /// Per counter and scoped instance, the counter style of the point that last
    /// advanced it: what an ancestor's number is written in.
    styles: BTreeMap<(Name, Option<NodeAddr>), Arc<CounterStyle>>,
    /// The counters already reported as undeclared, so that the condition is
    /// reported once per run instead of at every point.
    reported: BTreeSet<Name>,
    /// The counters already reported as advanced under a parent with no
    /// value, so that the condition is reported once per run and counter.
    uninitialized: BTreeSet<Name>,
    /// The last node and chain of instances found to enclose one another, so
    /// that the several points of one node, which share their instances, walk
    /// the node's ancestors once.
    enclosing: Option<(NodeAddr, Vec<NodeId>)>,
    /// Per declared counter style and number template that a point carries,
    /// the counter style derived from the first with its number format
    /// replaced by the second, or `None` for a template that does not compile.
    /// Each distinct text is compiled once per run for each counter style, and
    /// the points that carry the same text with the same counter style share
    /// one derived counter style.
    derived: BTreeMap<(Name, String), Option<Arc<CounterStyle>>>,
}

impl State {
    /// Returns the state at the start of a run: every counter has no value,
    /// except those to which the options give an initial value; each of those
    /// has it under every list of ancestors and counts as advanced before the
    /// document begins, the outermost counter first.
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

    /// Returns the current value of the count of `key`: the value its last
    /// advance left, else the counter's initial value, else none.
    fn value_of(&self, key: &Key) -> Option<CounterValue> {
        self.values.get(key).or_else(|| self.initial.get(&key.0)).cloned()
    }

    /// Records that `counter` was advanced now.
    fn mark_advanced(&mut self, counter: &Name) {
        self.advances += 1;
        self.advanced.insert(counter.clone(), self.advances);
    }

    /// Advances what `source` lists for the point `point` carried by `node`,
    /// and returns the record of the point, or `None` when the point takes no
    /// number.
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
            // One report per run and per counter, not one per point.
            // See decision rationale at
            // [§flm-dd-dr:std-numbering-undeclared-counter].
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
        // format; the record holds the derived style, which is where a record
        // has always held the style it is written in.
        // See decision rationale at [§flm-dd-dr:std-numbering-point-template].
        let style = match &source.number_format {
            None => Arc::clone(declared),
            Some(text) => self.derived(plan, source, declared, text, span, out),
        };

        // The levels the point advances are checked before anything is
        // advanced: a level the style declares no format for could never be
        // shown, and creating it would make the value as deep as the number
        // asked for.
        // See decision rationale at [§flm-dd-dr:std-numbering-unwritable].
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
        // enclosing instance stands at here.
        // See decision rationale at [§flm-dd-dr:std-numbering-instances].
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
    /// the template `text` that a point of `source` carries, compiled the first
    /// time the run reaches that text for that counter style. Where the
    /// template does not compile, it returns `declared` itself and reports the
    /// Error once for that text and counter style. The warnings the template
    /// raised are reported when it is compiled.
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
                            detail: error.to_string(),
                        },
                        span.clone(),
                    ));
                    None
                }
            }
        });
        Arc::clone(derived.as_ref().unwrap_or(declared))
    }

    /// Returns whether `instances`, outermost first, enclose one another and
    /// the node `node` at `addr`: walking up from the node's parent, the walk
    /// reaches the innermost instance first, then each one before it, in
    /// order. The node itself is never one of its own instances. An empty list
    /// encloses every node.
    ///
    /// The walk takes one step per ancestor of the node; the last list found
    /// to enclose a node is kept, so that the points of one node are checked
    /// once.
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

    /// Reports, once per run and counter, that a point counting in `counter`
    /// needs an ancestor with no value: `missing` is that ancestor, or the
    /// parents none of which has one.
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

    /// Returns the ancestors of a point counting in `counter` in the
    /// counter's single sequence through the document, by name and value,
    /// outermost first.
    ///
    /// The parent that applies is, among the counter's parents, the one
    /// advanced most recently; its own ancestors are found the same way, and
    /// its value is its value under them.
    ///
    /// # Errors
    ///
    /// The parents of a counter none of which has a value, or the one parent
    /// that has none under its own ancestors.
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

    /// Returns the ancestors of `chain` as a record carries them, each with
    /// the counter style it is written in, the instance it was counted in and
    /// its own joiner: the counter's nesting joiner for an enclosing instance
    /// that is itself nested, else the within-joiner of the counter.
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

/// Returns the list of ancestors under which `entry`'s count is kept when its
/// ancestors are `chain`: the chain itself, or, for a counter that does not
/// restart under its parent, the chain without the parent.
// See decision rationale at [§flm-dd-dr:std-numbering-no-restart].
fn key_of(entry: &Entry, chain: &Chain) -> Chain {
    match chain.split_last() {
        Some((_, above)) if entry.no_restart => above.to_vec(),
        _ => chain.clone(),
    }
}
