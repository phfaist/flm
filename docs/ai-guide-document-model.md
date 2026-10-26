# AI guide: document model

Condensed reference for the document tree, elements and aspects, names, addresses,
sources and spans, fragments and documents, document order, tree states, tables and the
store, and synthesized values. Load it together with the [AI guide](crate::guide::ai_guide),
whose section [Vocabulary](crate::guide::ai_guide#vocabulary) defines the terms used
here. Full treatment: [The document model](crate::guide::document_model).

## What holds for every document

- **The parse tree is the document model.** Nothing converts it into a second
  representation. Every node carries its [element](#elements) as a field of techy's node
  extension ([`FlmNodeExt::element`](crate::lang::FlmNodeExt#structfield.element)), read
  with [`element_of`](crate::model::element::element_of) ([the document
  tree](crate::guide::concepts_overview#the-document-tree)).
- **Trees change only before they are frozen**, through the three rewrite passes of
  [`crate::lang::rewrites`], and never again. Everything computed later is kept beside
  the trees in typed [tables](#tables-and-the-store) ([frozen trees and computed
  data](crate::guide::concepts_overview#frozen-trees-and-computed-data)).
- **A rewrite reads no computed data**: the signature of
  [`RewriteStage::rewrite`](crate::core::stage::RewriteStage::rewrite) holds no view of
  the store.
- **Identity is a name or an allocation, never a Rust type**, with one exception: an
  option type, whose identity is its Rust type ([names and
  identity](crate::guide::concepts_overview#names-and-identity)).
- **A document has no structure of its own**: no front matter, no chapters, no parts.
  Its structure is computed from the headings in its content. Its metadata (title,
  authors, date) is an option value bound on the document, not part of its trees.
- **Document order is one operation**,
  [`DocContent::walk`](crate::model::doc::DocContent::walk).
- **A parse is tolerant**: a problem in the source is a diagnostic, and the tree covers
  the whole input. The only outcomes without a tree are the cases of
  [`FragmentError`](crate::lang::FragmentError).

## The tree

| item | what it is |
|---|---|
| [`Flm`](crate::lang::Flm) | the techy language that FLM's trees are built over |
| [`parse_fragment`](crate::lang::parse_fragment) | parses one source into one tree, held in a [`Fragment`](crate::model::doc::Fragment) |
| [`FlmNodeExt`](crate::lang::FlmNodeExt) | FLM's data on every node: `element`, `source_map` (where each byte of a rewritten text node comes from), `payload` (the typed data of a presentation node or of a synthesized node, [`PresPayload`](crate::lang::PresPayload)) |
| [`element_of`](crate::model::element::element_of) | the element a node carries: a field read, not a lookup |
| [`element_value`](crate::model::element::element_value) | that element's own typed data, given a node |
| [`FlmGroupType`](crate::lang::FlmGroupType) | the group classes, among them the mathematics and the verbatim group |
| [`crate::lang::structural`] | the structural elements: the elements of the node shapes no feature declares ([`StructuralElements`](crate::lang::structural::StructuralElements); [structural elements](crate::guide::concepts_overview#structural-elements)), such as [`TEXT`](crate::lang::structural::TEXT), [`VERBATIM`](crate::lang::structural::VERBATIM), [`UNKNOWN_MACRO`](crate::lang::structural::UNKNOWN_MACRO), [`UNKNOWN_ENVIRONMENT`](crate::lang::structural::UNKNOWN_ENVIRONMENT) |

## Elements

An element is the value that says what a node means: a declaration with a stable dotted
type name, identified by the element value itself (its allocation) and not by its name
([`ElementSpec`](crate::model::element::ElementSpec); [elements and
aspects](crate::guide::concepts_overview#elements-and-aspects)). Its parts:

| part | item | notes |
|---|---|---|
| type name | [`ElementSpec::name`](crate::model::element::ElementSpec#structfield.name), an [`ElementName`](crate::model::names::ElementName) | public, stable, dotted: `flm.heading`, `flm.textstyle`; several commands may share it |
| the element's own data | [`ElementSpec::value`](crate::model::element::ElementSpec#structfield.value), read with [`element_value`](crate::model::element::element_value) or [`ElementSpec::value_as`](crate::model::element::ElementSpec::value_as) | one Rust struct per element type: a heading's level, a text style |
| aspects | [`ElementSpec::aspects`](crate::model::element::ElementSpec#structfield.aspects), an [`AspectMap`](crate::model::element::AspectMap); read with [`ElementSpec::aspect`](crate::model::element::ElementSpec::aspect) | the behavior the rest of the library asks the element about |
| documentation | [`ElementSpec::docs`](crate::model::element::ElementSpec#structfield.docs) | a description for people |

- **Identity is the element value's allocation**
  ([`ElementSpec::same_value`](crate::model::element::ElementSpec::same_value)), not the
  name. A family of related commands is declared as one element type with one element
  value per command: `\section` and `\subsection` are two element values of the type
  `flm.heading`, `\emph` and `\textbf` two of `flm.textstyle`. One lowering rule then
  serves the whole family.
- An **aspect** is a piece of behavior attached to an element when the feature registers
  it and read back through a trait object; it is evaluated per occurrence of the element
  and reads no options. Any object-safe trait can be one; the map is keyed by the *trait
  object* type (`dyn MyAspect`, never the implementing struct), and no aspect trait
  needs to be declared to the registry.
- Two rules bind every aspect: the node is a parameter of every aspect method (one heading element
  declares a different reference point at every heading), and no aspect method receives
  option values (aspects are read during the rewrites and while a plan is built, before
  option values exist).

The aspects the standard features share, and the aspect that says an element is whitespace,
in [`crate::model::aspects`]:

| aspect | what it says about an occurrence |
|---|---|
| [`BlockRoleAspect`](crate::model::aspects::BlockRoleAspect) → [`BlockRole`](crate::model::aspects::BlockRole) | `Block`, `Inline(InlineSpacing)` (whether the whitespace before or after is removed, `~` included) or `BlockDecorator(Placement)` (attached to the block it occurs in). Read with [`block_role_of`](crate::model::aspects::block_role_of), which gives [`BlockRole::DEFAULT`](crate::model::aspects::BlockRole::DEFAULT) (inline) for an element without the aspect; most elements declare [`FixedBlockRole`](crate::model::aspects::FixedBlockRole) |
| [`Referenceable`](crate::model::aspects::Referenceable) → [`RefPoint`](crate::model::aspects::RefPoint) | the reference points the occurrence carries, each with the label types it accepts ([`AcceptedRefTypes`](crate::model::aspects::AcceptedRefTypes)), its labels, its anchor policy ([`AnchorPolicy`](crate::model::aspects::AnchorPolicy)) and its number source ([`NumberSource`](crate::model::aspects::NumberSource)) |
| [`Referencing`](crate::model::aspects::Referencing) → [`RefSite`](crate::model::aspects::RefSite) | the reference sites the occurrence carries |
| [`Sectioning`](crate::model::aspects::Sectioning) | the heading level (every `u32` is valid; the standard kinds use 1 to 5), whether the occurrence is listed in the table of contents, and what its entry there shows (`toc_title`; the standard headings answer the `alt=` key's copy, else the title argument) |
| [`ApartFromText`](crate::model::aspects::ApartFromText) | the content of the occurrence that is not part of the words around it (a footnote's content in a heading's title, a citation's keys); [`plain_text_of`](crate::lang::text::plain_text_of) leaves it out; read with [`apart_from_text_of`](crate::model::aspects::apart_from_text_of) |
| [`InCopy`](crate::model::aspects::InCopy) → [`CopiedAs`](crate::model::aspects::CopiedAs) | what replaces the occurrence in a copy shown a second time: `Verbatim` (the default), `Nothing`, or `StandIn` ([`StandIn`](crate::model::aspects::StandIn), a childless node of an element of the feature's own); read with [`copied_as`](crate::model::aspects::copied_as) |
| [`Whitespace`](crate::model::aspects::Whitespace) | that the occurrence is whitespace (`~`, `\ `): a marker with no method, attached as [`IsWhitespace`](crate::model::aspects::IsWhitespace). The block-structure pass treats such a *whitespace node* as whitespace-only text: removed beside an element that absorbs the whitespace on that side, at a paragraph's edges and between blocks; no paragraph content; a straight quote after it opens. A paragraph a rule writes itself over content it cuts out (a quotation's passage, attribution and lines, a `\celldata` entry) is trimmed by [`stage_trimmed`](crate::stdcore::backends::lowering::stage_trimmed), which removes the same whitespace at its edges as the pass does, whitespace nodes included. The standard features' tests for blank content count it as whitespace (`\footnote{~}` is empty), except those of text read as typed or with its markup, of which it is part (a citation key, a term: `\term{~}` is not empty). Elsewhere it stays a node of its own, never merged into text or collapsed (`a~~b` keeps two), lowered by its own rule. Read with [`is_whitespace_of`](crate::model::aspects::is_whitespace_of) (`false` without the aspect) |

Aspects defined beside the code that reads them:

| aspect | read by | what it says |
|---|---|---|
| [`Expand`](crate::lang::rewrites::userdefs::Expand) | the expand pass | how the element rewrites its own use sites |
| [`BlockStructure`](crate::lang::rewrites::blocks::BlockStructure) | the block-structure pass | how the element lays out its own content (a block-structure hook) |
| [`BlockTransparent`](crate::lang::rewrites::blocks::BlockTransparent) | the block-structure pass | the element is block-transparent: laid out as if its content were written in the content around it, cut into parts at paragraph breaks and blocks ([block-transparent elements](crate::guide::concepts_overview#block-transparent-elements)) |
| [`Decoration`](crate::stdcore::backends::pres::Decoration) | the lowering driver | a node of the element is a decoration, moved out of a presentation node's content into an attached slot, a `flm.pres.deco-mark` node left in its place ([decorations, roles and kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)) |
| [`OpaqueContent`](crate::stdcore::backends::pres::OpaqueContent) | the lowering driver | the node's content is source text, copied unchanged and written again byte for byte |
| [`ContentInOpaque`](crate::stdcore::backends::pres::ContentInOpaque) | the lowering driver | the node's content is document content even inside opaque content (`flm.pres.math-text`, the argument of `\text` in a formula) |

## Names

One [`Name`](crate::model::names::Name) type, owning its text, because a name may be
computed while the registry is built (one environment per configured theorem kind, one
table per backend); a newtype per namespace keeps a capability name from being passed
where a stage name is expected: [`ElementName`](crate::model::names::ElementName),
[`CapName`](crate::model::names::CapName), [`StageName`](crate::model::names::StageName),
[`RuleName`](crate::model::names::RuleName),
[`FeatureName`](crate::model::names::FeatureName),
[`BackendName`](crate::model::names::BackendName),
[`WorkflowName`](crate::model::names::WorkflowName),
[`OutputName`](crate::model::names::OutputName), [`RefKind`](crate::model::names::RefKind).

- The segment before the first dot is the owner
  ([`Name::owner_prefix`](crate::model::names::Name::owner_prefix)): `flm` for the
  standard features. Some namespaces are flat (a feature is `refs`, a backend is `text`);
  a dotless name is its own owner prefix.
- **Capabilities, tables and collections share one namespace**, because a table's name
  is the capability "this table is present and complete". Every other namespace is its
  own.
- Two identities are not names: an element value (its allocation) and an option type
  (its Rust type, [`OptionKey`](crate::model::options::OptionKey)).

## Addresses

| item | what it is |
|---|---|
| [`TreeId`](crate::model::addr::TreeId) | a tree of the document ([`TreeId::tree`](crate::model::addr::TreeId::tree), assigned by the document's arena) or a [synthesized value](#synthesized-values) ([`TreeId::value`](crate::model::addr::TreeId::value), numbered per stage invocation); [`TreeId::is_value`](crate::model::addr::TreeId::is_value) tells them apart |
| [`NodeAddr`](crate::model::addr::NodeAddr) | a `TreeId` and techy's node identifier: twelve bytes, `Copy`, ordered; the key of every table that stores something per node |

- A node identifier carries a tag of the tree that assigned it (techy's tree layout, the
  storage of one tree's nodes), and the tag takes part in equality, ordering and hashing: addresses from different trees never collide.
- **Iterating a map keyed by `NodeAddr` is not document order** (trees in identifier
  order, then nodes in storage order). Document order is
  [`DocContent::walk`](crate::model::doc::DocContent::walk), and in a stage
  [`StageCx::walk`](crate::core::run::StageCx::walk).
- An address of a document node is taken in a frozen tree: a rewrite produces a new tree
  with new node identifiers.

## Sources and spans

- [`FlmSource`](crate::lang::FlmSource) is text together with an
  [`FlmOrigin`](crate::lang::FlmOrigin). The caller builds it before the parse.
- An origin's **canonical path** (for a file, its path relative to the root folder the
  host chose) is at once the label a diagnostic shows, the key by which the library
  detects a file that includes itself, and the key of a host's input-resolver tables. Two
  kinds of origin have none: [`FlmOrigin::none`](crate::lang::FlmOrigin::none) (an
  in-memory snippet) and [`FlmOrigin::synthetic`](crate::lang::FlmOrigin::synthetic)
  (text the library created: a compiled template, one use of a user definition), whose
  label is shown but never used as a key.
  [`FlmOrigin::definition`](crate::lang::FlmOrigin::definition) is the synthetic origin of
  a user definition's own text.
- [`FlmSpan`](crate::lang::FlmSpan) is a byte range and its source; **every node carries
  one**, and rewrites keep them. Where a rewrite resolves text (whitespace collapsed,
  runs merged, typographic substitutions applied), the node carries a
  [`TextSourceMap`](crate::lang::TextSourceMap) saying which source range each run of its
  bytes comes from.
- [`FlmDiagnostic`](crate::lang::FlmDiagnostic) is a typed condition, a severity and a
  span ([diagnostics](crate::guide::concepts_overview#diagnostics)). A parse's
  diagnostics belong to the fragment ([`Fragment::diags`](crate::model::doc::Fragment::diags));
  the stages' diagnostics belong to the run.

## Fragments, documents and document order

A fragment is one piece of source as the parser delivered it (its tree, its source, its
parse diagnostics); a tree is one placement of a fragment's node tree in a document,
numbered by the document's arena; a document is its content (the tree arena, the
traversal list and the resolution map) and the option values bound on it, and nothing
else ([`Doc`](crate::model::doc::Doc)).

- [`Fragment`](crate::model::doc::Fragment): [`tree`](crate::model::doc::Fragment::tree),
  [`source`](crate::model::doc::Fragment::source),
  [`diags`](crate::model::doc::Fragment::diags) and
  [`layout`](crate::model::doc::Fragment::layout) (a
  [`FragmentLayout`](crate::model::doc::FragmentLayout): `Blocks`, `Inline`, or
  `InlineOrBlocks`, the default, which lays the top level out as blocks when it holds a
  paragraph break or a block and as inline content otherwise). Shared as `Arc<Fragment>`; it has no identifier, its identity is the
  pointer. It holds exactly what the parse produced, source comments included.
- [`DocContent`](crate::model::doc::DocContent) has three parts:
  1. the **tree arena**: every placement, numbered with a `TreeId` in the order added
     ([`place`](crate::model::doc::DocContent::place) adds a tree and lists it for
     traversal, returning its `TreeId`;
     [`add_tree`](crate::model::doc::DocContent::add_tree) adds it without listing it,
     for a tree reached through a placing node);
  2. the **traversal list**: the order the listed trees are walked in
     ([`traversal`](crate::model::doc::DocContent::traversal));
  3. the **resolution map**: which node places which other tree
     ([`resolve`](crate::model::doc::DocContent::resolve), which refuses a placement that
     would break the walk with a [`PlacementError`](crate::model::doc::PlacementError)).
     The library uses it for an included file that opens with a heading (a heading input,
     placed by [`assemble`](crate::model::assemble::assemble)); an embedding program may
     use it to assemble a document from several fragments.
- [`Doc::new`](crate::model::doc::Doc::new) and
  [`Doc::with_options`](crate::model::doc::Doc::with_options) build a document from its
  content and the option values bound on it.

**The walk** ([`DocContent::walk`](crate::model::doc::DocContent::walk)): the traversal
list in order, each tree entered at most once (a tree that no node places and that is not
listed is never entered); a placed tree is entered at the node that
places it, its nodes following that node immediately, before the node's own children.
Projections: [`walk_nodes`](crate::model::doc::DocContent::walk_nodes),
[`walk_trees`](crate::model::doc::DocContent::walk_trees). The operation is
[`Walk`](crate::model::doc::Walk) over a [`WalkSource`](crate::model::doc::WalkSource):
the document is the walk source of its trees as parsed, a run the walk source of its
frozen trees. In a stage: [`StageCx::walk`](crate::core::run::StageCx::walk);
[`StageCx::position`](crate::core::run::StageCx::position) to sort nodes gathered from
several trees; [`StageCx::placement_of`](crate::core::run::StageCx::placement_of);
[`StageCx::trees`](crate::core::run::StageCx::trees) lists the trees in the order the walk
enters them, which is not document order across a placed tree. Every notion of "earlier
in the document" (numbering, note order, citation order) is this one operation
([documents, fragments, and the
corpus](crate::guide::concepts_overview#documents-fragments-and-the-corpus)).

**Documents of several files**: [`assemble`](crate::model::assemble::assemble) parses a
primary file with the files it includes through `\input`, reading each through the
caller's [`InputResolver`](crate::lang::InputResolver). A content input is parsed into
the including file's tree at the command and inserted in place of it by the rewrite
`flm.input.splice`; a heading input (an included file that opens with a heading,
[`IncludedFile::heading`](crate::lang::IncludedFile::heading)) becomes a tree of its own,
placed at the command's node ([content inputs and heading
inputs](crate::lang::input#content-inputs-and-heading-inputs)).

## Tree states

A frozen tree is a document tree after the rewrite stages have run: it is never changed
again, and everything a later stage computes about it is stored in tables beside it,
keyed by node address.

| state | produced by | read with |
|---|---|---|
| as parsed | [`parse_fragment`](crate::lang::parse_fragment) | [`Fragment::tree`](crate::model::doc::Fragment::tree) |
| spliced | [`crate::lang::rewrites::input`]: the content of every included file inserted in place of its `\input` (rewrite `flm.input.splice`) | [`flm.input.spliced`](crate::stdcore::features::input::SPLICED) |
| expanded | [`crate::lang::rewrites::userdefs`]: every use site of a user definition replaced by its template (rewrites `flm.userdefs.expand-always`, `flm.userdefs.expand`) | [`flm.userdefs.always-expanded`](crate::stdcore::features::userdefs::ALWAYS_EXPANDED) (expand-always), [`flm.userdefs.expanded`](crate::stdcore::features::userdefs::EXPANDED) (expand) |
| structured | [`crate::lang::rewrites::blocks`]: paragraphs and blocks built, text resolved once (rewrite `flm.blocks.structure`) | [`flm.blocks.structured`](crate::stdcore::features::blocks::STRUCTURED) |
| frozen | the last rewrite stage the plan scheduled | [`Run::frozen_tree`](crate::core::run::Run::frozen_tree), [`StageCx::frozen_tree`](crate::core::run::StageCx::frozen_tree) |

- Each pass is a plain function on a tree, callable without a pipeline, *and* a
  [`RewriteStage`](crate::core::stage::RewriteStage). Which passes run, and in which
  order, is decided by the plan: a plan may leave the trees without block structure
  (`flm.blocks.raw`, which the lowering rules of the `latex-recomposed` backend require) or keep the use sites
  of user definitions (`flm.userdefs.kept`).
- **Every rewrite stage the plan runs publishes its output for every tree** as a
  [`TreesTable`](crate::core::stage::TreesTable) under the capability it provides, so a
  later stage can read a tree in a given state.
- A rewrite that fails on one tree fails for that whole tree: one Error diagnostic, no
  frozen form, no later stage reads it; the other trees are unaffected.
- **Verbatim regions**: a verbatim region is a run of raw text in which no markup
  character means anything, always inside a group of the verbatim class, so "is this
  verbatim?" is one typed question about a node ([verbatim
  regions](crate::guide::concepts_overview#verbatim-regions);
  [`RestrictedInterior`](crate::lang::RestrictedInterior) says what such an interior
  still recognizes).
- **Opaque regions**: an opaque region is a part of a tree that the rewrite passes carry
  over untouched (mathematics and verbatim content, any run parsed in math mode, the
  content of an opaque wrapper); the lowering copies mathematics and verbatim content
  unchanged and writes an opaque wrapper through its children ([opaque
  regions](crate::guide::concepts_overview#opaque-regions)). An opaque wrapper is a group
  with no delimiters (`FlmGroupType::OpaqueWrapper`, element `flm.opaque-wrapper`) that
  the parser places around the content of an argument or a body declared
  `with_opaque_wrapper()`, of every characters-only group, and of a proof title that
  begins with a star, so that the element reads that text as typed. Opacity belongs to
  the region, not to the spelling that opened it.

## Tables and the store

A table is typed data that one stage writes beside the frozen trees and other stages
read; its identity is its registered dotted name, never its Rust type, and that name is
also a capability ([`Table`](crate::core::data::table::Table)).

| item | what it is |
|---|---|
| [`Table`](crate::core::data::table::Table) | the trait of a table's value type. Required: [`absorb`](crate::core::data::table::Table::absorb), how two table pieces combine (a table piece is the part of one table that one stage invocation produced). Optional: [`project`](crate::core::data::table::Table::project) (the serialized form `flm dump` and tests read), [`piece_class`](crate::core::data::table::Table::piece_class) ([`PieceClass`](crate::core::data::table::PieceClass): whether a table piece states a fact about the document as a whole) |
| [`TableName`](crate::core::data::table::TableName) | typed key for a fixed name, `const`-constructible |
| [`TableHandle`](crate::core::data::table::TableHandle) | typed key for a name computed while the registry is built (`flm.lowered.<backend>`) |
| [`SingletonTable`](crate::core::data::table::SingletonTable) | a value type with exactly one table |
| [`TableRef`](crate::core::data::table::TableRef) | a table's name with its value type's identity, type-erased; stage declarations, plans and messages carry it |

One Rust type may back many named tables
([`LoweredTrees`](crate::stdcore::backends::lowering::LoweredTrees) holds one table per
backend; [`TreesTable`](crate::core::stage::TreesTable) one per rewrite stage).

The store is the run's collection of computed tables: one writer per table, a declared
write always installs a value, and every read and write is checked against the stage's
declarations at the access site ([`Computed`](crate::core::data::store::Computed)).

| item | role |
|---|---|
| [`Computed`](crate::core::data::store::Computed) | one version of the store |
| [`ComputedView`](crate::core::data::store::ComputedView) | a stage's reading side, checked against its declared reads; [`with_overlay`](crate::core::data::store::ComputedView::with_overlay) reads a catch-up store on top of the run's store, a table present there read **whole** from it |
| [`StageOut`](crate::core::data::store::StageOut) | a stage's writing side: new table pieces and diagnostics; a stage never touches the store |
| [`fold_outs`](crate::core::data::store::fold_outs) | how the runner installs the collectors (the stages' `StageOut` values) into the store, the same on the sequential and the parallel path |

- **One writer per table**: plan construction guarantees it; the store checks it again.
- **A declared write always installs**, an empty table if the stage produced no table
  piece, so a read returning `None` means "no stage provides this table".
- **Every read and write is checked where it happens**: an undeclared access is an
  [`AccessError`](crate::core::data::store::AccessError) identifying the stage and the
  table.
- A lowered tree is computed data too (`flm.lowered.<backend>`), not a rewrite of a
  frozen tree.

## Synthesized values

A synthesized value is a small tree that a stage builds while a run is under way, held
outside the document's trees and numbered per stage invocation; a consumer brings it up to
date through value catch-up before reading it as document content ([synthesized
values](crate::guide::concepts_overview#synthesized-values)). Examples: the content a
reference displays, a formatted citation entry, a piece such as the list of footnotes.

- Built from document nodes with
  [`copy_run_into_value`](crate::lang::copy::copy_run_into_value); every copied node keeps
  its span and source, so a diagnostic about the value points into the document. The
  copy's purpose ([`ValueContent`](crate::lang::copy::ValueContent)): `Shown`, content
  shown a second time away from where it is written (each node copied as its element's
  [`InCopy`](crate::model::aspects::InCopy) aspect says: an xnote becomes a stand-in node
  that its rules write as the xnote's mark); `Own`, content the value holds as its own,
  copied as written.
- Numbered by [`TreeId::value`](crate::model::addr::TreeId::value) **per stage
  invocation, from zero**: an identifier inside a value means nothing outside the call
  that built it, while document trees are numbered by the arena and never reused.
- Therefore **no table installed into the run's store may be keyed by an address inside
  a value**, and a serialized table writes a stored value out in full, never by its
  identifier. The store refuses a table piece classified
  [`PieceClass::DocumentFact`](crate::core::data::table::PieceClass::DocumentFact) when it
  comes from a value
  ([`StoreError::DocumentFactInValue`](crate::core::data::store::StoreError::DocumentFactInValue)).
- Rendering one requires value catch-up
  ([`StageCx::resolve_value`](crate::core::run::StageCx::resolve_value); a lowering rule
  uses [`Lowerer::splice`](crate::core::lowering::Lowerer::splice)); see [AI guide:
  pipeline](crate::guide::ai_guide_pipeline).

## Examples

Elements, and element values of one element type:

```rust
use std::sync::Arc;

use flm_core::model::doc::FragmentLayout;
use flm_core::model::element::{element_of, element_value};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::stdcore::features::textstyles::{TEXT_STYLE, TextStyle, TextStyleElement};
use flm_core::stdcore::{self, StdcoreConfig};

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
let registry = builder.build(&RegistryInputs::new())?;

let source = Arc::new(
    FlmSource::new("\\section{Intro}\n\nSome \\emph{soft} and \\textbf{loud} text.")
        .with_origin(FlmOrigin::new("notes.flm")),
);
let fragment = parse_fragment(
    registry.lang(),
    source,
    FragmentLayout::default(),
    &ParseOptions::default(),
)?;

// Every node carries its element; the type name says what it is.
let names: Vec<&str> = fragment
    .tree()
    .iter_storage_order()
    .map(|node| element_of(&node).name.as_str())
    .collect();
assert!(names.contains(&"flm.heading"));

// `\emph` and `\textbf` are two *values* of the one element type `flm.textstyle`.
let styles: Vec<TextStyle> = fragment
    .tree()
    .iter_storage_order()
    .filter(|node| element_of(node).name == TEXT_STYLE)
    .filter_map(|node| element_value::<TextStyleElement>(&node).map(|value| value.style))
    .collect();
assert_eq!(styles, [TextStyle::Emphasis, TextStyle::Bold]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Placement and document order; one fragment placed twice is two trees:

```rust
use std::sync::Arc;

use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::lang::{
    FlmOrigin, FlmSource, LangInputs, ParseOptions, build_language, parse_fragment,
};

// A language with no feature at all: FLM's own structure only.
let lang = build_language(LangInputs::new());
let parse = |text: &str| {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    parse_fragment(&lang, source, FragmentLayout::default(), &ParseOptions::default())
        .expect("shallow enough to parse")
};

let title = Arc::new(parse("A title"));
let body = Arc::new(parse("Some text."));

let mut content = DocContent::new();
let first = content.place(Arc::clone(&title));
let second = content.place(title); // the same fragment again: a second tree
let third = content.place(body);

assert_ne!(first, second);
assert_eq!(content.walk_trees().collect::<Vec<_>>(), [first, second, third]);

let doc = Doc::new(content);
assert_eq!(doc.content().len(), 3);
```

## See also

- [AI guide](crate::guide::ai_guide): the entry point, with the vocabulary and the
  recipes.
- [AI guide: pipeline](crate::guide::ai_guide_pipeline): the stages that compute the
  tables, and value catch-up.
- [The document model](crate::guide::document_model): the full chapter.
- [Concepts overview](crate::guide::concepts_overview): every term used here.
