//! The lowering stage of a backend, [`LoweringStage`], and the computations
//! that plan construction gives it.
//!
//! A backend's feature registers one stage of this kind for its backend, named
//! `flm.<backend>.lower`, and declares the table `flm.lowered.<backend>` that
//! the stage writes ([`lowered_table_name`](super::lowered_table_name)). It is
//! a `Compute` stage of `PerTree` scope: the runner calls it once per frozen
//! tree, and it writes that tree's lowered form into the backend's
//! lowered-trees table ([`super::LoweredTrees`]). The lowering itself is done
//! by [`lower_tree`]; this module turns it into a stage: its declaration, the
//! needs it declares when the plan is built, and the inputs it gives the
//! driver.
//!
//! # What the stage declares it needs
//!
//! [`Stage::needs`] is computed when the plan is built, from the lowering
//! rules the plan selected for this backend: the union of their `requires`,
//! `reads`, `reads_opt` and option reads, without repetition ([`rules_needs`],
//! which every stage that lowers for a backend uses). A rule that reads the
//! references table therefore causes the stage that writes that table to be
//! scheduled. A rule's option reads are checked when the plan is built: a
//! read of an option type the registry does not declare, or of one whose
//! value is fixed when the registry is built
//! ([`Bind::Registry`](crate::model::options::Bind::Registry)), makes plan
//! building fail. Nothing else is added: the stage reads the frozen
//! tree directly and declares no table read of its own. Because the union
//! depends on the rule selection alone, two plans with the same selection
//! declare the same needs.
//!
//! An **optional** read of a rule (one declared in `reads_opt`) becomes a
//! required read whenever the table is one this registry declares *and* some
//! stage writes, so that the stage that writes it is scheduled and the rule
//! finds the table with its values rather than absent. A rule reads a table
//! optionally to say that it can do without the feature that owns the table,
//! not that it would rather not have the values; making the read required
//! whenever that feature is registered gives one behavior per registry
//! instead of one per plan. Two kinds of optional read stay as they are:
//!
//! - a read of a table that no stage writes, which only the embedding program
//!   installs when it constructs a run (an
//!   [external table](crate::core::run::Externals)), stays optional, since
//!   requiring it would make every plan fail for which the embedding program
//!   does not fill it;
//! - a read of a table this registry does not declare (a rule's read of a
//!   peer table that did not resolve) is left out of the needs altogether,
//!   and the rule sees that table as absent.
//!
//! # Requirements recorded whenever a rule is selected
//!
//! A rule may state that its output needs certain definitions (style and
//! script definitions on HTML, package requests and definition pieces on
//! LaTeX) whenever the rule is selected, whether or not it ever runs at a
//! node
//! ([`LoweringDecl::selection_requires`](crate::core::lowering::LoweringDecl::selection_requires)).
//! [`selection_requires`] collects those identifiers over the selected rules;
//! the result is added to every lowered tree's requirements
//! ([`LoweredTree::requires`](super::LoweredTree::requires)), beside what the
//! rules record at their own nodes. They are identifiers of definitions, not
//! capabilities, so they are not part of the plan's needs.
//!
//! # How many rounds lowering may run
//!
//! Lowering rewrites a tree repeatedly until only elements the backend writes
//! natively are left, and [`rank`] is the number of rounds that must suffice:
//! the longest chain of rule applications leading from any element that can
//! occur to the backend's native set. The count is over elements, not over
//! positions in a tree, so it covers content at any position, including
//! content inside mathematics, which a node carrying
//! [`ContentInOpaque`](crate::stdcore::backends::pres::ContentInOpaque) holds.
//! An element the backend writes natively counts no round; an element with a
//! selected rule counts one round more than the deepest element that rule
//! produces; an accepted unrenderable element (an element the plan request
//! accepts that the backend cannot render; see
//! [`PlanRequest::unsupported`](crate::core::plan::PlanRequest#structfield.unsupported))
//! counts one round, since the childless stand-in it lowers to is written
//! natively. Plan construction guarantees that this graph has no cycles; the
//! computation still guards against one, counting it as a single round, so a
//! run never loops.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::names::{BackendName, CapName, ElementName, Name};
use crate::model::options::OptionRef;
use crate::core::plan::{PlanCx, SelectedRules};
use crate::core::registry::frozen::FlmRegistry;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::core::data::store::StageOut;
use crate::core::data::table::{TableHandle, TableRef};
use techy::error::Diagnostic;

