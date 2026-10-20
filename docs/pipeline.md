# Stages, capabilities, and plans

Processing a document is a sequence of **stages**. A stage is one unit of work: it
declares the capabilities it provides and requires, the tables it reads and writes, its
effect (compute, check, or input/output) and its scope (once per tree, or once per
document), and the plan (defined below) decides whether and when it runs
([`Stage`](crate::core::stage::Stage)). A capability is a registered dotted
name for a state of processing that a stage brings about, such as
`flm.blocks.structured` ([`CapabilityDef`](crate::core::registry::CapabilityDef)).

No caller writes the sequence of stages. A caller *demands* the capabilities it wants,
that is, asks the plan for stages that provide them
([`PlanRequest::demand`](crate::core::plan::PlanRequest::demand)), and the library
derives which stages have to run and in which order. A **plan** is the list of stages
that satisfies a plan request, in dependency order and grouped in batches
([`Plan`](crate::core::plan::Plan)); a **run** is one execution of one plan over one
document ([`Run`](crate::core::run::Run)).

This chapter describes each of these in turn: what a stage is, what a capability is,
how a plan is built and what it refuses, how a run executes a plan, and which
guarantees hold about the result. The companion chapter [The document
model](crate::guide::document_model) describes what stages read and write, and the
concepts page summarizes this chapter under [stages, capabilities, and
plans](crate::guide::concepts_overview#stages-capabilities-and-plans).

Three properties explain most of the design:

- **A plan holds nothing about any document.** It is a value computed from the
  registry and the plan request alone, so one plan renders every document the same way
  and may be built once and reused.
- **A stage does not modify what it reads.** It reads the frozen trees and a checked
  view of the computed tables, and produces new table pieces and diagnostics into a
  collector. Only the runner, the code that executes a plan
  ([`Run::execute`](crate::core::run::Run::execute)), installs a collector's table
  pieces into the store.
- **The result does not depend on timing.** The batches are fixed when the plan is
  built, and the function that installs what a batch produced
  ([`fold_outs`](crate::core::data::store::fold_outs)) is the same code on the
  sequential and on the parallel path.

## What a stage is

There are two kinds of stage, which differ by when they run relative to the moment
the trees are frozen ([The document model](crate::guide::document_model)).

A [`RewriteStage`](crate::core::stage::RewriteStage) runs **before** the trees are
frozen and turns one tree into another. The runner applies the plan's rewrite stages
to each tree in turn, in the total order the plan fixes, and gives each one the
previous one's output; the last output is that tree's frozen tree. A rewrite reads no
computed data at all: the signature of its
[`rewrite`](crate::core::stage::RewriteStage::rewrite) method contains no view of the
store, so "trees change only before they are frozen" is guaranteed by the types rather
than by a rule to remember. Each rewrite publishes its own output for every tree as a
[`TreesTable`](crate::core::stage::TreesTable), so a later stage can read a tree in a
particular state.

A [`Stage`](crate::core::stage::Stage) runs **after** the trees are frozen. It reads
the frozen trees and the tables it declared, and writes tables of its own. Two parts of
its declaration tell the plan how to treat it:

- Its [`Effect`](crate::core::stage::Effect) says what kind of work it does:
  [`Compute`](crate::core::stage::Effect::Compute) produces computed data,
  [`Check`](crate::core::stage::Effect::Check) produces diagnostics only (a `Check`
  stage that declares a written table is refused when the registry is built), and
  [`Io`](crate::core::stage::Effect::Io) has effects observable outside the store,
  such as writing the output a caller asked for. A service that a `Compute` stage calls
  may reach the network or another program without making that stage `Io`.
- Its [`Scope`](crate::core::stage::Scope) says how often it runs:
  [`PerTree`](crate::core::stage::Scope::PerTree), once for each tree that has a frozen
  form, or [`Document`](crate::core::stage::Scope::Document), once for the whole
  document, which it reads through the walk.

A stage is registered once and may run many times, over many documents. It holds only
its own configuration and modifies nothing: the runner may run the per-tree calls of
one stage in parallel, and the result must not depend on that.

### Declaration and needs

What a stage states about itself comes in two parts, because the two are fixed at
different moments.

The **declaration** ([`StageDecl`](crate::core::stage::StageDecl), or
[`RewriteDecl`](crate::core::stage::RewriteDecl) for a rewrite stage) is fixed when a
feature registers the stage: its name, its effect, its scope, the capabilities it
provides and the tables it writes. Choosing the provider of a capability, and refusing
a table with two writers, rely on the declaration never varying from one plan to the
next.

The **needs** ([`Needs`](crate::core::stage::Needs)) are computed once per plan, by
[`Stage::needs`](crate::core::stage::Stage::needs), from the plan request: the
capabilities the stage requires, the tables it reads (and the tables it reads only if
they are provided, [`reads_opt`](crate::core::stage::Needs::reads_opt)), the option
types it reads, and its ordering-only edges ([`after`](crate::core::stage::Needs::after),
[`before`](crate::core::stage::Needs::before)). Most stages return constant needs; a
stage whose inputs depend on the request computes them here, from what
[`PlanCx`](crate::core::plan::PlanCx) gives access to. A backend's lowering stage is
the standard example: it reads every table that the lowering rules selected for that
backend read.

The plan joins the declaration and the needs into a stage manifest
([`Manifest`](crate::core::stage::Manifest)) and keeps it. From then on the runner and
every validation consult the plan's copy and never call the trait methods again, so one
plan cannot act on two different results from one stage.

Declaring a written table also provides that table's capability, and declaring a table
read also requires it. Both are derived, so a stage author lists reads and writes and
nothing else.

### What a stage works through

A stage never writes to the store, and never accesses anything it did not declare.
Everything it may read is given to it in a [`StageCx`](crate::core::run::StageCx), and
everything it produces goes into a [`StageOut`](crate::core::data::store::StageOut).

Through the context, a stage reads three kinds of thing:

- **The trees.** A per-tree stage gets the frozen tree it was called for
  ([`my_tree`](crate::core::run::StageCx::my_tree), identified by
  [`tree_id`](crate::core::run::StageCx::tree_id)); a document stage walks every frozen
  tree in document order ([`walk`](crate::core::run::StageCx::walk)). The address of a
  node the stage holds comes from [`addr`](crate::core::run::StageCx::addr).
- **The tables it declared**, by typed key ([`get`](crate::core::run::StageCx::get)). A
  read the stage did not declare is an
  [`AccessError`](crate::core::data::store::AccessError) that identifies the stage and
  the table, not a silent "nothing there"; a table declared as an optional read
  returns `None` when this plan has no stage that writes it.
- **Its option values** ([`option`](crate::core::run::StageCx::option)), resolved once
  for the whole run, and the services the registry holds
  ([`service`](crate::core::run::StageCx::service)).

Into the collector, a stage writes its table piece of each table it declared it writes
([`StageOut::table`](crate::core::data::store::StageOut::table)) and the diagnostics it
reports ([`diags`](crate::core::data::store::StageOut#structfield.diags)). Both sides
are checked where the access happens, against what the plan resolved for this stage, so
a mistake identifies the stage and the table instead of reaching the store.

A rewrite gets the much smaller [`RewriteCx`](crate::core::run::RewriteCx) instead: the
registry, the identity and layout of the tree being rewritten, and the option values it
declared — no tables at all.

## Capabilities

A capability is a registered dotted name for a state of processing that a stage brings
about, such as `flm.blocks.structured`: a stage declares the capabilities it provides
and the capabilities it requires, and a caller demands the capabilities it wants
([`CapabilityDef`](crate::core::registry::CapabilityDef)). Another example is
`flm.refs.resolutions`, the capability of the table of resolved references.
Capabilities are the only terms in which stages and callers state what they want.
Capability names, table names and [collection](#collections) names share one
namespace, because a table's name *is* a capability: "this table is present and
complete".

A feature therefore declares a capability explicitly
([`CapabilityDef`](crate::core::registry::CapabilityDef)) only for a state that no
table represents: a marker, the result of a check, or the parent of refinements.

### Refinement

A capability may **refine** another capability, which means that it is a stronger form
of it: `flm.blocks.structured` refines `flm.blocks`. Providing the refinement also
satisfies a demand for the parent, so a caller that just wants block structure does not
have to know which of several ways of structuring exists. At most one refinement of a
parent may be marked its **default**: the one a demand for the parent selects when
nothing else decides. In the standard package, `flm.blocks` is refined by
`flm.blocks.structured` (the default) and by `flm.blocks.raw`, which leaves the trees
without block structure.

### Collections

A **collection** ([`CollectionDef`](crate::core::registry::CollectionDef)) is a name for
a *set* of capabilities to which any feature may add a member. Demanding a collection
demands every member. A feature uses a collection to join a set of demands it does not
own: it declares its own check capability, adds it as a member with
[`collection_member`](crate::core::registry::FeatureBuilder::collection_member), and a
caller that demands the collection gets that check too. The standard example is
`flm.validated`, the set of every check the registry offers. Every workflow the standard
package registers demands it, a rendering workflow beside its output, so a render runs
the same checks as a run that only validates.

A collection can only be demanded: using one where a capability is expected — among
the capabilities a stage provides, or among those a stage requires — is refused.

```rust
# use flm_core::core::plan::{Plan, PlanRequest};
# use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
# use flm_core::stdcore::{self, StdcoreConfig};
# let mut builder = RegistryBuilder::new();
# builder.use_package(stdcore::package(StdcoreConfig::new()));
# builder.use_package(flm_core::math_passthrough::package());
# let registry = builder.build(&RegistryInputs::new())?;
// `flm.validated` is a collection: demanding it demands each of its members.
let plan = Plan::build(&registry, &PlanRequest::new().demand("flm.validated"))?;
assert!(plan.demands().len() > 1);

// Every scheduled check is there, and no backend takes part: nothing asked for
// an output.
assert!(plan.scheduled_names().iter().any(|name| name.as_str() == "flm.refs.check-labels"));
assert!(plan.active_backends().is_empty());
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Ordering-only edges

Requiring a capability makes the plan *schedule* a stage that provides it. Sometimes a
stage only needs to be placed relative to work that may or may not happen at all: it
does not want that work done on its account, but if the work is done, it must come
first (or last). That is an **ordering-only edge**.

An ordering-only edge refers to a capability, exactly as a requirement does, but says
only "run after (or before) the stage that provides this capability, if a stage does".
It never causes anything to be scheduled, and it has no effect when nothing provides
that capability. A stage declares its own edges with
[`after`](crate::core::stage::Needs::after) and
[`before`](crate::core::stage::Needs::before) on [`Needs`](crate::core::stage::Needs).
A feature may also place an edge on *another* feature's stage with
[`OrderingDef`](crate::core::registry::OrderingDef), to place its own work inside an
order it does not own. An edge to a capability that has refinements follows whichever
refinement the plan scheduled.

In the standard package, the rewrite of the `input` feature, which inserts the content
of included files in place of their `\input` commands, declares ordering-only edges
before the expansion of user definitions and before block structure. It requires
neither, but when either runs, the content of included files must already be in place.

## Asking for a plan

A [`PlanRequest`](crate::core::plan::PlanRequest) is everything a caller says about
what it wants from a plan. It has five parts, and nothing else decides what the plan
contains:

- **demands** — the capabilities wanted; a collection here stands for its members;
- **prefer** — which stage to schedule where several could provide one capability,
  most preferred first;
- **lowering** — which lowering rules to use, per backend, where several rules could
  lower one element;
- **unsupported** — per backend, the accepted unrenderable elements. An accepted
  unrenderable element is an element the plan request lists, per backend, as one the
  backend need not render: the plan builds anyway, and each occurrence lowers to a
  childless `flm.pres.unsupported` node with one Error diagnostic;
- **render_options** — values for the option types that are set on a render request.

The request also records the workflow it was made from, if any. Plan building does not
consult that name, but the plan's errors and warnings name the workflow ("the plan request
for workflow `html` prefers the stage …"; "the plan request prefers …" for a request built
by hand), so that a person reading them knows which workflow's request they are about.

Most callers never write a plan request. A **workflow**
([`WorkflowDef`](crate::core::registry::WorkflowDef)) is a named set of demands together
with the choices that go with them, held in the registry as a complete plan request —
which outputs to produce, what else to demand, and how the choices are decided — so a
caller chooses one by name and changes at most a few things
about it through [`Overrides`](crate::core::plan::Overrides). An **output**
([`OutputDef`](crate::core::registry::OutputDef)) is a name a caller can ask for, such
as `text`. It ties together the capability the plan must reach, the table in which the
resulting artifacts are found, and the media type and file extension of the entry
artifact. Demanding an output is demanding its capability. The standard package
registers five workflows, `check`, `text`, `latex-recomposed`, `latex-rendered` and
`html`, and four outputs, `text`, `latex-recomposed`, `latex-rendered` and `html`.

[`Plan::for_workflow`](crate::core::plan::Plan::for_workflow) turns a workflow into a
plan request and builds the plan; [`workflow_request`](crate::core::plan::workflow_request)
performs only the first half, for a caller that wants to inspect or adjust the request.
A workflow that extends another starts from the other's fields: each field it declares
replaces the inherited one. The registry gives the chain of workflows, root first
([`FlmRegistry::workflow_chain`](crate::core::registry::FlmRegistry::workflow_chain)),
and the fields combined along it
([`FlmRegistry::combined_workflow`](crate::core::registry::FlmRegistry::combined_workflow)),
from which the request is built.

Every plan carries a [`PlanKey`](crate::core::plan::PlanKey): one number computed over
the registry's identity together with everything the plan request decides. Two plan
requests with the same key against the same registry build the same plan, so a plan can
be cached under its key. The workflow name decides nothing and is left out: two workflows
that produce the same request share a plan.

```rust
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::stdcore::{self, StdcoreConfig};

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;

let plan = Plan::for_workflow(&registry, "text", &Overrides::none())?;

// The rewrites run first, in the total order the plan fixes for them.
let rewrites = plan.rewrite_names();
let rewrites: Vec<&str> = rewrites.iter().map(|name| name.as_str()).collect();
assert_eq!(
    rewrites,
    ["flm.input.splice", "flm.userdefs.expand-always", "flm.userdefs.expand", "flm.blocks.structure"]
);

// Then the stages, each after everything it depends on.
let stages = plan.stage_names();
let stages: Vec<&str> = stages.iter().map(|name| name.as_str()).collect();
assert!(stages.contains(&"flm.refs.resolve"));
assert!(stages.contains(&"flm.text.final"));

// The checks are here too: a rendering workflow demands the validation
// collection beside its output, so a render reports what `check` reports.
assert!(stages.contains(&"flm.refs.check-labels"));

// One backend takes part, because one output was demanded.
assert!(plan.active_backends().iter().any(|backend| backend.as_str() == "text"));

// Nothing here mentions a document: the same plan renders any of them.
assert_eq!(plan.demands().len(), 9);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## How a plan is built

[`Plan::build`](crate::core::plan::Plan::build) works in five steps:

1. **The plan request is checked.** Every demanded capability, preferred stage, backend,
   preferred rule, accepted unrenderable element and render option value must be
   something the registry declares. Each failure identifies what was written and lists
   what exists.
2. **A lowering rule is chosen per element, for every registered backend.** Which
   backends take part is not known yet — it follows from which stages are scheduled — so
   when the rules of a backend cannot be chosen, the failure is kept and raised only if
   that backend turns out to take part.
3. **Every stage is asked what it needs**, and the needs are resolved against the
   registry.
4. **A provider is chosen for each needed capability**, by the provider selection
   described below.
5. **The stages are ordered, and the plan is checked.**

### Choosing a provider

Several stages may provide one capability, and exactly one of them is scheduled.
Provider selection chooses one stage for each capability the plan needs: the forced
choices are made first and followed to a fixed point, then one tie is broken by the
request's preferences or a default refinement, and the forced choices are followed
again, until nothing is left to decide ([`crate::core::plan::selection`]). In detail:

1. Start from the demanded capabilities. A *candidate* of a capability is a stage that
   provides it — directly, or by providing a capability that refines it — and that has
   not been ruled out.
2. Follow the forced choices to a fixed point. A needed capability with exactly one
   candidate schedules that stage, adds what that stage requires to what is needed, and
   **rules out every other stage that provides any of the same capabilities**. This last
   part makes a stage indivisible: scheduling a stage commits the plan to everything the
   stage provides, so no second stage may provide any part of it.
3. A needed capability whose candidates have all been ruled out is
   [`PlanError::Unsatisfiable`](crate::core::plan::PlanError), which lists each provider
   and what ruled it out. The exception is a capability that belongs to an external
   table, which is satisfied by installing that table when the run is constructed.
4. When nothing more is forced, the remaining choices are decided one at a time. The
   first preference in the plan request that refers to a current candidate wins; a
   preference for a stage already ruled out is recorded as a
   [`PlanWarning`](crate::core::plan::PlanWarning) and skipped, and the runner reports
   it as a Warning at the start of the run. Without an applicable preference, a choice
   among refinements of one capability is decided by the marked default refinement.
   Anything still undecided is [`PlanError::Contested`](crate::core::plan::PlanError),
   which lists the candidates. After each choice, selection resumes at item 2 of this
   list.
5. Every step goes through the needed capabilities and their candidates in name
   order, so the outcome and every message are the same regardless of the order in
   which the features registered.

The plan keeps a record of these choices.
[`Plan::excluded`](crate::core::plan::Plan::excluded) lists each
[`Excluded`](crate::core::plan::Excluded) stage together with the stage that ruled it
out and a capability both provide, and
[`Plan::commit_order`](crate::core::plan::Plan::commit_order) gives the order in which
the choices were made; both serve to follow how a plan was built.
[`Plan::warnings`](crate::core::plan::Plan::warnings) holds the preferences the plan
could not honor — a preferred lowering rule that is set aside (excluded from rule
selection; see [`PeerName`](crate::core::registry::PeerName)) or belongs to another
backend, a preferred stage that a forced choice ruled out; the runner reports each as a
diagnostic of Warning severity at the start of the run.
[`Plan::render`](crate::core::plan::Plan::render) prints all of this as a few lines of
stable text, for a report or a test that compares two plans.

Lowering rules are chosen differently. Lowering rule selection chooses one rule per
element and backend in a fixed order: the rule the request prefers by name, else the
backend's native emission, else a rule marked default on that backend, else a rule
marked default everywhere, else the sole candidate
([`SelectedRules`](crate::core::plan::SelectedRules)). [Rendering: backends and
outputs](crate::guide::rendering) describes it in full.

### What a plan refuses

Plan construction catches the contradictions between features written independently.
Each is a [`PlanError`](crate::core::plan::PlanError) that identifies the stages,
capabilities or elements involved. Besides names the registry does not declare, a plan
refuses:

- a stage that needs a capability, table or option type nobody declares, or reads a
  table under the wrong value type;
- a stage that reads an option type whose value is fixed when the registry is built,
  which no stage may read;
- a rewrite stage that declares that it reads computed data, or requires a capability
  that a stage running after the trees are frozen provides, because rewrites run
  before anything is computed;
- a stage that requires a collection, which may only be demanded;
- a stage that requires a capability that scheduling the stage itself would rule out
  ([`SelfExclusion`](crate::core::plan::PlanError::SelfExclusion));
- a capability with no provider left, or with several and nothing to choose between
  them;
- several rules able to lower one element on one backend, with nothing to choose
  between them;
- a cycle among the scheduled stages, listed in dependency order;
- two scheduled stages that write one table;
- two rewrite stages with nothing that orders them relative to each other; the error
  gives the ordering edge that would decide the order;
- a stage without effects outside the store that reads what a stage with such effects
  produced;
- a cycle among the elements the chosen lowering rules produce;
- a backend that cannot, in the end, render every element a document may contain.

[`Plan::for_workflow`](crate::core::plan::Plan::for_workflow) refuses two more cases,
because it looks a workflow up by name first: a workflow the registry does not
register, and a workflow that lists an output that is not registered.

### Order and batches

The scheduled stages are sorted so that every stage follows what it depends on, with
registration order breaking ties, so two registries built from the same features give
the same order. Within that order the plan computes **batches**: consecutive groups of
stages that depend on nothing else inside the same batch, so they may run at the same
time ([`Plan::batches`](crate::core::plan::Plan::batches),
[`Plan::batch_names`](crate::core::plan::Plan::batch_names)). The batches are computed
once, here, and both runner paths execute exactly these batches; the two paths
therefore produce identical results.

Two placement rules complete the order:

- **Stages with effects outside the store run last.** The scheduled stages form two
  blocks: every `Compute` and `Check` stage first, then every `Io` stage, each block in
  its own dependency order. An edge running the other way is refused, and no batch
  mixes the two blocks, so the point at which a run with errors stops is a batch
  boundary ([`Plan::first_io_batch`](crate::core::plan::Plan::first_io_batch)).
- **Ordering edges between a rewrite and a later stage are ignored.** Every rewrite
  runs before every stage that runs after the trees are frozen anyway, so such an edge
  cannot change any placement, and it states no data dependency. A rewrite that needs a
  computed table is a different matter, and is refused.

## Running a plan

A run is one execution of one plan over one document; it owns the frozen trees, the
store and the diagnostics ([`Run`](crate::core::run::Run)). One document may therefore
be run several times, under different plans, without one run seeing another's results.

[`Run::new`](crate::core::run::Run::new) takes the document, the plan and the
[`Externals`](crate::core::run::Externals) the caller installs. It checks the option
values the document sets against the registry's declarations, resolves the option
values of every scheduled stage once for the whole run, and checks the tables the
caller installed. Nothing is executed there: a run that exists has sound inputs.
Anything `Run::new` cannot do is a [`RunError`](crate::core::run::RunError), never a
statement about the document; a problem with the document is always a diagnostic.

Execution then proceeds in the batches the plan fixed:

1. **The rewrites.** Each tree goes through every rewrite in the plan's order, each
   rewrite's output feeding the next; each rewrite's output for every tree is published
   as that rewrite's table, and the last output is the tree's frozen tree. A rewrite
   that fails on a tree records one error diagnostic; that tree has no frozen form, so
   no later stage reads it, and the document's other trees are unaffected.
2. **Each batch of stages.** A per-tree stage runs over every tree that has a frozen
   form; a document stage runs once. Every stage of a batch reads the store as it was
   when the batch began, so the stages of one batch — and the several calls of one
   per-tree stage — may run at the same time. When the batch is over, its table pieces
   are installed into the store: the table pieces of each stage in the run's order of
   the trees (the order in which the document's walk enters them,
   [`Run::tree_order`](crate::core::run::Run::tree_order)), the stages in the plan's
   order, and the tables of each stage in table-identifier order. The whole batch is collected before anything is installed, so
   a stage that panics leaves the store untouched and the run where it was.
3. **The error gate.** Before the first batch that holds a stage with effects outside
   the store, a run that has recorded any error diagnostic — from parsing or from the
   stages — stops, unless
   [`RunOptions::continue_with_errors`](crate::core::run::RunOptions#structfield.continue_with_errors)
   is set. A document with errors therefore produces no effects outside the store unless
   the caller asks for them. The standard package registers no `Io` stage — its
   rendering stages write their artifacts into tables, and the embedding program
   decides where they go — so a plan built from it alone has no gate to stop at, and
   [`Plan::first_io_batch`] returns `None`.

[`Run::execute`](crate::core::run::Run::execute) runs to the end and reports whether the
error gate stopped it; [`Run::step`](crate::core::run::Run::step) runs one batch and
returns [`Step::More`](crate::core::run::Step) or `Step::Done`. Stepping changes when the
work happens, never what is computed: the batches are fixed and the folds do not depend
on timing.

[`RunOptions`](crate::core::run::RunOptions) does not change what is computed either. It
says whether to run on one thread even where the parallel build allows more, whether to
pass the error gate anyway, and how many diagnostics to keep.

To read the result, [`Run::table`](crate::core::run::Run::table) reads one table by its
typed key (a caller, unlike a stage, may read any table of the registry);
[`Run::frozen_tree`](crate::core::run::Run::frozen_tree) and
[`Run::frozen_trees`](crate::core::run::Run::frozen_trees) give the trees the stages
read; and [`Run::diagnostics`](crate::core::run::Run::diagnostics) gives a
[`DiagnosticsView`](crate::core::run::DiagnosticsView) that chains each fragment's parse
diagnostics with the diagnostics the stages produced. A fragment placed twice reports its
parse diagnostics once, at the first place the walk reaches it, because they are facts
about the text, not about a placement.

[`Plan::first_io_batch`]: crate::core::plan::Plan::first_io_batch

## A stage of your own, demanded by name

The example below is a small version of the whole cycle: a table, a stage that writes
it, a feature that registers both, a demand for the table's capability, and a run that
produces the table. Nothing lists the stages to run: the stage's requirement of
`flm.blocks` makes the plan schedule the block-structure rewrite.

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

// The table: one typed value. `absorb` says how the pieces of the per-tree
// runs combine into the one table the store holds.
#[derive(Debug, Default)]
struct NodeCount(usize);

impl Table for NodeCount {
    fn absorb(&mut self, piece: NodeCount) {
        self.0 += piece.0;
    }
}

const NODE_COUNT: TableName<NodeCount> = TableName::new("example.node-count");

// The stage: it writes that table, once per tree, over structured trees.
struct CountNodes;

impl Stage for CountNodes {
    fn decl(&self) -> StageDecl {
        StageDecl::new("example.count", Effect::Compute, Scope::PerTree)
            .writes(NODE_COUNT)
            .docs("counts the nodes of each frozen tree")
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

// The feature: what registers the two with a registry.
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

// The demand names the table's capability, and nothing else.
let request = PlanRequest::new().demand("example.node-count");
let plan = Arc::new(Plan::build(&registry, &request)?);
assert!(plan.scheduled_names().iter().any(|name| name.as_str() == "example.count"));
// Requiring `flm.blocks` scheduled its default refinement's rewrite.
assert!(plan.rewrite_names().iter().any(|name| name.as_str() == "flm.blocks.structure"));

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
let doc = Arc::new(Doc::new(content));

// One batch at a time, to show what stepping does; `execute` runs them all.
let mut run = Run::new(doc, plan, Externals::none())?;
while run.step(&RunOptions::new())? == Step::More {}
assert!(run.is_done());
assert!(!run.diagnostics().has_errors());

let counted = run.table(NODE_COUNT)?.expect("the stage was scheduled, so the table is there");
assert!(counted.0 > 0);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## External tables

An external table is a table that no stage of the run writes: the caller installs its
value when the run is constructed ([`Externals`](crate::core::run::Externals)). Data
from outside one document reaches the stages of a run this way, for example an index of
the labels defined in the other documents of a collection, so that references between
the documents resolve. A feature declares an external table with
[`external_table`](crate::core::registry::FeatureBuilder::external_table); the standard
example is [`EXTERNAL_INDEX`](crate::stdcore::features::refs::EXTERNAL_INDEX).

Plan construction treats the capability of an external table as satisfied by
installation, and lists the tables that must be installed in
[`Plan::externals`](crate::core::plan::Plan::externals).
[`Run::new`](crate::core::run::Run::new) refuses a run that is missing one, that
installs a table the registry does not declare, that installs a table a stage writes,
or that installs a value of the wrong type.

An external table's value produced by an earlier run of this library carries a
[`Stamp`](crate::core::run::Stamp): the identity of the registry it was produced under,
the rewrite stages that ran, and a digest of the values of the options those rewrites
read ([`Run::stamp`](crate::core::run::Run::stamp) is a run's own stamp). Constructing a
run compares the stamp with what this run would produce and refuses a mismatch, so data
computed under one reading of a document cannot be used under another without notice.
Data that did not come from this library has no stamp and is installed with
[`Externals::install_unstamped`](crate::core::run::Externals::install_unstamped); the
caller is then responsible for its agreement with the document.

## Value catch-up

A synthesized value is a small tree that a stage builds while a run is under way, held
outside the document's trees and numbered per stage invocation, such as the content a
reference displays or a formatted citation entry ([synthesized
values](crate::guide::concepts_overview#synthesized-values); [The document
model](crate::guide::document_model#synthesized-values)). For the pipeline, what matters
is that a synthesized value has not been through any of the work the document's own
trees went through. Value catch-up is the work done on demand when something comes to
render a synthesized value: the plan's rewrites and the per-tree stages the consumer
depends on run over that one tree, and what they compute goes into a catch-up store that
a view reads on top of the run's store, a table present there read whole from it ([value
catch-up](crate::guide::concepts_overview#value-catch-up)).

[`StageCx::resolve_value`](crate::core::run::StageCx::resolve_value) performs it: the
plan's rewrites run over that tree alone, then the per-tree stages the calling stage
depends on ([`Plan::per_tree_closure`](crate::core::plan::Plan::per_tree_closure)) run
over it, and what they compute goes into the catch-up store, which belongs to that one
catch-up. The run's own store is never written. The result is a
[`CaughtUp`](crate::core::run::CaughtUp): the identifier given to the tree, the
rewritten tree itself, the catch-up store, and the diagnostics the catch-up raised,
which the calling stage records itself. A stage that runs again over a value reports
exactly as it does over a document tree, so content a value copies from the document
is reported again wherever the value is brought up to date, and each report still
points at the place where the content was written.

Two consequences are easy to overlook; both are also stated at
[`StageCx::resolve_value`](crate::core::run::StageCx::resolve_value):

- **A stage running again over a caught-up value reads a whole table from the
  catch-up's results whenever the catch-up computed that table again**, never entry by
  entry
  ([`ComputedView::with_overlay`](crate::core::data::store::ComputedView::with_overlay)).
  A stage that runs again over a value therefore sees the value's own entries in that
  table and nothing of the document's. A stage that must also work inside a value *and*
  needs data about the whole document reads a table that no per-tree stage of the
  catch-up writes — in practice, a table written once for the whole document. The
  `refs` feature's resolve stage is written that way.
- **A synthesized value states no fact about the document.** The store refuses a table
  piece classified as a fact about the document when it comes from a catch-up
  ([`StoreError::DocumentFactInValue`](crate::core::data::store::StoreError::DocumentFactInValue)),
  and the identifiers inside a value mean something only within the stage call that
  built it.

## Determinism

The output of a run depends on the document, the registry, the plan and the installed
external tables, and on nothing else. In particular it does not depend on how many
threads ran, or on the order in which work finished. Four things guarantee this, each
implemented in one place:

- **the batches**, computed once when the plan is built and executed in the same way by
  both runner paths: however the work is spread over threads, the results are collected
  in index order;
- **the fold**, [`fold_outs`](crate::core::data::store::fold_outs), shared by both
  paths: a stage's per-tree table pieces are absorbed in the run's order of the trees
  (the order in which the document's walk enters them), its tables installed in
  table-identifier order, and a table a stage declared but
  produced no table piece for still installs empty, so a read that returns nothing
  always means "no stage provided this" and never "a stage ran and found nothing";
- **one writer per table**, guaranteed by plan construction and checked again by the
  store;
- **scans in name order** during provider selection, so that the plan itself does not
  depend on the order in which the features registered.

The parallel path is compiled with the crate's `parallel` cargo feature, which is on by
default; the sequential path is always compiled, and it is the only one when the crate
is built without default features.
[`RunOptions::sequential`](crate::core::run::RunOptions::sequential) selects it
explicitly. The library's own tests check the two paths against each other over a whole
document: the same tables and the same output bytes.

## Where to go next

- [The document model](crate::guide::document_model) — the trees and tables that
  stages read and write.
- [Rendering: backends and outputs](crate::guide::rendering) — how the lowering and
  emit stages turn a frozen document into output.
- [Extending FLM: features and packages](crate::guide::extending) — writing stages,
  capabilities, tables and workflows of your own.
- [Concepts overview](crate::guide::concepts_overview) — the definitions of every term
  used here.
