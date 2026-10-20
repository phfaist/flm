# AI guide: pipeline

Condensed reference for stages, capabilities, plan requests and workflows, plan
construction, the runner, external tables and value catch-up. Load it together with the
[AI guide](crate::guide::ai_guide), whose section [Vocabulary](crate::guide::ai_guide#vocabulary)
defines the terms used here. Full treatment: [Stages, capabilities, and
plans](crate::guide::pipeline).

## The terms of this page

- A **stage** is one unit of work of the pipeline: it declares the capabilities it
  provides and requires, the tables it reads and writes, its effect (compute, check, or
  input/output) and its scope (once per tree, or once per document), and the plan decides
  whether and when it runs ([`Stage`](crate::core::stage::Stage)).
- A **rewrite stage** runs before the document's trees are frozen, turns one tree into
  another, reads no computed data, and publishes the trees it produced in a table named
  after the capability it provides ([`RewriteStage`](crate::core::stage::RewriteStage)).
- A **capability** is a registered dotted name for a state of processing that a stage
  brings about, such as `flm.blocks.structured`: a stage declares the capabilities it
  provides and the capabilities it requires, and a caller demands the capabilities it
  wants ([`CapabilityDef`](crate::core::registry::CapabilityDef)).
- To **demand** a capability is to ask the plan for a stage that provides it; the
  demands of a plan request are the capabilities it asks for, and a **workflow** is a
  named set of demands together with the choices that go with them
  ([`PlanRequest`](crate::core::plan::PlanRequest),
  [`WorkflowDef`](crate::core::registry::WorkflowDef)).
- A **plan** is the list of stages that satisfies a plan request, in dependency order and
  grouped in batches; it is a pure value of the registry and the request, with nothing of
  any document in it ([`Plan`](crate::core::plan::Plan)).
- A **run** is one execution of one plan over one document; it owns the frozen trees,
  the store and the diagnostics ([`Run`](crate::core::run::Run)).
- A **table** is typed data that one stage writes beside the frozen trees and other
  stages read; its identity is its registered dotted name, never its Rust type, and that
  name is also a capability ([`Table`](crate::core::data::table::Table)). The **store**
  is the run's collection of computed tables
  ([`Computed`](crate::core::data::store::Computed)).

## What holds for every plan and run

- **No caller lists stages.** A stage declares what it provides and requires; a caller
  demands capabilities, usually by choosing a workflow; the library derives the stages
  and their order ([stages, capabilities, and
  plans](crate::guide::concepts_overview#stages-capabilities-and-plans)).
- **A plan holds nothing about any document.** Build it once, reuse it for every
  document, cache it under [`Plan::key`](crate::core::plan::Plan::key).
- **A stage changes nothing it reads.** It reads frozen trees and a checked view of the
  store, and writes new table pieces and diagnostics into its collector, the
  [`StageOut`](crate::core::data::store::StageOut) the runner hands it; only the runner
  installs a collector's table pieces into the store (a table piece is the part of one
  table that one stage invocation produced;
  [`PieceClass`](crate::core::data::table::PieceClass)).
- **The result does not depend on timing.** The batches are fixed when the plan is
  built; the sequential and the parallel runner execute exactly those batches and install
  results through the same function
  ([`fold_outs`](crate::core::data::store::fold_outs)).
- **A table's name is its capability**: declaring a write provides it, declaring a
  read requires it. For a table, a stage author lists the read or the write and never the
  table's capability.

## The two kinds of stage

| | [`RewriteStage`](crate::core::stage::RewriteStage) | [`Stage`](crate::core::stage::Stage) |
|---|---|---|
| runs | before the trees are frozen | after the trees are frozen |
| does | one tree in, one tree out | reads trees and tables, writes tables |
| reads computed data | never: [`rewrite`](crate::core::stage::RewriteStage::rewrite) has no store view in its signature | only the tables it declared |
| declaration | [`RewriteDecl`](crate::core::stage::RewriteDecl) | [`StageDecl`](crate::core::stage::StageDecl) |
| context | [`RewriteCx`](crate::core::run::RewriteCx): the registry, the tree's identity and the region layout of its top level ([`FragmentLayout`](crate::model::doc::FragmentLayout)), the option values it declared | [`StageCx`](crate::core::run::StageCx) |
| publishes | its output for every tree, as a [`TreesTable`](crate::core::stage::TreesTable) under the capability it provides | the tables it declared |

The standard package registers four rewrite stages, which a plan orders totally:
`flm.input.splice` (the content of included files inserted in place of each `\input`),
`flm.userdefs.expand-always` and `flm.userdefs.expand` (user definitions replaced by
their templates), and `flm.blocks.structure` (paragraphs and blocks built, text resolved).

A `Stage` declares two more things:

- its [`Effect`](crate::core::stage::Effect):
  [`Compute`](crate::core::stage::Effect::Compute) (computed data),
  [`Check`](crate::core::stage::Effect::Check) (diagnostics only; a `Check` stage that
  declares a written table is refused when the registry is built), or
  [`Io`](crate::core::stage::Effect::Io) (effects outside the store, such as writing a
  file). A service a `Compute` stage calls may reach the network without making the
  stage `Io`;
- its [`Scope`](crate::core::stage::Scope):
  [`PerTree`](crate::core::stage::Scope::PerTree) (once per tree of the run: each tree
  the walk over the frozen trees reaches)
  or [`Document`](crate::core::stage::Scope::Document) (once, reading the document
  through the walk).

A stage is registered once and runs many times over many documents; it holds only its
own configuration and changes nothing, because the per-tree calls of one stage may run in
parallel.

### Declaration, needs and stage manifest

| fixed at | item | holds |
|---|---|---|
| registration | [`StageDecl`](crate::core::stage::StageDecl) | name, effect, scope, the capabilities it `provides`, the tables it `writes` |
| plan build | [`Needs`](crate::core::stage::Needs), returned by [`Stage::needs`](crate::core::stage::Stage::needs)`(&`[`PlanCx`](crate::core::plan::PlanCx)`)` | [`requires`](crate::core::stage::Needs::requires), [`reads`](crate::core::stage::Needs::reads), [`reads_opt`](crate::core::stage::Needs::reads_opt) (read only if some stage provides it), the option types read ([`option`](crate::core::stage::Needs::option)), ordering-only edges [`after`](crate::core::stage::Needs::after) / [`before`](crate::core::stage::Needs::before) |
| plan | [`Manifest`](crate::core::stage::Manifest), the stage manifest | the declaration joined with the needs. **The runner and every validation read the plan's copy and never call the trait methods again.** |

Most stages return constant needs. A stage whose inputs depend on the plan request
computes them from [`PlanCx`](crate::core::plan::PlanCx): a backend's lowering stage
reads every table that the lowering rules selected for that backend read.

### What a stage reads and writes

Through [`StageCx`](crate::core::run::StageCx):

- the trees: [`my_tree`](crate::core::run::StageCx::my_tree) and
  [`tree_id`](crate::core::run::StageCx::tree_id) (per-tree stage),
  [`frozen_tree`](crate::core::run::StageCx::frozen_tree),
  [`walk`](crate::core::run::StageCx::walk) (document order across every tree),
  [`trees`](crate::core::run::StageCx::trees),
  [`position`](crate::core::run::StageCx::position) (to sort nodes gathered from
  several trees), [`placement_of`](crate::core::run::StageCx::placement_of),
  [`addr`](crate::core::run::StageCx::addr) (a node's address);
- the tables it declared: [`get`](crate::core::run::StageCx::get),
  [`get_singleton`](crate::core::run::StageCx::get_singleton). An undeclared read is an
  [`AccessError`](crate::core::data::store::AccessError) that identifies the stage and the
  table, never a silent `None`; an optional read returns `None` when the plan has no
  writer for that table;
- its option values: [`option`](crate::core::run::StageCx::option), resolved once for
  the whole run;
- services, by value type: [`service`](crate::core::run::StageCx::service) returns the
  value of type `S` that takes precedence (the service slots holding `S` taken in name
  order; in a slot, between features, the value of the feature placed last) or `None`,
  [`services`](crate::core::run::StageCx::services) every value of type `S`; a stage
  reports an absent service through a diagnostic of its own;
- value catch-up: [`resolve_value`](crate::core::run::StageCx::resolve_value)
  (see [Value catch-up](#value-catch-up)).

Into [`StageOut`](crate::core::data::store::StageOut): a table piece of each declared
table ([`table`](crate::core::data::store::StageOut::table)) and diagnostics
([`diags`](crate::core::data::store::StageOut#structfield.diags)). Both sides are
checked where the access happens.

## Capabilities, refinements, collections and ordering-only edges

Capability names, table names and collection names share one namespace, because a
table's name is the capability "this table is present and complete". A feature declares a
capability explicitly ([`CapabilityDef`](crate::core::registry::CapabilityDef)) only for a
state no table represents: a marker, a check's result, the parent of refinements.

| mechanism | item | rule |
|---|---|---|
| **refinement** | [`CapabilityDef::refines`](crate::core::registry::CapabilityDef::refines), [`CapabilityDef::default`](crate::core::registry::CapabilityDef::default) | providing the refinement satisfies a demand for the parent; at most one refinement is the parent's default, which provider selection chooses for a demand of the parent when no preference decides. `flm.blocks` is refined by `flm.blocks.structured` (the default) and `flm.blocks.raw`; `flm.userdefs` by `flm.userdefs.expanded` (the default) and `flm.userdefs.kept` |
| **collection** | [`CollectionDef`](crate::core::registry::CollectionDef), [`FeatureBuilder::collection_member`](crate::core::registry::FeatureBuilder::collection_member) | a name for a set of capabilities any feature may add to; demanding it demands every member. It can only be demanded: listing one among a stage's provided or required capabilities is refused. `flm.validated` ([`VALIDATED`](crate::stdcore::VALIDATED)) is the standard one. A member added to a collection that no feature of the registry declares is dropped without a diagnostic |
| **ordering-only edge** | [`Needs::after`](crate::core::stage::Needs::after) / [`Needs::before`](crate::core::stage::Needs::before), or [`OrderingDef`](crate::core::registry::OrderingDef) on another feature's stage | "run after (or before) the stage that provides this capability, if a stage does". It never causes anything to be scheduled; on a capability with refinements it follows the refinement the plan scheduled. The `input` rewrite declares such edges before the user-definition expansion and before block structure |

## Asking for a plan

A [`PlanRequest`](crate::core::plan::PlanRequest) has five fields that decide a plan,
and nothing else does:

| field | meaning |
|---|---|
| `demands` | the capabilities wanted ([`demand`](crate::core::plan::PlanRequest::demand)); demanding a collection demands each of its members |
| `prefer` | which stage to schedule where several could provide one capability, most preferred first |
| `lowering` | per backend, a [`LoweringSelection`](crate::core::registry::LoweringSelection): which lowering rule to use where several could lower one element |
| `unsupported` | per backend, the accepted unrenderable elements ([`unsupported`](crate::core::plan::PlanRequest::unsupported)) |
| `render_options` | the option values set on this render request ([`OptionValues`](crate::model::options::OptionValues)) |

A sixth field, `workflow`, names the workflow the request was made from (`None` for a
request built by hand). Plan building does not consult it and the plan key leaves it out;
the plan's errors and warnings name that workflow ("the plan request for workflow `html`
prefers the stage …"; "the plan request prefers …" without one).

An **accepted unrenderable element** is an element the plan request lists, per backend,
as one the backend need not render: the plan builds anyway, and each occurrence lowers to
a childless `flm.pres.unsupported` node with one Error diagnostic.

- A **workflow** ([`WorkflowDef`](crate::core::registry::WorkflowDef)) is stored in the
  registry. A caller chooses one by name and changes it through
  [`Overrides`](crate::core::plan::Overrides), whose five fields mirror the plan
  request's: extra demands, extra preferences, lowering selections that replace the
  workflow's per backend, extra accepted unrenderable elements, render option values.
  [`Plan::for_workflow`](crate::core::plan::Plan::for_workflow) builds the request and
  the plan; [`workflow_request`](crate::core::plan::workflow_request) builds only the
  request. A workflow that extends another starts from the other's fields; each field it
  declares replaces the inherited one
  ([`FlmRegistry::workflow_chain`](crate::core::registry::FlmRegistry::workflow_chain),
  [`FlmRegistry::combined_workflow`](crate::core::registry::FlmRegistry::combined_workflow)).
- An **output** ([`OutputDef`](crate::core::registry::OutputDef)) is a name a caller
  can ask for (`text`, `html`, `latex-rendered`, `latex-recomposed`); it ties together the capability the
  plan must reach, the table that holds the resulting artifacts, and the entry artifact's
  media type and file extension. Demanding an output is demanding its capability. An
  output made from other outputs' files lists them in `made_from` (a host prepares them
  with it; the plan schedules them through the providing stage's declared reads).
- [`PlanKey`](crate::core::plan::PlanKey): one number over the registry's identity and
  everything the plan request decides. Same key and same registry, same plan.

## How a plan is built

[`Plan::build`](crate::core::plan::Plan::build) works in five steps:

1. **Check the plan request**: every demanded capability, preferred stage, backend,
   preferred rule, accepted unrenderable element and render option value must be
   declared by the registry.
2. **Choose a lowering rule per element, for every registered backend**, by lowering rule
   selection: the rule the request prefers by name, else the backend's native emission,
   else a rule marked default on that backend, else a rule marked default everywhere,
   else the sole candidate ([`SelectedRules`](crate::core::plan::SelectedRules)). Which
   backends take part is not known yet, so a failure for one backend is kept and raised
   only if that backend takes part.
3. **Ask every stage for its needs** and resolve them against the registry.
4. **Choose a provider for each needed capability**, by provider selection (below).
5. **Order the stages and check the plan.**

### Provider selection

Provider selection chooses one stage for each capability the plan needs: the forced
choices are made first and followed to a fixed point, then one tie is broken by the
request's preferences or a default refinement, and the forced choices are followed again,
until nothing is left to decide ([`crate::core::plan::selection`]).

1. Start from the demanded capabilities. A *candidate* of a capability is a stage that
   provides it, directly or by providing a refinement of it, and that is not ruled out.
2. Follow the forced choices: a needed capability with exactly one candidate schedules
   that stage, adds what the stage requires to what is needed, and **rules out every
   other stage providing any of the same capabilities** (a stage is indivisible).
3. A needed capability whose candidates are all ruled out is
   [`PlanError::Unsatisfiable`](crate::core::plan::PlanError::Unsatisfiable), except a
   capability of an [external table](#external-tables), which installation satisfies.
4. When nothing is forced, one remaining choice is decided: the first preference that
   refers to a current candidate wins (a preference for a stage already ruled out becomes
   a [`PlanWarning::IgnoredPreference`](crate::core::plan::PlanWarning::IgnoredPreference),
   which the runner reports as a Warning at the start of the run); without one, a choice
   among refinements goes to the default refinement; anything still undecided is
   [`PlanError::Contested`](crate::core::plan::PlanError::Contested). Then back to step 2.
5. Every step goes through the capabilities and candidates in name order, so the outcome
   and the messages do not depend on registration order.

To follow how a plan was built: [`Plan::excluded`](crate::core::plan::Plan::excluded)
([`Excluded`](crate::core::plan::Excluded): the stage, the stage that ruled it out, a
capability both provide), [`Plan::commit_order`](crate::core::plan::Plan::commit_order),
[`Plan::warnings`](crate::core::plan::Plan::warnings) (the preferences the plan could not
honor: [`PlanWarning`](crate::core::plan::PlanWarning) `PreferredRuleSetAside`,
`PreferredRuleNotCandidate`, `IgnoredPreference`; the runner reports each as a Warning at
the start of the run), [`Plan::render`](crate::core::plan::Plan::render) (stable text,
for a report or a test that compares two plans).

A lowering rule is **set aside** when a peer declaration it depends on resolves to
nothing: the rule is excluded from rule selection and from the completeness check, listed
by [`FlmRegistry::set_aside_rules`](crate::core::registry::FlmRegistry::set_aside_rules),
and the registry build records one Note and succeeds (see
[`PeerName`](crate::core::registry::PeerName)).

### What a plan refuses

Each refusal is a [`PlanError`](crate::core::plan::PlanError) that identifies what is
involved and lists what exists. Names the registry does not declare:
[`UnknownDemand`](crate::core::plan::PlanError::UnknownDemand),
[`UnknownPreference`](crate::core::plan::PlanError::UnknownPreference),
[`UnknownBackend`](crate::core::plan::PlanError::UnknownBackend),
[`UnknownRule`](crate::core::plan::PlanError::UnknownRule),
[`UnknownElement`](crate::core::plan::PlanError::UnknownElement),
[`UnknownWorkflow`](crate::core::plan::PlanError::UnknownWorkflow) and
[`UnknownOutput`](crate::core::plan::PlanError::UnknownOutput) (the last two from
[`Plan::for_workflow`](crate::core::plan::Plan::for_workflow)). The others:

| refusal | variant |
|---|---|
| a render option value the registry does not declare, or set at the wrong site | [`RenderOption`](crate::core::plan::PlanError::RenderOption) |
| a stage needs a capability, table or option type nobody declares | [`NeedsUndeclared`](crate::core::plan::PlanError::NeedsUndeclared) |
| a table read under the wrong value type | [`NeedsTableType`](crate::core::plan::PlanError::NeedsTableType) |
| a stage reads an option type whose value is fixed when the registry is built | [`RegistryBoundRead`](crate::core::plan::PlanError::RegistryBoundRead) |
| a rewrite declaring reads of computed data, or requiring a capability that a post-freeze stage provides | [`RewriteReads`](crate::core::plan::PlanError::RewriteReads), [`RewriteNeedsComputed`](crate::core::plan::PlanError::RewriteNeedsComputed) |
| a stage requiring a collection | [`CollectionRequired`](crate::core::plan::PlanError::CollectionRequired) |
| a stage requiring what scheduling it would rule out | [`SelfExclusion`](crate::core::plan::PlanError::SelfExclusion) |
| no provider left, or several and nothing to choose | [`Unsatisfiable`](crate::core::plan::PlanError::Unsatisfiable), [`Contested`](crate::core::plan::PlanError::Contested) |
| several rules could lower one element on one backend, nothing to choose | [`RuleContest`](crate::core::plan::PlanError::RuleContest) |
| a cycle among the scheduled stages | [`Cycle`](crate::core::plan::PlanError::Cycle) |
| two scheduled stages writing one table | [`TwoWriters`](crate::core::plan::PlanError::TwoWriters) |
| two rewrite stages with nothing ordering them | [`RewriteTie`](crate::core::plan::PlanError::RewriteTie) |
| a stage without effects outside the store reading what an `Io` stage produced | [`IoProductReadByPure`](crate::core::plan::PlanError::IoProductReadByPure) |
| a cycle among the elements the chosen lowering rules produce | [`ProducesCycle`](crate::core::plan::PlanError::ProducesCycle) |
| a backend that cannot render every element a document may contain | [`Incomplete`](crate::core::plan::PlanError::Incomplete) |

### Order and batches

- The scheduled stages are sorted so that each follows what it depends on, with
  **registration order breaking ties**: two registries built from the same features,
  registered in the same order, give the same order.
- **Batches** ([`Plan::batches`](crate::core::plan::Plan::batches),
  [`Plan::batch_names`](crate::core::plan::Plan::batch_names)) are consecutive groups of
  stages that depend on nothing else inside the same batch. They are computed once, at
  plan build; both runner paths execute exactly these.
- **`Io` stages come last**, as a block of their own: an edge running the other way is
  refused and no batch mixes the two blocks, so the error gate (the point before the first batch holding an `Io` stage, where a run
  that has recorded an Error stops; see [Running a plan](#running-a-plan)) falls on a
  batch boundary
  ([`Plan::first_io_batch`](crate::core::plan::Plan::first_io_batch)).
- **Ordering edges between a rewrite stage and a post-freeze stage are ignored**: every
  rewrite runs first anyway. A rewrite that needs a computed table is refused instead.

## Running a plan

[`Run::new`](crate::core::run::Run::new)`(Arc<Doc>, Arc<Plan>, Externals)` checks the
document's option values against their bindings, resolves the option values of every
scheduled stage once for the whole run, and checks the installed external tables. **It
executes nothing.** Anything it cannot do is a [`RunError`](crate::core::run::RunError),
never a statement about the document; a problem with the document is always a
diagnostic.

Execution, in the plan's batches:

1. **The rewrite stages.** Each tree goes through every rewrite stage in the plan's
   order, each one's output feeding the next; each publishes its output for every tree as
   its own table, and the last output is the tree's frozen tree. A rewrite that fails on a
   tree records one Error diagnostic; that tree has no frozen form and no later stage
   reads it; the other trees are unaffected. The run then computes its document order:
   the document's walk applied to the frozen trees ([`crate::core::run::order`];
   [`Run::tree_order`](crate::core::run::Run::tree_order) is the order in which the walk
   enters the trees).
2. **Each batch of stages.** A `PerTree` stage runs once for each tree of the run (the
   trees the walk over the frozen trees reaches; see
   [`Run::tree_order`](crate::core::run::Run::tree_order)); a `Document` stage runs once. Every stage of a batch reads the store as it was
   when the batch began. **The whole batch is collected before anything is installed**,
   so a stage that panics leaves the store untouched. Installation order: the table
   pieces of each stage in the run's order of the trees, the stages in the plan's order,
   the tables of one stage in table-identifier order.
3. **The error gate.** Before the first batch that holds an `Io` stage, a run that has
   recorded any Error diagnostic (from a parse or from a stage) stops, unless
   [`RunOptions::continue_with_errors`](crate::core::run::RunOptions#structfield.continue_with_errors)
   is set; [`Run::halted_at_gate`](crate::core::run::Run::halted_at_gate) and
   [`RunOutcome`](crate::core::run::RunOutcome) report it. **The standard package
   registers no `Io` stage**: a plan built from it alone has no gate
   ([`Plan::first_io_batch`](crate::core::plan::Plan::first_io_batch) is `None`), and a
   document full of errors still renders. The host library's `write` stage is an `Io`
   stage.

[`Run::execute`](crate::core::run::Run::execute) runs to the end and returns a
`RunOutcome`; [`Run::step`](crate::core::run::Run::step) runs one batch and returns
[`Step::More`](crate::core::run::Step::More) or [`Step::Done`](crate::core::run::Step::Done)
([`Run::is_done`](crate::core::run::Run::is_done)). Stepping changes when work happens,
never what is computed. [`RunOptions`](crate::core::run::RunOptions) has three fields:
`sequential` (one thread even where the `parallel` cargo feature is compiled),
`continue_with_errors`, and `max_diagnostics` (how many of the run's diagnostics are
kept, read from the first step's options only). Apart from the error gate, none changes
what is computed. Pass the same options to every step.

Reading the result back: [`Run::table`](crate::core::run::Run::table) (any table of the
registry, by typed key; a caller is not limited to declared reads; `Ok(None)` means no
scheduled stage writes it), [`Run::frozen_tree`](crate::core::run::Run::frozen_tree),
[`Run::frozen_trees`](crate::core::run::Run::frozen_trees),
[`Run::tree_order`](crate::core::run::Run::tree_order),
[`Run::computed`](crate::core::run::Run::computed) (the whole store),
[`Run::diagnostics`](crate::core::run::Run::diagnostics) (a
[`DiagnosticsView`](crate::core::run::DiagnosticsView) chaining each fragment's parse
diagnostics, once per fragment, with the stages' diagnostics),
[`Run::pipeline_diagnostics`](crate::core::run::Run::pipeline_diagnostics) (the stages'
diagnostics alone).

## External tables

An external table is a table that no stage of the run writes: the caller installs its
value when the run is constructed ([`Externals`](crate::core::run::Externals)). A feature
declares one with
[`FeatureBuilder::external_table`](crate::core::registry::FeatureBuilder::external_table);
the standard one is [`EXTERNAL_INDEX`](crate::stdcore::features::refs::EXTERNAL_INDEX)
(`flm.refs.external-index`), the labels of other documents processed together with this
one, so that references between documents resolve.

- Plan construction treats the external table's capability as satisfied by installation
  and lists what must be installed in [`Plan::externals`](crate::core::plan::Plan::externals)
  and [`Plan::external_names`](crate::core::plan::Plan::external_names).
- [`Externals::install`](crate::core::run::Externals::install) takes a
  [`Stamp`](crate::core::run::Stamp): the registry's identity, the rewrite stages that
  ran, and a digest of the option values those rewrites read
  ([`Run::stamp`](crate::core::run::Run::stamp) is a run's own stamp). `Run::new` refuses a
  mismatch, so data computed under one reading of a document is never used under another
  without notice. [`Externals::install_unstamped`](crate::core::run::Externals::install_unstamped)
  is for data that did not come from this library; the caller is then responsible for its
  agreement with the document.
- `Run::new` also refuses a missing installation, a table the registry does not declare,
  a table not declared as external, and a value of the wrong type
  ([`RunError`](crate::core::run::RunError)).

## Value catch-up

A synthesized value is a small tree that a stage builds while a run is under way, held
outside the document's trees and numbered per stage invocation, such as the content a
reference displays or a formatted citation entry ([synthesized
values](crate::guide::concepts_overview#synthesized-values)). It has been through none of
the work the document's trees went through. Value catch-up is the work done on demand
when something comes to render a synthesized value: the plan's rewrites and the per-tree
stages the consumer depends on run over that one tree, and what they compute goes into a
catch-up store that a view reads on top of the run's store, a table present there read
whole from it ([value catch-up](crate::guide::concepts_overview#value-catch-up)).

- [`StageCx::resolve_value`](crate::core::run::StageCx::resolve_value) (and
  [`resolve_value_with`](crate::core::run::StageCx::resolve_value_with)) performs it: the
  plan's rewrite stages over that tree alone, then the per-tree stages the calling stage
  depends on ([`Plan::per_tree_closure`](crate::core::plan::Plan::per_tree_closure)).
- The result is a [`CaughtUp`](crate::core::run::CaughtUp): the tree's identifier
  (`id`), the rewritten tree (`tree`), the catch-up store (`scratch`) and the diagnostics
  (`diags`), which the calling stage records itself; or a
  [`CatchUpError`](crate::core::run::CatchUpError). **The run's own store is never
  written.**
- A lowering rule renders a synthesized value with
  [`Lowerer::splice`](crate::core::lowering::Lowerer::splice), which performs the
  catch-up for it.

Two consequences, also stated at `resolve_value`:

- **Reading.** A stage run again over a value reads a whole table from the catch-up store
  whenever the catch-up computed that table, never entry by entry
  ([`ComputedView::with_overlay`](crate::core::data::store::ComputedView::with_overlay)):
  it sees the value's own entries and nothing of the document's. A stage that works
  inside a value *and* needs document-wide data reads a table that no per-tree stage of
  the catch-up writes, in practice one a `Document`-scope stage writes. The `refs`
  feature's resolve stage reads [`REF_TARGETS`](crate::stdcore::features::refs::REF_TARGETS)
  (document scope) for that reason, not [`LABELS`](crate::stdcore::features::refs::LABELS)
  (per tree).
- **Writing.** A table piece of class
  [`PieceClass::DocumentFact`](crate::core::data::table::PieceClass::DocumentFact) is
  refused during a catch-up
  ([`StoreError::DocumentFactInValue`](crate::core::data::store::StoreError::DocumentFactInValue));
  table pieces keyed by node address, and empty ones, go to the catch-up store
  ([`FoldTarget::Scratch`](crate::core::data::store::FoldTarget::Scratch)).

## Determinism

A run's output depends on the document, the registry, the plan and the installed
external tables, and on nothing else: not on the number of threads, not on the order in
which work finished. Four things guarantee it, each implemented once:

- **the batches**, computed at plan build and executed alike by both runner paths, their
  results collected in index order;
- **the fold**, [`fold_outs`](crate::core::data::store::fold_outs), shared by both paths:
  a stage's per-tree table pieces absorbed in the run's order of the trees, its tables
  installed in table-identifier order, and a declared write without a table piece still
  installed as an empty table, so a read returning `None` always means "no stage
  provides this";
- **one writer per table**, guaranteed by plan construction and checked again by the
  store;
- **name-ordered scans** during provider selection.

The parallel path is the crate's `parallel` cargo feature, on by default; the sequential
path is always compiled and is the only one without default features (the WebAssembly
configuration). [`RunOptions::sequential`](crate::core::run::RunOptions::sequential)
selects it per run.

## Examples

Demanding a collection, and reading a plan back:

```rust
use flm_core::core::plan::{Overrides, Plan, PlanRequest};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::stdcore::backends::text;
use flm_core::stdcore::{self, StdcoreConfig, VALIDATED};

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;

// A collection demand expands to its members: every check the registry holds.
let checks = Plan::build(&registry, &PlanRequest::new().demand(VALIDATED))?;
assert!(checks.demands().len() > 1);
assert!(checks.scheduled_names().iter().any(|n| n.as_str() == "flm.refs.check-labels"));
assert!(checks.active_backends().is_empty()); // nothing asked for an output

// The `text` workflow: rewrites in a total order, then the stages.
let render = Plan::for_workflow(&registry, text::WORKFLOW, &Overrides::none())?;
let rewrites: Vec<String> =
    render.rewrite_names().iter().map(|n| n.as_str().to_string()).collect();
assert_eq!(
    rewrites,
    ["flm.input.splice", "flm.userdefs.expand-always", "flm.userdefs.expand", "flm.blocks.structure"]
);
assert!(render.stage_names().iter().any(|n| n.as_str() == "flm.text.final"));
assert!(render.active_backends().iter().any(|b| b.as_str() == "text"));
// A rendering workflow demands `flm.validated` beside its output, so the
// checks are in the render's plan too.
assert!(render.scheduled_names().iter().any(|n| n.as_str() == "flm.refs.check-labels"));

// `stdcore` declares no `Io` stage, so no plan built from it has an error gate.
assert!(render.first_io_batch().is_none());
# Ok::<(), Box<dyn std::error::Error>>(())
```

A table, a stage that writes it, a feature that registers both, and a demand for the
table's capability; nothing lists the stages to run:

```rust
use std::sync::Arc;

use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::core::plan::{Plan, PlanCx, PlanRequest};
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, RegistryBuilder, RegistryInputs,
};
use flm_core::core::run::{Externals, Run, RunOptions, StageCx, Step};
use flm_core::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_core::core::data::store::StageOut;
use flm_core::core::data::table::{Table, TableName};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::stdcore::{self, StdcoreConfig};

#[derive(Debug, Default)]
struct NodeCount(usize);

// `absorb` combines the per-tree table pieces into the one table the store holds.
impl Table for NodeCount {
    fn absorb(&mut self, piece: NodeCount) {
        self.0 += piece.0;
    }
}

const NODE_COUNT: TableName<NodeCount> = TableName::new("example.node-count");

struct CountNodes;

impl Stage for CountNodes {
    fn decl(&self) -> StageDecl {
        // Writing a table provides its capability; nothing else to declare.
        StageDecl::new("example.count", Effect::Compute, Scope::PerTree).writes(NODE_COUNT)
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        // Requiring a capability makes the plan schedule a provider for it.
        Needs::new().requires("flm.blocks")
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let count = cx.my_tree().map_or(0, |tree| tree.node_count());
        out.table(NODE_COUNT).expect("declared as a write above").0 += count;
    }
}

struct Counting;

impl FlmFeature for Counting {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("example", "Counts the nodes of a document")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.table(NODE_COUNT, "how many nodes the document's trees hold")?;
        b.stage(Arc::new(CountNodes))?;
        Ok(())
    }
}

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.feature(Arc::new(Counting));
let registry = builder.build(&RegistryInputs::new())?;

let plan = Arc::new(Plan::build(&registry, &PlanRequest::new().demand("example.node-count"))?);
assert!(plan.scheduled_names().iter().any(|n| n.as_str() == "example.count"));
// The requirement on `flm.blocks` pulled in its default refinement's rewrite.
assert!(plan.rewrite_names().iter().any(|n| n.as_str() == "flm.blocks.structure"));

let source = Arc::new(
    FlmSource::new("One paragraph.\n\nAnd another.").with_origin(FlmOrigin::new("d.flm")),
);
let fragment = parse_fragment(
    registry.lang(),
    source,
    FragmentLayout::default(),
    &ParseOptions::default(),
)?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));

// One batch at a time; `execute` would run them all.
let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none())?;
while run.step(&RunOptions::new())? == Step::More {}
assert!(run.is_done() && !run.diagnostics().has_errors());
assert!(run.table(NODE_COUNT)?.expect("the stage was scheduled").0 > 0);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## See also

- [AI guide](crate::guide::ai_guide): the entry point, with the vocabulary, the recipes
  and the pitfalls.
- [AI guide: document model](crate::guide::ai_guide_document_model): the trees and tables
  stages read and write.
- [AI guide: extending FLM](crate::guide::ai_guide_extending): writing stages, tables,
  capabilities, workflows and lowering rules of your own.
- [Stages, capabilities, and plans](crate::guide::pipeline): the full chapter;
  [Rendering: backends and outputs](crate::guide::rendering) for the lowering, emit and
  final stages.
- [Concepts overview](crate::guide::concepts_overview): every term used here.