use super::driver::{LoweringInputs, lower_tree};
use super::error::LoweringFailed;
use super::table::{LoweredTrees, lowered_table};

/// The lowering stage of one backend, `flm.<backend>.lower`: it lowers every
/// frozen tree of a document for that backend and writes the result into the
/// table `flm.lowered.<backend>` (see the module documentation).
///
/// One value is the stage of one backend. The feature that registers the
/// backend creates it with [`LoweringStage::new`], registers it as one of the
/// backend's stages, and declares the table it writes. What the stage does
/// when the plan is built and when the run executes is in its [`Stage`]
/// implementation.
#[derive(Debug, Clone)]
pub struct LoweringStage {
    backend: BackendName,
    table: TableHandle<LoweredTrees>,
}

impl LoweringStage {
    /// The lowering stage of `backend`, which writes the table
    /// `flm.lowered.<backend>`.
    ///
    /// The feature registering the backend must declare that table itself
    /// (with `FeatureBuilder::table_named`, under the name
    /// [`lowered_table_name`] returns); this constructor only refers to it.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::lowering::LoweringStage;
    ///
    /// let stage = LoweringStage::new("text");
    /// assert_eq!(stage.name(), "flm.text.lower");
    /// assert_eq!(stage.table().name().as_str(), "flm.lowered.text");
    /// ```
    ///
    /// [`lowered_table_name`]: super::lowered_table_name
    pub fn new(backend: impl Into<BackendName>) -> LoweringStage {
        let backend = backend.into();
        let table = lowered_table(&backend);
        LoweringStage { backend, table }
    }

    /// The backend this stage lowers for.
    pub fn backend(&self) -> &BackendName {
        &self.backend
    }

    /// The stage's name, `flm.<backend>.lower`.
    pub fn name(&self) -> String {
        format!("flm.{}.lower", self.backend)
    }

    /// The typed handle on the lowered-trees table this stage writes.
    pub fn table(&self) -> &TableHandle<LoweredTrees> {
        &self.table
    }
}

impl Stage for LoweringStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(self.name(), Effect::Compute, Scope::PerTree)
            .writes(self.table.clone())
            .docs(format!("lowers every frozen tree for the `{}` backend", self.backend))
    }

    /// Declares what the stage reads: the capabilities, tables and options of
    /// the rules selected for this backend, read from the plan context's rule
    /// selection, with an optional read of a table some stage writes passed on
    /// as a required read (see the module documentation).
    ///
    /// # Panics
    ///
    /// Panics if the plan context's rule selection was built against a
    /// registry other than the one the context holds, so that a selected rule's
    /// index is out of range for that registry's rule list. A plan and the
    /// registry it was built from always match, so this is reachable only by
    /// pairing values from two registries by hand.
    fn needs(&self, cx: &PlanCx<'_>) -> Needs {
        rules_needs(cx, &self.backend)
    }

    /// Lowers the frozen tree the runner calls the stage for, with
    /// [`lower_tree`], and writes the result into the backend's lowered-trees
    /// table. A tree that fails to lower gets no entry in the table and one
    /// Error diagnostic ([`super::error`]); it never causes a panic.
    ///
    /// # Panics
    ///
    /// Panics under the same precondition as [`needs`](Self::needs): a rule
    /// selection built against a different registry than the one the run
    /// holds.
    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let Some(tree_id) = cx.tree_id() else { return };
        let Some(tree) = cx.my_tree() else { return };
        let registry = cx.registry();
        let Some(setting) = BackendLowering::of(cx, &self.backend) else { return };
        let inputs = setting.inputs(&self.backend, registry);
        let outcome = lower_tree(cx, &inputs, tree_id, &tree);
        for diagnostic in outcome.diags {
            out.diags.push(diagnostic);
        }
        if let Some(lowered) = outcome.lowered {
            match out.table(&self.table) {
                Ok(table) => {
                    table.trees.insert(tree_id, lowered);
                }
                Err(error) => out.diags.push(Diagnostic::error(
                    LoweringFailed { backend: self.backend.to_string(), detail: error.to_string() },
                    tree.root().span().clone(),
                )),
            }
        }
    }
}

