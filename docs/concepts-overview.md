# Concepts overview

This page defines the main concepts of FLM, one section per concept. Each section
opens with the definition of its concept, then states the rules that hold for it,
and ends with a list of the API items that embody it. The rest of the
documentation links into this page for these definitions; the section headings are
link targets and keep their wording once published.

## The document tree

The document tree is the node tree the parser produces from FLM source, and it is
the only representation of a document that the library has. The parser is built
on [techy](crate::techy), a parser toolkit for LaTeX-like languages, and a parse
yields one techy node tree over the FLM language. Every node carries the
[element](#elements-and-aspects) that says what the node means, stored in the
node's own extension ([`FlmNodeExt`](crate::lang::FlmNodeExt)), so finding out what
a node means is a field read rather than a lookup by name.

The library builds no second structure beside the tree, such as an abstract
document made of objects of its own. Two properties follow. Because there is only
one tree, there is nothing to keep consistent with it, and a feature that adds a
construct adds no case to any conversion. And because every node keeps the span of
the source it was written from, a diagnostic raised at any point of the processing
still points at the place the author wrote.

A node is addressed across a whole document by a [`NodeAddr`]: the pair of the tree
the node belongs to and the node's identifier within that tree. Every stage stores
what it computes about a node under that address. A node identifier is assigned by
the tree it belongs to, and a rewrite produces a *new* tree with new identifiers;
addresses are therefore taken after the trees are
[frozen](#frozen-trees-and-computed-data).

- [`Flm`](crate::lang::Flm) — the language definition the parser is driven by.
- [`parse_fragment`](crate::lang::parse_fragment) — source in, one tree out.
- [`NodeAddr`](crate::model::addr::NodeAddr) and
  [`TreeId`](crate::model::addr::TreeId) — the address of a node and the identifier
  of a tree.
- [`element_of`](crate::model::element::element_of) — the element a node carries.

[`NodeAddr`]: crate::model::addr::NodeAddr

## Elements and aspects

An element is the value that says what a node means: a declaration with a stable
dotted type name ([`ElementSpec`]), identified by the element value itself (its
allocation) and not by its name. A [feature](#features-and-packages) registers its
elements, and a node carries its element for as long as the node exists. Two
element values may share one type name: `\section` and `\subsection` are two
element values of the single `flm.heading` type, which differ in the data each
value holds. Two handles mean the same element when they point at the same
allocation ([`ElementSpec::same_value`](crate::model::element::ElementSpec::same_value)).

A family of related commands is declared as one element type with one element
value per command (the four text styles, the literals, the verbatim commands, the
text commands of mathematics mode): the type name says what kind of thing a node
is, and the data of each element value tells the commands apart. One lowering rule
then serves the whole family.

An aspect is a piece of behavior attached to an element when a feature registers the
element, and read back through a trait object; it is evaluated per occurrence of the
element and reads no options. A stage asks an element what it does — "is this a
block?", "which reference points does this node carry?" — instead of matching the
element's name against a list written into the stage. A stage written today
therefore works on an element that a feature written later declares, and nothing
in the library holds a closed list of node kinds.

Any object-safe trait can be an aspect, and no aspect is registered anywhere: a
feature, or a single test, may define an aspect of its own and attach it to its
elements, and code that does not use that trait is unaffected. Two rules hold for
every aspect:

- **An aspect is evaluated per occurrence.** The node being asked about is a
  parameter of every aspect method: one heading element declares a different
  reference point at every place it occurs.
- **An aspect reads no [options](crate::model::options).** An aspect method
  receives the element's own data and the node it is asked about, and nothing else.
  A fact that varies from document to document is therefore an option of the stage
  that consumes the aspect's result, not of the aspect.

- [`ElementSpec`](crate::model::element::ElementSpec) — the element declaration
  itself, and [`AspectMap`](crate::model::element::AspectMap), the aspects
  attached to it.
- [`crate::model::aspects`] — the aspects the standard features declare, among
  them [`BlockRoleAspect`](crate::model::aspects::BlockRoleAspect),
  [`Referenceable`](crate::model::aspects::Referenceable) and
  [`Sectioning`](crate::model::aspects::Sectioning).
- [`Decoration`](crate::stdcore::backends::pres::Decoration) and
  [`OpaqueContent`](crate::stdcore::backends::pres::OpaqueContent) — the two
  aspects that rendering consults.

[`ElementSpec`]: crate::model::element::ElementSpec

## Names and identity

Every entity FLM registers — an element, a capability, a table, a stage, a
lowering rule, a feature, a backend, a workflow, an output — is identified by a
stable, lowercase name ([`Name`]). Most names are dotted, and their first segment
says who owns the name: `flm` for the standard features, a prefix of its own for
anything a third party writes. The prefix is a convention that keeps the names of
different parties from colliding; the library itself does not read it.

A name is never a Rust type. The library is meant to be extended from outside
Rust — a theorem environment declared in a host's configuration, a table created
once per backend under the name `flm.lowered.<backend>` — so a name computed while
the registry is built must be exactly as good as one written as a literal. For the
same reason a [`Name`] owns its text as a shared string rather than borrowing a
`&'static str`; cloning one only increments a reference count.

Each namespace has a type of its own over [`Name`], so that a function that asks
for a capability name cannot be handed a stage name by mistake. Capabilities,
tables and [collections](#stages-capabilities-and-plans) (named sets of capabilities
that a caller demands together) share one namespace, because a table's name is also
the capability "this table is present and complete"; features, stages, lowering
rules, backends, outputs and workflows each have their own.

Two kinds of entity are identified by something other than a name. An element
value is identified by its allocation, so that two separately built elements with
the same name stay distinct. An option type is identified by its Rust type, so
that options add no namespace of their own: a feature declares one option struct
per [binding](crate::model::options::Bind), and a feature with more options gives
that struct more fields.

- [`Name`](crate::model::names::Name) — the shared representation, and
  [`Name::owner_prefix`](crate::model::names::Name::owner_prefix).
- The types of the namespaces:
  [`ElementName`](crate::model::names::ElementName),
  [`CapName`](crate::model::names::CapName),
  [`StageName`](crate::model::names::StageName),
  [`RuleName`](crate::model::names::RuleName),
  [`FeatureName`](crate::model::names::FeatureName),
  [`BackendName`](crate::model::names::BackendName),
  [`WorkflowName`](crate::model::names::WorkflowName),
  [`OutputName`](crate::model::names::OutputName),
  [`RefKind`](crate::model::names::RefKind).
- The two identities that are not names:
  [`ElementSpec::same_value`](crate::model::element::ElementSpec::same_value) (an
  element value is identified by its allocation) and
  [`OptionKey`](crate::model::options::OptionKey) (an option type is identified by
  its Rust type).

[`Name`]: crate::model::names::Name

## Documents, fragments, and the corpus

A fragment is one piece of source as the parser delivered it (its tree, its
source, its parse diagnostics); a tree is one placement of a fragment's node tree
in a document, numbered by the document's arena; a document is its content (the
tree arena, the traversal list and the resolution map) and the option values bound
on it, and nothing else.

A [`Fragment`](crate::model::doc::Fragment) also records how the content at its top
level is laid out ([`FragmentLayout`](crate::model::doc::FragmentLayout): as blocks, as
inline content, or either). It has no identifier of its own — its identity is the shared
pointer to it — so one fragment may be used by several documents at once without being
copied.

The three parts of a document's content are these:

- the **tree arena** numbers every *placement* of a fragment with a
  [`TreeId`](crate::model::addr::TreeId): placing one fragment twice makes two
  trees, each with computed data of its own;
- the **traversal list** is the order in which the listed trees are walked;
- the **resolution map** records which node places which other tree, so that the
  walk enters that tree at that node.

A document has no structure beyond that: no front matter, no chapters, no named
parts. The structure of a document comes from the headings in its own content and
is computed by a stage, so that there is exactly one account of how a document
nests. For the same reason a document holds none of the data that stages compute:
one document is rendered to several formats, or checked and then rendered, and
nothing about it may depend on which plan is running.

Document order is defined once, by the walk: the traversal list in order, each
tree entered exactly once, a tree that a node places entered at that node, with
that tree's nodes following that node immediately. Every notion of "earlier in the
document" — numbering, note order, citation order — uses this one operation, so a
placed tree needs no special case anywhere else.

A corpus is a set of documents processed separately whose cross-document references
still resolve. The data from outside one document arrives as an external table
([`Externals`](crate::core::run::Externals)), a table that no stage of the run writes:
the caller installs its value when the run is constructed. The standard case is an index
of the labels defined in the other documents. An external table's value that an earlier
run of this library produced carries a [`Stamp`](crate::core::run::Stamp) — the registry
it was produced under, the rewrite stages that ran, and a digest of the values of the
options those rewrites read — and constructing a run refuses one whose stamp differs
from what this run would produce. Data computed under one reading of a document
therefore cannot be used under another without notice.

- [`Fragment`](crate::model::doc::Fragment),
  [`DocContent`](crate::model::doc::DocContent) and
  [`Doc`](crate::model::doc::Doc) — the fragment, the document's content, and the
  document.
- [`DocContent::walk`](crate::model::doc::DocContent::walk) — the definition of
  document order.
- [`DocContent::resolve`](crate::model::doc::DocContent::resolve) — placing a tree
  at a node of another tree: how an `\input` of a file that opens with a heading
  places that file's tree, and how a program embedding the library assembles a
  document from several fragments.
- [`assemble`](crate::model::assemble::assemble) — parsing a document's primary
  file together with the files it brings in with `\input`. The content of an
  included file is parsed at the place of its command, in the tree of the file that
  includes it, and a rewrite then inserts it in place of the command before any
  other rewrite, so every later stage sees one text. The program embedding the
  library finds and reads the files ([`InputResolver`](crate::lang::InputResolver));
  the library checks the chain of inclusions. An included file that the input
  resolver reports as opening with a heading is a tree of its own instead, placed
  at its command.
- [`Externals`](crate::core::run::Externals),
  [`Stamp`](crate::core::run::Stamp) and
  [`EXTERNAL_INDEX`](crate::stdcore::features::refs::EXTERNAL_INDEX) — what a
  caller installs to process a corpus.

## Verbatim regions

A verbatim region is a run of raw text in which no markup character means
anything, always inside a group of the verbatim class: a backslash, a brace and a
percent sign are ordinary characters, braces need not balance, and the region ends
only at its own terminator.

FLM establishes one rule about such text while parsing, and the rest of the library
relies on it: **every run of verbatim content is wrapped in a group node of the
verbatim class**. Without the wrap, raw text would reach the tree as a bare list of
characters with nothing recording that it was verbatim, and every later pass would
have to work that fact out again from the construct the text came from. With the
wrap, whether content is verbatim is one typed question about a node. The rule can
also be checked: the nodes inside a verbatim group record the parsing state they
were read under, which shows independently that the region was read with the
language's rules switched off.

A feature does not write a parser to get this. It declares that an argument or a
body is raw text, and the two parsers that read raw text establish the wrap. The
standard `verbatim` feature is declared that way and offers `\verbtext`,
`\verbcode`, `\verba`, `{verbatimtext}` and `{verbatimcode}`.

What the *interior* of a verbatim region still recognizes is a policy carried by the
parsing state, which the characters-only argument of `\label{…}` and `\ref{…}` follows
too. In an ordinary document parse, a verbatim interior recognizes nothing but
characters and the terminator. In the parse of a user definition's template, the policy
keeps the placeholder syntax enabled, so that a placeholder still works inside raw text
and inside a label or a reference target.

- [`FlmGroupType`](crate::lang::FlmGroupType) — the group classes, verbatim among
  them, and [`VERBATIM`](crate::lang::structural::VERBATIM), the element a verbatim
  group carries.
- [`FlmVerbatimArgumentParser`](crate::lang::verbatim::FlmVerbatimArgumentParser)
  and [`FlmVerbatimBehavior`](crate::lang::verbatim::FlmVerbatimBehavior) — the
  two parsers that establish the wrap.
- [`ArgStdKind::Verbatim`](crate::lang::mint::ArgStdKind::Verbatim) and
  [`BodyDef::verbatim`](crate::lang::mint::BodyDef::verbatim) — how a feature
  declares that an argument or a body is raw text.
- [`RestrictedInterior`](crate::lang::RestrictedInterior) — what the interior of a
  region, and of a characters-only argument, still recognizes.
- [`crate::stdcore::features::verbatim`] — the standard feature that offers
  verbatim commands and environments, and
  [`VerbatimKind`](crate::stdcore::backends::pres::VerbatimKind), the kind of
  verbatim content a backend receives.

## Opaque regions

An opaque region is a part of a tree that the rewrite passes carry over untouched
(mathematics and verbatim content, any run parsed in math mode, the content of an
opaque wrapper); the lowering copies mathematics and verbatim content unchanged and
writes an opaque wrapper through its children. A verbatim region is a run of raw text
in which no markup character means anything, always inside a group of the verbatim
class ([verbatim regions](#verbatim-regions)).

"Untouched" means: no text resolution, no paragraph building, no block-structure hook
([`BlockStructure`](crate::lang::rewrites::blocks::BlockStructure)) consulted, no
paragraph-break marker removed or reported. Text resolution is the step that merges
adjacent text, makes each run of whitespace one space and applies the typographic
substitutions (`--` to a dash, `"` to a curly quote). The opaque regions there are:

- the content of a mathematics group;
- the content of a verbatim group, and a verbatim body;
- the content of an opaque wrapper: an argument or a body whose declaration asks for
  one, every characters-only group, such as the argument of `\label{…}`, and a proof
  title that begins with a star;
- any run parsed in mathematics mode.

An **opaque wrapper** is a group of its own class with no delimiters, which the parser
places around the content of an argument declared with
[`ArgDef::with_opaque_wrapper`](crate::lang::mint::ArgDef::with_opaque_wrapper), of a
body declared with
[`BodyDef::with_opaque_wrapper`](crate::lang::mint::BodyDef::with_opaque_wrapper), and of
every characters-only group
([`ArgStdKind::CharsGroup`](crate::lang::mint::ArgStdKind::CharsGroup)); the reader of a
proof's title places one around a title that begins with a star (see
[`theorems`](crate::stdcore::features::theorems::elements#a-starred-title-is-kept-as-typed)).
The content is parsed as usual, commands included; the wrapper only keeps it as written. It
serves an argument whose text an element reads as typed: the keys of `\cite`, the label
written in `\label` and in `\ref`, the target of `\hyperref`, a proof's back-reference. Two
places that spell such a text alike then hold the same characters, and a quote or a dash in
it is not turned into a typographic character. A group of `key=value` options is ordinary content unless its
declaration asks for a wrapper, because the value of a key may be text the document
shows, such as a heading's `alt=`.

Opacity belongs to the opaque *region*, not to the command that opened it. `\(x^2\)` and
`\begin{equation}…\end{equation}` are two ways of writing mathematics; a rule
attached to one of them would have to be repeated for the other and for every
mathematics construct a feature adds later. Because the rule is attached to the
region, mathematics content and verbatim content behave the same way however they
are written.

The parsing mode of the region decides, in both directions. A run parsed in
mathematics mode is opaque wherever it occurs. A run parsed in *text* mode inside
mathematics is ordinary content wherever it occurs: that is the argument of a text
command of mathematics mode, such as `\text{…}`, `\textbf{…}` or `\mbox{…}`, which
the mathematics feature declares for mathematics mode with an argument read in text
mode. Such an argument is laid out like any other inline content — its whitespace
resolved, typographic substitution applied, a paragraph break in it reported —
while the mathematics around it stays byte for byte what the author wrote. The
content of a verbatim group or of an opaque wrapper is opaque in every parsing mode,
and the exception for text inside mathematics does not reach inside it: in
`\cite{manual:{$\text{x}$}}`, the argument of `\text` stands under the opaque wrapper
of the keys, and is carried over as written.

The concept is used in two other places of the library. The checker that verifies
a structured tree applies its content rules only to the content *outside* opaque
regions, so an opaque region needs no exception. And at rendering, the lowering
driver copies a node whose element carries the opaque-content aspect together with
its whole subtree unchanged. It lowers nothing inside the subtree except a node that
holds content (the argument of a text command), and it does not look inside the
subtree when it decides whether another lowering round is needed. An emitter can
therefore reproduce the opaque region from its source, byte for byte. A backend may
instead select a lowering rule of its own for the element of an opaque region and
write the region from its nodes, still lowering nothing inside it: the recomposed
LaTeX does so for mathematics, so that a user definition used inside it is written as
a call of that definition. An opaque wrapper carries no such aspect: its element,
[`OPAQUE_WRAPPER`](crate::lang::structural::OPAQUE_WRAPPER), is written natively by
every backend through its children, like a group, so its text comes out as typed. An
element that declares a wrapper usually reads the argument itself, as `\cite` reads its
keys to write a citation mark.

- [`crate::lang::rewrites::blocks::pass`] — the pass that lays a content region out
  as blocks or as inline content, or leaves it opaque, and
  [`check_block_contract`](crate::lang::rewrites::blocks::check_block_contract),
  whose rules apply to the content outside opaque regions.
- [`FlmGroupType`](crate::lang::FlmGroupType) — the group classes; the mathematics,
  verbatim and opaque-wrapper classes open an opaque region.
- [`ArgDef::with_opaque_wrapper`](crate::lang::mint::ArgDef::with_opaque_wrapper) and
  [`BodyDef::with_opaque_wrapper`](crate::lang::mint::BodyDef::with_opaque_wrapper) —
  how a feature declares that an argument's or a body's content is kept as written.
- [`OpaqueContent`](crate::stdcore::backends::pres::OpaqueContent) — the aspect by
  which lowering copies such a subtree unchanged, and
  [`ContentInOpaque`](crate::stdcore::backends::pres::ContentInOpaque) — the
  aspect of a node inside such a subtree that is content and is lowered.
- [`ENVIRONMENT`](crate::stdcore::features::math::ENVIRONMENT) and
  [`OPAQUE`](crate::math_passthrough::OPAQUE) — the standard elements whose
  content is opaque (the second is the element `math-passthrough` gives to a command of
  mathematics that no feature declares); [`text`](crate::stdcore::features::math::text) — the text
  commands of mathematics mode, whose argument is content inside an opaque region.

## Block-transparent elements

The [block-structure pass](crate::lang::rewrites::blocks) is the rewrite that lays the
content of a document out in paragraphs and blocks. It does so according to the *block
role* each element declares ([`BlockRole`](crate::model::aspects::BlockRole)): a node of
block role is a block of its own between the paragraphs, and any other node is inline
content inside a paragraph. A node is always one or the other. An ordinary element whose
argument is laid out according to its content therefore becomes a block of its own when
that argument holds a paragraph break, and its content is laid out as paragraphs inside
it; an argument declared inline reports the paragraph break instead.

A *block-transparent* element is laid out as if its content were written directly
in the content around it: a paragraph break inside its content is a paragraph break
of the surrounding content, and a block inside it, such as a list, is a block of the
surrounding content. It never becomes a block of its own. A reviewer's highlight of
a passage ([`annotations`](crate::stdcore::features::annotations)) is the typical
case: the highlighted passage may cross a paragraph boundary, and the highlight
continues across it into the next paragraph.

Before any paragraph is built, the block-structure pass *cuts* every node of a
block-transparent element whose content crosses a paragraph break or holds a block. The
cut replaces the node by *cut parts* — nodes that each hold one stretch of inline
content, or one block — with the paragraph breaks between them (cut parts are not the
document-level [pieces](#pieces) that a backend places beside the content). The layout
step of the block-structure pass then places each cut part according to its block role,
like any other node. The element decides everything about its cut parts: which of its
arguments it cuts, what a cut part is, and which cut parts carry a *continuation
record*. A continuation record is data of the element's own type, attached to a cut
part, that says which cut the part came from and where it is among the parts, so that
the element's lowering rules can render the first cut part differently from the others
(a highlight shows the reviewer's initials on its first part only). A node that needed
no cut carries no record.

- [`BlockTransparent`](crate::lang::rewrites::blocks::BlockTransparent) — the
  aspect an element carries to be block-transparent, and
  [`BlockCutter`](crate::lang::rewrites::blocks::BlockCutter), the operations it
  cuts its node with.
- [`FlmNodeExt::payload`](crate::lang::FlmNodeExt::payload) — where a
  continuation record is attached to a node.

## Fields: data a construct reads

A field is a macro that a construct absorbs as data rather than as content
(`\label` after a heading; `\\`, `\label`, `\tag` and `\nonumber` in a display-math
body); every field node carries the one element `flm.field`, and a construct takes
its fields through its tack-on argument, a trailing argument that absorbs the field
macros written after the construct's other arguments
([`ArgStdKind::TackOn`](crate::lang::mint::ArgStdKind::TackOn)).

A field says something about the construct that reads it — this heading carries
this label, this equation line is not numbered — and means nothing at the place it
is written, so its position in the content makes no difference. The value of the
`flm.field` element is the field's name as written; the feature that declares a
field chooses no element of its own.

The construct that declared a field decides what the field means, and nothing else
does: a user definition may declare a `\label` field and give it any meaning, and no
scan of the document mistakes it for a label. Reading a field is the construct's own
work — a heading reads its `\label` fields and turns them into the reference point
it declares. A field node that reaches a backend is an error, because it means that
the construct that absorbed it did not read its data.

A field is declared in one of two ways: as a [tack-on
field](crate::lang::mint::TackOnField) composed into a construct's tack-on argument,
which is how `\label` is written after a heading; or as an ordinary command of a
[scoped package](#scoped-packages), which is how the four commands of an [equation
line](#equation-lines) are written inside a display-math environment's body.

- [`FIELD`](crate::lang::mint::FIELD) and [`Field`](crate::lang::mint::Field) —
  the element and its value; [`field_name`](crate::lang::mint::field_name) reads
  the name of a field node.
- [`ArgStdKind::TackOn`](crate::lang::mint::ArgStdKind::TackOn) — the tack-on
  argument that absorbs fields.
- [`FieldRule`](crate::stdcore::backends::lowering::FieldRule) — the only lowering
  rule of the field element, which reports a field that no construct read.

## Equation lines

An equation line is one line of a display-math environment as its body reads
between two `\\`: its mathematics, its labels, its tag and its no-number mark; each
line is one reference point.

The display-math environments are `{equation}`, `{align}` and the related
environments of the `math` feature. The body of such an environment is still an
[opaque region](#opaque-regions): nothing inside it is lowered or resolved, except
the argument of a text command such as `\text{…}` and the content of a `\tag`. The
`math` feature adds structure over that region, read once when the body is parsed:
a public reader splits a body into its lines, and a parse-time error reports a line
that carries a `\label` or a `\tag` where no number will be set. A line that holds no
mathematics at all is an error that the *processing feature* reports, the feature
that reads the mathematics and lowers the formulas (the [math
processor](#features-and-packages), such as `math-passthrough`), since only it reads
what a line holds. The four commands written inside such a body — `\\`,
`\label`, `\tag` and `\nonumber` — are [fields](#fields-data-a-construct-reads): data
the environment reads, not content at their position.

Three consumers use the lines. The rules of the processing feature write an
environment line by line: on the `latex-recomposed` backend, `math-passthrough` writes
each line back with its labels, as their [safe
labels](crate::stdcore::features::refs::SafeLabels), at the line's end, and on every
other backend its rule of a display-math environment
([`ENVIRONMENT_RULE`](crate::math_passthrough::ENVIRONMENT_RULE)) writes each
line with its anchor, its tag or its number, and its line break. The `math` feature's
library ([`frame`](crate::stdcore::features::math::frame)) gives any processing
feature each line's anchor and number. And each
line can be referred to on its own: the environment declares one reference point
per line, and a line that can be numbered carries that line's labels, so
`\ref{eq:name}` resolves to the line that carries `eq:name`.

A line that cannot carry a number — one marked `\nonumber`, and every line of
`equation*`, `align*` or `gather*` — is a reference point too, so that a tool can
walk every line the same way; it keeps no label, because a reference to it would
have no number to print. Every line still has an [anchor](#reference-points-labels-and-anchors),
labeled or not, numbered or not, so that a link can lead to any line of any
document. A line that can be numbered states its [number
source](crate::model::aspects::NumberSource) (the counter and the counter style);
the [numbering](crate::guide::numbering) feature computes the numbers, and a
reference to a numbered line shows that line's number.

- [`equation_lines`](crate::stdcore::features::math::lines::equation_lines) and
  [`EquationLine`](crate::stdcore::features::math::lines::EquationLine) — the
  reader, and what one line holds.
- [`EmptyLine`](crate::math_passthrough::EmptyLine) and
  [`LabelWithoutNumber`](crate::stdcore::features::math::lines::LabelWithoutNumber)
  — the error of `math-passthrough`'s check and the parse-time error.
- [`crate::stdcore::features::math::lines`] — the scoped package that declares
  `\\`, `\label`, `\tag` and `\nonumber` inside those bodies only, and the syntax
  chapter's [description of them](crate::guide::language_syntax#inside-a-display-math-environment).

## Frozen trees and computed data

A frozen tree is a document tree after the rewrite stages have run: it is never
changed again, and everything a later stage computes about it is stored in tables
beside it, keyed by node address.

The library has three rewrite passes, and they run before anything is computed:
splicing the content of included files in place of their `\input` commands,
expanding user definitions, and grouping content into paragraphs and blocks.
Nothing after them changes a tree. Everything computed later is stored beside the
trees, in typed **tables**: a map of labels, a set of resolved references, a
document-wide count, an output's artifact set.

A table is typed data that one stage writes beside the frozen trees and other
stages read; its identity is its registered dotted name, never its Rust type, and
that name is also a capability. One Rust type may hold several tables: the type
holding lowered trees is declared once per backend. Readers and writers refer to a
table through a typed key, so the value comes back already typed.

Later results are kept out of the trees because one document is processed in more
than one way: it is checked and then rendered, or rendered to two formats. If later
work wrote its results into the tree, the tree would depend on which plan was
running, and the results of one plan could be mistaken for those of another.
Because later results are kept out, the frozen trees depend only on the document
and the rewrites, node addresses stay valid for a whole run, and a table can be
recomputed, dropped or replaced without changing the document.

The [store](crate::core::data::store::Computed), the run's collection of computed
tables, follows three rules in every build:

- **One writer per table.** Plan construction guarantees it, and the store refuses
  a second writer if one ever reached it.
- **A declared write always installs a value.** A stage that declared a write but
  produced nothing still installs an empty table, so a read that returns `None`
  always means "no stage provided this table" and never "a stage ran and found
  nothing".
- **Reads and writes are declared, and checked where they happen.** An undeclared
  read or write is a typed error that identifies the stage and the table, not a
  silent `None`.

A lowered tree is computed data too, held in the table `flm.lowered.<backend>`; it
is not a rewrite of a frozen tree.

- [`crate::lang::rewrites`] — the three rewrite passes.
- [`Table`](crate::core::data::table::Table) and
  [`TableName`](crate::core::data::table::TableName) — the trait of a table's value
  type, and a table's typed name.
- [`Computed`](crate::core::data::store::Computed),
  [`ComputedView`](crate::core::data::store::ComputedView) and
  [`StageOut`](crate::core::data::store::StageOut) — the store, the reading side and
  the writing side.
- [`LoweredTrees`](crate::stdcore::backends::lowering::LoweredTrees) — the value
  type of the lowered trees of one backend.

## Stages, capabilities, and plans

A capability is a registered dotted name for a state of processing that a stage
brings about, such as `flm.blocks.structured`: a stage declares the capabilities it
provides and the capabilities it requires, and a caller demands the capabilities it
wants ([`CapabilityDef`](crate::core::registry::CapabilityDef)).

A stage is one unit of work of the pipeline: it declares the capabilities it
provides and requires, the tables it reads and writes, its effect (compute, check,
or input/output) and its scope (once per tree, or once per document), and the plan
decides whether and when it runs ([`Stage`](crate::core::stage::Stage)).

A caller *demands* the capabilities it wants (asks the plan for stages that provide
them; [`PlanRequest::demand`](crate::core::plan::PlanRequest::demand)), usually by
choosing a workflow, a named set of demands together with the choices that go with
them ([`WorkflowDef`](crate::core::registry::WorkflowDef)). The library
then derives which stages to run and in which order. A plan is the list of stages
that satisfies a plan request, in dependency order and grouped in batches; it is a
pure value of the registry and the request, with nothing of any document in it
([`Plan`](crate::core::plan::Plan)). A run is one execution of one plan over one
document; it owns the frozen trees, the store and the diagnostics
([`Run`](crate::core::run::Run)).

No caller writes a pipeline by hand, for two reasons. Which features a program
registers is the program's own choice, so no single place could know what the
pipeline should be; and a hand-written pipeline would have to be edited every time a
feature was added. Because a caller states what it wants rather than how to get it,
a stage that an added feature registers is scheduled automatically for every caller
that demands what the stage provides, and a caller that demands nothing more runs
nothing more.

A plan holds nothing of any document, so one plan serves every document rendered
the same way. A run owns everything it computes, so one document may be run several
times, under different plans, without one run seeing another's results.

The work of a run happens in this order:

```text
parse → rewrite stages → (trees frozen) → resolution and check stages
      → lowering stages → emit stages → the artifact set a caller reads
```

There are two kinds of stage accordingly. A **rewrite stage** runs before the
trees are frozen and turns one tree into another; it reads no computed data at all,
so its results can be shared by every plan that runs the same rewrites. Every other
stage runs after the trees are frozen and declares two things about itself: its
[`Effect`](crate::core::stage::Effect) — `Compute` (data and diagnostics), `Check`
(diagnostics only) or `Io` (effects outside the store) — and its
[`Scope`](crate::core::stage::Scope), `PerTree` or `Document`. The plan orders every
`Io` stage after all the others, and a run that has recorded an error diagnostic
stops before the first batch holding an `Io` stage. A document with errors therefore
produces no effects outside the store unless the caller asks for them with
[`RunOptions::continue_with_errors`](crate::core::run::RunOptions::continue_with_errors).

A stage author writes little of this down. Because a table's name is also a
capability, declaring a table written provides that capability, and declaring a
table read requires it; the registry and the plan derive both, so a stage lists its
reads and writes and nothing else. Four registered declarations decide the rest:

- a capability may **refine** another: a stage providing the refinement satisfies a
  demand for the parent ([refinement](crate::guide::pipeline#refinement));
- a capability may be **declared by one feature and provided by another**: the `math`
  feature declares [`flm.math.content`](crate::stdcore::features::math::CONTENT), "the
  mathematics of the document has been checked", and provides it with no stage of its
  own; the feature that processes mathematics provides it with its check stage
  (`math-passthrough`'s
  [`flm.math-passthrough.check`](crate::math_passthrough::CHECK)) and adds its check to
  the collection `flm.validated`, so that a workflow runs that check exactly when such a
  feature is registered;
- a **collection** is a name for a set of capabilities that any feature may join.
  `flm.validated` gathers that way every check a registry holds, and every workflow
  the standard package registers demands it, so a render runs the same checks as the
  `check` workflow;
- an **ordering** declaration adds edges that only order stages, for a feature that
  must place its stage relative to another feature's stage without needing its
  results;
- a **workflow** is registered by a feature under a name
  ([`WorkflowDef`](crate::core::registry::WorkflowDef)).

- [`Stage`](crate::core::stage::Stage),
  [`StageDecl`](crate::core::stage::StageDecl) and
  [`Needs`](crate::core::stage::Needs) — what a stage is and what it declares.
- [`CapabilityDef`](crate::core::registry::CapabilityDef),
  [`CollectionDef`](crate::core::registry::CollectionDef),
  [`OrderingDef`](crate::core::registry::OrderingDef) and
  [`WorkflowDef`](crate::core::registry::WorkflowDef) — how capabilities and their
  refinements, collections, ordering edges and workflows are registered.
- [`PlanRequest`](crate::core::plan::PlanRequest) and
  [`Plan`](crate::core::plan::Plan) — the plan request and the plan built from it.
- [`Run`](crate::core::run::Run) and [`StageCx`](crate::core::run::StageCx) —
  executing a plan, and what a stage can read while it runs.

## Synthesized values

A synthesized value is a small tree that a stage builds while a run is under way,
held outside the document's trees and numbered per stage invocation; a consumer
brings it up to date through value catch-up before reading it as document content.

Examples are the content a reference displays, a formatted citation entry, and the
exact source text an emitter has to print back. A synthesized value (a *value* for
short on this page) is lowered and rendered like document content, but no stage that
scans the document ever reads it.

A value needs an identifier, and it gets one from the type that identifies document
trees, [`TreeId`](crate::model::addr::TreeId), in a separate space. Document trees
are numbered by the document's arena in construction order and are never
renumbered or reused. Values are numbered by the *stage invocation* that builds
them, from zero and per invocation, so two invocations assign the same identifiers
to different trees: a value's identifier means nothing outside the call that built
it.

Two requirements follow from that numbering, and both apply to any code that stores
computed data:

- No table installed into a run's store may be keyed by an address inside a value,
  because entries belonging to two different values would collide there without
  anything noticing.
- A table that holds a value writes the value out in full when the table is
  serialized for inspection
  ([`Table::project`](crate::core::data::table::Table::project)), never as its
  identifier, because a reader of the serialized table has no way to resolve the
  identifier.

Every node a value copies from document content keeps its own span and its own
source, so a diagnostic raised about the value still points at the place in the
document where the content was written. A copy made to show document content a
second time (for example, the heading text a reference displays) asks each copied
node's element, through the [`InCopy`](crate::model::aspects::InCopy) aspect, what to
hold in the node's place: by default the node as written, otherwise nothing or a new
node of the element's own ([synthesized
values](crate::guide::document_model#synthesized-values) in the document model
chapter).

- [`TreeId::value`](crate::model::addr::TreeId::value) and
  [`TreeId::is_value`](crate::model::addr::TreeId::is_value) — the identifier space
  of values, separate from the document trees.
- [`NodeAddr`](crate::model::addr::NodeAddr) — the address that may point either
  into a document tree or into a value.
- [`copy_run_into_value`](crate::lang::copy::copy_run_into_value) — how a stage
  builds a value from a run of document nodes.

## Value catch-up

Value catch-up is the work done on demand when something comes to render a
synthesized value: the plan's rewrites and the per-tree stages the consumer depends
on run over that one tree, and what they compute goes into a catch-up store that a
view reads on top of the run's store, a table present there read whole from it.

A [synthesized value](#synthesized-values) has not been through any of the work the
document's own trees went through: no rewrite has run over it, and no stage has
computed anything about it. Value catch-up does that work at the moment something
comes to render the value. What the stages compute goes into the catch-up store,
which belongs to that one catch-up, and never into the run's own store.

The separate store enforces one rule: a value is not part of the document, so it
must neither add to the document's facts nor read them. Both sides are checked.

On the **writing** side, the value type of each table says what each [table
piece](crate::core::data::table::PieceClass) — the part of the table that one stage
invocation produced — holds. A table piece that states a fact about the document as
a whole — a label declared inside a value, a numbered point — is refused when it is
produced during a catch-up; table pieces keyed by node address, and empty table
pieces, are accepted and go to the catch-up store.

On the **reading** side, the catch-up store shadows a table **whole, never entry
by entry**. A stage that runs again over a value therefore sees, for every table
the catch-up computed again, the value's own entries and nothing of the
document's: a stage reading the document's reference points inside a value finds
the value's points, which are usually none. A feature author must therefore know
this: a stage that must also work inside a value *and* needs data about the whole
document must read a table that no per-tree stage of the catch-up writes, in
practice a table that a document-scope stage writes. The `refs` feature does so:
its resolve stage reads
[`REF_TARGETS`](crate::stdcore::features::refs::REF_TARGETS), written once for the
document, rather than the reference points of
[`REFPOINTS`](crate::stdcore::features::refpoints::REFPOINTS), written per tree.

The diagnostics a catch-up produces are returned to the consumer with the result,
and the consumer records them: a catch-up writes nothing into the run.

- [`StageCx::resolve_value`](crate::core::run::StageCx::resolve_value) and
  [`StageCx::resolve_value_with`](crate::core::run::StageCx::resolve_value_with)
  — performing a catch-up; [`CaughtUp`](crate::core::run::CaughtUp) and
  [`CatchUpError`](crate::core::run::CatchUpError) — its result.
- [`Plan::per_tree_closure`](crate::core::plan::Plan::per_tree_closure) — which
  stages run again over the value.
- [`Lowerer::splice`](crate::core::lowering::Lowerer::splice) — a lowering rule
  rendering a value where its node is placed; and, in the other direction,
  [`Lowerer::lower_region`](crate::core::lowering::Lowerer::lower_region) — a rule
  of a node inside a value showing content of the document, as the document's own
  content rather than a copy of it.
- [`PieceClass`](crate::core::data::table::PieceClass),
  [`Table::piece_class`](crate::core::data::table::Table::piece_class),
  [`FoldTarget::Scratch`](crate::core::data::store::FoldTarget::Scratch) and
  [`StoreError::DocumentFactInValue`](crate::core::data::store::StoreError::DocumentFactInValue)
  — the writing side, which refuses a table piece that states a fact about the
  document.
- [`ComputedView::with_overlay`](crate::core::data::store::ComputedView::with_overlay)
  — the reading side: it reads a catch-up store on top of the run's store, a table
  present in the catch-up store read whole from it.

## Backends, lowering, and outputs

A backend is the registration bundle for one output format: the elements its
emitter writes natively, a lowering stage, an emit stage and a final stage
([`BackendDef`](crate::core::registry::BackendDef)). Rendering is not a special
mechanism: lowering and emitting are ordinary stages over a frozen document, and a
final stage publishes the result.

Lowering rewrites each frozen tree into a presentation tree for one backend, by
applying the lowering rule selected for each element, in rounds, until only
elements the backend writes natively remain; the result is computed data in the
table `flm.lowered.<backend>`. A presentation tree is built from a small set of
*presentation elements* that every backend shares
([`crate::stdcore::backends::pres`]), plus the elements a backend writes natively.
Where the frozen tree holds a paragraph, the lowered tree holds `flm.pres.par`;
where it holds a heading, `flm.pres.block-heading`; where it holds a reference, a
`flm.pres.link` around the content the reference displays. The frozen tree itself
never changes.

Because the presentation elements are shared, a lowering rule written for one
element works for every backend, and only what a particular format really does
differently needs a rule of its own. A presentation node carries no open set of
properties and no handler per backend: everything it states beyond its element and
its children is one typed payload. An emitter that cannot express a field of a
payload ignores it, so a format with no way to show a detail of presentation may
drop that detail; content is never dropped.

A lowering rule is a named registry entity written for one element: it declares
what it requires, reads and produces, and implements `Lower`, which stages the
element's presentation for one backend
([`LoweringRule`](crate::core::lowering::LoweringRule)). To *stage* a node is to
build it into the tree that a lowering rule or a rewrite hook is producing, through
the context the rule or hook is handed (the [`Lowerer`](crate::core::lowering::Lowerer)
at lowering). What a rule requires and reads is added to the declared needs of its
backend's lowering stage. Because each rule declares every presentation element it may
produce, plan construction checks *before anything runs* that the backend can in
the end write everything the document may contain. Where several rules could lower
one element — the element author's own rule, and a more specific one written for a
particular format — the plan chooses one per backend and element, in a fixed order
([`SelectedRules`](crate::core::plan::SelectedRules)).

A rule written for a particular format often refers to that format's backend and
presentation elements, and sometimes to a table another feature computes, without
its own feature registering any of them. The feature then makes a peer declaration
of each: a forward declaration of an element, table, service slot, scoped package
or backend that another feature owns, with the element's payload type or the
table's value type as the signature ([`PeerName`](crate::core::registry::PeerName)).
The registry build resolves the peer declarations once every feature has
registered. A lowering rule that depends on a name nobody declares is *set aside*:
it is excluded from rule selection and from the completeness check, listed for
inspection, and the registry build records one Note and still succeeds. A peer
declaration of a table that the rule declares as an optional read is the exception:
when it resolves to nothing, the rule is not set aside; it runs and finds the table
absent. Separately, a rule whose default mark names particular backends
([`DefaultMark::OnBackends`](crate::core::lowering::DefaultMark::OnBackends)) is never
a candidate on any other backend.

The lowering driver stages two kinds of node on its own, without any rule: the
`flm.pres.deco-mark` node left where a [decoration](#decorations-roles-and-kinds)
was moved out of a content run, and the stand-in for an element the caller accepted
as unrenderable. No rule leads to these nodes, so a backend's native set (the list of
elements its emit stage writes directly,
[`BackendDef::native`](crate::core::registry::BackendDef::native)) must hold their
elements directly. A *native-set implication* is the declared form of such a
requirement: an element every backend must write natively, or one a backend must
write natively whenever its native set holds an element carrying a given aspect.
The registry build checks every backend against every implication, so a backend
missing one is refused before any document is processed.

An emitter is a backend's emit stage: it walks the lowered trees and writes the
output's bytes into a table, from which the backend's final stage makes the artifact
set. An artifact is one file's worth of output held as a value (a folder artifact
holds the files of a folder), identified by its producer and a name and never by a path;
an artifact set is the artifacts of one output, at most one of which is the entry
artifact, the one that represents the output as a whole (the HTML page, the text, the
LaTeX document)
([`ArtifactSet`](crate::core::artifact::ArtifactSet)). Nothing in this library
writes files: a host that writes them asks its naming service
([`ArtifactNamer`](crate::core::artifact::ArtifactNamer)) for the path of each
artifact. Because the naming service works from the artifact's identity and a few
facts known in advance (whether it is an entry artifact, its media type and file
extension, and the outputs the command produces), a producer can write a link to another
artifact before that artifact's bytes exist.
How a backend's final stage assembles the entry artifact is described under
[output regions and the final stage](#output-regions-and-the-final-stage).

A construct element is a presentation element that mirrors one fundamental construct
of an output format (an HTML tag; a LaTeX macro call, environment, raw text or
definition), so that format-specific output is structured data in the lowered tree
rather than a string. A rule stages a construct element to write output that no
shared presentation element carries.

The library ships four backends:

- [`text`](crate::stdcore::backends::text) builds a document for the [layout
  engine](crate::layout) and prints it at a chosen width.
- [`html`](crate::stdcore::backends::html) writes the document as an HTML page for
  a browser. Every byte goes through one typed sink, which escapes document text
  where it writes it. The CSS classes and the attributes a stylesheet may rely on
  are derived from the names of the presentation elements rather than written a
  second time. Each element requires the [style
  definitions](#style-and-script-definitions) it needs by identifier. The HTML
  construct element ([`crate::stdcore::backends::html::construct`]) holds one HTML
  tag as structured data.
- [`latex-recomposed`](crate::stdcore::backends::latex_recomposed) writes the
  document back as LaTeX source, close to what an author would have typed, from the
  trees without block structure (no paragraphs built), and leaves numbering and
  cross-references to LaTeX.
  Its rules stage the construct elements of the LaTeX family
  ([`crate::stdcore::backends::latex::construct`]): a macro call, an environment,
  raw text, a definition. Its output is the recomposed LaTeX together with a style
  file that holds exactly the definitions the recomposed LaTeX needs; see [LaTeX
  package requests, definition pieces and
  hooks](#latex-package-requests-definition-pieces-and-hooks).
- [`latex-rendered`](crate::stdcore::backends::latex_rendered) writes the document as
  LaTeX for the LuaLaTeX engine, from the trees laid out in paragraphs, with everything
  FLM resolves already resolved: numbers, cross-references, the table of contents, the
  notes. Each presentation element is written as one `\flmx…` command or environment
  whose key-value argument carries every fact of the element, and a style file defines
  each command with a default look. These commands, their keys and the roles and kind
  tokens they carry form the backend's **contract**, the counterpart of the HTML classes
  and attributes a stylesheet relies on: a LaTeX template registers its own look for any
  role or kind ([the rendered LaTeX
  contract](crate::guide::rendering#the-rendered-latex-contract)).

The two LaTeX backends are independent products: the recomposed LaTeX is LaTeX source a
person reads and edits, which LaTeX resolves; the rendered LaTeX is resolved content that
LuaLaTeX only lays out on pages. They share nothing but the library mechanisms of the LaTeX
family (escaping, the typed sink, the construct elements, the slots of package requests
and definition pieces).

- [`BackendDef`](crate::core::registry::BackendDef) — what a backend registers,
  and [`OutputDef`](crate::core::registry::OutputDef) — the name a caller asks for
  and the table that holds the output's artifacts.
- [`NativeImplication`](crate::core::registry::NativeImplication) — what a
  backend's native set must contain, checked when the registry is built.
- [`PeerName`](crate::core::registry::PeerName) — a feature's declaration of
  something another feature owns, and what setting a rule aside means.
- [`LoweringRule`](crate::core::lowering::LoweringRule),
  [`Lower`](crate::core::lowering::Lower) and
  [`Lowerer`](crate::core::lowering::Lowerer) — what a lowering rule is, and the
  only interface it has to the run.
- [`crate::stdcore::backends::pres`] — the presentation elements every backend
  shares.
- [`LoweringStage`](crate::stdcore::backends::lowering::LoweringStage) and
  [`lower_tree`](crate::stdcore::backends::lowering::lower_tree) — the lowering
  stage and its driver.
- [`crate::stdcore::backends::text`] — the plain-text backend, with
  [`TextBackend`](crate::stdcore::backends::text::TextBackend),
  [`TextEmitStage`](crate::stdcore::backends::text::TextEmitStage) and
  [`TextFinalStage`](crate::stdcore::backends::text::TextFinalStage).
- [`crate::stdcore::backends::html`] — the HTML backend, with
  [`HtmlBackend`](crate::stdcore::backends::html::HtmlBackend),
  [`emit_tree`](crate::stdcore::backends::html::emit_tree),
  [`HtmlEmitStage`](crate::stdcore::backends::html::HtmlEmitStage) and
  [`HtmlFinalStage`](crate::stdcore::backends::html::HtmlFinalStage); its typed
  sink [`HtmlSink`](crate::stdcore::backends::html::HtmlSink), and the classes and
  attributes a stylesheet may rely on, stated as data
  ([`crate::stdcore::backends::html::conventions`]).
- [`crate::stdcore::backends::latex_recomposed`] — the LaTeX backend, with
  [`LatexRecomposed`](crate::stdcore::backends::latex_recomposed::LatexRecomposed),
  [`emit_tree`](crate::stdcore::backends::latex_recomposed::emit_tree),
  [`LatexRecomposedEmitStage`](crate::stdcore::backends::latex_recomposed::LatexRecomposedEmitStage)
  and [`LatexRecomposedFinalStage`](crate::stdcore::backends::latex_recomposed::LatexRecomposedFinalStage);
  [`crate::stdcore::backends::latex`] — the LaTeX family the backend is built on,
  with its construct elements.
- [`crate::stdcore::backends::latex_rendered`] — the rendered LaTeX backend, with
  [`LatexRendered`](crate::stdcore::backends::latex_rendered::LatexRendered),
  [`emit_tree`](crate::stdcore::backends::latex_rendered::emit_tree) and its contract,
  stated as data ([`crate::stdcore::backends::latex_rendered::contract`]).
- [`Artifact`](crate::core::artifact::Artifact),
  [`ArtifactSet`](crate::core::artifact::ArtifactSet) and
  [`ArtifactNamer`](crate::core::artifact::ArtifactNamer) — what a run publishes,
  and how a host gives each artifact a path.

## Pieces

A piece is a document-level synthesized value that a backend places before or after
the document's content (the title, the list of footnotes, the bibliography), built
once by its builder stage into a table named for the piece and rendered by each
backend with its own rules.

The author does not write a piece where it appears. A stage of the feature that
offers the piece, its *builder*, builds it once over the whole document and writes it
into the piece's table. Other examples are a list of notes gathered from the text, a
page's authors, and the table of contents, a list of the document's headings that the
`toc` feature builds and that a render places only when asked (it is declared off by
default).

The content of a piece is a [synthesized value](#synthesized-values) made of
document elements, never a string or a tree of one backend's own. Each backend
brings it up to date ([value catch-up](#value-catch-up)) and lowers it with the
ordinary lowering rules, so one builder serves every backend, and a stage that
replaces the builder changes the piece on every output at once. The feature declares
the piece — its name, its table, its place, and whether a render places it by
default — into a service slot that every backend's pieces stage reads, and a render
option leaves a piece out or adds a piece declared as off by default.

- [`crate::core::piece`] — [`Piece`](crate::core::piece::Piece),
  [`PieceDecl`](crate::core::piece::PieceDecl) and
  [`PiecePlace`](crate::core::piece::PiecePlace).
- [`crate::stdcore::backends::pieces`] — the `pieces` feature, its render option,
  [`selected_pieces`](crate::stdcore::backends::pieces::selected_pieces) and the
  pieces stage of each backend,
  [`PiecesStage`](crate::stdcore::backends::pieces::PiecesStage).
- [the rendering chapter](crate::guide::rendering#pieces) — where the text and HTML
  backends place pieces.

## Features and packages

A feature is a Rust value that contributes a related bundle of things to a registry
(elements, stages, tables, lowering rules, backends, services, settings) through the
feature builder ([`FlmFeature`](crate::core::registry::FlmFeature)). A registry is
the frozen record of everything the language and the pipeline consult for a set of
features: the elements, stages, tables, capabilities, lowering rules, backends,
outputs, workflows, option types and services the features declared, built once and
shared by every document, plan and run ([`crate::core::registry`]).

A *package* is a named bundle of features together with the packages it builds on
([`PackageEntry`](crate::core::registry::PackageEntry)). A program chooses what FLM
can do by choosing which packages and features it registers.

The library ships two packages. The standard package `stdcore` holds the features that
have one canonical implementation, which every document is expected to use: the
language's constructs, the structure and the checks of a document, numbering,
references, floats, the standard backends and workflows. It reads where a formula
begins and ends, but processes no mathematics. That is the work of a *math processor*,
a package whose feature decides which commands a formula may hold, checks the formulas
and writes them into the outputs. Several math processors do the same job in different
ways, and each changes what a formula may legally contain, so the choice is made for
the document as a whole, never per workflow, and a registry holds exactly one. The
library's math processor, [`math-passthrough`](crate::math_passthrough), reads nothing
inside a formula and carries its source to the output; the extension `math-mathjax`,
which runs MathJax and accepts only the commands MathJax knows, is another. A host
chooses the default math processor for its documents, and the author may choose another
with the key `math` of the front matter.

The host library `flm-main` adds its own package `stdmain` (writing the output's files,
the live preview, the compose templates, image files and citations), and `std`, a
package with no features of its own that uses `stdcore` and `stdmain`. A document uses
the packages of an *initial list*, which the host gives and the document or the
command line may replace, then the package of its math processor, then those its front
matter lists under `use`
([the packages a document uses](crate::guide::command_line#the-packages-a-document-uses)).

The standard features have no special status: they register exactly as an added
feature does, through the same builder, with the same abilities. A program can
therefore add commands of its own, change how a standard element renders, add a
check, or add a whole output format, and it never needs an extension point made for
the purpose, because there is nothing the standard features can do that an added
feature cannot.

A feature is ordinarily constructed from a plain Rust value of its own, its
[configuration](#settings-options-and-configuration), so the compiler checks the
configuration rather than a schema. A feature reaches a registry in one of three
ways: added on its own with
[`RegistryBuilder::feature`](crate::core::registry::RegistryBuilder::feature); as part
of a package entry compiled into the program
([`RegistryBuilder::use_package`](crate::core::registry::RegistryBuilder::use_package));
or through a catalog that finds packages by name, for a program that decides its
features at run time.

A package found by name at run time may come from an [extension](#extensions): a set of
files, outside the program, that provides packages. An extension's *engine* reads it:
the engine `data` reads an extension written as data (settings only), the engine `js`
runs an extension written in JavaScript, and the engine `wasm` runs an extension
compiled to a WebAssembly module (from Rust, typically); the features of the last two
declare elements, stages and the rest through the same feature builder as a feature
written in Rust.

The registry build runs in five phases, in a fixed order, so that no feature needs
to know which other features are present or which registered first:

1. **Collect.** The features take their final order (see below). Every feature
   declares what it can declare by itself, in that order; the builder only records
   the declarations and resolves nothing. Then the [service
   slots](#services-and-service-slots) are assembled from the contributions.
2. **Finalize.** A feature's *finalizer* may read the assembled services and derive
   further declarations from what it finds.
3. **Validate.** Every check runs over the whole set of collected declarations:
   duplicate declarations, references to entities nobody declares, cycles. The peer
   declarations are resolved in the same phase (an unresolved one sets aside the
   lowering rules that depend on it and records a Note rather than refusing the
   build), and the fields registered for workflows by name
   ([`workflow_fields`](crate::core::registry::FeatureBuilder::workflow_fields)) are
   applied.
4. **Language.** The FLM language is built from the element declarations.
5. **Late initialization.** A feature's late-initialization build hook
   ([`LateInit`](crate::core::registry::LateInit)) runs once the language is finished;
   a feature compiles its author-written templates there.

Registration order still decides two things. Where two features define one command
name in one parsing mode, the definition of the feature registered last applies in
that mode. And elements, capabilities, tables, stages and rules are numbered in the
order they were declared, and that order breaks the tie when nothing else decides
between two stages a plan could run in either order.

The registration order is stated, not left to chance. The features are first added
in an *implicit order*: each package after the packages it uses, each package's
features in the package's order. *Chains* of the feature order then change it: the
chain `math-mathjax < phfqit` places the feature `math-mathjax` before `phfqit`
([`FeatureChain`](crate::core::registry::FeatureChain)). A package states the
chains it needs ([`PackageEntry::feature_order`](crate::core::registry::PackageEntry::feature_order)),
and the embedding program states those of a document
([`RegistryBuilder::feature_order`](crate::core::registry::RegistryBuilder::feature_order));
the build combines all of them. Each chain is first reduced to the features the
registry holds, so a chain that names a feature not in use says nothing about it.
The final order is a sort of the features in which each comes after every feature
the chains place before it; at each step, among the features whose predecessors are
all placed, the one that comes first in the implicit order is placed next. The sort
does not keep the implicit order of every pair the chains leave unordered: with the
implicit order `x`, `y`, `z` and the chain `z < x`, the final order is `y`, `z`, `x`.
Chains that place a feature before itself are an error.

The chains also decide when two features may define one command name. Each parsing
mode is decided on its own, with the features that define the name in that mode: the
collision is allowed when the feature placed last among them is placed, by the chains,
after every other of them; otherwise it is an error, unless the feature placed last
declares its definition as an override
([`FeatureBuilder::override_callable`](crate::core::registry::FeatureBuilder::override_callable)).
Only the override of the feature placed last is consulted. A definition for text mode
only and one for math mode only never collide. One chain settles every collision
between two features; a feature that wants finer control is split into smaller
features, ordered separately.

Wherever else the registration order picks one of several competing items that
different features contribute, the feature placed last wins too, and within one
feature its own order is kept: the owner recorded for an element value two features
declare, the stylesheet or script registered under one identifier, the LaTeX
definition piece contributed under one identifier, the first value of a service slot.
A service slot read for precedence, such as the citation providers or the image
converters, gives the values contributed to come first
([`contribute_first`](crate::core::registry::FeatureBuilder::contribute_first))
ahead of the others; within each of these two groups, the values of the feature
placed last come first, each feature's in the order it contributed them, and the
embedding program's values come last. The order does not settle competing stages or
competing lowering rules: workflows decide those.

- [`FlmFeature`](crate::core::registry::FlmFeature),
  [`FeatureInfo`](crate::core::registry::FeatureInfo) and
  [`FeatureBuilder`](crate::core::registry::FeatureBuilder) — writing a feature.
- [`PackageEntry`](crate::core::registry::PackageEntry),
  [`PackageFactory`](crate::core::registry::PackageFactory) and
  [`PackageCatalog`](crate::core::registry::PackageCatalog) — bundling features, and
  finding packages by name.
- [`SettingsDecl`](crate::core::settings::SettingsDecl) — a feature's settings given
  as data.
- [`RegistryBuilder`](crate::core::registry::RegistryBuilder) and
  [`FlmRegistry`](crate::core::registry::FlmRegistry) — the registration and its
  frozen result.
- [`FeatureChain`](crate::core::registry::FeatureChain) and
  [`crate::core::registry::order`] — the chains of the feature order and the final
  order of the features.
- [`Finalizer`](crate::core::registry::Finalizer) and
  [`LateInit`](crate::core::registry::LateInit) — declaring something that needs
  the assembled services or the finished language.
- [`crate::stdcore`] — the standard package.
- [`crate::math_passthrough`] — the library's math processor.

## Extensions

An *extension* is a set of files that a host finds by name while it processes a
document, without having been compiled with it, and that provides one or more packages.
Its *manifest*, a YAML file, says which *engine* reads it: the engine `data` reads an
extension written as data, whose one package holds settings, workflows and compose
templates; the engine `js` runs an extension written in JavaScript, and the engine
`wasm` an extension compiled to a WebAssembly module, whose packages have features with
behavior, declared through the same feature builder as a feature written in Rust. A document names a package of an extension with a `use` entry, `LOCATOR` or
`LOCATOR#PACKAGE`: the locator names the extension, never a path, and a missing
`#PACKAGE` means the default package `_`.

The library reads no file. The crate `flm-ext` holds the extension layer: the manifest's
reader, the grammar of a `use` entry, the engines, and a loader that a host adds to its
[`PackageCatalog`](crate::core::registry::PackageCatalog) as a
[`PackageResolver`](crate::core::registry::PackageResolver), over *sources* that the host
gives it. The `flm` program's source looks for an extension in the folders of its
search path: `_flmextensions` beside the document (or the folders given with
`--extensions-dir`), the folders of the environment variable `FLMPATH`, and
`~/.flm/extensions`.

The settings that a package of an extension writes are a source of settings of their
own, above the program's defaults and below the document's front matter. Between
packages, a package wins over the packages it uses, and two packages of which neither
uses the other must agree ([`crate::core::settings::cascade`]).

- [Extensions](crate::guide::extending#extensions) — writing an extension, in data, in
  JavaScript, or in Rust compiled to WebAssembly or into the program.
- [Where extensions are looked
  for](crate::guide::command_line#where-extensions-are-looked-for) — the search path of
  the `flm` program.
- [`SettingsDecl::config_data`](crate::core::settings::SettingsDecl::config_data) and
  [`SettingsDecl::options_data`](crate::core::settings::SettingsDecl::options_data) —
  settings declared from data, as a feature written in JavaScript declares them.

## Diagnostics

A diagnostic reports a problem with a document: a severity (Error, Warning or Note),
a position, and a typed condition with its data; its message is written for a person
and is not a stable interface.

A condition is the typed body of a diagnostic: a value that says what happened and
holds the facts describing it, identified as `flm.<feature>.<condition>`; any techy
condition type can be one. An example is `flm.blocks.nesting-too-deep`.

**No document content can make the library panic.** Every failure a document can
cause — malformed syntax, an unknown command, content nested deeper than the
[recursion guard](#the-recursion-guard-and-worker-threads) allows, a rule that
refuses — is a diagnostic or a typed error. A host renders documents it does not
control, and a panic there would crash the host instead of reporting a problem with
the document. Parsing follows the same principle: a problem becomes a diagnostic,
and the tree still covers the whole input. The only parses that return no tree are
the cases of [`FragmentError`](crate::lang::FragmentError), among them content nested
deeper than the recursion guard allows
([`FragmentError::NestingTooDeep`](crate::lang::FragmentError::NestingTooDeep)).

The identifier of a condition and its data fields are a stable interface that a caller
and a test may rely on; the message is not, and may be reworded at any time. The
diagnostics test helper ([`crate::dump::diag`]) therefore offers no way to match on the
text of a message: a test selects a diagnostic by its identifier and narrows the
selection by the source text it covers, its position, its severity or its data.

A diagnostic is kept by what produced it. The diagnostics of a parse belong to the
fragment, because they are facts about the text: a fragment placed in a document
more than once reports them once. The diagnostics of a run belong to the run.
Reading a run's diagnostics returns both in one sequence, the parse diagnostics of
each fragment first, in the order in which the document's walk reaches the fragments.

The severity decides whether a run performs effects outside the store: a run that
has recorded an error stops before the first stage that has such effects.

- [`FlmDiagnostic`](crate::lang::FlmDiagnostic),
  [`FlmDiagnostics`](crate::lang::FlmDiagnostics) and
  [`Severity`](crate::techy::error::Severity) — the diagnostic and its severity.
- [`Fragment::diags`](crate::model::doc::Fragment::diags) and
  [`DiagnosticsView`](crate::core::run::DiagnosticsView) — where the diagnostics of
  a parse and of a run are found.
- [`NestingTooDeep`](crate::lang::NestingTooDeep) and
  [`StructureError`](crate::lang::rewrites::blocks::StructureError) — two
  conditions, as examples.
- [`crate::dump::diag`] — asserting on diagnostics in a test.
- [`crate::guide::panics`] — the list of the items that can panic, and the
  requirement each of them states.

## Services and service slots

A service is a value that can be shared across threads which one feature supplies
for another feature, or for the embedding program, to use; the feature that owns the
interaction declares a service slot (a dotted name, the type of the values the slot
takes and its arity), any feature contributes values to it, and the registry build
assembles all contributions once.

A service is behavior behind a trait object — something that fetches citation data,
something that renders mathematics — or plain data that another feature reads. The
arity of a slot says whether it takes at most one value or any number of values. Any
feature may contribute to a slot through the feature builder, and the program that
embeds the library may contribute through the registry builder.

Declaring the slot makes the interaction safe: the feature that needs
something states what it needs and of which type, instead of asking at run time
whether some other feature happens to be present. Four rules complete the picture:

- **A peer contribution may go unused.** A contribution is a value a feature places
  in a service slot. A peer contribution is a contribution to a slot another feature
  owns, declared with
  [`FeatureBuilder::peer_slot`](crate::core::registry::FeatureBuilder::peer_slot);
  the registry build drops it with one Note when that feature is not registered. A
  contribution to a slot nobody declares is otherwise a build error.
- **A read returns an `Option`.** A stage reads a service from its context
  ([`StageCx::service`](crate::core::run::StageCx::service)), and an absent service
  is never a failure in the middle of a render: the stage reports it as a diagnostic
  of its own.
- **A slot may be required.** A feature that cannot work in any plan without a
  service marks the slot required
  ([`require_service`](crate::core::registry::FeatureBuilder::require_service)); an
  unfilled required slot is then an error when the registry is built.
- **Slots never change once the registry is built.** Every run shares the assembled
  services. A value that exists only after some work has been done — an index built
  by scanning a collection of documents — is not a service but an [external
  table](crate::core::run::Externals), installed when a run is constructed.

- [`crate::core::services`] — services, slots and contributions, with an example.
- [`FeatureBuilder::service_slot`](crate::core::registry::FeatureBuilder::service_slot),
  [`FeatureBuilder::contribute`](crate::core::registry::FeatureBuilder::contribute)
  and
  [`FeatureBuilder::peer_slot`](crate::core::registry::FeatureBuilder::peer_slot) —
  declaring a slot, contributing to one, and contributing to a slot another feature
  owns.
- [`StageCx::service`](crate::core::run::StageCx::service) — reading a service from
  a stage.

## Settings, options and configuration

A feature takes its values in three forms that differ by when they are fixed: its
configuration, a plain Rust value fixed when the registry is built; its option
values, set on a document or on a render request according to the option type's
binding; and its settings, the same values written as data (one map per feature,
from key to value), for example on a command line or in a document's front matter, by
a program that does not construct the Rust values itself.

A feature's configuration is the plain Rust value the feature is constructed from;
it is fixed when the registry is built and cannot vary per document or per render.

An option type is a plain Rust struct a feature declares once; its values are set on
a document or on a render request (the type's binding says which), and a stage reads
the value for an option type it declared a read of: the render value if one is set,
otherwise the document value, otherwise the type's default
([`crate::model::options`]). The binding ([`Bind`](crate::model::options::Bind))
is one of four: fixed with the registry, set on the document, set on the render
request, or set on the document and overridden on a render request. A value set at
a place the binding does not admit is an error, and so is a read the stage did not
declare.

Settings are a feature's configuration and option values written as data, one map
per feature from key to value, which the library reads into the feature's own Rust
types; a settings key is one field of those types ([`crate::core::settings`]). The
settings add no second definition of anything: the keys, their defaults and their
documentation are those of the Rust types. A feature states the declaration of its
settings once ([`SettingsDecl`](crate::core::settings::SettingsDecl)), and the
registry build registers the feature's option types from it, each with its binding.

The library reads one map per feature, already combined. A host decides where the
settings come from; the `flm` program, for example, combines settings from its
defaults, the packages of the [extensions](#extensions) a document uses, the front
matter, the workflow and the command line. The library's settings cascade
([`crate::core::settings::cascade`]) combines such sources by rank, each value keeping
where it was written, with the rule between packages. A feature without Rust types for
its settings declares them from a JSON Schema, and its values are read as data
([`DataValue`](crate::model::options::DataValue)), an option type of it being named by
a name rather than a Rust type ([`OptionKey`](crate::model::options::OptionKey)).

- [`crate::core::settings`] — settings as data, and
  [`SettingsDecl`](crate::core::settings::SettingsDecl), a feature's declaration of
  them.
- [`crate::model::options`] — option types and option values, and
  [`Bind`](crate::model::options::Bind), the four bindings.
- [`crate::core::settings::cascade`] — combining settings from ranked sources.

## Output regions and the final stage

An output region is one named part of an output's entry artifact (the document's
content, each piece placed beside it, and on HTML the stylesheets, the scripts, the
title and the language), which the backend writes once into the table
`flm.output.<backend>.regions`; the backend's final stage assembles the entry
artifact from those regions in a layout of its own, and a host may assemble the same
regions through a template of its own instead.

The final stage publishes the assembled entry artifact as the output's artifact set.
A host that assembles the regions through a template of its own (the `flm` program's
compose feature does) registers a stage that provides the final stage's capability,
and the plan then schedules the host's stage in place of the backend's final stage.

The regions table also states what the entry artifact is — its identity, its media
type and the artifacts it refers to — so that every assembler makes the same
artifact from one statement, and nothing is computed twice. A region is a list of
parts, each a text the backend wrote.

A backend whose output is made of regions says so with a regions output: a regions
output is a backend's statement that its output is made of regions (which table
holds them and which capability its final stage provides), contributed to the
service slot `flm.output.regions` so that a host's own assembler can read it
([`RegionsOutput`](crate::core::region::RegionsOutput)). A host's assembler reads
that slot to learn which outputs it can serve, and so serves exactly the outputs of
the backends a registry holds.

- [`crate::core::region`] — the names of the regions, and
  [`Regions`](crate::core::region::Regions), the regions of one output.
- [`RegionsOutput`](crate::core::region::RegionsOutput) — a backend's statement that
  its output is made of regions.
- [`ArtifactSet`](crate::core::artifact::ArtifactSet) and
  [`ArtifactSet::insert_entry_with`](crate::core::artifact::ArtifactSet::insert_entry_with)
  — the artifact set a final stage publishes, and how its entry artifact is added.

## Reference points, labels and anchors

A reference point is one place in a document that can be referred to, numbered, or
given an anchor (a heading, an equation line, a theorem, a footnote); a label is the
text an author attaches to it with `\label{type:label}`; an anchor is the identifier
a link points at, computed once for every point; and a reference site (`\ref`,
`\hyperref`, a proof's back-reference, a term link) is resolved against the points.

An element declares its reference points per occurrence through the
[`Referenceable`](crate::model::aspects::Referenceable) aspect, and the `refpoints`
feature records them for every tree in the table
[`REFPOINTS`](crate::stdcore::features::refpoints::REFPOINTS)
([`RefPoint`](crate::model::aspects::RefPoint)).

A label is written after a construct, which reads it as a
[field](#fields-data-a-construct-reads) and attaches it to its reference point; the
label type, the part before the colon, must be one the point accepts, and a label
may be written only once in a document. A construct that accepts labels declares the
tack-on argument [`label_fields_arg`](crate::stdcore::features::refs::label_fields_arg)
and reads its labels with [`label_uses`](crate::stdcore::features::refs::label_uses).

The element of a reference point says how its anchor is formed
([`AnchorPolicy`](crate::model::aspects::AnchorPolicy)): `<type>-<label>` from the
point's first label; `<prefix>--<title>` from a heading's title when it has no label;
`<prefix>-<number>` from the number the document gave the point; or, for an
unlabeled equation line, `<prefix>--<position><four letters>` from its position. A
point whose policy yields no identifier (a point with no label under the label-only
policy, or the second of two labels that encode alike) has no anchor. The `refs`
feature computes the anchors once, in its anchors stage, and the backends write them
as link targets ([`ANCHORS`](crate::stdcore::features::refs::ANCHORS)).

A reference site is declared per occurrence through the
[`Referencing`](crate::model::aspects::Referencing) aspect and resolved by the `refs`
feature ([`RefSite`](crate::model::aspects::RefSite)).

- [`RefPoint`](crate::model::aspects::RefPoint) and
  [`RefSite`](crate::model::aspects::RefSite) — a reference point and a reference
  site.
- [`label_fields_arg`](crate::stdcore::features::refs::label_fields_arg) and
  [`label_uses`](crate::stdcore::features::refs::label_uses) — declaring the
  `\label` field and reading it.
- [`ANCHORS`](crate::stdcore::features::refs::ANCHORS) — the table of anchors.
- [`REFPOINTS`](crate::stdcore::features::refpoints::REFPOINTS) — the table of
  reference points.
- [`crate::stdcore::features::refs`] — the feature that resolves references.

## Counters and counter styles

A counter is a dotted name under which things are counted in document order,
declared by a feature together with the counter it is numbered within, and a counter
style says how a counter value is shown; a reference point states its number source
(the counter and the counter style it is numbered by), and the numbering feature's
one counting stage records every point's counter value.

A counter value is a number with optional sub-numbers under it. A counter style
holds the number template, the sub-levels, the prefix words with their variants, the
delimiters and the texts that join several numbers, each text written in FLM and
parsed once, when the counter style is compiled
([`CounterStyleDecl`](crate::stdcore::features::numbering::CounterStyleDecl)); the
number template
([`NumberTemplate`](crate::stdcore::features::numbering::NumberTemplate)) is written
in the [template language](#the-template-language-shared-by-features) (`#{arabic}`)
or as a single letter standing for its number format (`a`, `i`, `1`). A
counter is numbered within its parent counter through a
[`Within`](crate::stdcore::features::numbering::Within), which also says how the
parent's number is written before the counter's own: a joining text, a within
template, nothing, or a joiner registered by name. The numbering feature records every
point's counter value in the counter table `flm.counters`
([`COUNTERS`](crate::stdcore::features::numbering::COUNTERS)).

A feature declares its counters and counter styles through [peer
contributions](#services-and-service-slots) to the numbering feature's service slots,
so the declaring feature still registers where no numbering feature is present.

- [`crate::stdcore::features::numbering`] — the numbering feature.
- [`CounterDecl`](crate::stdcore::features::numbering::CounterDecl) and
  [`CounterStyleDecl`](crate::stdcore::features::numbering::CounterStyleDecl) —
  declaring a counter and a counter style.
- [`NumberSource`](crate::model::aspects::NumberSource) — what a reference point is
  numbered by.
- [the numbering chapter](crate::guide::numbering) — counter styles and the numbering
  feature in full.

## Scoped packages

A scoped package is a set of element declarations that can be written only inside
the environment bodies and arguments whose declarations refer to the package by name,
for instance `\item` inside a list's body or `\caption` inside a float's body; a
feature declares one.

A body or an argument may refer to a scoped package that another feature declares.
The referring feature then makes a peer declaration of the package's name, so that
a registry without the owning feature still builds: the referring bodies and
arguments then make no command available through that package.

- [`ScopedPackageDef`](crate::lang::mint::ScopedPackageDef) — the declaration of a
  scoped package.
- [`FeatureBuilder::peer_scoped_package`](crate::core::registry::FeatureBuilder::peer_scoped_package)
  — the peer declaration of a scoped package another feature owns.

## Structural elements

The structural elements are the elements of the node shapes no feature declares:
text, comments, containers, groups of each class, paragraphs, paragraph breaks,
template placeholders, and the macros and environments a parse could not attribute to
any feature; there is one element value per shape, every backend's [native
set](#backends-lowering-and-outputs) holds the ones it writes as they are, and the
lowering feature registers their default rules.

Because of the structural elements, every node has an element: every node shape has
one, and giving a node its element never needs a name lookup. The unknown
macros and environments carry the elements `flm.unknown.macro` and
`flm.unknown.environment`. The structural elements are ordinary element values, so a
feature may attach aspects to them. Each standard backend lists the structural
elements it emits natively, without a lowering rule, in a `FLOW_THROUGH` constant (for
example [`text::FLOW_THROUGH`](crate::stdcore::backends::text::FLOW_THROUGH)).

- [`StructuralElements`](crate::lang::structural::StructuralElements) — the set of
  structural elements, and [`crate::lang::structural`], their names.
- [`LoweringFeature`](crate::stdcore::backends::lowering::LoweringFeature) — the
  feature that registers their default lowering rules.

## The recursion guard and worker threads

The recursion guard refuses deeply nested content with the condition
`flm.blocks.nesting-too-deep` instead of overflowing the stack; because the guard
compares the stack a run has used with a fixed budget and does not check how large the
thread's stack is, every guarded run (a parse, a rewrite, an emit) must execute on a
worker thread, a thread created with at least `WORKER_STACK_SIZE` bytes of stack, which
the library's entry points create when the calling thread is not one.

Parsing, walking and rewriting a tree are recursive, so deeply nested input uses
stack. The guard measures how much stack a recursive run has used and refuses to
descend one level further once its budget is spent; the alternative would be a stack
overflow, which crashes the process. On a thread with a smaller stack than the
budget, a guarded run would overflow before the guard refused, which is why the
worker-thread requirement exists. A refusal is never a panic: a parse returns no tree
and reports the condition, and a rewrite pass publishes nothing for the tree it was
working on.

- [`crate::lang::guard`] — the guard and the worker-thread requirement in full.
- [`on_worker_thread`](crate::lang::guard::on_worker_thread) — runs work on the
  current thread when it is a worker thread, otherwise on a worker thread it creates.
- [`run_with_worker_stack`](crate::lang::guard::run_with_worker_stack) — runs work on
  a new worker thread.
- [`WORKER_STACK_SIZE`](crate::lang::guard::WORKER_STACK_SIZE) — the stack size of a
  worker thread.

## The template language shared by features

The template language is FLM text with placeholders (`#1`, `#{name}`, `#{body}`,
`#[…]{ref}`), compiled once when the registry is built; a user definition's template
is written in it, and so are the numbering feature's number templates and within
templates, the list label templates, the heading title templates, the theorem heading
templates and the defterm lead-in, each with its own placeholder vocabulary.

A placeholder is a reference in a template to an argument, the body, or a name whose
content an expansion hook computes; at expansion it is replaced by that content's
nodes and nothing else. Each feature that uses the template language says which
placeholders its templates may contain.

- [`crate::lang::rewrites::userdefs::template`] — the template language, compiled.
- [`NumberTemplate`](crate::stdcore::features::numbering::NumberTemplate) and
  [`Within`](crate::stdcore::features::numbering::Within) — the numbering feature's
  number templates and within templates.
- [the template language](crate::guide::language_syntax#the-template-language) in the
  syntax chapter — the language as an author writes it.

## Decorations, roles and kinds

A decoration is a node whose element carries the `Decoration` aspect (a run-in
heading, a margin note, a closing mark), which the lowering driver moves out of its
content run into an attached slot of the enclosing presentation node, leaving a
`flm.pres.deco-mark` node where it was; a role and kinds are feature-owned names on a
presentation payload that say what a container is, so that a stylesheet or a host's
lowering rule can select on them.

An attached slot is a slot of a presentation node, named after a decoration's element,
that holds the decoration nodes the lowering driver moved out of the node's content; the
attached slots follow the content slots, the named groups of the node's children
([`read`](crate::stdcore::backends::pres::read)). The `flm.pres.deco-mark` node, the
decoration mark, records which attached slot holds the decoration, and an emitter writes
the decoration at the mark's position. A backend that writes any decoration element
natively must therefore write `flm.pres.deco-mark` natively too, which the registry
build checks.

A role is a feature-owned dotted name on a presentation payload that says what a
container is (a theorem's heading, a quotation's attribution), and kinds are
feature-owned names that classify it further. A document role
([`DocRole`](crate::stdcore::backends::pres::DocRole)) is different: it is one of a
small fixed set of structural roles (a list of notes, a link to a cited work) that a
backend with such a notion renders.

- [`Decoration`](crate::stdcore::backends::pres::Decoration) — the aspect that makes
  an element a decoration.
- [`crate::stdcore::backends::pres::payload`] — the payloads of the presentation
  elements, with their role and kind fields.
- [`crate::stdcore::backends::pres::read`] — reading a presentation node's content
  slots and attached slots.
- [`DocRole`](crate::stdcore::backends::pres::DocRole) — the document roles.

## Style and script definitions

A style definition or a script definition is a stylesheet or a script that a
feature's output depends on, recorded in the registry under an identifier; a lowering
rule or an emitter requires it by identifier, and the backend that writes the output
decides its delivery: written into the output, loaded from an address, or left to the
host.

A requirement is an identifier a lowering rule or an emitter records to say that its
output needs a definition: a style or script definition on HTML, a package request or
a definition piece on LaTeX; the backend that writes the output resolves the
requirements recorded for a lowered tree. Two features that require the same
definition require it once, because both use its identifier. A backend that has no
use for a stylesheet, such as the plain-text one, records the requirements and reads
none.

- [`StyleDef`](crate::core::registry::StyleDef) and
  [`ScriptDef`](crate::core::registry::ScriptDef) — a style definition and a script
  definition.
- [`DEFINITIONS_TABLE`](crate::stdcore::backends::html::DEFINITIONS_TABLE) — the
  table in which the HTML backend lists the definitions a document requires, each with
  its delivery.

## Content pointers

A content pointer is the payload of a synthesized node that shows, where the node is
placed, content written elsewhere in the tree (an xnote's content in the list of
xnotes, a term in its definition's lead-in, a theorem's title) without copying it:
the address of the node the content belongs to and the node region that holds it,
lowered once, where the pointer is placed.

A node region is one of a node's arguments or one of its slots
([`NodeRegion`](crate::core::lowering::NodeRegion)). Because the content is lowered
where the pointer is placed and not copied, it is shown as the document's own content.

- [`ContentPointer`](crate::stdcore::backends::lowering::ContentPointer) — the
  payload.
- [`lower_content_pointer`](crate::stdcore::backends::lowering::lower_content_pointer)
  — lowering the content a pointer shows.
- [`NodeRegion`](crate::core::lowering::NodeRegion) — an argument or a slot of a
  node.

## LaTeX package requests, definition pieces and hooks

The LaTeX family ([`crate::stdcore::backends::latex`]) is what every LaTeX backend
shares: the construct elements, the escaping of document text, and the service slots
that resolve package requests and definition pieces, registered once by the `latex`
feature. A lowering rule of a
LaTeX backend requires, by identifier, the LaTeX packages
(`flm.latex.pkg.<name>`) and the definition pieces (`flm.latex.def.<name>`) that its
output needs; the style file written beside each LaTeX output holds exactly what the
document required. Among the definition pieces of the recomposed LaTeX are the LaTeX
hooks, macros the recomposed LaTeX writes and the style file defines with a plain
default, which a user redefines in the preamble; the definition pieces of the rendered
LaTeX define the `\flmx…` commands of its contract.

A package request is a LaTeX package that a lowering rule or an emitter requires by
identifier (`flm.latex.pkg.<name>`), resolved through the service slot
`flm.latex.packages` to a package definition and merged with every other request for
that package into one `\RequirePackage` line per package and LaTeX engine (pdfLaTeX,
LuaLaTeX, XeLaTeX; [`ENGINES`](crate::stdcore::backends::latex::packages::ENGINES))
([`crate::stdcore::backends::latex::packages`]).

A definition piece is a unit of LaTeX definitions that a feature registers under an
identifier `flm.latex.def.<name>`, together with the identifiers it needs in turn; a
lowering rule requires it by identifier, and the style file holds the text of every
piece the document requires, in registration order
([`LatexPreambleDef`](crate::stdcore::backends::latex::LatexPreambleDef)).

A definition piece may also **deliver** a feature's style identifier, the identifier
under which the feature registers its stylesheet for HTML (`flm.theorems.style`)
([`delivers`](crate::stdcore::backends::latex::LatexPreambleDef::delivers)). A feature's
general lowering rule requires that identifier wherever it writes the feature's content,
whatever the backend; the HTML backend writes the stylesheet, and the rendered LaTeX
writes every definition piece that delivers the identifier, so that the feature's LaTeX
definitions are in the style file exactly when the document holds the feature's content.
The recomposed LaTeX does not read `delivers`: its own lowering rules require their
pieces by name.

A LaTeX hook is a macro that the recomposed LaTeX writes and the style file defines
with a plain default (`\flmHeading`, `\flmLabels`, `\flmRef`, `\flmHyperref`, and one
or more per feature such as `\flmXN<kind>`), which a user redefines in the document's
preamble to change what the recomposed LaTeX does at that point
([`crate::stdcore::backends::latex::preamble`]). The four hooks of the LaTeX family
serve these purposes:

- `\flmHeading` makes a heading's labels refer to the heading: by its number where
  the document class numbered it, by its title where it did not.
- `\flmLabels` writes a group of labels, and makes several labels on one line of a
  display-math environment all resolve.
- `\flmRef` writes a reference's options as the document wrote them, and its default
  translates them for the package that sets the reference.
- `\flmHyperref`, the hook of a reference that shows the author's own words, links
  those words through the `hyperref` package where the document loaded it and prints
  them alone where it did not, so the recomposed document compiles either way.

A user may redefine a LaTeX hook in the preamble, or turn the heading hook and the
label hooks off with the render options
[`heading_hook`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::heading_hook)
and
[`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks),
to get plain LaTeX back.

- [`crate::stdcore::backends::latex`] — the LaTeX family.
- [`crate::stdcore::backends::latex::packages`] — package requests.
- [`crate::stdcore::backends::latex::preamble`] and
  [`LatexPreambleDef`](crate::stdcore::backends::latex::LatexPreambleDef) —
  definition pieces and the LaTeX hooks.
- [`crate::stdcore::backends::latex::style`] — the style file.
