# The document model

This chapter describes what a document is to this library: how source text becomes a
tree, what a node of that tree means, how trees and nodes are addressed, what may
change a tree and when, and where everything computed about a document is kept.

Two design decisions shape the whole chapter:

- **The parse tree is the only representation of a document.** The tree the parser
  produces is not converted into a second structure. Every node carries, as a field,
  the FLM element that says what the node means.
- **Trees are rewritten only at the beginning, and then frozen.** Everything
  computed afterwards is stored beside the trees in typed *tables*, never written
  back into them.

The companion chapter [Stages, capabilities, and plans](crate::guide::pipeline)
describes the stages that do the computing; this chapter describes what those stages
read and write. The concepts page defines the terms used here in one section each,
in particular [the document tree](crate::guide::concepts_overview#the-document-tree)
and [frozen trees and computed
data](crate::guide::concepts_overview#frozen-trees-and-computed-data).

## One tree, and the elements on it

[`parse_fragment`](crate::lang::parse_fragment) parses one source into one tree. The
tree is a node tree of [techy](crate::techy), the parser toolkit FLM is built on,
specialized to the FLM language ([`Flm`](crate::lang::Flm)). FLM attaches its own
data to every node through techy's node extension
([`FlmNodeExt`](crate::lang::FlmNodeExt)), and one field of that extension is the
node's **element**.

An element is the value that says what a node means: a declaration with a stable
dotted type name, identified by the element value itself (its allocation) and not by
its name ([`ElementSpec`](crate::model::element::ElementSpec)). Reading the element
of a node is a field read — [`element_of`](crate::model::element::element_of) — not
a lookup by name, not a downcast, and not a match on a closed list of node kinds. It
works the same way on a freshly parsed tree and on a tree a rewrite produced.

A parse is *tolerant*: a problem in the source becomes a diagnostic and the parse
continues, so the tree covers the whole input. The only outcomes without a tree are the
cases of [`FragmentError`](crate::lang::FragmentError).

```rust
use std::sync::Arc;

use flm_core::model::doc::FragmentLayout;
use flm_core::model::element::{element_of, element_value};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::stdcore::features::textstyles::{TEXT_STYLE, TextStyle, TextStyleElement};
use flm_core::stdcore::{self, StdcoreConfig};

// A registry says which elements exist; it carries the language to parse with.
let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
let registry = builder.build(&RegistryInputs::new())?;

let source = Arc::new(
    FlmSource::new("\\section{Introduction}\n\nSome \\emph{soft} and \\textbf{loud} text.")
        .with_origin(FlmOrigin::new("notes.flm")),
);
let fragment = parse_fragment(
    registry.lang(),
    source,
    FragmentLayout::default(),
    &ParseOptions::default(),
)?;

// Every node carries its element, and the element's type name says what it is.
let names: Vec<&str> = fragment
    .tree()
    .iter_storage_order()
    .map(|node| element_of(&node).name.as_str())
    .collect();
assert!(names.contains(&"flm.heading"));

// `\emph` and `\textbf` are two element *values* of the one element type
// `flm.textstyle`; they differ in the data the element carries.
let styles: Vec<TextStyle> = fragment
    .tree()
    .iter_storage_order()
    .filter(|node| element_of(node).name == TEXT_STYLE)
    .filter_map(|node| element_value::<TextStyleElement>(&node).map(|value| value.style))
    .collect();
assert_eq!(styles, [TextStyle::Emphasis, TextStyle::Bold]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Elements: names, values, aspects

An element ([`ElementSpec`](crate::model::element::ElementSpec)) has three parts,
which are easy to confuse; the concepts page summarizes them under [elements and
aspects](crate::guide::concepts_overview#elements-and-aspects).

- **The type name** ([`ElementName`](crate::model::names::ElementName)) says what
  kind of thing the node is: `flm.heading`, `flm.textstyle`, `thm.exercise`. It is
  public and stable, and several commands may share it.
- **The element's own data**
  ([`ElementSpec::value`](crate::model::element::ElementSpec#structfield.value)) is
  one Rust struct per element type: a heading's level, a formatting command's style.
  Read it with [`element_value`](crate::model::element::element_value) given a node,
  or with [`ElementSpec::value_as`](crate::model::element::ElementSpec::value_as)
  given an element.
- **The aspects** ([`AspectMap`](crate::model::element::AspectMap)) are the
  behavior the rest of the library asks the element about.

The identity of an element is neither its name nor a Rust type: it is the shared
element value itself, compared by allocation with
[`ElementSpec::same_value`](crate::model::element::ElementSpec::same_value).
`\section` and `\subsection` are therefore two distinct element values that share
the type name `flm.heading`; and two commands that a feature declares with one element
value share that value, and with it their identity. See [names and
identity](crate::guide::concepts_overview#names-and-identity) for why identity in
this library is a name or an allocation, and never a Rust type.

### Aspects

An aspect is a piece of behavior attached to an element when a feature registers the
element, and read back through a trait object; it is evaluated per occurrence of the
element and reads no options ([elements and
aspects](crate::guide::concepts_overview#elements-and-aspects)). A stage or a
backend asks an element a question by looking the aspect up, instead of matching the
element's name against a list. Code written today therefore works on an element that
a feature written later declares. Nothing registers an aspect anywhere: any
object-safe trait can be one, and the aspect map is keyed by the trait object type,
so an aspect is always looked up as `dyn MyAspect`, never as the struct that
implements it.

The aspects the standard features share are declared in [`crate::model::aspects`]:

- [`BlockRoleAspect`](crate::model::aspects::BlockRoleAspect) — whether an
  occurrence is a block, inline content, or a *block decorator*, an element attached
  to the block it occurs in ([`BlockRole`](crate::model::aspects::BlockRole)). Read
  it with [`block_role_of`](crate::model::aspects::block_role_of), which supplies the
  default for elements that declare none.
- [`Referenceable`](crate::model::aspects::Referenceable) — the [reference
  points](crate::model::aspects::RefPoint) an occurrence carries: the places a
  reference may point at, each with the [label
  types](crate::model::aspects::AcceptedRefTypes) that may be written for it.
- [`Referencing`](crate::model::aspects::Referencing) — the counterpart of the
  previous aspect: the [reference sites](crate::model::aspects::RefSite) an
  occurrence carries, the places that point at a reference point.
- [`Sectioning`](crate::model::aspects::Sectioning) — an occurrence's heading level,
  and whether it enters the table of contents.
- [`ApartFromText`](crate::model::aspects::ApartFromText) — the content of an
  occurrence that is not part of the words of the content around it, such as the
  words of a footnote in a heading's title, which are shown elsewhere. The plain text
  of content ([`plain_text_of`](crate::lang::text::plain_text_of)), from which a
  heading's identifier is made when the heading has no label, leaves that content
  out; read it with [`apart_from_text_of`](crate::model::aspects::apart_from_text_of).
- [`InCopy`](crate::model::aspects::InCopy) — what represents an occurrence in a
  copy of content shown a second time (see [synthesized
  values](#synthesized-values) below).

Two rules follow from the way aspects are consulted, and both matter when writing
one. An aspect method always receives the *node*, so it is evaluated per occurrence
rather than per element: the same heading element declares a different reference
point at every place it occurs. And an aspect reads no
[options](crate::model::options): an aspect method receives the element's own data and
the node it is asked about, and nothing else, and aspects are consulted while a plan is
being built, when no document's option values exist. A fact that varies from document
to document is therefore an option of the stage that consumes the aspect's result, not
of the aspect.

## Names

Every entity FLM registers — an element, a capability, a table, a stage, a lowering
rule, a feature, a backend, a workflow, an output — is identified by a lowercase
[`Name`](crate::model::names::Name). Most of these names are dotted, and the segment
before the first dot says who owns the name: `flm` for the standard features, a
prefix of its own for anything a third party writes
([`Name::owner_prefix`](crate::model::names::Name::owner_prefix)). A few namespaces
are flat — a feature is `refs`, a backend is `text` — and a name with no dot is its
own owner prefix. Each namespace has a type of its own, so that a function that
expects a capability name cannot be handed a stage name:
[`ElementName`](crate::model::names::ElementName),
[`CapName`](crate::model::names::CapName),
[`StageName`](crate::model::names::StageName),
[`RuleName`](crate::model::names::RuleName),
[`FeatureName`](crate::model::names::FeatureName),
[`BackendName`](crate::model::names::BackendName),
[`WorkflowName`](crate::model::names::WorkflowName),
[`OutputName`](crate::model::names::OutputName),
[`RefKind`](crate::model::names::RefKind).

A name owns its text rather than borrowing a `&'static str`, because a name may be
computed while a registry is being built: one environment per theorem kind declared
in a configuration, one table per registered backend. The library is meant to be
extended from outside Rust, so a name computed while the registry is built must be
exactly as good as one written as a literal.

## Addresses: trees and nodes

A **tree** is one placement of a fragment's node tree in a document, identified by a
[`TreeId`](crate::model::addr::TreeId) that the document's arena assigns. Placing the
same shared fragment twice makes two trees: two places in the document, numbered
separately, each with computed data of its own.

A **node** is addressed by a [`NodeAddr`](crate::model::addr::NodeAddr): a `TreeId`
together with techy's node identifier within that tree. A `NodeAddr` is twelve bytes,
`Copy` and ordered, so it can serve as the key of an ordered map, and it is the key of
every table that stores something per node.

Two facts about that ordering are easy to get wrong:

- A node identifier carries a tag of the tree that assigned it (techy's tree layout, the
  storage of one tree's nodes), and the tag takes part in equality, ordering and
  hashing. Addresses from different trees therefore never collide.
- Iterating a map keyed by `NodeAddr` visits trees in identifier order and, within
  each tree, nodes in storage order. That order is deterministic, but it is **not
  document order**. Document order is the walk, described in the next section.

## Fragments, documents, and document order

A fragment is one piece of source as the parser delivered it (its tree, its source,
its parse diagnostics); a tree is one placement of a fragment's node tree in a
document, numbered by the document's arena; a document is its content (the tree
arena, the traversal list and the resolution map) and the option values bound on it,
and nothing else.

A [`Fragment`](crate::model::doc::Fragment) also records how the content at its top
level is laid out ([`FragmentLayout`](crate::model::doc::FragmentLayout): as blocks, as
inline content, or either). A fragment is shared as `Arc<Fragment>` and has no
identifier of its own; its identity is the shared pointer, so one fragment may be used
by several documents at once. The tree it holds is exactly what the parse produced,
source comments included, so a tool can recover the author's own text from it.

A [`Doc`](crate::model::doc::Doc) is its content
([`DocContent`](crate::model::doc::DocContent)) together with the option values bound
when it was assembled. The content has exactly three parts:

- the **tree arena** — every placement of a fragment, numbered with a `TreeId` in the
  order it was added;
- the **traversal list** — the order in which the listed trees are walked;
- the **resolution map** — which node places which other tree, so that the walk
  enters that tree at that node
  ([`DocContent::resolve`](crate::model::doc::DocContent::resolve), which reports a
  [`PlacementError`](crate::model::doc::PlacementError) rather than recording a
  placement that would break the walk).

[`DocContent::place`](crate::model::doc::DocContent::place) does two things at once:
it adds a placement of a fragment to the arena, and lists that tree for traversal.
[`DocContent::add_tree`](crate::model::doc::DocContent::add_tree) adds a tree without
listing it, for a tree the walk should reach through a node that places it instead; a
tree that is neither listed nor placed by a node belongs to the document but is never
walked. No element of the standard features places a tree through the resolution map
directly; a program embedding the library uses it to assemble a document from several
fragments, and an `\input` of a file that opens with a heading uses it (see
[documents of several files](#documents-of-several-files)).

A document has no structure of its own beyond those three parts: no front matter, no
chapters, no named parts. The structure of a document comes from the headings in its
own content and is computed by a stage, so that there is exactly one account of how a
document nests. The concepts page states the same division of work under [documents,
fragments, and the
corpus](crate::guide::concepts_overview#documents-fragments-and-the-corpus).

**Document order is one operation**:
[`DocContent::walk`](crate::model::doc::DocContent::walk). It goes through the
traversal list in order and enters each tree exactly once. A tree that a node places
is entered at that node, and its nodes follow that node immediately, before the
node's own children; "everything before the placed tree" is therefore exactly what
precedes the node that places it. Every notion of "earlier in the document" —
numbering, note order, citation order — is defined by this one operation.
[`walk_nodes`](crate::model::doc::DocContent::walk_nodes) and
[`walk_trees`](crate::model::doc::DocContent::walk_trees) are its two common
projections. The operation itself is [`Walk`](crate::model::doc::Walk), over a
[`WalkSource`](crate::model::doc::WalkSource): the trees by identifier, the listed
trees, and which node places which tree. The document is the walk source of its trees
as parsed, and a run is the walk source of its rewritten trees, so both are walked by
the same code.

```rust
use std::sync::Arc;

use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::lang::{
    FlmOrigin, FlmSource, LangInputs, ParseOptions, build_language, parse_fragment,
};

// A language with no feature declared at all: FLM's own structure only.
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
let second = content.place(title); // the same fragment placed again: a second tree
let third = content.place(body);

// Two occurrences of one fragment are two trees, with computed data of their own.
assert_ne!(first, second);
// The walk is document order; here it is the traversal list.
assert_eq!(content.walk_trees().collect::<Vec<_>>(), [first, second, third]);

let doc = Doc::new(content);
assert_eq!(doc.content().len(), 3);
```

### Documents of several files

A document may be written in several files: the primary file brings the others in
with `\input{file}`. [`assemble`](crate::model::assemble::assemble) is the operation
that parses a primary file together with its included files. The content of an
included file is parsed at the place of its command, into the tree of the file that
includes it, and its nodes keep spans into the included file, so a problem there is
reported at its line in that file. The rewrite `flm.input.splice` then inserts that
content in place of the command, before any other rewrite. From then on the tree is
one text: a paragraph may begin in one file and end in the other, and numbering,
labels and nesting continue across the files as if the content had been written in
place. The program embedding the library finds and reads each file through the input
resolver it supplies ([`InputResolver`](crate::lang::InputResolver)); the library
checks which file includes which, and reports a file that includes itself.

An included file that the input resolver reports as opening with a heading
([`IncludedFile::heading`](crate::lang::IncludedFile::heading), which the host reads
from the file's front matter) is a **heading input**; any other included file is a
**content input**, described above. A heading input becomes a tree of its own in the
arena: the file is parsed under the parsing state that applies at its command, the
heading first, and the tree is placed at the command's node
([`DocContent::resolve`](crate::model::doc::DocContent::resolve)), where the walk
enters it. The command's node is a block, and every output writes the placed tree
there (`flm.pres.subtree`). The placement is keyed by the node as parsed; a run finds
the node again in its rewritten trees by its span, which every rewrite keeps.

A run also reads the document in the walk's order. Once its rewrites are over, the
run walks its rewritten trees ([`crate::core::run::order`]): a placed tree is entered
at its node, so numbering, notes, citations and every other stage that counts or
orders things find the heading input's content at the place of the command, after
what precedes the command in the including file and before what follows it. A stage
walks the document with [`StageCx::walk`](crate::core::run::StageCx::walk), and sorts
what it gathers from several trees by
[`StageCx::position`](crate::core::run::StageCx::position).

A rewrite may copy or remove the node of such an `\input`. When a user definition
writes twice the argument that holds the command, the tree stays placed at the first
copy; the second copy is reported (`flm.run.placed-twice`), and every output writes a
dropped mark (`flm.pres.dropped`) naming the file at its position. When a user
definition's template does not use the argument or body that holds the command, the
placed tree is removed with it, like any other text of that argument: the run's trees
are the trees its walk reaches, and nothing of the file is written or reported.

### The document's metadata

A document's **metadata** — its title, its authors, its date, and any other key its
author writes there — is not part of its trees: it is one of the option values bound
on the document, [`DocMeta`](crate::stdcore::features::meta::DocMeta), which the
standard feature `meta` declares. The title, each author and the date are FLM text,
parsed as inline content when the document is processed, each as a source of its own
named after the file it was written in, so that a problem in it is reported at its
place there. Each becomes a [piece](crate::core::piece::Piece), a document-level
synthesized value that a backend places before the content: `flm.meta.title`,
`flm.meta.author` (one node per author, each kept apart, which the `meta` feature's
lowering rule for the authors joins) and
`flm.meta.date`. A document with no metadata places none of them. The library keeps
every other key of the metadata as data and does not show it. See [the document's
metadata](crate::guide::embedding#the-documents-metadata) for setting it.

## Sources and spans

Text does not reach the parser as a `String` but as a shared
[`FlmSource`](crate::lang::FlmSource): the text together with its
[`FlmOrigin`](crate::lang::FlmOrigin), which says where the content comes from. An
origin holds a **canonical path**: for a file, its path relative to the root folder
the host chose;
for any other content a host supplies, a key that host chooses. That one string has
three uses: it is the label a diagnostic shows, the key by which the library detects a
file that includes itself through a chain of inclusions, and the key of the tables a
host's input resolver keeps. For this reason `parse_fragment` takes a source the
caller has already built: a source created inside the parse would carry no such key,
and every check keyed by one would silently skip it.

Two kinds of source have no canonical path. A snippet held in memory carries
[`FlmOrigin::none`](crate::lang::FlmOrigin::none), and every keyed check skips it
rather than confusing it with other unnamed content. A **synthetic** source, created
by the library rather than read from anywhere — the compiled template of a user
definition, the source created for one use of that definition — carries
[`FlmOrigin::synthetic`](crate::lang::FlmOrigin::synthetic) with a label that says
what it is; the label is shown in diagnostics but never used as a key.

A [`FlmSpan`](crate::lang::FlmSpan) is a byte range together with the source it
belongs to, and every node carries one. A [`FlmDiagnostic`](crate::lang::FlmDiagnostic)
is a typed condition, a severity and a span; see
[diagnostics](crate::guide::concepts_overview#diagnostics). A diagnostic is kept by
what produced it: the diagnostics of a parse belong to the fragment
([`Fragment::diags`](crate::model::doc::Fragment::diags)), and the diagnostics of the
stages belong to the run.

Rewrites preserve spans. Where a rewrite resolves text — collapsing whitespace,
merging adjacent runs of text, applying typographic substitutions — the resulting node
carries a [`TextSourceMap`](crate::lang::TextSourceMap): a list of entries, each of
which says how many bytes of the node's text come from which range of the source. The
origin of every byte survives even when the characters changed.

## Tree states, and the frozen tree

A frozen tree is a document tree after the rewrite stages have run: it is never
changed again, and everything a later stage computes about it is stored in tables
beside it, keyed by node address ([frozen trees and computed
data](crate::guide::concepts_overview#frozen-trees-and-computed-data)).

Rewriting is the only thing that ever changes a tree, and the library has three
rewrite passes ([`crate::lang::rewrites`]): splicing included files, expanding user
definitions, and building block structure. A tree therefore passes through these
states:

1. **As parsed** — what [`Fragment::tree`](crate::model::doc::Fragment::tree) holds.
2. **Spliced** — the content of every included file is inserted in place of the
   `\input` command that brought it in ([`crate::lang::rewrites::input`]).
3. **Expanded** — every use of a user definition (a command or environment a document
   or a host defines by substitution) is replaced by that definition's template,
   instantiated with the arguments written at the use site
   ([`crate::lang::rewrites::userdefs`]).
4. **Structured** — content is laid out in paragraphs and blocks, and text is resolved
   once and for all ([`crate::lang::rewrites::blocks`]).
5. **Frozen** — the output of the last rewrite the plan scheduled. From here on the
   tree never changes.

Each pass is a plain function from one tree to another, which can be called on any
tree without a pipeline around it; each is also placed in a pipeline as a *rewrite
stage* ([`RewriteStage`](crate::core::stage::RewriteStage)). Which passes run, and in
which order, is decided by the plan, so the same document can be frozen in different
states for different purposes: a plan may leave a tree without block structure, for a
tool that wants the text as written.

Two consequences matter:

- **A rewrite reads no computed data.** The signature of
  [`RewriteStage::rewrite`](crate::core::stage::RewriteStage::rewrite) contains no view
  of the store, so "trees change only before they are frozen" is guaranteed by the
  types rather than by a rule to remember.
- **The output of each rewrite is kept.** Every rewrite the plan runs publishes its
  output for every tree as a [`TreesTable`](crate::core::stage::TreesTable) under the
  capability it provides, so a later stage can ask for a tree in a particular state —
  for example the tree with user definitions expanded — and not only for the frozen
  tree. The frozen trees themselves are read with
  [`Run::frozen_tree`](crate::core::run::Run::frozen_tree) and
  [`StageCx::frozen_tree`](crate::core::run::StageCx::frozen_tree).

A rewrite that fails on one tree fails for that whole tree: it records one error
diagnostic, the tree has no frozen form, and no later stage reads it. The document's
other trees are unaffected.

## Computed data: tables and the store

After the trees are frozen, everything computed about a document is written beside
the trees rather than into them. A table is typed data that one stage writes beside
the frozen trees and other stages read; its identity is its registered dotted name,
never its Rust type, and that name is also a capability
([`Table`](crate::core::data::table::Table)).

- **The value type implements [`Table`](crate::core::data::table::Table).** Its one
  required method, [`absorb`](crate::core::data::table::Table::absorb), says how two
  table pieces combine; a table piece is the part of one table that one stage
  invocation produced ([`PieceClass`](crate::core::data::table::PieceClass)).
  `absorb` is how the table pieces a per-tree stage produces become one table. A map
  keyed by `NodeAddr` and a map keyed by a symbolic name are the two usual shapes, but
  a document-wide count is a table too, one whose `absorb` adds.
- **A table's identity is its name, never its Rust type.** One Rust type can hold many
  named tables — the type holding lowered trees is declared once per backend — so the
  type alone could not say which table is meant. Readers and writers refer to a table
  through a typed key: a [`TableName`](crate::core::data::table::TableName) constant
  exported beside the value type for a fixed name, a
  [`TableHandle`](crate::core::data::table::TableHandle) for a name computed while the
  registry is built, and
  [`SingletonTable`](crate::core::data::table::SingletonTable) for a value type that
  has exactly one table. [`TableRef`](crate::core::data::table::TableRef) is the form
  of any of these with its type erased — the name together with the value type's
  identity — which stage declarations, plans and messages carry.
- **A table's name is also a capability**, the capability "this table is present and
  complete", so declaring a write provides that capability and declaring a read
  requires it. This connects the tables of this chapter to the stages of the next one;
  see [stages, capabilities, and
  plans](crate::guide::concepts_overview#stages-capabilities-and-plans).

The store is the run's collection of computed tables: one writer per table, a declared
write always installs a value, and every read and write is checked against the stage's
declarations at the access site ([`Computed`](crate::core::data::store::Computed)). The
store is versioned, and a stage never accesses it directly: it reads through a
[`ComputedView`](crate::core::data::store::ComputedView), which refuses a read the stage
did not declare, and writes new table pieces into a
[`StageOut`](crate::core::data::store::StageOut) collector. Only the run
([`Run`](crate::core::run::Run)) installs a collector's table pieces into the next
version of the store, with [`fold_outs`](crate::core::data::store::fold_outs); every
rule about ordering and about one writer per table is implemented there, once, for both
the sequential and the parallel path.

Because the store belongs to the run and not to the document, one document may be run
through several plans — checked, then rendered to two formats — without anything about
the document depending on which plan is running.

A table is serialized as plain data for inspection through
[`Table::project`](crate::core::data::table::Table::project); the `flm dump` command
and the tests read that serialized form. Addresses inside a table
are written through the helpers of [`crate::dump::project`] rather than by hand, so
that every serialized table uses the same notation.

## Synthesized values

A synthesized value is a small tree that a stage builds while a run is under way, held
outside the document's trees and numbered per stage invocation; a consumer brings it
up to date through value catch-up before reading it as document content ([synthesized
values](crate::guide::concepts_overview#synthesized-values)). Examples are the content
a reference displays and a formatted citation entry.

[`copy_run_into_value`](crate::lang::copy::copy_run_into_value) is how a stage builds a
synthesized value from a run of document nodes. Every copied node keeps its own span
and source, so a diagnostic about the value still points at the place in the document
where the content was written. A copy also states its purpose
([`ValueContent`](crate::lang::copy::ValueContent)):

- **Content shown a second time**, away from where it was written, such as what a
  reference shows of a heading. In such a copy, each element decides what represents
  its node, through the aspect [`InCopy`](crate::model::aspects::InCopy): the node as
  written, which is the default; nothing; or a new node of the element's own. An xnote
  (a note set apart from the running text, such as a footnote) is represented by such
  a new node, which its rules write as the note's mark, so that a heading's title that
  a reference shows holds the note's mark and never a second note.
- **Content the value holds as its own.** Such a copy is verbatim.

A diagnostic raised over content shown a second time is reported again at every place
the content is shown, and a report raised over the copy carries a line saying where the
copy is shown ([reports over content shown
again](crate::guide::rendering#reports-over-content-shown-again)).

A synthesized value is identified by a `TreeId` too, in a separate space:
[`TreeId::value`](crate::model::addr::TreeId::value), told apart from a document tree
by [`TreeId::is_value`](crate::model::addr::TreeId::is_value). The two spaces are
numbered differently. Document trees are numbered by the document's arena and are never
renumbered or reused. Synthesized values are numbered **per stage invocation**, from
zero, so two invocations assign the same identifiers to different trees.

Two rules follow, and both are requirements on code that stores computed data:

- **No table installed into a run's store may be keyed by an address inside a
  synthesized value.** Entries belonging to two different values would collide there
  without anything noticing. The store enforces the corresponding rule on content: a
  table piece that states a fact about the document
  ([`PieceClass::DocumentFact`](crate::core::data::table::PieceClass)) is refused when
  it comes from a synthesized value, with
  [`StoreError::DocumentFactInValue`](crate::core::data::store::StoreError::DocumentFactInValue).
- **A table serialized for inspection writes a stored synthesized value out in full,
  never as its identifier**, since that identifier would mean nothing to a reader.

A synthesized value has not been through any of the work the document's own trees went
through, so a consumer that renders one must first bring it up to date. That work is
[value catch-up](crate::guide::concepts_overview#value-catch-up), performed by
[`StageCx::resolve_value`](crate::core::run::StageCx::resolve_value); it is part of the
pipeline, and [Stages, capabilities, and plans](crate::guide::pipeline#value-catch-up)
describes it.

## The whole model, end to end

The example below parses a source into a fragment, places the fragment in a document,
runs a plan over the document, and reads one computed table and the rendered output
back:

```rust
use std::sync::Arc;

use flm_core::model::addr::TreeId;
use flm_core::model::aspects::Label;
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::stdcore::backends::text::TEXT_DOCUMENT;
use flm_core::stdcore::features::refs::LABELS;
use flm_core::stdcore::{self, StdcoreConfig};

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;

let source = Arc::new(
    FlmSource::new("\\section{Introduction}\\label{sec:intro}\n\nSee \\ref{sec:intro}.")
        .with_origin(FlmOrigin::new("doc.flm")),
);
let fragment = parse_fragment(
    registry.lang(),
    source,
    FragmentLayout::default(),
    &ParseOptions::default(),
)?;

let mut content = DocContent::new();
let tree = content.place(Arc::new(fragment));
let doc = Arc::new(Doc::new(content));
assert_eq!(tree, TreeId::tree(0));

let plan = Arc::new(Plan::for_workflow(&registry, "text", &Overrides::none())?);
let mut run = Run::new(doc, plan, Externals::none())?;
run.execute(&RunOptions::new())?;

// The tree the stages read is the frozen tree, not the parsed one.
assert!(run.frozen_tree(tree).is_some());

// Computed data sits beside the trees, keyed by node address.
let labels = run.table(LABELS)?.expect("the workflow scans labels");
let point = labels
    .by_label
    .get(&Label::parse("sec:intro").expect("a well-formed label"))
    .expect("the heading carries it");
assert_eq!(point.addr.tree, tree);

// And the rendered output is a table too.
let artifacts = run.table(TEXT_DOCUMENT)?.expect("the text workflow writes it");
let bytes = artifacts.entry().and_then(|a| a.as_bytes()).expect("one entry artifact");
assert!(String::from_utf8_lossy(bytes).contains("Introduction"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Where to go next

- [Stages, capabilities, and plans](crate::guide::pipeline) — the stages that compute
  the tables, and how their work is ordered.
- [Rendering: backends and outputs](crate::guide::rendering) — how a frozen document
  becomes output.
- [Extending FLM: features and packages](crate::guide::extending) — declaring
  elements, aspects and tables of your own.
- [Concepts overview](crate::guide::concepts_overview) — the definitions of every term
  used here.