/// What the lowering rules selected for `backend` need, taken together: the
/// union of their `requires`, `reads`, `reads_opt` and option reads, in name
/// order and without repetition. An optional read of a table some stage
/// writes becomes a required read, and an optional read of a table this
/// registry does not declare is left out (see the module documentation).
///
/// Every stage that lowers for `backend` declares these needs when the plan
/// is built: the lowering stage ([`LoweringStage`]) and the pieces stage
/// ([`PiecesStage`](crate::stdcore::backends::pieces::PiecesStage)), which
/// lowers the backend's pieces with the same rules. Any other stage that
/// calls [`lower_value`](super::lower_value) must declare them too. The
/// result is empty when the plan context holds no rule selection for
/// `backend`.
///
/// # Panics
///
/// Panics if the plan context's rule selection was built against a registry
/// other than the one the context holds, so that a selected rule's index is out
/// of range for that registry's rule list. A plan and the registry it was built
/// from always match, so this is reachable only by pairing values from two
/// registries by hand.
pub fn rules_needs(cx: &PlanCx<'_>, backend: &BackendName) -> Needs {
    let mut needs = Needs::new();
    let Some(selected) = cx.lowering_selection(backend) else { return needs };
    let registry = cx.registry();
    let mut requires: BTreeSet<CapName> = BTreeSet::new();
    let mut reads: BTreeMap<CapName, TableRef> = BTreeMap::new();
    let mut reads_opt: BTreeMap<CapName, TableRef> = BTreeMap::new();
    let mut options: BTreeSet<OptionRef> = BTreeSet::new();
    for idx in selected.rules() {
        let decl = &registry.rule(idx).rule.decl;
        requires.extend(decl.requires.iter().cloned());
        for table in &decl.reads {
            reads.insert(table.name.clone(), table.clone());
        }
        for table in &decl.reads_opt {
            reads_opt.insert(table.name.clone(), table.clone());
        }
        options.extend(decl.options.iter().cloned());
    }
    needs.requires = requires.into_iter().collect();
    // Each optional read the rules made is one of three things (see the
    // module documentation): a read of a table no feature of this registry
    // declares — a rule's read of an unresolved peer table — which the
    // stage does not name at all, so the rule sees the table as absent;
    // a read of a table some stage writes, which becomes a required read,
    // so that the writer is scheduled; or a read of a table no stage
    // writes, which stays optional, since only the host can fill it.
    let mut optional: Vec<TableRef> = Vec::new();
    for (name, table) in reads_opt {
        if reads.contains_key(&name) {
            continue;
        }
        match registry.table_named(&name) {
            None => {}
            Some(entry) if registry.table_has_writer(entry.id) => {
                reads.insert(name, table);
            }
            Some(_) => optional.push(table),
        }
    }
    needs.reads_opt = optional;
    needs.reads = reads.into_values().collect();
    needs.options = options.into_iter().collect();
    needs
}

/// What the running plan decided for lowering on one backend, owned: the
/// elements the backend writes natively under the rule selection, the
/// accepted unrenderable elements, the requirements recorded whenever a rule
/// is selected, and the number of rounds allowed. A stage that lowers for a
/// backend builds this once per call and borrows [`LoweringInputs`] from it.
pub(crate) struct BackendLowering<'r> {
    selected: &'r SelectedRules,
    native: BTreeSet<ElementName>,
    holes: BTreeSet<ElementName>,
    selection_requires: BTreeSet<Name>,
    rank: u32,
}

