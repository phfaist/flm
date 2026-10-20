# Rendering: backends and outputs

Rendering turns a [frozen](crate::guide::concepts_overview#frozen-trees-and-computed-data)
document into output in one output format: plain text, an HTML page, or LaTeX source.
Rendering is done by ordinary [stages](crate::core::stage::Stage), which a
[plan](crate::core::plan::Plan) schedules like any other stage. Each of these stages reads
[tables](crate::core::data::table::Table) and writes tables; none of them changes the document's trees. Everything rendering
produces is therefore computed data, held in tables beside the trees, and a caller reads the
result it wants from one of those tables.

This chapter serves two kinds of reader. A program that renders documents with the
backends this library ships finds here how to ask for an output, which table holds the
result, what the render options do, and what each backend writes for each element. An
author of an extension (a feature with lowering rules of its own, or a new backend) finds
here what a lowering rule is, declares and may do, the vocabulary of elements every backend
shares, and how a backend turns that vocabulary into an output.

The chapter covers, in order: a complete example; lowering and how a lowering rule is
chosen; the presentation vocabulary the rules build with; decorations; elements a backend
cannot render; the emit, regions and final stages; pieces, the document-level values a
backend places beside the content; outputs and artifacts; the plain-text backend and the
layout engine it prints with; the HTML backend; the rendered LaTeX backend, which writes
resolved content for LuaLaTeX; and the recomposed LaTeX backend, which writes the document
back as LaTeX source.

## Terms used on this page

These terms are used throughout the chapter, including the chapters on the HTML and the two
LaTeX backends. Each is defined here once, with a link to the item or the concept page that
defines it in full.

- A **backend** is the registration bundle for one output format: the elements its emitter
  writes natively, a lowering stage, an emit stage and a final stage
  ([`BackendDef`](crate::core::registry::BackendDef)). The library ships four backends:
  `text`, `html`, `latex-rendered` and `latex-recomposed`. The feature that registers a backend also
  registers two more stages of the backend, beside the `BackendDef`: its pieces stage and
  its regions stage (both described below).
- A backend's **native set** is the list of elements its emit stage writes directly,
  without a lowering rule ([`BackendDef::native`](crate::core::registry::BackendDef::native)).
- **Lowering** rewrites each frozen tree into a presentation tree for one backend, by
  applying the lowering rule selected for each element, in rounds, until only elements the
  backend writes natively remain; the result is computed data in the table
  `flm.lowered.<backend>` ([backends, lowering, and
  outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)).
- A **lowering rule** is a named registry entity written for one element: it declares what
  it requires, reads and produces, and implements `Lower`, which stages the element's
  presentation for one backend ([`LoweringRule`](crate::core::lowering::LoweringRule)). To
  **stage** a node is to build it into the tree that the rule is producing, through the
  context the rule is handed ([`Lowerer`](crate::core::lowering::Lowerer)).
- A **presentation element** is an element that never occurs in a source document: lowering
  rules stage nodes of it, and backends write it
  ([`FeatureBuilder::presentation_element`](crate::core::registry::FeatureBuilder::presentation_element)).
  The presentation elements, named
  `flm.pres.<name>`, form one vocabulary that every backend shares
  ([`crate::stdcore::backends::pres`]). A **presentation tree**, also called a **lowered
  tree**, is the result of lowering one tree: a tree of presentation elements and of
  elements in the backend's native set.
- An **emitter** is a backend's emit stage: it walks the lowered trees and writes the
  output's bytes into a table, from which the backend's final stage makes the artifact set
  ([backends, lowering, and
  outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)).
- An **output region** is one named part of an output's entry artifact (the document's
  content, each piece placed beside it, and on HTML the stylesheets, the scripts, the title
  and the language), which the backend writes once into the table
  `flm.output.<backend>.regions` ([`Regions`](crate::core::region::Regions)).
- A backend's **final stage** assembles the output's entry artifact from the output's
  regions, in a layout of the backend's own, and publishes the result as the output's
  artifact set; a host may instead assemble the same regions through a template of its own
  ([output regions and the final
  stage](crate::guide::concepts_overview#output-regions-and-the-final-stage)).
- An **artifact** is one file's worth of output held as a value (a folder artifact holds
  the files of a folder), identified by its producer and a name and never by a path; an
  **artifact set** is the artifacts of one output, at most one of which is the **entry
  artifact**, the one that represents the output as a whole (the HTML page, the text, the
  LaTeX document)
  ([`ArtifactSet`](crate::core::artifact::ArtifactSet)).
- An **output** is a name a caller asks for, such as `text`, registered with the
  [capability](crate::core::registry::CapabilityDef) a plan must reach to produce it and the table that holds its artifact set
  ([`OutputDef`](crate::core::registry::OutputDef)). A **workflow** is a named set of
  demands (capabilities the plan is asked to provide; see
  [`PlanRequest`](crate::core::plan::PlanRequest)) together with the choices that go with
  them; the `text` workflow lists the
  `text` output ([`WorkflowDef`](crate::core::registry::WorkflowDef)).
- A **synthesized value** is a small tree that a stage builds while a run is under way,
  held outside the document's trees ([synthesized
  values](crate::guide::concepts_overview#synthesized-values)).
- A **piece** is a document-level synthesized value that a backend places before or after
  the document's content (the title, the list of footnotes, the bibliography), built once
  by its builder stage into a table named for the piece and rendered by each backend with
  its own rules ([`Piece`](crate::core::piece::Piece)). On this page, "piece" means this
  and nothing else.
- A **render** is one run of a plan that produces outputs. A **render option** is a value
  of an option type set on the [plan request](crate::core::plan::PlanRequest) rather than
  on the document ([`Bind::Render`](crate::model::options::Bind::Render)); the render
  options are part of the plan's identity.
- A **host** is any program that embeds this library. The `flm` command-line program is
  one.
- The two LaTeX backends are two independent products. The **recomposed LaTeX**
  (`latex-recomposed`) writes the document back as LaTeX source close to what the author
  typed, and leaves numbering and cross-references to LaTeX. The **rendered LaTeX**
  (`latex-rendered`) writes content that FLM has resolved, through one `\flmx…` command or
  environment per presentation element, for LuaLaTeX to lay out on pages. They share only
  the LaTeX family's library mechanisms ([Two LaTeX backends: rendered and
  recomposed](#two-latex-backends-rendered-and-recomposed)).
- A backend's **contract** is what its output promises to the code that styles it: on HTML,
  the classes and attributes a stylesheet may select ([the HTML
  contract](#the-html-contract)); on the rendered LaTeX, the commands, environments, keys,
  roles and kind tokens a LaTeX template may redefine ([the rendered LaTeX
  contract](#the-rendered-latex-contract)). Each contract is written down once, as data,
  one row per presentation element.

A caller never asks for a backend's stages by name. It asks for an output (`text`), or for
a workflow that lists outputs (the `text` workflow), and plan construction works out which
stages must run, in which order, together with everything they depend on.

For each document, a backend's stages run as follows. The stage names given are those of
the plain-text backend; the other three backends follow the same sequence with stages of
their own, the HTML backend adds two more (`flm.html.emit-pieces` and
`flm.html.resolve-definitions`), and the rendered LaTeX backend one more
(`flm.latex-rendered.emit-pieces`). Steps 1 and 2 do not depend on each other; step 3 reads
the results of both, and steps 4 and 5 follow in order:

1. The **lowering stage** (`flm.text.lower`) runs once per document tree and writes the
   lowered trees into `flm.lowered.text` ([Lowering](#lowering)).
2. The **pieces stage** (`flm.text.pieces`) lowers the pieces the render places into
   `flm.lowered.text.pieces` ([Pieces](#pieces)).
3. The **emit stage** (`flm.text.emit`) writes the output's bytes into a table of the
   backend's own, `flm.output.text`.
4. The **regions stage** (`flm.text.regions`) writes the output's regions into
   `flm.output.text.regions`.
5. The **final stage** (`flm.text.final`) assembles the entry artifact from the regions and
   writes the output's artifact set into `flm.output.text.document`, the `text` output's
   table.

Steps 3 to 5 are described in [the emit stage and the final
stage](#the-emit-stage-and-the-final-stage).

## Rendering a document, end to end

This example renders a document as plain text through the public entry points: it builds
a registry, parses one fragment, builds the plan of the `text` workflow, runs it, and
reads the printed text.

```rust
use std::sync::Arc;

use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{FlmOrigin, ParseOptions, parse_fragment};
use flm_core::stdcore::backends::text::{self, TEXT_OUTPUT};
use flm_core::stdcore::{self, StdcoreConfig};
use flm_core::techy::source::Source;

// A registry holding the standard package, and nothing else.
let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;

// A document of one parsed fragment.
let source = Arc::new(
    Source::new("\\section{Title}\n\nSome \\emph{emphasized} text.\n")
        .with_origin(FlmOrigin::none()),
);
let fragment =
    parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));
let doc = Arc::new(Doc::new(content));

// The plan of the `text` workflow, and one run of it over that document.
let plan = Arc::new(Plan::for_workflow(&registry, text::WORKFLOW, &Overrides::none())?);
let mut run = Run::new(doc, plan, Externals::none())?;
run.execute(&RunOptions::new())?;

// The printed text is a table the run wrote.
let output = run.table(TEXT_OUTPUT)?.expect("the `text` workflow writes it");
assert_eq!(output.text, "Title\n=====\n\nSome emphasized text.\n");
# Ok::<(), Box<dyn std::error::Error>>(())
```

A caller always reads a result this way: a backend's stages write tables, and the caller
reads the table that holds what it wants. The plain-text backend writes the printed text
twice: as a [`TextOutput`](crate::stdcore::backends::text::TextOutput), with the position
of every printed element, in
[`TEXT_OUTPUT`](crate::stdcore::backends::text::TEXT_OUTPUT) (`flm.output.text`), and as
the `text` output's artifact set in
[`TEXT_DOCUMENT`](crate::stdcore::backends::text::TEXT_DOCUMENT)
(`flm.output.text.document`). Besides its output, the `text` workflow demands the
[collection](crate::guide::concepts_overview#stages-capabilities-and-plans) (a named set of
capabilities that any feature may join) [`flm.validated`](crate::stdcore::VALIDATED), which
every check of the standard package joins, so a render also runs every check of the
document
([`TextBackend`](crate::stdcore::backends::text::TextBackend)).

## Lowering

Lowering rewrites one frozen tree into one presentation tree for one backend. Where the
frozen tree holds a paragraph, the presentation tree holds a `flm.pres.par` node; where it
holds a heading, a `flm.pres.block-heading` node; where it holds a cross-reference, a
`flm.pres.link` node around the content the reference displays. The frozen tree itself is
never changed.

Each backend has one lowering stage,
[`LoweringStage`](crate::stdcore::backends::lowering::LoweringStage), named
`flm.<backend>.lower`, which runs once per document tree. It writes its result into the
table `flm.lowered.<backend>` as a
[`LoweredTrees`](crate::stdcore::backends::lowering::LoweredTrees) value, which holds one
[`LoweredTree`](crate::stdcore::backends::lowering::LoweredTree) per document tree: the
lowered tree itself, the requirements its rules recorded (identifiers of the definitions
its output needs, below), and the number of rounds lowering ran over it. The work on one
tree is done by the **lowering driver**, the function
[`lower_tree`](crate::stdcore::backends::lowering::lower_tree): it walks the tree and calls
the rules.

Each element's part of that rewrite is done by a lowering rule
([`LoweringRule`](crate::core::lowering::LoweringRule)), which pairs an element with an
implementation of the trait [`Lower`](crate::core::lowering::Lower). A rule is written once
for its element and serves every backend that needs it: the `headings` feature's rule is
written for the element `flm.heading`, not for plain text or for HTML. A feature registers
a rule with
[`FeatureBuilder::lowering_rule`](crate::core::registry::FeatureBuilder::lowering_rule).

### What a rule does at a node

The lowering driver walks the tree from the root down, parents before children, and
handles each node in one of three ways:

1. A node whose element the backend writes natively under this plan (its native set, less
   any element for which the plan request prefers a rule) is copied, and its children are
   lowered. An element whose content is source text carries the
   [`OpaqueContent`](crate::stdcore::backends::pres::OpaqueContent) aspect
   (`flm.pres.math-passthrough` and `flm.pres.verbatim` are the two elements of the vocabulary
   that carry it): the driver copies its subtree unchanged and lowers nothing inside it
   except a node carrying `ContentInOpaque`, no element inside it counts toward the rounds,
   and the emitter writes it byte for byte. The recomposed LaTeX is the exception for
   mathematics: the rules of `math-passthrough` for that backend write a formula from its
   nodes, so that a user definition used inside it is handled ([Mathematics in the
   recomposed LaTeX](#mathematics-in-the-recomposed-latex)).
2. Otherwise, a node whose element has a rule selected by the plan is handed to that rule:
   the driver calls [`Lower::lower`](crate::core::lowering::Lower::lower) with the node.
3. Otherwise, the node is replaced by a childless `flm.pres.unsupported` node, and an Error
   is reported ([Accepted unrenderable elements](#accepted-unrenderable-elements)).

A rule returns one of two values ([`Lowering`](crate::core::lowering::Lowering)):

- [`Lowering::Default`](crate::core::lowering::Lowering::Default): copy the node unchanged
  and lower its children. This is the right value for an element the backend writes
  natively. For an element the backend does not write natively, the copy still holds that
  element, so the same rule is called again in the next round, until lowering has used up
  the rounds it is allowed and the tree fails ([Rounds](#rounds)).
- [`Lowering::Emit`](crate::core::lowering::Lowering::Emit): the nodes the rule staged to
  replace the node, in their order among its siblings. An empty list removes the node.

A rule has no direct access to the [run](crate::core::run::Run). Everything it reads, and every node it builds,
goes through the [`Lowerer`](crate::core::lowering::Lowerer) it is handed, together with a
[`LowerCx`](crate::core::lowering::LowerCx), the tree under construction into which it
stages nodes. The `Lowerer` offers five groups of methods:

- **Information about the node and the run**: [`rule`](crate::core::lowering::Lowerer::rule)
  and [`backend`](crate::core::lowering::Lowerer::backend) return the names of the rule and
  the backend; [`addr`](crate::core::lowering::Lowerer::addr),
  [`addr_of`](crate::core::lowering::Lowerer::addr_of) and
  [`tree_id`](crate::core::lowering::Lowerer::tree_id) return the addresses that computed
  data is keyed by; [`placement`](crate::core::lowering::Lowerer::placement) returns the
  tree the node places, if any; [`layout`](crate::core::lowering::Lowerer::layout),
  [`lang`](crate::core::lowering::Lowerer::lang) and
  [`comment_start`](crate::core::lowering::Lowerer::comment_start) return the region
  layout, the language and the comment prefix the registry was built with.
- **Lowering parts of the node** through the rules:
  [`lower_body`](crate::core::lowering::Lowerer::lower_body),
  [`lower_arg`](crate::core::lowering::Lowerer::lower_arg),
  [`lower_arg_named`](crate::core::lowering::Lowerer::lower_arg_named),
  [`lower_children`](crate::core::lowering::Lowerer::lower_children) and
  [`lower_nodes`](crate::core::lowering::Lowerer::lower_nodes) lower part of the node and
  return the nodes it became; [`default`](crate::core::lowering::Lowerer::default) makes
  the copy that `Lowering::Default` asks for, for a rule that returns the copy as one part
  of a larger result.
- **Building nodes**: `pres_with` stages a presentation node carrying its typed payload;
  `pres_slots_with` stages one whose content is several named content slots, such as a
  list's `label` and `item` pairs; [`text`](crate::core::lowering::Lowerer::text) stages a
  text node.
- **Showing content from elsewhere**: [`splice`](crate::core::lowering::Lowerer::splice)
  lowers a synthesized value where the rule's node is placed, after bringing it up to date
  ([value catch-up](crate::guide::concepts_overview#value-catch-up));
  [`lower_region`](crate::core::lowering::Lowerer::lower_region) lowers an argument or a
  content slot of a node of another tree, where the rule's node is placed, as content of
  that node's own tree (the [content
  pointers](crate::guide::concepts_overview#content-pointers) of the xnotes, defterm and
  theorems features are built on it).
- **Reading and reporting**: `get` reads a table the rule declared, `option` the value of
  an [option type](crate::model::options) it declared, and
  [`producer_of`](crate::core::lowering::Lowerer::producer_of) returns the name of the
  stage the plan scheduled to write a table, for a message about data the rule expected and
  did not find; [`require`](crate::core::lowering::Lowerer::require) records a requirement
  of the tree being lowered (below), and
  [`diagnose`](crate::core::lowering::Lowerer::diagnose), with the `error`, `warning` and
  `note` methods, reports a diagnostic at the node being lowered.

Every read goes through the `Lowerer` so that it can be checked against what the rule
declared: a read of a table or an option type the rule did not declare fails. The methods
that take a type parameter (`get`, `option`, `pres_with`, `pres_slots_with`, `error`,
`warning`, `note`) cannot be methods of the trait, because a trait used as a trait object
cannot have type parameters. They are defined on `dyn Lowerer`, the form in which a rule
receives the interface, and are listed on the [`Lowerer`](crate::core::lowering::Lowerer)
page under its `impl dyn Lowerer` block. This page writes them without a link, because
rustdoc has no link syntax for a method of a trait object.

These functions of [`crate::stdcore::backends::lowering`] lower or copy content that a rule
has chosen itself: a run of sibling nodes (the content of one argument, the children of one
node, a single node) or one argument of the node:

- [`lower_run`](crate::stdcore::backends::lowering::lower_run) lowers such a run of nodes
  through the rules, so that each node becomes what its own rule stages for it.
- [`copy_run_verbatim`](crate::stdcore::backends::lowering::copy_run_verbatim) and
  [`copy_subtree_verbatim`](crate::stdcore::backends::lowering::copy_subtree_verbatim)
  copy the run of nodes or one subtree unchanged. A rule uses them to put the source of an
  [opaque region](crate::guide::concepts_overview#opaque-regions), such as a formula,
  into a presentation node.
- [`stage_verbatim_content`](crate::stdcore::backends::lowering::stage_verbatim_content)
  stages the content of a [verbatim
  region](crate::guide::concepts_overview#verbatim-regions); every rule that stages
  verbatim content calls it, the default rule of a bare verbatim group included. It reads
  the content as characters first. Content that a user definition's template put there and
  that is not characters (a command, a formula) is reported as
  [`ContentNotCharacters`](crate::stdcore::backends::lowering::ContentNotCharacters) at
  the command or environment that owns the region, and the region then shows that
  content's source text.
- [`lower_arg_or_nothing`](crate::stdcore::backends::lowering::lower_arg_or_nothing) and
  [`lower_arg_named_or_nothing`](crate::stdcore::backends::lowering::lower_arg_named_or_nothing)
  lower a mandatory argument and produce nothing where a recovered parse error left the
  argument absent, so that the parse's own report stays the only one.

The `headings` feature's rule
([`HeadingRule`](crate::stdcore::features::headings::HeadingRule)) is a short example of
what a rule does. It reads the heading's level from its element, reads the heading's
anchor from the anchors table and its number from the counter table, lowers the title
argument, and stages one `flm.pres.block-heading` node carrying the level and the anchor,
over the number and the title. It reads both tables as *optional* reads, because neither
table belongs to the `headings` feature. Where nothing computes anchors (a registry
without the `refs` feature, or a plan that resolves no reference), the heading is written
with no anchor and nothing is reported at it. Where nothing numbers it (a registry without
the `numbering` feature, or a headings configuration that numbers no heading kind), it is
written with no number.

### What a rule declares

Plan construction reads a rule's [`LoweringDecl`](crate::core::lowering::LoweringDecl)
before any rule runs, so a rule takes part in scheduling without having run. The
declaration has six fields. Four of them (`requires`, `reads`, `reads_opt` and `options`)
say what the rule needs; `produces` lists what it may stage; `selection_requires` lists
the requirements it records just by being selected:

- `requires`, `reads`, `reads_opt` and the option types the rule reads are added to the
  needs of its backend's lowering stage ([`rules_needs`](crate::stdcore::backends::lowering::rules_needs)).
  A rule that reads the references table therefore causes the stage that writes that table
  to be scheduled, and a plan whose backend lowers paragraphs runs the block-structure
  rewrite, because the paragraph rule requires the capability `flm.blocks.structured`.
- `reads_opt` states that the rule can do its work without the feature that owns the
  table. It does not state that the rule would rather not have the values: where the
  registry declares the table and some stage writes it, the lowering stage turns the
  optional read into a required read, so that the stage writing the table is scheduled and
  the rule finds the values. An optional read stays optional in two cases. A read of a table that no
  stage writes (an [external table](crate::core::run::Externals), which the caller
  installs when the run is constructed) finds the table where the caller installed it, and
  nothing otherwise. A read of a table that no feature of the registry declares (a [peer
  declaration](crate::core::registry::PeerName) that resolved to nothing) always finds
  nothing.
- `produces` lists every presentation element the rule may stage. Plan construction
  follows these lists to check that the backend can, in the end, write every element a
  document may contain ([Accepted unrenderable elements](#accepted-unrenderable-elements)),
  and the `Lowerer` refuses to stage an element the rule did not list
  ([`LowerError::UndeclaredProduces`](crate::core::lowering::LowerError::UndeclaredProduces)).
  The one exception is text: a rule may stage a text node without declaring `flm.text`
  when the backend's native set holds `flm.text`.
- `selection_requires` lists **requirements** the rule has merely because it was selected,
  whether or not it ever runs at a node. A requirement is an identifier a lowering rule or
  an emitter records to say that its output needs a definition: a style or script
  definition on HTML, a package request or a definition piece on LaTeX
  ([`StyleDef`](crate::core::registry::StyleDef)). These identifiers are added to the
  requirements of every lowered tree, beside those a rule records at one node with
  [`Lowerer::require`](crate::core::lowering::Lowerer::require). The backend that writes
  the output resolves them: the HTML backend turns them into the stylesheets and scripts of
  the page ([Style and script
  definitions](#style-and-script-definitions)), and the recomposed LaTeX
  into the packages and definitions of its style file. The plain-text backend has no use
  for a definition: it records the requirements in the lowered-trees table, where a host
  can read them, and reads none itself.

### Choosing a rule per backend

Several rules may target one element: the rule of the feature that declares the element,
and a more specific rule someone wrote for a particular backend. The plan chooses at most
one per backend and element, in this order, the **rule selection order**
([`SelectedRules`](crate::core::plan::SelectedRules)):

1. a rule the [plan request](crate::core::plan::PlanRequest) (everything a caller says
   about what it wants of a plan) prefers by name
   ([`LoweringSelection`](crate::core::registry::LoweringSelection)); a preference applies
   even where the backend would otherwise write that element natively;
2. otherwise no rule at all, if the element is in the backend's native set;
3. otherwise a rule marked [`DefaultMark::OnBackends`](crate::core::lowering::DefaultMark::OnBackends)
   with that backend;
4. otherwise a rule marked [`DefaultMark::Everywhere`](crate::core::lowering::DefaultMark::Everywhere);
5. otherwise the only candidate, if there is exactly one.

Two kinds of rule are never candidates, regardless of the selection order: a rule the
registry has [set aside](crate::core::registry::RuleEntry::set_aside) (excluded from rule
selection because a peer declaration it depends on resolved to nothing; see
[`PeerName`](crate::core::registry::PeerName)), and a rule marked
[`DefaultMark::OnBackends`](crate::core::lowering::DefaultMark::OnBackends) with other
backends than this one. A rule marked `OnBackends` with other backends is excluded because
the mark states where the rule belongs: such a rule stages that backend's own elements. Neither kind of rule is chosen even as the only rule
for its element, nor when the plan request prefers it by name; the preference is reported
instead (below). A rule marked [`DefaultMark::Never`](crate::core::lowering::DefaultMark::Never)
is different: that mark restricts precedence and not place, so such a rule is still chosen
when it is the only candidate.

Several candidates with nothing to choose between them fail the plan with
[`PlanError::RuleContest`](crate::core::plan::PlanError::RuleContest), which lists the
candidates. No candidate at all leaves the element without a rule; whether that is
acceptable is decided by the completeness check described under [accepted unrenderable
elements](#accepted-unrenderable-elements). A feature that ships only a rule marked for one
backend is therefore incomplete on the other backends, and that check reports it.

A preference the plan cannot honor is never dropped without a report. The plan keeps it as
a [`PlanWarning`](crate::core::plan::PlanWarning), and the runner reports each one as a
Warning before the run's own diagnostics:

- `flm.run.preferred-rule-set-aside`: the preferred rule is set aside; the warning lists
  the peer declarations that did not resolve.
- `flm.run.preferred-rule-not-candidate`: the preferred rule is marked for other backends
  only.
- `flm.run.ignored-preference`: a preferred *stage* could not be scheduled, because
  scheduling another stage that provides a capability in common ruled it out.

The result of rule selection is one `SelectedRules` value per backend. The lowering stage
reads it to compute its needs, and the lowering driver reads it at every node.

### Rounds

A rule may stage nodes of elements that themselves need lowering, so one pass over the tree
is not always enough. Lowering therefore runs in **rounds**: each round is one pass over
the tree, and another round follows while the tree still holds an element outside the
backend's native set. The number of rounds allowed is
[`rank`](crate::stdcore::backends::lowering::rank), computed from the plan's rule
selection: the length of the longest chain of rule applications from any element a
document may contain to the backend's native set, following what each selected rule
declares in `produces`.

A tree that still holds a non-native element after the last allowed round fails: it is
reported as `flm.lowering.rounds-exceeded`
([`RoundsExceeded`](crate::stdcore::backends::lowering::RoundsExceeded)), which lists the
elements left. Two mistakes of a rule lead there: a `produces` list that omits an element
the rule stages, and `Lowering::Default` returned for an element the backend does not
write natively.

### When a rule fails

Lowering reports every problem of a document as a diagnostic. It neither panics nor
returns an error to the caller because of the document, so a malformed document still
produces a run. The only panic is for a plan run against a registry other than the one it
was built from ([`LoweringStage`](crate::stdcore::backends::lowering::LoweringStage)). A
failure has one of two extents
([`crate::stdcore::backends::lowering::error`]):

- **One node.** The node is replaced by a childless `flm.pres.unsupported` node, and one
  Error is reported at the node; the rest of the tree is lowered normally. This applies to
  a rule that failed
  ([`RuleFailed`](crate::stdcore::backends::lowering::RuleFailed)), to a rule that refused
  the node with a diagnostic of its own
  ([`LowerError::Refused`](crate::core::lowering::LowerError::Refused), whose diagnostic is
  reported instead), to a rule that asked for a region of another node that does not exist
  ([`RegionNotFound`](crate::stdcore::backends::lowering::RegionNotFound)), to an element
  the backend cannot render that the plan request accepted
  ([`UnsupportedElement`](crate::stdcore::backends::lowering::UnsupportedElement)), and to
  an element that nothing lowers at all
  ([`NoRule`](crate::stdcore::backends::lowering::NoRule)).
- **The whole tree.** The tree gets no entry in the lowered-trees table, so nothing is
  written for it, and one Error at its root gives the reason:
  [`RoundsExceeded`](crate::stdcore::backends::lowering::RoundsExceeded),
  [`LoweringFailed`](crate::stdcore::backends::lowering::LoweringFailed) (a node could not
  be staged at all, or the lowered tree could not be written into the backend's table), or
  [`NestingTooDeep`](crate::stdcore::backends::lowering::NestingTooDeep) (the tree nests
  more deeply than the recursion guard allows).

[`LowerError`](crate::core::lowering::LowerError) lists which failure of a rule leads to
which extent. A rule must return every error the `Lowerer` gives it, rather than continue:
after a refused call, the `Lowerer` has staged nothing.

### Reports over content shown again

Some content is shown again away from where it is written: a heading's title that a
reference prints, a [kept
definition](crate::stdcore::features::userdefs::kept)'s template (a user definition left in
the tree for the recomposed LaTeX) staged at each use site, a list's own
label template. Such content is brought up to date and lowered at every place it is shown,
and every diagnostic its lowering raises is reported at each of those places: an
unresolved reference in a heading's title is reported where the heading is written, and
once more at every reference that prints the heading's title.

Each such report keeps the span where the content was written. Every report made while a
synthesized value is [spliced](crate::core::lowering::Lowerer::splice) gains one frame in
its traceback ([`Diagnostic::frames`](techy::error::Diagnostic::frames)), titled `a copy of
this content, shown here`, whose span is the node being lowered when the value is spliced:
the reference, the use site, or the whole list for its items' labels. When a copy is shown
inside a copy, the frames run from the innermost outward. A rendered diagnostic lists
these frames under `Inside:`.

No report is filtered out, including repeated ones. A program that displays the
diagnostics merges duplicates if it wants to.

## The presentation vocabulary

The presentation elements form one vocabulary that every backend shares
([`crate::stdcore::backends::pres`]). Because every backend reads the same elements, a
lowering rule written once serves all of them, and a format needs a rule of its own only
where it does something different. The vocabulary has no general-purpose element that can
carry anything: a feature whose content none of these elements can carry registers a
presentation element of its own, with its own payload type
([`FeatureBuilder::presentation_element`](crate::core::registry::FeatureBuilder::presentation_element);
see [Extending FLM](crate::guide::extending#lowering-rules)).

The elements are listed below; each is named `flm.pres.<name>`. A node's **body** is the
run of nodes it contains; a **content slot** is a named group of a node's children, of
which the body is one ([`crate::stdcore::backends::pres::read`]).

| element | what it is |
|---|---|
| [`PAR`](crate::stdcore::backends::pres::PAR) | a paragraph; its body is a run of inline nodes |
| [`BLOCK`](crate::stdcore::backends::pres::BLOCK) | a block container with a role and kind tokens (below); its body is a run of blocks |
| [`BLOCK_HEADING`](crate::stdcore::backends::pres::BLOCK_HEADING) | a heading placed between blocks, with its heading kind's name, its level and whether it is in the table of contents |
| [`RUNIN_HEADING`](crate::stdcore::backends::pres::RUNIN_HEADING) | the title of a run-in heading, inside the run-in decoration that leads its paragraph, with the same payload as a block heading |
| [`SPAN`](crate::stdcore::backends::pres::SPAN) | inline content with a display style, a role and kind tokens |
| [`LIST`](crate::stdcore::backends::pres::LIST) | a list: one node holding a `label` and an `item` content slot per entry, read by position because the names repeat; its kind tokens include at most one of `numbered`, `unnumbered` and `terms`, which say what shape of list it is, and `labeled` when every entry carries its own label ([`LIST_RESERVED_KINDS`](crate::stdcore::backends::pres::LIST_RESERVED_KINDS)) |
| [`CELLS`](crate::stdcore::backends::pres::CELLS) | a table of cells: one node whose payload is the table's grid, holding one `cell` content slot per cell, in the order of the payload's entries |
| [`LINES`](crate::stdcore::backends::pres::LINES) / [`LINE`](crate::stdcore::backends::pres::LINE) | a run of lines, each with its own indentation and alignment |
| [`VERBATIM`](crate::stdcore::backends::pres::VERBATIM) | byte-exact content of a [`VerbatimKind`](crate::stdcore::backends::pres::VerbatimKind) (`text` for characters set as they are, `code` for computer code, `a` for an identifier), inline or as a block, with an optional language name |
| [`MATH_PASSTHROUGH`](crate::stdcore::backends::pres::MATH_PASSTHROUGH) | mathematics passed on as source text: its body is the original content of the formula |
| [`MATH_TEXT`](crate::stdcore::backends::pres::MATH_TEXT) | content inside a formula: the argument of a text command such as `\text`, or, holding nothing, a link target placed in the formula |
| [`LINK`](crate::stdcore::backends::pres::LINK) | a link around inline content |
| [`IMAGE`](crate::stdcore::backends::pres::IMAGE) | an image; it has no content |
| [`UNSUPPORTED`](crate::stdcore::backends::pres::UNSUPPORTED) | the childless node that replaces something the backend could not render |
| [`DROPPED`](crate::stdcore::backends::pres::DROPPED) | the childless **dropped mark** a lowering rule stages where it left out something it could not place, with the condition of the diagnostic it reported and a short statement of what was left out |
| [`UNKNOWN`](crate::stdcore::backends::pres::UNKNOWN) | a macro or environment that no feature declares, carried into the output as the content the author wrote |
| [`DECO_RUNIN`](crate::stdcore::backends::pres::DECO_RUNIN), [`DECO_FINAL`](crate::stdcore::backends::pres::DECO_FINAL), [`DECO_MARGIN`](crate::stdcore::backends::pres::DECO_MARGIN) | the three decoration elements ([Decorations](#decorations)) |
| [`DECO_MARK`](crate::stdcore::backends::pres::DECO_MARK) | the **decoration mark**, left where a decoration was moved out of a content slot ([Decorations](#decorations)) |
| [`SUBTREE`](crate::stdcore::backends::pres::SUBTREE) | the place where another document tree is placed ([Trees placed inside other trees](#trees-placed-inside-other-trees)) |

[`ALL`](crate::stdcore::backends::pres::ALL) lists them in registration order. The
`lowering` feature ([`LoweringFeature`](crate::stdcore::backends::lowering::LoweringFeature))
registers the whole vocabulary, so a registry that renders anything holds it once.

### Typed payloads

Everything a presentation node states beyond its element and its children is its
**payload**: one typed value per node, of the type its element declares. A node carries no
untyped collection of properties and no handler per backend;
[`crate::stdcore::backends::pres::payload`] lists every payload type. Some examples:

- A `flm.pres.span` carries a [`SpanPayload`](crate::stdcore::backends::pres::SpanPayload):
  its [`SpanStyle`](crate::stdcore::backends::pres::SpanStyle) (emphasis, bold,
  superscript, …), its role, its kind tokens and its anchor field.
- A `flm.pres.link` carries a [`LinkPayload`](crate::stdcore::backends::pres::LinkPayload):
  its `href`, which is `#anchor` for a target inside the document and an address
  otherwise; its link kind (`ref`, `cite`, `url`, `term`); its optional `label`, the
  link's **accessible name**: the text that assistive technology, such as a screen reader,
  speaks in place of the link's content; and
  [`shows_target`](crate::stdcore::backends::pres::LinkPayload::shows_target), which says
  whether the link's content is the target's own display rather than words an author wrote.
  The feature that stages the link states it, and it is true for a `\url` and an `\email`,
  whose content is derived from the address.
- A `flm.pres.line` carries a [`LinePayload`](crate::stdcore::backends::pres::LinePayload):
  its indentation from the left and from the right, each a length (below), and its
  [`LineAlign`](crate::stdcore::backends::pres::LineAlign).
- A `flm.pres.math-text` carries a
  [`MathTextPayload`](crate::stdcore::backends::pres::MathTextPayload): the text command
  its content was written in, whether it is an equation line's tag (`at_tag`), and its
  anchor field.
- A `flm.pres.cells` carries a [`CellsPayload`](crate::stdcore::backends::pres::CellsPayload),
  the table's grid: its number of rows and columns, and one
  [`CellSpec`](crate::stdcore::backends::pres::CellSpec) per cell, in row-major order of
  the cells' top-left positions. A cell's entry gives its top-left position counted from
  zero, its row and column spans, its alignment, whether it heads its column or its row,
  the edges at which a rule is drawn, and the custom cell styles it carries by name; the
  content of entry *i* is in the node's content slot *i*. The method
  [`grid`](crate::stdcore::backends::pres::CellsPayload::grid) states what occupies every
  position of the grid (nothing, a cell's top-left corner, or a cell's span), and every
  backend reads the grid through it rather than working it out again.
- A `flm.pres.dropped` carries a
  [`DroppedPayload`](crate::stdcore::backends::pres::DroppedPayload): the condition of the
  diagnostic reported for what was left out, and a short statement of it.

Three conventions apply to every payload:

- The payload's **anchor field** is a plain `String`: the identifier a link points at for
  this node, as the `refs` feature's anchors stage computed it
  ([`ANCHORS`](crate::stdcore::features::refs::ANCHORS)), not a reference still to be
  resolved.
- A **role** is a dotted name, owned by a feature, that says what a container is (a
  theorem's heading, a quotation's attribution), and **kind tokens** are names, owned by
  the feature, that classify it further, so that a stylesheet or a host's lowering rule can
  select on them ([decorations, roles and
  kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)). A role is a dotted
  [`Name`](crate::model::names::Name) whose first segment is the owning feature, so the
  roles of two features do not collide. Kind tokens are `Name`s too. Four of them are
  reserved by the vocabulary and shared by every feature that stages a list: `numbered`,
  `unnumbered`, `terms` and `labeled`
  ([`LIST_RESERVED_KINDS`](crate::stdcore::backends::pres::LIST_RESERVED_KINDS)).
- A **document role** ([`DocRole`](crate::stdcore::backends::pres::DocRole)) is one of a
  small fixed set of structural roles in a document (a list of notes, a note, a link to a
  note, a link back from one, a list of cited works, a link to a cited work, a table of
  contents), which a backend with such a notion renders.

A **length** ([`Length`](crate::stdcore::backends::pres::Length)) is how a payload states
a dimension: an exact amount and a unit, written `1tab`, `1.5em` or `10pt`, never a bare
number. The amount is a fixed-point number with sixteen fractional bits (one unit of the
fraction is 1/65536 of the unit, as in TeX), so that equal lengths compare and hash alike
and the output written from them is the same on every platform. The units form a closed
set ([`LengthUnit`](crate::stdcore::backends::pres::LengthUnit)):

- `em`, the width of an em in the current font;
- `pt`, `mm`, `cm` and `in`, the units of fixed size, the point being 1/72 inch, the point
  of CSS and PostScript;
- `tab`, whose width each backend sets through one setting of its own: the plain-text
  backend's render option `tab_width`, the HTML backend's custom property `--flm-tab`, and
  the LaTeX length `\flmTabWidth`. A line of a quotation indented by one `\indent` is
  indented by one tab.

**An emitter that cannot express a field of a payload ignores it.** Plain text cannot show
a bold span or a block's role, so the plain-text backend drops them. A detail of
presentation may thus be dropped in a given output format; content never is.

A rule stages a node with `pres_with`, which takes the payload by value:

```rust
# use flm_core::core::lowering::{LowerCx, LowerError, Lowerer, Lowering};
# use flm_core::model::names::ElementName;
# use flm_core::lang::Flm;
use flm_core::stdcore::backends::pres::{self, SpanPayload, SpanStyle};
use flm_core::techy::core::node::NodeRef;

fn lower_emphasis(
    node: NodeRef<'_, Flm>,
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
) -> Result<Lowering, LowerError> {
    let body = lw.lower_arg(cx, &node, 0)?;
    let payload = SpanPayload { style: Some(SpanStyle::Emphasis), ..SpanPayload::default() };
    let span = lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)?;
    Ok(Lowering::Emit(vec![span]))
}
# let _ = lower_emphasis;
```

Every payload type also has a stable serialized form
([`PresPayload::project`](crate::lang::PresPayload::project)), made only of names, numbers
and strings and never of spans or node identifiers, so a lowered tree can be written out
and read back without a tree to resolve identifiers against.
[`payload_readers`](crate::stdcore::backends::pres::payload_readers) supplies the
functions that read the payloads back.

### Reading a presentation tree

An emitter walks a lowered tree with the functions of
[`crate::stdcore::backends::pres::read`]:

- [`is_presentation`](crate::stdcore::backends::pres::is_presentation) tests whether a node
  is a presentation node;
- [`payload_of`](crate::stdcore::backends::pres::payload_of) returns a node's typed
  payload, and [`body_of`](crate::stdcore::backends::pres::body_of) its body;
- [`list_entries`](crate::stdcore::backends::pres::list_entries) returns a list's
  label and item pairs, and [`cells_of`](crate::stdcore::backends::pres::cells_of) a
  table of cells' payload with each cell's content;
- [`plain_text`](crate::stdcore::backends::pres::plain_text) returns the characters of a run
  of nodes with every construct dropped; the test that decides whether a run-in title gets a stop
  ([`ends_in_sentence_punctuation`](crate::stdcore::backends::pres::ends_in_sentence_punctuation))
  is built on it;
- [`recompose_source`](crate::stdcore::backends::pres::recompose_source) returns the source
  text of a run of nodes: byte for byte for a run parsed from one source, and with a space
  kept between a control word and a letter that follows it from another source. An emitter
  prints opaque content (mathematics, verbatim content) from it.

An emitter that prints the content of an opaque node as source must expect one kind of
node inside it that is not source: a node whose element carries the
[`ContentInOpaque`](crate::stdcore::backends::pres::ContentInOpaque) aspect, whose body is
ordinary content that the lowering rules have been through. `flm.pres.math-text` is the
one element of the vocabulary that carries this aspect, and it occurs among the direct
children of a `flm.pres.math-passthrough` body. The emitter writes the source around it and its
content itself. The plain-text backend writes `\command{…}` around the content, printed on
one line, and nothing at all for a node with no content.

## Decorations

A **decoration** is a node whose element carries the
[`Decoration`](crate::stdcore::backends::pres::Decoration) aspect (a run-in heading, a
margin note, a closing mark): when a rule stages the presentation node that contains it,
the lowering driver moves it out of the content slot into an attached slot of that node and
leaves a `flm.pres.deco-mark` node where it was ([decorations, roles and
kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)). A decoration is
content attached to a block rather than part of the block's ordinary content: a theorem's
heading (`Theorem.` before the first word), an end-of-proof mark closing the block, a note
in the margin beside it. The vocabulary has three decoration elements, `flm.pres.deco.runin`,
`flm.pres.deco.final` and `flm.pres.deco.margin`
([`DECORATIONS`](crate::stdcore::backends::pres::DECORATIONS)).

A rule stages a decoration node as an ordinary child of the presentation node it belongs
to. When the rule stages that presentation node (with `pres_with` or `pres_slots_with`),
the lowering driver does the same thing for every rule: it moves each decoration found
among the direct children of a content slot into an **attached slot** of the node, named
after the decoration's element, and stages in its place a childless **decoration mark**, a
`flm.pres.deco-mark` node whose
[`DecoMarkPayload`](crate::stdcore::backends::pres::DecoMarkPayload) holds the position of
that attached slot. The attached slots follow the content slots. An emitter writes the
decoration at the mark's position: the emitter decides how the decoration is set, while the
tree keeps it apart from the block's ordinary content.
[`attached_slots`](crate::stdcore::backends::pres::attached_slots) lists a node's attached
slots, and [`decoration_at`](crate::stdcore::backends::pres::decoration_at) returns the
decoration a mark refers to.

The driver recognizes a decoration by its element's `Decoration`
[aspect](crate::guide::concepts_overview#elements-and-aspects), not by the list of three
names above, so a decoration element that a feature registers itself is handled exactly as
the standard ones are.

The decoration mark is staged by the driver itself, so a rule does not declare
`flm.pres.deco-mark` in `produces`. A backend whose native set holds any decoration element
must therefore hold `flm.pres.deco-mark` too. The `lowering` feature declares this as a
[native-set implication](crate::core::registry::NativeImplication)
([`native_implications`](crate::stdcore::backends::lowering::native_implications)), a
requirement on native sets that the registry build checks: a backend missing the mark is
refused when the registry is built, before any document is lowered. The decoration element
itself is not special in that respect: the rule that stages one lists it in `produces`,
like everything else it stages.

Four standard features stage decorations:

- The [`headings`](crate::stdcore::features::headings) feature stages a
  `flm.pres.deco.runin` for a heading of a run-in heading kind: a heading set at the start
  of the paragraph it introduces rather than on a line of its own. A heading kind is run
  in when its configuration says so
  ([`HeadingDecl::run_in`](crate::stdcore::features::headings::HeadingDecl#structfield.run_in)); no
  kind is run in by default. The decoration holds the title inside a
  `flm.pres.runin-heading`, which carries what a `flm.pres.block-heading` carries (the
  heading kind's name, the level, whether the heading is in the table of contents, and the
  anchor field), because a decoration's own payload carries nothing.
- The [`defterm`](crate::stdcore::features::defterm) feature stages a
  `flm.pres.deco.runin` holding a definition's lead-in at the head of the definition's
  first paragraph.
- The [`theorems`](crate::stdcore::features::theorems) feature stages a
  `flm.pres.deco.runin` holding a theorem's heading at the head of its first paragraph,
  and, for a theorem kind with a closing mark (a proof's `□`), a `flm.pres.deco.final`
  holding the mark at the end of its last paragraph.
- The [`annotations`](crate::stdcore::features::annotations) feature stages a
  `flm.pres.deco.margin` holding an annotator's initials as the first node of a highlight's
  or a comment's span or block, so that its decoration mark is placed where the annotation
  starts.

The `defterm` and `theorems` features place their decorations through
[`lower_blocks_with_edges`](crate::stdcore::backends::lowering::lower_blocks_with_edges),
which gives a decoration a paragraph of its own where the body begins or ends with a block
other than a paragraph.

## Accepted unrenderable elements

Plan construction checks that the backend can render every element a document may contain.
If no chain of selected rules leads from some element down to the backend's native set, the
plan is refused with [`PlanError::Incomplete`](crate::core::plan::PlanError::Incomplete),
which lists the elements, rather than a run producing incomplete output without a report.
A cycle among the elements the selected rules produce, which would make lowering run
forever, is refused the same way
([`PlanError::ProducesCycle`](crate::core::plan::PlanError::ProducesCycle)).

A caller who wants an output anyway says so, per backend and element, through
[`PlanRequest::unsupported`](crate::core::plan::PlanRequest::unsupported) or the field
[`Overrides::unsupported`](crate::core::plan::Overrides::unsupported). An **accepted
unrenderable element** is an element the plan request lists, per backend, as one the
backend need not render: the plan builds anyway, and each occurrence lowers to a childless
`flm.pres.unsupported` node with one Error diagnostic
([`UnsupportedElement`](crate::stdcore::backends::lowering::UnsupportedElement)). The node
carries an [`UnsupportedPayload`](crate::stdcore::backends::pres::UnsupportedPayload),
which holds the element's name and a message.

The construct's source text never reaches the output; the diagnostic shows it. The
plain-text backend prints
`[unsupported: <element>]` in its place.

The `flm.pres.unsupported` node is staged by the driver itself, with no rule declaring it,
and it also replaces a node whose rule failed ([When a rule fails](#when-a-rule-fails)).
Every backend must therefore hold `flm.pres.unsupported` in its native set. That is the
second native-set implication the `lowering` feature declares, and a backend without it
fails the registry build
([`BuildError::NativeSetIncomplete`](crate::core::registry::BuildError::NativeSetIncomplete)).

## The emit stage and the final stage

Lowering leaves one presentation tree per document tree. Three more stages of the backend
turn those trees into the output the caller reads: the emit stage writes the output's
bytes, the regions stage cuts them into the output's named regions, and the final stage
assembles the entry artifact from the regions and publishes the artifact set. This
section describes them with the plain-text backend as the example; the chapters on the
HTML and the two LaTeX backends describe theirs.

The **emit stage** writes the output format's bytes into a table of the backend's own.
The plain-text backend's emit stage (`flm.text.emit`,
[`TextEmitStage`](crate::stdcore::backends::text::TextEmitStage)) runs once per document,
not once per tree as lowering does: it declares
[`Scope::Document`](crate::core::stage::Scope::Document), because the printed text is one
text. It reads the lowered trees `flm.lowered.text`, the lowered [pieces](#pieces)
`flm.lowered.text.pieces` and the backend's render options, and writes one
[`TextOutput`](crate::stdcore::backends::text::TextOutput) into `flm.output.text`.

Two more stages run once per document after it. The **regions stage** (`flm.text.regions`,
[`TextRegionsStage`](crate::stdcore::backends::text::TextRegionsStage)) writes the output
regions the entry artifact is made of into `flm.output.text.regions`. The **final stage**
(`flm.text.final`, [`TextFinalStage`](crate::stdcore::backends::text::TextFinalStage))
assembles the entry artifact from those regions and writes the output's artifact set into
`flm.output.text.document`. Because emitting and publishing are separate stages, a caller
that wants the printed text and the positions of its elements reads the emit stage's
table, and never deals with artifacts.

### Regions and the final stage

An output region is one named part of an output's entry artifact
([`Region`](crate::core::region::Region)): the document's content, each [piece](#pieces)
placed beside it, and, on HTML, the stylesheets, the scripts, the words of the page's
title and its language. Each backend writes the regions of its output once, in its regions
stage, into the table `flm.output.<backend>.regions`
([`Regions`](crate::core::region::Regions)). That table also states what the entry
artifact is: its identity, its media type, and the artifacts it refers to.

| Backend | Regions stage | Regions |
|---|---|---|
| `text` | `flm.text.regions` | `content`: the whole printed text, with the pieces laid out in it, because a wrapped line may continue across them |
| `html` | `flm.html.regions` | `content` (every tree in document order, a tree placed inside another written where it is placed), one region per piece, `styles`, `scripts`, `title` (the words of the page's title: the title piece's words where the render places that piece and it holds content, else the render option `title`), and `lang` when the render options give a language |
| `latex-rendered` | `flm.latex-rendered.regions` | `content` (the body), one region per piece, and `style-package`, the package name of the style file, which this output always has |
| `latex-recomposed` | `flm.latex-recomposed.regions` | `content` (the body), one region per piece, and `style-package` when the output has a style file |

A region is a list of **parts**, each a text the backend wrote in its output format. A
piece's region also carries the piece's **words**: its displayed characters as plain
text, for a place in the entry artifact where no markup is allowed, such as the title of
a page. The HTML regions `title` and `lang` carry words too (the title's words and the
language tag), and one part, which is those words escaped for HTML text and for an
attribute value respectively. The final stage and a host's template read the same regions,
so the page's title and language are the same, whichever of the two assembles the page.

The files written beside the entry artifact come from tables that the output lists in its
[`RegionsOutput::artifacts`](crate::core::region::RegionsOutput::artifacts): a backend's
own file (the style file of either LaTeX output, in `flm.output.latex-rendered.style` or
`flm.output.latex-recomposed.style`), and
the output's **artifact table** `flm.output.<backend>.artifacts`, which only the
`graphics` feature's publish stage writes (its copies of images, under the strategy
`copy`; see [Images and how an output reaches
them](#images-and-how-an-output-reaches-them)).

Each backend's final stage places the regions in a layout of the backend's own, takes no
template, and adds the files beside the entry artifact through one function,
[`ArtifactSet::insert_entry_with`](crate::core::artifact::ArtifactSet::insert_entry_with).
That function also reports a malformed artifact set, with one Error per problem: two
different files under one identity
([`DuplicateArtifact`](crate::core::artifact::DuplicateArtifact)), or two files that both
claim to be the entry artifact
([`SecondEntryArtifact`](crate::core::artifact::SecondEntryArtifact)). The same file
contributed twice, for example the copy of an image that two trees show, is one file, and
nothing is reported. On HTML the final stage writes the page described in the HTML
chapter; on plain text, the content region alone; on the rendered LaTeX, a complete
document of the class `article`, preceded by a comment line that gives the style file's
name; on the recomposed LaTeX, the body, preceded by the same kind of comment line.

A host can assemble the same regions another way, through a template of its own, as the
`flm` program does. Each backend states which of its outputs are made of regions with a
[`RegionsOutput`](crate::core::region::RegionsOutput), contributed to the [service
slot](crate::guide::concepts_overview#services-and-service-slots) `flm.output.regions`
([`REGIONS_SLOT`](crate::stdcore::backends::lowering::REGIONS_SLOT)): the regions table,
the tables of the files beside the entry artifact, and the capability the final stage
provides. The host registers a stage that provides that capability from the regions, and
prefers it in the plan request; the plan then schedules the host's stage in place of the
backend's final stage, and nothing of the regions is computed twice.

### Trees placed inside other trees

A document may place one tree inside another: one
[fragment](crate::guide::concepts_overview#documents-fragments-and-the-corpus) of the
document placed inside another fragment. The lowered form of the outer tree then holds a
`flm.pres.subtree` node at that place, whose
[`SubtreePayload`](crate::stdcore::backends::pres::SubtreePayload) identifies the placed
tree. An emitter that writes one output document inserts the placed tree's own lowered
form at that node, so that, on plain text, a wrapped line continues across the boundary
between the two trees.

The plain-text emitter therefore prints, in the document's order, every tree that no other
tree places, each with the trees it places inserted into it; trees printed at the top level
are one blank line apart. Trees left over, those that place each other in a cycle, are
printed afterwards, so that every tree is printed exactly once and the cycle is reported.
A placed tree that has no lowered form, and a placement that would print a tree inside
itself, print nothing and are reported
([`MissingSubtree`](crate::stdcore::backends::text::MissingSubtree),
[`SubtreeCycle`](crate::stdcore::backends::text::SubtreeCycle)).

None of the plain-text backend's emit, regions and final stages fails the run. When one of
them cannot read or write a table its declaration describes, its output is absent and the
reason is reported as `flm.text.stage-failed`
([`StageFailed`](crate::stdcore::backends::text::StageFailed)).

## Pieces

A piece (see [Terms used on this page](#terms-used-on-this-page); [`crate::core::piece`];
[pieces](crate::guide::concepts_overview#pieces)) is placed before or after the document's
content. The standard features offer these pieces: the document's title, authors and date (the `meta`
feature), the table of contents (the `toc` feature), the list of each listed kind of
xnote (the `xnotes` feature), and the bibliography (the `citations` feature). An author does not write a piece where it
appears: a stage *builds* it once, over the whole document, and the backend *places* it
before or after the content ([`PiecePlace`](crate::core::piece::PiecePlace)), never at a
position in a tree.

A piece has three parts:

1. **Its content** is a synthesized value made of document elements (the elements the
   feature declares, text, references, mathematics), held in a table of value type
   [`Piece`](crate::core::piece::Piece) named for the piece. A stage of the feature that
   offers the piece, called its **builder**, runs once per document and writes that table
   from what it reads. `Piece { content: None }` places nothing and reports nothing.
2. **Its declaration** is a [`PieceDecl`](crate::core::piece::PieceDecl): the piece's name,
   its table, its place, and whether a render places it by default. The feature contributes
   the declaration to the service slot `flm.pieces`
   ([`SLOT`](crate::stdcore::backends::pieces::SLOT)), which the `pieces` feature declares.
   The feature contributes it as a peer contribution: a value offered to a service slot
   that another feature owns, which the registry build drops with one Note when no feature
   of the registry declares that slot ([`crate::core::services`]). The feature therefore
   also builds in a registry without the `pieces` feature. The order of the contributions is the order in
   which two pieces at the same place are placed. [`PieceDecl`](crate::core::piece::PieceDecl)
   lists what a feature does to offer a piece.
3. **Its rendering** belongs to each backend. The backend's pieces stage
   `flm.<backend>.pieces` ([`PiecesStage`](crate::stdcore::backends::pieces::PiecesStage))
   brings the value up to date, lowers it with the rules the plan selected for that backend
   ([`lower_value`](crate::stdcore::backends::lowering::lower_value)), and writes the table
   `flm.lowered.<backend>.pieces`
   ([`LoweredPieces`](crate::stdcore::backends::pieces::LoweredPieces)); the backend's own
   stages then write the result and place it.

Because the content is made of document elements, and never of a string or of one
backend's own elements, a piece is rendered by the same rules that the feature wrote for
its elements: one builder serves every backend, and another stage writing the same table,
preferred by the plan request, changes the piece on every output at once. A title, an
author or a date is FLM content like any other.

**Which pieces a render places** is decided by one function,
[`selected_pieces`](crate::stdcore::backends::pieces::selected_pieces), per backend, from
the render option
[`PiecesRenderOptions`](crate::stdcore::backends::pieces::PiecesRenderOptions):

- A render places every piece declared on by default that the option's `exclude` does not
  list, and every piece its `include` lists.
- On a backend that the option's `backends` lists, it places only those of them that the
  entry for that backend lists. A host sets these entries from what the template it
  assembles the output with accepts, so that a piece no template places is never built.
  The `backends` field is not a setting that can be written as data.
- On a backend that `backends` does not list, it places all of them, except where the
  backend's pieces stage places only the pieces listed for it
  ([`PiecesStage::placing_only_named`](crate::stdcore::backends::pieces::PiecesStage::placing_only_named)).
  The recomposed LaTeX's pieces stage does, so by default the recomposed LaTeX places no
  piece.

The pieces stage reads the table of every selected piece, and that read makes the plan
schedule the piece's builder; a piece left out schedules nothing. A name in `include` or `exclude`
that no feature declares is reported as `flm.pieces.unknown-piece` (a Warning) and
ignored.

**Reading tables inside a piece.** A lowering rule running inside a piece behaves as
follows:

- It reads the document's document-scope tables (anchors, counters, the label index)
  unchanged, so a reference inside a piece resolves to a label of the document.
- It looks a table entry up by the address of the node it lowers, and inside a piece that
  is an address in the value, which those tables do not hold. A reference point that the
  value states itself (an item of a list that a builder wrote into the value) therefore
  has no number and no anchor; the standard rules report it (`flm.run.missing-entry`) and
  write it without them.
- Content of the document that a piece shows is not copied into the value: the value holds
  a node that points at that content (a [content
  pointer](crate::guide::concepts_overview#content-pointers)), and the content is lowered
  where that node is placed, as the document's own content
  ([`Lowerer::lower_region`](crate::core::lowering::Lowerer::lower_region)). Its reference
  points therefore keep the numbers and the anchors the document gave them; the list of
  footnotes is built this way.
- Content of the document that a piece shows a second time, such as a heading's title in
  the table of contents, is copied into the value as a *shown copy*: each node is copied
  as its element's [`InCopy`](crate::model::aspects::InCopy) aspect says, so a footnote
  becomes its mark and its note stays listed once, where the document wrote it.
- A value must not state a fact about the document as a whole, such as a labeled reference
  point: bringing such a value up to date is refused, the piece is left out with
  `flm.pieces.lowering-failed` (an Error), and the rest of the document is written.
- A piece is never placed inside a tree and never places one: a `flm.pres.subtree` node
  inside a piece writes nothing and is reported (`flm.text.piece-placement`,
  `flm.html.piece-placement`, `flm.latex-rendered.piece-placement`).

**Where each backend places pieces.** The plain-text backend prints, in one layout
document, the pieces placed before the content, then the trees, then the pieces placed
after the content, one blank line between every two; it records position marks for the
trees' bytes and none for a piece's bytes. The HTML backend writes each piece's HTML in
the stage `flm.html.emit-pieces` and makes it a region, which the page places inside
`<main>`, before or after the trees' HTML, as block-level markup like a tree's ([The
stages, the tables and the artifact](#the-stages-the-tables-and-the-artifact)). The
rendered LaTeX writes each piece's LaTeX in the stage `flm.latex-rendered.emit-pieces` and
makes it a region; its final stage places the pieces placed before the content after
`\begin{document}` and the others before `\end{document}`, and what each piece requires is
added to the style file ([The stages, the tables and the artifact set of the rendered
LaTeX](#the-stages-the-tables-and-the-artifact-set-of-the-rendered-latex)). The
recomposed LaTeX writes each piece's LaTeX as a region in its regions stage, and what each
piece requires is added to the style file; its final stage places no piece, because where
a title belongs in a LaTeX document is for a template to decide (`\title`).

A registry holding the plain-text, the HTML or either LaTeX backend must hold the
`pieces` feature: each backend's pieces stage reads `PiecesRenderOptions`, which that
feature declares, and every plan of a registry without it fails with an error that identifies the option
type.

## Outputs and artifacts

An artifact ([`Artifact`](crate::core::artifact::Artifact)) is one file's worth of output,
held as a value (a *folder artifact*, described below, holds the files of a folder); this
library writes no file. An artifact holds:

- its identity, an [`ArtifactId`](crate::core::artifact::ArtifactId): the feature or
  backend that produced it, and a name that this producer chose, written `owner:name`
  (`text:document`);
- its media type, such as `text/plain`;
- its content, an [`ArtifactData`](crate::core::artifact::ArtifactData): the bytes
  themselves, a [`FileRef`](crate::core::artifact::FileRef), the location of a file
  that holds them, or, for a *folder artifact*, the files of a folder (below);
- whether it is the entry artifact of its set;
- optionally its file extension, without the dot (`sty` for the recomposed LaTeX's style
  file, `png` for a copied PNG image), which a host uses in the file's name; without one,
  a host derives the extension from the media type;
- optionally a fixed path, which a host writing the artifact uses as given (the
  recomposed LaTeX's style file has one), and the identities of the other artifacts its
  bytes link to.

An artifact is identified by its identity, not by a path. Because of this, one artifact
can link to another before that other artifact's bytes exist. A host decides where each
artifact is written, through its **naming service**: an
[`ArtifactNamer`](crate::core::artifact::ArtifactNamer), which turns an identity into a
relative path. A host contributes its naming service to the service slot
`flm.artifacts.namer` ([`NAMER_SLOT`](crate::stdcore::backends::lowering::NAMER_SLOT));
the library provides no implementation. The library itself asks the naming service only
where an output must write the address of another artifact: the address of the copy of an
image (below), the name of the recomposed LaTeX's style file, which the document loads by
that name ([the stages, the tables and the artifact
set](#the-stages-the-tables-and-the-artifact-set) of the recomposed LaTeX), and the path of
a file that a compose template ships. A host that writes the files asks it where each
identity goes.

The naming service computes a path from two values, so that a stage that asks during the
run and a host that writes the files afterwards get the same path for one artifact:

- an [`ArtifactRef`](crate::core::artifact::ArtifactRef), what is named: the identity,
  whether it is an entry artifact, its media type, its file extension when its producer
  gives one, whether it is a folder artifact (a folder is named with no extension), and
  the output it is placed under when the caller knows it (a host writing the files does,
  and so does a stage that reads the files of another output; a stage that asks for the
  address of a file it writes does not);
- a [`NamingScope`](crate::core::artifact::NamingScope), the outputs that the command
  produces, each with the file extension of its entry artifact
  ([`NamingScope::of`](crate::core::artifact::NamingScope::of) computes it from the
  registry and the plan's demands). It is the same for every call of one run. A host's
  naming scheme may use it to name every file of a command after one stem: the `flm`
  program's scheme strips the extension of any of the command's outputs from the name it
  is given, so that `-o paper.html` and `-o paper.txt` both write `paper.html` and
  `paper.txt` for the outputs `html` and `text`.

The path depends only on these two values and the naming service's own configuration,
never on which artifacts exist or how far the run has progressed.

[`path_of`](crate::core::artifact::path_of) states where a host writes an artifact: at
its [fixed path](crate::core::artifact::Artifact::fixed_path) when it has one, else at the
path the naming service gives it, an entry artifact being named with the file extension
its output declares. The `flm` program's writer places every file with it, and a stage
that hands the files of an output to another program (as a PDF output compiles the files
of `latex-rendered`) writes each file at the path it gives, so that the links between the
files hold.

An output is what a caller asks for by name (`text`, `latex-rendered`), registered as an
[`OutputDef`](crate::core::registry::OutputDef). The declaration ties three things
together: the capability a plan must reach for the output to exist, the table where the
result is found, and what is known in advance about the entry artifact (its media type and
file extension). The value in that table is an
[`ArtifactSet`](crate::core::artifact::ArtifactSet): every artifact of the output, at most
one of them marked as the entry artifact, the one a reader is meant to open first. An
output made from the files of other outputs instead of from the document (a PDF compiled
from the files of `latex-rendered`) lists them in
[`OutputDef::made_from`](crate::core::registry::OutputDef::made_from), so that a host
prepares them with it: the `flm` program sets up their compose templates.
An output that a package adds, compiled or provided by an extension (an extension in
JavaScript may declare a backend and an output of its own), is asked for by its name in
the same way: the `flm` program runs a workflow that lists it with `-p` (the profile of
the workflow's name), and `-f NAME` selects it among that workflow's outputs ([extensions](crate::guide::command_line#extensions)).

```rust
use flm_core::core::artifact::{Artifact, ArtifactId, ArtifactSet};

let mut set = ArtifactSet::new();
set.insert(
    Artifact::bytes(ArtifactId::new("text", "document"), "text/plain", b"Hello.\n".to_vec())
        .as_entry(),
)?;

assert_eq!(set.len(), 1);
assert_eq!(set.entry().map(|a| a.id.to_string()), Some(String::from("text:document")));
assert_eq!(set.entry().and_then(|a| a.as_bytes()), Some(&b"Hello.\n"[..]));
# Ok::<(), flm_core::core::artifact::ArtifactSetViolation>(())
```

An output of one file is a set of one artifact. Every output uses the same set type, so a
host reads every backend's result the same way, whether it is one file or several.

**A folder artifact** holds a whole folder of files as one artifact
([`Artifact::folder`](crate::core::artifact::Artifact::folder)): its data is
[`ArtifactData::Files`](crate::core::artifact::ArtifactData::Files), a list of files, each
by its path relative to the folder (with `/` between its parts, no `/` at its start, no
`.` or `..` part) and its bytes, the paths distinct, in the order the producer gives. It
has no file extension and its media type is the empty text: each of its files has the
media type of its own extension. The naming service names it as one folder, with the
flag [`ArtifactRef::folder`](crate::core::artifact::ArtifactRef::folder) set, and returns
the folder's path with no extension and no `/` at its end; a host that writes the files
writes each file of the folder under that path. A compose template that ships a folder
(an extension's folder of icons, for example) makes one: the `compose` feature names it
`compose:NAME`, and the template reads its path in `artifacts`
([templates and template engines](crate::guide::extending#templates-and-template-engines)).
The `flm` program names a folder artifact `STEM-NAME` (`paper-icons`) and writes its files
under it (`paper-icons/check.svg`).

### Images and how an output reaches them

An image is a file this library never opens itself. Where a host supplies a **graphics
resolver** ([`GraphicsResolver`](crate::stdcore::features::graphics::GraphicsResolver),
contributed to the service slot `flm.graphics.resolver`; see [the services a host
provides](crate::guide::embedding#services-the-host-provides)), the resolver finds the
file a path refers to and reads it. Each output then **publishes** its images by one of
three **strategies**, chosen per output by the render option `graphics.strategy`, a
map from an output's name to a strategy
([`GraphicsRenderOptions`](crate::stdcore::features::graphics::GraphicsRenderOptions);
[`publish`](crate::stdcore::features::graphics::publish)):

| strategy | the address written | what the output holds |
|---|---|---|
| `reference` (the default) | the file's own address: the resolver's address where it found the file, else the path as written | the entry artifact alone |
| `copy` | the address of the copy, where the host's naming service places it, for example `paper-plot-3fa9c2d1.png` | the entry artifact and one copy per file |
| `data-uri` | the content itself: `data:image/png;base64,iVBORw0…` | the entry artifact alone |

A copy is the artifact `graphics:<stem>-<digest>`: the file's name without its extension,
then eight hexadecimal digits of a digest of the file's content, the copy's format and the
conversion rule's options. A copy depends on the file alone, so a file shown twice (in one
tree, in two trees, or again where a reference shows a heading's title holding the image)
is one copy, whose address every showing writes. The copies go to the output's artifact
table `flm.output.<backend>.artifacts`, which the output's final stage adds to its set. The
plain-text output is one file and holds no copy: `copy` there is an Error, and the image
is published by `reference`. A data address can be shown only in a web page: `data-uri` on
the plain-text output or on either LaTeX output is an Error in the same way.

Under `copy` and `data-uri`, the output's **conversion rules** (`graphics.convert`, a map
from an output's name to a list of
[`ConversionRule`](crate::stdcore::features::graphics::ConversionRule)s) turn an image of
one format into another. The first rule whose `from` lists the file's format applies, and
the first of the host's converters, in the order the host contributed them, that the
rule's `via` allows (every converter, where `via` is empty) and that offers that conversion
performs it; the copy, or the data address, then has the rule's `to` format. A rule that no
converter can perform, a `via` that lists no registered converter, and a converter's
failure are each an Error at the image, which is then published as the file is. Under
`reference` no rule applies, because nothing is read.

```yaml
graphics:
  strategy:
    html: data-uri
    latex-recomposed: copy
  convert:
    html:
      - { from: pdf, to: png, options: { dpi: 192 } }
    latex-recomposed:
      - { from: svg, to: pdf }
```

A path that begins with a scheme (`https://example.org/a.png`, `data:…`) is written as it
is under every strategy, and never read. Where a strategy cannot be carried out (the output
cannot use it, the registry holds no graphics resolver, the file cannot be read, or no
naming service places a copy), an Error is reported at the image and the image is
published by `reference`. With no graphics resolver and no setting, every output writes
each image's path as written.

The size an image is shown at is decided once, for every output alike, where the image is
lowered ([`shown_size`](crate::stdcore::features::graphics::rules::shown_size)): the
`width=` and `height=` the author gave; one of them with the other scaled from the size
the host's **image inspector** reported, so that the image keeps its proportions; or, with
neither, the inspected size. Each dimension is then multiplied by the output's
**magnification** (`graphics.magnification`, a map from an output's name to a
[`Magnification`](crate::stdcore::features::graphics::Magnification)): its factor `raster`
for an image the inspector reported as a raster image (a grid of pixels: PNG, JPEG, GIF,
WebP), its factor `vector` for any other image (a drawing, SVG or PDF, and an image that
was not inspected). Each factor is a number greater than zero, 1 where not given, and it
multiplies the dimensions the author gave as well. The factors let images take another
size relative to the text on one output than on another:

```yaml
graphics:
  magnification:
    html: { raster: 1.5, vector: 1.25 }
```

An image whose size is not decided at all (neither `width=` nor `height=` given, and
nothing inspected: no inspector is registered, its inspection failed, or the path is a web
address, which is never inspected) is reported with the Warning `flm.graphics.unknown-size`
([`UnknownSize`](crate::stdcore::features::graphics::UnknownSize)) on each output, and is
written without a size.

## The plain-text backend

The `text` backend ([`crate::stdcore::backends::text`]) writes a document as plain text,
wrapped to a chosen line width. Its native set is the whole presentation vocabulary,
together with the five structural elements that it writes natively, without a lowering
rule ([`FLOW_THROUGH`](crate::stdcore::backends::text::FLOW_THROUGH): `flm.text`,
`flm.group`, `flm.comment`, `flm.container` and `flm.parbreak`), because they carry no
meaning that plain text has to translate. The feature
[`TextBackend`](crate::stdcore::backends::text::TextBackend) registers the backend, the
`text` output and the `text` workflow.

The backend has five stages: three in its `BackendDef` and two that the `TextBackend`
feature registers beside it. Stages 1 and 2 do not depend on each other; the emit stage
reads the results of both, and stages 4 and 5 follow it in order:

1. the shared lowering stage
   ([`LoweringStage`](crate::stdcore::backends::lowering::LoweringStage)) for `text`,
   `flm.text.lower`, which writes the lowered trees into `flm.lowered.text`;
2. the shared [pieces](#pieces) stage
   ([`PiecesStage`](crate::stdcore::backends::pieces::PiecesStage)) for `text`,
   `flm.text.pieces`, which writes the lowered pieces into `flm.lowered.text.pieces`;
3. the emit stage [`TextEmitStage`](crate::stdcore::backends::text::TextEmitStage);
4. the regions stage [`TextRegionsStage`](crate::stdcore::backends::text::TextRegionsStage);
5. the final stage [`TextFinalStage`](crate::stdcore::backends::text::TextFinalStage).

A caller reads one of two tables:

- [`TEXT_OUTPUT`](crate::stdcore::backends::text::TEXT_OUTPUT) (`flm.output.text`), which
  the emit stage writes, holds a [`TextOutput`](crate::stdcore::backends::text::TextOutput):
  the printed text and the position marks
  ([Position marks](#position-marks-where-a-printed-byte-came-from));
- [`TEXT_DOCUMENT`](crate::stdcore::backends::text::TEXT_DOCUMENT)
  (`flm.output.text.document`), which the final stage writes, holds the artifact set with
  the one entry artifact `text:document`, of media type `text/plain`. It is the table of
  the `text` output, which the `text` workflow lists.

### Render options

[`TextRenderOptions`](crate::stdcore::backends::text::TextRenderOptions) has six fields:

- `width` is the line width in columns, or `None` to never wrap; the default is 80.
- `tab_width` is the width of a tab in columns, 4 by default: a line of a quotation
  indented by `\indent\indent` starts eight columns in. Every length is converted to
  columns by one function,
  [`TextRenderOptions::columns`](crate::stdcore::backends::text::TextRenderOptions::columns):
  a tab spans `tab_width` columns and an em two; the units of fixed size are converted at
  twelve points to the em, so a point is a sixth of a column and an inch twelve columns;
  each result is rounded to the nearest whole column.
- `run_in_stop` is the stop written after the title of a run-in heading (a heading set at
  the start of the paragraph it introduces), so that `\paragraph{Note} Some content.`
  prints as `Note. Some content.` when the `paragraph` heading kind is run in. The default
  is a period, and the empty text writes no stop. The HTML backend has an option of the
  same name and meaning, so the two outputs of one document agree.
- `root_heading_level` is the heading level printed as a first-level heading, 1 by
  default. The HTML backend has it too, with the same meaning ([Headings in
  HTML](#headings-in-html)).
- `link_addresses`, on by default, makes a link to an address outside the document print
  that address after its content, between angle brackets:
  `\href{https://example.com}{Click here}` prints `Click here <https://example.com>`. Off,
  every link prints its content alone, which keeps a text with many links readable.
- `link_addresses_when_shown`, off by default, concerns a link whose content is already
  its address's display, which the link states itself
  ([`LinkPayload::shows_target`](crate::stdcore::backends::pres::LinkPayload::shows_target)):
  `\url{https://example.com}` prints `example.com` and `\email{a@b.c}` prints `a@b.c`;
  with the option on, they print `example.com <https://example.com>` and
  `a@b.c <mailto:a@b.c>`. This option has no effect when `link_addresses` is off.

A link to a place inside the document never prints its target.

The run-in stop is written only where the title does not already end in `.`, `,`, `:`,
`;`, `?` or `!`
([`ends_in_sentence_punctuation`](crate::stdcore::backends::pres::ends_in_sentence_punctuation),
the test both backends use). That test looks only at the characters the title ends in: a
title ending in a formula gets the stop after it. A program that needs another treatment
registers a lowering rule of its own ([Headings in HTML](#headings-in-html) gives
the details).

All six fields are declared with
[`Bind::Render`](crate::model::options::Bind::Render): their values come from the render
request rather than from the document, and they are part of the plan's identity, so two
runs asking for different widths use two plans.

```rust
# use std::sync::Arc;
# use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
# use flm_core::model::options::OptionValues;
# use flm_core::core::plan::{Overrides, Plan};
# use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
# use flm_core::core::run::{Externals, Run, RunOptions};
# use flm_core::lang::{FlmOrigin, ParseOptions, parse_fragment};
# use flm_core::stdcore::{self, StdcoreConfig};
# use flm_core::techy::source::Source;
use flm_core::stdcore::backends::text::{self, TEXT_OUTPUT, TextRenderOptions};

# let mut builder = RegistryBuilder::new();
# builder.use_package(stdcore::package(StdcoreConfig::new()));
# builder.use_package(flm_core::math_passthrough::package());
# let registry = builder.build(&RegistryInputs::new())?;
# let source = Arc::new(
#     Source::new("one two three four five six seven eight nine ten")
#         .with_origin(FlmOrigin::none()),
# );
# let fragment =
#     parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())?;
# let mut content = DocContent::new();
# content.place(Arc::new(fragment));
# let doc = Arc::new(Doc::new(content));
// Wrap at twenty columns instead of the default eighty.
let overrides = Overrides {
    render_options: OptionValues::new()
        .with(TextRenderOptions { width: Some(20), ..TextRenderOptions::default() }),
    ..Overrides::none()
};

let plan = Arc::new(Plan::for_workflow(&registry, text::WORKFLOW, &overrides)?);
let mut run = Run::new(doc, plan, Externals::none())?;
run.execute(&RunOptions::new())?;

assert_eq!(
    run.table(TEXT_OUTPUT)?.expect("written").text,
    "one two three four\nfive six seven eight\nnine ten\n",
);
# Ok::<(), Box<dyn std::error::Error>>(())
```

### What the emitter prints

The emitter ([`mod@crate::stdcore::backends::text::emit`]) maps each presentation element
onto the constructs of the [layout engine](#the-layout-engine), and the layout engine
breaks the lines and produces the characters. The emitter works in two contexts.

In **block context**, a run of blocks is printed one blank line between every two blocks:

- A `par` (paragraph) is filled word by word.
- A `block-heading` prints its title, then a line under it as wide as what the title
  printed, a number the heading carries included: `=` for a heading at the root heading
  level (1 by default), `-` for one level deeper. A heading deeper still gets no line but
  `#` marks before its title, three for a heading two levels below the root
  (`### Title` for a level-3 heading under the default root). A heading above the root
  level is reported as `flm.pres.heading-above-root` and printed with the line of `=`. A
  heading of a run-in heading kind is not a `block-heading`: it is a run-in decoration,
  printed before the first word of its paragraph (inline context, below).
- A `block` prints its body's blocks, regardless of its role. A decoration whose mark is among
  the block's blocks prints as inline content at its mark, in a paragraph of its own where
  a block follows. A quotation, for example, prints its quotation sections one blank line
  apart, with no prefix and no indentation, and an attribution as a paragraph.
- A `list` prints, for each entry, the label, padded to the width of the widest label plus
  one column, with the item's blocks hanging at that column; where every label of the list
  is empty, there is no label column. An entry whose item is empty prints its label alone.
  Entries are one blank line apart.
- A `cells` node (a table of cells) is laid out as a table of the layout engine ([Tables
  of the layout engine](#tables-of-the-layout-engine)): one column per column of its grid,
  each cell's blocks padded in the cell's own alignment, the content of a cell that spans
  several positions at the first position of its span, and the leading rows of column
  headers above a line of `-`. A row header, a rule at a cell's edge and a custom cell
  style do not show in plain text.
- A `lines` node prints one line per `line`, indented by the line's left indentation
  converted to columns ([Render options](#render-options)); a quotation's `\lines` section
  is written so.
- A display `math-passthrough` prints its source inside `\[…\]`, or inside
  `\begin{env}…\end{env}` for a math environment, indented by two columns. Content inside
  the formula (a text command's argument, an equation line's tag) is printed as the text it
  became, inside its command (`\text{…}`), so that markup written inside it and the escapes
  an author typed there do not show in this output.
- A block `verbatim` node prints its content byte for byte.
- A block `image` prints `[image: <alternative text or address>]`.
- An `unsupported` node prints `[unsupported: <element>]`, and a `dropped` mark prints
  `[dropped: <statement>]`, so that what is missing is easy to see.
- An `unknown` environment (one that no feature declares) prints as it was written:
  `\begin{name}` on a line of its own, then its lowered body as a run of blocks, then
  `\end{name}` on a line of its own.

In **inline context**, every whitespace character is a place where the line may break,
except the non-breaking spaces (U+00A0, U+2007 and U+202F), which join the words on either
side of them into one word. Each word is an *atom*: a run of text the layout engine
prints whole and never splits across two lines.

- A `span` prints its content; every style other than superscript and subscript is
  dropped. A superscript span writes each character of its text in its Unicode
  superscript form (`10` as `¹⁰`, `a` as `ᵃ`), and a subscript span in its subscript form
  (`₁`). A character that has no such form is written as it is (`q`, `S`), and no marker
  such as `^` is added, because the output is text a reader reads. The text of content
  nested inside the span (a link, another span) is converted too. A link's printed
  address, mathematics, verbatim content, an `unsupported` node and a decoration moved out
  of the span are written unchanged. The forms are listed in
  [`superscript`](mod@crate::stdcore::backends::text::superscript).
- A `link` prints its content, followed by ` <address>` when the target is outside the
  document and the render options ask for it.
- An inline `math-passthrough` prints as one atom, `\(source\)`, and an inline `verbatim` node
  prints as one atom too; verbatim content holding a line break or a tab is printed as a
  block instead.
- A decoration mark prints the decoration attached at that mark. A run-in decoration
  prints as `content.` followed by a breakable space; the stop after it is the render
  option `run_in_stop`, added only where the content does not already end in `.`, `,`,
  `:`, `;`, `?` or `!`. A final decoration (a closing mark) prints after a non-breaking
  space (U+00A0), so that it never begins a line by itself. A margin decoration prints as
  `[content]`, between breakable spaces.
- An `unknown` macro prints its source text, `\name`, as one atom.
- An inline `image`, an `unsupported` node and a `dropped` mark print as in block context,
  each as one atom.

What plain text cannot express is dropped without a report: a span's style other than
superscript and subscript, roles, document roles and kind tokens, a link's accessible name
and link kind, anchor fields, an image's width, height and alternative sources, a line's right
indentation and alignment, a verbatim node's verbatim kind and language, and the target of a link
inside the document. Content is never dropped.

This FLM document, using only the standard features:

```text
\section{Introduction}\label{sec:intro}

See \ref{sec:intro} for an \emph{emphasized} bit, in a paragraph long
enough to wrap at forty columns.

\subsection{Details}

The identity \[ a^2 + b^2 = c^2 \] holds, and \verbcode+x = 1+ is code.

\begin{verbatimcode}
let x = 1;
let y = 2;
\end{verbatimcode}
```

prints, at a width of forty columns, as:

```text
Introduction
============

See Introduction for an emphasized bit,
in a paragraph long enough to wrap at
forty columns.

Details
-------

The identity
  \[ a^2 + b^2 = c^2 \]
holds, and x = 1 is code.

let x = 1;
let y = 2;
```

The reference prints its target's title, because `\ref` lowers to a link around what the
target has to show (here its title, since nothing in this document is numbered), and the
target is inside the document, so the emitter prints no address after it. A target with
neither a number nor content to show (a float with no number and no caption, an equation
in a registry without the `numbering` feature) prints the label as the reference wrote it,
and the reference is reported (`flm.refs.nothing-to-show`) on every backend that shows it;
the recomposed LaTeX, which writes the reference for LaTeX to set, reports nothing.

### Xnotes

An xnote is a note set apart from the running text, marked where it belongs and
shown elsewhere or in place (a footnote, an endnote, a side note); a kind of xnote, or
**xnote kind**, is one way of being an xnote, declared by the feature's configuration
([`xnotes`](crate::stdcore::features::xnotes)). The rule
[`XNOTE_RULE`](crate::stdcore::features::xnotes::XNOTE_RULE) writes an xnote where it is
written, with elements of the shared vocabulary; this backend prints them as it prints
any span, superscript or block. What is printed where the xnote is written depends on the
xnote kind's placement.

A **listed** xnote leaves its **xnote mark**: a superscript span holding a link to the
xnote, printed as the mark text in superscript form, the link's target inside the document
printed as nothing. The mark text is, for a numbered xnote kind, the xnote's own number
written in the xnote kind's mark style (`#{arabic}` by default; for an xnote kind numbered
within sections, the number within the section, without the section's part), and, for an
unnumbered xnote kind, the xnote kind's fixed mark, empty by default. A fixed mark is FLM text like
any other, so `\dag` prints the character the macro writes. A space written before the
command is not printed: the xnote mark attaches to the word before it.

```text
The claim holds in every case.\footnote{Except the empty one, which we exclude
by convention.} The next sentence goes on.
```

prints

```text
The claim holds in every case.¹ The next sentence goes on.
```

The content of a listed xnote is shown in **the xnote kind's list**, a piece placed after
the document's content ([Pieces](#pieces)). The list is one synthesized value, built once
for every output by the xnote kind's builder,
[`ListBuilder`](crate::stdcore::features::xnotes::ListBuilder): one entry per xnote of that
xnote kind, in document order, each holding a [content
pointer](crate::guide::concepts_overview#content-pointers) to the xnote's content, never a
copy of it. The xnote kind's list rule,
[`ListRule`](crate::stdcore::features::xnotes::ListRule), lowers the list into a block
holding the xnote kind's title as a heading and a labeled list. Each entry's label is the
xnote's mark text, in superscript form, and its item is the xnote's content: one
paragraph where the content is inline, its paragraphs and blocks as they are otherwise.
This backend prints the heading as it prints every block heading of its level, hangs the
entries at one column as it hangs every labeled list, and indents an entry's second
paragraph to that column after a blank line. The two examples, as one document:

```text
The claim holds in every case.\footnote{Except the empty one, which we exclude
by convention.} The next sentence goes on.

Two paragraphs.\footnote{First paragraph.

Second paragraph.} And on.
```

print

```text
The claim holds in every case.¹ The next sentence goes on.

Two paragraphs.² And on.

Footnotes
---------

¹ Except the empty one, which we exclude by convention.

² First paragraph.

  Second paragraph.
```

An xnote kind with no title
([`XnoteKind::without_title`](crate::stdcore::features::xnotes::XnoteKind::without_title))
prints no heading: a blank line alone separates the list from the content. A title two
levels or more below the root heading level prints as such a heading prints (`### Notes`
for a title at level 3 under the default root heading level, 1). A title is FLM text,
compiled when the registry is built, and a title that is not valid FLM fails the build. It
is read in the language of a number template with no placeholder available, so a literal
`#` is written `\#` or `##`, and a placeholder such as `#{n}` fails the build. The entries
of an unnumbered xnote kind are labeled with its fixed mark, or, for an empty mark, hang at the
margin. Each xnote kind has a list of its own, in the order of the xnote kinds; a kind shown in
place, and a kind with no xnote in the document, has none. A render leaves a list out
through the pieces' render options
([`PiecesRenderOptions::exclude`](crate::stdcore::backends::pieces::PiecesRenderOptions::exclude)
with the piece's name, `flm.xnotes.list.<kind>`), and its builder is then not scheduled.

The content of a listed xnote is lowered once, in the list, as the document's own content;
where the xnote is written, only its xnote mark is written. Everything inside the content
is therefore what it is in the document: a numbered list inside a footnote shows its
numbers in the list of footnotes, and a link to one of its items leads there; a heading
inside a footnote shows the number the document gave it, and its anchor is placed there;
a labeled displayed formula inside a footnote is an ordinary labeled reference point, which
`\eqref` reaches; a reference inside it resolves to the document's label, and a problem
inside it (an unresolved reference) is reported once.

An xnote may be written inside another xnote's content, of its own xnote kind or of another. It
is an xnote like any other, numbered in document order and listed in its own kind's list,
and its xnote mark is placed inside the outer xnote's content, where that content is shown:

```text
A claim.\footnote{Proved in the appendix.\footnote{Written with a friend.}}
```

prints

```text
A claim.¹

Footnotes
---------

¹ Proved in the appendix.²

² Written with a friend.
```

An xnote of another xnote kind written inside a footnote is listed with its own kind; an
in-place xnote inside a listed one is printed in place inside the entry, and a listed xnote
inside an in-place one has its xnote mark in place. A label on an inner xnote labels it as
any label does.

An **in-place** xnote holding inline content is printed where it is written, in brackets,
with its xnote mark at the head, and with the author's spacing kept: `case.\footnote{…}`
prints `case.[¹…]` and `case. \footnote{…}` prints `case. [¹…]`; an unnumbered xnote kind
with no mark prints `[An aside.]`. An in-place xnote holding paragraphs interrupts its
paragraph as a displayed formula does: the paragraph before it ends, the xnote's
paragraphs follow with `[` and the mark at the head of the first and `]` at the end of the
last, and then the paragraph continues:

```text
Two paragraphs.

[¹First paragraph.

Second paragraph.]

And on.
```

Content that begins or ends with a block other than a paragraph, such as a list, gets the
opening bracket and the mark in a paragraph of their own before it, and the closing bracket
in a paragraph of its own after it. Where such an xnote is written in a place that holds
no block (a heading's title, the argument of `\textbf{…}`), the language layer reports it
(`flm.blocks.block-in-inline`), and the xnote is printed inline, its paragraphs joined by
a space: `Title [¹a b]`. An in-place xnote written inside another is printed in place
inside it, with its own mark and number: `A\footnote{a \footnote{b} c}` prints
`A[¹a [²b] c]`.

An xnote in content shown again, away from where it is written (a heading's title that a
reference prints, an unnumbered float's caption, a defined term, a list item's tag),
prints its xnote mark alone there, for either placement: a reference to
`\section*{Intro\footnote{N.}}` prints `Intro¹`, and the note itself is printed, and
listed, only where it is written. A labeled note there keeps its label where it is
written, so `\ref` to that label refers to the note. On the HTML page, such a mark has no
identifier of its own and links to the note (see [Xnotes in
HTML](#xnotes-in-html)). The recomposed LaTeX writes no such mark: it lowers no content shown again,
because it writes a reference back as `\ref`, for LaTeX to resolve.

### Defined terms

A term definition, the `defterm` environment
([`defterm`](crate::stdcore::features::defterm)), is written by its rule
[`DEFINITION_RULE`](crate::stdcore::features::defterm::DEFINITION_RULE) as a block around
its paragraphs. The first paragraph begins with the **lead-in**: the configured template
([`DeftermConfig`](crate::stdcore::features::defterm::DeftermConfig)), `#{term}: ` by
default, staged as a run-in decoration. This backend prints it as it prints a run-in
heading: before the paragraph's first words, followed by the run-in stop only where it
ends in no punctuation. The default ends in a colon, so
`\begin{defterm}{Entropy}A measure of disorder.\end{defterm}` prints
`Entropy: A measure of disorder.` Where the body begins with a block other than a
paragraph, or is empty, the lead-in is printed in a paragraph of its own. With no lead-in
configured, the definition prints its body alone.

A term link, `\term[ref]{shown}`, is written by
[`TERM_RULE`](crate::stdcore::features::defterm::TERM_RULE) as a link of link kind `term`
around the text written for it, which this backend prints as that text, the link's target
inside the document printed as nothing. Inside the definition of its own term, a term link
is a span of role `flm.defterm.term` instead, printed as its text. A term link that nothing
resolved is written as the span of an unresolved reference, and printed as its text too.

The HTML backend writes a term definition and a term link through the classes and
attributes a stylesheet may rely on (see [Defined terms in
HTML](#defined-terms-in-html)), and the `latex-recomposed` backend writes a term definition back as its
environment and a term link through a LaTeX hook of its own ([Defined terms in the
recomposed LaTeX](#defined-terms-in-the-recomposed-latex)).

### Annotations

A reviewer's annotations ([`annotations`](crate::stdcore::features::annotations)) print as
their text, the annotator's initials first. On this backend every annotation is lowered by
the backend's own rule
([`AnnotationTextRule`](crate::stdcore::features::annotations::AnnotationTextRule)), which
differs from the general rule
([`AnnotationRule`](crate::stdcore::features::annotations::AnnotationRule)) in comments
alone. A **highlight** is lowered as the general rule lowers it: a span holding a margin
decoration with the initials, then the highlighted passage. This backend prints the
decoration as it prints every margin decoration, then the passage:
`word \phf{passage} word` prints `word [PhF] passage word`. A **comment** is lowered with
its brackets and initials written as text around the remark, and no decoration:
`word \phf[note] word` prints `word [PhF: note] word`, and `word [note] word` for an
annotator with no initials. An annotator with no initials prints a highlight as its
passage alone.

An annotation that crosses a paragraph break is cut into parts at each paragraph break, and
only the first part carries the initials; a comment's bracket opens on its first part and
closes on its last:

```text
Some text [PhF: A comment.

With multiple paragraphs.] More text
```

A part around a block (a list inside a highlighted passage) prints the initials, where it
is the first part, in a paragraph of their own above the block. Under the render option
[`hidden`](crate::stdcore::features::annotations::AnnotationsRenderOptions::hidden), a
highlight prints as its passage, and a comment as nothing.

### Images

An image ([`graphics`](crate::stdcore::features::graphics)) is lowered by the image rule
([`image_rule`](crate::stdcore::features::graphics::image_rule)) as a `flm.pres.image`
node carrying the address the plain-text output published for it (the plain-text output
always uses the `reference` strategy: the graphics resolver's address where it found the
file, else the path as written), whether the image is a block or inline, the alternative
text and the size. This backend prints it as `[image: <alternative text or address>]`,
with the alternative text where there is one: `\includegraphics[alt=A plot]{plot.png}`
prints `[image: A plot]`, and `\includegraphics{plot.png}` prints `[image: plot.png]`. An
image is inline content everywhere except as a direct child of a float's body ([Graphics](crate::guide::language_syntax#graphics) says where an
image is a block). Inline, it is one word of the text it is
written in (`See [image: A plot] here.`); as a block, the content of a float, it prints on
a line of its own. The size is not printed.

### Floats

A float ([`floats`](crate::stdcore::features::floats)) is written by its rule
[`FLOAT_RULE`](crate::stdcore::features::floats::FLOAT_RULE) as a block of role
`flm.floats.float` holding a block of its content and, where the float is numbered or has
a caption, a block holding the line under the content, the number in a span of role
`flm.floats.float.number`. This backend prints them as it prints any blocks: the content's
blocks, then the line under it, one blank line apart, with nothing drawn around them.

```text
[image: plot.png]

Figure 1: A plot of the data.
```

A float with a caption and no number prints its caption alone under its content, and a
float with neither prints its content alone.

### Citations

A **citation chunk** ([`citations`](crate::stdcore::features::citations)) is a
`\cite[note]{keys}` together with the `\cite` calls written directly after it, or an
`\R\cite{keys}`. Its rule writes it where it is written by staging its **citation mark**
([`stage_mark`](crate::stdcore::features::citations::stage_mark)): a span of role
`flm.citations.mark`, whose kind token says how it is written, `attached` for a `\cite`
and `standalone` for an `\R`. The span holds the numbers of the cited works, each a link of
link kind `cite` to the work's identifier `#citation-<n>`, with the delimiters, the join texts,
the note separator and the notes outside the links: first one group of the keys without a
note, then one group per note. This backend prints the span as its content, the links'
targets inside the document printed as nothing.

```text
Codes of this family are well studied [1,2]. The bound is tight [3][1; Thm 4].
```

The rule of a `\cite` writes, before the span, the space its form calls for: a no-break
space under the bracketed form (a span of role `flm.citations.space`, placed directly
before the citation mark's span and holding that character, printed as the character), and
nothing under the superscript form, where the span carries the superscript style and
prints raised digits, `text¹,²`. The rule of an `\R` writes the word of the [citation counter style](crate::stdcore::features::citations#the-configuration)
before the span, in a span of role `flm.citations.word` (`refs. [1,2]`, `Ref. [1]` under
the flag `S`, `\R[S]`, which capitalizes the word; no word under the flag `I`, `\R[I]`), and nothing before the word. Its citation mark is never
raised, and under the superscript form it shows its numbers bare, `refs. 1,2`.

A citation chunk that shows no number (every call lists no citation key, or an `\R` has
no citation after it) is written as a dropped mark,
`[dropped: citation]`, beside the Error its rule reports. Under the in-place placement
([`CitationPlacement::InPlace`](crate::stdcore::features::citations::CitationPlacement)),
the span shows the citation entries instead, with the kind tokens `attached` or
`standalone`, then `in-place`. For each cited work, in the order written, it shows the
style's opening delimiter, the work's citation entry (read by citation key from the table
`flm.citations.entries`, and spliced), the note separator and the note where there is one,
and the closing delimiter; it holds no link under either form, and a `\cite` writes its
no-break space before it. A citation key with no citation entry shows a dropped mark
inside its brackets.

```text
The bound is tight [Knuth, 1984][Lamport, 1994; Thm 4]; see [[dropped: bibliography entry]].
```

The **bibliography**, the piece `flm.citations.bibliography`
([`BIBLIOGRAPHY`](crate::stdcore::features::citations::BIBLIOGRAPHY)), holds one
synthesized element of that name. Its rule
([`BibliographyRule`](crate::stdcore::features::citations::BibliographyRule)) stages a
block of role `flm.citations.bibliography`, with the kind token `citations`, holding a
heading (the configured title, `References` by default, at the configured level, 2 by
default, with the heading kind name `citations`, out of any table of contents) and a
labeled list (kind tokens `numbered`, `citations`, `labeled`) with one entry per cited
work, in the order of the numbers. An entry's label is a span of role
`flm.citations.entry.number` holding the number written through the citation counter style (`[1]`,
or `1` under the superscript form); its item is the citation entry's content as one
paragraph, or a dropped mark for a citation key with no citation entry; and the entry's
identifier `citation-<n>` is placed on it (the list payload's `entry_anchors`). The piece
is placed after the lists of xnotes. Nothing is placed under the in-place placement, or
where the document cites nothing.

```text
References
----------

[1] Knuth, The Art of Computer Programming, 1984

[2] [dropped: bibliography entry]
```

A citation entry whose content states a fact about the document (a labeled footnote, for
example) is refused where it is shown, as any synthesized value stating one is: the
bibliography is left out with the pieces' Error (`flm.pieces.lowering-failed`), and under
the in-place placement the citation chunk's rule fails. On the recomposed LaTeX, a citation
is written through the LaTeX hooks described in [Citations in the recomposed
LaTeX](#citations-in-the-recomposed-latex); no bibliography is written there, because
LaTeX writes its own.

### The table of contents

The **table of contents**, the piece `flm.toc` of the [`toc`](crate::stdcore::features::toc)
feature ([`PIECE`](crate::stdcore::features::toc::PIECE)), lists the document's headings
(the [language syntax guide](crate::guide::language_syntax#the-table-of-contents) says
which, and how a render asks for it). It is off by default, and it is placed before the
content, after the metadata's pieces. Its content holds three synthesized elements: the
root `flm.toc`, holding one list `flm.toc.list`, whose entries `flm.toc.entry` each hold
the entry's title and, where the heading has listed sub-headings, a list of them. The
feature's general rules ([`toc::rules`](crate::stdcore::features::toc::rules)), the same
on every backend, stage:

- the root as a `block` of role `flm.toc`, with the kind token `toc` and the document
  role `toc`;
- each list as a `list` with the kind tokens `unnumbered` and `toc`, and an empty label
  for every entry;
- each entry as a `link` of link kind `toc` to the heading's anchor, holding the heading's
  number (where it has one) in a span of role `flm.toc.entry.number`, a space, and the
  entry's title; then the list of sub-entries, where there is one. The number is the
  heading's number alone, without the word and the delimiters a reference writes (`1.2`,
  not `§ 1.2`). An entry whose heading has no anchor (in a registry without the `refs`
  feature) holds the same content without the link.

This backend prints the root's list as every list whose labels are all empty: one entry
per paragraph, at the left margin, the entries one blank line apart. A nested list is
printed the same way, after the entry that holds it, so the nesting shows only through
the numbers. For `\section{One}`, `\subsection{Two}` and `\section{Three}` under
numbering:

```text
1 One

1.1 Two

2 Three
```

### Position marks: where a printed byte came from

The emitter wraps every presentation node it prints in a layout mark, and
[`TextOutput::marks`](crate::stdcore::backends::text::TextOutput::marks) records, for each
mark, the byte range and the line and column range of what that node printed: a **position
mark** ([`TextMark`](crate::stdcore::backends::text::TextMark)).

A position mark refers to a *lowered* node, by a
[`LoweredAddr`](crate::stdcore::backends::lowering::LoweredAddr): the document tree the
node was lowered from, and the node's position in that tree's lowered form. It does not
refer to a node of a frozen tree, because a rule may stage nodes that correspond to no
single node of the document. The lowered node is found with
[`LoweredAddr::resolve`](crate::stdcore::backends::lowering::LoweredAddr::resolve), and its
span and provenance lead back to the document. Because a position is the same in two runs
over the same document, two such runs produce equal position marks.

[`TextOutput::slice`](crate::stdcore::backends::text::TextOutput::slice) returns the
printed text that one of this output's position marks covers (it panics for a mark of
another output), and
[`TextOutput::marks_at`](crate::stdcore::backends::text::TextOutput::marks_at) the position
marks of one lowered node. An editor that shows a document beside its rendering, or a test
that checks where something was printed, is built on these.

### The depth limit and the worker thread

The emitter's walk recurses as deeply as the trees nest, counting the trees they place,
and is bounded by [`DEPTH_LIMIT`](crate::stdcore::backends::text::DEPTH_LIMIT). A tree that
nests more deeply prints nothing and is reported as `flm.text.nesting-too-deep`
([`NestingTooDeep`](crate::stdcore::backends::text::NestingTooDeep)). A tree parsed from a
document never comes near the limit, because the parse refuses far shallower nesting; a
lowered tree can reach it when rules stage levels of their own, or when placed trees form
a long chain.

Because the walk recurses, it must run on a worker thread: a thread created with at least
[`WORKER_STACK_SIZE`](crate::lang::guard::WORKER_STACK_SIZE) bytes of stack
([`on_worker_thread`](crate::lang::guard::on_worker_thread)). Every stage of a run already
runs on one. [`emit`](crate::stdcore::backends::text::emit()) therefore runs in place when
its caller is a worker thread, and creates one otherwise. When the operating system refuses
to create a thread, it reports `flm.text.worker-thread-unavailable`
([`NoWorkerThread`](crate::stdcore::backends::text::NoWorkerThread)) and prints nothing. It
never panics.

## The layout engine

The [layout engine](crate::layout) breaks lines, indents, pads, and solves the column
widths of tables, for output made of character cells. It is a module of its own that does
not depend on [techy](crate::techy) (the parser toolkit FLM is built on), on the document
model or on any backend, so its constructs can be extended without affecting anything
else. In this library, only the plain-text backend's emitter builds layout documents. The HTML
emitter and the presentation readers use the engine's test of which spaces allow a line
break ([`is_breakable_space`](crate::layout::is_breakable_space)).

An emitter that uses the engine does not write characters as it walks a document. It builds
a **layout document** in a [`DocBuf`](crate::layout::DocBuf): the words as atoms, the
places where a line may or must end, indentation, prefixes written at the start of every
line, verbatim blocks, tables and layout marks. Then [`print`](crate::layout::print) or
[`print_full`](crate::layout::print_full) decides the line breaks for a target width and
produces the text.

```rust
use flm_core::layout::{DocBuf, Profile, print};

let mut b = DocBuf::new();
let label = b.text("  • ");
let body = b.fill_words("a list item whose text wraps under its own first word");
let hung = b.align(body);
let item = b.concat(&[label, hung]);

assert_eq!(
    print(&b, item, &Profile::new(Some(32))),
    "  • a list item whose text wraps\n    under its own first word\n",
);
```

### The constructors of a layout document

The constructors of [`DocBuf`](crate::layout::DocBuf) are all the constructs a layout
document can hold:

| constructor | what it adds to the layout document |
|---|---|
| [`text`](crate::layout::DocBuf::text) | an atom: printed whole, never split, never rewritten |
| [`line`](crate::layout::DocBuf::line) | a place where the line may break; one space when it does not break |
| [`softbreak`](crate::layout::DocBuf::softbreak) | a place where the line may break; nothing when it does not break |
| [`hardline`](crate::layout::DocBuf::hardline) | a line break that always happens |
| [`blocksep`](crate::layout::DocBuf::blocksep) | a line break followed by *n* blank lines |
| [`nest`](crate::layout::DocBuf::nest) | indents the child by a fixed number of columns |
| [`align`](crate::layout::DocBuf::align) | indents the child to the column where it starts (a hanging indent) |
| [`group`](crate::layout::DocBuf::group) | a unit whose line breaks are all taken or none: it prints on one line if it fits |
| [`concat`](crate::layout::DocBuf::concat) | a sequence |
| [`prefix`](crate::layout::DocBuf::prefix) | puts a string in front of every line of the child (`> `) |
| [`raw`](crate::layout::DocBuf::raw) | verbatim lines, never rewrapped; its [`RawPolicy`](crate::layout::RawPolicy) says whether they follow the surrounding indentation or start at column zero |
| [`table`](crate::layout::DocBuf::table) | rows of cells in columns whose widths the engine computes |
| [`mark`](crate::layout::DocBuf::mark) | a layout mark: asks the printer to report where the child was printed |
| [`fill_words`](crate::layout::DocBuf::fill_words) | prose: the words of a string joined by one group per space between words (a non-breaking space joins two words into one) |

### Width and wrapping

A [`Profile`](crate::layout::Profile) carries the target width and nothing else. `None`
means "never wrap"; everything else is fixed: `\n` line endings, exactly one final
newline after a non-empty text, and atoms never split.

Widths are measured in **display columns**: the sum, over grapheme clusters, of each
cluster's Unicode width. A *grapheme cluster* is what a reader perceives as one character:
an `e` followed by a combining accent, or a whole family emoji joined by zero-width
joiners. Summing cluster by cluster makes the measure additive (the width of two strings
joined is the sum of their widths) and monotone (a longer string never measures narrower
than its beginning), which the printer relies on. `Über Bäume — 日本語` is 19 columns,
and a family emoji 2. East-Asian Ambiguous characters, such as `…`, `→`, `•` and the em
dash, count as one column, as on a terminal configured for Western languages. [`display_width`](crate::layout::display_width) gives the same
measure to an emitter that aligns text itself, for example to draw a line of `=` as wide
as a heading or to pad a list label to a column. In the same way,
[`is_breakable_space`](crate::layout::is_breakable_space) gives the engine's own test of
what separates two words, to an emitter that cuts text into words before handing them to
the engine; an emitter that asks it rather than `char::is_whitespace` keeps a non-breaking
space inside the word it joins.

A [`group`](crate::layout::DocBuf::group) prints on one line if it, *and what follows it on
that line*, fits; otherwise its line breaks are taken. Prose is filled with
[`fill_words`](crate::layout::DocBuf::fill_words), which puts one group around each space
between words, so each space decides on its own whether it stays a space or becomes a line
break. Because a group's test includes what follows it, wrapping is correct even before
trailing text that has no place to break of its own.

Four of the engine's guarantees matter to an emitter:

- **One indentation stack.** [`nest`](crate::layout::DocBuf::nest),
  [`align`](crate::layout::DocBuf::align) and [`prefix`](crate::layout::DocBuf::prefix)
  all push units onto one stack, written at the start of every line in nesting order, so a
  list inside a quotation and a quotation inside a list both come out right, and a prefix's
  width is subtracted from the width available for wrapping.
- **Blank lines come only from [`blocksep`](crate::layout::DocBuf::blocksep).** Adjacent
  requests for a line break merge, taking the larger number of blank lines; line breaks at
  the very start and end of the output are dropped. A blank line is written with the
  indentation its surroundings have in common, trimmed of trailing whitespace, so the
  blank line between two paragraphs inside one quotation prefixed by `> ` prints `>`, and
  the one between two such quotations prints nothing.
- **No trailing whitespace of the engine's own.** Indentation, and the space of a line
  break not taken, are written only when content characters follow on the same line.
- **Determinism.** The output depends only on the layout document, the root node and the
  profile. Nothing is carried from one print call to the next, no hash map is iterated,
  and no floating-point arithmetic is used. Printing twice gives identical bytes.

### Tables of the layout engine

[`table`](crate::layout::DocBuf::table) lays rows of cells out in columns. Each column is
described by a [`ColSpec`](crate::layout::ColSpec), which gives its
[`ColAlign`](crate::layout::ColAlign): left, right or centered. The engine computes the
column widths from each column's minimum and maximum content width. When the maxima do not
all fit, each column starts at its minimum, and the width left over is shared out in whole
columns in proportion to each column's room to grow: each column first takes the whole part
of its share, then the columns with the largest fractional shares take one more column each
until nothing is left. Cells are then wrapped at their column's width and separated by two
spaces, and a line of `-` is drawn under the header rows. A cell is a
[`TableCell`](crate::layout::TableCell): its layout document and, optionally, an alignment
of its own, which replaces the column's for that cell alone. A row given as bare
[`DocId`](crate::layout::DocId)s holds cells that follow their columns' alignment.

```rust
use flm_core::layout::{ColSpec, DocBuf, Profile, print};

let mut b = DocBuf::new();
let cells: Vec<_> = ["name", "count", "alpha", "3"].iter().map(|s| b.text(s)).collect();
let t = b.table(
    &[ColSpec::left(), ColSpec::right()],
    &[&cells[0..2], &cells[2..4]],
    1, // one header row
);

assert_eq!(print(&b, t, &Profile::new(Some(40))), "name   count\n-----  -----\nalpha      3\n");
```

A table is a block: it starts on a line of its own and ends one. A table that prints
nothing (no columns, no rows, or only empty cells) is not a block, and leaves the
surrounding line unchanged. Layout marks inside table cells are not reported; a layout mark
around a whole table is.

The plain-text backend builds one layout table for each table of cells (`flm.pres.cells`): one column per column of the
grid, each cell with its own alignment, and the rows of column headers as the header rows.
The engine has no cell that spans several rows or columns: the content of a cell that spans
several positions is placed at the first position of its span, and every other position of
the span holds an empty layout document.

### Layout marks

A [`mark`](crate::layout::DocBuf::mark) only observes: it never changes the layout, and a
layout document with marks prints to the same bytes as one without.
[`print_full`](crate::layout::print_full) reports one
[`MarkSpan`](crate::layout::MarkSpan) per occurrence of a layout mark, in the order the
marks were opened. A span gives the byte range and the zero-based
[`LineCol`](crate::layout::LineCol) range of the child's printed content: from its first
content character, after any line break and indentation before it, to just after its last
content character, not counting whitespace that has not yet been written. The plain-text
backend's position marks are built from these spans.

## The HTML backend

The `html` backend ([`crate::stdcore::backends::html`]) writes a document as an HTML page
for a browser. It reads the lowered trees of the document, made mostly of the presentation elements
that every backend shares, and writes each element as HTML tags with classes and
attributes derived from the element's name and payload.

A feature's **general lowering rule** is the rule it writes once for its element to serve
every backend, marked
[`DefaultMark::Everywhere`](crate::core::lowering::DefaultMark::Everywhere) ([Choosing a
rule per backend](#choosing-a-rule-per-backend)). Because such a rule stages presentation
elements, the feature's elements are written on the page without a rule of their own for
this backend; a feature writes a rule for this backend only where the page needs something
the general rule does not write. A general rule that needs the feature's stylesheet requires
it by the feature's style identifier, which each backend resolves in its own way ([Style and
script definitions](#style-and-script-definitions) below).

One feature, [`HtmlBackend`](crate::stdcore::backends::html::HtmlBackend), registers the
backend, the output and the workflow, all three named `html`, together with the HTML
construct element `flm.pres.html.tag`, the backend's stages, tables and stylesheets, and
the formula renderer's script. A caller asks for the page
through the `html` output or the `html` workflow, and reads it from the table
`flm.output.html.document`, whose entry artifact is the page.

The chapter covers, in order:

- which elements the backend writes without a lowering rule, and when the page holds a
  paragraph;
- the **HTML contract**, the classes and attributes a stylesheet or a script may rely on,
  with the table of what is written for each presentation element;
- how each element, and each standard feature, is written on the page;
- mathematics, which is handed to a formula renderer in the reader's browser;
- escaping and the checks that keep document text from becoming markup, and the attribute
  that records where each element came from;
- the stages and tables, how stylesheets and scripts reach the page, the rules for a page
  updated in place, and the stylesheets this backend ships;
- the render options, and the plans that produce HTML alone or together with plain text.

The markup differs from the one the Python implementation of FLM writes; [Migrating from
Python FLM](crate::guide::python_flm_migration) lists the differences.

### The elements the backend writes without a lowering rule

The backend's native set
([`native_elements`](crate::stdcore::backends::html::native_elements)) holds 28 elements:

- every presentation element ([`pres::ALL`](crate::stdcore::backends::pres::ALL));
- the HTML construct element
  [`flm.pres.html.tag`](crate::stdcore::backends::html::TAG), one HTML tag that a lowering
  rule stages when no presentation element carries what it must write ([Escaping, and the
  trust guarantee](#escaping-and-the-trust-guarantee) below);
- five [structural elements](crate::lang::structural::StructuralElements) that
  need no translation ([`FLOW_THROUGH`](crate::stdcore::backends::html::FLOW_THROUGH)):
  `flm.text`, `flm.group`, `flm.comment`, `flm.container` and `flm.parbreak`.

No element of a content feature (a feature that declares elements an author writes in a
document, such as `headings`, `refs` or `xnotes`, as opposed to a backend feature such as
`HtmlBackend`) is in the set, so every such element is lowered by its feature's own rule. This matters for references. The `refs` feature's rule
for `flm.ref` reads the table of resolved references, so a plan that writes HTML schedules
the `refs` feature's resolve stage and, before it, the stage that computes each reference
point's **anchor**, the identifier a link points at
([`ANCHORS`](crate::stdcore::features::refs::ANCHORS)). A heading therefore reaches the
emitter with its anchor, and the emitter writes the anchor as the heading's `id`. In a
registry without the `refs` feature no anchor is computed, and a heading has no `id`.

### When the page holds a paragraph

The emitter writes a `<p>` for each `flm.pres.par` node and adds no paragraph of its own.
The paragraphs are built before lowering, by the block-structure pass
([`crate::lang::rewrites::blocks`]), and lowered to `flm.pres.par` nodes. Whether the
top level of a fragment is made of paragraphs depends on the layout declared for it
([`FragmentLayout`](crate::model::doc::FragmentLayout)):

- under `Blocks`, the top level is made of paragraphs and blocks;
- under `Inline`, the top level is inline content and holds no paragraph;
- under `InlineOrBlocks`, the default, the top level is inline content unless it holds a
  paragraph break or a block element, in which case it is made of paragraphs and blocks.

A fragment parsed as inline content, such as a title or a caption, is written as bare
inline HTML with no `<p>` around it. A host that wants a fragment written as paragraphs
in every case parses it with the `Blocks` layout.

### The HTML contract

The **HTML contract** is the set of class names and attributes that a stylesheet or a
script may rely on in this backend's output. Three things are outside the HTML contract,
so that the output can change them: the whitespace between tags, the order of the
attributes, and the choice of tag. A stylesheet that selects
`.flm-pres-span[data-flm-style="bold"]` keeps working whichever tag the emitter writes for
a bold span; a stylesheet that selects `strong` depends on something this backend does not
promise.

Every class and attribute name is derived by one function from a name the element or its
payload already has:

- **The class of an element.** A node's first class is the name of its presentation
  element with every hyphen doubled and every dot replaced by one hyphen
  ([`class_of_element`](crate::stdcore::backends::html::class_of_element)):
  `flm.pres.block-heading` becomes `flm-pres-block--heading`, and `flm.pres.deco.runin`
  becomes `flm-pres-deco-runin`. Because hyphens are doubled, a class converts back to
  exactly one element name (a single hyphen was a dot, a double hyphen was a hyphen), so
  two element names never produce the same class.
- **The class of a role.** A payload's role, a dotted name owned by a feature that says
  what a container is ([roles and kind
  tokens](crate::stdcore::backends::pres::payload)), becomes the
  node's second class by the same conversion
  ([`class_of_role`](crate::stdcore::backends::html::class_of_role)): `flm.heading.number`
  becomes `flm-heading-number`.
- **A `data-flm-` attribute.** A payload field that is not one of HTML's own attributes
  is written as `data-flm-` followed by the field's name, with every `_` and `.` replaced
  by `-` ([`data_attribute`](crate::stdcore::backends::html::data_attribute)). Three fields
  are written under another name. A field holding a list of kind tokens (`kinds`) is
  written as `data-flm-kind`, the attribute of a field holding a single kind token, so that
  a stylesheet selects a kind token in the same way whichever field carried it. A heading's
  `name` and `in_toc` are written as `data-flm-heading` and `data-flm-toc`, after what an
  author writes (the key `toc` of a heading's options).

Three classes are not derived from an element or a role: `flm-content` on the element
that holds the page's content, `flm-pres-list-label` on the label of a list entry, and
`flm-pres-unknown-marker` on the `\begin` and `\end` lines of an environment that no
feature declares ([`AUXILIARY_CLASSES`](crate::stdcore::backends::html::AUXILIARY_CLASSES)).
They are written by hand but follow the same conversion: read back, they give
`flm.content`, `flm.pres.list.label` and `flm.pres.unknown.marker`, each an element's name
followed by the name of one of its parts, which is the form of a role's name. So none of
them can coincide with the class of an element.

One node carries the class of another element: an image that is a block, the direct
child of a float's body, carries `flm-pres-block`, the class of a block container
(`flm.pres.block`), before its own class `flm-pres-image`. An inline image carries its own
class alone.

Because a stylesheet selects on these classes, **the name of a presentation element and
the name of a role are public interface**. Renaming either one changes the class it
produces, and every stylesheet written against the old class stops matching. A feature
that introduces a role chooses its name once and keeps it, as it would keep the name of a
public function.

**Lengths.** A payload field holding a [length](crate::stdcore::backends::pres::Length),
such as a line's indentation, is written as a **custom property** (a CSS variable) in the
element's `style` attribute. The property's name is `--flm-element-` followed by the
field's name converted as for a `data-flm-` attribute
([`style_property`](crate::stdcore::backends::html::style_property)); its value is the
length as a CSS length
([`css_length`](crate::stdcore::backends::html::conventions::css_length)). A length in
tabs is written as a multiple of the custom property `--flm-tab`, which the core
stylesheet declares on the page's root (`2em` by default); every other unit is written as
it is, since the units of the presentation vocabulary are CSS's own units and its point
is CSS's point:

```html
<div class="flm-pres-line" style="--flm-element-indent-left: calc(2 * var(--flm-tab))" data-flm-align="left">Line two</div>
```

A custom property has no effect by itself: a stylesheet applies it, so a page's own rules
decide what an indentation looks like. A length is written once, as the custom property
and never also as a `data-flm-` attribute, and a zero length writes nothing, so an
ordinary line has no `style` attribute. **The `style` attribute holds nothing else**: only
the custom properties of a payload's length fields and the ratio of two of them, and never
a CSS property, which would take precedence over every rule of a stylesheet. An image's
width and height are written the same way ([Images in HTML](#images-in-html) below). A page served with a
Content Security Policy that forbids inline styles loses these values; the page this
backend assembles already needs inline styles allowed for its `<style>` elements.

The HTML contract is written down once, as data
([`CONVENTIONS`](crate::stdcore::backends::html::CONVENTIONS)), with one row per
presentation element. The emitter reads an element's row before it writes a node of that
element, the function
[`markdown_table`](crate::stdcore::backends::html::markdown_table) prints the rows, and
tests compare the output, the rows and the table below, which that function printed:

| element | tag | classes | attributes | dropped | style definition |
|---|---|---|---|---|---|
| `flm.pres.par` | `<p>` | `flm-pres-par` | `id` ← `anchor` | — | `flm.html.style.core` |
| `flm.pres.block` | by role: `<blockquote>` for the role `flm.quote`, `<figure>` for the role `flm.floats.float`, `<figcaption>` for the role `flm.floats.float.caption`, `<nav>` for the role `flm.toc`; `<div>` otherwise | `flm-pres-block`, the role's own class | `id` ← `anchor`, `role` ← `doc_role`, `data-flm-kind` ← `kinds` | — | `flm.html.style.core` |
| `flm.pres.block-heading` | `<h1>`–`<h6>` by level counted from `root_heading_level`, `<h6>` five levels below it and deeper; a render option maps a heading kind to a tag of its own | `flm-pres-block--heading` | `id` ← `anchor`, `data-flm-heading` ← `name`, `data-flm-level` ← `level`, `data-flm-toc` ← `in_toc` | `number` (the number stands in the content) | `flm.html.style.headings` |
| `flm.pres.runin-heading` | `<span>` | `flm-pres-runin--heading` | `id` ← `anchor`, `data-flm-heading` ← `name`, `data-flm-level` ← `level`, `data-flm-toc` ← `in_toc` | `number` (the number stands in the content) | `flm.html.style.core` |
| `flm.pres.span` | by style; `<span>` otherwise | `flm-pres-span`, the role's own class | `id` ← `anchor`, `role` ← `doc_role`, `data-flm-style` ← `style`, `data-flm-kind` ← `kinds` | — | `flm.html.style.core` |
| `flm.pres.list` | `<ol>`, `<ul>` or `<dl>` by the kind `numbered`, `unnumbered` or `terms`; `<ul>` otherwise | `flm-pres-list` | `id` ← `anchor`, `data-flm-kind` ← `kinds`, each entry's `id` ← `entry_anchors` | — | `flm.html.style.lists` |
| `flm.pres.cells` | `<table>` | `flm-pres-cells` | each entry's `rowspan` ← `row_span`, each entry's `colspan` ← `col_span`, each entry's `scope` ← `header`, each entry's `data-flm-align` ← `align`, each entry's `data-flm-rules` ← `rules`, each entry's `data-flm-style-<name>` ← `styles` | `rows` (the grid's size is its shape: one `<tr>` per row, and in it one element per position a span does not cover, an empty `<td>` where there is no cell), `cols` (the grid's size is its shape, as for `rows`), `cells` (each entry is a `<td>`, or a `<th>` for a header, at its top-left position, holding its `cell` slot, its fields written as the attributes of each entry) | `flm.cells.style` |
| `flm.pres.lines` | `<div>` | `flm-pres-lines` | — | — | `flm.html.style.lists` |
| `flm.pres.line` | `<div>` | `flm-pres-line` | `style` ← `indent_left` as `--flm-element-indent-left`, `indent_right` as `--flm-element-indent-right` (custom properties, none for a zero length), `data-flm-align` ← `align` | — | `flm.html.style.lists` |
| `flm.pres.verbatim` | `<code>` inline, `<pre><code>` as a block | `flm-pres-verbatim` | `data-flm-kind` ← `kind`, `data-flm-display` ← `display`, `data-flm-lang` ← `lang` | — | `flm.html.style.verbatim` |
| `flm.pres.math-passthrough` | `<span>` | `flm-pres-math--passthrough` | `data-flm-display` ← `display`, `data-flm-env` ← `env` | — | `flm.html.style.math-passthrough` |
| `flm.pres.math-text` | `<span>` | `flm-pres-math--text` | `id` ← `anchor`, `data-flm-command` ← `command` | `at_tag` (the content is written at the position of an equation line's tag; no attribute: a pair of dollar signs is written around the element instead, inside the formula) | `flm.html.style.math-passthrough` |
| `flm.pres.link` | `<a>` | `flm-pres-link` | `href` ← `href`, `role` ← `doc_role`, `aria-label` ← `label`, `data-flm-kind` ← `kind` | — | `flm.html.style.core` |
| `flm.pres.image` | `<img>` | `flm-pres-image`, `flm-pres-block` before it where `display` is `block` | `id` ← `anchor`, `src` ← `src`, `alt` ← `alt`, `srcset` ← `srcset`, `style` ← `width` as `--flm-element-width`, `height` as `--flm-element-height`, `width` ÷ `height` as `--flm-element-aspect-ratio` (custom properties, none for a zero length) | — | `flm.html.style.core` |
| `flm.pres.unsupported` | `<span>` | `flm-pres-unsupported` | `data-flm-element` ← `element` | `message` (the reason is reported as a diagnostic, not shown in the page) | `flm.html.style.errors` |
| `flm.pres.dropped` | `<span>` | `flm-pres-dropped` | `data-flm-condition` ← `condition` | `detail` (written as the element's content, after `dropped: `, so that it shows without a stylesheet) | `flm.html.style.errors` |
| `flm.pres.unknown` | `<span>` inline, `<div>` as a block | `flm-pres-unknown` | `data-flm-kind` ← `kind`, `data-flm-name` ← `name` | `spelling` (what the author typed, written as the element's content: a macro's spelling, an environment's `\begin` and `\end` lines) | `flm.html.style.errors` |
| `flm.pres.deco.runin` | `<span>` | `flm-pres-deco-runin` | — | — | `flm.html.style.core` |
| `flm.pres.deco.final` | `<span>` | `flm-pres-deco-final` | — | — | `flm.html.style.core` |
| `flm.pres.deco.margin` | `<span>` | `flm-pres-deco-margin` | — | — | `flm.html.style.core` |
| `flm.pres.deco-mark` | none of its own | — | — | `slot` (identifies the decoration written in the mark's place) | — |
| `flm.pres.subtree` | none of its own | — | — | `tree` (identifies the document tree written in the node's place) | — |

The last column gives the **style definition** each element requires: a stylesheet
registered under that identifier ([`StyleDef`](crate::core::registry::StyleDef); [style
and script definitions](crate::guide::concepts_overview#style-and-script-definitions)). The emitter
*requires* it as it writes the element, that is, records the identifier as something the
page needs. How the stylesheet then reaches the page is decided separately, by the render
option `definitions` ([Style and script definitions](#style-and-script-definitions) below).

A **role** is written as the node's second class, after the element's own class, on the
two rows (`flm.pres.block` and `flm.pres.span`) whose classes column says so. The roles of
each standard feature are documented with that feature below.

### How each element is written

The table above gives each element's tag, classes and attributes. This section describes
the elements whose markup takes more than one tag, or holds something that the table
cannot express.

- **A block** (`flm.pres.block`) is written with the tag that its role is given in the
  backend's table of block tags,
  [`BLOCK_TAGS`](crate::stdcore::backends::html::conventions::BLOCK_TAGS): a quotation
  (role `flm.quote`) is a `<blockquote>`, a float (role `flm.floats.float`) a `<figure>`,
  the line under a float's content (role `flm.floats.float.caption`) a `<figcaption>`, the
  table of contents (role `flm.toc`) a `<nav>`, and every other block a `<div>`. No feature, configuration value or lowering rule chooses a
  tag; a role that needs a tag of its own receives a row in that table. A stylesheet
  selects a block by its role's class (`.flm-quote`), never by its tag.
- **A list** is an `<ol>`, a `<ul>` or a `<dl>`, chosen by the first of the kind tokens
  `numbered`, `unnumbered` and `terms` that the list carries
  ([`LIST_NUMBERED`](crate::stdcore::backends::pres::LIST_NUMBERED),
  [`LIST_UNNUMBERED`](crate::stdcore::backends::pres::LIST_UNNUMBERED),
  [`LIST_TERMS`](crate::stdcore::backends::pres::LIST_TERMS)), and a `<ul>` when it
  carries none of them. The tag never depends on the name of the environment that produced
  the list, so a list environment a host configures is written according to its structure.
  Each entry is an `<li>`; an entry's label, when the list has one, is a
  `<span class="flm-pres-list-label">` at the start of the `<li>`. A description list
  (`<dl>`) may hold only `<dt>` and `<dd>` children, so its label is a `<dt>` carrying the
  same class and its entry a `<dd>`. An entry's anchor is the `id` of the element that
  begins the entry, the `<li>` or the `<dt>`, which is where a link to a list item leads.
- **A table of cells** is a `<table class="flm-pres-cells">` with one `<tr>` per row of the
  grid ([`CellsPayload::grid`](crate::stdcore::backends::pres::CellsPayload::grid)), and
  in each row one element per position that no spanning cell covers. A cell is written at
  its top-left position as a `<td>`, or as a `<th>` for a header cell with `scope="col"`
  (a column header) or `scope="row"` (a row header), with `colspan` and `rowspan` where it
  spans more than one position. Its alignment is the attribute `data-flm-align`; the edges
  at which it has a rule are listed, separated by spaces, in `data-flm-rules`
  (`above left`); each custom style it carries is an attribute of its own,
  `data-flm-style-<name>` with an empty value; its content is its blocks. A position that
  no cell covers is an empty `<td>`. Nothing else is written: no class on a cell, no
  `<thead>` or `<tbody>`, and nothing that marks a cell as being at an edge of the table;
  the rules above and below the table are drawn on the `<table>` element by the cells
  stylesheet `flm.cells.style` ([The stylesheets this backend
  ships](#the-stylesheets-this-backend-ships)). The table has no identifier of its own; a link
  to it points at the `table` float that holds it.

  ```html
  <table class="flm-pres-cells">
    <tr>
      <th scope="col">
        <p class="flm-pres-par">Name</p>
      </th>
      <th scope="col">
        <p class="flm-pres-par">City</p>
      </th>
    </tr>
    <tr>
      <td colspan="2" data-flm-align="center" data-flm-style-green="">
        <p class="flm-pres-par">Both</p>
      </td>
    </tr>
  </table>
  ```

- **A dropped mark** (`flm.pres.dropped`), the node a lowering rule leaves where it left
  something out, is a `<span class="flm-pres-dropped" data-flm-condition="…">` whose text
  is `dropped: ` followed by the mark's short statement, so that it is readable on a page
  with no stylesheet; the errors stylesheet gives it the look of an error. The dropped
  marks of a table of cells are written before the table, one per thing left out.
- **Verbatim content** written as a block is `<pre><code>…</code></pre>`, with the class
  and the attributes on the `<pre>` and the content written as text, unchanged except for
  escaping.
- **A macro or an environment that no feature declares** (`flm.pres.unknown`) is shown as
  the author wrote it: a macro is a `<span class="flm-pres-unknown">` around its
  spelling; an environment is a `<div>` holding a `\begin{…}` line, the lowered body and
  an `\end{…}` line, each line a `<span class="flm-pres-unknown-marker">`. The errors
  stylesheet shows both as errors.
- **An element the backend cannot write** (`flm.pres.unsupported`) is an empty
  `<span class="flm-pres-unsupported" data-flm-element="…">`; the errors stylesheet shows
  the element's name in it through generated content. **An unresolved reference** is a
  span carrying `data-flm-style="unresolved-reference"`, which the core stylesheet shows
  as an error.
- **A document role** ([`DocRole`](crate::stdcore::backends::pres::DocRole)), what a
  block, a span or a link is among a document's notes, cited works and table of contents,
  is written as the `role` attribute: `doc-` followed by the role's name (`doc-endnotes`,
  `doc-footnote`, `doc-noteref`, `doc-backlink`, `doc-bibliography`, `doc-biblioref`,
  `doc-toc`), the roles of the
  digital-publishing vocabulary that assistive technology reads. A link's **accessible
  name** ([`LinkPayload::label`](crate::stdcore::backends::pres::LinkPayload::label)) is
  written as `aria-label`: a screen reader speaks it instead of the link's text, which
  stays what a reader sees.
- **A decoration** ([`Decoration`](crate::stdcore::backends::pres::Decoration)) of every
  kind is a `<span>` written where its decoration mark is, in the flow of the text. A
  margin decoration is written at its mark too: inside a paragraph, inside a heading's
  title, or at the start of the block that carries it. The core stylesheet places it in a
  column right of the text when the page turns that column on, and leaves it at its mark
  otherwise ([The stylesheets this backend ships](#the-stylesheets-this-backend-ships) below). An `<aside>` is not used,
  because an `<aside>` inside a `<p>` is not valid HTML.
- **A link inside another link's content** (a term link in the text of an `\href`, a
  `\ref` in the text of a `\hyperref`, an xnote mark in a heading title that a reference
  shows) is written as a `<span>` carrying the link's class and `data-flm-kind`, with no
  `href`, `role` or `aria-label`, because an `<a>` inside an `<a>` is not valid HTML and a
  browser would close the outer link early. The outer link, which the author wrote around
  the whole text, is kept and is the one a reader follows. Nothing is reported.
- **A source comment** is not written, because document text inside an HTML comment
  could close the comment.
- **A paragraph break** between blocks writes nothing: block-level elements are separate
  by themselves.
- **A paragraph with nothing to show**, whose content is empty or holds only spaces,
  source comments and paragraph breaks, writes nothing, as the plain-text backend prints
  nothing for it. An annotation hidden by its render option can leave such a paragraph
  behind. An element inside the paragraph, even an empty one, counts as something to show,
  and a paragraph that carries an anchor is always written, since a link may lead to it.
- **A tree placed inside another tree** (`flm.pres.subtree`) ends the part of the HTML
  being written and records the placed tree; the regions stage writes the placed tree's
  HTML at that point.
- **Mathematics** is not translated into HTML: a formula is written as the LaTeX the
  author wrote, for a formula renderer in the reader's browser ([Mathematics in HTML](#mathematics-in-html)
  below).

The emitter lays the bytes out for a person reading the page's source: a line break and
two spaces of indentation per level between block-level tags, nothing added inside inline
content, and nothing at all inside a `<pre>`. When the HTML of one tree is placed inside
the page, it begins on a line of its own at the indentation of the place it is written
at and keeps its own indentation inside; the backend does not indent finished HTML again,
because that would require reading its output back. The attributes are written in a fixed
order: `class`, `id`, the element's other attributes, `style`, the `data-flm-` attributes
in the order of the element's row, then the source-span attribute. The HTML contract does
not promise this order; the golden files of the tests rely on it.

### Headings in HTML

A heading set on a line of its own (`flm.pres.block-heading`) is written as `<h1>` to
`<h6>`, chosen by its level counted from the **root heading level**, the render option
[`root_heading_level`](crate::stdcore::backends::html::HtmlRenderOptions::root_heading_level)
(1 by default). A heading at the root's level is `<h1>`, a heading one level deeper
`<h2>`, and a heading five or more levels below the root `<h6>`
([`heading_depth`](crate::stdcore::backends::pres::heading_depth)). A heading's level is
a number counted from zero, smaller for outer headings; the root heading level says which
level the page writes as its first-level heading. The five heading kinds of the standard
package, `section` to `subparagraph`, are levels 1 to 5, and a host may declare kinds of
its own, deeper ones and a kind at level 0 above the sections
([`HeadingsConfig`](crate::stdcore::features::headings::HeadingsConfig)). Under the default
root, a section is `<h1>`; with a root of 0, a chapter kind declared at level 0 is `<h1>`
and a section `<h2>`; with a root of 2, the subsections of one section are the page's
first-level headings. The root changes the tags and nothing else: the numbering, the
run-in headings, the anchors and the text of a reference use the heading's own level.

A heading whose level is less than the root heading level is the Error
[`flm.pres.heading-above-root`](crate::stdcore::backends::pres::HeadingAboveRoot), and is
written as `<h1>` unless the host maps its kind to a tag (below). The plain-text output
reports the same condition under the same identifier. The `check` workflow does not
report it, because the root heading level is a render option.

Three attributes are written on every heading, regardless of its tag: `data-flm-heading`, the
name of its heading kind (`section`); `data-flm-level`, its own level; and `data-flm-toc`,
`true` or `false`, whether it is listed in a table of contents. A heading of a run-in kind
(below) carries the same attributes, and its anchor as `id`, on the `<span>` that holds its
title.

The render option
[`heading_tags`](crate::stdcore::backends::html::HtmlRenderOptions::heading_tags) maps the
name of a heading kind to a tag chosen by the host, for example `section` to `h2` on a page
whose own title is its `<h1>`. A kind that the option does not list keeps the tag of its
level. The mapping chooses the tag and nothing else: the attributes are the same, so a
stylesheet or a script that selects `[data-flm-heading="section"]` works with any tag, and
a heading above the root is reported regardless of its tag. A mapped tag that is not a valid tag
name is reported as `flm.html.invalid-name`, and the heading is written with the tag of its
level. The title above a list of xnotes ([Xnotes in HTML](#xnotes-in-html) below) is a
heading of the heading kind `xnotes` for every xnote kind, so mapping `xnotes` to `h3`
writes every such title as `<h3>`.

The heading's `id` is its **anchor**
([`ANCHORS`](crate::stdcore::features::refs::ANCHORS)), computed for every backend by the
`refs` feature. A heading
with a `\label` carries that label's anchor. A heading without a label is still a point a
reference can name, so it also carries an anchor, derived from the words of its title. An
xnote in the title is shown elsewhere and is left out of those words, so
`\section{A title with a note\footnote{…}}` has the anchor `sec--A_title_with_a_note`
([`ApartFromText`](crate::model::aspects::ApartFromText) is how an element states that some
of its content is not part of the words around it). In a registry without the `refs`
feature a heading has no `id`.

**Run-in headings.** A heading kind that a host declares as run in
([`HeadingDecl::run_in`](crate::stdcore::features::headings::HeadingDecl::run_in)) is set at
the start of the paragraph it introduces rather than on a line of its own. The lowered tree
holds a run-in decoration (`flm.pres.deco.runin`) inside the paragraph, holding a
`flm.pres.runin-heading` with the title; the backend writes the heading's `<span>` inside the
decoration's `<span>`. The stop after the title is not part of the tree's content: the
backend writes it as text inside the decoration's span, after the title, from the render
option [`run_in_stop`](crate::stdcore::backends::html::HtmlRenderOptions::run_in_stop) (a
period by default; the empty text writes no stop). The stop is therefore part of the page's
text: a reader copies it with the rest, and a host that replaces the stylesheet keeps it.
The gap between the title and the following text is set by the core stylesheet
(`--flm-runin-gap`).

The stop is written only when the title holds text and does not already end in `.`, `,`,
`:`, `;`, `?` or `!` ([`RUN_IN_STOPS`](crate::stdcore::backends::pres::RUN_IN_STOPS)), so
`Why?` is written `Why?`. The test reads the decoration's content tree
([`ends_in_sentence_punctuation`](crate::stdcore::backends::pres::ends_in_sentence_punctuation)),
and the plain-text backend uses the same test, so the two outputs of one document agree.
The test looks only at the characters the title's plain text ends in, not at the title's
last element: a title that ends in a formula, or in a box drawn by an author's own command,
receives the stop after it. A host or an extension that needs headings written
differently registers a lowering rule or a stage of its own ([lowering
rules](crate::guide::extending#lowering-rules)).

**Numbered headings.** A numbered heading carries its number inside the heading, before
the title. What precedes the title is written by the heading kind's title number format
([`HeadingDecl::title_number_format`](crate::stdcore::features::headings::HeadingDecl::title_number_format),
`#{number}. ` by default); in it, the number alone is a `<span>` with the class of the role
`flm.heading.number`, and the rest is ordinary text:

```html
<h1 class="flm-pres-block--heading" id="sec-intro" data-flm-heading="section"
  data-flm-level="1" data-flm-toc="true"><span
  class="flm-pres-span flm-heading-number">1</span>. Introduction</h1>
```

(The page writes this on one line; it is broken here to fit.) The number is part of the
heading's content, so it is inside the `<h1>` a reader copies and inside the element a
stylesheet selects, and the heading's anchor is unchanged by it. A heading that is not
numbered (`\section*{…}`, or `\section[numbered=false]{…}`) has no number, and the
headings after it continue from the last number given; it is still listed in a table of
contents (`data-flm-toc="true"`) unless it also says `toc=false`. Whether headings are
numbered at all is decided by the document: every heading counter is off unless the
document turns it on (the [numbering guide](crate::guide::numbering)).

### References in HTML

The `refs` feature's two referencing commands, `\ref{label}`, which shows what its target
has to show, and `\hyperref[label]{text}`, which shows the author's own text, are both
written as a link carrying `data-flm-kind="ref"`, whose `href` is the target's anchor
after a `#`. The kind token is an attribute rather than a class, so a stylesheet that
selects the link class keeps working when another kind of link is added. The display
options a reference may carry are applied by the feature that resolves the reference: they
change the text the backend receives, never the markup.

A `\ref` with several targets becomes several links, one per target, and the text between
them (the commas, the word "and", the ranges, the word before a list of numbers) is
written outside the links, so that each target can be followed on its own.

An unresolved reference does not stop the render. Each one is written as a span carrying
`data-flm-style="unresolved-reference"`, which the core stylesheet shows as an error, and
each is reported once, as an Error by default or as a Warning under the render option
`unresolved: allow` ([`RefsRenderOptions`](crate::stdcore::features::refs::RefsRenderOptions)),
so that an author sees every broken reference at once. No link is written for it, so the
page holds no address that leads nowhere.

A reference to a labeled line of a display-math environment leads to an empty element
inside the formula at the start of that line ([Mathematics in HTML](#mathematics-in-html) below).

### Hyperlinks in HTML

`\href{address}{text}`, `\url{address}` and `\email{address}`
([`href`](crate::stdcore::features::href)) are written as a link carrying
`data-flm-kind="url"`, whose `href` is the address as the author wrote it (with `mailto:`
before the address of an `\email`), around the text or around the address as displayed:

```html
<a class="flm-pres-link" href="https://example.com" data-flm-kind="url">Click here</a>
```

The address is checked like every address the page writes ([Escaping, and the trust
guarantee](#escaping-and-the-trust-guarantee) below): `\href{javascript:alert(1)}{click}` writes `click` in a link with no
`href` and reports `flm.html.refused-address`. The `href` feature checks no address
itself; which addresses a page accepts is decided by this backend.

### Defined terms in HTML

A term definition ([`defterm`](crate::stdcore::features::defterm)) is a block of role
`flm.defterm` carrying the definition's anchor as its `id`. Its lead-in is the run-in
decoration at the start of its first paragraph, holding a span of role
`flm.defterm.leadin`, so that a page can tell a definition's lead-in from a run-in
heading:

```html
<div class="flm-pres-block flm-defterm" id="defterm-Entropy">
  <p class="flm-pres-par"><span class="flm-pres-deco-runin"><span class="flm-pres-span flm-defterm-leadin">Entropy: </span></span>A measure of disorder.</p>
</div>
```

A term link is a link carrying `data-flm-kind="term"`, whose `href` is the definition's
anchor after a `#`, around the text written for it:
`<a class="flm-pres-link" href="#defterm-Entropy" data-flm-kind="term">entropy</a>`.
Inside the definition of its own term, a term link is a span of role `flm.defterm.term`
and no link: `<span class="flm-pres-span flm-defterm-term">Entropy</span>`. The `defterm`
feature's stylesheet, `flm.defterm.style`, sets that span in italics, as the recomposed
LaTeX sets it with `\emph`; the feature's general rule requires the stylesheet
wherever it writes a definition, and the custom property `--flm-defterm-term-style`
(`italic` by default) changes its look. A term link that nothing resolved is written as an unresolved
reference.

### Theorems in HTML

A theorem ([`theorems`](crate::stdcore::features::theorems)) is a block of role
`flm.theorems.theorem`, a `<div>` whose second class is the role's class. It carries the
anchor of its reference point as its `id` and, as its kind tokens, the name of its theorem
class and then the name of its theorem kind. Its heading is the run-in decoration at the
start of its first paragraph, holding the name in a span of role `flm.theorems.name` and
the title in a span of role `flm.theorems.title`, joined as the theorem class's heading
template says; the stop after the heading is written by the backend, as for a run-in
heading:

```html
<div class="flm-pres-block flm-theorems-theorem" id="theorem-1" data-flm-kind="theoremlike theorem">
  <p class="flm-pres-par"><span class="flm-pres-deco-runin"><span class="flm-pres-span flm-theorems-name">Theorem&#xA0;1</span> (<span class="flm-pres-span flm-theorems-title">My theorem</span>).</span>The square root of two is irrational.</p>
</div>
```

A proof's back-reference to its statement, where the proof shows one, is the reference's
link, of kind `ref`, inside the title span. The theorem class's closing mark is the final
decoration (`flm.pres.deco.final`) at the end of the last paragraph:

```html
<div class="flm-pres-block flm-theorems-theorem" data-flm-kind="prooflike proof">
  <p class="flm-pres-par"><span class="flm-pres-deco-runin"><span class="flm-pres-span flm-theorems-name">Proof</span> (<span class="flm-pres-span flm-theorems-title"><a class="flm-pres-link" href="#theorem-1" data-flm-kind="ref">Theorem&#xA0;1</a></span>).</span>We argue by contradiction.<span class="flm-pres-deco-final">□</span></p>
</div>
```

A theorem with no anchor (a proof, which accepts no label, or any theorem in a registry
without the `refs` feature) has no `id`. Where the body begins with another block, a list
or a displayed formula, the heading is in a paragraph of its own before it; where the body
ends with one, the closing mark is in a paragraph of its own after it; an empty body is one
paragraph holding both. No tag or class belongs to the feature alone: a host's own theorem
kinds and classes appear on the page as kind tokens, as the shipped ones do.

The `theorems` feature's stylesheet, `flm.theorems.style`, sets the name in bold, the rest
of the heading in the weight of the text, and the name of a kind of the `prooflike` class
in italics. The feature's general rule requires it wherever it writes a theorem.
Its custom properties:

| property | default | what it changes |
|---|---|---|
| `--flm-theorems-heading-weight` | `normal` | the weight of a theorem's heading |
| `--flm-theorems-name-weight` | `bold` | the weight of the name in the heading |
| `--flm-theorems-prooflike-name-weight` | `normal` | the weight of the name of a kind of the `prooflike` class |
| `--flm-theorems-prooflike-name-style` | `italic` | the style of the name of a kind of the `prooflike` class |

### Quotations in HTML

A quotation ([`quote`](crate::stdcore::features::quote)) is lowered by the feature's
general rule, which requires the feature's stylesheet
([`quote::html`](crate::stdcore::features::quote::html)) wherever it writes a quotation. The quotation is a block of role `flm.quote`, written as a
`<blockquote>` (from the table of block tags) for every quote environment, and carrying the
environment's name as its kind token. Each quotation section is a `<div>` whose second
class is its role's class: `flm-quote-text`, `flm-quote-lines`, `flm-quote-attributed` or
`flm-quote-block`:

```html
<blockquote class="flm-pres-block flm-quote" data-flm-kind="quote">
  <div class="flm-pres-block flm-quote-text">
    <p class="flm-pres-par">To be or not to be.</p>
  </div>
  <div class="flm-pres-block flm-quote-attributed">
    <p class="flm-pres-par">Shakespeare</p>
  </div>
</blockquote>
```

A lines section holds a run of lines, each line carrying its indentation as a custom
property of its `style` attribute ([The HTML contract](#the-html-contract) above), and no `style` attribute when it
has no indentation. An `address` environment is one lines section:

```html
<blockquote class="flm-pres-block flm-quote" data-flm-kind="address">
  <div class="flm-pres-block flm-quote-lines">
    <div class="flm-pres-lines">
      <div class="flm-pres-line" data-flm-align="left">123 Main St</div>
      <div class="flm-pres-line" style="--flm-element-indent-left: calc(1 * var(--flm-tab))" data-flm-align="left">City, ST 12345</div>
    </div>
  </div>
</blockquote>
```

No tag or class belongs to the feature alone: a stylesheet selects every quotation by
`.flm-quote`, a quotation section by its role's class, and one environment by its kind
token. The `quote` feature's stylesheet, `flm.quote.style`, which the page requires
wherever it holds a quotation, sets a quotation apart from the text around it. A `quote`
and every environment a host configures have a faint background, inner space and their
passage and lines in italics; a `blockquote` and an `address` have neither. Its custom
properties:

| property | default | what it changes |
|---|---|---|
| `--flm-quote-indent` | `2rem` | the indentation of every quotation, on both sides |
| `--flm-quote-spacing` | `1em` | the space above and below every quotation |
| `--flm-quote-background` | `rgb(120 140 150 / 0.15)` | the background of a `quote` |
| `--flm-quote-padding-block` | `1em` | the inner space above and below the content of a `quote` |
| `--flm-quote-padding-inline` | `1.5rem` | the inner space on each side of the content of a `quote` |
| `--flm-quote-passage-style` | `italic` | the style of the passage and the lines of a `quote` |
| `--flm-quote-block-rule` | `4pt solid rgb(120 140 150 / 0.3)` | the rule on the left of a block section |
| `--flm-quote-block-padding` | `1em` | the space between that rule and the section's content |
| `--flm-quote-address-size` | `0.933em` | the size of the lines of an `address` |
| `--flm-quote-attributed-align` | `right` | the alignment of the attribution |
| `--flm-quote-attributed-style` | `normal` | the style of the attribution |
| `--flm-quote-attributed-space` | `0.5em` | the space above the attribution |
| `--flm-quote-attributed-lead` | `"\2014\00A0"` (an em dash and a no-break space) | the text written before the attribution |

Each rule selects an environment by its kind token and a quotation section only as a
direct child of its quotation, so that a quotation nested in another keeps its own look.
A quotation also sets `--flm-margin-note-inset` to how far it is indented from the right
edge of the text, so that a margin decoration inside it still reaches the column of notes
([The stylesheets this backend ships](#the-stylesheets-this-backend-ships) below).

### Annotations in HTML

A reviewer's annotations ([`annotations`](crate::stdcore::features::annotations)) are
lowered by the feature's general rule, which requires the feature's stylesheet
([`annotations::html`](crate::stdcore::features::annotations::html)) wherever it writes an
annotation. A **highlight** is a span of role
`flm.annotations.highlight` in the flow of the text, whose kind tokens are the annotator's
command and number. The annotator's initials are at its start, in a margin decoration
written where its mark is, holding a span of role `flm.annotations.initials`:

```html
<p class="flm-pres-par">Some text <span class="flm-pres-span flm-annotations-highlight" data-flm-kind="phf annotator-0"><span class="flm-pres-deco-margin"><span class="flm-pres-span flm-annotations-initials" data-flm-kind="phf annotator-0">PhF</span></span>a passage</span> more text.</p>
```

A **reviewer's comment** is written in the same way under the role
`flm.annotations.comment`, with the remark in place of the passage. An annotator with no
initials has no decoration. An annotation across a paragraph break is cut into parts: one
span per paragraph, with the initials on the first part only. A part around a list or a
displayed formula is a `<div>` of the role around that block, holding the initials at its
start when it is the first part. Each part of a cut annotation also carries the kind token
`mark-continues` when a part follows it and `mark-continued` when a part precedes it; an
annotation that was not cut carries neither. The hyphen keeps these two tokens apart from
the command name beside them, which is made of letters only:

```html
<p class="flm-pres-par">Some text <span class="flm-pres-span flm-annotations-comment" data-flm-kind="phf annotator-0 mark-continues"><span class="flm-pres-deco-margin"><span class="flm-pres-span flm-annotations-initials" data-flm-kind="phf annotator-0">PhF</span></span>A comment.</span></p>
<p class="flm-pres-par"><span class="flm-pres-span flm-annotations-comment" data-flm-kind="phf annotator-0 mark-continued">With a second paragraph.</span> More text.</p>
```

Like every kind token, these two have a meaning only together with the role's class
(`.flm-annotations-comment[data-flm-kind~="mark-continues"]`); another feature's element
that carries a token of the same spelling is not selected by such a selector. Under the
render option
[`hidden`](crate::stdcore::features::annotations::AnnotationsRenderOptions::hidden),
nothing of the annotation itself is written: a highlight is written as its passage, with
its paragraphs and blocks, and a reviewer's comment is not written at all.

The `annotations` feature's stylesheet, `flm.annotations.style`, which the page requires
wherever it holds an annotation (and never under `hidden`), colors each annotation by its
annotator's number, read from the kind token `annotator-<n>`. The palette is one custom
property per number, `--flm-annotation-color-0` to `--flm-annotation-color-7`: the eight
colors that the `phfcc` LaTeX package presets, in its order, so that an annotator has the
same color on the page as in the recomposed LaTeX. An annotator numbered eight or more
takes `--flm-annotation-color-fallback`, red, the color `phfcc` gives such an annotator.
With that color:

- a **highlight**'s text is in the color, and its background is tinted with the color only
  when a page turns the tint on (`--flm-annotation-highlight-tint`, `0%` by default);
- a **reviewer's comment** is in the color, in the annotations' font family
  (`--flm-annotation-family`, sans-serif by default) and weight
  (`--flm-annotation-comment-weight`), between brackets that the stylesheet draws
  (`--flm-annotation-comment-open`, `--flm-annotation-comment-close`): the opening bracket
  on a part not marked `mark-continued` and the closing bracket on a part not marked
  `mark-continues`, so that a cut comment has one pair of brackets. A part around a block
  has its bracket on a line of its own, above or below the block;
- the **initials** are smaller than the text around them
  (`--flm-annotation-initials-size`), in the annotations' font family, upright and of
  normal weight, in a thin box of the color (`--flm-annotation-initials-border`,
  `--flm-annotation-initials-radius`, `--flm-annotation-initials-padding`,
  `--flm-annotation-initials-line-height`); they are in a margin decoration, which the
  core stylesheet places beside the text.

### Images in HTML

An image ([`graphics`](crate::stdcore::features::graphics)) is lowered by the image rule
([`image_rule`](crate::stdcore::features::graphics::image_rule)) to a `flm.pres.image`,
written as an `<img>`:

- an inline image, in the text around it, is a bare `<img class="flm-pres-image">`; an
  image that is a block, the direct child of a float's body, also carries the block class
  `flm-pres-block`, before its own class;
- its `src` is the address that the HTML backend's published table holds for the image,
  as the output's strategy decides it ([images and how an output reaches
  them](#images-and-how-an-output-reaches-them)), checked like every address the page
  writes ([Escaping, and the trust guarantee](#escaping-and-the-trust-guarantee) below);
- its `alt` is the image's alternative text;
- its size, decided as on every output and multiplied by the output's magnification
  ([images and how an output reaches them](#images-and-how-an-output-reaches-them)), is
  written as custom properties of its `style` attribute, as every length on the page is: `--flm-element-width` and `--flm-element-height`, each where the image has
  that dimension, and, where it has both in units of fixed size, their ratio
  `--flm-element-aspect-ratio`, two amounts in points separated by a slash, the form CSS's
  `aspect-ratio` property reads
  ([`css_ratio`](crate::stdcore::backends::html::conventions::css_ratio)).

```html
<img class="flm-pres-image" src="figures/plot.png" alt="A plot" style="--flm-element-width: 5cm; --flm-element-height: 2in; --flm-element-aspect-ratio: 141.73228 / 144">
<img class="flm-pres-image" src="figures/wide.png" style="--flm-element-width: 5cm">
```

The core stylesheet applies these properties. An image is shown at the size it has, at
its natural size in a dimension it does not set, and never wider than the text around it.
When it has both dimensions, its box keeps their ratio, the height following the width, so
that an image narrowed to fit a narrow page keeps its proportions; when it has a height
alone, that height is kept and the picture is fitted inside its box without distortion. A
page's own rule that reads the two properties changes how an image is shown without any
question of precedence, since the properties have no effect by themselves. No `width` or
`height` attribute is ever written. A block image is a block of the page, spaced like a
block container; an inline image flows with the text. An image whose address is refused
is written with no `src` and keeps its alternative text, and the Error
`flm.html.refused-address` is reported.

### Floats in HTML

A float ([`floats`](crate::stdcore::features::floats)) is lowered by the feature's general
rule, which requires the feature's stylesheet
([`floats::html`](crate::stdcore::features::floats::html)) wherever it writes a float. The float is a block of role
`flm.floats.float`, written as a `<figure>` (from the table of block tags) for every
float type, carrying the float type's name and `numbered` or `unnumbered` as its kind
tokens, and the anchor of its reference point as its `id`. Its content is a `<div>` of role
`flm.floats.float.content`. The line under the content, written when the float is numbered
or has a caption, is a block of role `flm.floats.float.caption`, written as a
`<figcaption>` and placed last in the `<figure>` regardless of the order in the source; its one
paragraph holds the number in a span of role `flm.floats.float.number`:

```html
<figure class="flm-pres-block flm-floats-float" id="figure-2" data-flm-kind="figure numbered">
  <div class="flm-pres-block flm-floats-float-content"><img class="flm-pres-block flm-pres-image" src="b.png"></div>
  <figcaption class="flm-pres-block flm-floats-float-caption" data-flm-kind="figure numbered">
    <p class="flm-pres-par"><span class="flm-pres-span flm-floats-float-number">Figure&#xA0;2</span>: A photo</p>
  </figcaption>
</figure>
```

No tag or class belongs to the feature alone: a stylesheet selects every float by
`.flm-floats-float`, one float type by its kind token
(`.flm-floats-float[data-flm-kind~="table"]`), and the line under an unnumbered float by
the kind tokens of the caption block, which repeat the float type and `unnumbered`. The
`floats` feature's stylesheet, `flm.floats.style`, which the page requires wherever it
holds a float, sets the float apart from the text around it: space above and below it
(`--flm-floats-spacing`, `1.5em` by default), the text of its content and of the line
under it aligned by `--flm-floats-align` (`center`), and that line slightly apart from the
content (`--flm-floats-caption-spacing`, `0.5em`) and slightly smaller than the text
(`--flm-floats-caption-size`, `0.9em`). An image that is the float's content is a block, which text
alignment does not move, so the stylesheet centers it with its side margins regardless of
the alignment; the core stylesheet's image rule keeps it within the width of the text and
keeps its proportions.

### Xnotes in HTML

An xnote ([`xnotes`](crate::stdcore::features::xnotes)) is a piece of text set apart from
the running text, marked where it belongs and shown elsewhere or in place, such as a
footnote. On the page an xnote is written in the same shapes as on every other output
([Xnotes](#xnotes) under the plain-text backend above), built by the same lowering code of
the `xnotes` feature. The feature's rules for this backend
([`xnotes::html`](crate::stdcore::features::xnotes::html)) differ from its general rules in
three ways: they add a link back from each entry of a list of xnotes to its mark; they
write no bracket text around an xnote shown in place, since the feature's stylesheet draws
the brackets; and they require that stylesheet. Every element below is a presentation element written as
its row of the HTML contract says.

**The xnote mark** of an xnote shown in a list is a `<sup>` carrying the mark's
identifier, around a link to the xnote. The link carries the document role `doc-noteref`
and an accessible name made of the kind's word and the xnote's number, so that a screen
reader announces "Footnote 1" rather than "1":

```html
<p class="flm-pres-par">The claim holds in every case.<sup class="flm-pres-span flm-xnotes-xnote-mark" id="fnref--1yuhk" data-flm-style="superscript"><a class="flm-pres-link" href="#fn-1" role="doc-noteref" aria-label="Footnote 1" data-flm-kind="xnote">1</a></sup> The next sentence goes on.</p>
```

The mark's identifier ends in four letters derived from its position, as does the anchor of
every reference point without a label. A mark style that a host configures is used here
too: the mark style `[#{arabic}]` writes `<sup …><a …>[1]</a></sup>`.

**The list** of an xnote kind is placed after the trees, inside `<main>` (see
[`Piece`](crate::core::piece::Piece)). It is a block of the document role `doc-endnotes` carrying the kind
tokens `xnotes` and the kind's name, holding the title as a block heading and a labeled
list whose entries carry the xnotes' identifiers. The last paragraph of each entry ends,
after a space, with the link back to the mark: its document role is `doc-backlink`, its
accessible name is `Back to ` followed by the xnote's name as a reference writes it, and
its text is the kind's [`backlink`](crate::stdcore::features::xnotes::XnoteKind::backlink)
text (`↩`). Both accessible names use the kind's configured words as written: the mark's
name uses the kind's capitalized variant (its usual word when it has none), and the link
back's name uses its usual word. A list of footnotes with two entries:

```html
<div class="flm-pres-block flm-xnotes-footnote-list" role="doc-endnotes" data-flm-kind="xnotes footnote">
  <h2 class="flm-pres-block--heading" data-flm-heading="xnotes" data-flm-level="2" data-flm-toc="false">Footnotes</h2>
  <ol class="flm-pres-list" data-flm-kind="numbered xnotes footnote labeled">
    <li id="fn-1"><span class="flm-pres-list-label"><sup class="flm-pres-span flm-xnotes-entry-mark" data-flm-style="superscript">1</sup></span>
      <p class="flm-pres-par">Except the empty one, which we exclude by convention. <a class="flm-pres-link" href="#fnref--1yuhk" role="doc-backlink" aria-label="Back to Footnote 1" data-flm-kind="xnote-back">↩</a></p>
    </li>
    <li id="fn-2"><span class="flm-pres-list-label"><sup class="flm-pres-span flm-xnotes-entry-mark" data-flm-style="superscript">2</sup></span>
      <p class="flm-pres-par">First paragraph.</p>
      <p class="flm-pres-par">Second paragraph. <a class="flm-pres-link" href="#fnref--2yurg" role="doc-backlink" aria-label="Back to Footnote 2" data-flm-kind="xnote-back">↩</a></p>
    </li>
  </ol>
</div>
```

The kind token `labeled` lays the list out as every labeled list is laid out ([The
stylesheets this backend ships](#the-stylesheets-this-backend-ships) below). The list of an unnumbered kind carries
`unnumbered` instead of `numbered`, and its labels are the kind's fixed mark. An entry
whose content ends with a list has its link back in a paragraph of its own after that
list; a kind whose `backlink` text is empty writes no link back. The title is written
with the tag of its level, or with the tag that the render option
[`heading_tags`](crate::stdcore::backends::html::HtmlRenderOptions::heading_tags) maps
`xnotes` to ([Headings in HTML](#headings-in-html) above).

**An xnote shown in place** whose content is inline is one span at the place where the
xnote is written. The span carries the xnote's identifier, the document role
`doc-footnote` and the kind tokens, and holds the mark's `<sup>` with no link (the xnote
is right there) and no bracket text:

```html
…every case. <span class="flm-pres-span flm-xnotes-xnote-inplace" id="fn-1" role="doc-footnote" data-flm-kind="xnotes footnote"><sup class="flm-pres-span flm-xnotes-xnote-mark" id="fnref--1yuhk" data-flm-style="superscript">1</sup>Except the empty one, which we exclude by convention.</span> The next…
```

An xnote shown in place whose content holds paragraphs is a block between the two halves
of the paragraph it interrupts, with the mark at the start of its first paragraph:

```html
<p class="flm-pres-par">Two paragraphs.</p>
<div class="flm-pres-block flm-xnotes-xnote-inplace" id="fn-1" role="doc-footnote" data-flm-kind="xnotes footnote">
  <p class="flm-pres-par"><sup class="flm-pres-span flm-xnotes-xnote-mark" id="fnref--1yuhk" data-flm-style="superscript">1</sup>First paragraph.</p>
  <p class="flm-pres-par">Second paragraph.</p>
</div>
<p class="flm-pres-par">And on.</p>
```

The mark of an unnumbered kind with no mark text is an empty `<sup>`, so that its
identifier still exists. In a registry without the `refs` feature no element carries an
identifier, the mark has no link and no entry has a link back.

**The stylesheet** `flm.xnotes.style` belongs to the `xnotes` feature and is required by
every rule of an xnote and of a list of xnotes, so a page with no xnote does not carry it.
It draws a rule above a list of xnotes and the brackets around an xnote shown in place,
and nothing more. It selects a list by its document role as well as its kind token,
because an xnote shown in place as a block carries the `xnotes` token too. Its custom
properties:

| property | default | what it changes |
|---|---|---|
| `--flm-xnotes-list-rule` | `1px solid #999` | the rule above a list of xnotes |
| `--flm-xnotes-list-size` | `0.9em` | the size of a list's text |
| `--flm-xnotes-list-space` | `2em` | the space above a list |
| `--flm-xnotes-inplace-open` | `"["` | the opening bracket of an xnote shown in place |
| `--flm-xnotes-inplace-close` | `"]"` | the closing bracket of an xnote shown in place |

**No script is shipped.** An xnote shown in place is always written whole, with every
identifier, document role and kind token as an attribute, so that a page's own stylesheet
or script can lay it out, hide it or show it on request; a script shipped by the library
could interfere with the page's own scripts. A host adds a stylesheet or a script of its
own as a style or script definition ([Style and script definitions](#style-and-script-definitions) below): a feature of
the host declares it with
[`style_def`](crate::core::registry::FeatureBuilder::style_def) or
[`script_def`](crate::core::registry::FeatureBuilder::script_def) and requires it from a
rule of its own or through the render option `required_definitions`. A host can also
replace the content of `flm.xnotes.style` through the render option `definitions`, or
assemble a page of its own around the HTML of the trees.

### The table of contents in HTML

The table of contents ([`toc`](crate::stdcore::features::toc); [The table of
contents](#the-table-of-contents) under the plain-text backend above) is written on the
page with the same presentation elements as on every other output, each written as its
row of the HTML contract says. Three rows decide its markup:

- The root is a block of role `flm.toc`, which the contract's tag table writes as a
  `<nav>` ([`BLOCK_TAGS`](crate::stdcore::backends::html::conventions::BLOCK_TAGS)), carrying the
  document role `doc-toc`, the document role that the digital-publishing roles define for a
  table of contents.
- Each list is a `<ul>` with the kind tokens `unnumbered` and `toc`. Its labels are empty,
  so no label span is written.
- Each entry is an `<li>` holding a link to its heading's anchor, of link kind `toc`; inside
  the link stand the number, in a `<span>` of role `flm.toc.entry.number`, a space, and the
  title. The list of sub-entries follows the link, inside the same `<li>`.

For `\section{One}` and `\subsection{Two}` under numbering:

```html
<nav class="flm-pres-block flm-toc" role="doc-toc" data-flm-kind="toc">
  <ul class="flm-pres-list" data-flm-kind="unnumbered toc">
    <li><a class="flm-pres-link" href="#sec--One" data-flm-kind="toc"><span class="flm-pres-span flm-toc-entry-number">1</span> One</a>
      <ul class="flm-pres-list" data-flm-kind="unnumbered toc">
        <li><a class="flm-pres-link" href="#sec--Two" data-flm-kind="toc"><span class="flm-pres-span flm-toc-entry-number">1.1</span> Two</a></li>
      </ul>
    </li>
  </ul>
</nav>
```

An unnumbered heading's entry holds the title alone, with no number span and no space. An
entry whose heading has no anchor (in a registry without the `refs` feature) holds its
content without the link. A reference or an xnote mark inside a title is written as a
`<span>` rather than as a link, as every link inside another link's content is (see [The
HTML contract](#the-html-contract) above). The anchors are the headings' own (`#sec--One` is the anchor that the
`headings` feature derives from the title `One`).

On the page assembled by the library, the table of contents is placed inside `<main>`
before the trees, after the metadata's pieces, as every piece placed before the content
is ([Pieces](#pieces)). A compose template of a host places it where it chooses: the
program's compose template `simple` places it between the page's header and its main
content, inside the `<div id="flm-region-flm.toc">` that wraps every piece.

**The stylesheet** `flm.toc.style` ([`toc::html`](crate::stdcore::features::toc::html))
belongs to the `toc` feature and is required by the feature's general rule of the root, so a page
without a table of contents does not carry it. It removes the list markers and the lists'
own margins and padding, and indents each nested list by one step; the space after an
entry's number is the text the entry writes, not a rule of the stylesheet. Its custom
property:

| property | default | what it changes |
|---|---|---|
| `--flm-toc-indent` | `1.5em` | the indentation of each nested list |

### Citations in HTML

A citation chunk (adjacent `\cite` commands read as one;
[`citations`](crate::stdcore::features::citations)) is lowered by the feature's general
rules: the citation mark is a span, its numbers are links, and the list of cited works is
a block holding a heading and a labeled list ([Citations](#citations) under the plain-text backend
above). The feature's general rules require the feature's stylesheet
([`citations::html`](crate::stdcore::features::citations::html)) wherever they write a
citation or the list of citation entries.

`Codes of this family are well studied \cite{a,b}.` writes a no-break space and then the
citation mark: a span of role `flm.citations.mark` whose kind token `attached` says that it
belongs to the text before it. Each number is a link of kind `cite` to its citation entry,
with the document role `doc-biblioref`; the delimiters and the join texts are outside the
links:

```html
<p class="flm-pres-par">Codes of this family are well studied<span class="flm-pres-span flm-citations-space">&#xA0;</span><span class="flm-pres-span flm-citations-mark" data-flm-kind="attached">[<a class="flm-pres-link" href="#citation-1" role="doc-biblioref" data-flm-kind="cite">1</a>,<a class="flm-pres-link" href="#citation-2" role="doc-biblioref" data-flm-kind="cite">2</a>]</span>.</p>
```

The space before an attached mark is a character of the text, in a span of role
`flm.citations.space` of its own before the mark's span: it is part of the text a reader
copies, the line does not break between the word and the mark, and a stylesheet selects
the span by its class `flm-citations-space` (to hide the space or change its width).
Directly after an element that the browser lays out as one solid box in the line, such as
an image or an inline block, the line may still break before the mark, as CSS specifies
for every such box. Whether a mark has a space before it follows from the form of the
mark ([`MarkForm`](crate::stdcore::features::citations::MarkForm)) and the placement of the
entries ([`CitationPlacement`](crate::stdcore::features::citations::CitationPlacement)),
both set in the `citations` feature's configuration: a space under
the bracketed form and under the in-place placement, none under the superscript form of
the listed placement.

`\R` writes its word in a span of role `flm.citations.word`, then its mark with the kind
token `standalone`, and nothing before the word:
`As shown in <span class="flm-pres-span flm-citations-word">refs.&#xA0;</span><span class="flm-pres-span flm-citations-mark" data-flm-kind="standalone">[…]</span>`.

Under the superscript form of the listed placement, the mark of a `\cite` is a `<sup>`
(the span carries the superscript display style), with no space before it and no
delimiters:
`studied<sup class="flm-pres-span flm-citations-mark" data-flm-style="superscript" data-flm-kind="attached"><a …>1</a>–<a …>3</a></sup>`.
The mark of an `\R` is never raised. Under the in-place placement, the mark carries the
kind tokens `attached in-place` (or `standalone in-place`) and holds the citation entries
themselves in their brackets, with no link; a missing citation entry is a dropped mark
inside the brackets.

A citation mark inside content that a reference shows again, such as a heading title shown
by a `\ref`, is inside the reference's link; its numbers, which are links inside that
link, are written as their text ([How each element is written](#how-each-element-is-written) above).

The list of cited works is placed after the lists of xnotes: a `<div>` of the document role
`doc-bibliography` holding the title as a heading and a labeled list whose entries carry
the works' identifiers. The entries carry no document role, since the
digital-publishing role once defined for a bibliography entry is deprecated:

```html
<div class="flm-pres-block flm-citations-bibliography" role="doc-bibliography" data-flm-kind="citations">
  <h2 class="flm-pres-block--heading" data-flm-heading="citations" data-flm-level="2" data-flm-toc="false">References</h2>
  <ol class="flm-pres-list" data-flm-kind="numbered citations labeled">
    <li id="citation-1"><span class="flm-pres-list-label"><span class="flm-pres-span flm-citations-entry-number">[1]</span></span>
      <p class="flm-pres-par">D. Knuth, <em class="flm-pres-span" data-flm-style="emph">The Art of Computer Programming</em>, 1984</p>
    </li>
  </ol>
</div>
```

**The stylesheet** `flm.citations.style` belongs to the `citations` feature and is
required by the rules of `\cite` and `\R` for this backend, so a page with no citation
does not carry it. It keeps a raised citation mark from moving its line away from the
line above, and nothing more; a mark wraps where text wraps. The raising itself is done by
the core stylesheet, for every span of the superscript display style. Its one custom
property:

| property | default | what it changes |
|---|---|---|
| `--flm-citations-raised-line-height` | `0` | the line height of a raised citation mark, which keeps its line from moving away from the line above |

### Verbatim content in HTML

Verbatim content, the text an author wrote taken exactly as written, is a `<code>` element
inline and `<pre><code>…</code></pre>` as a block, with the class and the attributes on
the outer element. Three attributes describe it: `data-flm-kind`, the verbatim kind:
`text`, `code`, or `a` (the kind of `\verba`, for an identifier such as a keyword or a
name; [`VerbatimKind`](crate::stdcore::backends::pres::VerbatimKind)); `data-flm-display`, inline or block; and
`data-flm-lang`, the language tag, where the author gave one.

The content is written unchanged except for the escaping HTML requires everywhere (`&`,
`<`, `>`); no character is decorated, wrapped or replaced. A backslash is written as a
backslash, and nothing has to be hidden from the formula renderer, because the shipped
configuration of the renderer looks only inside the elements that hold mathematics (the
[module that declares the renderer's script](crate::stdcore::backends::html::mathjax)
lists what that configuration says).

No stylesheet or rule of this library reads `data-flm-lang`. It is written so that a
host's stylesheet, or a feature that highlights syntax, can use the language tag the
author gave.

### User definitions in HTML

A document may define its own macros and environments
([`crate::lang::rewrites::userdefs`]). No rule of the `userdefs` feature is written for
this backend, so the feature's general rule applies: the expansion rewrite runs before
lowering, and the emitter never receives a use site. The page holds the definition's
template filled with the use site's arguments, lowered and escaped like any other
content. This holds in text and inside
mathematics alike, so a formula renderer in a browser, which has none of the author's
macros, receives formulas whose user definitions are already expanded. User definitions
are not passed to the formula renderer as macros of its own.

### Mathematics in HTML

What a formula becomes on this backend is decided by the *processing feature* of the
registry, the feature that reads mathematics and registers the lowering rules of the
formulas (the `math` feature only introduces mathematics into the language; see
[mathematics](crate::guide::language_syntax#mathematics)). This section describes the
library's math processor, [`math-passthrough`](crate::math_passthrough), which
carries each formula's source to the page unread. The `math-mathjax` extension, a
processing feature written in JavaScript, writes formulas its own way ([mathematics
processed by MathJax](crate::guide::command_line#mathematics-processed-by-mathjax)). In a
registry with no processing feature, the rule of last resort of the `lowering` feature
([`MathGroupRule`](crate::stdcore::backends::lowering::MathGroupRule)) reports every math
group as the Error `flm.math.unprocessed`
([`MathUnprocessed`](crate::stdcore::backends::lowering::MathUnprocessed)) and writes its
content as ordinary text, and the `math` feature's own rule of last resort
([`unprocessed`](crate::stdcore::features::math::unprocessed)) does the same for every
display-math environment.

Under `math-passthrough`, the backend does not translate a formula into HTML. It writes
the formula whole, as the LaTeX the author wrote, for a **formula renderer**: a script in
the reader's browser that reads LaTeX out of the page and typesets it. The renderer this
backend ships is MathJax ([`mathjax`](crate::stdcore::backends::html::mathjax)). Every
formula, inline or displayed, is a `<span>` (`flm.pres.math-passthrough`, the
presentation element of a formula's unread source) carrying the class, `data-flm-display`
and, where the author wrote a mathematics environment, `data-flm-env` with the
environment's name. Inside it are the delimiters this backend chose and, between them,
the document's own source, written as text like any other document text:

```html
<span class="flm-pres-math--passthrough" data-flm-display="inline">\(x^2 + y\)</span>
<span class="flm-pres-math--passthrough" data-flm-display="display" data-flm-env="align">\begin{align}a &amp;= b \\
c &amp;= d\end{align}</span>
```

The delimiters are `\(…\)` for inline mathematics, `\[…\]` for displayed mathematics
without an environment, and `\begin{…}…\end{…}` for an environment. A displayed formula
is distinguished from an inline one by `data-flm-display`, and the math stylesheet makes
it look like a block: on a line of its own, with margins, and scrolling sideways when it
is wider than the text. Both use a `<span>` because FLM treats displayed mathematics as
part of the text around it, so `\[…\]` may appear between two runs of text in one
paragraph, and HTML allows no block element such as a `<div>` inside a paragraph: a
browser would close the paragraph before the formula. Nothing wraps a line and nothing is
rearranged, so a multi-line formula is aligned at its `&` marks by the renderer, exactly
as the author wrote it. The formula's element has no identifier of its own.

Writing a formula **requires** two definitions ([Style and script definitions](#style-and-script-definitions) below):
the style definition `flm.html.style.math-passthrough` that the mathematics rows of the HTML contract
give (the stylesheet of a displayed formula and of content written inside a formula, which
a page whose formulas the `math-mathjax` extension writes requires too, for its text
inside formulas), and the script definition `flm.html.mathjax`, the formula renderer itself. The
emitter requires the script at the first formula of each tree, so a document with no
mathematics loads no renderer. The page loads the renderer from the network unless the
host decides otherwise through the render option `definitions`.

#### Content inside a formula

A word of text inside a formula (`\text{…}` and the other text commands of mathematics),
a link, or a point in the formula that a reference can lead to is ordinary document
content, lowered like content anywhere else, as a `flm.pres.math-text` node. The formula
renderer offers one way to put HTML inside a formula: a `<tex-html>` element, whose
content the renderer takes out of the characters of the formula and inserts into the
typeset result. This backend writes each `flm.pres.math-text` node inside a formula that
way, with the lowered content inside a `<span>`:

```html
<span class="flm-pres-math--passthrough" data-flm-display="inline">\(x = <tex-html><span
  class="flm-pres-math--text" data-flm-command="text">one half</span></tex-html> + y\)</span>
```

The renderer takes exactly one element out of a `<tex-html>` and drops the attributes of
the `<tex-html>` tag itself, so the class, the identifier and the command are written on
the `<span>` inside it. A node with an identifier and no content is that same `<span>`,
empty: an invisible point in the formula that a link can lead to, which is how the start
of an equation line is marked. The math stylesheet keeps an empty one at zero size, so
that it moves nothing.

#### What `math-passthrough` stages for this backend

The rules that decide what a formula holds belong to the processing feature, not to this
backend: a backend refers to no content feature, while a content feature may write a rule
for a backend. `math-passthrough` registers two rules that serve this backend and the
text backend alike: [`MATH_RULE`](crate::math_passthrough::MATH_RULE)
(`flm.math.lower.passthrough`), for inline mathematics and for displayed mathematics
without an environment, and
[`ENVIRONMENT_RULE`](crate::math_passthrough::ENVIRONMENT_RULE), for
the six display-math environments, which handles the equation lines (their bodies are in
[`source`](crate::math_passthrough::source)). Both rules hand this
backend a formula whose source is what the author wrote, with three differences.

**A text command becomes content.** `\text{…}`, `\textbf{…}`, `\mbox{…}` and the other
text commands of mathematics read their argument in text mode, so the argument is document
content. The rule does not copy such a command's source: it lowers the command like any
other node, and this backend receives the `flm.pres.math-text` node described above,
holding the argument lowered as inline content. A reference written in the argument is a
real link on the page, and a special character is escaped like any other document text.
This holds wherever the command is written: among the formula's other nodes, inside a
group as in `x_{\text{eff}}`, or inside a nested environment such as `cases`, because the
rule walks the whole formula and writes everything that is not a text command back in its
LaTeX syntax.

**Each equation line begins with a point a reference can lead to.** Every line of a
display-math environment is an equation line, a reference point that a `\ref` can name,
and the formula's element has no identifier. The rule therefore stages an empty
`flm.pres.math-text` node at the start of each line, carrying that line's anchor; this
backend writes it as the empty `<span>` above. Every line has an anchor, labeled or not: a
labeled line has the anchor of its label (`eq-x`), and every other line has the prefix
`eq`, a double hyphen, its position among the document's equation lines and four letters
([`auto_anchor`](crate::stdcore::features::refs::auto_anchor)). The anchors come from the
`refs` feature; in a registry without it there are no anchors, no such node is staged, and
nothing is reported.

The rule does **not** write back a line's `\label` commands, nor its `\nonumber`: a
formula renderer has no use for a label, and a line that takes no number has no number
to suppress. (`\notag` is not a command of an equation line: the `math` feature does not
declare it, so inside a body it is an unknown command kept as source, and it is written
back with the line.) The `\\` line breaks, the alignment marks, the whitespace and the
source comments are kept as the author wrote them, so a multi-line formula is aligned
exactly as written.

```html
<span class="flm-pres-math--passthrough" data-flm-display="display" data-flm-env="align">\begin{align}<tex-html><span
  class="flm-pres-math--text" id="eq-x"></span></tex-html>
  x &amp;= y \\<tex-html><span class="flm-pres-math--text" id="eq--2oqua"></span></tex-html>
  z &amp;= w
\end{align}</span>
```

**A line's tag is content inside the formula.** A line shows its name at the end of the
line: the content of a `\tag` the author wrote, or the number the document gave the line.
Both are written the same way: the LaTeX `\tag*{`, a `flm.pres.math-text` node marked as
being at a tag's place
([`MathTextPayload::at_tag`](crate::stdcore::backends::pres::MathTextPayload::at_tag)),
and `}`. This backend writes a pair of dollar signs around the `<tex-html>` element it
writes for such a node. An author's `\tag{A}` puts the parentheses that FLM reads into the
unstarred form inside that content, and a number arrives with the delimiters of its
counter style. A line of `equation*`, `align*` or `gather*`, a line marked `\nonumber`, and
every line under a configuration of the `math` feature that numbers no equations are
written with no tag.

```html
<span class="flm-pres-math--passthrough" data-flm-display="display" data-flm-env="equation">\begin{equation}<tex-html><span
  class="flm-pres-math--text" id="eq-euler"></span></tex-html>
  e^{i\pi} + 1 = 0 \tag*{$<tex-html><span
  class="flm-pres-math--text">(1)</span></tex-html>$}\end{equation}</span>
```

#### What this backend changes inside a formula

The formula renderer reads the characters of the whole formula, including the HTML
written inside it and the attribute values, *before* it parses any LaTeX, and escaping
hides nothing from it: it reads the text a browser built from the page, in which `&lt;` is
`<` again. A document must therefore not be able to write the renderer's `<tex-html>` tag
or the formula's closing delimiter where the renderer would read them. This backend
changes three things to that end; they are the emitter's work rather than a lowering
rule's, because only the emitter has written the delimiter that ends the formula.

- In a document's own **mathematics source**, `<tex-html` and `</tex-html` are written
  with an empty TeX group after the `<` (`<{}tex-html`), so that a document cannot use the
  mechanism by typing its tag. The group typesets nothing and keeps the spacing of the
  `<`, so the reader sees the same characters.
- In the **text written inside a `<tex-html>`**, an empty `<span></span>` is written inside
  every occurrence of the closing delimiter of the surrounding formula (`\)`, `\]` or
  `\end{align}`) and of the two tag spellings: `write \<span></span>) here`. An empty
  element shows nothing and keeps the renderer from ending the formula there.
- An **attribute value** written inside a `<tex-html>` cannot be changed that way, because
  an attribute value is text and an element written into it would reach the reader as
  characters. An attribute whose value holds one of those sequences (for example an
  identifier derived from a label the author wrote) is left out; the element and its
  content are written as usual, and the Error `flm.html.refused-in-math` gives the
  attribute and the sequence. The attributes of a construct node ([Escaping, and the trust
  guarantee](#escaping-and-the-trust-guarantee) below) are checked in the same way. A body that a lowering rule hands over as
  already-formed markup is written unchanged; the rule's author must ensure that it holds
  neither the closing delimiter nor a `<tex-html>` tag.

The content of a formula is not inspected ([Mathematics written by authors the host does
not trust](#mathematics-written-by-authors-the-host-does-not-trust) below): these changes protect only the backend's own `<tex-html>` mechanism.

#### A formula inside content inside a formula

`\text{if \(x>0\) holds}` puts a formula inside content that is itself inside a formula.
The renderer does not read again the HTML it inserts, so the inner formula cannot be
written as HTML there. Where the inner formula is among the content's own nodes, this
backend **splits** the content: it closes the `<tex-html>` being written, writes the inner
formula's source into the surrounding formula with no delimiters of its own (the
characters are already in mathematics, which is what `\text{if \(x>0\)}` means in LaTeX),
and opens a new `<tex-html>` for what follows. The identifier and the command are written
on the first part only, since an identifier may occur only once in a page; the class is
written on every part.

```html
\(y = <tex-html><span class="flm-pres-math--text" id="eq:cond"
  data-flm-command="text">if </span></tex-html>x &gt; 0<tex-html><span
  class="flm-pres-math--text"> holds</span></tex-html>\)
```

A formula nested more deeply, inside a styled span inside the content
(`\text{\emph{\(x\)}}`), is written the same way one level further out: every element
open inside the `<tex-html>` is closed, then the `<tex-html>` itself; the inner formula's
source is written into the surrounding formula; then the `<tex-html>` and the same
elements are opened again. Each element opened again carries the classes and attributes
it had, except its identifier, which stays on its first half. Nothing is dropped and
nothing is reported.

```html
\(y = <tex-html><span class="flm-pres-math--text" id="eq:deep"
  data-flm-command="text"><em class="flm-pres-span" id="em-1"
  data-flm-style="emph">if </em></span></tex-html>x<tex-html><span
  class="flm-pres-math--text"><em class="flm-pres-span"
  data-flm-style="emph"> holds</em></span></tex-html>\)
```

### Escaping, and the trust guarantee

**Every byte this backend writes outside a mathematics region is escaped or checked by
the library.** Inside a mathematics region that `math-passthrough` writes, the author is
trusted: that feature does not inspect the content of a formula ([Mathematics
written by authors the host does not trust](#mathematics-written-by-authors-the-host-does-not-trust) below). The rest of this section concerns
everything outside mathematics.

Every byte this backend writes goes through one typed sink
([`HtmlSink`](crate::stdcore::backends::html::HtmlSink)), whose entry points differ by the
kind of content they write: document text, an attribute value, an attribute value that is
a piece of JSON, a piece of already-formed markup, or the content of a `<style>` or a
`<script>`. Text that comes from a document reaches the page through the first three
entry points only, and each escapes where it writes. The sink offers no way to read back
what it wrote, so no text can be escaped twice and no decision can depend on bytes already
written; it reports only its own state (how many bytes it has written, and the tag of the
element it would close next).

- **Text** has `&`, `<` and `>` escaped, and eight invisible space characters (the
  no-break space and seven others,
  [`SPACE_REFERENCES`](crate::stdcore::backends::html::escape::SPACE_REFERENCES)) written
  as numeric character references such as `&#xA0;`, so that a space a reader cannot see is
  still apparent in the page's source, and so that a parser that reads no named HTML references, an
  XML parser included, reads the output.
- **An attribute value** between double quotation marks has `&`, `<`, `>` and `"`
  escaped, always, so the output is valid regardless of what the value holds.
- **An attribute value that is a piece of JSON** is written between single quotation
  marks and has `&` and `'` escaped; its double quotation marks are written as themselves,
  so the raw page shows the JSON as JSON ([The source-span attribute](#the-source-span-attribute) below).

Escaping applies inside `<pre>` and `<code>` as everywhere else.

Unescaped bytes have exactly three sources, none of them a document:

- the syntax that the sink's tag builder writes itself;
- the content of a style or script definition, or what a host put in its place, written
  in one place, the assembly of the page;
- [`Markup::trusted`](crate::stdcore::backends::html::Markup::trusted), the call by which
  an extension puts HTML that it takes responsibility for into the body of a construct
  node.

A test of this library reads its own source code and fails if one of these three sources
of unescaped bytes is used anywhere else.

Some values are checked rather than escaped, because escaping is not what makes them
safe.

**Addresses.** A link's `href`, an image's `src` and each candidate of its `srcset` must
pass one policy ([`check_address`](crate::stdcore::backends::html::check_address)): a
fragment (`#…`), a relative address, or an address with one of the schemes `http`,
`https`, `mailto`, `ftp` and `tel`; for an image, also a `data:` address holding a
picture. Every other address, `javascript:` above all, is refused: the attribute is left
out, the link keeps its text and the image its alternative text, and the Error
`flm.html.refused-address` gives the scheme. The list is fixed data, not a setting.

**Tag and attribute names.** A tag or attribute name is valid when it is an ASCII letter
followed by ASCII letters, digits and `-` (an attribute name may also hold `_`, `:` and
`.`). Three kinds of name are checked, each reported as the Error `flm.html.invalid-name`
when it is not valid:

- the tag and the attribute names of a construct node, which a lowering rule wrote; the
  node is then written as the empty `flm.pres.unsupported` span;
- a tag that the render option `heading_tags` maps a heading kind to; the heading is then
  written with the tag of its level;
- the attribute `data-flm-style-<name>` of a custom style a cell carries; that attribute
  is then left out.

**Roles and kind tokens.** A role is written as one class, and a kind token as one token
of `data-flm-kind`; both are names that a lowering rule wrote. Both attributes separate
their tokens by whitespace, so a name that holds whitespace, or an empty one, would be
read as several tokens or as none. Such a name is refused
([`is_valid_token`](crate::stdcore::backends::html::is_valid_token)): that one class or
that one token is left out, and where every kind token of the node is refused, the
`data-flm-kind` attribute is not written at all. Everything else about the node is
written, and the Error `flm.html.invalid-name` gives the name and says which of the two
happened.

**Construct nodes.** The HTML construct element `flm.pres.html.tag` is one HTML tag that a
lowering rule stages, with its attributes and either child nodes or a body of
already-formed markup ([`HtmlTagPayload`](crate::stdcore::backends::html::HtmlTagPayload)).
The HTML contract and the address policy do not apply to it: the emitter writes the tag,
the attributes and the body as the payload gives them and adds nothing of its own, so the
rule's author is responsible for the attributes and addresses it writes. Inside a formula
the attribute values are checked like every attribute written there ([What this backend
changes inside a formula](#what-this-backend-changes-inside-a-formula) above), and a markup body must hold neither the formula's closing
delimiter nor a `<tex-html>` tag. [Extending FLM](crate::guide::extending) shows a rule
that stages a construct node.

### The source-span attribute

With the render option `source_spans` on, every element that the emitter writes for a node
with a position in a source that has a name (an origin with a path,
[`FlmOrigin`](crate::lang::FlmOrigin)) carries the attribute `data-flm-source-span`, which
gives that position. Inline and block-level elements carry it alike:

```html
<p class="flm-pres-par" data-flm-source-span='["book/ch1.flm",0,15]'>One <em class="flm-pres-span" data-flm-style="emph" data-flm-source-span='["book/ch1.flm",4,14]'>two</em>.</p>
```

The value is a JSON array of three items: the source's canonical path, the byte at which
the range begins, and the byte just past its end. Because it is JSON, a script needs no
parsing of its own: `JSON.parse(element.dataset.flmSourceSpan)` on the paragraph above
returns `["book/ch1.flm", 0, 15]`. It is the only attribute of this backend written between
**single** quotation marks, which HTML allows, so that the JSON's double quotation marks
are written as themselves. The value is still escaped (an `&` becomes `&amp;` and a `'`
becomes `&#39;`), so a source path holding either character reaches a script intact.

A lowering rule stages its nodes on a source that the library creates for the purpose,
which refers back to the node the rule was lowering; the emitter follows these references
back to the document to find the position. A node whose references reach no named source
(content the library created that corresponds to nothing in a document, or a fragment that
a host parsed without giving its source a name) carries no attribute. The attribute is written
once per node, on the element written for that node:

- an element that corresponds to no node of its own (the `<code>` inside a `<pre>`, the
  `<span>` around a list label) carries none;
- a node written as more than one element carries it on the first of them;
- the empty span written for an element this backend cannot write is the element written
  for that node, so it carries the node's position;
- a construct node is written exactly as its rule gave it, so the emitter adds no position
  to it.

Inside a formula the attribute is checked like every attribute written there ([What this
backend changes inside a formula](#what-this-backend-changes-inside-a-formula) above): a source path holding the formula's closing
delimiter means that the elements inside the formula are written without their position,
and each such attribute is reported. The option is off by default, and turning it on
changes nothing else in the page.

### The stages, the tables and the artifact

The backend has seven stages: the three every backend has (lowering, emit and final), the
two stages of the [pieces](crate::core::piece::Piece), the definitions stage and the regions stage:

1. `flm.html.lower`: the shared lowering stage, set up for this backend, writes the
   lowered trees into `flm.lowered.html`.
2. `flm.html.pieces`: the shared pieces stage, set up for this backend, writes the
   lowered pieces into `flm.lowered.html.pieces`.
3. `flm.html.emit` ([`HtmlEmitStage`](crate::stdcore::backends::html::HtmlEmitStage)),
   once per tree, writes that tree's HTML into the table `flm.output.html`
   ([`HtmlFragments`](crate::stdcore::backends::html::HtmlFragments)) as an
   [`HtmlFragment`](crate::stdcore::backends::html::HtmlFragment): the HTML as a list of
   **segments** (bytes, or another tree placed at that point) together with the
   identifiers of the style and script definitions the tree requires. Trees are emitted
   independently, possibly at the same time, and the table orders them by tree, so the
   bytes do not depend on how the run was executed.
4. `flm.html.emit-pieces`
   ([`HtmlEmitPiecesStage`](crate::stdcore::backends::html::HtmlEmitPiecesStage)), once
   per document, writes the HTML of each lowered piece, with the same emitter, into the
   table `flm.output.html.pieces`
   ([`HtmlPieces`](crate::stdcore::backends::html::HtmlPieces)), each with the identifiers
   of the definitions it requires.
5. `flm.html.resolve-definitions`
   ([`ResolveDefinitionsStage`](crate::stdcore::backends::html::ResolveDefinitionsStage)),
   once per document, turns the identifiers that the trees and the pieces required into the
   table `flm.html.definitions`
   ([`ResolvedDefinitions`](crate::stdcore::backends::html::ResolvedDefinitions)), which
   says how each definition reaches the page ([Style and script definitions](#style-and-script-definitions) below).
6. `flm.html.regions`
   ([`HtmlRegionsStage`](crate::stdcore::backends::html::HtmlRegionsStage)), once per
   document, makes the page's [output regions](crate::core::region::Regions) and writes
   them into the table `flm.output.html.regions`
   ([`REGIONS`](crate::stdcore::backends::html::REGIONS)): the content, with each placed
   tree written where it was placed; each piece; the stylesheets and the scripts, one part
   per element written for a definition; the words of the page's title; and the language.
7. `flm.html.final.singlefile`
   ([`HtmlFinalStage`](crate::stdcore::backends::html::HtmlFinalStage)), once per
   document, is the backend's final stage. It assembles the page from the regions and
   writes it into the artifact set `flm.output.html.document`
   ([`DOCUMENT`](crate::stdcore::backends::html::DOCUMENT)) as the entry artifact
   `html:document`, of media type `text/html` and extension `html`. It then adds to that
   set every artifact of the **artifact table** `flm.output.html.artifacts`
   ([`ARTIFACTS`](crate::stdcore::backends::html::ARTIFACTS)), when a stage of the plan
   wrote that table: files the page links to that exist only once a document is rendered,
   such as copies of the images the page shows. No standard stage writes the artifact
   table, so the set holds the page alone unless a host's stage writes it; a table has one
   writer per plan, so that stage writes every such artifact of the render.

The page that the final stage assembles is a complete HTML document, made of:

- the document type line;
- `<html>`, with the `lang` attribute when the render option `lang` gives a language;
- `<head>`, holding the character encoding, a viewport line, the `<title>`, and the
  stylesheets the document requires. The title's words are those of the document's title
  piece when the render places that piece and it holds text, and otherwise the render
  option `title`;
- `<body>`, holding `<main class="flm-content" id="flm-content">` and then the scripts the
  document requires. `<main>` holds the pieces placed before the content, the trees' HTML
  in document order (a placed tree written where it was placed), and the pieces placed
  after the content.

A page is something a person can open in a browser and a validator can check. An embedder
that wants only the HTML of the content reads the tables `flm.output.html`,
`flm.output.html.pieces` and `flm.html.definitions`, or the regions, which hold the same
HTML ready to be placed in a page of the embedder's own.

The name of the artifact-set table, `flm.output.html.document`, is also the capability "the
document has been written as HTML". The capability `flm.output.html.singlefile`
([`SINGLEFILE`](crate::stdcore::backends::html::SINGLEFILE)) refines it with *how*: one
self-contained file. It is the default among the refinements, and the final stage provides
it. A host that assembles the regions through a template of its own provides the same
table from the same regions, and the plan schedules one of the two.

The conditions this backend reports:

- `flm.html.invalid-name` and `flm.html.refused-address` ([Escaping, and the trust
  guarantee](#escaping-and-the-trust-guarantee) above);
- `flm.pres.heading-above-root` ([Headings in HTML](#headings-in-html) above);
- `flm.html.refused-in-math` ([Mathematics in HTML](#mathematics-in-html) above);
- `flm.html.nesting-too-deep`, for a lowered tree nested more deeply than the emitter's
  limit ([`DEPTH_LIMIT`](crate::stdcore::backends::html::DEPTH_LIMIT)), and
  `flm.html.worker-thread-unavailable`, when the emitter cannot run on a [worker
  thread](crate::lang::guard::on_worker_thread);
- `flm.html.missing-subtree` and `flm.html.subtree-cycle`, from the regions stage, for a
  placed tree that has no HTML or that is already being written;
- `flm.html.piece-placement`, from the pieces' emit stage, for a piece that places a tree;
- `flm.html.unknown-requirement`, `flm.html.unknown-definition` and
  `flm.html.definition-not-inlinable` ([Style and script definitions](#style-and-script-definitions) below);
- `flm.html.stage-failed`, when a stage of this backend cannot read or write a table it
  declared (the plan does not match the stage's declarations), or when an internal
  invariant of the emitter is broken.

### Style and script definitions

A page usually needs more than the HTML of the document: a stylesheet for the elements it
holds, and a formula renderer where it holds mathematics. A **style definition** or a
**script definition** is a stylesheet or a script that a feature's output depends on,
registered under an identifier ([`StyleDef`](crate::core::registry::StyleDef),
[`ScriptDef`](crate::core::registry::ScriptDef); [style and script
definitions](crate::guide::concepts_overview#style-and-script-definitions)). Three steps
keep the code that needs a definition apart from the decision of how it reaches the page:
requiring, declaring, and deciding the delivery.

**Requiring.** A lowering rule requires a definition, that is, records its identifier as
something the output needs, with
[`Lowerer::require`](crate::core::lowering::Lowerer::require) at a node it lowers, or with
[`LoweringDecl::selection_requires`](crate::core::lowering::LoweringDecl::selection_requires)
whenever the plan selects the rule. The emitter requires the style definition that the
HTML contract gives for each element it writes ([The HTML contract](#the-html-contract) above), and the formula
renderer at the first formula of each tree. The result is a set of identifiers per tree in
`flm.output.html`, and per piece in `flm.output.html.pieces`; nothing there says how a
definition reaches the page.

A host may also require definitions for the page as a whole, beside those of the content,
through the render option `required_definitions`. This is how a host adds something of its
own to every page it builds, such as a script that keeps an open page in step with the
document and the stylesheet of what that script shows, without editing the page after it
is written: a feature of the host declares the definitions, and the option requires them.
A definition required both ways is required once and written once in the page.

**Declaring.** A feature registers a definition with
[`style_def`](crate::core::registry::FeatureBuilder::style_def) or
[`script_def`](crate::core::registry::FeatureBuilder::script_def). Its content is an
ordered list of parts ([`DefContent`](crate::core::registry::DefContent)): text, an address
the page loads, a file the host reads, or the text a table holds for the document being
written. Several parts let one identifier cover one concern: the formula renderer is one
script definition whose first part is its configuration and whose second part is the
address its program is loaded from.

**A part supplied by a table.** Some content is known only once the document is read: a
configuration that lists the packages a document uses, or a stylesheet computed per
document. A definition takes such a part from a table
([`DefContent::Table`](crate::core::registry::DefContent::Table)) that names a table of
the value type [`DefinitionText`](crate::core::registry::DefinitionText), one text. The
feature that declares the definition declares that table and writes it with a stage of its
own; the stage `flm.html.resolve-definitions`, which lists the page's definitions (below),
puts the table's text in the part's place. The registry build fails when the part names a
table that no feature declares or one whose value type is not `DefinitionText`. The
resolving stage reads the table without asking the plan to schedule its writer, so the
declaring feature schedules the writer itself, for instance by a rule that requires the
table's capability. The `math-mathjax` extension's `load-in-browser` mode is the example:
its MathJax configuration lists the TeX packages the document uses, from such a table.

**Deciding the delivery.** The render option `definitions` maps an identifier to what the
host puts in place of the declared content
([`DefinitionOverride`](crate::stdcore::backends::html::DefinitionOverride)): either its
own ordered parts, or `ProvidedByHost`, which writes nothing because the host's own page or
bundle supplies the definition. An override never changes *which* definitions the document
requires, only how they reach the page.

The three steps meet in the table `flm.html.definitions`, written by the stage
`flm.html.resolve-definitions`. It lists every definition the document requires, once
each, with its kind (stylesheet or script), the feature that declared it and its delivery.
The stage always adds the page's own stylesheet, `flm.html.style.page`. The table lists the
stylesheets first and then the scripts, each group in the order the features registered
them, which is also the order the page writes them in: the stylesheets in the `<head>`,
the scripts at the end of the `<body>`. The page assembly reads this table and nothing
else about definitions. A text part becomes a `<style>` or a `<script>` holding the text;
an address part becomes a `<link rel="stylesheet">` or a `<script src>` (with `defer`).
Every element written for a definition carries the attribute `data-flm-def` holding the
definition's identifier.

A definition's address is written as it is and is never fetched by this library. The
address policy ([Escaping, and the trust guarantee](#escaping-and-the-trust-guarantee) above) does not apply to it, because
the address comes from a feature's author or from the host, not from a document.

Four conditions concern definitions:

| condition | severity | when it is reported, and what happens |
|---|---|---|
| `flm.html.unknown-requirement` | Error | a required identifier, from the content or from the `required_definitions` option, that no feature declares: nothing is written for it, and it is left out of the table |
| `flm.html.unknown-definition` | Warning | an entry of the `definitions` option whose identifier no feature declares: the entry is ignored |
| `flm.html.definition-not-inlinable` | Error | a part the page cannot hold, either a file (this library opens no file) or text holding the closing tag of the element it would be written into: nothing is written for that part |
| `flm.html.definition-part-missing` | Error | a part supplied by a table for which the run holds no text, because no stage wrote the table or it holds an empty text: a mistake of the feature that declares the definition; the part is empty |

#### The three ways to load the formula renderer

The formula renderer is the script definition `flm.html.mathjax`
([`mathjax_script`](crate::stdcore::backends::html::mathjax_script)), whose two parts are
its configuration and then the address
`https://cdn.jsdelivr.net/npm/mathjax@4/tex-chtml.js`
([`MATHJAX_LOADER`](crate::stdcore::backends::html::MATHJAX_LOADER)). A host changes how
the renderer is loaded with an entry for it in the render option `definitions`:

```rust
use std::collections::BTreeMap;

use flm_core::core::registry::DefContent;
use flm_core::model::names::Name;
use flm_core::stdcore::backends::html::{DefinitionOverride, HtmlRenderOptions, MATHJAX_LOADER};

// 1. From the network, the default: the declared parts, nothing to say.
let from_the_network = HtmlRenderOptions::default();
assert!(from_the_network.definitions.is_empty());
assert_eq!(MATHJAX_LOADER, "https://cdn.jsdelivr.net/npm/mathjax@4/tex-chtml.js");

// 2. A copy installed beside the page, for a reader with no network. Keep the
//    declared configuration by listing it again as the first part: an override
//    replaces the whole content, and a loader without its configuration reads
//    the wrong delimiters.
let declared = flm_core::stdcore::backends::html::mathjax_script();
let configuration = declared.content[0].clone();
let mut definitions = BTreeMap::new();
definitions.insert(
    Name::from("flm.html.mathjax"),
    DefinitionOverride::Parts(vec![
        configuration,
        DefContent::Url(String::from("/static/mathjax/tex-chtml.js")),
    ]),
);
let from_a_local_copy = HtmlRenderOptions { definitions, ..HtmlRenderOptions::default() };
assert_eq!(from_a_local_copy.definitions.len(), 1);

// 3. The host's own bundle — a copy installed with a package manager and built
//    into the host's JavaScript. The page writes nothing, and the table still
//    says the document needs it, so a bundler reading the table includes it.
let mut definitions = BTreeMap::new();
definitions.insert(Name::from("flm.html.mathjax"), DefinitionOverride::ProvidedByHost);
let from_the_hosts_bundle = HtmlRenderOptions { definitions, ..HtmlRenderOptions::default() };
assert_eq!(from_the_hosts_bundle.definitions.len(), 1);
```

The [module that declares the renderer's
script](crate::stdcore::backends::html::mathjax) lists what the shipped configuration says:
it holds what this backend's output needs, and nothing that restricts what an author may
write inside a formula.

#### Mathematics written by authors the host does not trust

`math-passthrough` does not inspect the content of a mathematics region. **This library
therefore does not handle mathematics written by authors a host does not trust**, and
ships no configuration that claims to. A host in that
situation writes a configuration of its own for the formula renderer and delivers it
through the render option `definitions`; the [module that declares the renderer's
script](crate::stdcore::backends::html::mathjax) gives the configuration to write.

### A page updated in place

A host may bring a page that this backend wrote up to date in the reader's browser instead
of loading it again: the live preview of the `flm` program does so after each build of the
document being edited. Such an update replaces the content of one output region of the
page at a time (the element, identified by its `id`, that holds the document's content or
one placed piece; see [`Regions`](crate::core::region::Regions)), and keeps
every element inside it that did not change, so that the reader keeps their place. A
script of the page that transforms elements in the browser, as the formula renderer
replaces a formula's source with its typeset form, needs to know when a region changed,
and the updater must not undo that script's work where the HTML did not change. Two rules
govern how the updater and such a script work together, whoever performs the update:

- **The event.** Once the content of a region's element is new, the updater dispatches the
  event `flm:region-updated`
  ([`REGION_UPDATED_EVENT`](crate::stdcore::backends::html::page::REGION_UPDATED_EVENT)) on
  that element. The event bubbles, so a listener on the element and a listener on
  `document` both receive it. Its `detail` holds `region`, the element's identifier, and
  `artifacts`, the paths of the files beside the page that changed with it.
- **The record.** A script that transforms an element records the HTML the element held
  before, in the attribute `data-flm-original-html`
  ([`ORIGINAL_HTML_ATTRIBUTE`](crate::stdcore::backends::html::page::ORIGINAL_HTML_ATTRIBUTE)).
  The updater leaves such an element as it is when the new HTML inside it equals the
  record, and otherwise replaces its content, which removes the record, since the new HTML
  carries none. On the event, the script transforms again the elements of the region that
  carry no record.

The formula renderer's shipped configuration follows these rules: it writes the record on
each formula just before typesetting it, and on the event it typesets the formulas of the
region that carry no record, the new ones and those whose HTML changed. A formula whose HTML
did not change stays typeset. These rules define no name on `window`: the updater
dispatches an event and each script listens for it, so several scripts can listen and each
learns which region changed. A feature whose script transforms elements follows the same
two rules ([extending FLM](crate::guide::extending#style-and-script-definitions) says
how).

### The stylesheets this backend ships

This backend ships seven stylesheets, one per concern, so that a page carries only what its
content needs: a document with no code loads no rules for code. Each is an ordinary style
definition, delivered like any other ([Style and script definitions](#style-and-script-definitions) above).

| identifier | what it styles | required by |
|---|---|---|
| `flm.html.style.core` | paragraphs, blocks, the display styles of a span (the unresolved reference among them), links, images, the three decorations | the elements every document has |
| `flm.html.style.headings` | the six heading sizes | a block heading |
| `flm.html.style.verbatim` | verbatim content | verbatim content |
| `flm.html.style.math-passthrough` | a displayed formula, which it makes a block, and content written inside a formula | mathematics |
| `flm.html.style.lists` | lists with their entries and labels, and runs of lines | a list, a run of lines |
| `flm.html.style.errors` | what the page shows where something could not be produced | a macro or an environment no feature declares, an element the backend cannot write, a dropped mark |
| `flm.html.style.page` | the page's own typography and the width of its content | the page itself, always, and no element |

Each element's rules are in the stylesheet that the element's row of the HTML contract
gives ([`conventions`](crate::stdcore::backends::html::conventions)), so a page that
requires a stylesheet receives every rule for the content it holds. The unresolved
reference is styled by the core stylesheet for that reason: it is a display style of a
span, and the span's row gives the core stylesheet.

A content feature may ship a stylesheet of its own, under the same rules, and require it
from its general rules by its style identifier (on the rendered LaTeX, the same identifier
brings in the feature's LaTeX definitions; [Style identifiers and definition
pieces](#style-identifiers-and-definition-pieces)):

- the `xnotes` feature's `flm.xnotes.style`: the rule above a list of xnotes and the
  brackets around an xnote shown in place ([Xnotes in HTML](#xnotes-in-html) above);
- the `toc` feature's `flm.toc.style`: the nested lists of the table of contents,
  without markers ([The table of contents in HTML](#the-table-of-contents-in-html) above);
- the `defterm` feature's `flm.defterm.style`: the term being defined, inside its own
  definition ([Defined terms in HTML](#defined-terms-in-html) above);
- the `theorems` feature's `flm.theorems.style`: the name in a theorem's heading
  ([Theorems in HTML](#theorems-in-html) above);
- the `quote` feature's `flm.quote.style`: a quotation set apart from the text around it
  ([Quotations in HTML](#quotations-in-html) above);
- the `annotations` feature's `flm.annotations.style`: a reviewer's colors ([Annotations in
  HTML](#annotations-in-html) above);
- the `floats` feature's `flm.floats.style`: a float set apart, and the line under its
  content ([Floats in HTML](#floats-in-html) above);
- the `cells` feature's `flm.cells.style`: a table's rules and its cells (below);
- the `citations` feature's `flm.citations.style`: a raised citation mark kept from moving
  its line ([Citations in HTML](#citations-in-html) above);
- the `meta` feature's `flm.meta.style`: the document's title large and bold, and the
  authors and the date each on a line of its own, indented under the title, wherever the
  page places them, whatever the compose template. Its custom properties are
  `--flm-meta-title-size` (`1.6rem`) and `--flm-meta-title-line-height` (`1.3em`) for the
  title, and `--flm-meta-line-size` (`1.1rem`), `--flm-meta-line-indent` (`2em`) and
  `--flm-meta-line-space` (`0.5em`, the space above each line) for the authors and the date.
  The rule under the header that holds them on the default page is the compose template's.

In the standard package these features are registered before the backends, so their
stylesheets are registered first and are written first in a page's `<head>`.

**The stylesheet of tables of cells.** `flm.cells.style` is the stylesheet that the HTML
contract's row of `flm.pres.cells` gives, so the page requires it wherever a table of cells
is written; the declaration of each custom style that the `cells` feature ships requires it
too. It draws a rule above and below the table, on the `<table>` itself, a thinner rule
under a column header, and a rule at each edge that a cell's `data-flm-rules` lists,
regardless of the other cells. It pads every cell, aligns a cell by its `data-flm-align`
and to the left when it has none (a header cell included), gives the four shipped custom
styles their backgrounds, and centers a table inside a float. Its custom properties:

| property | default | what it changes |
|---|---|---|
| `--flm-cells-spacing` | `1em` | the space above and below a table |
| `--flm-cells-padding` | `0.3em 0.5em` | the space around the content of a cell |
| `--flm-cells-align` | `left` | the alignment of a cell that states none |
| `--flm-cells-outline` | `1pt solid` | the rules above and below a table |
| `--flm-cells-header-rule` | `0.5pt solid` | the rule under a column header |
| `--flm-cells-rule` | `0.5pt solid` | a rule a cell lists at one of its edges |
| `--flm-cells-green`, `--flm-cells-blue`, `--flm-cells-yellow`, `--flm-cells-red` | `#d8f0d8`, `#d8e4f8`, `#f8f0c8`, `#f8d8d8` | the backgrounds of the custom styles `green`, `blue`, `yellow` and `red` |

A custom style that a host declares is styled by a stylesheet of the host, which the
custom style's declaration requires
([`CellStyle::requires`](crate::stdcore::features::cells::CellStyle::requires)) and which
selects a cell by its attribute `data-flm-style-<name>`.

**What every shipped stylesheet follows.** The first line of every shipped stylesheet is a
comment holding its identifier (`/* flm.html.style.core */`), so that a stylesheet copied
out of a page can still be identified; the page writes the same identifier as the
`data-flm-def` attribute of the `<style>` element. The formula renderer's configuration
begins the same way. Three further rules make the stylesheets safe to include in someone
else's page:

- **One cascade layer.** Every rule is inside `@layer flm { … }`. A cascade layer is a named
  group of rules over which a page's own rules outside any layer take precedence, regardless
  of their selectors, so a host restyles this output with ordinary rules and never needs a
  more specific selector than the shipped one.
- **Every selector begins with a class of the HTML contract**, never a bare `p`, `a`, `h1`
  or `table`, so including this output in a page restyles nothing of that page's own
  content. `flm.html.style.page` is the one exception: it sets the typography of the page
  this backend assembles, so it has a rule on `body`. A host that builds its own page
  around the HTML of the trees leaves this stylesheet out of its page.
- **Every adjustable value is a custom property**, declared on `:root` inside the layer, in
  the same stylesheet that uses it. No stylesheet depends on another, and a host that wants
  a different value sets the property instead of replacing a rule. Setting a property that
  belongs to a stylesheet the page does not carry has no effect.

Every custom property a host can set in the seven stylesheets, the stylesheet that
declares it (`core` for `flm.html.style.core`, and so on), its value when the host sets
none, and what it changes (the core stylesheet's image rule also uses two working values of
its own, `--flm-image-height` and `--flm-image-fit`, which it computes from the image's
size and which a host does not set):

| property | stylesheet | default | what it changes |
|---|---|---|---|
| `--flm-block-spacing` | core | `1em` | the space above and below a paragraph or a block |
| `--flm-tab` | core | `2em` | the width of one tab, the unit of indentation: every length the page writes in tabs, such as a line indented by `\indent`, is a multiple of it |
| `--flm-link-color` | core | `#3232c8` | a link's color |
| `--flm-link-hover-color` | core | `#22228a` | a link's color under the pointer |
| `--flm-link-decoration` | core | `none` | whether a link is underlined when the pointer is elsewhere |
| `--flm-runin-gap` | core | `0.5em` | the gap between a run-in heading and the text that follows it |
| `--flm-final-gap` | core | `0.5em` | the gap before a closing mark at the end of a block |
| `--flm-margin-note-width` | core, page | `8rem` | the width of the column right of the text in which a margin decoration is placed |
| `--flm-margin-note-gap` | core, page | `1rem` | the gap between the text and that column |
| `--flm-margin-note-inset` | core | `0rem` | how far the block holding a margin decoration is indented from the right edge of the text; a stylesheet that indents a block on the right sets it on that block |
| `--flm-margin-note-size` | core | `0.85rem` | the size of a margin decoration's text, the same wherever its mark is |
| `--flm-margin-note-weight` | core | `normal` | its weight, set rather than taken from the text around its mark |
| `--flm-margin-note-style` | core | `normal` | its style (upright), set rather than taken from the text around its mark |
| `--flm-margin-note-color` | core | `#555` | its color |
| `--flm-margin-note-space` | core | `0.25em` | the space on each side of a margin decoration that stays at its mark |
| `--flm-margin-note-column` | core | `initial` | the switch that places margin decorations in the column: `initial` keeps each at its mark, an empty value places it in the column; the page stylesheet sets it to an empty value on its content element when the window is at least `52rem` wide |
| `--flm-unresolved-color` | core | `#a00000` | the color of an unresolved reference |
| `--flm-unresolved-background` | core | `#fdeaea` | the tint behind it |
| `--flm-unresolved-radius` | core | `0.2em` | the rounding of that tint's corners |
| `--flm-unresolved-padding` | core | `0 0.2em` | the space between the reference's text and the edge of the tint |
| `--flm-heading-size-1` … `-6` | headings | `1.8em`, `1.5em`, `1.25em`, `1.1em`, `1em`, `1em` | the size of a heading at each level (`data-flm-level`) |
| `--flm-heading-weight` | headings | `bold` | a heading's weight |
| `--flm-heading-line-height` | headings | `1.2` | the distance between the lines of one heading |
| `--flm-heading-space-above` | headings | `1.5em` | the space above a heading |
| `--flm-heading-space-below` | headings | `0.5em` | the space below a heading |
| `--flm-verbatim-family` | verbatim | `ui-monospace, SFMono-Regular, Menlo, Consolas, monospace` | the font family of verbatim content |
| `--flm-verbatim-size` | verbatim | `0.9em` | its size |
| `--flm-verbatim-background` | verbatim | `#f2f2f2` | the tint behind it |
| `--flm-verbatim-radius` | verbatim | `0.2em` | the rounding of that tint's corners |
| `--flm-verbatim-inline-padding` | verbatim | `0.1em 0.3em` | the space inside that tint, for verbatim content in a line of text |
| `--flm-verbatim-block-padding` | verbatim | `0.6em 0.8em` | the space inside it, for a block of verbatim content |
| `--flm-verbatim-block-spacing` | verbatim | `1em` | the space above and below a block of verbatim content |
| `--flm-math-display-space-above` | math | `1em` | the space above a displayed formula |
| `--flm-math-display-space-below` | math | `1em` | the space below it |
| `--flm-math-anchor-space-above` | math | `1.5em` | the space a browser keeps above a line of a formula when a link takes the reader to it |
| `--flm-list-spacing` | lists | `1em` | the space above and below a list |
| `--flm-list-indent` | lists | `2em` | how far a list's entries are indented |
| `--flm-list-entry-spacing` | lists | `0.25em` | the space between two entries |
| `--flm-list-label-gap` | lists | `0.5em` | the gap after a label written by the document |
| `--flm-list-label-max` | lists | `12em` | the maximum width of a list's column of labels |
| `--flm-lines-spacing` | lists | `1em` | the space above and below a run of lines |
| `--flm-error-color` | errors | `#a00000` | the color of an error |
| `--flm-error-background` | errors | `#fdeaea` | the tint behind it |
| `--flm-error-border` | errors | `1px solid #a00000` | its border, width and color together |
| `--flm-error-radius` | errors | `0.2em` | the rounding of its corners |
| `--flm-error-family` | errors | `ui-monospace, SFMono-Regular, Menlo, Consolas, monospace` | its font family |
| `--flm-error-inline-padding` | errors | `0 0.2em` | the space inside it, in a line of text |
| `--flm-error-block-padding` | errors | `0.4em 0.6em` | the space inside it, on lines of its own |
| `--flm-dropped-gap` | errors | `0.3em` | the space between two dropped marks side by side |
| `--flm-unsupported-open` | errors | `"["` | what is written before the name of an element the backend cannot write |
| `--flm-unsupported-close` | errors | `"]"` | what is written after that name |
| `--flm-page-family` | page | `"Source Serif 4", "Times New Roman", Times, serif` | the font family of the page's text; another stylesheet may read it to use the page's font, as the `math-mathjax` extension's stylesheet does for the text inside a formula |
| `--flm-page-size` | page | `100%` | its size |
| `--flm-page-line-height` | page | `1.5` | the distance between the page's lines |
| `--flm-page-color` | page | `#1a1a1a` | the color of the page's text |
| `--flm-page-background` | page | `#ffffff` | the page's background |
| `--flm-page-measure` | page | `42rem` | the width of the content |
| `--flm-page-padding` | page | `1.5rem` | the space between the content and the edge of the page |

A host that wants a stylesheet of its own in place of a shipped one gives its text under
the same identifier through the render option `definitions`: the requirement is unchanged,
and the page receives the host's text. This replaces the whole stylesheet, so a host that
only wants to add rules to `flm.html.style.core` leaves it as it is, or sets it to
`ProvidedByHost` and writes the complete stylesheet in its own page.

**Run-in headings.** A run-in heading reaches the page as a run-in decoration. The stop after
its title is written by the backend, as text inside the decoration's span;
`flm.html.style.core` sets only the gap after it, from `--flm-runin-gap`. [Headings in HTML](#headings-in-html)
above says which titles receive a stop and how a host changes it.

**Margin decorations.** A margin decoration, such as a reviewer's initials, is shown either
small, at its mark, in the line, or in a column right of the text, level with the line of
its mark. One switch chooses between the two, the custom property
`--flm-margin-note-column`: while it is `initial` (the default) a margin decoration stays at
its mark; where it is set to an empty value, the decoration is placed in the column. In the
column, `flm.html.style.core` floats the decoration to the right and moves it out of the
text by a negative right margin as wide as the column (`--flm-margin-note-width`), the gap
(`--flm-margin-note-gap`) and the right indentation of the block holding it
(`--flm-margin-note-inset`), so the text keeps its whole width; decorations marked on one
line are stacked in the column, one below the other. Each rule of the column applies only
while the switch is empty, because CSS cannot choose whether to float an element from a
property's value. A margin decoration has one size,
in `rem`, wherever its mark is (`--flm-margin-note-size`), and one weight and style
(`--flm-margin-note-weight`, `--flm-margin-note-style`), never those of a heading or an
emphasis around its mark. Inside text written in a formula, a margin decoration stays at its
mark regardless of the switch.

The core stylesheet has no condition on the window's width: the element that leaves room
for the column turns the switch on. `flm.html.style.page` does so on the content element
when the window is at least `52rem` wide; there the content element grows by the column and
the gap, and so does its right padding, so that the text keeps the width
`--flm-page-measure` gives it. A host that includes the HTML of the trees in a page of its
own does the same on its own element around them, under a condition of its own (or none,
to always use the column), and leaves at least the width of the column and the gap free to
the right of the text, for example as right padding.

The column has two limits. A block that contains its floats, such as a list entry laid out
as a grid, grows to hold a margin decoration that extends below its last line, so several
margin decorations marked on one line of a list entry may lengthen the entry; and a margin
decoration in such a block is not stacked with those of the text before the block and may
overlap them. A margin decoration also stops short of the column by any right indentation
that its block's `--flm-margin-note-inset` does not account for: that of a quotation around
the quotation it is in, or a line's indentation from the right.

**Labeled lists.** A list whose every entry carries a label of its own says so with the
kind token `labeled` ([`LIST_LABELED`](crate::stdcore::backends::pres::LIST_LABELED)),
since only the feature that produced the list has that information; the `lists` feature writes it on
every list. `flm.html.style.lists` turns off the marker a browser draws beside each entry
of such a list and lays the list out as a grid of two columns: the labels in the first,
the entries' content in the second, the label gap (`--flm-list-label-gap`) between the
columns and the entry spacing (`--flm-list-entry-spacing`) between the rows. The column of
labels is as wide as the widest label of the list, up to `--flm-list-label-max`; a longer
label wraps inside the column. A label is aligned to the right of its column, level with
the top of its entry's first line, and an entry with an empty label leaves its first cell
empty. Each `<li>` passes its two cells to the list's grid through a *subgrid* (a grid whose
columns are those of the grid around it), so that the labels of all entries share one
column; a browser without subgrid support lays out each entry as a grid of its own, the
labels then no longer lining up from one entry to the next, with nothing cut off. A list
nested in an entry is a grid of its own, with its own column of labels.

A list of terms (`terms`) sets each term in bold. When it also carries `labeled`, the list
itself is the grid, each term aligned to the left in the first column and its description
beside it; without `labeled`, each term is above its indented description. A list of
another kind without `labeled` keeps the marker a browser draws.

**Runs of lines.** A line of a run of lines carries its indentation as the custom
properties `--flm-element-indent-left` and `--flm-element-indent-right` of its `style`
attribute ([The HTML contract](#the-html-contract) above), and `flm.html.style.lists` applies them as padding, zero
where a line has none. A length in tabs is a multiple of `--flm-tab`, so a host changes the
width of every indentation with that one property.

### HTML render options

[`HtmlRenderOptions`](crate::stdcore::backends::html::HtmlRenderOptions) holds the
backend's render options. They are declared with
[`Bind::Render`](crate::model::options::Bind::Render): their values come from the render
request rather than from the document, and they are part of the plan's identity
([`PlanKey`](crate::core::plan::PlanKey)), so two runs asking for different values use two
plans.

| option | default | what it does |
|---|---|---|
| `title` | `FLM document` | the page's `<title>` when the document has no title piece with text |
| `lang` | none | the `lang` attribute of `<html>`; none writes no attribute |
| `source_spans` | `false` | the source-span attribute on every element written for a node ([The source-span attribute](#the-source-span-attribute) above) |
| `run_in_stop` | `.` | the stop written after a run-in heading's title; the empty text writes none |
| `definitions` | empty | what the host puts in place of a definition's declared content ([Style and script definitions](#style-and-script-definitions) above) |
| `required_definitions` | empty | the identifiers of the definitions the host requires for the page as a whole, beside those the content requires; each is written once in the page |
| `heading_tags` | empty | the tag of the headings of each listed heading kind ([Headings in HTML](#headings-in-html) above); the name `xnotes` gives the tag of the title above a list of xnotes |
| `root_heading_level` | 1 | the heading level written as `<h1>`; each deeper level one tag deeper, down to `<h6>`; a heading above it is an Error and is written as `<h1>` ([Headings in HTML](#headings-in-html) above) |

No option renames a class or an attribute of the HTML contract: the HTML contract is not
configurable.

### The plan an `html` workflow builds, and the one it shares with `text`

The `html` workflow lists the `html` output and also demands the capability
`flm.validated`, as every rendering workflow of the standard package does, so that one
document receives the same checks whichever workflow is run over it. The rest of the plan
follows from what the selected lowering rules and stages require.

This backend, the plain-text backend and the rendered LaTeX backend read the trees in the
same state, with block structure built and user definitions expanded, so **one plan can
produce these outputs together**.
The recomposed LaTeX backend reads the trees as parsed instead, so a request for that output
together with this one is refused when the plan is built.

```rust
use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan, PlanError, PlanRequest};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::stdcore::backends::html;
use flm_core::stdcore::backends::latex_recomposed;
use flm_core::stdcore::backends::text::TEXT_DOCUMENT;
use flm_core::stdcore::{self, StdcoreConfig};

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;

// The `html` workflow: the anchors are computed, because no element of the
// `refs` feature is in the backend's native set.
let plan = Plan::for_workflow(&registry, html::WORKFLOW, &Overrides::none())?;
let stages: Vec<String> = plan.stage_names().iter().map(|name| name.to_string()).collect();
assert!(stages.contains(&String::from("flm.refs.anchors")));
assert!(stages.contains(&String::from("flm.html.emit")));

// One plan carries the text output and the HTML output together.
let both = PlanRequest::new().demand(TEXT_DOCUMENT.name()).demand(html::DOCUMENT.name());
let shared = Plan::build(&registry, &both)?;
let backends: Vec<String> =
    shared.active_backends().iter().map(|name| name.to_string()).collect();
assert_eq!(backends, ["html", "text"]);

// The recomposed LaTeX output wants the raw tree: no plan carries both.
let clash =
    PlanRequest::new().demand(html::DOCUMENT.name()).demand(latex_recomposed::DOCUMENT.name());
assert!(matches!(Plan::build(&registry, &clash), Err(PlanError::Unsatisfiable { .. })));

// One document, rendered.
let source = Arc::new(
    FlmSource::new("\\section{Sums}\\label{sec:sums}\n\nSee \\ref{sec:sums}.")
        .with_origin(FlmOrigin::new("doc.flm")),
);
let fragment =
    parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));
let mut run = Run::new(Arc::new(Doc::new(content)), Arc::new(plan), Externals::none())?;
run.execute(&RunOptions::new())?;
let set = run.table(html::DOCUMENT)?.expect("written");
let page = std::str::from_utf8(set.entry().unwrap().as_bytes().unwrap()).unwrap();
assert!(page.starts_with("<!DOCTYPE html>"));
assert!(page.contains("<h1 class=\"flm-pres-block--heading\" id=\"sec-sums\" data-flm-heading=\"section\" data-flm-level=\"1\" data-flm-toc=\"true\">Sums</h1>"));
assert!(page.contains("<a class=\"flm-pres-link\" href=\"#sec-sums\" data-flm-kind=\"ref\">Sums</a>"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## The rendered LaTeX backend

The `latex-rendered` backend ([`crate::stdcore::backends::latex_rendered`]) writes a
document as LaTeX for the LuaLaTeX engine, with everything FLM can resolve already
resolved: the numbers of headings, equations, theorems, figures and notes, the
cross-references, the table of contents, the lists of notes and the bibliography are
computed by FLM and written as text. LuaLaTeX is used only to lay the result out on pages.
This chapter calls the LaTeX document the backend writes the **rendered LaTeX**.

The rendered LaTeX writes every element of the [presentation
vocabulary](#the-presentation-vocabulary) as one LaTeX command or environment named
`\flmx…` (`\flmxHeading`, `\flmxSpan`, `\begin{flmxBlock}`, and so on), and passes every
fact the element carries to that command as a key-value argument. The backend also writes
a **style file**, a LaTeX package (a `.sty` file) that the document loads and that defines
every `\flmx…` command the document uses, with a default look. A **template** (the LaTeX
document around the content, or LaTeX text loaded after the style file) changes the look
of any element by registering LaTeX code for it. The `\flmx…` commands play the part that
the tags and classes play on the HTML page: FLM decides the structure, and the template
decides the look.

A caller asks for the `latex-rendered` output, or for the `latex-rendered` workflow, and
reads the artifact set from the table `flm.output.latex-rendered.document`
([`DOCUMENT`](crate::stdcore::backends::latex_rendered::DOCUMENT)). The entry artifact is
a complete LaTeX document of the class `article`; the style file is a second artifact,
whose name the document's first line gives; image copies, where the output copies images,
follow. One feature, [`LatexRendered`](crate::stdcore::backends::latex_rendered::LatexRendered),
registers the backend, the output and the workflow, all three named `latex-rendered`.

The chapter covers, in order:

- what this backend shares with the recomposed LaTeX backend, and what it does not;
- the elements the backend writes without a lowering rule;
- the **rendered LaTeX contract**: the command or environment written for each
  presentation element, the key-value argument, how a template registers a look, and how
  anchors become LaTeX labels;
- the library's default definitions, and what a template is left to decide;
- headings; links, the table of contents and page numbers; mathematics; xnotes; the
  remaining elements;
- each standard feature as it is written in the rendered LaTeX, with the roles and kind
  tokens it writes and the default look of each;
- the style file and how a requirement reaches it; the render options; the stages and
  tables; the compose templates and how to compile the result.

The macros and the preamble differ from the ones the Python implementation of FLM writes;
[Migrating from Python FLM](crate::guide::python_flm_migration) says so.

### Two LaTeX backends: rendered and recomposed

The library ships two LaTeX backends, and they are two independent products:

- the **recomposed LaTeX** (`latex-recomposed`, [The recomposed LaTeX
  backend](#the-recomposed-latex-backend) below) writes the document back as LaTeX source close to what the author typed, and
  leaves numbering, cross-references and the table of contents to LaTeX and the author's
  document class. Its output is meant to be read and edited by a person, and it compiles
  under pdfLaTeX, XeLaTeX or LuaLaTeX;
- the **rendered LaTeX** (`latex-rendered`, this chapter) writes resolved content through
  the `\flmx…` commands, for LuaLaTeX alone. Its output is meant to be typeset, not edited.

Each has its own backend feature, emitter, style file, render options, default
definitions, compose templates and tests; nothing of one is used by the other. What they
share is library mechanism with no policy in it, all of it in the **LaTeX family**
([`crate::stdcore::backends::latex`], described in [What the LaTeX family
provides](#what-the-latex-family-provides)): the escaping of document text, the typed sink
through which an emitter writes LaTeX, the four construct elements, and the two service
slots through which features contribute LaTeX package requests and definition pieces. The
rendered LaTeX uses these as a caller; the recomposed LaTeX's style-file assembly, its
LaTeX hooks (`\flmHeading`, `\flmRef`, …), its Unicode declarations and its templates are
not used.

### The elements the backend writes without a lowering rule

The backend's native set
([`native_elements`](crate::stdcore::backends::latex_rendered::native_elements)) holds 31
elements:

- every presentation element ([`pres::ALL`](crate::stdcore::backends::pres::ALL));
- the four construct elements of the LaTeX family
  ([`construct::ALL`](crate::stdcore::backends::latex::construct::ALL)): a macro call, an
  environment, a raw node written byte for byte, and a definition;
- the five structural elements `flm.text`, `flm.group`, `flm.comment`, `flm.container` and
  `flm.parbreak`
  ([`FLOW_THROUGH`](crate::stdcore::backends::latex_rendered::FLOW_THROUGH)).

Every element of a content feature is therefore lowered by its feature's general lowering
rule, the rule that serves every backend. The standard features write one rule of their
own for this backend: the xnotes feature's, for a note shown in place ([Xnotes in the
rendered LaTeX](#xnotes-in-the-rendered-latex)). Everything else a feature contributes to
this backend is a definition piece: LaTeX definitions that give its roles and kind tokens
their default look ([Style identifiers and definition
pieces](#style-identifiers-and-definition-pieces)).

The rendered LaTeX reads the document's trees laid out in paragraphs, as the HTML page and
the plain text do, so a workflow may list `latex-rendered` together with `html` or `text`
and one plan produces all of them. Paragraphs are built before lowering
([When the page holds a paragraph](#when-the-page-holds-a-paragraph) describes how a
fragment's layout decides them); the rendered LaTeX writes each `flm.pres.par` node as a
LaTeX paragraph.

### The rendered LaTeX contract

The **rendered LaTeX contract** is the set of commands, environments, keys and names that
a template may rely on in this backend's output. It has the same purpose as [the HTML
contract](#the-html-contract): FLM writes structure, and a template, like a stylesheet,
decides the look from the names it finds.

**Names and shape.** Each presentation element is written as one command or environment
whose name is `flmx` followed by the element's name without `flm.pres.`, in camel case,
each hyphen or dot starting a capital: `flm.pres.block` is the environment `flmxBlock`,
`flm.pres.runin-heading` the command `\flmxRuninHeading`, `flm.pres.deco.runin` the
command `\flmxDecoRunin`. The one abbreviation is `flm.pres.block-heading`, written
`\flmxHeading`. An element that holds blocks is an environment; an inline element is a
command whose content is its last mandatory argument. Every field of the element's
payload (the typed data the element carries, [Typed payloads](#typed-payloads)) is a key
of an optional key-value argument written first. A key is the field's name with `_`
replaced by `-` (`shows_target` becomes `shows-target`, `doc_role` becomes `doc-role`),
with two renames on a heading: its `name` is written `kind` and its `in_toc` is written
`toc`. A key whose field is absent, empty or false is left out, so that a definition may
test whether a key is present.

The contract is written down once, as data
([`contract::ROWS`](crate::stdcore::backends::latex_rendered::contract::ROWS)), one row per
presentation element. The emitter reads an element's row before it writes a node of that
element, and requires the row's definition piece (the part of the style file that defines
the command), so that the style file holds the definition of every command the document
uses. The function
[`print_rows`](crate::stdcore::backends::latex_rendered::contract::print_rows) prints the
rows, as below: each line gives the element, its LaTeX form with the keys in the order the
emitter writes them (a renamed key with the payload field it comes from in parentheses, a
key filled from a slot as `slot NAME`, an item's key as `item`), its mandatory arguments
other than the content, and the definition piece it requires.

```text
flm.pres.par: a paragraph (flm.latex.def.flmx.core)
flm.pres.block: \begin{flmxBlock}[role, kinds, anchor, doc-role] … \end{flmxBlock} (flm.latex.def.flmx.core)
flm.pres.block-heading: \flmxHeading[kind (name), level, toc (in_toc), anchor, number, plain, alt (slot alt)] (flm.latex.def.flmx.headings)
flm.pres.runin-heading: \flmxRuninHeading[kind (name), level, toc (in_toc), anchor, number, plain, alt (slot alt), stop] (flm.latex.def.flmx.headings)
flm.pres.span: \flmxSpan[style, role, kinds, anchor, doc-role] (flm.latex.def.flmx.core)
flm.pres.list: \begin{flmxList}[kinds, anchor, depth, item label (slot label), item anchor (entry_anchors)] … \end{flmxList} (flm.latex.def.flmx.lists)
flm.pres.cells: \begin{flmxCells}[rows, cols, cells] … \end{flmxCells} (flm.latex.def.flmx.cells)
flm.pres.lines: \begin{flmxLines} … \end{flmxLines} (flm.latex.def.flmx.lines)
flm.pres.line: \flmxLine[indent-left, indent-right, align] (flm.latex.def.flmx.lines)
flm.pres.verbatim: \flmxVerbatim[kind, lang] or \begin{flmxVerbatimBlock}[kind, lang] … \end{flmxVerbatimBlock} (flm.latex.def.flmx.verbatim)
flm.pres.math-passthrough: the formula's delimiters around its source
flm.pres.math-text: content inside the formula's source
flm.pres.link: \flmxLink[kind, href, target, doc-role, label, shows-target] (flm.latex.def.flmx.links)
flm.pres.image: \flmxImage[width, height, alt, display, anchor] (flm.latex.def.flmx.images)
flm.pres.unsupported: \flmxUnsupported{element}{message} (flm.latex.def.flmx.marks)
flm.pres.dropped: \flmxDropped{condition}{detail} (flm.latex.def.flmx.marks)
flm.pres.unknown: \flmxUnknown[kind]{name}{spelling} (flm.latex.def.flmx.marks)
flm.pres.deco.runin: \flmxDecoRunin[stop] (flm.latex.def.flmx.core)
flm.pres.deco.final: \flmxDecoFinal (flm.latex.def.flmx.core)
flm.pres.deco.margin: \flmxDecoMargin (flm.latex.def.flmx.core)
flm.pres.deco-mark: the decoration at the mark's place
flm.pres.subtree: the placed tree in place
```

The commands `\flmxHeading`, `\flmxRuninHeading`, `\flmxSpan`, `\flmxLine`,
`\flmxVerbatim`, `\flmxLink`, `\flmxDecoRunin`, `\flmxDecoFinal` and `\flmxDecoMargin`
take their content as a last argument in braces; `\flmxImage` takes the image's path
there. A list's entries are written `\flmxItem[label=…, anchor=…]`, each followed by the
entry's content. A test checks that every field of every payload appears in its row,
either as a key or as a field the row lists as not written, with the reason (a
paragraph's anchor is written as `\flmxTarget` at its start; a formula's display and
environment choose its delimiters).

Because a template registers a look under these names, **the name of an element's
command, the name of every key, and every role and kind token a feature writes are public
interface on this backend**, as the classes derived from them are on HTML. The roles and
kind tokens of each standard feature are listed with the feature below.

#### The key-value argument

The argument is written `[key=value, key={value}, …]`, one space after each comma, on the
command's line, and is never broken across lines. Its values are of five sorts:

- A **name** (a role, a kind token, an anchor, a link kind, a heading kind, a style name,
  a display or alignment word, a verbatim kind, a language tag) is written bare, or in
  braces where it holds a character outside letters, digits and `. - _ : / + @ *`. The
  kind tokens are one value, separated by spaces, in braces:
  `kinds={theoremlike theorem}`.
- **Content** (an alternative title, a list entry's label, an image's alternative text) is
  lowered like any other content and written as LaTeX, always in braces:
  `alt={Short \flmxSpan[style=emph]{title}}`.
- **Text computed by the emitter** (a heading's `plain` and `number`) is written as
  escaped document text, in braces: `plain={3.1. Salt \& pepper}`.
- An **address** (`href`) is written in braces, with `#`, `%` and `\` escaped and `~`
  written `\string~`: `href={\#sec-intro}`.
- A **length** is spelled for LaTeX (`indent-left=2\flmxTabWidth`, `width=4.5cm`,
  `height=12bp`; [Lines and lengths](#lines-and-lengths-in-the-rendered-latex)); a
  **number** (`level`, `depth`, `rows`, `cols`) is written bare; a **boolean** is written
  `true` or `false`.

The values of the keys that hold names are **names, not text**. A template reads them with
`\flmxKey` to choose a look (to dispatch on them, or to compare them with a name it
expects), and does not typeset them: `\flmxKey{kinds}` is `theoremlike theorem`, a list of
tokens, not words for a reader. What a reader sees is always in the content, or in a key
of the content or text sort (`plain`, `number`, `alt`, `label`).

#### Registering a look

The style file defines each command and environment once. Its definition looks up code
registered under the values of the element's keys, in a fixed order, and runs the first
code it finds, else the element's default:

| element | looked up under, in order |
|---|---|
| `flmxBlock` | its role, then each kind token in order |
| `\flmxSpan` | its role, then each kind token in order, then its style |
| `\flmxHeading`, `\flmxRuninHeading` | its kind, then its level |
| `\flmxLink`, `\flmxVerbatim`, `flmxVerbatimBlock`, `\flmxXnote` | its kind |
| `flmxList`, `\flmxItem` | each kind token of the list, in order |
| a cell of `flmxCells` | the keys registered for each of its styles, in order, added to the cell's own |

The decorations, `flmxLines`, `\flmxLine`, `flmxCells`, `\flmxImage` and the three marks
have one default each. A registration is made with one of these commands, all defined by
the core of the style file; each takes the name to match, then the code:

```latex
\flmxDefineBlock{<role or kind token>}{<begin code>}{<end code>}
\flmxDefineSpan{<role, kind token or style name>}{<code over #1>}
\flmxDefineHeading{<kind name or level>}{<code over #1>}
\flmxDefineRuninHeading{<kind name or level>}{<code over #1>}
\flmxDefineLink{<kind>}{<code over #1>}
\flmxDefineList{<kind token>}{<begin code>}{<end code>}
\flmxDefineItem{<kind token>}{<code run at the item>}
\flmxDefineVerbatim{<kind>}{<code over #1>}
\flmxDefineVerbatimBlock{<kind>}{<begin code>}{<end code>}
\flmxDefineXnote{<kind>}{<code over #1 (the mark) and #2 (the content)>}
\flmxDefineCellStyle{<style name>}{<tblr cell keys>}
```

`#1` is the element's content. Each element's default is replaced with
`\flmxDefine<Element>Default{…}`: `\flmxDefineBlockDefault{<begin>}{<end>}`,
`\flmxDefineSpanDefault{<code over #1>}`, and likewise for `Heading`, `RuninHeading`,
`Link`, `List`, `Item`, `Cells`, `Lines`, `Line`, `Verbatim`, `VerbatimBlock`, `Image`,
`Xnote`, the three decorations `DecoRunin`, `DecoFinal` and `DecoMargin`, and the three
marks `Dropped`, `Unsupported` and `Unknown` (whose code is over `#1` and `#2`). A later
registration replaces an earlier one under the same name.

Inside the registered code, the keys of the element being written are available by name:
`\flmxKey{anchor}` expands to the value, empty where the element does not carry the key,
and `\flmxIfKey{anchor}{<yes>}{<no>}` tests whether it carries it. Each element sees its
own keys only: a block inside a block sees its own, and the outer block's keys are back
after it. Registered code replaces the default whole, so code that wants the element to
remain a link target places the target itself, as the defaults do:
`\flmxIfKey{anchor}{\flmxTarget{\flmxKey{anchor}}}{}`.

Where a role and a kind token both have code, the role's wins; where two kind tokens do,
the first token in the written order wins. A theorem, for example, is a block of role
`flm.theorems.theorem` with the kind tokens `theoremlike theorem` (its theorem class, then
its theorem kind): code registered under `theoremlike` changes every theorem of that class,
and code registered under the role changes every theorem.

**An example.** A template that sets every theorem of the class `theoremlike` in LaTeX's
`quotation` environment loads the style file, then registers the class token:

```latex
\documentclass{article}
\usepackage{paper-flmx.1a2b3c4d}
% A theorem of the class theoremlike: LaTeX's quotation environment, the target
% of the theorem's anchor first.
\flmxDefineBlock{theoremlike}
  {\begin{quotation}\flmxIfKey{anchor}{\flmxTarget{\flmxKey{anchor}}}{}}
  {\end{quotation}}
\begin{document}
...
```

The registration stands after `\usepackage`, because the style file registers the
defaults and a later registration wins. Every other class keeps its default look.

The commands that are part of the contract are named `\flmx…` in camel case; the style
file's internal commands are named `\__flmx_…`, in the naming convention of LaTeX's
programming layer (expl3), except a few that LaTeX itself must read back from its
auxiliary file, named `\flmx@…` (`\flmx@headingplace`, `\flmx@xnotemark`). A template
uses neither of the last two kinds.

#### Targets and labels

Every `anchor` key is turned into a link target by `\flmxTarget{<anchor>}`, which the
default definitions place at the start of the element: in a block after its `\begin`; in a
span or a link before the content; in a heading after its vertical space and before its
title, so that a link leads to the heading; in a list entry right after LaTeX's `\item`;
in a cell at its start; before an image; at the start of a paragraph, where the
paragraph's own anchor is written as `\flmxTarget{…}` on its first line. In text,
`\flmxTarget` writes `\phantomsection` and `\label{flmx:<anchor>}`; inside mathematics, a
`hyperref` target and the label written into the auxiliary file by hand. A template may
redefine `\flmxTarget`; the label's prefix `flmx:` is fixed, so that any definition can
reach an element whose anchor it knows with `\hyperref[flmx:<anchor>]{…}` or
`\pageref{flmx:<anchor>}`. A link inside the document carries that anchor as its `target`
key ([Links, the table of contents and page numbers in the rendered
LaTeX](#links-the-table-of-contents-and-page-numbers-in-the-rendered-latex)).

### The library's default definitions

The style file holds a default definition for every element and for every role and kind
token the standard features write, so that the rendered LaTeX compiles under
`\documentclass{article}` and looks like an article: headings in the article's sizes with
FLM's numbers; theorems with a bold run-in name and an italic body for the theorem-like
class; bulleted, numbered and labeled lists; verbatim blocks in a typewriter font;
figures and tables as LaTeX floats with the caption below; tables of cells set with
`tabularray`, with rules above and below; colored links; notes at the bottom of the page
for a kind shown in place and as a list after the content for a kind collected in a list;
a title block in the style of `\maketitle`; a table of contents with dotted leaders and
page numbers. Each default is described with its element below.

The defaults set no font, no margins, no page style, no document class and no package
that changes the typesetting of the whole text (such as `microtype` or `unicode-math`):
those are the template's.

**Characters the font lacks.** Document text is written as UTF-8 characters, apart from
the ten characters that are special to LaTeX, which are escaped (`\&`, `\%`, `\{`,
`\textbackslash{}`, …). Under LuaLaTeX with `fontspec` the font supplies the glyphs, so
nothing else is declared. A character the template's font does not have is LuaLaTeX's
"Missing character" warning in the log, and an empty space in the output; choosing a font
that has the document's characters is the template's concern. The default font, Latin
Modern, lacks for instance the end-of-proof mark `□` (U+25A1) that the theorems feature
writes at the end of a proof.

**Content the defaults cannot place.** Some content cannot be typeset by the default
definitions where it stands: a float (a figure or a table) inside a note shown in place,
inside a table cell or inside a box, where LaTeX cannot make a float and stops with "Not in
outer par mode"; and the text of a note shown in place inside a table cell, which LaTeX
loses. The defaults have no special case for these places; a template that needs such
content registers a look for the elements concerned (for instance a float type set as a
centered block rather than a LaTeX float, or a note kind written as text).

**Lists nested more than six deep.** The default list definition is LaTeX's `\list`, which
stops with "Too deeply nested" past six levels of lists. The default adds no limit of its
own; a template that needs deeper lists defines `flmxList` otherwise.

### Headings in the rendered LaTeX

A heading ([`headings`](crate::stdcore::features::headings)) is written
`\flmxHeading[…]{<title>}` on a line of its own. FLM numbers the heading itself: the
number stands in the title, in a span of role `flm.heading.number`, followed by the rest of
the kind's title template, and no LaTeX counter is stepped (`\section` and `secnumdepth`
play no part). From the headings golden test:

```latex
\flmxHeading[kind=section, level=1, toc=true, anchor=sec-alt, number={2}, plain={2. A long and fancy title}, alt={Short \flmxSpan[style=emph]{title}}]{\flmxSpan[role=flm.heading.number]{2}.
A long and \flmxSpan[style=emph]{fancy} title}
```

Its keys:

- `kind`: the heading kind's name (`section`, `subsection`, or a kind the configuration
  adds, such as `chapter`);
- `level`: the heading's level, counted from zero (`section` is 1, `subsection` 2);
- `toc`: `true` or `false`, always written: whether the heading is listed in a table of
  contents;
- `anchor`: the heading's anchor, where it is a reference target;
- `number`: the heading's number as a reference to it writes it, without the word and the
  delimiters around it (`2`, `3.1`), where the document numbered the heading; it also
  stands in the title;
- `plain`: the title's characters as plain text, the number included, with runs of white
  space collapsed; always written, empty for an empty title;
- `alt`: the alternative title, as content, where the author wrote `alt=` on the heading.

A template lays out the number and the title separately either from the keys (with
`\flmxKey{number}`, the span of role `flm.heading.number` registered to write nothing) or
by registering that span's role; it sets a running head from `alt` or `plain`, and a PDF
bookmark from `plain`.

**A run-in heading.** A heading kind configured as run in is written inside a run-in
decoration at the start of the next paragraph, as `\flmxRuninHeading`, with the same keys
and the key `stop`, the punctuation after the title (the render option `run_in_stop`,
`.` by default, left out where the title already ends in sentence punctuation):

```latex
\flmxDecoRunin[stop={.}]{\flmxRuninHeading[kind=paragraph, level=4, toc=true, anchor=sec-a, number={a}, plain={a. A run-in title}, stop={.}]{\flmxSpan[role=flm.heading.number]{a}.
A run-in title}}Text after it, see \flmxLink[kind=ref, href={\#sec-a}, target=sec-a]{§ a}.
```

**The default look.** `\flmxHeadingLevel{<level>}{<title>}`, a public command of the style
file, typesets a title as a heading of a level, through LaTeX's `\@startsection` in its
starred form: level 0 with the article's `\section` spacing, a page-break penalty before
it and `\huge`; levels 1 to 5 as the article's `\section`, `\subsection`,
`\subsubsection`, `\paragraph` and `\subparagraph` (levels 4 and 5 run into the paragraph
after them); a level past 5 as level 5. Inside the heading, before the title, it places the
target of the anchor; where `toc=true`, a PDF bookmark (`\pdfbookmark` at the heading's
level, so that hyperref's `bookmarksdepth` counts FLM's levels as an article counts its
sections, its text the `plain` key); and for levels 0 and 1, the running head
(`\markboth` for level 0, `\markright` for level 1) from `alt` where present, else
`plain`. These act once, where the heading itself is typeset: a `\nameref` to the heading
typesets its title alone.

A heading is written with the code registered under its kind, else under its level, else
with `\flmxHeadingLevel` at its level. The library registers no kind and no level, so a
template changes the look of one kind with `\flmxDefineHeading{section}{…}` (looked up
first), or of every heading of a level with `\flmxDefineHeading{2}{…}`; the code receives
the title as `#1` and may call `\flmxHeadingLevel` itself. `\flmxRuninHeading`'s default
sets the name a `\nameref` to the heading prints to the `plain` key, places the target,
the bookmark and the running head, and writes the title; its look is registered with
`\flmxDefineRuninHeading`.

The span of role `flm.heading.number` is written as it stands by default (the title
template supplies the separator after it); a template changes it with
`\flmxDefineSpan{flm.heading.number}{…}`.

### Links, the table of contents and page numbers in the rendered LaTeX

**Links.** Every link is written `\flmxLink[…]{<content>}`: a reference
([`refs`](crate::stdcore::features::refs)), a hyperlink
([`href`](crate::stdcore::features::href)), a term link, a citation's number, an xnote's
mark, an entry of the table of contents. From the links golden test:

```latex
See \flmxLink[kind=ref, href={\#sec-links}, target=sec-links]{§ 1} and \flmxLink[kind=ref, href={\#eq-one}, target=eq-one]{Eq. (1)}.
Visit \flmxLink[kind=url, href={https://example.org/\string~user/a\%20b_c?x=1&y=2\#frag}]{the site}, \flmxLink[kind=url, href={https://example.org/path_x/\string~y}, shows-target=true]{example.org/path\_x/\textasciitilde{}y},
```

Its keys:

- `kind`: what the link is: `ref` (a reference), `url` (a hyperlink), `term`, `cite`,
  `xnote`, `xnote-back`, `toc`, or a kind a feature adds;
- `href`: the address, always written; `#` followed by the anchor for a link inside the
  document;
- `target`: for a link inside the document, the anchor it leads to, without the `#`, so
  that a definition reaches it with `\hyperref[flmx:<target>]{…}` or
  `\pageref{flmx:<target>}` without reading the address;
- `doc-role`: the link's document role, where it has one (`noteref`, `biblioref`, …);
- `label`: a text that names the link for assistive technology, where it has one;
- `shows-target`: `true` where the content shows the address itself; absent otherwise.

The default of every kind: a link with a `target` is `\hyperref[flmx:<target>]{<content>}`,
whatever its kind, so that a `url` link to an address inside the document leads to the
target too; any other link is `\href{<href>}{<content>}`; where `shows-target=true`, the
content is set in `\flmxUrlFont` (`\ttfamily`, redefined with
`\RenewDocumentCommand\flmxUrlFont{}{…}`). The kinds `ref` and `url` are registered by the
backend's links definitions; each kind a feature writes is registered by that feature's
definition piece, described with the feature. A template changes one kind with
`\flmxDefineLink{<kind>}{<code over #1>}`. A link inside another link's content is written
as its content alone, inside the outer link, and nothing is reported.

**`hyperref`.** The style file loads `hyperref` at the end of the preamble, through the
LaTeX hook `\AddToHook{begindocument/before}{…}`, so that the packages a template loads
after the style file are loaded before it:

```latex
\AddToHook{begindocument/before}{\RequirePackage[colorlinks=true, linkcolor=flmxLinkColor, urlcolor=flmxUrlColor, citecolor=flmxCiteColor]{hyperref}}
```

The three colors are defined by the style file (`blue!60!black` for the two link colors,
`green!40!black` for citations) and redefined by a template with `\colorlet`. Other
`hyperref` options are the template's, with `\PassOptionsToPackage{…}{hyperref}` before
the style file is loaded, or with `\hypersetup` in the same hook. A template that loads
`hyperref` itself lists it in the render option `skip_packages`, which leaves the line out.

**The table of contents.** FLM builds the table of contents itself, as the piece `flm.toc`
of the [`toc`](crate::stdcore::features::toc) feature; nothing of LaTeX's own
table-of-contents machinery is used. It is a block of role `flm.toc` and kind token `toc`
holding nested lists of kind tokens `unnumbered toc`, one entry per listed heading; an
entry holds a link of kind `toc` to the heading, whose content is the heading's number in
a span of role `flm.toc.entry.number`, a space and the title. From the table-of-contents
golden test:

```latex
\begin{flmxBlock}[role=flm.toc, kinds={toc}, doc-role=toc]
\begin{flmxList}[kinds={unnumbered toc}, depth=1]
\flmxItem
\flmxLink[kind=toc, href={\#sec--Introduction}, target=sec--Introduction, shows-target=true]{\flmxSpan[role=flm.toc.entry.number]{1}
Introduction}

\begin{flmxList}[kinds={unnumbered toc}, depth=2]
\flmxItem
\flmxLink[kind=toc, href={\#sec--Background}, target=sec--Background, shows-target=true]{\flmxSpan[role=flm.toc.entry.number]{1.1}
Background}
```

The `depth` key of every list counts the nesting of lists from 1 at the outermost list. The
toc feature's definition piece defines:

| role or kind | default |
|---|---|
| block role `flm.toc` | the title `\contentsname` as an unnumbered heading of level 0, then the lists |
| list kind token `toc` | one paragraph per entry, without a marker, indented by `\flmxTocIndent` (1.5em) for each level of depth past the first |
| link kind `toc` | the title linked to the heading, a dotted leader, and the heading's page number, unlinked, flush right: `\hyperref[flmx:<target>]{#1}\nobreak\dotfill\nobreak\pageref*{flmx:<target>}` |
| span role `flm.toc.entry.number` | the number followed by `\enspace` |

A heading without an anchor is listed without a link. A template changes the look with
`\flmxDefineBlock{flm.toc}{…}{…}`, `\flmxDefineList{toc}{…}{…}`,
`\flmxDefineItem{toc}{…}`, `\flmxDefineLink{toc}{…}` (in which `\flmxKey{target}` is the
heading's anchor), `\flmxDefineSpan{flm.toc.entry.number}{…}`, and the length
`\flmxTocIndent`.

**Page numbers.** Page numbers are the one fact FLM does not know. They reach the output
through `\pageref` on the labels that every anchor writes. LaTeX therefore needs a second
run to resolve them, through its auxiliary file, as it does for any `\pageref`: compile
twice with `lualatex`, or once with `latexmk -lualatex` (which the `flm` program's
profile `pdf-latex` runs: see [PDF through
LaTeX](crate::guide::command_line#pdf-through-latex)).

### Mathematics in the rendered LaTeX

The [`math-passthrough`](crate::math_passthrough) feature's general
rules stage each formula as an `flm.pres.math-passthrough` node holding its source, with
`flm.pres.math-text` nodes inside it for the content of a text command (`\text{…}`), for
an equation line's anchor, and for a line's number or tag. The emitter writes the source
byte for byte between delimiters it chooses:

- an inline formula as `\(` … `\)`, a displayed formula without an environment as `\[` …
  `\]`;
- a displayed environment as `\begin{<env>*}` … `\end{<env>*}`, a star added to the
  environment's name unless it already ends in one, so that LaTeX numbers no line itself:
  FLM's numbers are written as `\tag*{…}` where the lines stand;
- the content of a text command as `\text{…}` (or the command the formula used), its
  content written as any other content; a formula inside it is written with its own
  delimiters `\(…\)`, since inside `\text` LaTeX is in text mode;
- an equation line's anchor as `\flmxTarget{<anchor>}` at the line's start.

Each line's source is written under the `math` library's newline rule, as in the recomposed
LaTeX (*The newline rule* under [Mathematics in the recomposed
LaTeX](#mathematics-in-the-recomposed-latex)): the run of whitespace that ends a line's
source is written after the line's tag and its `\\`. From the mathematics golden test:

```latex
\begin{equation*}\flmxTarget{eq-a}
E = mc^2 \quad \text{where \(c\) is the speed of light}
\tag*{(1)}\end{equation*}
...
\begin{align*}\flmxTarget{eq-b}
a &= b  \tag*{(2)}\\\flmxTarget{eq--4oomy}
c &= d \\\flmxTarget{eq-star}
e &= f\tag*{(\(\star\))}
\end{align*}
```

`amsmath` is always loaded by the style file. A reference to an equation, `\eqref`
included, is a link of kind `ref`. With the extension `math-mathjax` in place of
`math-passthrough`, the formulas reach this backend in the same form (the extension's
rules for `latex-rendered` stage the same elements). The backend sets no mathematics font;
`unicode-math` and the like are the template's.

### Xnotes in the rendered LaTeX

How an xnote (a footnote or another kind of note, [`xnotes`](crate::stdcore::features::xnotes))
is written follows its kind's placement
([`XnoteKind::placement`](crate::stdcore::features::xnotes::XnoteKind::placement)), as on
every backend; the backend adds no policy of its own.

**A kind collected in a list** (the placement `List`, the default of the standard
`footnote` kind): the note's mark in the text is a superscript span of role
`flm.xnotes.xnote.mark` holding a link of kind `xnote` to the note; the notes are the
piece `flm.xnotes.<kind>.list`, placed after the content: a block of role
`flm.xnotes.<kind>.list` and kind tokens `xnotes` and the kind's name, holding the kind's
title as an unnumbered heading and a labeled list, each entry's label a superscript span
of role `flm.xnotes.entry.mark`. From the golden test:

```latex
The claim holds in every case.\flmxSpan[style=superscript, role=flm.xnotes.xnote.mark, anchor=fnref--1yuhk]{\flmxLink[kind=xnote, href={\#fn-1}, target=fn-1, doc-role=noteref, label={Footnote 1}]{1}}
...
\begin{flmxBlock}[role=flm.xnotes.footnote.list, kinds={xnotes footnote}, doc-role=endnotes]
\flmxHeading[kind=xnotes, level=2, toc=false, plain={Footnotes}]{Footnotes}
\begin{flmxList}[kinds={numbered xnotes footnote labeled}, depth=1]
\flmxItem[label={\flmxSpan[style=superscript, role=flm.xnotes.entry.mark]{1}}, anchor=fn-1]
Except the empty one, which we exclude by convention.
```

With the standard configuration, a document therefore has endnotes. A document that wants
notes at the bottom of each page sets its kind's placement to `InPlace`.

**A kind shown in place** (the placement `InPlace`): LaTeX's `\footnote` needs the mark
and the text as two arguments, so the xnotes feature has a lowering rule of its own for
this backend, `flm.xnotes.xnote.lower.latex-rendered`, which writes the note where it
stands as

```latex
The claim holds in every case.\flmxXnote[kind=footnote, anchor=fn-1, doc-role=footnote]{\flmxSpan[style=superscript, role=flm.xnotes.xnote.mark, anchor=fnref--1yuhk]{1}}{Except
the empty one, which we exclude by convention.} The next sentence goes on.
```

with the keys `kind` (the note's kind), `anchor` (the note's own anchor) and `doc-role`;
the first argument is FLM's mark, carrying the target of the mark's anchor; the second is
the note's content, which may hold several paragraphs. The brackets that a kind shows
around an in-place note on other backends are not written: the LaTeX definition decides
the look. An in-place note whose content holds paragraphs is a block of the document, so
it stands between paragraphs, with its mark on a line of its own; an in-place note that
holds inline content stands inside its paragraph. For a kind collected in a list the rule
writes exactly what the general rule writes.

**The defaults** (the xnotes feature's definition piece):

| role or kind | default |
|---|---|
| block role `flm.xnotes.<kind>.list`, registered under the kind token `xnotes` | a block of paragraphs, the target of its anchor first; the heading and the list keep their own looks |
| span roles `flm.xnotes.xnote.mark` and `flm.xnotes.entry.mark` | not registered: the span's default for the style `superscript` |
| link kinds `xnote` (mark to note) and `xnote-back` (note back to its mark) | the default link |
| `\flmxXnote`, every kind | a footnote at the bottom of the page carrying FLM's mark, in the text and before the note's text (LaTeX's footnote counter is not used); the note's text starts with the target of its anchor |

A template changes the look with `\flmxDefineBlock{xnotes}{…}{…}` (every kind's list) or
`\flmxDefineBlock{flm.xnotes.<kind>.list}{…}{…}`,
`\flmxDefineSpan{flm.xnotes.xnote.mark}{…}`, `\flmxDefineSpan{flm.xnotes.entry.mark}{…}`,
`\flmxDefineLink{xnote}{…}`, `\flmxDefineLink{xnote-back}{…}`, and
`\flmxDefineXnote{<kind>}{<code over #1 (the mark) and #2 (the content)>}`; for instance
`\flmxDefineXnote{footnote}{#1\marginpar{\footnotesize #1 #2}}` writes margin notes.
`\flmxDefineXnoteDefault{…}` changes every kind that has no registration of its own.

### Spans and blocks in the rendered LaTeX

A block is the environment `flmxBlock[role, kinds, anchor, doc-role]`; its default is a
paragraph break before and after, the target of its anchor first. A span is
`\flmxSpan[style, role, kinds, anchor, doc-role]{<content>}`; its default by style is
`\emph` for `emph`, `\textbf` for `bold`, `\textit` for `italic`, `\texttt` for
`monospace`, `\underline` for `underline`, `\textsc` for `small-caps`,
`\textsuperscript` for `superscript`, `\textsubscript` for `subscript`, the color
`flmxMarkColor` for `unresolved-reference`, and the content as it stands where it has no
style. A role registered for a span replaces the look of its style, since the role is
looked up first.

### Lists in the rendered LaTeX

A list ([`lists`](crate::stdcore::features::lists), and every list another feature
writes) is the environment `flmxList`, each entry `\flmxItem[…]` on a line of its own
followed by the entry's content, up to the next `\flmxItem` or the `\end`, as with
LaTeX's `\item`, so that an entry may hold paragraphs, verbatim blocks and nested lists.
FLM computes every label (a number, a bullet, a term) and writes it in the entry's keys;
no LaTeX counter is stepped. From the lists golden test:

```latex
\begin{flmxList}[kinds={numbered enumerate labeled}, depth=1]
\flmxItem[label={1.}, anchor=item-first]
First, before \flmxLink[kind=ref, href={\#item-term}, target=item-term]{Entropy}.

\flmxItem[label={2.}, anchor=item--2otxj]
Second, holding a list:

\begin{flmxList}[kinds={unnumbered itemize labeled}, depth=2]
\flmxItem[label={•}, anchor=item--3ouhf]
Inner.
```

The keys: `kinds` (the list's kind tokens: at most one of the structural tokens
`numbered`, `unnumbered` and `terms`, first, then others, such as the environment the list
comes from, `enumerate`, `itemize` or `description`, and `labeled` where every entry
carries its own label); `anchor` (the list's or the entry's); `depth` (the list's depth
among lists, 1 for an outermost list); `label` (the entry's label, as content, where it has
one).

The default, over LaTeX's `\list` and `\item`, knows the structural tokens only:
`numbered` and `labeled` write `\item[<label>]`, the label flush right in the label box;
`unnumbered` writes `\item[<label>]` where the entry has a label, else `\item` with a
bullet by depth (`\textbullet`, `\textendash`, `\textasteriskcentered`, then
`\textperiodcentered`); `terms` sets the label in bold, run into the entry, the entry's
lines hanging at the list's indentation. A list without a structural token is written as
`unnumbered`. The default stops past six levels of nested lists, as LaTeX's `\list` does
([The library's default definitions](#the-librarys-default-definitions)).

A list and its entries are written with the code registered under the first of the list's
kind tokens that has one, else with the default. Both registrations go by the list's
kind tokens, since an entry has none of its own:
`\flmxDefineList{enumerate}{\begin{enumerate}}{\end{enumerate}}` and
`\flmxDefineItem{enumerate}{\item[\flmxKey{label}]}`. An entry's code sees the entry's
keys (`\flmxKey{label}`); the list's kind tokens and depth are `\flmxListKinds` and
`\flmxListDepth`.

### Lines and lengths in the rendered LaTeX

A group of lines (the lines of a poem or an address) is the environment `flmxLines`, each
line `\flmxLine[indent-left, indent-right, align]{<content>}`, its three keys always
written:

```latex
\begin{flmxLines}
\flmxLine[indent-left=0\flmxTabWidth, indent-right=0\flmxTabWidth, align=left]{No indentation}
\flmxLine[indent-left=1.5em, indent-right=2em, align=center]{Centered, one and a half em in}
\flmxLine[indent-left=10bp, indent-right=0bp, align=right]{Flush right, ten points in}
\end{flmxLines}
```

The default sets the group with `\parindent` and `\parskip` zero, ragged right, and each
line as a paragraph whose width is the line width less the two indentations (`\leftskip`
and `\rightskip`), its content flush left, centered or flush right in that width; a line
too long for it wraps, its following lines indented as its first. A template changes the
look with `\flmxDefineLinesDefault{…}{…}` and `\flmxDefineLineDefault{<code over #1>}`.

A length of the presentation vocabulary is spelled for LaTeX as follows: a number of tabs
as a multiple of `\flmxTabWidth` (a length of the style file, `2em` by default, changed
with `\setlength`); `em`, `mm`, `cm` and `in` as they are; a point as `bp`, since FLM's
point is 1/72 inch, which is LaTeX's big point, not its `pt`.

### Verbatim content in the rendered LaTeX

Verbatim content ([`verbatim`](crate::stdcore::features::verbatim)) is written as ordinary
LaTeX text in which every character stands for itself: the ten special characters
escaped, every space a control space `\ `, a tab four of them. No verbatim category codes
are used, so verbatim content may stand anywhere: in a list entry, a note, a table cell, a
template's argument. Inline, `\flmxVerbatim[kind, lang]{<text>}`; as a block, the
environment `flmxVerbatimBlock`, one `\flmxVerbLine{<line>}` per line:

```latex
Text \flmxVerbatim[kind=text]{\{\}\%\&\#\_\$\textbackslash{}\textasciitilde{}\textasciicircum{}\ x\ \ y}
and code \flmxVerbatim[kind=code, lang=rust]{let\ x\ =\ 1;} and an identifier \flmxVerbatim[kind=a]{my\_name}.

\begin{flmxVerbatimBlock}[kind=code, lang=python]
\flmxVerbLine{def\ f(x):}
\flmxVerbLine{}
\flmxVerbLine{\ \ \ \ pass}
\end{flmxVerbatimBlock}
```

The `kind` key is `text` (characters), `code` (computer code) or `a` (an identifier, such
as a keyword or a name); `lang` is the language the text is written in, where one was
given. The three kinds share one default: inline, the text in `\flmxVerbatimFont` (the
font switch, `\ttfamily` by default, redefined with `\renewcommand`); as a block, a
paragraph group in `\flmxVerbatimFont` with `\parindent` and `\parskip` zero, ragged
right, each line `\par\noindent\strut<line>`, between the hooks `\flmxVerbatimBlockBegin`
and `\flmxVerbatimBlockEnd`, empty by default, which a template redefines for a frame or a
background. A template distinguishes the kinds, or a language, by registering a look per
kind with `\flmxDefineVerbatim{<kind>}{<code over #1>}` and
`\flmxDefineVerbatimBlock{<kind>}{…}{…}`.

### Tables of cells in the rendered LaTeX

A table of cells ([`cells`](crate::stdcore::features::cells)) is the environment
`flmxCells`, every row of the grid holding every grid position as a column, the positions
separated by `&`, each row ended by `\\`. A cell's content stands in braces at its
top-left position, with `\par` between the cell's blocks where it holds several; an empty
position and a position that a spanning cell covers hold nothing:

```latex
\begin{flmxCells}[rows=3, cols=2, cells={{row=1, col=1, header=column}, {row=1, col=2, header=column}, {row=2, col=1}, {row=2, col=2}, {row=3, col=1}, {row=3, col=2}}]
{One} & {Two} \\
{A} & {B} \\
{C} & {D} \\
\end{flmxCells}
```

The keys: `rows` and `cols`, the size of the grid; `cells`, one braced key-value list per
cell in the table's order, with `row` and `col` (the cell's top-left position, counted
from 1), `row-span` and `col-span` (where the cell spans more than one row or column),
`align` (`left`, `center` or `right`, where the cell sets one), `header` (`column` or
`row`, where the cell heads its column or its row), `rules` (the edges at which a rule is
drawn, among `above`, `below`, `left` and `right`) and `styles` (the names of the cell's
styles).

The default sets the table with the `tblr` environment of the `tabularray` package, with
its `booktabs` library and its `varwidth` library, which measures each cell as a box of
paragraphs so that a cell may hold a list or a displayed formula: `\toprule` above the
rows and `\bottomrule` below them; for each cell, its span, its alignment, `\bfseries` for
a header, then the keys registered for each of its styles, in order; a rule along each
edge the cell's `rules` name, and below a cell that heads its column. A cell's own rule on
the table's top or bottom edge is drawn beside `\toprule` or `\bottomrule`. Columns are
left-aligned. The table is set at its natural width; where that is wider than
`\flmxCellsMaxWidth` (`0.96\linewidth`), it is set again at that width, its columns sharing
the width in proportion to their natural widths, their text wrapped. The table stands
centered between paragraphs.

A cell style is given its look with `\flmxDefineCellStyle{<name>}{<tblr cell keys>}`
(`bg=yellow!30`, `fg=blue`); a later registration replaces an earlier one, and a style
with no registration changes nothing. The cells feature registers its four custom styles
`green`, `blue`, `yellow` and `red` as light backgrounds in the colors `flmxCellsGreen`,
`flmxCellsBlue`, `flmxCellsYellow` and `flmxCellsRed`, which a template changes with
`\colorlet`; the structural styles (`align`, `head`, `rule` and their short forms) are
written as the keys `align`, `header` and `rules` and need no registration. A cell style
of a host ([extending FLM](crate::guide::extending) shows one) states, for this backend,
the definition piece that registers its keys
([`CellStyle::requires`](crate::stdcore::features::cells::CellStyle::requires), with the
backend `latex-rendered`). A template replaces the whole default with
`\flmxDefineCellsDefault{<begin code>}{<end code>}`, between which the rows stand as
written (for instance `\begin{tblr}{…}` and `\end{tblr}`, with its own options), and
changes the widest width with
`\RenewExpandableDocumentCommand\flmxCellsMaxWidth{}{…}`.

### Images in the rendered LaTeX

An image ([`graphics`](crate::stdcore::features::graphics)) is written
`\flmxImage[width, height, alt, display, anchor]{<path>}`:

```latex
\flmxImage[width=4.5cm, height=3cm, alt={A plot}, display=block]{figures/plot.png}
```

The path is the one the `graphics` feature published for this output, under one of two
strategies ([Images and how an output reaches
them](#images-and-how-an-output-reaches-them)): `reference`, the default, writes the path
of the image file as seen from the output's folder, and `copy` writes the name of a copy
of the file beside the output. The strategy `data-uri` cannot be used for this output. Under
`reference`, the path must be one LaTeX can read as a file name: a path holding `#` or `%`
stops LaTeX, and FLM reports nothing. LuaLaTeX includes PDF, PNG and JPEG files; the `flm`
program's default conversion rules turn an SVG drawing into a PDF file and a GIF image
into a PNG image under `copy`.

The keys: `width` and `height`, the size FLM computed, as LaTeX lengths, each where known;
`alt`, the alternative text, where the author gave one; `display`, `block` (the image stands
as a paragraph of its own) or `inline`, always written; `anchor`, where the image is a
reference target. The default is `\includegraphics` with the width and the height where
given (at its natural size where neither is), inside `\par\centering … \par` for
`display=block`, in the line otherwise, the target of the anchor first; `alt` is not
typeset. A template changes it with `\flmxDefineImageDefault{<code over #1, the path>}`.

### Decorations and the marks of what could not be written

The three decorations ([Decorations](#decorations)) are written where their marks stand:
`\flmxDecoRunin[stop]{…}` at the start of the paragraph, by default the content and the
stop in bold, then a space of `\flmxRuninSep` (0.8em); `\flmxDecoFinal{…}` at the end of
the last paragraph, by default set flush right at the end of the last line, in a paragraph
of its own where it stands alone (after a list, for instance); `\flmxDecoMargin{…}`, by
default a margin note in `\footnotesize`. Each default is replaced with
`\flmxDefineDecoRuninDefault`, `\flmxDefineDecoFinalDefault` or
`\flmxDefineDecoMarginDefault`.

What FLM could not write is shown by one of three marks, visible by default in red
(the color `flmxMarkColor`) and in the typewriter font:
`\flmxDropped{<condition>}{<detail>}` (something left out, `condition` naming the
diagnostic reported for it; default `[dropped: <detail>]`),
`\flmxUnsupported{<element>}{<message>}` (an element this output cannot write; default
`[unsupported: <element>]`) and `\flmxUnknown[kind=<macro or
environment>]{<name>}{<spelling>}` (a macro or an environment no feature knows; default
`[unknown: <spelling>]`). A template changes them with `\flmxDefineDroppedDefault`,
`\flmxDefineUnsupportedDefault` and `\flmxDefineUnknownDefault`, whose code is over `#1`
and `#2`; code that writes nothing hides the marks. An unresolved reference is a span of
style `unresolved-reference`, in `flmxMarkColor` by default.

### References and hyperlinks in the rendered LaTeX

A reference ([`refs`](crate::stdcore::features::refs)) is a link of kind `ref` whose
content is the reference's text as FLM resolved it (`§ 1`, `Eq. (1)`, `Theorem 1`), and
whose `target` is the anchor of the reference point: `\hyperref[flmx:<target>]{…}` by
default. A hyperlink ([`href`](crate::stdcore::features::href): `\href`, `\url`,
`\email`) is a link of kind `url`: `\href{<href>}{…}` by default, its content in
`\flmxUrlFont` where it shows the address (`\url`, `\email`). Both kinds are registered by
the backend's own link definitions; neither feature has a definition piece. An unresolved
reference is a span of style `unresolved-reference` ([Decorations and the marks of what
could not be written](#decorations-and-the-marks-of-what-could-not-be-written)).

### The title, the authors and the date in the rendered LaTeX

The [`meta`](crate::stdcore::features::meta) feature places three pieces before the
content, each a block of kind token `meta` and of the role of what it shows:

```latex
\begin{flmxBlock}[role=flm.meta.title, kinds={meta}]
On \flmxSpan[style=emph]{flexible} markup and \(x^2\)
\end{flmxBlock}

\begin{flmxBlock}[role=flm.meta.author, kinds={meta}]
\flmxSpan[role=flm.meta.author.name]{Ada Lovelace}, \flmxSpan[role=flm.meta.author.name]{Grace \flmxSpan[style=bold]{Hopper}}
\end{flmxBlock}

\begin{flmxBlock}[role=flm.meta.date, kinds={meta}]
September 2026
\end{flmxBlock}
```

The meta feature's definition piece sets them as LaTeX's `\maketitle` sets a title block,
with its spacing: every line centered; the title (`flm.meta.title`) in `\LARGE\bfseries`
after 2em of space, then 1.5em; the authors (`flm.meta.author`) in `\large`, then 1em; the
date (`flm.meta.date`) in `\normalsize`, then 1.5em; each author's name
(`flm.meta.author.name`) as it stands. A template registers each role with
`\flmxDefineBlock` and `\flmxDefineSpan`; the kind token `meta` is reached only where a role
has no registration.

### Defined terms in the rendered LaTeX

A term definition ([`defterm`](crate::stdcore::features::defterm)) is a block of role
`flm.defterm`; its lead-in, where one is configured, is a run-in decoration at the start
of its first paragraph holding a span of role `flm.defterm.leadin`; a term link inside the
definition of its own term is a span of role `flm.defterm.term`; any other term link is a
link of kind `term`:

```latex
\begin{flmxBlock}[role=flm.defterm, anchor=defterm-Entropy]
\flmxDecoRunin{\flmxSpan[role=flm.defterm.leadin]{Entropy: }}A measure of disorder: the \flmxSpan[role=flm.defterm.term]{Entropy}
of a system never decreases.
\end{flmxBlock}

The \flmxLink[kind=term, href={\#defterm-Entropy}, target=defterm-Entropy]{Entropy} of a gas,
```

The defaults: the definition as a block of paragraphs, the target of its anchor first; the
lead-in as it stands (the decoration sets it in bold); the term in `\bfseries`; the link of
kind `term` as the default link.

### Theorems in the rendered LaTeX

A theorem ([`theorems`](crate::stdcore::features::theorems)) is a block of role
`flm.theorems.theorem` whose kind tokens are its theorem class's name, then its theorem
kind's. Its heading is a run-in decoration at the start of the first paragraph, the kind's
word and number in a span of role `flm.theorems.name`, the title in a span of role
`flm.theorems.title`; a class with a closing mark (the shipped class `prooflike`) ends the
last paragraph with the mark in `\flmxDecoFinal`:

```latex
\begin{flmxBlock}[role=flm.theorems.theorem, kinds={theoremlike theorem}, anchor=theorem-1]
\flmxDecoRunin[stop={.}]{\flmxSpan[role=flm.theorems.name]{Theorem 1} (\flmxSpan[role=flm.theorems.title]{My
theorem})}The square root of two is irrational.
\end{flmxBlock}
\begin{flmxBlock}[role=flm.theorems.theorem, kinds={prooflike proof}]
\flmxDecoRunin[stop={.}]{\flmxSpan[role=flm.theorems.name]{Proof} (\flmxSpan[role=flm.theorems.title]{\flmxLink[kind=ref, href={\#theorem-1}, target=theorem-1]{Theorem 1}})}We
argue by contradiction.\flmxDecoFinal{□}
\end{flmxBlock}
```

The theorems feature's definition piece registers the shipped classes' kind tokens:

| class token | default |
|---|---|
| `theoremlike` | a group of paragraphs with `\medskip` before and after, the first not indented, the body in `\itshape` |
| `definitionlike` | the same, the body in `\normalfont` (upright, also inside a theorem-like theorem) |
| `prooflike` | the same, the body in `\normalfont` |

In these three, the heading is upright and in the text's weight, the name in `\bfseries`
(in `\itshape` for `prooflike`) and the title as it stands; a later run-in decoration in
the body keeps its own look; the closing mark is written with the decoration's default. A
theorem of a class of the host's own has no registration and is written with the block's
default. A theorem is looked up under its role, then its class token, then its kind token:
registering the role `flm.theorems.theorem` replaces every class's look, and a kind token
(`theorem`, `lemma`, …) is reached only for a class that has no registration ([the
example](#registering-a-look) registers a class token). The name and the title are
registered with `\flmxDefineSpan{flm.theorems.name}{…}` and
`\flmxDefineSpan{flm.theorems.title}{…}`. The closing mark `□` is not in the default font,
Latin Modern ([The library's default
definitions](#the-librarys-default-definitions)).

### Quotations in the rendered LaTeX

A quotation ([`quote`](crate::stdcore::features::quote)) is a block of role `flm.quote`
whose kind token is the environment's name (`quote`, `blockquote`, `address`, or one the
host configures), holding one block per section of the quotation, of role
`flm.quote.text` (a passage), `flm.quote.lines` (lines, as `flmxLines`),
`flm.quote.attributed` (an attribution) or `flm.quote.block` (paragraphs and blocks):

```latex
\begin{flmxBlock}[role=flm.quote, kinds={quote}]
\begin{flmxBlock}[role=flm.quote.block]
one line
\end{flmxBlock}
\begin{flmxBlock}[role=flm.quote.lines]
\begin{flmxLines}
\flmxLine[indent-left=0\flmxTabWidth, indent-right=0\flmxTabWidth, align=left]{a}
```

The defaults: every quotation as LaTeX's `quote` environment sets one, indented on both
sides, whatever its kind token; an attribution flush right; the other sections with the
block's default, the lines with the default of `flmxLines`. The quote feature's piece
registers the role `flm.quote`, and a registration under the role is found before one
under a kind token, so a template that wants a look per environment registers the role
again and chooses inside its code by the value of `\flmxKey{kinds}`. The sections are
registered by their roles (`\flmxDefineBlock{flm.quote.attributed}{…}{…}`).

### Annotations in the rendered LaTeX

An annotation ([`annotations`](crate::stdcore::features::annotations)) is a span of role
`flm.annotations.highlight` (a highlight) or `flm.annotations.comment` (a comment), whose
kind tokens are the annotator's command name and `annotator-<n>`, `<n>` being the
annotator's number counted from 0; the annotator's initials stand at the start, in a span
of role `flm.annotations.initials` (with the same kind tokens) inside a margin decoration:

```latex
\flmxSpan[role=flm.annotations.highlight, kinds={phf annotator-0}]{\flmxDecoMargin{\flmxSpan[role=flm.annotations.initials, kinds={phf annotator-0}]{PhF}}passage}
```

An annotation cut around a block (a list inside a highlighted passage) is a block of the
same role. A cut part also carries the kind token `mark-continues` where a part follows it,
and `mark-continued` where one precedes it.

The defaults: annotator `<n>` has the color `flmxAnnotatorColor<n>` for `<n>` from 0 to 7
(eight colors in the order of the `phfcc` package's presets), and
`flmxAnnotatorColorFallback` (red) for any other number. A highlight is in its annotator's
color; a comment is in the color, in `\sffamily` and between brackets, the opening one
left out on a part marked `mark-continued` and the closing one on a part marked
`mark-continues`; the initials are in the color and in `\sffamily`, in the margin
decoration's default. A template changes the colors with `\colorlet` and the looks by
registering the three span roles and the two block roles, in which
`\flmxAnnotatorColor` expands to the name of the color of the annotator the element's
kind tokens give. With the render option
[`AnnotationsRenderOptions::hidden`](crate::stdcore::features::annotations::AnnotationsRenderOptions::hidden),
the annotations are not written, as on every backend.

### Floats in the rendered LaTeX

A float ([`floats`](crate::stdcore::features::floats)) is a block of role
`flm.floats.float` whose kind tokens are the float type's name (`figure`, `table`, or a
type the document configures) and `numbered` or `unnumbered`. It holds the content in a
block of role `flm.floats.float.content` and, where the float is numbered or has a
caption, the line under the content in a block of role `flm.floats.float.caption` (with the
same kind tokens), whose number is in a span of role `flm.floats.float.number`:

```latex
\begin{flmxBlock}[role=flm.floats.float, kinds={figure numbered}, anchor=figure-1]
\begin{flmxBlock}[role=flm.floats.float.content]
\flmxImage[width=4.5cm, height=3cm, alt={A plot}, display=block]{figures/plot.png}
\end{flmxBlock}
\begin{flmxBlock}[role=flm.floats.float.caption, kinds={figure numbered}]
\flmxSpan[role=flm.floats.float.number]{Figure 1}: A plot.
\end{flmxBlock}
\end{flmxBlock}
```

The floats feature's definition piece registers the kind tokens `figure` and `table` as
LaTeX's `figure` and `table` environments, placed `[htbp]`, their content centered, the
target of the anchor first; the content as it stands; the caption a paragraph in `\small`,
`\abovecaptionskip` below the content; the number in `\bfseries`. A float of a type the
document configures has no registration and is written with the block's default, where it
stands. LaTeX's own `\caption` is not used: FLM writes the number and the caption, and
LaTeX's list of figures does not see them. A template registers a type with
`\flmxDefineBlock{<type>}{…}{…}` (for instance
`\flmxDefineBlock{video}{\begin{figure}[htbp]\centering}{\end{figure}}`), every float
with the role `flm.floats.float`, and the parts with their roles.

### Citations in the rendered LaTeX

A citation ([`citations`](crate::stdcore::features::citations)) is a span of role
`flm.citations.space` holding a no-break space, then the citation mark, a span of role
`flm.citations.mark` (kind tokens `attached` or `standalone`, then `in-place` where the
kind places the entries in the text) holding the delimiters, the join texts and the
numbers, each number a link of kind `cite` to its work's entry; `\R` writes its word
before the mark in a span of role `flm.citations.word`:

```latex
As shown in \flmxSpan[role=flm.citations.word]{ref. }\flmxSpan[role=flm.citations.mark, kinds={standalone}]{[\flmxLink[kind=cite, href={\#citation-3}, target=citation-3, doc-role=biblioref]{3}]}
```

The list of cited works is a block of role `flm.citations.bibliography` (kind token
`citations`) holding its title as an unnumbered heading and a labeled list whose kind
tokens include `citations`, each entry's label a span of role
`flm.citations.entry.number`. The defaults: the space, the word and the entry's number as
they stand; the list of cited works as a block of paragraphs, the target of its anchor
first; the link of kind `cite` as the default link, in the color `flmxCiteColor`. The
mark's span is not registered: the span's default sets it as it stands, or raised under
the style `superscript`; registering its role replaces that look. A template registers
the four span roles, the block role, the list and its entries under the kind token
`citations` (`\flmxDefineList{citations}{…}{…}`, `\flmxDefineItem{citations}{…}`), and
the link kind `cite`.

### User definitions in the rendered LaTeX

Every user definition ([`userdefs`](crate::stdcore::features::userdefs)) is expanded
before this backend writes anything, as on HTML: the rendered LaTeX holds the result of
each call, never a LaTeX definition of the author's macro.

### The style file of the rendered LaTeX

The style file is the artifact `latex-rendered:<stem>.<hash>`, of media type
`application/x-latex` and extension `sty` ([`style`](crate::stdcore::backends::latex_rendered::style)).
`<stem>` is the render option `style_stem` (`flmx` by default) and `<hash>` eight
hexadecimal digits of a digest of the file's content, so that two documents that need the
same definitions share one file name. The program's naming of files may add a prefix: the
`flm` program writes `paper-flmx.1a2b3c4d.sty` beside `paper.tex`. The document loads the
file by its package name, the file name without `.sty`
(`\usepackage{paper-flmx.1a2b3c4d}`). The style file is always written, since every
document needs the core of the contract. Its content, in order, each block under a comment
line naming it:

1. `\NeedsTeXFormat{LaTeX2e}[2022/06/01]` and `\ProvidesPackage{<name>}`: the file needs a
   LaTeX of 2022 or later;
2. `\RequirePackage{iftex}` and `\RequireLuaTeX`, which stops a compilation under any
   engine other than LuaLaTeX with the `iftex` package's own message;
3. the author's text written before the packages (the render option `extra_preamble`);
4. the package block: one `\RequirePackage[<options>]{<package>}` line per package, each
   under a comment naming the identifiers that requested it: `fontspec`, `amsmath` and
   `xcolor` always, then what the document's definition pieces need (`graphicx` for
   images, `tabularray` for tables of cells, …), then the author's own requests (the render
   option `extra_packages`). A package listed in the render option `skip_packages` is left
   out;
5. the line that loads `hyperref` at the end of the preamble ([Links, the table of
   contents and page numbers in the rendered
   LaTeX](#links-the-table-of-contents-and-page-numbers-in-the-rendered-latex)), unless
   `hyperref` is in `skip_packages`;
6. the author's text written after the packages;
7. the definition pieces: the core of the contract, then every other piece the document
   requires, each under a comment line naming its identifier;
8. the definitions that lowering rules staged as definition constructs, each once by name;
9. the author's text written at the end;
10. `\endinput`.

The beginning of a style file, from the headings golden test:

```latex
\NeedsTeXFormat{LaTeX2e}[2022/06/01]
\ProvidesPackage{flmx.7938e330}
% the definitions of a document written by FLM's latex-rendered backend
% the engine: LuaLaTeX
\RequirePackage{iftex}
\RequireLuaTeX
% the packages
% flm.latex.pkg.fontspec
\RequirePackage{fontspec}
% flm.latex.pkg.amsmath
\RequirePackage{amsmath}
% flm.latex.pkg.xcolor
\RequirePackage{xcolor}
% hyperref, loaded at the end of the preamble
\AddToHook{begindocument/before}{\RequirePackage[colorlinks=true, linkcolor=flmxLinkColor, urlcolor=flmxUrlColor, citecolor=flmxCiteColor]{hyperref}}
% flm.latex.def.flmx.core
```

Every definition piece of the style file opens with a comment block that says what it
defines, which keys each command takes, and how a template changes it: the style file is
the reference for a template's author, beside this chapter.

The backend's own definitions are nine pieces, each required by the emitter where it writes
an element of the concern:

| identifier | defines | requires |
|---|---|---|
| `flm.latex.def.flmx.core` | the registration commands, `\flmxKey`, `\flmxIfKey`, `\flmxTarget`, the lengths `\flmxTabWidth` and `\flmxRuninSep`, the colors, `flmxBlock`, `\flmxSpan`, the three decorations | `fontspec`, `amsmath`, `xcolor` |
| `flm.latex.def.flmx.headings` | `\flmxHeading`, `\flmxRuninHeading`, `\flmxHeadingLevel` | the core |
| `flm.latex.def.flmx.links` | `\flmxLink`, the kinds `ref` and `url`, `\flmxUrlFont` | the core |
| `flm.latex.def.flmx.lists` | `flmxList`, `\flmxItem`, `\flmxListKinds`, `\flmxListDepth` | the core |
| `flm.latex.def.flmx.lines` | `flmxLines`, `\flmxLine` | the core |
| `flm.latex.def.flmx.verbatim` | `\flmxVerbatim`, `flmxVerbatimBlock`, `\flmxVerbLine`, `\flmxVerbatimFont`, `\flmxVerbatimBlockBegin`, `\flmxVerbatimBlockEnd` | the core |
| `flm.latex.def.flmx.cells` | `flmxCells`, `\flmxCellsMaxWidth` | the core, `tabularray` |
| `flm.latex.def.flmx.images` | `\flmxImage` | the core, `graphicx` |
| `flm.latex.def.flmx.marks` | `\flmxDropped`, `\flmxUnsupported`, `\flmxUnknown` | the core |

Mathematics needs no piece of its own, since `amsmath` is always loaded.

### Style identifiers and definition pieces

A feature gives its roles and kind tokens their look on this backend through a
**definition piece**: a unit of LaTeX definitions registered in the LaTeX family's
service slot `flm.latex.preamble` under an identifier `flm.latex.def.<name>`
([`LatexPreambleDef`](crate::stdcore::backends::latex::LatexPreambleDef)). The standard
features' pieces are named `flm.latex.def.flmx.<feature>` and register their looks with
the commands of [Registering a look](#registering-a-look).

A feature's general lowering rule does not know which backend it writes for, so it does
not require a LaTeX piece by name. It requires the feature's **style identifier**
(`flm.theorems.style`, `flm.quote.style`, …) at every node it stages, the identifier under
which the same feature registers its stylesheet for the HTML page. Each backend resolves
that identifier in its own way: the HTML backend writes the stylesheet; the plain-text
backend reads no requirement; this backend writes every definition piece that declares it
**delivers** the identifier
([`LatexPreambleDef::delivers`](crate::stdcore::backends::latex::LatexPreambleDef::delivers),
set with [`delivering`](crate::stdcore::backends::latex::LatexPreambleDef::delivering)). A
document that holds a theorem therefore holds the theorems piece in its style file, and a
document without one does not.

One feature has nothing to style on the HTML page but a look to give on this backend: the
headings feature. It registers an **empty style definition**, `flm.headings.style`, a style
definition that holds no stylesheet. It exists so that the feature's general rules have an
identifier to require that each backend's definitions can deliver: the HTML page writes
nothing for it, and on this backend the feature's piece delivers it. The meta feature's
identifier, `flm.meta.style`, names its stylesheet on the HTML page and its piece on this
backend.

This backend resolves each identifier that a tree, a piece or the emitter requires
([`resolve`](crate::stdcore::backends::latex_rendered::resolve)):

1. `flm.latex.pkg.<name>`, a LaTeX package request, through the family's package slot;
2. `flm.latex.def.<name>`, a definition piece, through the family's preamble slot, with
   the pieces each piece requires in turn;
3. any other identifier through the pieces that deliver it, every one of them;
4. an identifier that no piece delivers but that the registry holds as a style or script
   definition is another backend's, and writes nothing;
5. an identifier known nowhere is the Error `flm.latex-rendered.unknown-requirement`
   ("the requirement `…` names no LaTeX package request, no definition piece, and no style
   or script definition"), and is left out.

**One owner per name.** Each role, kind token and link kind is registered by one piece:
a feature's piece registers the roles, kind tokens and link kinds its own feature writes,
and the backend's links piece registers the kinds `ref` and `url`, which belong to features
without a piece. A test checks that no name is registered twice across the shipped pieces.
A template that wants another look registers the name again; its registration, loaded
after the style file, wins.

**A piece of your own.** A feature of yours ships a piece for this backend with
`LatexPreambleDef::new(…).requires([…]).delivering([…])`, contributed to the slot
`flm.latex.preamble` ([extending FLM](crate::guide::extending#definition-pieces-for-the-rendered-latex)).
An extension written in JavaScript or as data cannot contribute a definition piece yet: its
style identifiers resolve on this backend as another backend's definitions, and write
nothing.

### Rendered LaTeX render options

[`LatexRenderedRenderOptions`](crate::stdcore::backends::latex_rendered::LatexRenderedRenderOptions)
holds the backend's render options. They are declared with
[`Bind::Render`](crate::model::options::Bind::Render): their values come from the render
request rather than from the document, and they are part of the plan's identity. The `flm`
program sets them with `-O latex-rendered.KEY=VALUE`.

| option | default | what it does |
|---|---|---|
| `style_stem` | `flmx` | the style file's name before its hash; the program's naming of files may add a prefix (`paper-flmx.1a2b3c4d.sty`) |
| `run_in_stop` | `.` | the stop written as the key `stop` after a run-in heading's title and a run-in decoration's content; the empty text writes none |
| `skip_packages` | empty | LaTeX packages this backend never requests, for a template that loads them itself; `hyperref` leaves out the line that loads it. The author's own requests are not left out |
| `extra_packages` | empty | LaTeX packages the style file always requests, with their options, merged after the requests of the document's content into one `\RequirePackage` line per package |
| `extra_preamble` | empty | three parts of LaTeX text written byte for byte into the style file: after the engine check, after the line that loads `hyperref`, and at the end |

Two requests that give one package option two different values are the Error
`flm.latex-rendered.conflicting-package-options`, and the package is not requested. There
is no option for the Unicode handling or for the definitions' names: on this backend the
font supplies the characters, and the names are those of the contract. Three other
features' render options also change this output:
[`AnnotationsRenderOptions::hidden`](crate::stdcore::features::annotations::AnnotationsRenderOptions::hidden),
the `graphics` feature's strategy for this output
([`GraphicsRenderOptions`](crate::stdcore::features::graphics::GraphicsRenderOptions)),
and [`PiecesRenderOptions`](crate::stdcore::backends::pieces::PiecesRenderOptions), which
chooses the pieces placed beside the content.

### The stages, the tables and the artifact set of the rendered LaTeX

For each document, the backend's stages run as follows:

1. the shared lowering stage writes the lowered trees into `flm.lowered.latex-rendered`;
   the shared pieces stage lowers every piece the render places (every piece by default)
   into `flm.lowered.latex-rendered.pieces`;
2. the emit stage `flm.latex-rendered.emit`, once per tree, writes each lowered tree's
   LaTeX into `flm.output.latex-rendered`
   ([`OUTPUT_TABLE`](crate::stdcore::backends::latex_rendered::OUTPUT_TABLE)), together
   with the identifiers it requires and the definitions it staged;
   `flm.latex-rendered.emit-pieces` writes each piece's LaTeX and its words into
   `flm.output.latex-rendered.pieces`
   ([`PIECES_TABLE`](crate::stdcore::backends::latex_rendered::PIECES_TABLE));
3. the regions stage `flm.latex-rendered.regions` writes each tree placed inside another
   where it is placed, resolves every requirement, assembles the style file and names it,
   and writes the regions `content`, one region per piece and `style-package` (the style
   file's package name) into `flm.output.latex-rendered.regions`
   ([`REGIONS`](crate::stdcore::backends::latex_rendered::REGIONS)), and the style file
   into `flm.output.latex-rendered.style`
   ([`STYLE_FILE`](crate::stdcore::backends::latex_rendered::STYLE_FILE));
4. the final stage `flm.latex-rendered.final` writes the document as the entry artifact
   `latex-rendered:document` of `flm.output.latex-rendered.document`
   ([`DOCUMENT`](crate::stdcore::backends::latex_rendered::DOCUMENT)), followed by the
   style file and the image copies of `flm.output.latex-rendered.artifacts`
   ([`ARTIFACTS`](crate::stdcore::backends::latex_rendered::ARTIFACTS)).

The final stage writes a fixed skeleton, a complete document of the class `article`; from
the headings golden test:

```latex
% flm: latex-rendered; style file flmx.7938e330.sty
\documentclass{article}
\usepackage{flmx.7938e330}
\begin{document}
\flmxHeading[kind=chapter, level=0, toc=true, anchor=sec--Opening, number={1}, plain={1. Opening}]{\flmxSpan[role=flm.heading.number]{1}.
Opening}
...
\end{document}
```

The pieces placed before the content (the title, the authors, the date, the table of
contents) stand after `\begin{document}`, and the pieces placed after it (the lists of
notes, the bibliography) before `\end{document}`, each in the order of the piece slot. A
host assembles the same regions through a template of its own instead ([Regions and the
final stage](#regions-and-the-final-stage)).

The emitter writes each block-level element on lines of its own, with one blank line
between two paragraphs and between a paragraph and a block; inline content flows, and a
line is broken at a space of document text once it passes 100 characters, never inside a
key-value argument, verbatim text or mathematics. It runs on a worker thread under a depth
limit, as the HTML emitter does: a tree nested too deeply is the Error
`flm.latex-rendered.too-deep`, and a worker thread that cannot be started is
`flm.latex-rendered.worker-thread`. The other conditions of this backend are
`flm.latex-rendered.stage-failed` (a table could not be read or written),
`flm.latex-rendered.style-path` (the program's naming of files gave the style file a path
that is not a bare file name ending in `.sty`; the file is then named `<stem>.<hash>.sty`),
`flm.latex-rendered.missing-subtree`, `flm.latex-rendered.subtree-cycle` and
`flm.latex-rendered.piece-placement` (a tree placed where it cannot be written), besides
the two of the requirements above. Every one is an Error.

### Compose templates for the rendered LaTeX, and compiling

The `flm` program ships compose templates for the `latex-rendered` output, which `flm
render --help` lists under the option `-t`. The default, `simple`,
is a complete document: the class and its options from the template's settings, the style
file loaded by its package name, the pieces placed before the content, the content, and the
pieces placed after it. For a LaTeX file of one's own that loads the style file and includes
the content, `-t none` gives the content alone.

The `simple` template is written in the template engine `minijinja` with delimiters that
do not collide with LaTeX's braces: statements stand between `<<*` and `*>>`, values between
`<<<` and `>>>`, comments between `<<!--` and `-->>`:

```latex
<<!-- flm latex-rendered: the simple template -->>
\documentclass<<* if config.classoptions *>>[<<< config.classoptions >>>]<<* endif *>>{<<< config.documentclass >>>}
\usepackage{<<< regions["style-package"] >>>}
\begin{document}
<<* for name, piece in pieces|items if piece.place == "before-content" *>>
<<< piece.text >>>
<<* endfor *>>
<<< regions.content >>>
<<* for name, piece in pieces|items if piece.place == "after-content" *>>
<<< piece.text >>>
<<* endfor *>>
\end{document}
```

Its settings are `documentclass` (`article` by default) and `classoptions` (empty by
default), set under the output's name and the template's name: `-O
'compose.template_config={latex-rendered: {simple: {documentclass: report}}}'`. A
template of one's own registers its looks after the line `\usepackage{…}` ([Registering a
look](#registering-a-look)). With `-o out/`, the program writes `out/index.tex` and the
style file `out/index-flmx.1a2b3c4d.sty`; with `-o paper.tex`, `paper.tex` and
`paper-flmx.1a2b3c4d.sty`.

The result is compiled with `lualatex`, twice so that the page numbers resolve, or with
`latexmk -lualatex`, which runs LuaLaTeX as often as needed. No other engine is accepted.
The `flm` program makes the PDF file in one command with the profile `pdf-latex`, the
extension `profile-pdf-latex`, which compiles the output with `latexmk -lualatex` in a temporary
folder and writes the PDF alone (see [PDF through
LaTeX](crate::guide::command_line#pdf-through-latex)).

## The recomposed LaTeX backend

The `latex-recomposed` backend
([`crate::stdcore::backends::latex_recomposed`]) writes a document back as LaTeX source,
close to the way an author would have typed it, and leaves every resolution to LaTeX:
LaTeX and the author's document class number the headings, the equations and the
theorems, resolve the cross-references, and build the table of contents. This chapter
calls the LaTeX document the backend writes the **recomposed LaTeX**; the other LaTeX
output, which writes resolved content for LuaLaTeX, is described in [The rendered LaTeX
backend](#the-rendered-latex-backend) above. Where the document
needs one, the backend also writes a **style file**: a LaTeX package (a `.sty` file) holding
exactly the definitions that the recomposed LaTeX needs in order to compile
([`style`](crate::stdcore::backends::latex::style); when it is written: [The stages, the
tables and the artifact set](#the-stages-the-tables-and-the-artifact-set)).

A caller asks for the `latex-recomposed` output, or for the `latex-recomposed` workflow,
and reads the artifact set from the table `flm.output.latex-recomposed.document`
([`DOCUMENT`](crate::stdcore::backends::latex_recomposed::DOCUMENT)). The entry artifact is
the recomposed LaTeX; the style file is a second artifact, whose file name the recomposed LaTeX's first line
gives (the program's naming of files gives it, `paper-flmrecomposed.1a2b3c4d.sty` under
`-o paper.tex`; without one, `<stem>.<hash>.sty`); image copies, where the output copies images,
follow ([The stages, the tables and the artifact
set](#the-stages-the-tables-and-the-artifact-set)). The recomposed LaTeX holds the
document's body only, with no `\documentclass` and no `\begin{document}`: the LaTeX file
that includes it provides those, and loads the style file with
`\usepackage{NAME}` in its preamble, NAME being the style file's name without `.sty`.

The output is LaTeX that a person reads and edits. Three things differ from what the
author typed, each for a reason given in its section:

- a call of a user definition is written in a fixed shape, under a name of its own, and the
  definition is written into the style file ([User definitions in the recomposed
  LaTeX](#user-definitions-in-the-recomposed-latex));
- with the default render options, headings, labels and references are written through
  *LaTeX hooks*, macros of the style file that a user may redefine ([The LaTeX
  hooks](#the-latex-hooks)), and every label is written under a generated name, its safe label
  ([References and safe labels](#references-and-safe-labels));
- the syntax of every macro call and environment is normalized ([What the backend writes,
  and from which tree](#what-the-backend-writes-and-from-which-tree)).

The chapter covers, in order: the LaTeX family the backend is built on; what each element
is written as, then one section per feature with LaTeX of its own (lists, xnotes,
hyperlinks, defined terms, theorems, quotations, annotations, images, floats, tables of
cells, citations, pieces); the LaTeX hooks; escaping and the Unicode modes; references and
safe labels; mathematics; user definitions; verbatim content; the stages, tables and
artifacts; the render options; and an example rendering one document both as plain text
and as LaTeX.

### What the LaTeX family provides

The backend is built over the **LaTeX family**,
[`crate::stdcore::backends::latex`], which the `latex` feature
([`LatexFamily`](crate::stdcore::backends::latex::LatexFamily)) registers. The family holds
what a LaTeX backend needs beside its own stages:

- the four **construct elements** of LaTeX
  ([`construct`](crate::stdcore::backends::latex::construct)): a macro call, an
  environment, a raw node (LaTeX text written byte for byte) and a definition, named
  `flm.pres.latex.*`. A construct element is a presentation element that mirrors one
  construct of an output format, so that LaTeX syntax is structured data in the lowered
  tree rather than a string ([backends, lowering and
  outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)). The lowering
  rules of this backend stage construct elements;
- the escaping of document text ([`escape`](crate::stdcore::backends::latex::escape);
  [Escaping and the Unicode modes](#escaping-and-the-unicode-modes));
- the **typed sink** ([`LatexSink`](crate::stdcore::backends::latex::LatexSink)), the only
  interface through which the emitter writes bytes: document text goes through the
  escaping, byte-exact content (mathematics, verbatim content, raw nodes) is written
  unchanged, and the emitter's own syntax (braces, `\begin`, a comment) is written by
  methods that write that syntax and nothing else;
- the two service slots through which features contribute what a document needs in order
  to compile: **package requests** and **definition pieces** ([LaTeX package requests,
  definition pieces and
  hooks](crate::guide::concepts_overview#latex-package-requests-definition-pieces-and-hooks)).
  A package request is a LaTeX package that a lowering rule or the emitter requires by
  identifier (`flm.latex.pkg.<name>`), resolved through the service slot
  `flm.latex.packages` to a package definition
  ([`packages`](crate::stdcore::backends::latex::packages)). A definition piece is a unit
  of LaTeX definitions that a feature registers under an identifier
  `flm.latex.def.<name>`, together with the identifiers it needs in turn
  ([`LatexPreambleDef`](crate::stdcore::backends::latex::LatexPreambleDef)). A rule
  *requires* a package or a definition piece by recording its identifier as a requirement of the
  lowered tree; the backend's regions stage resolves the requirements and writes the
  style file;
- the assembly of the style file
  ([`assemble_style`](crate::stdcore::backends::latex::assemble_style));
- the names under which user definitions are written
  ([`names`](crate::stdcore::backends::latex::names)).

Two rules of the sink matter to an author of a lowering rule for this backend:

- **A comment ends its own line.** A comment is written only by
  [`LatexSink::comment`](crate::stdcore::backends::latex::LatexSink::comment), which also
  writes the newline that ends the comment's line. A comment runs to the end of its line,
  so a comment whose line did not end would turn what is written next into comment text.
  A rule that wants a comment stages a comment node.
- **A raw node must not contain a `%` that starts a comment.** The library writes every
  `%` it means literally as `\%`, and every `%` the author wrote belongs to a comment node,
  so such a `%` can only be a mistake of a lowering rule. The sink refuses the text, and the
  emitter reports `flm.latex-recomposed.stage-failed` at the node it came from. A rule that
  wants a per cent sign stages it as text.

### What the backend writes, and from which tree

The backend's native set
([`native_elements`](crate::stdcore::backends::latex_recomposed::native_elements)) holds
fifteen elements:

- the four construct elements of the family;
- `flm.pres.math-passthrough` and `flm.pres.verbatim`, whose content LaTeX reads byte for byte;
- `flm.pres.unknown`, a macro or environment that no feature declares, written as the
  author wrote it;
- `flm.pres.unsupported`, the node that stands for an [accepted unrenderable
  element](#accepted-unrenderable-elements), which every backend must hold in its native
  set;
- `flm.pres.dropped`, the dropped mark a rule leaves where it left something out;
- `flm.pres.subtree`, the place of another tree of the document, where the recomposed
  LaTeX holds that tree's LaTeX;
- `flm.pres.span`, written as its content: its role and its kinds have no meaning for
  LaTeX, so a rule that stages a span on every backend (the rule of last resort of a math
  group among them) is written on this one too;
- four of the [structural elements](crate::guide::concepts_overview#structural-elements):
  `flm.text`, `flm.group`, `flm.comment` and `flm.container`.

The native set holds no other presentation element (no paragraph or heading node),
because no rule for this backend stages one. Instead, each standard feature registers a
**recomposition rule** for each of its elements: a lowering rule that stages construct
elements, marked as the default on the `latex-recomposed` backend alone. For instance,
`\emph` belongs to the `textstyles` feature, and so does the rule that writes it back
([`textstyles::latex`](crate::stdcore::features::textstyles::latex)). The feature that
owns a recomposition rule registers none of the things the rule refers to: the backend, the
construct elements, and the tables of other features that the rule reads. It makes a peer
declaration ([`PeerName`](crate::core::registry::PeerName)) of each, a forward declaration
of something another feature owns, which the registry build resolves once every feature
has registered. A registry that holds a feature without the LaTeX features therefore still
builds, with the feature's recomposition rule set aside (excluded from rule selection; see
[`PeerName`](crate::core::registry::PeerName)) and one build Note per unresolved name. The
backend's own feature registers two rules only
([`rules`](mod@crate::stdcore::backends::latex_recomposed::rules)) and depends on no
content feature.

Several rules read the safe-labels table that the `refs` feature owns, and some of them
read it *optionally*: a registry that holds the backend but not the `refs` feature still
renders, and the labels are then written as the document wrote them ([References and safe
labels](#references-and-safe-labels)).

**The raw tree.** The recomposed LaTeX is written from the **raw tree**: the frozen tree
that the block-structure pass has not processed, with no paragraphs built, no typographic
substitution, and the author's whitespace and blank lines as parsed. The backend states
this through the requirements of its two structural rules
([`rules`](mod@crate::stdcore::backends::latex_recomposed::rules)): the rule for the
paragraph-break marker and the rule for a paragraph both require the capability
[`flm.blocks.raw`](crate::stdcore::features::blocks::RAW). The lowering stage requires
what every selected rule requires, so every plan for this backend keeps its trees raw. A
paragraph-break marker, which stands for one or more blank lines, is written as one blank
line.

A plan for this backend also keeps the use sites of the `per-plan` user definitions in the
tree (the capability alternative `flm.userdefs.kept`; [Stages, capabilities, and
plans](crate::guide::concepts_overview#stages-capabilities-and-plans)) instead of expanding them, because the rule
that the `userdefs` feature marks as the default on this backend requires it ([User
definitions in the recomposed LaTeX](#user-definitions-in-the-recomposed-latex)).

**What each element is written as:**

| written in the document | written in the recomposed LaTeX | written by |
|---|---|---|
| document text | the text, escaped: `$ & # ^ _ % { } \ ~` as commands (`^` as `\textasciicircum{}`), every other non-ASCII character according to the Unicode mode ([Escaping and the Unicode modes](#escaping-and-the-unicode-modes)) | the emitter |
| `{…}` | `{…}`, its content recomposed | the emitter |
| `%% a comment` | the comment as written: its prefix, its text, the newline that ends its line, then the whitespace that followed it in the source minus that newline; nothing at all under the render option `drop_comments` | the emitter |
| a blank line | one blank line | [`ParbreakRule`](crate::stdcore::backends::latex_recomposed::ParbreakRule) |
| `\emph{…}`, `\textbf{…}`, `\textit{…}`, `\texttt{…}` | the same command around the recomposed argument | [`textstyles::latex`](crate::stdcore::features::textstyles::latex) |
| `\%`, `\#`, `\&`, `\$`, `\ `, `\{`, `\}`, `\textbackslash`, `~`, `\@` | the spelling as written, followed by the whitespace that followed a command name | [`literals::latex`](crate::stdcore::features::literals::latex) |
| `\section*{T}\label{sec:a}` | `\flmHeading[toc=true]`, a newline, `\section*{T}{\flmLabels{\label{ref1}}}`: the heading's LaTeX hook on a line of its own with the heading's `toc` key; the LaTeX command the heading kind maps to ([`HeadingsLatexOptions`](crate::stdcore::features::headings::latex::HeadingsLatexOptions); by default the heading kind's own name), starred when the heading is not numbered (written with a star or with `numbered=false`); the title; a group holding each label under its safe label ([References and safe labels](#references-and-safe-labels)). The heading's bracket of key-values is not written back | [`headings::latex`](crate::stdcore::features::headings::latex), [`refs::latex`](crate::stdcore::features::refs::latex) |
| `\ref[S,cap=Figure]{sec:a}` | `\flmRef[S,cap={Figure}]{ref1}`: the reference hook around the safe label, the options in FLM's own vocabulary with their text escaped ([The LaTeX hooks](#the-latex-hooks) says how the hook's default translates them) | [`refs::latex`](crate::stdcore::features::refs::latex) |
| `\(…\)`, `\[…\]`, `$…$`, `\ensuremath{…}` written in text | `\(…\)` or `\[…\]` (`\ensuremath{…}` for `\ensuremath{…}`), the content written from its nodes, byte for byte except where a user definition is used | [`math::latex`](crate::stdcore::features::math::latex) |
| `\begin{align}…\end{align}` and the other display-math environments | rebuilt line by line: each line's content, its tag as `\tag*{(A)}`, its labels as one group `\flmLabels{\label{ref1}\label{ref2}}`, `\nonumber`, the line's `\\` ([Mathematics in the recomposed LaTeX](#mathematics-in-the-recomposed-latex)) | [`math::latex`](crate::stdcore::features::math::latex) |
| `\verbcode[python]{…}`, `\verbtext{…}`, `\verba{…}`, `{verbatimtext}` | `\EscVerb[formatcom=\flmFmtVRB{verbcode}]{…}`, the content in the encoding `\EscVerb` reads | [`verbatim::latex`](crate::stdcore::features::verbatim::latex) |
| `{verbatimcode}[python]` | `\begin{flmVerbatimCode}[python]`, the content on lines of its own, `\end{flmVerbatimCode}`; the brackets are always written, empty where the block gives no language | [`verbatim::latex`](crate::stdcore::features::verbatim::latex) |
| `\begin{enumerate}[(i)]\item[Tag]\label{item:a} …` | `\begin{enumerate}[label={(\roman*)}]\item[Tag]\flmLabels{\label{ref1}} …` ([Lists in the recomposed LaTeX](#lists-in-the-recomposed-latex)) | [`lists::latex`](crate::stdcore::features::lists::latex) |
| `\footnote[\label{fn:a}]{…}`, an xnote of any xnote kind | `\flmXNfootnote[{\flmLabels{\label{ref1}}}]{…}` ([Xnotes in the recomposed LaTeX](#xnotes-in-the-recomposed-latex)) | [`xnotes::latex`](crate::stdcore::features::xnotes::latex) |
| `\begin{lemma}[Key]\label{lem:a}\noproofref …`, `\begin{proof}[*lem:a]` | `\begin{lemma}[Key]\flmLabels{\label{ref1}}\noproofref …`, `\begin{proof}[*ref1]` ([Theorems in the recomposed LaTeX](#theorems-in-the-recomposed-latex)) | [`theorems::latex`](crate::stdcore::features::theorems::latex) |
| `\hello{World}`, a use site of a `per-plan` user definition | `\flmUDhello{World}`, and `\providecommand\flmUDhello[1]{Hello, #1!}` in the style file, or the expansion where no LaTeX definition can express the definition ([User definitions in the recomposed LaTeX](#user-definitions-in-the-recomposed-latex)) | [`userdefs::kept`](crate::stdcore::features::userdefs::kept) |
| `\begin{cells}…\end{cells}` | one call `\flmCellsTable{<natural>}{<fitted>}{<options>}{<rows>}` ([Tables of cells in the recomposed LaTeX](#tables-of-cells-in-the-recomposed-latex)) | [`cells::latex`](crate::stdcore::features::cells::latex) |
| `\nope`, `\begin{nowhere}…\end{nowhere}`, a macro or environment no feature declares | as written; the parse reports the error | the emitter |
| an accepted unrenderable element | the comment `% flm: unsupported element <name>` on a line of its own; one Error is reported | the emitter |
| a dropped mark (`flm.pres.dropped`), such as the stray content of a table of cells | the comment `% flm: dropped <detail>` on a line of its own, for instance `% flm: dropped content outside \cell`. The mark is a comment, so the document compiles and the mark shows in its source | the emitter |

**Syntax is normalized, content is preserved.** A construct element is written from its
payload, never from the author's spelling of the call:

- a mandatory argument is always braced, and no space separates a command name from its
  arguments: `\section  {T}` is written `\section{T}`;
- a macro written with no argument, whose name ends in a letter, is followed by one space
  where the next character written is a letter, so that the letter does not extend the
  name; LaTeX ignores that space;
- an optional argument is written `[…]` where its content is made of text without `]`, raw
  nodes without `]`, and brace groups, and `[{…}]` as soon as it holds anything else (a
  `]`, or a command whose bytes the emitter does not inspect), so that a `]` inside it
  cannot end the argument. LaTeX reads an optional argument that is one brace group as the
  group's content. For instance, a `\ref`'s options are written `[S,cap={Figure}]`, and
  `[{cap={\emph{z}}}]` once a value holds a command.

The bytes inside every argument are the document's own.

### Lists in the recomposed LaTeX

A list ([`lists::latex`](crate::stdcore::features::lists::latex)) is written as its
environment, under the list kind's own name, and each item as `\item`: an item's tag is
the optional argument of `\item`, and the item's labels follow it inside the labels hook
`\flmLabels` ([The LaTeX hooks](#the-latex-hooks)).

**A list's own label template** (its `label=` key, or its short form such as `[(i)]`) is
written as the `label=` key of the LaTeX package `enumitem`, which the rule then requests.
The template's content is recomposed as any content of the document is, through the
ordinary rules, and each number format in it is written as the `enumitem` command for that
format, wherever it occurs in the content: `#{arabic}`, `#{alph}`, `#{Alph}`, `#{roman}`
and `#{Roman}` become `\arabic*`, `\alph*`, `\Alph*`, `\roman*` and `\Roman*`. The value is
written as one brace group. For instance:

| written | recomposed |
|---|---|
| `[(i)]` | `[label={(\roman*)}]` |
| `[-]` | `[label={-}]` |
| `label={Step~#{arabic}:}` | `[label={Step~\arabic*:}]` |
| `label={\textbf{#{arabic}}.}` | `[label={\textbf{\arabic*}.}]` |
| `[1.:]` | `[label={\arabic*.:}, ref={\arabic*}]` |
| `label=(#{alph})` | `[label={(\alph*)}]` |

**A numbered list's own reference template** (its `ref=` key, or the one its short form
gives) is written in the same way as the `ref=` key of `enumitem`, beside `label=` where
the two templates differ. Where they are the same, `label=` alone is written.

Two templates are reported and their key is not written:

- a template that writes a number format for which `enumitem` has no command, such as
  `#{fnsymbol}` (`flm.lists.no-enumitem-format`);
- a template that writes a number format inside verbatim content, such as
  `label={\verbtext|x#{arabic}|}` (`flm.lists.number-read-as-characters`): the characters
  of verbatim content are read when the LaTeX is written, while the `enumitem` command is
  LaTeX run later, as each label is set.

**The labels that a list kind declares for each depth are not written.** `itemize` and
`enumerate` are LaTeX's own environments, and LaTeX sets their labels, as it makes every
other choice of presentation in this output: for `enumerate`, LaTeX writes `1.`, `(a)`,
`i.` and `A.` at the four depths, where the list kind's default in FLM writes `1.`, `(i)`,
`a-`. A list environment that a host configures is written under its own name, and the
author's LaTeX must define it, for instance with `enumitem`'s `\newlist` in the text that
the render option `extra_preamble` adds to the style file.

### Xnotes in the recomposed LaTeX

An xnote is a note set apart from the running text, marked where it belongs and
shown elsewhere or in place, such as a footnote ([`xnotes`](crate::stdcore::features::xnotes)).
On this backend, an xnote is written as the command of its xnote kind, `\flmXN<kind>`,
which the style file defines and LaTeX sets where the command is written
([`xnotes::latex`](crate::stdcore::features::xnotes::latex)):

```latex
The claim holds in every case.\flmXNfootnote{Except the empty one, which we exclude by convention.} The next sentence goes on.

Two paragraphs.\flmXNfootnote{First paragraph.

Second paragraph.} And on.

A labeled note.\flmXNfootnote[{\flmLabels{\label{ref1}}}]{See the appendix.} See \flmRef{ref1}.
```

The letters `XN` stand for the `xnotes` feature, so the command of the xnote kind
`endnote` is `\flmXNendnote`. The optional argument is written only where the xnote has
labels: the labels inside the labels hook, as a heading's labels are written (bare `\label`
commands under the render option `label_hooks` off). The content is recomposed like any
other content, and a blank line in it stays the note's paragraph break.

**The definition in the style file.** Each xnote kind that the document uses gets one
definition piece, `flm.latex.def.xnotes.<kind>`:

```latex
% \flmXNfootnote[<labels>]{<note>}: a note of the kind `footnote' in the FLM document;
% by default it passes the note, its labels first, to \footnote.
% Redefine it to set these notes another way.
\ProvideDocumentCommand\flmXNfootnote{+O{} +m}{\footnote{#1#2}}
```

The default hands the note to the LaTeX command of the xnote kind
([`XnoteKind::latex_command`](crate::stdcore::features::xnotes::XnoteKind::latex_command),
`footnote` by default), with the labels first inside the command's argument, where LaTeX
makes a label refer to the footnote: a reference to it prints "footnote 1". The definition
is guarded with `\ProvideDocumentCommand`, so a definition of the command made before the
style file is loaded is the one the document uses; after loading the style file,
`\RenewDocumentCommand` replaces it. The command is robust, so an xnote in a heading's title
reaches the table of contents as the command itself. The definition piece requests no LaTeX package: an
xnote kind handed to a command that a package defines (an `endnote` kind handed to
`\endnote`) needs that package in the author's own preamble, or in the render options
`extra_packages` and `extra_preamble`.

**Placement and lists.** The command is the same for every placement of the xnote kind:
an xnote kind that the other outputs show in place is also handed to `\footnote` by default, and
setting it differently is done by redefining its command. No list of xnotes is written:
LaTeX sets and numbers each note itself.

**Numbers.** The numbers LaTeX prints agree with the numbers FLM computes for an xnote kind
whose LaTeX command counts its notes as FLM does, such as `footnote` handed to `\footnote`,
both counting in document order. Two cases differ:

- a second xnote kind left on the default command `footnote` shares LaTeX's footnote
  counter, so its numbers continue from the footnotes' numbers, until that xnote kind is
  handed to a command with a counter of its own;
- an xnote in a list's own label template is set by LaTeX beside every item's label, and
  again inside every `\ref` to an item that prints the label template (every `\ref` to an
  item of a list without its own reference template, since `enumitem`'s `ref=` defaults to
  `label=`). The other outputs write no xnote mark there and report the missing table entry
  (`flm.run.missing-entry`) at each item, so every later note of that counter has a higher
  number in the PDF than the number FLM computes for it.

**An xnote inside another xnote** is written as a command inside the other's argument, its
labels included, and LaTeX numbers it as FLM does, after the note that holds it. LaTeX's own
`\footnote` prints the inner note's mark but does not set its text, under `pdflatex` and
`lualatex` alike, so that text is missing from the PDF and a reference to a label of the
inner note is undefined. No short definition of `\flmXNfootnote` built from LaTeX's footnote
commands (`\footnotemark`, `\footnotetext`) sets nested notes; an author whose document
needs them gives `\flmXNfootnote` a definition of their own before loading the style file.

**An xnote in content that is not the document's own tree** is written as its command too:

- the template of a user definition written as a LaTeX definition holds the command, as in
  `\providecommand\flmUDnoted{a definition's text\flmXNfootnote{…}}` for a definition
  `\noted` of `a definition's text\footnote{…}`, so LaTeX sets the note at each call, as the
  other outputs do;
- the display text of `\flmLabelsWithDisplayText` (the term of a defined term, the caption
  of an unnumbered float) is written as the author wrote it, a note included, as the heading
  hook reuses a heading's title; LaTeX prints the display text at every reference to the
  labels, so it sets the note again there.

**A labeled xnote in a user definition's template.** A label in the template of a kept
definition (a user definition written as a LaTeX definition) is refused: the template is
lowered as a synthesized value, and the value's catch-up refuses a label, because a label is
a fact about the document, which a synthesized value may not state ([value
catch-up](crate::guide::concepts_overview#value-catch-up)). A labeled xnote there, like a labeled
heading there, makes the definition refused with the Error
[`flm.userdefs.kept-template-declares-point`](crate::stdcore::features::userdefs::kept::KeptTemplateDeclaresPoint),
reported at each call of the definition. Every call is then written as a comment, and a
reference to the label is unresolved. The remedy is to mark the definition to be expanded
in every plan (the expansion policy `always`): its use sites are then expanded in the
document, and each label is the document's own, written once per use site.

### Hyperlinks in the recomposed LaTeX

The three hyperlink commands of the [`href`](crate::stdcore::features::href) feature are
written as three LaTeX hooks of their own
([`href::latex`](crate::stdcore::features::href::latex)):

| written | recomposed | what the default definition does |
|---|---|---|
| `\href{https://example.com}{Example link}` | `\flmHref{https://example.com}{Example link}` | `\href{#1}{#2}` |
| `\url{https://example.com/}` | `\flmUrl{https://example.com/}` | `\href{#1}{\nolinkurl{<display>}}`: the display derived in LaTeX from the address (`example.com`), typeset as `\url` typesets an address |
| `\email{a@b.com}` | `\flmEmail{a@b.com}` | `\href{mailto:#1}{\nolinkurl{<display>}}`: the display of `mailto:a@b.com`, which is the address |

The style file defines the three hooks with `\ProvideDocumentCommand`, so a definition made
first in the author's class or preamble is the one used, and `\RenewDocumentCommand` after
loading the style file replaces one. The defaults are macros of the `hyperref` package, so
the style file requests `hyperref`: a document that holds a hyperlink loads it, and every
reference that shows the author's own words (`\flmHyperref`, [The LaTeX hooks](#the-latex-hooks)) is
then a link too. A document that loads `hyperref` itself with options, after the style file
(`\usepackage[colorlinks]{hyperref}`), stops with LaTeX's "Option clash for package
hyperref" unless the render option `skip_packages` lists `hyperref`, which removes the
style file's request.

Every argument is an ordinary LaTeX argument, so each hook works in a moving argument (a
heading's title, which LaTeX writes to the table of contents and reads back) and
contributes only its text or its display to a PDF bookmark. In an address, `#`, `%` and `\`
are written `\#`, `\%` and `\\` ([`escape_url`](crate::stdcore::backends::latex::escape_url)),
which `\href` reads back as those characters; every other character is written unchanged.
The text of `\flmHref` is content, written as all content is.

**The display of `\flmUrl` and `\flmEmail`.** What a `\url` or an `\email` shows is
presentation, so these two hooks take the address alone and derive the display in LaTeX,
as the HTML output does: the display is the link's target without one leading `http://`,
`https://` or `mailto:`, and without its trailing run of `/`, `#` and `?`. So
`\flmUrl{https://example.com/?\#}` shows `example.com` and `\flmEmail{a@b.c/}` shows
`a@b.c`, as the text and HTML outputs do. The derivation is an internal helper of the style
file; to show something else, redefine the hook, which receives the address.

An address is read as characters, so a user definition whose template puts a placeholder
into an address (`\href{https://doi.org/#1}{#1}`, `\url{#1}`) has each use site expanded,
and the address holds the use site's characters ([User definitions in the recomposed
LaTeX](#user-definitions-in-the-recomposed-latex)). A use site that passes something other
than characters there (`\doi{\emph{x}}`) is reported, as on every output.

### Defined terms in the recomposed LaTeX

A term definition (the `defterm` environment of the
[`defterm`](crate::stdcore::features::defterm) feature) is written as the environment the
author wrote, with the term, written as content, as its one argument
([`defterm::latex`](crate::stdcore::features::defterm::latex)). The body opens with
`\phantomsection`, `\flmLabelsWithDisplayText{<term>}{<labels>}` and `\ignorespaces`,
followed by the definition's paragraphs:

```latex
\begin{defterm}{Entropy}\phantomsection\flmLabelsWithDisplayText{Entropy}{\flmLabels{\label{ref1}\label{ref2}\label{ref3}}}\ignorespaces
A measure of disorder.
\end{defterm}
```

- `\phantomsection` is the `hyperref` package's way of placing an anchor where no
  sectioning command places one, so that a link to the definition leads to it.
- `\flmLabelsWithDisplayText` ([The LaTeX hooks](#the-latex-hooks)) makes the labels refer to that
  place, with the term as what a reference prints: `\ref{topic:entropy}`, written
  `\flmRef{ref3}`, prints "Entropy". The term is written there as the author wrote it, so a
  footnote in it is part of the display text and LaTeX sets the note again at every
  reference to the definition ([Xnotes in the recomposed
  LaTeX](#xnotes-in-the-recomposed-latex)).
- The labels are every label of the definition, each under its safe label ([References and
  safe labels](#references-and-safe-labels)): the label a term link finds the definition
  by, one label per `\aliasterm`, then the labels the author wrote.
- `\ignorespaces` drops the line break after the labels, so that the definition's text
  follows the lead-in with only the lead-in's own space.

With the render option `label_hooks` off, the labels are written as bare `\label` commands
after `\phantomsection`, with no display text, and a reference to the definition prints
what LaTeX recorded last.

**The environment's definition.** The style file defines the `defterm` environment from
the configured lead-in
([`DeftermConfig::lead_in`](crate::stdcore::features::defterm::DeftermConfig::lead_in)):
its begin code is the lead-in template, written through the ordinary rules, with
`#{term}` written as the environment's parameter `#1`; its end code is empty. The lead-in a
compiled document shows therefore follows the configuration:

| lead-in | the environment's definition |
|---|---|
| `#{term}: ` (the default) | `\ProvideDocumentEnvironment{defterm}{+m}{#1: }{}` |
| `\textbf{Definition} (\emph{#{term}}): ` | `\ProvideDocumentEnvironment{defterm}{+m}{\textbf{Definition} (\emph{#1}): }{}` |
| none | `\ProvideDocumentEnvironment{defterm}{+m}{}{}` |

The definition is guarded, so a definition of `defterm` made in the class or preamble
before the style file is loaded is the one used, and `\RenewDocumentEnvironment` after
loading replaces it. The style file writes it in a block of its own, the definitions that a
feature writes from its configuration, after the definition pieces and before the
document's own definitions. The feature's definition piece requests what the body uses:
the `hyperref` package, which defines `\phantomsection` (so a document holding a
definition loads `hyperref`, and a document that loads it itself with options lists it in
`skip_packages`), and the labels hooks with the `zref-clever` reference type that makes a
reference to such a label print its text alone.

**Term links.** A term link is written as one of two LaTeX hooks that the feature's definition piece
defines with `\ProvideDocumentCommand`; the text of the link is written as content:

| written | recomposed | what the default definition does |
|---|---|---|
| `\term{Entropy}`, `\term[Entropy]{the entropy}`, anywhere except inside the definition of `Entropy` | `\flmTerm{ref1}{Entropy}`, `\flmTerm{ref1}{the entropy}` | `\flmHyperref{#1}{#2}`: a link to the definition, where the document loads `hyperref` |
| `\term{Entropy}` inside the definition of `Entropy` | `\flmTermInDefterm{Entropy}` | `\emph{#1}`: the term being defined, set apart, with no link |

The first argument of `\flmTerm` is the safe label of the label the link refers to. Which
of the two hooks is written is decided when the document is lowered, from the document's
structure (whether a definition of the term encloses the link), as on every output; LaTeX
keeps no state of its own for it. A term that no definition carries is given a safe label
as any reference to nothing is (`\flmTerm{ref1}{Nothing}`), regardless of what the term
holds, so the document compiles, LaTeX reports an undefined reference, and FLM reports the
unresolved term. Each hook is robust, so it works in a moving argument such as a heading's
title, and contributes only its text to a PDF bookmark. Redefine either hook, before or
after loading the style file, to set term links differently.

A user definition whose template holds a term link with a placeholder in its term
(`\term[#1]{this term}`, `\term{#1}`, `\term[\emph{#1} chain]{it}`) is not written as a
LaTeX definition: the term, and so the definition the link finds, is known only when a call
supplies it, and a LaTeX definition could not write the link under the safe label of that
definition. Each use site is expanded instead, and its link is written under the
definition's safe label, with nothing reported. A term link with a fixed term in a
template is written under its safe label, and the template is kept.

### Theorems in the recomposed LaTeX

A theorem-like environment of the [`theorems`](crate::stdcore::features::theorems) feature
is written as the author wrote it ([`theorems::latex`](crate::stdcore::features::theorems::latex)):
the environment of its theorem kind's name, the title as the optional argument (lowered as
the content it is), the labels inside the labels hook right after the begin line, and
`\noproofref` where the author wrote it:

```latex
\begin{theorem}[My Theorem]\flmLabels{\label{ref1}}
The square root of two is irrational.
\end{theorem}
```

The heading (the theorem kind's name, its number, the title) and a proof's closing mark are
LaTeX's: the environment's declaration sets them. `\noproofref` is written wherever the
author wrote it, whichever form the render option `resolved_proof_refs` selects, because
LaTeX can use it: a statement can print where its proof is ("proof on page 12"),
and needs to know which statements have none. The feature's definition piece defines
`\noproofref` as a command that does nothing, with `\ProvideDocumentCommand`, where no
package defined it first.

**A proof's title** (the title of a theorem kind whose theorem class reads back-references)
is written inside the proof title hook: `\begin{proof}[{\flmProofTitle{Sketch}}]`. The
default of `\flmProofTitle` depends on the form of back-references, below: "Proof
(Sketch)" over `amsthm`, as FLM's other outputs set it, and "Proof of Sketch" over `phfthm`.

**A proof's back-reference** is `[*lem:a]`, which shows the statement in the proof's
heading, or `[**lem:a]`, which records the relation and shows nothing. It is written in one
of two forms, which the render option
[`resolved_proof_refs`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::resolved_proof_refs)
chooses:

| written | `resolved_proof_refs` off (the default) | on |
|---|---|---|
| `\begin{proof}[*lem:a]` | `\begin{proof}[*ref1]` | `\begin{proof}[{\flmProofTitle{\flmRef{ref1}}}]` |
| `\begin{proof}[**lem:a]` | `\begin{proof}[**ref1]` | `\begin{proof}` |
| the LaTeX package the style file requests | `phfthm` | `amsthm` |

- **Off**, the back-reference is written as the author wrote it, under the statement's safe
  label, and the style file requests the `phfthm` package, which reads that syntax over
  `amsthm`: `[*ref1]` sets the heading "Proof of Theorem 1" (referring to the statement through
  `hyperref`'s `\autoref` where the document loads `hyperref`, through `\ref` where it does
  not), and `[**ref1]` sets "Proof". The `phfthm` package is not part of every TeX
  installation; a document compiled without it lists it in `skip_packages` and defines the
  environments itself. The default of the proof title hook is the title alone, `#1`, since
  `phfthm` reads an optional argument that is not a back-reference as the thing the proof
  proves and sets it after its own "Proof of": "Proof of Sketch".
- **On**, the recomposed LaTeX needs `amsthm` alone. A back-reference that the proof shows
  is written as a title is, inside the proof title hook, the title being the reference hook
  for the statement: "Proof (theorem 1)" by default, `\flmRef` printing what `\zcref`
  prints. A back-reference that is recorded and not shown writes no optional argument, and
  the proof's heading is LaTeX's own, "Proof". The default of the proof title hook is
  `\proofname\space (#1)`, since `amsthm` sets the optional argument as the whole heading:
  "Proof (Sketch)". The heading template of the theorem class is not used on this output:
  the heading is LaTeX's, and the proof title hook is the only place a title joins it.

**Declarations.** Each theorem kind that the document uses is declared in the style file,
in the block of definitions a feature writes from its configuration. Its display name is
the theorem kind's word, lowered as content. Every declaration is guarded on the name it
declares (`\ifcsname lemma\endcsname\else … \fi`), so a theorem kind that the document
class, a package or the author's preamble defines before the style file is loaded keeps
that definition; to set up a theorem kind differently, define it first.

| the theorem kind | off | on |
|---|---|---|
| the theorem class of a numbered theorem kind, before its first theorem kind | `\newtheoremstyle{theoremlike}{}{}{\itshape}{}{\bfseries}{.}{5pt plus 1pt minus 1pt}{}` | the same |
| numbered, in the shared counter | `\phfMakeTheorem[thmstyle=theoremlike,counter=phfthmcounter]{lemma}{Lemma}` | `\begingroup\theoremstyle{theoremlike}\newtheorem{lemma}[flmtheorem]{Lemma}\endgroup` |
| numbered, in a counter of its own, expecting no proof | `\phfMakeTheorem[thmstyle=exerciselike,proofref=false]{exercise}{Exercise}` | `\begingroup\theoremstyle{exerciselike}\newtheorem{exercise}{Exercise}\endgroup` |
| unnumbered | `\phfMakeProofEnv[defaultproofname={Proof}]{proof}` | an environment over `amsthm`'s `proof`, its default title the word |

- A numbered theorem kind is set in the theorem style named after its theorem class, which
  the style file declares, guarded on `\th@<class>` (the control sequence `amsthm` creates
  for a theorem style), with the parameters of `amsthm`'s `plain` style: a bold heading
  and an italic body. The feature's definition piece, written before those declarations,
  first declares the shipped theorem class `definitionlike` in `amsthm`'s `definition`
  style (an upright body). To set a theorem class's kinds differently, declare a theorem
  style of the class's name before loading the style file.
- A theorem kind that expects no proof is declared with `proofref=false` for `phfthm`.
- The style file requests `phfthm` with the option `thmset=empty`
  (`\RequirePackage[thmset=empty]{phfthm}`), so that the package declares no theorem kind
  of its own and the configuration is the only source of the declared kinds; the package
  still defines `proof` and its counter `phfthmcounter`, in which the theorem kinds that
  share a counter are declared. An author who loads `phfthm` before the style file writes
  `\usepackage[thmset=empty]{phfthm}`.
- In the resolved form, the shared counter is a LaTeX counter of its own, `flmtheorem`,
  declared once, guarded on its register
  (`\ifcsname c@flmtheorem\endcsname\else\newcounter{flmtheorem}\fi`), and every theorem
  kind counting in it is declared `[flmtheorem]`, each `\theoremstyle` and `\newtheorem`
  inside a group so that the theorem style it sets does not apply after the style file. A
  theorem kind with a counter of its own counts in the counter of its own name.
- A theorem kind that the author defines before the style file counts where that
  definition says: an author's own `\newtheorem{theorem}{Theorem}` numbers theorems apart
  from the theorem kinds the style file declares, in both forms ("Theorem 1", then "Lemma
  1").
- The declarations do not number the theorem kinds within sections: LaTeX numbers each
  counter in one sequence, regardless of the configured numbering.

**Reference types.** Each numbered theorem kind's declaration also makes the kind a
reference type of its own for `zref-clever`, the LaTeX package with which the reference
hook sets references, with the theorem kind's words:
`\zcRefTypeSetup{lemma}{Name-sg={Lemma},name-sg={lemma},Name-pl={Lemmas},name-pl={lemmas},Name-sg-ab={Lem.},…}`.
The words map as for every reference type of the family
([`reference_type_words`](crate::stdcore::features::refs::reference_type_words)):

- the capitalized keys take the kind's `capital` variant where it declares one, and its
  base words otherwise;
- the lowercase keys take its `lowercase` variant where it declares one, and its base
  words otherwise;
- the abbreviated keys, capitalized and lowercase alike, take its `abbreviated` variant,
  and are written only where the theorem kind declares that variant.

The declaration also maps the counter that the kind's environment steps to that reference
type, inside the environment (on:
`\AddToHook{env/lemma/begin}{\zcsetup{countertype={flmtheorem=lemma}}}`; off:
`countertype={lemma=lemma}`), so that a reference to a lemma counting in the shared counter
reads "lemma 2", and a reference to a theorem kind with a counter of its own reads its number. A
theorem kind defined before the style file keeps the reference type its own definition
gives it.

### Quotations in the recomposed LaTeX

A quotation ([`quote::latex`](crate::stdcore::features::quote::latex)) is written as the
environment the author wrote. Inside it, the body is written as the author wrote it (the
whitespace, blank lines and comments between the quotation sections unchanged), and each
quotation section through the LaTeX hook of its kind:

| written | recomposed |
|---|---|
| `\begin{quote}\text{To be.}\attributed{Shakespeare}\end{quote}` | `\begin{quote}\flmQuoteText{To be.}\flmQuoteAttributed{Shakespeare}\end{quote}` |
| `\lines{First\\ \indent Second}` | `\flmQuoteLines{First\\\relax`, then on the next line `\flmIndent{1\flmTabWidth}Second}` |
| `\block{One.` ⏎⏎ `Two.}` | `\flmQuoteBlock{One.` ⏎⏎ `Two.}` |
| `\begin{blockquote}One.` ⏎⏎ `Two.\end{blockquote}` | `\begin{blockquote}\flmQuoteBlock{One.` ⏎⏎ `Two.}\end{blockquote}` |
| `\begin{address}123 Main St\\ City\end{address}` | `\begin{address}\flmQuoteLines{123 Main St\\\relax`, then `City}\end{address}` |

**A lines section** is split at every `\\`. The whitespace at both ends of each line is
trimmed, and the lines are joined by `\\\relax` and a line break. The `\relax` ends the
`\\` command there, so that a line beginning with `[` or `*` is printed as written instead
of being read as an argument or a star of `\\`. A line's indentation (its leading
`\indent` commands, a [length](crate::stdcore::backends::pres::Length) of as many tabs) is
written `\flmIndent{<length>}` before the line's content. The family writes every length
the same way
([`stage_length`](crate::stdcore::backends::latex::preamble::stage_length)): a length in
tabs as a multiple of the LaTeX length `\flmTabWidth` (which also requires the definition
piece that declares `\flmTabWidth`), a point as `bp` (FLM's point is 1/72 inch, which
LaTeX calls a big point), and an em, a millimeter, a centimeter and an inch unchanged.

**Content outside the quotation section commands** (a `blockquote`'s paragraphs, an
`address`'s lines) is written through the hook of the environment's bare-content kind,
with the whitespace at its edges outside the hook. Where the environment accepts no bare
content, the content is reported (`flm.quote.bare-content`) and written in place. A
quotation section written inside another (`flm.quote.nested-section`) and an `\indent` in
the middle of a line (`flm.quote.indent-mid-line`) are reported as on every other backend.

**The definitions in the style file.** The feature's definition piece defines the four
hooks and `\flmIndent` with `\ProvideDocumentCommand`, so that a definition made before
the style file is loaded is the one used. The family's definition piece declares the length
`\flmTabWidth`, `2em`, guarded on its name, wherever a line is indented in tabs; change it
with `\setlength`. `quote` is LaTeX's own environment; `blockquote` and `address`, which
LaTeX does not define, are defined by the feature's definition piece with
`\ProvideDocumentEnvironment`, so that an environment of that name defined first is the
one used. An environment of any other name that a host configures is written under its
name, and the author's LaTeX must define it (`\newenvironment{myquote}{…}{…}` before the
style file).

### Annotations in the recomposed LaTeX

A reviewer's annotations ([`annotations`](crate::stdcore::features::annotations)) are
written as the author wrote them, over the LaTeX package `phfcc`, which defines a
reviewer's commands with the same three forms
([`annotations::latex`](crate::stdcore::features::annotations::latex)):

| written | recomposed |
|---|---|
| `\phf{a passage}` | `\phf{a passage}` |
| `\phf a passage\endphf` | `\phf a passage\endphf` |
| `\phf[a remark]` | `\phf[a remark]` |

The command, the opening delimiter and the closing delimiter are the ones the author
wrote, with the whitespace around them, and the content between them is lowered through
the ordinary rules. A passage left open is written with no closing command, as the author
wrote it.

**One invocation across paragraph breaks.** This backend writes the document before
paragraphs are built, so an annotation written across a paragraph break, or around a list,
is not cut into parts: it is one invocation, with the paragraph break inside it. A
reviewer's comment holding a blank line is therefore written back byte for byte, and
`phfcc` colors the whole passage:

```latex
Some text \phf[A comment.

With multiple paragraphs.] More text
```

**The commands, made by `phfcc`.** The style file requests `phfcc` with no option and
makes one command per configured annotator, in configuration order, which is the order in
which the package assigns its colors, so that annotator 0 takes the package's first color
as it takes the HTML output's first color. Each command is guarded on its name:

```latex
\RequirePackage{phfcc}
\ifcsname phf\endcsname\else\phfMakeCommentingCommand[initials={PhF}]{phf}\fi
\ifcsname abc\endcsname\else\phfMakeCommentingCommand[formatinitials=hide]{abc}\fi
```

An annotator with no initials is made with `formatinitials=hide`, because the package
otherwise shows the command's name beside the annotation. A command of that name made in
the author's preamble before the style file is loaded is the one used: that is where an
author makes their own `\phfMakeCommentingCommand` with keys of their own. Every configured
annotator's command is made, whether the document uses it or not. `phfcc` sets the initials
in the margin itself, so the margin decoration that the HTML output shows is not written here. The
package is not part of every TeX installation.

Under the render option
[`hidden`](crate::stdcore::features::annotations::AnnotationsRenderOptions::hidden),
nothing of the package is written: a highlight is written as its passage, and a reviewer's
comment as nothing but the paragraph breaks it held, so that the text around a comment
written across a paragraph break stays two paragraphs. The package's own
`\phfDisableCommentingCommands` is different: it makes every use of a commenting command an
error, for a final version that must hold none.

### Images in the recomposed LaTeX

An image ([`graphics`](crate::stdcore::features::graphics)) is written as the LaTeX hook
`\flmIncludeGraphics[<keys>]{<path>}`
([`graphics::latex`](crate::stdcore::features::graphics::latex)):

| written | recomposed |
|---|---|
| `\includegraphics{figures/plot.png}` | `\flmIncludeGraphics{figures/plot.png}` |
| `\includegraphics[width=5cm]{figures/plot.png}` | `\flmIncludeGraphics[width=5cm]{figures/plot.png}` |
| `\includegraphics[width=10pt]{plot.png}` | `\flmIncludeGraphics[width=10bp]{plot.png}` |

- The keys are `width=` and `height=` where the image is shown at a set size. The size is
  decided as on every output: the size the options give; one dimension scaled from the size
  the host's image inspector reported, where the options give the other alone; otherwise
  the inspected size; each dimension multiplied by the output's magnification ([images
  and how an output reaches them](#images-and-how-an-output-reaches-them)). Each length is written as the family writes lengths, a point as
  LaTeX's big point `bp`. The bracket is left out where there is no key.
- The path is the address held in the backend's table of published images, which the
  output's strategy decides: the file where it is by default, a copy beside the document
  under the strategy `copy` ([images and how an output reaches
  them](#images-and-how-an-output-reaches-them)). `#`, `%` and `\` in it are preceded by a
  backslash, as in every address.
- The alternative text is not written, because `\includegraphics` has no place for it.

A user definition whose template holds a placeholder in a path is not written as a LaTeX
definition: its use sites are expanded.

The style file defines the hook with `\ProvideDocumentCommand` as
`\includegraphics[max width=\linewidth,#1]{#2}`, and requests `graphicx`, and `adjustbox`
with its option `export`, which makes `max width` a key of `\includegraphics`: an image is
never wider than the line. Define `\flmIncludeGraphics` before loading the style file, or
redefine it after, to show images differently.

### Floats in the recomposed LaTeX

A float ([`floats::latex`](crate::stdcore::features::floats::latex)) is written as the
LaTeX hook environment `flmFloat`, over the float type and the float's **case**: whether it
is numbered (under its float type's policy and its fields) and whether it has a caption.
The four cases are `NumCap`, `NumOnly`, `CapOnly` and `Bare`:

| written | recomposed |
|---|---|
| `\begin{figure}\includegraphics{a.png}\caption{A photo}\label{figure:a}\end{figure}` | `\begin{flmFloat}{figure}{NumCap}\flmIncludeGraphics{a.png}\caption{A photo}\flmLabels{\label{ref1}}\end{flmFloat}` |
| `\begin{figure}\includegraphics{b.png}\label{figure:b}\end{figure}` | `\begin{flmFloat}{figure}{NumOnly}\flmIncludeGraphics{b.png}\caption{}\flmLabels{\label{ref2}}\end{flmFloat}` |
| `\begin{figure}\nonumber\includegraphics{c.png}\caption[alt=Photo C]{A photo}\label{figure:c}\end{figure}` | `\begin{flmFloat}{figure}{CapOnly}\flmIncludeGraphics{c.png}\caption{A photo}\phantomsection\flmLabelsWithDisplayText{Photo C}{\flmLabels{\label{ref3}}}\end{flmFloat}` |
| `\begin{figure}\includegraphics{d.png}\end{figure}` | `\begin{flmFloat}{figure}{Bare}\flmIncludeGraphics{d.png}\end{flmFloat}` |

The float's content is recomposed through the ordinary rules, followed by the caption (its
full content), followed by the labels:

- A numbered float's labels follow its caption inside the labels hook, so that they refer
  to its number. A numbered float without a caption gets an empty `\caption{}`, so that
  LaTeX shows its number.
- A captioned float without a number has no number to refer to, so its labels follow
  `\phantomsection` inside the display-text hook. The display text is the caption's `alt`
  where the author wrote one, and the caption's content otherwise, as the author wrote it,
  so that a reference to the float prints the caption's words as FLM's other outputs do. A
  footnote in that text is part of it, and LaTeX sets the footnote again at every reference
  to the float; to keep it out, write the caption's alternative text without it
  (`\caption[alt=A photo]{A photo\footnote{…}}`). For such a float with labels, the
  style file requests `hyperref`, which defines `\phantomsection`; a document that loads
  `hyperref` itself with options lists it in `skip_packages`.
- A float with neither number nor caption writes its labels in the plain labels hook.

`\nonumber` is never written: the case carries it. The case follows the float type's
policy, regardless of the numbers the run's counters hold, since LaTeX numbers the float
itself. A footnote inside a float's caption shows its mark and not its text in the PDF, and
a reference to a label on it is undefined: that is how LaTeX sets a note in a caption.

**The definitions in the style file.** The style file defines the environment with
`\ProvideDocumentEnvironment{flmFloat}{m m}`. The environment asks
`\flmFloatSetUseConfig<case>{<type>}{<placement>}` how to set a float of the case,
`<placement>` being `\flmFloatPlacementArgs<case>` (`[tbph]` for the numbered cases, `[h]`
for the others), and centers the content with `\centering`. By default:

- a numbered float is set in the environment of its float type's name, at its placement;
- an unnumbered float is set where it is written, in a `center` environment in which
  `\caption` works as in a float of the float type (`\@captype`);
- a captioned float without a number has its caption set without a number, as `\caption*`
  of the `caption` package, which the style file requests.

No default definition reads the placement of the unnumbered cases, `[h]`, because those
floats are not set in a float environment; the placement exists for a redefinition that
sets one. Redefine any of the eight commands, or the environment, before or after loading
the style file.

A numbered float inside content that LaTeX sets in a box (a footnote's text, a cell of a
table) stops LaTeX with "Not in outer par mode", because a float environment cannot be
placed there. That is LaTeX's limit, left to the author. The remedy is to redefine the
numbered cases, `\flmFloatSetUseConfigNumCap` and `\flmFloatSetUseConfigNumOnly`, to a
set-up that does not float, such as a `minipage` of the line's width holding `\centering`
and the `caption` package's `\captionof`; a `center` environment cannot be placed in a cell
of a table either.

**Reference types.** Each float type that the document uses declares its words for
`zref-clever` once, guarded:
`\zcRefTypeSetup{figure}{Name-sg={Figure},name-sg={Fig.},Name-pl={Figures},name-pl={Figs.}}`.
The float type's words are mapped as a theorem kind's are ([Theorems in the recomposed
LaTeX](#theorems-in-the-recomposed-latex)), each word without the tie (`~`) that joins it to
a number on FLM's outputs, so that `\flmRef` prints "Fig. 1" in the default wording and
"Figure 1" under `S` or `capital`, as FLM's other outputs do. Call `\zcRefTypeSetup` after
loading the style file to use other words.

`figure` and `table` are LaTeX's own environments. The environment of a float type that a
host configures (`diagram`) is written under its name, and the author's LaTeX must define
it: a float environment whose counter has the float type's name, for instance
`\DeclareFloatingEnvironment{diagram}` of the `newfloat` package, before the style file.

### Tables of cells in the recomposed LaTeX

A table of cells ([`cells::latex`](crate::stdcore::features::cells::latex)) is written as
one call of the LaTeX hook `\flmCellsTable`, read from the table's model by the feature's
reader, [`read_grid`](crate::stdcore::features::cells::read_grid), which every backend uses,
over the LaTeX package `tabularray`:

| written | recomposed |
|---|---|
| `\begin{cells}\celldata<H>{One & Two}\celldata{A & B}\end{cells}` | `\flmCellsTable{cc}{X[-1]X[-1]}{hspan=minimal, cell{1}{1}={}{m, font={\flmCellsHeaderFont}}, cell{1}{2}={}{m, font={\flmCellsHeaderFont}}, hline{2}={1}{.4pt,solid}, hline{2}={2}{.4pt,solid}}{\toprule One&Two\\ A&B\\ \bottomrule}` (each option and row on a line of its own) |
| `\cell<r green>[\merge{1-2};1]{X}` | the option `cell{1}{1}={r=2}{r}` and the cell's text `\flmCellsSet{green}X` |
| `\cell<lvert rule{below}>{X}` in row 1, column 2 of a table of three rows | the options `vline{2}={1}{.4pt,solid}` and `hline{2}={2}{.4pt,solid}` |

The hook's four arguments are:

1. the table's **natural** column specification, `c` for each column;
2. its **fitted** column specification, `X[-1]` for each column, whose columns share a given
   width and break their lines;
3. its other `tblr` options: `hspan=minimal`; then one
   `cell{<row>}{<column>}={<span>}{<keys>}` option for each cell that spans rows or columns
   (`r=2`, `c=3`) or states an alignment or a header (`l`, `c` or `r`, otherwise `m`;
   `font={\flmCellsHeaderFont}` for a header); then the drawn lines (LaTeX's rules): one `hline` under a cell
   that heads its column (except in the table's last row, which `\bottomrule` closes), and
   one at each edge where a cell style draws a rule (`hline` above and below, `vline` left
   and right). Rows and columns are counted from 1;
4. its rows, between `\toprule` and `\bottomrule`, the cells separated by `&` and each row
   ended by `\\`; a position left empty or covered by a spanning cell is written empty.

A cell's text is one `\flmCellsSet{<name>}` for each custom cell style it carries, in the
order of its style tokens, then its content recomposed through the ordinary rules: a
`\cell`'s blank lines are kept, and a `\celldata` entry is written without the whitespace
around it. The dropped marks for what the table's reader left out are written before the
call, each as a comment line (`% flm: dropped cell overlapping another`). A command of
tables of cells used outside its place is reported, and its content shown where it is
written or a dropped mark written there.

**The definitions in the style file.** The definition piece `flm.latex.def.cells` defines
the hook with `\ProvideDocumentCommand`. The hook sets the table in `tblr` at its natural
width; where that is wider than `\flmCellsMaxWidth` (`0.96\linewidth`), it sets the table
again at that width with the fitted columns. It sets the table between
`\flmCellsBeginCenter` and `\flmCellsEndCenter` (a `center` environment), except where
LaTeX sets a single line, such as a cell of another table set at its natural width, where
no paragraph can begin. A header cell is set in `\flmCellsHeaderFont` (`\bfseries`). The
definition piece requests `tabularray`, whose `booktabs` and `counter` libraries it loads, and
`xcolor`. Define `\flmCellsTable` before loading the style file, or redefine it after, to
set tables differently; each of the four other commands changes one thing.

**Custom cell styles.** A custom cell style reaches LaTeX as the table command
`\flmCellsSet{<name>}`, which `tabularray` reads only at the head of a cell's text. It
applies the `tabularray` cell keys held in `\flmCellsKeys<name>` (a background `bg=`, a
font `font=`, a command around the text `cmd=`, of which a cell takes one: of two styles
that set it, the later one wins), and nothing where no keys are held.
`\flmCellsStyleKeys{<name>}{<keys>}` registers the keys of a cell style where none are
registered yet, as `\providecommand` defines a command. The definition piece registers the shipped
cell styles `green`, `blue`, `yellow` and `red` as the background colors
`flmCellsColor<name>` (declared with `\providecolor`, so a color defined first is the one
used). A host's own cell style is registered by a definition piece of the host's, which
the cell style's declaration requires on this backend:
`\flmCellsStyleKeys{<name>}{<keys>}`. To give a cell style other keys, define
`\flmCellsKeys<name>` before loading the style file, or redefine it after; for a name that
holds a digit, write `\csname flmCellsKeys<name>\endcsname`, since a command name written
after a backslash holds letters only.

**Floats and footnotes in a cell.** A float inside a cell is left to LaTeX and the author,
as for floats above: a cell is a box, and the `minipage` set-up given there works in it. A
footnote inside a cell is not set as it is elsewhere, as in every LaTeX tabular: its text is
lost, or set more than once, depending on what contains the table. `tabularray` sets a cell
several times, first to measure it, and each pass steps LaTeX's counters; the style file
loads `tabularray`'s `counter` library, which undoes those steps, except that a table set a
second time as a whole makes every later note count one step more. To have a note's text
set, give `\flmXNfootnote` a definition of your own before loading the style file, as for
nested notes ([Xnotes in the recomposed LaTeX](#xnotes-in-the-recomposed-latex)).

### Citations in the recomposed LaTeX

A citation ([`citations`](crate::stdcore::features::citations)) is written as the author
wrote it, through three LaTeX hooks that the style file defines with
`\ProvideDocumentCommand`, so that a definition made first in the author's class or
preamble is the one used ([`citations::latex`](crate::stdcore::features::citations::latex)).
The style file requests no LaTeX package for them: loading `natbib` or `biblatex` is the
author's choice. Nothing is numbered, sorted or compressed here: LaTeX numbers the cited
works, and sorts and compresses them (or not) as the author's bibliography set-up says.
The configuration's placement and form change nothing on this backend.

| written | recomposed | what the default definitions print |
|---|---|---|
| `text\cite[Thm 4]{a,b}\cite{c}` | `text\flmCitePreSpace\flmCite[Thm 4]{a,b}\flmCite{c}` | `text [1, 2, Thm 4][3]`: `\flmCitePreSpace` is `\unskip~`, which removes the space written before it and writes a space that does not break; each `\flmCite` is `\cite`, with its note where it has one |
| `\R\cite{a,b,c}` | `\flmRefCite{refs.~}{\flmCite{a,b,c}}` | `refs. [1, 2, 3]`: `\flmRefCite` writes the word, then the citations |
| `\R[S]\cite{a}\cite{b}` | `\flmRefCite{Refs.~}{\flmCite{a}\flmCite{b}}` | `Refs. [1][2]` |
| `\R[I]\cite{a}` | `\flmRefCite{}{\flmCite{a}}` | `[1]`: the citation alone |

`\cite` calls written one after the other, with nothing but whitespace, a single line break
and comments between them, are read as one *citation chunk*
([`citations`](crate::stdcore::features::citations)), and `\R` writes the citation chunk
that follows it as a noun of its sentence.

- A citation chunk of `\cite` calls writes one `\flmCitePreSpace`, then one `\flmCite` per
  call, in order: the first call and each call added after it. The author's whitespace
  before the chunk is written unchanged, and the hook removes it, so `text\cite{a}` and
  `text \cite{a}` both print `text [1]`.
- A citation chunk of `\R` writes no space hook, because it is a part of its sentence. Its
  word is the citation counter style's, as every output resolves it: singular or plural according
  to the chunk's number of items, capitalized under `S`, empty under `I`.
- The citation keys are written exactly as typed, each trimmed, the keys of one call joined
  by commas, with no escaping and no generated name (an underscore stays an underscore),
  because they are the names of the entries of the author's bibliography database. A
  citation key holding a character LaTeX cannot read is left to the author.
- The note is content, written as all content is; the optional argument is braced,
  `[{…}]`, where its content could be misread, as for every optional argument the family
  writes.
- A user definition whose template puts a placeholder among the citation keys of a
  `\cite` has each use site expanded ([User definitions in the recomposed
  LaTeX](#user-definitions-in-the-recomposed-latex)). A definition citing fixed keys, or
  with a placeholder in the note only, is written as a LaTeX definition, and LaTeX numbers
  its citations wherever the definition is used.

To change how citations are set, redefine the hooks:

- `\flmCite[<note>]{<keys>}`, to show entries in place (over `biblatex`'s `\fullcite`), or
  to print bare numbers (around `natbib`'s `\citenum`);
- `\flmCitePreSpace`, to `\unskip` for superscript citations, which take no space before
  them;
- `\flmRefCite{<word>}{<citations>}`, to set a citation used as a noun differently; with
  `\flmCite` printing bare numbers, the default prints `refs. 1, 2` unchanged.

**A citation in a heading set in capitals** (a running head under
`\pagestyle{headings}`, which uses `\MakeUppercase`): the style file adds `\flmCite` to the
LaTeX kernel's list of commands whose argument a change of case leaves alone, the list in
which the kernel keeps `\cite`. The kernel protects the first brace group after the
command, so the citation keys stay as written and the note is uppercased with the heading
(`RESULTS [1, THM 4]`). A note written braced or holding a command (`\emph{p.}`) takes that
protection instead, and the citation's keys are then uppercased, as for LaTeX's own
`\cite`. Where `hyperref` is loaded, a citation contributes nothing to a PDF bookmark, the
word of `\flmRefCite` included.

**The bibliography is LaTeX's.** No bibliography is written: the list of the cited works
and its citation entries lower to nothing on this backend. The list is made by LaTeX
(`thebibliography`, BibTeX, `biblatex`) from the author's database. The check that every
cited key has a citation entry still runs, so a citation key that no citation provider
supplies is an Error on this backend too.

### Pieces in the recomposed LaTeX

The backend places no [piece](#pieces) unless the `pieces` feature's render options list the
pieces to place on it
([`PiecesRenderOptions`](crate::stdcore::backends::pieces::PiecesRenderOptions),
[`PiecesStage::placing_only_named`](crate::stdcore::backends::pieces::PiecesStage::placing_only_named)),
because where a piece belongs in a LaTeX document (a title in `\title`) is for the LaTeX
file around the recomposed LaTeX to decide. Each piece listed there is lowered
by the backend's pieces stage, written by the regions stage as an output region of its own,
and left out of the entry artifact by the final stage; the caller places it, for instance
in a template of the LaTeX file. The
`meta` feature's rules for this backend ([`meta::latex`](crate::stdcore::features::meta::latex))
write the title and the date as their recomposed content, and the authors as each author's
recomposed content joined by LaTeX's `\and` (`Ada Lovelace \and Grace Hopper`), which is
what a LaTeX document hands to `\title`, `\author` and `\date`.

The table of contents is LaTeX's own on this output. LaTeX builds it from the headings it
typesets, where the LaTeX file asks for one with `\tableofcontents`; the recomposed LaTeX
hands each heading's `toc` and `alt` to LaTeX through the heading hook ([The LaTeX
hooks](#the-latex-hooks) below). The `toc` feature's piece `flm.toc` writes nothing on this
backend: where a render lists it for this backend, its three elements are lowered by
rules that write nothing ([`toc::latex`](crate::stdcore::features::toc::latex)), and the
recomposed LaTeX is the same as without it.

### The LaTeX hooks

A **LaTeX hook** is a macro that the recomposed LaTeX writes and the style file defines
with a plain default, which a user redefines in the document's preamble to change what the
recomposed LaTeX does at that point
([`preamble`](crate::stdcore::backends::latex::preamble)). The LaTeX family defines four
LaTeX hooks of its own, and `\flmLabelsWithDisplayText`, which `\flmHeading` calls. They
exist because LaTeX alone does not do two things that the recomposed LaTeX needs (give a
label of an unnumbered heading its title; keep several labels on one line of a
display-math environment), because FLM's reference options need a translation, and because
whether a document loads `hyperref` is its author's decision; the paragraphs after the
table explain each.

The features add LaTeX hooks of their own in the same way: the three hyperlink hooks
([Hyperlinks in the recomposed LaTeX](#hyperlinks-in-the-recomposed-latex)), the two hooks
of a term link ([Defined terms in the recomposed
LaTeX](#defined-terms-in-the-recomposed-latex)), the proof title hook ([Theorems in the
recomposed LaTeX](#theorems-in-the-recomposed-latex)), the four hooks of a quotation's
sections and `\flmIndent` ([Quotations in the recomposed
LaTeX](#quotations-in-the-recomposed-latex)), one hook per xnote kind, `\flmXN<kind>`
([Xnotes in the recomposed LaTeX](#xnotes-in-the-recomposed-latex)), the image hook
`\flmIncludeGraphics` ([Images in the recomposed LaTeX](#images-in-the-recomposed-latex)),
the float environment `flmFloat` with its eight commands ([Floats in the recomposed
LaTeX](#floats-in-the-recomposed-latex)), the table hook `\flmCellsTable` with its four
commands that each change one thing and the table command of a custom cell style ([Tables
of cells in the recomposed LaTeX](#tables-of-cells-in-the-recomposed-latex)), the formatting
hook of each verbatim kind, `\flmFmtVRB<kind>`, and `\flmVerbatimCodeSetup` for a code
block ([Verbatim content in the recomposed
LaTeX](#verbatim-content-in-the-recomposed-latex)), and the three hooks of a citation
([Citations in the recomposed LaTeX](#citations-in-the-recomposed-latex)).

| LaTeX hook | where the recomposed LaTeX writes it | what its default does | redefine it to |
|---|---|---|---|
| `\flmHeading` | on the line before every sectioning command: `\flmHeading[toc=true]`, then `\section*{Title}{\flmLabels{\label{ref1}}}` on the next line (the newline is for the reader; LaTeX reads the same tokens) | runs the sectioning command as written, then makes the heading's labels refer to it: by its number where the document class numbered it, by its **title** where it did not; lists a starred heading written `toc=true` in the table of contents; takes a key-value argument of its own, described below | change what a heading does |
| `\flmLabels` | around every group of labels | runs them in text; on a line of a display-math environment, runs the first label and makes every further label an alias of it | change what a group of labels does (`\def\flmLabels#1{#1}` restores plain LaTeX behavior) |
| `\flmLabelsWithDisplayText` | at the head of a term definition's body ([Defined terms in the recomposed LaTeX](#defined-terms-in-the-recomposed-latex)), after the caption of a captioned float without a number ([Floats in the recomposed LaTeX](#floats-in-the-recomposed-latex)); `\flmHeading` calls it | makes the labels refer to the current place with a given text as what a reference prints, by stepping a private counter whose printed form is that text | change how a label with a display text is made |
| `\flmRef` | for every reference: `\flmRef[S]{ref1}` | sets the reference with `\zcref` of the `zref-clever` package, translating FLM's reference options into that package's options where a counterpart exists and dropping any other | send references elsewhere: another package, another wording, no link |
| `\flmHyperref` | for every reference that shows the author's own words: `\flmHyperref{ref1}{the \emph{first} part}` | links that text through `\hyperref` where the document loaded the `hyperref` package, and prints the text alone where it did not | link such references differently: another package, a footnote, no link |
| `\flmHref`, `\flmUrl`, `\flmEmail` | for every hyperlink: `\flmUrl{https://example.com/}` | links through `hyperref`'s `\href`; the last two derive the address's display, as the HTML output shows it, and typeset it with `\nolinkurl` | typeset or link hyperlinks differently, or show something other than the display |
| `\flmTerm`, `\flmTermInDefterm` | for every term link: `\flmTerm{ref1}{entropy}`, and inside the definition of its own term `\flmTermInDefterm{Entropy}` | the first through `\flmHyperref`, the second as `\emph` with no link | set term links, or the term being defined, differently |
| `\flmProofTitle` | for every proof's title: `\begin{proof}[{\flmProofTitle{Sketch}}]`; with `resolved_proof_refs` on, also around the reference hook of a back-reference the proof shows, `[{\flmProofTitle{\flmRef{ref1}}}]` | with `resolved_proof_refs` on, over `amsthm`: `\proofname\space (#1)`, "Proof (Sketch)", as FLM's other outputs set a proof's title; off, over `phfthm`, which words the heading itself: the title alone, `#1`, "Proof of Sketch" | set a proof's title differently (`\renewcommand\flmProofTitle[1]{#1}` sets the title alone in the resolved form) |
| `\flmQuoteText`, `\flmQuoteLines`, `\flmQuoteAttributed`, `\flmQuoteBlock` | for every quotation section, inside its environment: `\begin{quote}\flmQuoteText{To be.}\flmQuoteAttributed{Shakespeare}\end{quote}` | the passage ending a paragraph, `#1\par`; the lines without paragraph indentation, the paragraph started first so that an empty first line has a line to end, `\par\begingroup\setlength{\parindent}{0pt}\leavevmode#1\par\endgroup`; the attribution on a line of its own, to the right, after a dash, `\par\begingroup\raggedleft---~#1\par\endgroup`; the paragraphs indented as the one item of a list without a label, with a left margin of `4em`, `\par\list{}{\itemindent\parindent\leftmargin=4em\relax}\item\relax #1\endlist` | set a quotation section differently |
| `\flmIndent` | before the content of every indented line of a lines section: `\flmIndent{1\flmTabWidth}` | space of that length that no line break removes, `\hspace*{#1}` | indent lines differently |
| `\flmXN<kind>` | for every xnote of that xnote kind: `\flmXNfootnote{…}` | hands the note to LaTeX's `\footnote`, or to the command the xnote kind gives | set that xnote kind differently |
| `\flmIncludeGraphics` | for every image: `\flmIncludeGraphics[width=5cm]{plot.png}` | `\includegraphics[max width=\linewidth,#1]{#2}`, over `graphicx` and `adjustbox` | show images differently |
| `flmFloat`, `\flmFloatSetUseConfig<case>`, `\flmFloatPlacementArgs<case>` | around every float: `\begin{flmFloat}{figure}{NumCap}…\end{flmFloat}` | a numbered float in the environment of its float type at `[tbph]`; an unnumbered float in a `center` environment where it is written, its caption through `\caption*`; the content centered | set floats, or one case of them, differently |
| `\flmCellsTable`, `\flmCellsHeaderFont`, `\flmCellsBeginCenter`, `\flmCellsEndCenter`, `\flmCellsMaxWidth` | for every table of cells: `\flmCellsTable{cc}{X[-1]X[-1]}{hspan=minimal,…}{\toprule …\bottomrule}` | sets the table in `tabularray`'s `tblr` at its natural width, and again with the fitted columns at `\flmCellsMaxWidth` (`0.96\linewidth`) where it is too wide, between `\flmCellsBeginCenter` and `\flmCellsEndCenter` (a `center` environment) except where LaTeX sets a single line; a header cell in `\flmCellsHeaderFont` (`\bfseries`) | set tables differently, or change one of those four commands |
| `\flmCellsSet{<name>}`, `\flmCellsStyleKeys{<name>}{<keys>}`, `\flmCellsKeys<name>` | at the head of a cell's text, for each custom cell style it carries: `\flmCellsSet{green}Berlin` | applies the `tabularray` cell keys held in `\flmCellsKeys<name>`, which `\flmCellsStyleKeys` registers where none are registered yet; the shipped `green`, `blue`, `yellow` and `red` set the background colors `flmCellsColor<name>` | give a cell style other keys |
| `\flmFmtVRB<kind>`, `\flmVerbatimCodeSetup` | around every inline verbatim content of that verbatim kind: `\EscVerb[formatcom=\flmFmtVRB{verbcode}]{…}`; at the start of every code block, with its language tag | `\flmFmtVRBverbtext` nothing, `\flmFmtVRBverbcode` `\ttfamily`, `\flmFmtVRBverba` `\itshape`; `\flmVerbatimCodeSetup` nothing | set a verbatim kind, or code blocks, differently |
| `\flmCite`, `\flmCitePreSpace`, `\flmRefCite` | for every citation: `\flmCitePreSpace\flmCite[Thm 4]{a,b}`, and for a citation used as a noun `\flmRefCite{refs.~}{\flmCite{a,b}}` | `\cite`, with the note where there is one; `\unskip~`; the word, then the citations | cite through another package, show the entries in place, or set superscript citations |

**Why the heading hook.** A LaTeX label records the last thing that was numbered. A
heading that the document class did not number (`\section*`, or a level below the class's
`secnumdepth`, which is `\paragraph` in `article`) numbers nothing, so a plain `\label`
after it records the number of the last thing numbered before it, and a reference to it prints the
wrong number without any warning. `\flmHeading` asks the LaTeX kernel whether the
sectioning command stepped a counter (`\@currentcounter`, which `\refstepcounter` sets)
and, where it did not, gives the labels the heading's title instead. A reference to an
unnumbered section therefore reads "Introduction". Nothing of LaTeX is patched, and the
test works on `article`, `report`, `book`, KOMA-Script, `memoir` and `titlesec`, with and
without `hyperref`
([`heading_piece`](crate::stdcore::backends::latex::preamble::heading_piece)).

**The heading hook's key-value argument.** `\flmHeading` takes an optional key-value
argument of its own, written on the hook and never on the sectioning command:

```latex
\flmHeading[alt={Short title}]
\section{A long and \emph{fancy} title}{\flmLabels{\label{ref1}}}
```

The argument has two keys:

- `toc=true|false` says whether the heading is listed in the table of contents. The
  recomposed LaTeX writes it on every heading, from the heading's own `toc` key-value. LaTeX
  never lists a starred sectioning command, so after a starred heading written `toc=true`
  the hook writes `\addcontentsline{toc}{section}{Title}`, with the sectioning command's own
  name and the alternative title where `alt` was written, otherwise the title; under
  `hyperref` the line links to the anchor that the starred command placed at the title. The
  hook does not remove an unstarred heading from the table of contents: `toc=false` there
  leaves the heading listed, and the hook writes a LaTeX package warning saying so. Without
  the key (a document written by hand), a heading is listed as LaTeX lists it.
- `alt` is an **alternative title**: the shorter or plainer form of the title that LaTeX
  puts in the table of contents, in the running head and in the PDF bookmark, while the
  page keeps the full title. The hook passes it to the sectioning command as that command's
  own optional argument, so `alt` does what `\section[Short title]{A long title}` does and
  nothing more. On a starred heading the hook does not pass the key to the sectioning
  command, because a starred sectioning command takes no optional argument: given one,
  `\section*[Short]{Long}` takes the opening bracket alone as the heading and runs `Short`,
  the closing bracket and `Long` into the following paragraph, without an error. A starred
  heading uses `alt` only as its table-of-contents line, where it is written `toc=true`
  (the key `toc` above). Writing the key on the hook gives starred and unstarred headings
  one shape.

Whether the key `alt` was written decides, not what it holds: `alt={}` is an empty
alternative title, and the hook passes `[]` to the sectioning command, which is LaTeX's way
of asking for an empty table-of-contents entry. A hook written without the key, or with an
empty key list (`\flmHeading[]`), passes nothing, and the sectioning command runs exactly
as written. Writing `alt` without a value is a LaTeX error, because the key requires one.
An unnumbered heading's labels take the *full* title as what a reference prints, not the
alternative title, so a reference to an unnumbered heading prints what the page shows. The
values, and the record that `alt` was written, are cleared at every call, so one heading's
keys never apply to the next heading; a key that the style file does not define is a LaTeX
error, reported at the heading that wrote it.

The recomposed LaTeX writes `alt` after `toc` where the heading was written with an
alternative title (the heading key `alt`; see [the language syntax
guide](crate::guide::language_syntax#the-star-and-the-key-values)): `\section[alt={Short
title}]{A long title}` is written `\flmHeading[toc=true,alt={Short title}]`. Where the
alternative title holds a command, the whole optional argument is braced, as every
optional argument holding a command is, so that a `]` inside it cannot end it:
`\flmHeading[{toc=true,alt={Short \emph{title}}}]`; LaTeX removes those braces when it
reads the argument. With the render option `heading_hook` off, nothing carries the key.

**Why the labels hook.** The LaTeX package `amsmath` keeps one label per line of a
display-math environment: given two, it raises the error "Multiple \label's: label 'x'
will be lost", and a reference to the lost label prints `??`. `\flmLabels` runs the first
label as written and records every further label as an *alias* of it. After the last page,
one line per alias is written to the auxiliary file; on the next LaTeX run, after every
label has been read from that file, each line makes the alias refer to what the first
label refers to, for LaTeX's own references and for `zref-clever`'s alike. Two LaTeX runs
resolve the aliases, as two runs resolve any LaTeX label
([`labels_piece`](crate::stdcore::backends::latex::preamble::labels_piece)).

**Where a link to a label with a display text leads.** With `hyperref`, a link to a label
made through `\flmLabelsWithDisplayText` leads to the heading's title, as a link to a
numbered heading does. The macro places no anchor of its own: a sectioning command,
starred or not, already places one at its title, and a `\label` written after it records
that anchor. The macro therefore steps its counter with the plain `\stepcounter`, and sets
only the two things a label needs: `\@currentlabel` (what the label records) and
`\@currentcounter` (what `zref-clever` reads for the reference type). `\refstepcounter`,
LaTeX's usual interface, would place a second anchor below the title and send the link
there
([`label_display_text_piece`](crate::stdcore::backends::latex::preamble::label_display_text_piece)).
A place with no sectioning command before it, such as a term definition in running text,
has no anchor to reuse, so the recomposed LaTeX writes `\phantomsection` there, which is
`hyperref`'s way of placing an anchor where no sectioning command places one.

**Why the reference hook.** A *reference option* is what a document writes in the
brackets of `\ref[…]{…}`, in FLM's own vocabulary: the key `noprefix`, which turns off the
name a reference puts before its target's number or title, and a key alone giving the
*prefix variant*, the form of that name to use. `zref-clever` has a vocabulary of its own,
and no key `noprefix`. The recomposed LaTeX writes the options as the document wrote them,
and the default of `\flmRef` translates them as it calls `\zcref`
([`refs_piece`](crate::stdcore::backends::latex::preamble::refs_piece)):

| written in the document | passed to `\zcref` | what it asks for |
|---|---|---|
| `noprefix` | `noname` | the target's number or title alone, with no name before it: "2", not "Section 2" |
| `S` | `S` | the first name capitalized and unabbreviated, for a reference that opens a sentence; both vocabularies spell it the same way |
| `capital` | `cap=true,abbrev=false` | the names capitalized and unabbreviated: "Lemma 1" |
| `lowercase` | `cap=false,abbrev=false` | the names in lowercase and unabbreviated: "lemma 1" |
| `abbreviated` | `abbrev=true` | the abbreviated names, "Lem. 1", where the reference type declares them, and the full names where it does not (the package's own fallback) |
| any other key | nothing | the option is dropped, and the reference prints the default wording |

The translation reads the option's key alone: a value written after the key is ignored, as
FLM's own reader ignores it (`\ref[noprefix=x]{…}` turns the name off like `noprefix`).
The names of the variants other than `S` are those a configuration declares, and only the
names the library ships (`capital`, `lowercase` and `abbreviated`, which every shipped
theorem kind declares) have a counterpart in `zref-clever`. A variant that a configuration
declares itself, such as a `short` variant for exercises, has none: the default drops it, so
the reference prints its target's default wording in LaTeX where FLM's other outputs print
the configured words. Dropping the option is necessary because `zref-clever` stops the
compilation on a key it does not define; the default wording is also what FLM prints for a variant that
a target does not declare. To carry a variant of your own into LaTeX, redefine `\flmRef`:
the translation is in the hook's default and nowhere else, so a redefinition receives the
options as the document wrote them, `noprefix` included. The abbreviated words belong to
the reference type: each theorem kind's declaration gives them beside the full words
([Theorems in the recomposed LaTeX](#theorems-in-the-recomposed-latex)).

**Why `\flmHyperref` asks whether the document loaded `hyperref`.** Making a link inside a
PDF takes the `hyperref` package, and loading it changes a whole document: every reference
becomes a link, PDF bookmarks are built, and the package needs a place of its own among the
packages the document loads. That is the author's decision about the document, so the
definition piece of the references neither loads `hyperref` nor requests it. (A document
holding a hyperlink, a term definition, or a labeled float with a caption and no number
loads it anyway, because those features' rules request it for `\href` or
`\phantomsection`.) `\flmHyperref` asks instead, with the LaTeX kernel's `\IfPackageLoadedTF`,
where the reference is written, so the answer is right whether the author loads `hyperref`
before the style file or after it. It links the text where the package is loaded and prints
the text alone where it is not; either way the recomposed document compiles and the
author's words appear where they were written. The same definition piece also tells `hyperref` what
such a reference contributes to a **PDF bookmark**: the text alone, since a bookmark is
plain text and holds no link. To link such references differently (another package, a
footnote with the target's number, no link at all), redefine `\flmHyperref`, before or after
loading the style file. To have the package loaded for you, request it with the render
option `extra_packages`
([`hyperref_piece`](crate::stdcore::backends::latex::preamble::hyperref_piece)).

**Turning the hooks off.** Two render options, both on by default, control these hooks:

- [`heading_hook`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::heading_hook)
  off writes the sectioning command on its own, `\section*{Title}`, with the heading's
  labels after it. The hook's test is then not run and nothing replaces it.
- [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks)
  off writes bare `\label{ref1}` commands; `\zcref[…]{ref1}` for a reference, requesting
  `zref-clever`; and `\hyperref[ref1]{…}` for a reference showing the author's words,
  requesting `hyperref`. A term definition's labels are written bare after
  `\phantomsection`, with no display text.

With both off, the recomposed LaTeX holds only standard LaTeX, and the style file holds only
what the document's own content needs. [Options that make you responsible for a LaTeX
behavior](#options-that-make-you-responsible-for-a-latex-behavior) says what each of the two
options, when off, leaves to you.

### Escaping and the Unicode modes

Document text goes through the family's escaping
([`escape`](crate::stdcore::backends::latex::escape)). The text is first composed to
Unicode's canonical composed form, so that a letter followed by a combining accent is the
accented letter. The ten special characters `$ & # ^ _ % { } \ ~` become commands. Every
other non-ASCII character is looked up first in the render option `unicode_overrides`, then in the
Unicode-to-LaTeX table of [`untechxt`](crate::untechxt), which gives each character a LaTeX
spelling and states what that spelling needs. What is written then depends on the render
option
[`unicode`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::unicode):

- [`Declare`](crate::stdcore::backends::latex::UnicodeMode::Declare), the default: the
  character itself, as UTF-8, and the style file declares it once with
  `\newunicodechar{<char>}{<spelling>}` (requesting `newunicodechar`). The recomposed LaTeX
  keeps the characters the author typed and compiles under pdfLaTeX as well as under the
  Unicode engines (LuaLaTeX, XeLaTeX). `inputenc` is never requested: a LaTeX of 2018 or
  later reads UTF-8 itself.
- [`Replace`](crate::stdcore::backends::latex::UnicodeMode::Replace): the spelling in place,
  braced where it would otherwise merge with the letters after it
  (`Caf\'e {\textemdash} na\"ive`).

**What a spelling needs** comes from the table. Each entry of the table carries the set of
*preamble chunks* (untechxt's term) that its spelling needs, and a preamble chunk is one of
two things:

- a LaTeX package with the options it is loaded with: `\ensuremath{\mathbb{C}}` for `ℂ`
  needs `amssymb`; a Cyrillic letter needs `fontenc` with the font encoding that declares
  its command;
- a block of declarations that no package makes: `\ensuremath{\UnxTBbold{0}}` for `𝟘` needs
  the `bbold` font family declared as a math alphabet, because the package that offers that
  font would change `\mathbb` for the whole document.

The style file holds exactly the preamble chunks that the document's characters need, in both modes:
the packages as `\RequirePackage` lines in its package block, and the declarations as
definition pieces after the packages, each guarded so that two style files loaded in one
LaTeX run do not clash. Nothing is deduced from the commands a spelling writes. A spelling
whose commands belong to a font encoding that the document does not select carries the
switch itself: the Cyrillic letter commands are declared in one font encoding and are
unusable while another is selected, so `я` is spelled
`{\fontencoding{T2A}\selectfont\cyrya}`, and the style file requests `fontenc` with `T2A`
beside the engine's own font encoding (`T1` under pdfLaTeX, `TU` under LuaLaTeX and
XeLaTeX), which stays the document's default. Such a request differs by engine, so the
package block writes it inside a test of the engine ([A line can differ by
engine](#the-stages-the-tables-and-the-artifact-set)). Every command that a declaration
defines carries untechxt's own prefix, `\UnxT…`, a name space this family does not use.

An override
([`unicode_overrides`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::unicode_overrides))
replaces the spelling of one character in either mode. Its spelling is written as given,
and **nothing it needs reaches the style file**: the style file requests only what the
table says its own spellings need, so what an override's spelling needs belongs in the
preamble of the LaTeX file that loads the style file, or in the render options
`extra_packages` and `extra_preamble`.

A character for which no spelling is known is written unchanged under `Declare`, and as its
code point (`\ensuremath{\langle}\texttt{U+0E18}\ensuremath{\rangle}`) under `Replace`; it
is reported once per tree as the Warning `flm.latex-recomposed.unknown-character`.
Mathematics and verbatim content are written byte for byte in every mode: a non-ASCII
character inside them is neither declared nor replaced, so under `Declare` every non-ASCII
character of the document's text, and not of its mathematics or verbatim content, is
declared.

### References and safe labels

The recomposed LaTeX writes no anchor and resolves no reference. Every label is written as
a **safe label**: the key the recomposed LaTeX writes for an FLM label, `ref1`, `ref2`, …
in order of first mention, from one counter for the whole document regardless of the label
type, so that any label text an author wrote is carried through LaTeX's auxiliary file
([`SafeLabels`](crate::stdcore::features::refs::SafeLabels)). The document-scope stage
`flm.refs.safe-labels` allocates the safe labels, and the rules read them through the table
[`flm.labels.safe`](crate::stdcore::features::refs::SAFE_LABELS), which holds both
directions of the correspondence. Every rule that writes labels, except the `refs`
feature's own reference rule, reads that table optionally: in a registry without the `refs`
feature, where nothing allocates safe labels, each writes the labels as the document wrote
them, inside the `\flmLabels` group as with the feature.

A label is mentioned in two ways only:

- by something that carries a [reference point](crate::model::aspects::RefPoint) (a heading,
  a line of a display-math environment), which mentions the labels of its reference points;
- by a reference (`\ref`, `\hyperref`, `\eqref`, a term link), which mentions every target
  it refers to, whether or not anything carries that target.

Every reference is therefore written under a safe label, including a reference to nothing,
which LaTeX reports as an undefined reference and prints as `??`. A `\label` written for
something that carries no reference point, such as a label a user definition absorbs as a
field of its own, refers to nothing in the document: it gets no safe label and is not
written.

**What is written.**

- A label becomes `\label{ref1}` inside the labels hook.
- A `\ref` becomes `\flmRef{ref1}`, whose default sets it with `\zcref` of the
  `zref-clever` package. `\zcref` prints its target the way FLM's `\ref` does ("Section 2"),
  which LaTeX's own `\ref` does not. A `\ref` to several targets is one `\flmRef` whose
  argument holds their safe labels separated by commas (`\flmRef{ref1,ref2}`), which is how
  `\zcref` reads a list of targets, so that LaTeX writes the list, its ranges and its plural
  name.
- The options of a `\ref` are written back in FLM's own vocabulary, as `key` or
  `key={value}`, the key and the value's text escaped; the default of `\flmRef` translates
  them ([The LaTeX hooks](#the-latex-hooks)). A user whose reference command reads the options
  differently maps them in their own definition of `\flmRef`, the only place where the
  recomposed LaTeX refers to a reference command.
- `\hyperref[sec:a]{the \emph{first} part}` becomes
  `\flmHyperref{ref1}{the \emph{first} part}`: the same safe label, and the author's words
  recomposed as the content they are, so a formatting command, a special character or a user
  definition inside them is written as anywhere else.

**A reference in a user definition's template.** In the template of a user definition, a
reference whose target a placeholder completes (`(see \ref{#1})`, `\ref{sec:#1}`,
`\eqref{#1}`, `\hyperref[#1]{…}`) refers to a label that only a call supplies, and a safe
label is computed from the label's characters. Each use site of such a definition is
therefore expanded ([User definitions in the recomposed
LaTeX](#user-definitions-in-the-recomposed-latex)), and its reference is written under the
safe label of the label that use site gives. Where the target a use site completes is no
label of the document, the [value catch-up](crate::guide::concepts_overview#value-catch-up)
of the expanded use site reports it as the document's own references are reported
(`flm.refs.unresolved`, or `flm.refs.bad-label` for a target not written `type:label`),
once per use site. The per-tree stage `flm.refs.value-safe-labels` then gives it a **value
name** ([`SafeLabels`](crate::stdcore::features::refs::SafeLabels)): the safe label written
for a label inside a synthesized value, `ref-v1`, `ref-v2`, … numbered from one in each use
site ([`value_name`](crate::stdcore::features::refs::value_name)), which no safe label of
the document takes. A target such as `x%y` or `u\v` therefore never reaches the output as its
characters. A target that a use site gives and that the document also mentions is written
under the document's safe label.

**Validation runs anyway.** The recomposed LaTeX does not depend on whether a reference
resolves, but the run reports an unresolved reference anyway: the `latex-recomposed`
workflow, like every rendering workflow, demands the validation collection
[`flm.validated`](crate::stdcore::VALIDATED) beside its output, so the resolution of the
references and the label check run as under the
[`check`](crate::stdcore::CHECK_WORKFLOW) workflow.

**Options.** [`RefsRecomposeOptions`](crate::stdcore::features::refs::RefsRecomposeOptions)
changes two things:

- `safe_label_types` marks a label type as written raw: `sec:intro` is written as written,
  and no safe label is allocated for it. A label not written `type:label` has no label type,
  so this option cannot select it: it is given a safe label from the same counter, keyed by
  its text, and written like any other, so that the recomposed LaTeX compiles and a
  reference to it resolves; the malformed label is reported by the label check, which every
  workflow runs, whether the text was written as a construct's label or as a `\ref` target.
- `protect` (off by default) wraps every reference, of either kind, a `\hyperref`'s own
  words included, in `\NoCaseChange{\protect …}`, and the style file then requests
  `textcase`, which supplies `\NoCaseChange` on every LaTeX kernel. A command that changes
  case, such as `\MakeUppercase`, changes the whole reference, its options and its safe
  label alike, and that fails in two ways without an error: inside `\MakeUppercase`,
  `\flmRef[noprefix]{ref1}` reaches the hook as `NOPREFIX`, a key the hook has no
  translation for, so the option is dropped and the name the author turned off comes back;
  and the safe label is uppercased, so LaTeX looks for a label `REF1`, finds none, and
  prints `??` with only a warning. A document whose headings or captions are uppercased
  should turn this option on.

### Mathematics in the recomposed LaTeX

The rules of `math-passthrough` for this backend
([`math_passthrough::latex`](crate::math_passthrough::latex)), built
from the parts of the `math` feature's library
([`math::latex`](crate::stdcore::features::math::latex)), write inline and display
mathematics as the delimiters around the content written from its nodes: byte for byte,
except that a use site of a user definition is written as its call, with the definition in
the style file, or as its expansion ([Definitions used inside
mathematics](#definitions-used-inside-mathematics)).

**Text commands.** One part of a mathematics body is not written byte for byte: the
argument of a **text command**, such as `\text{…}`, `\textbf{…}` or `\mbox{…}` (the commands
of [`COMMANDS`](crate::stdcore::features::math::text::COMMANDS), which the `math` feature
declares for mathematics mode with an argument read in text mode). The command is written
under its own name, and its argument is recomposed as the document text it is: a special
character escaped (`\text{50\% of it}`), a user definition written as its call beside its
definition, mathematics nested in it written from its nodes. `\text` itself belongs to the
`amsmath` package, so a document that writes it has that package requested by the style
file; every other command of the list belongs to the LaTeX kernel and needs nothing.

**The packages that the author's mathematics needs are the author's to load.** The rest of
a mathematics body is written byte for byte, so a body that uses `\boldsymbol` or `\lVert`
is written with those commands unchanged, and the style file does not request `amsmath` or
`amssymb` because of them: the style file requests only what the library's own rules write.
A document whose mathematics uses a command that its preamble does not provide does not
compile; this is one of the two ways the recomposed LaTeX can fail to compile with every
option at its default ([What the default options aim
for](#what-the-default-options-aim-for) gives the other). To have the style file request
such a package, list it once in the render option `extra_packages`, for the whole document.
The library does not check which commands a mathematics body uses.

**Display-math environments** are rebuilt from their equation lines
([`equation_lines`](crate::stdcore::features::math::lines::equation_lines)). An equation line
is one line of a display-math environment, as its body reads between two `\\`: its
mathematics, its labels, its tag and its no-number mark
([equation lines](crate::guide::concepts_overview#equation-lines)). For each line, the rule
writes, in order:

1. the line's content, node by node: a comment as everywhere (as written, or nothing under
   `drop_comments`), the rest byte for byte, a use site of a user definition as its call or
   its expansion;
2. `\tag*{…}` around the tag's text where the line has a tag. The tag is content of the
   document, lowered and escaped like any other (`\tag{50\%}` becomes `\tag*{(50\%)}`, and a
   text command inside it is written as its own LaTeX); the parentheses of an unstarred tag
   are written here, so that LaTeX adds none of its own;
3. the line's labels as one group, `\flmLabels{\label{<safe label>}…}`;
4. `\nonumber` where the line is not numbered;
5. the line's `\\` with its spacing argument. A `\\` that ends the body, followed only by
   whitespace and comments, is not written, because `amsmath` would set an empty line for
   it.

The labels of a line are written from the line's own data (the environment reads them as
it splits the body into lines), never by lowering a `\label` node, which is a field and is
not written where it occurs. Every environment except `equation` requests the `amsmath`
package. Numbering is left to LaTeX: the numbers of a compiled document are those of the
document class, not those FLM computed, and LaTeX resolves a reference to an equation line.

**The newline rule** is the `math` library's
([`normalize_line_newlines`](crate::stdcore::features::math::latex::normalize_line_newlines)),
and every LaTeX writer of an environment's lines applies it, the rendered LaTeX's included:
a label or a final `\\` removed from a line of its own would otherwise leave a blank line,
which LaTeX refuses inside mathematics. Inside a line's content, every run of spaces, tabs
and newlines that holds a newline becomes one newline followed by the indentation after the
run's last newline, so that a label removed from a line of its own, or a dropped comment,
leaves no blank line and no trailing spaces. A run that comes right after a line's end (the
run the previous line ended with, or a comment) is dropped. The run that ends the content is
written back after the labels and the `\\`, so that `\label{ref1}` ends the line as written:

```text
\begin{align}                  \begin{align}
  x &= 0;                        x &= 0;\label{ref1}\\
  \label{eq:one}\\       →       z &= 1;
  z &= 1;                      \end{align}
\end{align}
```

A comment is a node of its own among the line's content, and the comment writer ends the
comment's line itself, so a line whose content ends in a comment has its tag, labels,
`\nonumber` and `\\` on the next line, after that newline and after the indentation the
source left there. A comment can therefore never hide a line's commands.

[`MathRecomposeOptions`](crate::stdcore::features::math::MathRecomposeOptions) changes two
things: `surround_display_math` adds LaTeX before and after every displayed formula, the
display-math environments and the `\[…\]` groups alike, and
`emit_flm_math_environment_macro` writes every display-math environment
through the guarded macro `\flmMathEnvironment{<env>}{<args>}{<body>}`.

**Without the references feature,** the group of labels is written in the same way. That
feature allocates the safe labels, so in a registry built without it (for instance, one that holds
the `math` feature and few others), each `\label` holds the label as the document wrote it, byte
for byte and **unescaped**, as for a heading in the same registry ([References and safe
labels](#references-and-safe-labels)). The `\flmLabels` group around it is unchanged, which
keeps a line carrying two labels from breaking the compilation: `amsmath` alone raises an
error and loses all but the last label.

**Leaving the labels out.** In such a registry, and only there, the render option
`drop_math_labels_without_refs` (off by default) writes nothing for an equation line's
labels: no `\label` command and no group. A `\tag` and a `\nonumber` are not labels and are
written as usual ([Options that make you responsible for a LaTeX
behavior](#options-that-make-you-responsible-for-a-latex-behavior) says what the option
leaves to you).

**A label on a line without a number.** A line that can carry no number (a line marked
`\nonumber`, and every line of `equation*`, `align*` or `gather*`) is a reference point that
keeps no label, because nothing can refer to a line without a number to print. A `\label`
there is an error of the document, reported when the body is parsed. The label is still
written into the recomposed LaTeX: under its safe label where something allocated one (a
`\ref` to it does, so that the `\label` and the `\ref` still pair up), and as written
otherwise, byte for byte and unescaped, as any label without a safe label is written. What
LaTeX does with such a label, or with a reference to an unnumbered line, is left to LaTeX;
the library has already reported the document's error.

### User definitions in the recomposed LaTeX

A user definition is a macro, environment or special character sequence that a document's
author or the host defines by substitution: a name, an argument syntax, a template, and the
parsing modes it is defined for
([`Definition`](crate::lang::rewrites::userdefs::Definition)). On this backend:

- a definition with the expansion policy `always` is expanded before lowering, as in every
  plan;
- a definition with the expansion policy `per-plan` (the default) is *kept*: its use sites
  stay in the tree, and its `emit-call` rule
  ([`userdefs::kept`](crate::stdcore::features::userdefs::kept)) writes each use site as a
  call of the definition, while the style file holds the definition itself, once per name,
  regardless of the number of use sites and trees. Such a definition is a **kept
  definition**.

#### When a definition is written as a LaTeX definition

A user definition is written into the style file as one LaTeX definition, and each use site
as a call of it, except in the five cases below, where each use site is written as its
expansion and nothing is reported. In the LaTeX definition, each placeholder of the template
(a `#1` or a `#{name}` that refers to an argument) becomes a LaTeX parameter, which LaTeX
replaces at each call by what the call passes. The template's content is lowered through
the rules as document content is (a formatting command, a literal spelling, an `always`
definition's expansion, a use of another kept definition, which then reaches the style file
too), and each placeholder is written as the LaTeX parameter of its argument's value (`#1`,
`#2`; a named placeholder `#{greeting}` by the number of the argument it refers to).

Each use site is **instantiated**, that is, written as its expansion, as the `text` backend
prints it, in five cases:

- **The template reads an argument's characters.** A parameter works wherever the template
  sets the argument as content (`see #1`, `\emph{#1}`, `\ifblank{#1}{…}{…}`). A template
  that puts an argument into an address (`\href{https://doi.org/#1}{#1}`, `\url{#1}`), a
  label (`\ref{sec:#1}`, `\eqref{#1}`, `\hyperref[#1]{…}`), a term (`\term[#1]{…}`), or
  code (`\verbcode{#1}`, a code block) needs characters that are known at each call and not
  before, which no parameter can give it. The rule that reads the characters finds this as
  the template is lowered
  ([`chars_at_lowering`](crate::core::lowering::chars_at_lowering)); nothing inspects the
  template's shape, so a command that a later feature adds behaves the same way.
- **A specials definition** (a character sequence written without a backslash) is not a
  LaTeX command, so there is nothing for LaTeX to define.
- **A definition marked always expanded** (the expansion policy `always`) is expanded
  because its author asked for it.
- **A definition the registry build refused** has no template to write; the refusal was
  reported when it happened.
- **A definition that uses a definition of the three previous cases** is written expanded
  with it.

The last four cases are decided once, when the registry is built; a host can ask a
definition why
([`UserDefinition::recomposable`](crate::lang::rewrites::userdefs::UserDefinition::recomposable),
which returns the reason as a
[`NotRecomposable`](crate::lang::rewrites::userdefs::NotRecomposable)). One further shape
exists only for code that extends the library: a template holding a placeholder that is
neither an argument nor the body, whose content only that code's expansion hook computes.
No definition that a document declares can have it.

Every other definition is written as a LaTeX definition, regardless of what it holds:

- arguments of every shape that can be declared, at any position: a `{…}` group, an
  optional `[…]` group, a characters-only group, a key-value group, a verbatim argument, a
  `*` marker, and a trailing tack-on argument, which absorbs the field macros (such as
  `\label`) written after the call
  ([fields](crate::guide::concepts_overview#fields-data-a-construct-reads)) and which the
  template cannot refer to (below);
- every conditional and every declared default;
- an environment's template, regardless of what it does with its body: set it between two halves,
  wrap it, repeat it, or drop it;
- any number of arguments (past nine, the call packs them into one group, below);
- a macro and an environment of one name, which are two definitions in FLM and two in the
  output, under two prefixes;
- one name declared once per parsing mode, which becomes two definitions under two names
  ([Per-mode definitions](#per-mode-definitions));
- a definition that uses itself ([Definitions that use
  themselves](#definitions-that-use-themselves)).

#### The canonical call

Where the document writes `\around[see]{Knuth}`, the recomposed LaTeX writes

```latex
\flmUDaround{see}{Knuth}{p.}
```

This is the **canonical call**: the definition's name ([The names of recomposed
definitions](#the-names-of-recomposed-definitions)), then one pair of braces per argument,
in the order the arguments are declared, and nothing else. Every call of every user
definition is written this way: no square brackets, no star, no argument left out. FLM has
already read the document, so every argument's value is known when the LaTeX is written,
and LaTeX has nothing left to work out: a call ends at its last closing brace, LaTeX looks
ahead for nothing, and a `[` or a `*` written after a call is printed as written. The call
is less recognizable than what the author typed.

- **An argument left out is written out.** Where the definition declares a default for an
  argument, the call writes the default where the use site omitted the argument (the `{p.}`
  above). Where it declares none, the call writes an empty group `{}`. So `\around{Knuth}`
  is written `\flmUDaround{}{Knuth}{p.}`. This is exactly what FLM substitutes at that
  place, so the printed result is the same.
- **A default computed from the call is computed before it is written.** A default may
  refer to another argument (`#{long}`), or test one with a conditional. For a definition
  `\tag` with an optional argument `short` and a mandatory argument `long`, whose body is
  `[#{short}] #{long}` and whose default for `short` is `#{long}`,
  `\tag[TL;DR]{A long title}` is written `\flmUDtag{TL;DR}{A long title}`, and
  `\tag{A long title}` is written `\flmUDtag{A long title}{A long title}`: the default is
  worked out from what this call wrote, then written as an ordinary group. A default holding
  a conditional is also decided here, so the style file's definition takes plain groups and
  holds no default at all.
- **Every argument becomes one pair of braces**, regardless of its shape in the document. A
  **key-value argument** (`\kv[width=3,align=left]{…}`) is written as its content, keys
  included: `\flmUDkv{width=3,align=left}{…}`. FLM never reads the keys separately, and
  parses a key-value argument as it parses an optional one. A **verbatim argument**
  (`\code|a_b|`, `\code{a_b}`, whichever delimiters were chosen) is written as its
  characters, escaped as text inside the argument's group: `\flmUDcode{a\_b}`. The
  delimiters belong to the use site's syntax. The definition is therefore an ordinary
  expandable macro, and LaTeX's verbatim argument specifier `v` is never used. Where the
  template puts the argument inside verbatim content (`\verbcode{#1}`), the use sites are
  expanded instead (above).
- **A question about whether an argument was written adds a flag.** If the template asks
  `\IfProvidedTF{#1}{…}{…}` ("did the writer write this argument?"), the call carries the
  answer as a *flag*: `{\BooleanTrue}` where the use site wrote the argument,
  `{\BooleanFalse}` where it did not. The flags come after all the values, one per argument
  asked about, in the order the arguments are declared, and the definition's body reads them
  with LaTeX's `\IfBooleanTF`. A definition of two arguments that asks about the first is
  called `\flmUDmymacro{Hello}{Albert}{\BooleanFalse}`.

An **effective argument** is one brace group of a call: one per declared argument, plus
one per question. Values and flags count together.

**More than nine effective arguments.** A LaTeX macro body can refer to nine parameters,
`#1` to `#9`, and no more, and values and flags count together against that limit (a
definition of six arguments that asks about five of them has eleven effective arguments).
Past nine, a call passes **every** effective argument in one group, one pair of braces
around each, in the same order:

```latex
\flmUDbig{{a}{b}{c}{d}{e}{f}{g}{h}{i}{j}{k}{l}}
```

The definition then takes that one group, and its body reads effective argument *n* with
`\flmArg{<n>}{#1}`, a command the style file defines for this purpose, beside two tests
(`\flmIfArgTF` for a flag, `\flmIfArgBlankTF` for a blank test):

```latex
\providecommand\flmUDbig[1]{[\flmArg{1}{#1}|\flmArg{7}{#1}|\flmArg{12}{#1}]}
```

An environment packs in the same way: it declares one `+m` in place of one `+m` per
effective argument, and its begin code and end code both read from that group with
`\flmArg`, so an argument is available after the body too:

```latex
\ProvideDocumentEnvironment{flmUEpackednote}{+m}
  {(note \flmArg{1}{#1}: }{ — end \flmArg{10}{#1})}
```

A definition of nine effective arguments or fewer keeps the plain form, with `#1` to `#9`
in its body.

**Tack-on arguments.** A tack-on argument is the last argument a definition may declare:
it absorbs the macros written *after* the call (a `\label{…}` after a theorem, for
instance) rather than anything between braces
([fields](crate::guide::concepts_overview#fields-data-a-construct-reads)). A template cannot refer to such an argument:
what it absorbed is whole commands, which a parameter cannot hold. Referring to one is an
error, reported when the definitions are read, on every backend. A definition that declares
a tack-on argument and does not refer to it is written as a LaTeX definition like any other,
and the tack-on argument is not one of its effective arguments: no group is written for it,
so `\thm{Euclid}\label{thm:euclid}` and `\thm{Thales}` are both written `\flmUDthm{…}` with
the title alone, and what the call absorbed is not written. A `\label` that a definition
absorbs that way is not a label of the document: nothing refers to it, and no safe label is
allocated for it.

#### Expandable definitions, and robust ones on request

Each recomposed macro is written as a plain long expandable macro:

```latex
% \flmUDaround (the document's \around): defined unless the name is already taken, …
\providecommand\flmUDaround[3]{\textbf{#1}\emph{#2}\textbf{#3}}
```

`\providecommand` gives three properties:

- the macro is **long**, so an argument may hold a paragraph break;
- it is **expandable**, so a call works everywhere LaTeX expands text instead of typesetting
  it: a PDF bookmark under `hyperref` holds the definition's text, with no warning in the log,
  and so do an `\edef` and a case change;
- the name is **guarded**: a definition of it made in the author's class or preamble
  *before* the style file is loaded is the one used, as for every other definition in the
  style file. After loading, `\renewcommand` replaces it, as the comment line above each
  definition says.

**Robust on request.** An expandable definition cannot hold something that must not be
expanded early, such as a command with a side effect (which a user definition's body is not
meant to have, since a definition stands for content). Where a body holds one, or where the
words of a definition must be left alone by a case change, list the definition in the
render option
[`robust_names`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::robust_names).
It is then written through LaTeX's document command interface, with one `+m` per effective
argument, which makes it a *robust* command (one LaTeX protects from expansion):

```latex
\ProvideDocumentCommand\flmUDhello{+m}{Hello, #1!}
```

The call is written exactly as before, and the guard is the same
(`\DeclareDocumentCommand`, unconditional, where the name is also listed in `bare_names`).
The comment line above such a definition gives `\RenewDocumentCommand` as the command that
replaces it after the style file is loaded; `\renewcommand` would also work, but would put
an expandable macro in its place. What changes is what LaTeX does with the call where it
does not typeset it:

| | expandable (the default) | robust (the option) |
|---|---|---|
| in a table of contents or an auxiliary file | the definition's own text | the call itself, run when the file is read back |
| in a PDF bookmark (`hyperref`) | the definition's own text, no warning | dropped, with "Token not allowed in a PDF string" in the log |
| under `\MakeUppercase` | the definition's words are uppercased too | the definition's words stay as the definition wrote them |
| a body holding something that must not be expanded early | expanded where the call is, which may fail | run where the call is |

Every definition can be written either way, regardless of its shape: a definition of more than
nine effective arguments declares the one `+m` its packed group needs, and a verbatim
argument's value is a brace group like any other. An entry of `robust_names` that has
nothing to act on is accepted and does nothing, so that listing every name you own is safe:

- an **environment**, because an environment is robust already;
- a name that the document does not declare, since an entry is a wish about a name;
- a definition that is not written as a LaTeX definition at all (one written expanded at
  each use site, above).

The only entry refused is a name beginning with `flm`, the name space of the library's own
LaTeX; it is reported as `flm.latex-recomposed.definition-name-refused`, and the definition
is written as an expandable macro.

#### Environments

An environment cannot be expandable (`\begin{env}` opens a group, runs the begin code, sets
the body and runs the end code, which expansion alone cannot produce), so a recomposed
environment is written `\ProvideDocumentEnvironment{flmUEnote}{+m +m}{…}{…}`, with one `+m`
per effective argument (or one `+m` where the call packs its arguments), guarded in the same
way.

**An environment whose template does not put the body between two halves takes the body as
an argument.** LaTeX sets an environment's body between the begin code and the end code. A
template that refers to its body once, at its own top level, is cut in two there:
`[Note: #{body}]` becomes the begin code `[Note: ` and the end code `]`. A template that
puts the body elsewhere cannot be cut that way: `\emph{#{body}}` puts it inside another
command's argument, and `#{body}#{body}` uses it twice. A template that does not use the
body at all cannot be cut either. Such an environment is written with LaTeX's
body-grabbing argument specifier `+b`, which passes the body as an argument instead of
setting it:

```latex
\ProvideDocumentEnvironment{flmUEwrap}{+m +b}{(\emph{#2} — #1)}{}
```

The body is always the **last** argument, after one `+m` per effective argument (or after
the one `+m` of a packed environment, where the body is `#2`); the whole definition is the
begin code, with the body written wherever the template puts it, and the end code is empty.
The call does not change: what a call writes between `\begin` and `\end` is the body either
way, the body is not counted among the effective arguments, and one such environment nests
inside another of the same name. The `+` makes the body long, so it may hold a paragraph
break.

A template that does not use the body is written the same way and never writes that
argument, so LaTeX drops the body, as FLM does when it expands the definition. One
difference remains: LaTeX *reads* the body before dropping it, so a body whose braces or
environments do not balance stops the compilation, where FLM drops it unread.

**A code block works wherever it is placed.** A verbatim block
(`\begin{verbatimcode}…\end{verbatimcode}`) is content whose characters LaTeX must read with
meanings of their own (`%` is not a comment, `\` starts nothing), and LaTeX gives a
character its meaning as it reads it from the file, once. A block written out in full relies
on that, so it works only where it reaches LaTeX as lines of the file. Everywhere else
(inside an argument of a call, inside the body an environment passes to its definition as
an argument, inside a definition's own body in the style file) the recomposed LaTeX writes
the block as one call instead:

```latex
\flmVerbatimBlock[python]{flmVerbatimCode}{{{def\ f(x):}}{{\ \ \ \ return\ x}}}
```

The style file defines that command. Its last argument holds one brace group per line of
the block and, inside each line, one brace group per tab-separated segment, each written in
the escaped encoding of inline verbatim content (a backslash before every printable ASCII
character that is not a letter or a digit). The command rebuilds the block where it is and
enters the block environment, so the language tag, the `\flmVerbatimCodeSetup` hook and any
redefinition apply exactly as to a block written out in full, and the two forms set the
same block, tabs and empty lines included. The brace groups also survive a *moving
argument* (a heading's title, which LaTeX writes to the table of contents and reads back),
where a line feed would come back as a space.

The rule, in one sentence: a block is written out in full wherever it reaches LaTeX as lines
of the file (in the document's own text, and in the body of an environment whose template
is cut in two), and as the call above wherever it reaches LaTeX inside a group LaTeX reads
as tokens. A code block in the document's text is therefore written as the author wrote
it. Inline verbatim content needs none of this: `\verbtext{…}` is written as `\EscVerb{…}`,
an ordinary braced argument, which works anywhere.

**A PDF bookmark cannot hold a block**, because a bookmark is plain text. Where `hyperref`
is loaded, the style file tells it what to put in a bookmark instead: inline verbatim
content gives its text, and a block gives its characters run together, with no line breaks.

#### The names of recomposed definitions

A recomposed definition is never written under the name the document gives it
([`recomposed_name`](crate::stdcore::backends::latex::names::recomposed_name)):

- a macro whose name is ASCII letters only becomes `\flmUD<name>`: `\hello{World}` is
  written `\flmUDhello{World}`;
- an environment becomes `flmUE<name>`: `\begin{note}…\end{note}` is written
  `\begin{flmUEnote}…\end{flmUEnote}`; an environment name is never encoded, since LaTeX
  only looks it up through `\begin{…}` and never reads it as a command name;
- a macro whose name holds anything else (a control symbol such as `~`, a non-ASCII
  character, a digit) becomes `\flmUX<encoding>`, where the encoding writes each
  character's code point in uppercase hexadecimal without leading zeros, with the decimal
  digits `0` to `9` written as the letters `a` to `j` and the letters `A` to `F` kept, and
  several characters joined by the letter `x`: a definition of the control symbol `\~`
  (U+007E) is written `\flmUXhE`.

The renaming applies to the definition in the style file and to every call, in text and
inside mathematics alike, and the comment line above each definition gives the author's
own name beside it. Macros and environments take different prefixes because LaTeX keeps
both in one name space (`\newenvironment{foo}` defines `\foo`), so an author's macro `foo`
and environment `foo`, two definitions in FLM, reach the style file as `\flmUDfoo` and
`flmUEfoo`. No name that the library's own LaTeX writes begins with one of the prefixes of
recomposed definitions (`flmUD`, `flmUE`, `flmUX`, and `flmUT` and `flmUM` below), so a
recomposed definition never collides with the style file's own commands.

The renaming makes an author's definition safe: under a name of its own, it cannot replace
a command that LaTeX, a package, the author's class or the style file relies on. The render
option `bare_names` lists the names written exactly as the document gives them, for an
author who *wants* LaTeX's own command of that name replaced in the whole document
([Options that make you responsible for a LaTeX
behavior](#options-that-make-you-responsible-for-a-latex-behavior)). A definition listed
there is written unconditionally (`\long\def` for a macro, `\DeclareDocumentEnvironment` for
an environment), because a guard would prevent the replacement the author asked for.
Whether a definition overrides another FLM element plays no part in the renaming: an
author's definition of `emph` is renamed like every other.

#### Per-mode definitions

FLM lets an author declare one name twice, once for text mode and once for mathematics mode,
with different templates and even different arguments, because the two never apply in the
same place. LaTeX has one command per name, so the style file holds two definitions under
two names, the **per-mode definitions**:

```latex
% \flmUTDvec (the document's \vec, in text): defined unless the name is already taken, …
\providecommand\flmUTDvec[1]{\textbf{#1}}
% \flmUMDvec (the document's \vec, in mathematics): defined unless the name is already taken, …
\providecommand\flmUMDvec[2]{\mathbf{#2}#1}
```

The comment line above each definition gives the author's own name and the mode. The name
is the ordinary one with the mode's letter after `flmU` (`T` for text, `M` for mathematics):
`\vec` gives `\flmUTDvec` and `\flmUMDvec`, a per-mode `\~` gives `\flmUTXhE` and `\flmUMXhE`,
and a per-mode environment `note` gives `flmUTEnote` and `flmUMEnote`. These names never
collide with the author's own, because the letter after `flmU` says which kind of name it
is: a macro named `Tvec` is `\flmUDTvec`, not `\flmUTDvec`.

Every call is written under the name of the definition that applies where it is written
(`\flmUTDvec{v}` in text, `\flmUMDvec{}{w}` inside `\(…\)`), so LaTeX does not choose. A
LaTeX command could test whether it is inside mathematics, but it tests when it is used, and
a heading's title is used twice: where it is written, and when LaTeX reads it back from the
table of contents. The second use is outside mathematics regardless of the title, so a title
holding `\(\vec{w}\)` would come back as the text version. FLM knows the mode where it reads
the document, so it chooses there.

A per-mode name cannot be listed in `bare_names`, since two definitions cannot share one
command: the entry is refused with `flm.latex-recomposed.definition-name-refused`, and both
definitions are written under their prefixes. Listing a per-mode name in `robust_names`
makes both definitions robust.

#### Conditionals

A template may ask about its own arguments with the fourteen conditional macros of the
template language (`\IfNoValueTF`, `\IfValueTF`, `\IfBooleanTF`, `\IfProvidedTF`,
`\ifblank`, `\notblank` and their one-branch forms). Every one of them can be written, and
which of three forms each takes is decided once, when the style file is written:

| what the template asks | what the style file holds |
|---|---|
| anything whose answer cannot change from one use site to the next: a question about a mandatory argument, which is always written; a question about an argument with a declared default, which always has a value; a test whose argument is not a single placeholder | the branch the answer selects, and nothing else |
| whether an argument was *written*: `\IfProvidedTF` and `\IfBooleanTF` about any argument, and `\IfNoValueTF` or `\IfValueTF` about one without a declared default, where the two questions are the same | LaTeX's `\IfBooleanTF` over that argument's flag, the branches exchanged where the question is the opposite one |
| `\ifblank`, `\notblank` about an argument's content | `\flmIfBlankTF` over the argument's value, a command the style file defines for this purpose; `\notblank` is the same test with its branches exchanged |

A `*` marker is an argument like any other here: its value group holds the `*` the writer
wrote (FLM substitutes that `*` for a placeholder of the argument), and a question about it reads
its flag.

In a definition whose arguments are packed into one group, the same answers are written
with the style file's own tests: `\flmIfArgTF{<n>}{#1}{…}{…}` for a flag and
`\flmIfArgBlankTF{<n>}{#1}{…}{…}` for a blank test, both referring to the argument by its
number in the group. LaTeX's `\IfBooleanTF` and the plain blank test cannot be given one
part of a group: written that way, a test either stops the compilation or selects the wrong
branch. A blank test over several arguments lists them together:
`\flmIfArgBlankTF{1,6}{#1}{…}{…}`. `\flmIfArgBlankTF` calls `\flmIfBlankTF` on what it reads,
so the two answer the same way; the style file of a document with a packed definition
therefore holds `\flmIfBlankTF` too, whether or not a definition asks a blank test.

**The blank test means the same on both sides.** FLM calls content blank when it is empty
or holds nothing but comments, and counts whitespace as content, so a single space is not
blank. `\flmIfBlankTF` is true only for an argument holding no token at all. These agree
because of how TeX reads an argument: a comment character discards the rest of the line and
the line end with it, and the spaces at the start of the next line are skipped, so a comment
never becomes a token. Both sides answer the same way for each of these six cases: nothing
(blank); one space (not blank); a comment alone (blank); a comment followed by text (not
blank); a comment followed by whitespace (blank, since FLM counts the whitespace after a
comment as part of the comment); whitespace followed by a comment (not blank).

#### Definitions that use themselves

A definition that uses itself is written as a LaTeX definition that calls itself: asking for
the recomposed LaTeX is asking LaTeX to run the expansion of the definitions. A recursion
that stops (one that tests an argument and stops when it is empty, for instance) stops in
both FLM and LaTeX and prints the same thing. A recursion that never stops is a broken
document in both, reported differently: where the document is expanded, FLM stops after a
fixed number of nested expansions (32 unless configured otherwise) and reports
`flm.userdefs.recursion-limit` with the definition's name, while LaTeX stops with "TeX
capacity exceeded" or never finishes. A document rendered only to LaTeX is not expanded, so
the LaTeX run is where such a definition fails. To learn *which* definition it is, run the
[`check`](crate::stdcore::CHECK_WORKFLOW) workflow beside the LaTeX render: the check
expands the document and reports the definition. The render itself does not require this.

#### Definitions used inside mathematics

Mathematics is recomposed from its nodes, not copied: the rules of `math-passthrough` write every
mathematics region (`\(…\)`, `\[…\]`, each equation line of a display-math environment)
through the mathematics writer of
[`userdefs::kept`](crate::stdcore::features::userdefs::kept::write_math_run), byte for byte
except where a user definition is used. Nothing inside mathematics is lowered through the
rules.

- A use site of a definition that is written as a LaTeX definition is written as its
  canonical call under its recomposed name, each group's content through the same writer
  (so a use site nested in an argument or a group is handled too). The definition reaches
  the style file as from a use site in text, once per name: a definition that applies in
  mathematics mode alone (`\calN` for `\mathcal{N}`), and a definition used both in text and
  inside mathematics, are each defined once.
- A use site of any other definition is written as its expansion.

#### Definitions inside moving arguments

A heading's title, a caption and an index entry are *moving arguments*: LaTeX writes them to
a file of its own (the table of contents, the list of figures, the auxiliary file) and reads
them back on a later run. A call of a recomposed definition works there: the definition is
loaded from the preamble, so it is defined when the file is read back. So
`\section{Greeting \hello{World}}` sets the title, puts the call into the table of contents,
and lets a reference to the heading resolve; where the document class does not number the
heading, the reference prints its title, with the definition's text in it. In a **PDF
bookmark**, where `hyperref` expands the title into a string instead of typesetting it, an
expandable definition is expanded like any other text: the bookmark holds
`Greeting Hello, World!`, and the log holds no warning. A case change expands the call too,
so `\MakeUppercase` over `\hello{World}` prints `HELLO, WORLD!`: the definition's own text is
uppercased with the argument. To keep a definition's words unchanged under a case change,
list it in `robust_names` (above).

#### Packages a definition needs

A definition never declares the LaTeX packages it needs. Where a definition's LaTeX uses a
command that a package supplies, request that package once, for the whole document, with the
render option `extra_packages` ([LaTeX render options](#latex-render-options)).

#### Selecting the rule that instantiates at lowering

For a definition with the expansion policy `per-plan`, the `userdefs` feature registers
three lowering rules ([`userdefs::kept`](crate::stdcore::features::userdefs::kept)):
`expand-in-tree`, the default on every backend; `emit-call`, the default on this backend
alone; and `instantiate-at-lowering`, which is never a default. A plan request can choose
`instantiate-at-lowering` by name
([`LoweringSelection::prefer`](crate::core::plan::LoweringSelection::prefer)). That rule
writes every use site as its expansion, as for a definition that cannot be written as a
LaTeX definition. On this backend the choice can differ from one definition to the next,
because `emit-call` and `instantiate-at-lowering` both require the capability alternative
`flm.userdefs.kept`. On `text`, choosing it for one definition and not for another is
refused when the plan is built: the other definitions keep `expand-in-tree`, which requires
the expanded alternative, and the two alternatives of `flm.userdefs` exclude each other.

### Verbatim content in the recomposed LaTeX

Inline verbatim content (the three macros `\verbtext`, `\verbcode`, `\verba` and the
environment `{verbatimtext}`) is written through the `fvextra` package's `\EscVerb`
([`verbatim::latex`](crate::stdcore::features::verbatim::latex)). `\EscVerb` reads an
ordinary braced argument in which every printable ASCII character that is not a letter or a
digit is preceded by a backslash (except `@`, which `fvextra` refuses escaped), so the
content works inside a heading's title or a caption. The verbatim kind's formatting hook
(`\flmFmtVRBverbtext`, `\flmFmtVRBverbcode`, `\flmFmtVRBverba`) is the LaTeX hook a user
redefines to change how that verbatim kind is set.

A `{verbatimcode}` block is written as the `flmVerbatimCode` environment, its language tag as
the optional argument and its content on lines of its own. The brackets are always written,
empty where the block gives no language, because LaTeX's lookahead for an absent optional
argument would consume the line end after which the verbatim content must begin. Where such
a block reaches LaTeX inside a group LaTeX reads as tokens, it is written as one call of
`\flmVerbatimBlock` instead, which rebuilds the block where it is ([User definitions in the
recomposed LaTeX](#user-definitions-in-the-recomposed-latex), "A code block works wherever
it is placed").

The family's verbatim definition piece, which every verbatim rule requires, defines the
formatting hooks, the environments and `\flmVerbatimBlock`. `\EscVerb` itself comes from
`fvextra` 1.5 (of 2022) or later; the definition piece requires the package and does not test its
version.

### The stages, the tables and the artifact set

Besides the validation stages that every rendering workflow runs (the workflow demands
[`flm.validated`](crate::stdcore::VALIDATED)), a plan for this backend schedules:

- `flm.blocks.raw`, the stage that marks the trees as raw;
- `flm.refs.safe-labels`, which allocates the safe labels;
- the backend's five stages: `flm.latex-recomposed.lower`, `flm.latex-recomposed.pieces`
  (the pieces the render options list), `flm.latex-recomposed.emit`,
  `flm.latex-recomposed.regions` and `flm.latex-recomposed.final`;
- where the registry holds a `per-plan` user definition, the stage `flm.userdefs.keep`,
  which marks the use sites as kept; where it holds an `always` definition, the rewrite
  `flm.userdefs.expand-always`.

**The emit stage**
([`LatexRecomposedEmitStage`](crate::stdcore::backends::latex_recomposed::LatexRecomposedEmitStage))
runs once per tree. It walks the tree's lowered form, writes every byte through the
family's typed sink, and writes three tables:

- `flm.output.latex-recomposed`: per tree, a
  [`LatexFragment`](crate::stdcore::backends::latex_recomposed::LatexFragment) holding the
  recomposed LaTeX, the characters it declares, the preamble chunks their spellings need,
  and the requirement identifiers it needs;
- `flm.latex-recomposed.packages`: the package identifiers the trees require;
- `flm.latex-recomposed.preamble`: the definition pieces the trees require, and the user
  definitions their definition nodes recomposed.

**The regions stage**
([`LatexRecomposedRegionsStage`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRegionsStage))
runs once per document. It emits the pieces the render options list, resolves every
requirement identifier of the trees and of the pieces through the family's two service
slots, assembles the style file into the backend's own table
`flm.output.latex-recomposed.style`, and writes the output regions into
`flm.output.latex-recomposed.regions`: the body, each piece, and the style file's package
name. The body is the recomposed LaTeX of every tree in document order, each followed by the
comment line `%%`; a tree placed inside another tree is written at its place and not again.
The comment line ends a tree's last line whether or not the tree ended with a newline, so two
trees never run into one line, and no line ending is added that the author did not write.

**The final stage**
([`LatexRecomposedFinalStage`](crate::stdcore::backends::latex_recomposed::LatexRecomposedFinalStage))
is the backend's final stage ([output regions and the final
stage](crate::guide::concepts_overview#output-regions-and-the-final-stage)). It assembles
the entry artifact from the output regions and writes the artifact set
`flm.output.latex-recomposed.document`, the table of the `latex-recomposed` output. It places
no piece. The artifact set holds:

- `latex-recomposed:document`, the entry artifact, with the extension `tex`: a first line
  `% flm: style file NAME.sty`, so that a reader knows which file to load, then the
  body. Where there is no style file (below), that first line is not written, and the
  recomposed LaTeX begins with the body.
- `latex-recomposed:<stem>.<hash>`, the **style file**, of media type
  `application/x-latex` and extension `sty`. Its stem is the render option `style_stem`
  (`flmrecomposed` by default), and its hash is computed from the file's content (without
  its `\ProvidesPackage` line), so two documents' style files never clash in one folder,
  and identical documents share one;
  [`style_artifact`](crate::stdcore::backends::latex_recomposed::stages::style_artifact)
  finds it in a set. The regions stage asks the host's naming service for the file's
  path and gives the artifact that path as its fixed path: the `flm` program names it
  after the document's file, `paper-flmrecomposed.1a2b3c4d.sty` for `-o paper.tex` and
  `index-flmrecomposed.1a2b3c4d.sty` in a folder. The LaTeX package name, `NAME`, is that
  path without `.sty`: the file begins with `\ProvidesPackage{NAME}`, the first line of the
  document gives `NAME.sty`, and the output region `style-package` holds `NAME`, from which
  a compose template writes `\usepackage{NAME}`. Without a naming service (a registry built
  without a host, as in the example below), the path is `<stem>.<hash>.sty`. LaTeX loads
  the file from the document's folder by its name, so a path from the naming service that
  is not a bare file name ending in `.sty` is the Error
  [`StylePathRefused`](crate::stdcore::backends::latex_recomposed::stages::StylePathRefused)
  (`flm.latex-recomposed.style-path`), and the file is then named as without a naming
  service.
- under the strategy `copy` for this output, `graphics:<stem>-<digest>`, one copy of each
  image file that the document shows, which `\flmIncludeGraphics` refers to by the address
  that the host's naming service gives ([images and how an output reaches
  them](#images-and-how-an-output-reaches-them)). The copies are written into the output's
  artifact table `flm.output.latex-recomposed.artifacts` by the `graphics` feature's publish
  stage. An image shown only in content that the output synthesizes at a use site (a user
  definition whose image path holds a placeholder, expanded where it is used) gets the
  address of its copy but no copy.

**What the style file holds, in order.** Each block below is written only where it holds
something, and each starts with a comment line that states what the block holds:

1. the author's own LaTeX for the top of the file (the part `before_packages` of the render
   option `extra_preamble`);
2. the package requests: one `\RequirePackage` line per LaTeX package, inside a test of the
   engine where the line differs by engine;
3. the command that states each engine's default font encoding, where the package block
   loads `fontenc` with that engine's encoding;
4. the author's own LaTeX for after the packages (`after_packages`);
5. the `\newunicodechar` declarations of the `Declare` mode;
6. the definition pieces that the rules and the declared characters required, in the order
   the registry holds them, each with the definition pieces it needs in turn;
7. the definitions that a feature writes from its configuration (the `defterm` environment,
   the theorem declarations, the annotators' commands, the reference types of float types);
8. the recomposed user definitions, in order of first use;
9. the author's own LaTeX for the end of the file (`at_end`), followed by `\endinput`.

Each package request and each definition piece is preceded by a comment line giving the
identifiers it was resolved from, so that a reader can find what asked for a line. Every
definition piece is guarded or is a LaTeX hook, so a definition of one of the family's
commands made in the class file or the preamble before loading is the one used, and
`\RenewDocumentCommand` after loading replaces it. A user definition written under its
prefix is guarded the same way, and `\renewcommand` after loading replaces it
(`\RenewDocumentCommand` where the definition is robust). A LaTeX file that loads the style
file can patch it after loading; where the patch is the same for every render of the
document, the render option `extra_preamble` writes it into the style file instead.

**One `\RequirePackage` line per package.** LaTeX loads a package once: a second
`\RequirePackage` for a package already loaded with other options is an option clash that
stops the compilation. So every request for one package (from the library's rules,
definition pieces and character spellings, and from the render option `extra_packages`) is
merged into a single line whose options are the union of the requests' options, each option
once, in the order first encountered
([`merge_requests`](crate::stdcore::backends::latex::packages::merge_requests)); the comment
line above it gives every identifier behind it, separated by `, `. An option list is read as
LaTeX reads one: split at its top-level commas (a comma inside braces belongs to the value
it is in), each option trimmed, and the option's key is what precedes the first `=`. Where
two requests give one key two different values (`papersize=a4` against
`papersize=letter`, or a flag `draft` against `draft=false`), no line can carry both: the
run reports the Error `flm.latex-recomposed.package-options-conflict`, with the package,
the key and the two options, and the style file does not request that package at all. A
package the author requests in their own preamble is not part of that merge, and LaTeX's
own rule applies to it: for most packages, a request with an option the package's first
request lacked is an option clash, so give both requests the same options.

**A line can differ by engine.** The library's own request for a package can differ between
LaTeX engines: `fontenc` loads a Cyrillic font encoding beside `T1` under pdfLaTeX, and
beside `TU` under LuaLaTeX and XeLaTeX, which set text in Unicode fonts. A request from
`extra_packages` applies to every engine. The merge runs per engine. A package whose line is
the same under every engine is written once; where the lines differ, the package block
requests the package `iftex` first and writes the lines inside its test of the engine,
`\iftutex … \else … \fi` (the first branch for LuaLaTeX and XeLaTeX, the second for
pdfLaTeX and any other engine; `\ifluatex` and `\ifxetex` nested where all three differ),
each line under its own comment line. The document is the same regardless of the engine
that compiles it, and there is no engine option: the engine is chosen where the document is
compiled.

**The default font encoding is stated.** `fontenc` makes the *last* encoding of its option
list the encoding the document's text is set in, and a merged option list has no meaningful
last option. So where the package block loads `fontenc` with an engine's own encoding (`T1`
under pdfLaTeX, `TU` under LuaLaTeX and XeLaTeX), the style file writes
`\renewcommand{\encodingdefault}{T1}` or `\renewcommand{\encodingdefault}{TU}` for that
engine right after the block, in a test of the engine of its own. Latin text is then set in
the engine's own encoding, regardless of the order of the merged encodings, and a Cyrillic
letter carries its own `\fontencoding` switch. Nothing is written for an engine under which
the block loads no `fontenc`, or loads it without that engine's encoding: the style file
states a default only for a package it requests itself.

**When the style file is written.** Under
[`StyleFile::WhenNeeded`](crate::stdcore::backends::latex_recomposed::StyleFile::WhenNeeded),
the default of the render option `style_file`, a document that requires no package,
declares no character, needs no definition piece and recomposes no user definition gets no
style file: the artifact set holds the recomposed LaTeX alone. Under
[`Always`](crate::stdcore::backends::latex_recomposed::StyleFile::Always) the file is written
even when it holds nothing, for a pipeline that expects one beside every recomposed LaTeX.
In practice:

- with the LaTeX hooks on, every heading, label and reference requires a definition piece,
  so a document holding one of them always has a style file; a heading alone requests a
  LaTeX package too, because the heading's definition piece requires the display-text definition piece and the
  `zref-clever` reference type that goes with it, so a document with a heading and no
  reference still requests `zref-clever`;
- a render that sets the render option `extra_packages` or `extra_preamble` always gets a
  style file, regardless of `style_file`, because what those options ask for is written in that file
  only.

A requirement identifier for which no feature contributed a definition is reported as
`flm.latex-recomposed.unknown-requirement`, and its line is left out.

**The LaTeX the style file relies on.** The definition pieces are written for a LaTeX
system of 2022 or later: they use `\NewDocumentCommand` and its family, the kernel hooks and
`\@currentcounter`. The LaTeX packages that the default recomposed LaTeX requests need
nothing newer: `zref-clever` (2022) for `\zcref` and `fvextra` 1.5 (2022) for `\EscVerb`
date from the same year, and `enumitem`, for a list's own label template, is older. No
definition piece tests what is installed (whether a command exists, which kernel is loaded),
so one recomposed LaTeX behaves the same way wherever it is compiled. The only tests of the
engine in the style file are the package block's, for a package whose line differs by
engine, and the one that states the default font encoding; they change which fonts set the
text, not what the document says. The only question the style file asks is about the
document's own preamble (whether it loaded the `hyperref` package), in two places:
`\flmHyperref` links the author's words where the package is loaded and prints them where it
is not ([The LaTeX hooks](#the-latex-hooks)), and the style file tells `hyperref`, where it is loaded,
what verbatim content and such a reference contribute to a PDF bookmark. The preamble is
part of the document, so the answer is the same on every TeX installation. The only text in
the style file that this library did not write is the three parts of `extra_preamble`.

### LaTeX render options

[`LatexRecomposedRenderOptions`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions),
[`RefsRecomposeOptions`](crate::stdcore::features::refs::RefsRecomposeOptions) and
[`MathRecomposeOptions`](crate::stdcore::features::math::MathRecomposeOptions) are render
options ([`Bind::Render`](crate::model::options::Bind::Render)): their values are supplied
when the plan is built, through [`Overrides`](crate::core::plan::Overrides), and each is
part of the plan's identity.

| option | held by | default | effect |
|---|---|---|---|
| `unicode` | `LatexRecomposedRenderOptions` | `Declare` | the Unicode mode ([Escaping and the Unicode modes](#escaping-and-the-unicode-modes)) |
| `unicode_overrides` | `LatexRecomposedRenderOptions` | none | a spelling of your own for a character, in place of the table's |
| `drop_comments` | `LatexRecomposedRenderOptions` | off | a comment is written as written, or left out together with the whitespace that followed it |
| `skip_packages` | `LatexRecomposedRenderOptions` | none | LaTeX packages this library never requests, except `iftex`, which a test of the engine in the package block needs; a request of yours for one through `extra_packages` is still written |
| `extra_packages` | `LatexRecomposedRenderOptions` | none | LaTeX packages the style file always requests, each with its options, merged with the document's own requests |
| `extra_preamble` | `LatexRecomposedRenderOptions` | empty | three parts of LaTeX written as given into the style file: before the packages, after them, at the end |
| `bare_names` | `LatexRecomposedRenderOptions` | none | user definitions written under the document's own name, not under their prefix |
| `robust_names` | `LatexRecomposedRenderOptions` | none | user definitions written as robust commands, not as expandable macros |
| `style_stem` | `LatexRecomposedRenderOptions` | `flmrecomposed` | the style file's own name, before its hash; the host's naming service may add a prefix (`paper-flmrecomposed.1a2b3c4d.sty`) |
| `style_file` | `LatexRecomposedRenderOptions` | `WhenNeeded` | whether a style file that holds nothing is written |
| `heading_hook` | `LatexRecomposedRenderOptions` | on | headings written through `\flmHeading` |
| `label_hooks` | `LatexRecomposedRenderOptions` | on | labels and references written through `\flmLabels`, `\flmRef` and `\flmHyperref` |
| `drop_math_labels_without_refs` | `LatexRecomposedRenderOptions` | off | in a registry without the `refs` feature, the labels of an equation line not written at all; no effect where that feature is loaded |
| `resolved_proof_refs` | `LatexRecomposedRenderOptions` | off | a proof's back-reference written as written, over `phfthm`, or resolved into the proof title hook, `\flmProofTitle{\flmRef{ref1}}`, over `amsthm` alone |
| `safe_label_types` | `RefsRecomposeOptions` | none | label types written as written instead of under a safe label |
| `protect` | `RefsRecomposeOptions` | off | `\NoCaseChange{\protect …}` around every reference, of either kind |
| `surround_display_math` | `MathRecomposeOptions` | empty | LaTeX written before and after every displayed formula (every display-math environment and every `\[…\]` group) |
| `emit_flm_math_environment_macro` | `MathRecomposeOptions` | off | every display-math environment written as `\flmMathEnvironment` |

The `headings` feature adds one more render option,
[`HeadingsLatexOptions`](crate::stdcore::features::headings::latex::HeadingsLatexOptions),
which gives the LaTeX sectioning command each heading kind is written as. Three other
features' render options also change this output:
[`AnnotationsRenderOptions::hidden`](crate::stdcore::features::annotations::AnnotationsRenderOptions::hidden)
([Annotations in the recomposed LaTeX](#annotations-in-the-recomposed-latex)), the
`graphics` feature's strategy for this output
([`GraphicsRenderOptions`](crate::stdcore::features::graphics::GraphicsRenderOptions);
[images and how an output reaches them](#images-and-how-an-output-reaches-them)), and
[`PiecesRenderOptions`](crate::stdcore::backends::pieces::PiecesRenderOptions) ([Pieces in
the recomposed LaTeX](#pieces-in-the-recomposed-latex)).

#### What the default options aim for

With every option at its default, the aim is that the recomposed LaTeX and its style file
compile as they are, and that what LaTeX then sets is what the document says: references
that point at the right thing, verbatim content that survives a moving argument, every
character printed. The library's own tests compile its reference outputs with `pdflatex`
and with `lualatex`, each with and without `hyperref`, on the `article` class of one TeX Live
installation, and require that the run finishes with no error and no undefined reference.
This is an aim and not a guarantee: the classes and packages a user may load are unbounded.
Two things are outside the aim even with the default options, both written unchanged:

- a document whose own mathematics uses a command that its preamble does not provide
  ([Mathematics in the recomposed LaTeX](#mathematics-in-the-recomposed-latex); request the
  package with `extra_packages`);
- a `\ref` option that the hook has no translation for and that `zref-clever` refuses.

Every character that the Unicode-to-LaTeX table spells is inside the aim: each entry of
the table compiles and prints its glyph with the preamble the table asks for. A character
the table cannot spell is outside the aim: it is written unchanged under `Declare`, or as its
code point under `Replace`, and reported as the Warning
`flm.latex-recomposed.unknown-character` ([untechxt's
table](crate::untechxt::builtin::default_table) lists what it does not spell, and why).

#### Options that make you responsible for a LaTeX behavior

Some options hand a part of LaTeX's behavior to you. They are marked **You own this** below
and in their reference documentation, with what you must then provide:

- **`heading_hook` off. You own this.** Nothing gives an unnumbered heading a display text,
  so a reference to one prints the number of the last thing LaTeX numbered, with no warning.
  You must provide either a document with no unnumbered heading among its reference
  targets, or a mechanism of your own that gives them one.
- **`label_hooks` off. You own this.** Labels are written bare, and several labels on one
  line of a display-math environment are left to LaTeX: `amsmath` raises an error, keeps the
  last label of the line and loses the others, so the run fails and references to the lost
  labels print `??`. You must provide either one label per line, or an aliasing of your
  own. A reference's options also reach `\zcref` untranslated, so you must write options in
  `zref-clever`'s vocabulary: `\ref[noprefix]{…}` stops the compilation, where the hook
  would have made it `noname`. A `\hyperref` is written as the `hyperref` package's own
  command, with the package requested by the style file, so you must provide a document that
  can load that package from the style file, beside the packages you load yourself.
- **`drop_math_labels_without_refs` on. You own this**, in a registry that does not hold the
  references feature; where that feature is loaded the option does nothing and the output is
  unchanged. The labels written on the lines of a display-math environment are then left out
  of the recomposed LaTeX entirely, so a label of the document reaches no output and nothing
  marks the line. You must give anything that refers to those lines (LaTeX you write around
  the recomposed document, or a tool that reads the file) a way of finding them that does
  not use a `\label`. The option affects mathematics only: a heading's labels are
  written as without it, and no such option exists for headings.
- **`skip_packages`. You own this.** The style file stops requesting the packages you list,
  and the recomposed LaTeX then needs your preamble or your class to load each of them: you
  must provide those `\usepackage` lines, without which the document does not compile. The
  style file's own request for `iftex`, written wherever its package block writes a test of
  the engine, is not stopped, because the test needs that package. Listing `fontenc` also
  stops the command that states the default font encoding, so you must also set the encoding
  the document's text is set in (`\renewcommand{\encodingdefault}{T1}` under pdfLaTeX and
  `\renewcommand{\encodingdefault}{TU}` under LuaLaTeX and XeLaTeX is what the library would
  write), unless one of your `extra_packages` requests loads `fontenc` with an engine's own
  encoding: the style file then writes that command for that engine. The option stops this
  library's requests, not yours: a package you list here and also request through
  `extra_packages` is requested anyway, on a line of its own, with your options and no
  others. Listing a package here and requesting it there is how you load a package with an
  option list that this library's requests cannot add to. Your own requests for that package
  are still merged with each other, by the same rule as everywhere else, so the style file
  holds one line for it; two of your requests that give one option key two different values
  are the same Error as anywhere else, and leave the package unrequested.
- **`extra_packages`. You own this.** Every entry is a
  [`LatexPackageDef`](crate::stdcore::backends::latex::LatexPackageDef), the type of every
  package request: a package name with the options it is loaded with, built as
  `LatexPackageDef::new("amsmath", "why this document needs it").options(["fleqn"])`. Two of
  its parts are not written into the style file: the description the constructor asks for,
  which documents the request for whoever reads the requests a registry holds, and the
  identifier (`flm.latex.pkg.<package>` unless `.id(…)` sets another), which only the comment
  line above the request gives. A request written in this option applies to every engine.
  Your entries join the requests the document's own content made, after them and in the
  order you list them; your options, their order and their number are kept, and only the
  separators are normalized, so `.options(["a4paper, margin=2cm"])` reaches the file as
  `[a4paper,margin=2cm]`. This is where you state what the parts of the document that this
  library writes unchanged need in order to compile, above all your own mathematics. Nothing
  is checked: not that the package exists, not that the options are its own, not that loading
  it agrees with what your preamble or class loads. Requesting a package that this library
  requests itself adds your options to its line (`amsmath` with `fleqn` beside the library's
  request), and requesting it twice adds them once. One option key given two different values
  (`papersize=a4` against `papersize=letter`) cannot be merged: it is an Error, and the
  package is not requested. Setting this option makes the style file exist regardless of
  `style_file`.
- **`extra_preamble`. You own this.** Three parts of text, each written into the style file
  **byte for byte** under a comment line of its own, at the place its name gives:
  - `before_packages`, between the file's header and the package block. This is where
    `\PassOptionsToPackage` still reaches a package the block requests, and where a package
    that must be loaded before the others is loaded.
  - `after_packages`, directly below the package block and the default-encoding command,
    above everything this library defines. A LaTeX hook you define here is the one the
    document uses: every definition this library makes is guarded
    (`\ProvideDocumentCommand` and the related commands) and finds the name taken, so nothing
    is patched.
  - `at_end`, after the character declarations, the definition pieces and the user
    definitions, and before the `\endinput` that ends the file, so it can use any of them:
    redefine a LaTeX hook, patch a definition piece, define a command your own mathematics
    uses.

  Nothing in a part is escaped, quoted, parsed or checked: a stray backslash or an unclosed
  group is yours to find, and so is a definition of a name that this library's output relies
  on. Where a part does not end in a line break, one is added, so that the next line begins a
  line of its own; your bytes are unchanged. An empty part writes nothing at all, and setting
  any of the three makes the style file exist regardless of `style_file`.
- **`bare_names`. You own this.** A user definition whose name you list is written into the
  style file under that name, instead of under the prefix that every recomposed definition
  otherwise takes, so it becomes the definition of that LaTeX command for the whole
  document: in your own text, in what a package writes, and in what your class writes. That
  is what the option is for, and nothing warns you where it changes a result. You must
  provide a definition that can replace the command it replaces. Give the name without a
  backslash (`emph`, not `\emph`); one entry covers a macro and an environment of that name
  alike. Three kinds of entry are refused, each with the Error
  `flm.latex-recomposed.definition-name-refused`, which gives the reason:
  - a name beginning with `flm`, the name space of this library's own LaTeX; the definition is
    then renamed as usual;
  - a name declared once per parsing mode ([Per-mode definitions](#per-mode-definitions));
    both definitions are written under their prefixes;
  - an entry under which two of your own definitions would become one LaTeX command, because
    `\DeclareDocumentEnvironment{foo}` defines the commands `\foo` and `\endfoo` itself: a
    name you declared as both a macro and an environment, or a macro named `endfoo` beside an
    environment named `foo` where you listed both. Under the prefixes these never meet, and
    listing the names brings them together; the definitions are still written as
    the option asks, so remove the entry or rename one of the two definitions.
- **`robust_names`. You own this.** A user definition whose name you list is written as a
  robust command instead of the expandable macro it otherwise becomes ([Expandable
  definitions, and robust ones on
  request](#expandable-definitions-and-robust-ones-on-request)). Give the name without a
  backslash (`hello`, not `\hello`); one entry covers every definition of that name. An entry
  that has nothing to act on is accepted and changes nothing, so listing every name you own is
  safe. In return, a heading whose title uses that definition loses the definition's words
  from its PDF bookmark, with "Token not allowed in a PDF string" in the log and nothing else,
  and nothing warns you where a bookmark, an `\edef` or another place that expands instead of
  typesetting needed the words. You must judge that the definition's body must not be
  expanded early, or that its words must survive a case change. The only entry refused is a
  name beginning with `flm`, reported as `flm.latex-recomposed.definition-name-refused`; the
  definition is then written as an expandable macro.
- **`unicode_overrides`. You own this.** The spelling is yours and is written in place of the
  table's. You must provide a spelling that is valid wherever the character occurs, in text
  and in mathematics, and, in the preamble of the LaTeX file that loads the style file (or in
  `extra_packages` and `extra_preamble`), everything that spelling needs in order to compile: an
  override adds nothing to the style file. The library checks neither.
- **`safe_label_types` set to `Raw`. You own this.** Labels of that label type are written as
  the document wrote them, instead of under a generated safe label. You must provide labels
  LaTeX can carry: a label holding a character LaTeX treats specially cannot be read back
  from the auxiliary file, which is the reason safe labels exist.
- **`protect` on. You own this** only in that the wrapper it writes, `\NoCaseChange`, belongs
  to the `textcase` package. The style file requests `textcase`, so the option needs nothing
  from you unless you also list `textcase` in `skip_packages`.

`unicode`, `drop_comments`, `style_stem` and `style_file` hand nothing to you: whichever way
they are set, the library is responsible for what it writes.

### One document rendered as plain text and as LaTeX

The same document renders as plain text and as LaTeX from the same registry, with one plan
for each output. The two plans cannot be one: the plain-text backend lowers paragraphs, so
its plan builds the block structure of the trees, while this backend's rules require the raw
tree, and the two states of the tree are exclusive alternatives of the capability
`flm.blocks`. A plan request that demands both outputs is refused when the plan is built,
with [`PlanError::Unsatisfiable`](crate::core::plan::PlanError::Unsatisfiable):

```rust
use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan, PlanError, PlanRequest};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::stdcore::backends::latex_recomposed;
use flm_core::stdcore::backends::text::{self, TEXT_DOCUMENT, TEXT_OUTPUT};
use flm_core::stdcore::{self, StdcoreConfig};

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;

let source = Arc::new(
    FlmSource::new("\\section{A}\\label{sec:a}\n\nSee \\ref{sec:a}, 50\\% \\emph{more}.")
        .with_origin(FlmOrigin::new("doc.flm")),
);
let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));
let doc = Arc::new(Doc::new(content));

// The text plan: structured trees, references resolved.
let text_plan = Arc::new(Plan::for_workflow(&registry, text::WORKFLOW, &Overrides::none())?);
let mut text_run = Run::new(Arc::clone(&doc), text_plan, Externals::none())?;
text_run.execute(&RunOptions::new())?;
let text = &text_run.table(TEXT_OUTPUT)?.expect("written").text;
assert_eq!(text, "A\n=\n\nSee A, 50% more.\n");

// The LaTeX plan: the raw tree, references left to LaTeX.
let latex_plan =
    Arc::new(Plan::for_workflow(&registry, latex_recomposed::WORKFLOW, &Overrides::none())?);
let mut latex_run = Run::new(doc, latex_plan, Externals::none())?;
latex_run.execute(&RunOptions::new())?;
let set = latex_run.table(latex_recomposed::DOCUMENT)?.expect("written");
let recomposed = std::str::from_utf8(set.entry().unwrap().as_bytes().unwrap()).unwrap();
assert!(recomposed.starts_with("% flm: style file flmrecomposed."));
assert!(recomposed.ends_with(
    "\\flmHeading[toc=true]\n\\section{A}{\\flmLabels{\\label{ref1}}}\n\nSee \\flmRef{ref1}, 50\\% \\emph{more}.%%\n"
));
let style = latex_recomposed::style_artifact(&set).unwrap();
assert!(std::str::from_utf8(style.as_bytes().unwrap()).unwrap().contains("\\RequirePackage{zref-clever}"));

// One plan for both outputs does not exist.
let both = PlanRequest::new().demand(TEXT_DOCUMENT.name()).demand(latex_recomposed::DOCUMENT.name());
assert!(matches!(Plan::build(&registry, &both), Err(PlanError::Unsatisfiable { .. })));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## See also

- [Stages, capabilities, and plans](crate::guide::pipeline): how a plan is derived from the
  capabilities a caller demands, and how a run executes it.
- [The document model](crate::guide::document_model): the frozen trees that lowering reads.
- [Extending FLM](crate::guide::extending): writing a lowering rule, a presentation element
  or a backend of your own.
- [Backends, lowering, and outputs](crate::guide::concepts_overview#backends-lowering-and-outputs):
  the concept entry, with the API items in one list.