impl<'r> BackendLowering<'r> {
    /// The plan's lowering setting for `backend`; `None` when the plan holds
    /// no rule selection for it or the registry has no such backend.
    ///
    /// # Panics
    ///
    /// Panics under the precondition of [`rules_needs`]: a rule selection
    /// built against a different registry than the one the run holds.
    pub(crate) fn of(cx: &StageCx<'r>, backend: &BackendName) -> Option<BackendLowering<'r>> {
        let plan = cx.plan();
        let registry = cx.registry();
        let selected = plan.selection(backend)?;
        let entry = registry.backend(backend)?;
        let native: BTreeSet<ElementName> = entry
            .native
            .iter()
            .filter(|element| !selected.overridden_native.contains(*element))
            .cloned()
            .collect();
        let holes: BTreeSet<ElementName> =
            plan.request().unsupported.get(backend).into_iter().flatten().cloned().collect();
        let selection_requires = selection_requires(registry, selected);
        let rank = rank(registry, selected, &native);
        Some(BackendLowering { selected, native, holes, selection_requires, rank })
    }

    /// The driver's inputs, borrowed from this setting.
    pub(crate) fn inputs<'a>(
        &'a self,
        backend: &'a BackendName,
        registry: &'a FlmRegistry,
    ) -> LoweringInputs<'a> {
        LoweringInputs {
            backend,
            registry,
            selected: self.selected,
            native: &self.native,
            holes: &self.holes,
            guard: registry.lang().guard(),
            rank: self.rank,
            selection_requires: &self.selection_requires,
        }
    }
}

/// The identifiers of the definitions that the rules of `selected` require
/// whenever they are selected
/// ([`LoweringDecl::selection_requires`](crate::core::lowering::LoweringDecl::selection_requires)),
/// in identifier order (see the module documentation). The driver adds them
/// to every lowered tree's requirements.
///
/// # Panics
///
/// Panics if `selected` was built against a different registry than
/// `registry`: its rule indices are then out of range for `registry`'s rule
/// list.
pub fn selection_requires(registry: &FlmRegistry, selected: &SelectedRules) -> BTreeSet<Name> {
    selected
        .rules()
        .flat_map(|idx| registry.rule(idx).rule.decl.selection_requires.iter().cloned())
        .collect()
}

/// How many rounds lowering may run for a backend under the rule selection
/// `selected`, given the elements `native` the backend writes natively: the
/// length of the longest chain of rule applications in the graph of what each
/// selected rule produces (see the module documentation). Always at least
/// one. [`lower_tree`] reports `flm.lowering.rounds-exceeded` when a tree
/// still holds elements the backend does not write natively after this many
/// rounds.
///
/// # Panics
///
/// Panics if `selected` was built against a different registry than
/// `registry`: its rule indices are then out of range for `registry`'s rule
/// list.
pub fn rank(
    registry: &FlmRegistry,
    selected: &SelectedRules,
    native: &BTreeSet<ElementName>,
) -> u32 {
    fn depth(
        element: &ElementName,
        registry: &FlmRegistry,
        selected: &SelectedRules,
        native: &BTreeSet<ElementName>,
        memo: &mut BTreeMap<ElementName, u32>,
        visiting: &mut BTreeSet<ElementName>,
    ) -> u32 {
        if native.contains(element) {
            return 0;
        }
        if let Some(known) = memo.get(element) {
            return *known;
        }
        if !visiting.insert(element.clone()) {
            return 1;
        }
        let rounds = match selected.rule_for(element) {
            Some(idx) => {
                let produces = &registry.rule(idx).rule.decl.produces;
                1 + produces
                    .iter()
                    .map(|produced| depth(produced, registry, selected, native, memo, visiting))
                    .max()
                    .unwrap_or(0)
            }
            None => 1,
        };
        visiting.remove(element);
        memo.insert(element.clone(), rounds);
        rounds
    }
    let mut memo = BTreeMap::new();
    let mut visiting = BTreeSet::new();
    let mut elements: BTreeSet<ElementName> = registry.occurrable_elements().clone();
    for idx in selected.rules() {
        elements.extend(registry.rule(idx).rule.decl.produces.iter().cloned());
    }
    elements.extend(selected.by_element.keys().cloned());
    elements
        .iter()
        .map(|element| depth(element, registry, selected, native, &mut memo, &mut visiting))
        .max()
        .unwrap_or(1)
        .max(1)
}
