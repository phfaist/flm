# Migrating from Python FLM

This chapter is for a reader who knows the Python implementation of FLM, the
first one (its documents, its `flm` program, its Python interface or its
features), and moves to this library. It gives the correspondence between the
Python concepts and this library's items, and it lists every difference in
behavior between the two implementations: first the language and each feature,
then the four output formats, then the command line. A difference in reporting
is given by severity (Error, Warning or Note) and by the identifier of the
condition reported, `flm.<feature>.<condition>`
([diagnostics](crate::guide::concepts_overview#diagnostics)).

This chapter is the only place where a difference from the Python implementation
is recorded. The other chapters state this library's rules on their own terms,
and the ones that use a name carried over from the Python implementation point
here. Where this chapter lists no difference, the other chapters describe this
library's behavior and make no claim about the Python implementation's. The
wording of a diagnostic message is not compared: a message is written for a
person and is not a stable interface, so a message worded differently is not a
difference.

## The concept map

| Python concept | This library |
|---|---|
| the environment: what a document may be written in | the [registry](crate::core::registry), built once from the features a program registers |
| a feature | an [`FlmFeature`](crate::core::registry::FlmFeature), which declares elements, capabilities, tables, stages, lowering rules, backends, outputs, workflows, option types and service slots |
| a macro or environment specification | an [`ElementDef`](crate::lang::mint::ElementDef) with a [`SyntaxDef`](crate::lang::mint::SyntaxDef), plus the [`ElementSpec`](crate::model::element::ElementSpec) it carries |
| a fragment | a [`Fragment`](crate::model::doc::Fragment) |
| a document | a [`Doc`](crate::model::doc::Doc), plus a [`Run`](crate::core::run::Run) that processes it |
| the render context | the [`Run`](crate::core::run::Run), and the [`StageCx`](crate::core::run::StageCx) through which a stage works |
| a fragment renderer | a [backend](crate::core::registry::BackendDef): the elements its emitter writes natively, a lowering stage, an emit stage and a final stage; the features supply the [lowering rules](crate::core::lowering::LoweringRule) |
| the pure-LaTeX recomposer (`FLMPureLatexRecomposer`) and the `flmlatex` workflow | the [`latex-recomposed`](crate::stdcore::backends::latex_recomposed) backend and workflow (see [Recomposing to LaTeX](#recomposing-to-latex)) |
| the HTML fragment renderer (`HtmlFragmentRenderer`) and its options | the [`html`](crate::stdcore::backends::html) backend and workflow, whose classes and attributes are a new design (see [The HTML output](#the-html-output)) |
| `substmacros` | [user definitions](crate::lang::rewrites::userdefs) |
| the `endnotes` feature and its categories (`EndnoteCategory`) | the [`xnotes`](crate::stdcore::features::xnotes) feature and its xnote kinds ([`XnoteKind`](crate::stdcore::features::xnotes::XnoteKind)); see [Footnotes and other xnotes](#footnotes-and-other-xnotes) |

For each row of the table, the two implementations differ in structure, not
only in names. The paragraphs below say what each item of this library is.

**The registry.** A registry is the frozen record of everything the language and
the pipeline consult for a set of features: the elements, stages, tables,
capabilities, lowering rules, backends, outputs, workflows, option types and
services the features declared, built once and shared by every document, plan
and run ([`crate::core::registry`]). Building the registry is a step of its own,
done before any document is read: the parser is derived from the element
declarations of the registry, so what an author may write is fixed before the
first character of a document is read. A program chooses what FLM can do by
choosing which packages and features it registers. The standard features have no
special status: they register through the same builder as a feature a program
adds. See [features and
packages](crate::guide::concepts_overview#features-and-packages).

**A fragment** is one piece of source as the parser delivered it: its node tree,
its source, the diagnostics of its parse, and the layout of its top level
([`Fragment`](crate::model::doc::Fragment)). It holds no processing state and no
rendered form. A document holds it through a shared pointer, so one fragment may
be placed in several documents at once.

**A document** is its content (the tree arena, the order in which the trees are
walked, and the map that says which node places which other tree) and the option
values set on it, and nothing else ([`Doc`](crate::model::doc::Doc)). A document
holds nothing that a stage computed: results belong to the
[`Run`](crate::core::run::Run) that computed them. One document may therefore be
checked and then rendered, or rendered to two formats, and nothing about it
depends on which processing is under way.

**A run** is one execution of one plan over one document; it owns the frozen
trees, the computed tables and the diagnostics ([`Run`](crate::core::run::Run)).
It does the work that the Python render context did, in another way. Python
renders by a recursion that carries a mutable context down the tree. Here the
trees are first rewritten (user definitions expanded, content grouped into
paragraphs and blocks) and then frozen. Everything computed after that is stored
beside the trees in typed [tables](crate::core::data::table::Table): each table
is written by exactly one stage and read by its registered name. A stage reads
the document through a [`StageCx`](crate::core::run::StageCx), which gives it the
frozen trees, the tables it declared that it reads, its option values and the
registry's [services](crate::core::services), and it returns what it computed
rather than changing anything shared. See [frozen trees and computed
data](crate::guide::concepts_overview#frozen-trees-and-computed-data).

**A backend** takes the place of the fragment renderer. A backend is the
registration bundle for one output format: the elements its emitter writes
natively, a lowering stage, an emit stage and a final stage
([`BackendDef`](crate::core::registry::BackendDef)). The work of the Python
renderer is split in two:

- *Lowering* rewrites each frozen tree into a presentation tree for one backend,
  made of the shared [presentation elements](crate::core::registry::FeatureBuilder::presentation_element)
  (headings, paragraphs, links, text styles, verbatim regions), by applying the
  [lowering rule](crate::core::lowering::LoweringRule) selected for each element
  ([backends, lowering and
  outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)).
- The *emit stage* (the emitter) walks the lowered trees and writes the output's
  bytes.

A feature that adds an element supplies a lowering rule for it; only a backend
supplies an emitter. This library has four backends:
[`text`](crate::stdcore::backends::text), which prints plain text through the
[layout engine](crate::layout);
[`latex-rendered`](crate::stdcore::backends::latex_rendered), which writes the
document as resolved LaTeX for LuaLaTeX;
[`latex-recomposed`](crate::stdcore::backends::latex_recomposed), which writes
the document back as LaTeX source; and [`html`](crate::stdcore::backends::html),
which writes the document as a page for a browser.

**Which stages run is computed, not written by hand.** A stage is one unit of
work of the pipeline ([`Stage`](crate::core::stage::Stage)); it declares the
*capabilities* it provides and the capabilities it requires (a capability is a
registered dotted name for a state of processing that a stage brings about,
[`CapabilityDef`](crate::core::registry::CapabilityDef)). A caller demands the
capabilities it wants, usually by choosing a
[workflow](crate::core::registry::WorkflowDef) (a named set of demands together
with the choices that go with them, such as `check` or `html`), and the library
computes the plan, the list of stages that provides them
([`Plan`](crate::core::plan::Plan)).
See [stages, capabilities and
plans](crate::guide::concepts_overview#stages-capabilities-and-plans) and [the
embedding chapter](crate::guide::embedding).

**What a node means is its element.** An element is the value that says what a
node means: a declaration with a stable dotted type name
([`ElementSpec`](crate::model::element::ElementSpec)). Behavior is attached to an
element as an *aspect*: a piece of behavior attached to an element when the
feature registers it and read back through a trait object, so that a stage asks
the element what it does instead of matching its name against a fixed list. A
feature ported from Python declares elements and aspects where the Python feature
registered a handler with a renderer for each node type. See [elements and
aspects](crate::guide::concepts_overview#elements-and-aspects).

## User definitions and the template language

A user definition is a macro, environment or special character sequence that a
document's author or the host defines by substitution: a name, an argument
syntax, a template and the parsing modes it is defined for
([`Definition`](crate::lang::rewrites::userdefs::Definition)). Its template is
written in the [template language](crate::lang::rewrites::userdefs::template),
FLM text with placeholders (`#1`, `#{name}`, `#{body}`). The Python
implementation calls the same construct `substmacros`.

User definitions are part of the configuration of the
[`userdefs`](crate::stdcore::features::userdefs) feature,
[`UserdefsConfig`](crate::stdcore::features::userdefs::UserdefsConfig): a
feature's configuration is the plain Rust value the feature is constructed from,
fixed when the registry is built ([settings, options and
configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
A document cannot define a macro in its body (there is no `\newcommand`),
because the parser is derived from the registry, so a definition must exist
before the document is read.

In settings written as data ([`settings`](crate::core::settings): a feature's
configuration and option values written as one map per feature, from key to
value), the settings of the `userdefs` feature are read as a
[`UserdefsData`](crate::stdcore::features::userdefs::UserdefsData), and the
definitions are written as in the Python implementation, in one map per kind:
`macros`, `environments` and `specials`, each from a name (for a special
character sequence, its characters) to the definition. The keys of a definition
are this library's ([`DefinitionData`](crate::stdcore::features::userdefs::DefinitionData)).
Its `content` is either one text, which is the template in both parsing modes
and applies in each mode where it parses, or a map with the keys `text` and
`math`, one or both written, where the Python implementation writes `textmode`
and `mathmode`:

```yaml
userdefs:
  macros:
    hi: { content: "Hello, #1!", args: ["{"] }
    vec: { content: { text: "arrow", math: "\\vec{#1}" }, args: ["{"] }
  environments:
    note: { content: "#{body}" }
```

A definition's `args` may be written with the Python implementation's argument
codes (`{` or `m`, `[` or `o`, `*` or `s`, `t<c>`, `r<c1><c2>`, `d<c1><c2>`,
`v`, `v<c1><c2>`, `e{<chars>}`, `AnyDelimited`, `AnyDelimitedOptional`; the
[table of argument codes](crate::lang::mint::codes#the-codes) gives each one),
and in the same three shapes: one text, each character one code (`"[{{"`); a
list of codes; or a list whose entries are each a code or a map. The keys of
such a map are this library's: `code` and `name`, where the Python
implementation writes `parser` and `argname`, beside `layout` and `docs`. The
number of leading arguments that `#1` skips is the definition's key
`argument_number_offset`, not a marker written among the arguments. Besides the
Python implementation's codes, this library accepts the code `BracedOnly`, taken
from techy, the parser toolkit FLM is built on: a mandatory `{…}` group, never a
single token; and four codes of its own for an argument that runs up to a
terminator: `u<terminator>` (document content up to `<terminator>`, as in
`\phf … \endphf`), `U<terminator>` (raw text up to it), `f<c><terminator>` and
`F<c><terminator>` (the same after a run of the character `<c>`, up to the same
run followed by `<terminator>`).

The codes are shorthands for this library's **structured syntax**, which the
Python implementation does not have: a map whose key `syntax` holds the name of
a standard kind of argument and whose other keys hold the kind's parameters,
beside the argument's own keys:
`{syntax: delimited-group, delimiters: ["(", ")"], name: size}` declares the
argument that `{code: "r()", name: size}` declares. Every code has its
structured syntax (the table of argument codes gives it), and three forms have
a structured syntax but no code: a group read as plain characters
(`chars-group`), a group of `key=value` pairs (`key-vals`), and a terminated
argument with an opener or with fixed text before the repeated run of its
terminator (`terminated` with `opener` or `head`). The kinds and their
parameters are listed in [the kind as
data](crate::lang::mint::ArgStdKind#the-kind-as-data). A map gives exactly one
of `syntax` and `code`.

A plan expands the use sites of a definition with the [expand
pass](crate::lang::rewrites::userdefs::expand), which replaces every use site of
a user definition by its template instantiated with the use site's arguments.
It is one of the two rewrite passes that run before the trees are frozen (the
other is the block-structure pass, which groups content into paragraphs and
blocks). A plan for the recomposed LaTeX keeps the use sites of most definitions
instead and writes them as calls (difference 2 below). A placeholder is replaced
by the content of its argument, regardless of the form the argument was declared
with: a verbatim argument gives the characters between its delimiters, so
`\c{x%y}`, with the template `see #1`, gives the text `see x%y`.

### The three tests a template can write

The template language has three families of conditional commands. They are easy
to confuse, and difference 4 below concerns them. Each takes as its first
argument the thing it tests: one placeholder for the *provided* and *value*
tests, any content for the *blank* test.

- **provided**: `\IfProvidedT`, `\IfProvidedF`, `\IfProvidedTF`, and the
  synonyms `\IfBooleanT/F/TF`. The test holds when the use site *wrote*
  something for this argument (a `*` marker, an optional argument in brackets,
  a mandatory group). A declared default value does not make it hold.
- **value**: `\IfValueT/F/TF` and their negations `\IfNoValueT/F/TF`. The test
  holds when the placeholder has something to substitute. An argument the use
  site left out but that has a declared default value *has* a value, because a
  default is bound before the conditionals are evaluated.
- **blank**: `\ifblank` and its negation `\notblank`. The test holds when its
  argument, with its placeholders substituted, is empty or holds nothing but
  comments. Whitespace is content, so `\m{ }` is not blank. The test sees a
  declared default value, for the same reason as the *value* test.

### Differences from `substmacros`

The command names of the template language are the Python implementation's, so
the differences below are places where a familiar name behaves differently.

1. **A template uses the registry's commands.** A template is parsed once, when
   the registry is built, so the commands it can use are those of the registry.
   The Python implementation parses a template again at each use, in the scope
   of the use site.
2. **No substitution flag at render time.** Whether a use site is expanded is
   decided by the definition's expansion policy
   ([`ExpandPolicy`](crate::lang::rewrites::userdefs::ExpandPolicy): `always`,
   expanded in every plan, or `per-plan`, the default, as each plan decides)
   and, for a `per-plan` definition, by the lowering rule that a plan selects
   among the definition's three: `expand-in-tree`, `emit-call` and
   `instantiate-at-lowering`
   ([`userdefs::kept`](crate::stdcore::features::userdefs::kept)). A `per-plan`
   definition is expanded on the plain-text and HTML outputs; on the recomposed
   LaTeX it is kept, and each use site is written as a call of the definition
   (see [Recomposing to LaTeX](#recomposing-to-latex)).
3. **A substitution argument on an argument placeholder is refused when the
   registry is built.** A substitution argument (`#[…]{name}`) is data for a
   placeholder whose content an expansion hook computes (an expansion hook is
   the code of a feature that supplies the content of a named placeholder when a
   use site is expanded; see
   [`expand`](crate::lang::rewrites::userdefs::expand)); a numbered or named
   argument placeholder and `#{body}` accept none. The Python implementation
   discards it silently.
4. **A declared default value is bound first.** An argument the use site left
   out but that has a declared default value therefore has a value for
   `\IfValue` and `\IfNoValue`, and `\ifblank` tests the default value. The
   Python implementation tests the argument as the use site wrote it, in all
   three families. The *provided* family exists under a name of its own for this
   reason: it tests what the use site wrote.
5. **`\ifblank` may test any content**, not only a placeholder. Content that is
   not a placeholder is tested as it is: anything but comments is content, hence
   not blank. The Python implementation raises an error for an argument that is
   not a placeholder.
6. **One template, and the parsing modes it applies in.** A definition has one
   template and a setting of the parsing modes it is defined for
   ([`DefinitionModes`](crate::lang::rewrites::userdefs::DefinitionModes)), both
   modes by default. The Python implementation's scalar `content`, which serves
   every mode, is that default. A definition given one text applies in each mode
   where its text parses: a template holding a command available only in
   formulas, such as `\lvert{#1}\rangle`, applies in formulas, and a use of it
   in running text is an Error reported at that use, while the rest of the
   document is processed. The Python implementation fails at that use too, on
   the formula command in the substituted text. A one-text template that parses
   in neither mode is refused when the registry is built, and so is a template
   declared for one mode that does not parse in that mode.

   The Python implementation's dict form `{'textmode': …, 'mathmode': …}` is two
   definitions of one name in one configuration, each restricted to one mode; in
   data it is written `{text: …, math: …}` and read as those two definitions. A
   definition restricted to one mode is not defined in the other mode. A
   mathematics-only definition used in running text is therefore an unknown
   macro, reported by the parse like any unknown command and kept as written. A
   text-only definition used inside mathematics is an unknown command of
   mathematics, which the
   [`math-passthrough`](crate::math_passthrough) feature keeps
   as written without a report, as it keeps every command it does not know. The
   Python implementation raises an error for a text-only definition used inside
   mathematics and stops rendering.
7. **Numbered default keys follow the placeholders' numbering.** The argument
   number offset is the definition's key `argument_number_offset`
   ([`Definition::argument_number_offset`](crate::lang::rewrites::userdefs::Definition#structfield.argument_number_offset));
   as in the Python implementation, it is one value for the whole definition.
   The numbered keys of the *default values* follow the numbering of the
   placeholders, so `#1` and the default value keyed `"1"` always refer to the
   same argument. Once an offset is declared, the Python implementation reads a
   numbered default key as an absolute position of the argument but a
   placeholder as a number shifted by the offset, so the two disagree; this
   library does not follow it. With no offset declared, which is the common
   case, the two implementations agree.
8. **`\IfProvidedT/F/TF`** is this library's name for the Python
   implementation's `\IfBooleanT/F/TF`. Both are accepted.
9. **A placeholder that refers to an argument the definition does not have
   refuses the definition when the registry is built**
   (`flm.userdefs.bad-placeholder`). The Python implementation raises an error
   at each use site's parse. A template writing `#3` for a definition of one
   argument can never work, so the mistake is reported once, where it was made,
   rather than at every use.
10. **A definition with no template is refused when the registry is built.** The
    Python implementation accepts an entry with no `content` and expands it to
    nothing. Here a declaration that does nothing is taken as a mistake in the
    configuration, most often a mistyped key: it is reported once when the
    registry is built, and each use site is then reported as
    `flm.userdefs.bad-definition` and shown as its source text. For a command
    that produces nothing, declare an empty template.
11. **An expansion is inserted in place of the use site, without a wrapping
    group.** The Python implementation inserts the expansion inside a group with
    empty delimiters, which hides a paragraph break written in a template from
    the pass that builds blocks. Here the expanded nodes become plain siblings
    of the nodes around the use site, so a paragraph break in a template splits
    the enclosing paragraph exactly as the same characters typed by the author
    would. The same markup means the same thing wherever it came from.
12. **No command is declared a second time for use inside a template.** While
    it parses a template, the Python implementation replaces the commands whose
    arguments are read as characters (the references, the labels, the hyperlink
    commands) by variants of its own, so that a placeholder in their arguments
    is substituted. This library has no such variants. The two argument forms
    that are not read as FLM content, a characters-only group (the characters
    of a label or of a reference target) and verbatim content, keep the
    placeholder escape `#` while a template is parsed, for every command that
    declares them, standard or not. `\ref{#1}`, `\label{sec:#1}` and
    `\verbcode{<#1>}` substitute their argument, and `##` is a literal `#`
    there as everywhere in a template. What a placeholder puts inside verbatim
    content is read as characters (the content of a brace group of an ordinary
    argument as its characters, without the braces); a command, a special
    character sequence or mathematics there is reported by the command or
    environment that owns the verbatim content
    (`flm.verbatim.content-not-characters`) and shown as its source text.

Three properties of the expansion matter when a definition is ported. The
arguments are the use site's nodes as already parsed, copied and not tokenized
again, so a definition cannot change how its arguments were read. Every
expansion gets a source of its own, so a diagnostic in an expanded node gives
its position in the template and then the position of the use site. The
whitespace after a macro's use site is part of the use site and is removed with
it.

## Verbatim content

The [`verbatim` feature](crate::stdcore::features::verbatim) declares three
macros and two environments: `\verbtext{…}`, `\verbcode[lang]{…}`,
`\verba[lang]{…}`, `{verbatimtext}` and `{verbatimcode}[lang]`. A macro's
content is delimited by the first character after the command and its optional
language argument: the content ends at the partner of that character for `{`,
`[`, `<` and `(`, and at the same character again for any other character.

One difference from the Python implementation concerns the language argument.
Here `[lang]` is an ordinary optional argument, declared with the same argument
forms as every other command's. The Python parser requires a group of plain
characters immediately after the command. This library's argument forms have no
optional characters-only group, and one argument did not justify adding one.

No standard feature interprets the language name; a backend that highlights
syntax may use it. The text backend prints the content byte for byte and prints
neither the verbatim kind nor the language, which plain text has no way to show.

## Inline formatting, literal characters, `~` and `\@`

The Python implementation's `baseformatting` feature is two features here:

- [`textstyles`](crate::stdcore::features::textstyles) declares the inline
  formatting commands `\emph`, `\textbf`, `\textit` and `\texttt`. `\texttt`, a
  monospaced face, has no counterpart in the Python implementation; it reaches a
  backend as a monospaced text style, and the text backend prints its content
  without marks, since plain text has no typefaces. `\emph` is an element of its
  own, different from `\textit`; the Python implementation treats the two as
  one.
- [`literals`](crate::stdcore::features::literals) declares the commands that
  produce a single character (`\%`, `\#`, `\&`, `\$`, `\ `, `\{`, `\}`,
  `\textbackslash`, and the special character sequence `~` for a non-breaking
  space) and `\@`. These ten are the declarations Python's `baseformatting`
  makes, with the same meanings, so they port unchanged. `baseformatting` also
  declares the paragraph break as a special character sequence; here the
  paragraph break is part of the language itself and no feature declares it.

`\@` is accepted and produces nothing in the text output; the recomposed LaTeX
writes it back as `\@`, as the Python implementation's LaTeX output does.

## Blocks, paragraphs and typography

- **A display-mathematics environment is a block here.** `{equation}`,
  `{align}`, `{gather}` and their starred forms have the block role, so text
  written on the lines before and after one becomes two paragraphs, with the
  environment between them. In the Python implementation the environment is
  inline content, so the same source is one paragraph. A count of a document's
  blocks therefore differs between the two. Display mathematics written
  `\[…\]` is inline content in both.
- **Whitespace is always collapsed, with no switch.** A run of spaces, tabs and
  line breaks in the source is one space in the text; a line break is a space,
  not a line break. The Python option `simplify_whitespace` turns the same
  collapsing off. Here collapsing is part of what the source format means, and
  the [tree contract](crate::lang::rewrites::blocks::check) (what every frozen
  tree satisfies) requires collapsed text everywhere downstream.
- **The typographic substitutions are four independent switches**, the fields of
  [`BlocksConfig`](crate::lang::rewrites::blocks::BlocksConfig), all on by
  default. The Python implementation groups them under one `auto` key that also
  accepts `'quotes'` and `'ligatures'`. There is no such shorthand here: a
  configuration sets each family it wants.

## Headings

The [`headings`](crate::stdcore::features::headings) feature declares the
heading commands. A *heading kind* is what a heading is (a section, a
subsection), declared with a level
([`HeadingDecl`](crate::stdcore::features::headings::HeadingDecl)); a *heading
command* is a macro that writes a heading of one kind
([`HeadingCommand`](crate::stdcore::features::headings::HeadingCommand)). The
differences:

- **Five levels by default, not six.** The standard heading kinds are
  `section`, `subsection`, `subsubsection`, `paragraph` and `subparagraph`, each
  written by the command of its own name; the Python implementation declares a
  sixth, `\subsubparagraph`. Five is LaTeX's own set below `\chapter`. A document
  that writes `\subsubparagraph` is reported as using an unknown macro, and a
  host that wants a sixth kind declares it in the configuration
  ([`HeadingsConfig::headings`](crate::stdcore::features::headings::HeadingsConfig::headings),
  a `HeadingDecl` with its name and level).
- **The levels are the Python implementation's; level 0 and the root heading
  level are new.** The standard kinds keep the Python levels, `section` 1 to
  `subparagraph` 5, and a section is still written `<h1>` by default. A level
  here is a number counted from zero, and every value is valid, so a host may
  declare a kind at level 0 above the sections, such as a chapter. Which level a
  render writes as its first-level heading is the render option
  `root_heading_level` (a render option is an [option value](crate::model::options)
  set per render request), 1 by default, of the HTML output
  ([`HtmlRenderOptions::root_heading_level`](crate::stdcore::backends::html::HtmlRenderOptions::root_heading_level))
  and of the text output. A heading above that level is an Error
  (`flm.pres.heading-above-root`) and is written as a first-level heading. The
  Python HTML renderer takes a heading's tag from a table that a host may set,
  level by level, and refuses a level the table does not hold; its text output
  takes the way a heading is printed from a similar table. Here no level is
  refused. A host that set those tables does one of two things instead: to move
  every heading up or down, it sets `root_heading_level`; to give the headings
  of one kind a tag of their own on the page, it maps that kind, by its name
  rather than by its level, with the render option
  [`heading_tags`](crate::stdcore::backends::html::HtmlRenderOptions::heading_tags).
  The text output's heading marks follow the root heading level and have no
  other setting.
- **The star means "not numbered" and nothing else; a starred heading stays in
  the table of contents.** This differs from LaTeX too, where `\section*` is
  also left out of the table of contents. Here `\section*{T}` is exactly
  `\section[numbered=false]{T}`. A heading is left out of the table of contents
  only when it says `toc=false` (`\section*[toc=false]{T}`) or when its kind is
  declared never to be listed
  ([`skip_toc`](crate::stdcore::features::headings::HeadingDecl::skip_toc)).
  Every heading command takes that optional bracket of key-value pairs, with the
  keys `numbered`, `toc` and `alt` (an alternative title, which the table of
  contents shows in place of the title); the key `pdfbookmark` is reserved and
  not implemented yet (a Warning). LaTeX's short title,
  `\section[Short]{A long title}`, is the Error `flm.heading.unknown-key` here:
  the bracket holds key-value pairs, and the alternative title is written
  `alt={Short}`. The feature `toc` builds a table of contents, off by default
  and placed by the output's template; the heading also carries whether it
  is listed (`data-flm-toc` on a block heading in the HTML output). In the
  recomposed LaTeX, the LaTeX hook `\flmHeading` (a macro the style file defines
  with a default and a user may redefine) receives it as its key `toc`. The
  hook's default adds a starred heading that is listed (`toc=true`) to LaTeX's
  table of contents with `\addcontentsline`; it does not take an unstarred
  heading out, and it writes a LaTeX warning where `toc=false` asks for that.
- **A heading is a block by default, and run-in headings are a setting.** A
  run-in heading is a heading set at the start of the paragraph it introduces,
  on the paragraph's first line, rather than on a line of its own (see
  [`headings`](crate::stdcore::features::headings#run-in-headings)). The Python
  implementation always sets `\paragraph` and `\subparagraph` that way
  (`Note:  Some content.`), as a property of the heading command itself, except
  in its Markdown output, which makes them blocks. Here a heading is a block at
  every level unless the host asks otherwise: the host declares the kinds it
  wants run in
  ([`HeadingDecl::run_in`](crate::stdcore::features::headings::HeadingDecl::run_in),
  or by level with
  [`HeadingsConfig::run_in_levels`](crate::stdcore::features::headings::HeadingsConfig::run_in_levels)).
  No standard kind is run in, so a document ported unchanged gets a heading
  block where the Python implementation gave a run-in heading. The level is the
  same either way: the level is what the heading *is*, and being run in is how a
  host chooses to set it. A run-in heading is lowered to a decoration, not to a
  second kind of heading (a decoration is a node whose element carries the
  [`Decoration`](crate::stdcore::backends::pres::Decoration) aspect, which the
  lowering moves out of the content into an attached slot of the node around
  it).
- **The stop after a run-in heading is a render option of each backend.** In
  plain text this library writes the title, a period and a space
  (`Note. Some content.`), and in HTML it writes the same period as text inside
  the run-in span. The period is the render option `run_in_stop`, which the text
  and the HTML backends both have under that name; an empty text writes no stop.
  The stop is added only where the title does not already end in `.`, `,`, `:`,
  `;`, `?` or `!`; the test reads the end of the title's text, not the element
  that ends the title. A program that needs headings handled another way
  registers a lowering rule or a stage of its own ([the extension
  guide](crate::guide::extending#lowering-rules)). Because the stop belongs to
  each backend rather than to the document, another backend may set the same
  heading differently. [The rendering chapter](crate::guide::rendering)
  describes the plain-text shapes.
- **The text backend marks levels 3 and deeper with `#` characters.** Levels 1
  and 2 are underlined with `=` and `-` as wide as the title; levels 3, 4 and 5
  are preceded by as many `#` characters as the level (under the default root
  heading level, 1; the count starts from the root heading level). The Python
  implementation underlines level 3 with `~` and does not tell deeper levels
  apart.
- **The headings configuration says which levels are numbered.** The
  Python headings feature takes one setting for this,
  `numbering_section_depth`: `False` numbers nothing, an integer numbers the
  levels down to it, `True` numbers all of them; it is set when the feature is
  constructed, and a render may override it. Here the same three choices are
  [`HeadingsConfig::numbering`](crate::stdcore::features::headings::HeadingsConfig::numbering):
  `HeadingNumbering::NothingNumbered`, `UpToDepth(n)` and `AllNumbered`, so the
  Python `numbering_section_depth=2` is `UpToDepth(2)`. It is fixed when the
  registry is built, and a render cannot override it, because the reference
  points a heading declares depend on it. No level is numbered unless the
  configuration says so, which is the Python default too. The numbers look the
  same in both: `1.`, `1.2.`, `1.2.3.` for the first three levels, letters for
  the last two, and `§ 1.2` in a reference. The space after the section sign is
  a narrow no-break space (U+202F) here and a thin space in the Python
  implementation, so that no line breaks between the sign and the number and
  plain text keeps the space. The counter of each level is named after its
  command, so a host that wants numbers of another form replaces the counter's
  counter style by name, through
  [`NumberingConfig`](crate::stdcore::features::numbering::NumberingConfig),
  rather than passing a formatter to the headings feature.
- **The text between a heading's number and its title is a template of the
  heading kind.** The Python `heading_joiner` is one joining text. Here each
  heading kind has a heading title template,
  [`HeadingDecl::title_number_format`](crate::stdcore::features::headings::HeadingDecl::title_number_format),
  written in the [template
  language](crate::guide::concepts_overview#the-template-language-shared-by-features),
  in which `#{number}` is the number: `#{number}. ` by default, which prints
  `1. Intro` as the Python implementation does. A kind may use
  `Chapter~#{number}: `, or `#[bare]{number} `, which writes the kind's own
  number without the numbers of the counters it is numbered within.
- **A reference to a heading is written in the counter style of its level.**
  Each heading level has a counter style of its own, so `\ref` to a numbered
  `\paragraph` reads `§ a`, in the letters that level's numbers are written in.
  The Python implementation registers one base style for every level and writes
  a reference with it, so the same reference prints a decimal digit there, which
  does not match the heading it points at.
- **The configuration declares the heading kinds and the commands.** The
  Python `section_commands_by_level` replaces the command name at a level, so
  that level 1 can be `\chapter`. Here a heading kind is declared by name with
  its level (`HeadingDecl`), and a command is mapped to a kind separately
  (`HeadingCommand`), both in
  [`HeadingsConfig`](crate::stdcore::features::headings::HeadingsConfig): a kind
  `chapter` at level 1 written `\chapter` is one declaration of each, and two
  commands may write one kind, each with default key-values of its own.

## Labels and references

A label is the text an author writes as `\label{type:label}` after a construct,
which the construct attaches to its reference point (one place in a document
that can be referred to, numbered or given an anchor;
[`RefPoint`](crate::model::aspects::RefPoint)); the label type, the part before
the colon, must be one the point accepts (see [reference points, labels and
anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)).
As in the Python implementation, each element decides which label types its
points accept; a heading accepts `sec` and `topic`
([`LABEL_TYPES`](crate::stdcore::features::headings::LABEL_TYPES)), and a label of
any other type, such as `\label{eq:x}` on a section, labels nothing.

**What happens with a label of a type that is not accepted.** In the Python
implementation the document fails to parse, so there is no output at all. Here
the label defines nothing: no reference resolves to it, and an element whose
labels were all refused has no anchor, so nothing links to it. The label check
of the [`refs`](crate::stdcore::features::refs) feature reports it as the Error
`flm.refs.label-type-not-accepted`. That check runs under every workflow of the
standard features, the rendering workflows included, so the error is reported
wherever the document is processed. A reference to the label is reported as
unresolved beside it: an Error in a render by default, a Warning under the
`check` workflow. An Error does not stop the computation, and the run still
computes its outputs. A run that has recorded an Error stops only before a stage
that acts outside the run, such as one that writes files, unless the caller sets
[`RunOptions::continue_with_errors`](crate::core::run::RunOptions::continue_with_errors)
(see [the error gate](crate::guide::embedding#the-error-gate)); the `flm` program
sets it under `--continue-with-errors`.

**A reference's target is split at commas, as in the Python implementation.**
`\ref{a,b}` refers to two targets, each resolved and linked on its own, and the
numbers of one counter style are written together. Four things differ:

- The options of a reference apply to **every** target. The Python
  implementation reads the option `S` (the capitalized wording) for a single
  target only and ignores it for several.
- A target, among several, that this document did not number shows its content
  (such as a heading's title or a float's caption), or else its label as
  written, at its place in the list. The Python implementation leaves it out of
  the list and writes it after the list. The text reads the same; here one rule
  serves one target and several.
- The items of the list (the numbers of each counter style, then each target
  without a number) are joined by join texts, where the Python
  implementation always writes `, `: `\ref{eq:a,thm:big}` reads
  `Eq. (1) and Thm. 1.1` where the Python implementation writes
  `Eq. (1), Thm. 1.1`, and three items read `Eq. (1), Thm. 1.1, and A note`.
  The join texts are the configuration of the references feature
  ([`RefsConfig`](crate::stdcore::features::refs::RefsConfig)), declared in the
  same form as a counter style's join texts.
- Any option other than `S` and `noprefix` asks for a wording variant (a variant
  of the words written before a number), where the Python implementation reads
  only `S` and ignores every other key. Asking for a variant that the counter
  style of a numbered target does not declare is the Error
  `flm.refs.undeclared-variant` at the reference, and the usual wording is
  written; a misspelled option such as `\ref[captial]{…}` is therefore reported
  instead of ignored. `S` behaves as in the Python implementation: the
  capitalized wording where the counter style declares one, the usual wording
  otherwise, with nothing reported.

The label written on a reference point is **not** split: one `\label` declares
one label, commas included.

**An unresolved reference is never a link.** Under
[`UnresolvedRefs::Allow`](crate::stdcore::features::refs::UnresolvedRefs) the
reference is reported as a Warning and shown as the label's own text, marked as
unresolved, with no link target. The Python implementation writes a link to a
`javascript:` address; this library never writes one, because nothing it
produces may become a script that runs in a reader's browser.

**References to other documents are data that a host installs, not code that
this library calls.** The Python implementation takes a list of resolver
objects and calls each one, in order, for every reference the document cannot
resolve itself, giving it the reference's type and label together with the
resource and rendering context. Here a host installs one external table, the
external index `flm.refs.external-index`
([`EXTERNAL_INDEX`](crate::stdcore::features::refs::EXTERNAL_INDEX)), when it
constructs the run: a map from a label to a target, which holds a destination
and the content to show. An external table is a table that no stage of the run
writes; the caller installs its value
([`Externals`](crate::core::run::Externals)). The resolve stage of the
references feature reads the index for a label that no point of the document
carries, and a label found in neither is reported once as
`flm.refs.unresolved`. What a ported host changes:

- **A resolver is data, not a function.** Nothing in a run calls back into the
  host's code, so a run can be repeated and its results cached. A host that
  has to *compute* a destination computes it before the run and installs the
  result.
- **A resolver that raises an error has no counterpart.** An unknown reference
  is a diagnostic; the run continues and computes its outputs.
- **The context of a reference has no counterpart.** There is no resource
  information and no rendering context to give a resolver, because there is no
  resolver to give them to.
- **The order of several resolvers has no counterpart.** There is one table. A
  host with several sources merges them itself and decides in its own code
  which one wins.

What is gained: the text shown for an external target is FLM content, not a
string, so markup in it is written to the output the same way as markup in a
heading's title.

## Counter styles and number formatting

A counter style says how a counter value is shown: the number template, the
sub-levels, the prefix words with their variants, the delimiters and the join
texts ([`CounterStyleDecl`](crate::stdcore::features::numbering::CounterStyleDecl);
see [counters and counter
styles](crate::guide::concepts_overview#counters-and-counter-styles)). The code
that turns a counter's value into the number a reader sees is a port of the
Python implementation's `counter` module, and a configuration written for that
module maps onto it almost key for key: the same eight number formats under the
same names, the same single-letter number templates (`(a)` writes `(c)` for the
number three;
[`NumberTemplate::Initial`](crate::stdcore::features::numbering::NumberTemplate::Initial)),
the same join specification with its thirteen texts, its four variables and its
compact preset, the same sorting, the same collapsing of consecutive values into
ranges, and the same rule that a lone pair of consecutive values is written `1
and 2` rather than as a range. See [the numbering
chapter](crate::guide::numbering).

The differences:

1. **A counter style's texts are FLM, and a number is content.** The word
   before a number, the delimiters, the join texts and the text around the
   number in a number template are parsed once, when the counter style is
   compiled, and what they produce is content, not a string of markup. Writing a
   counter value produces a list of *segments*
   ([`Segment`](crate::stdcore::features::numbering::Segment)), each a piece of
   content together with the position of the value it links to. The Python
   implementation joins strings and calls a function to wrap each linked part.
2. **A template is written `#{…}`, not `${…}`.** A counter style's number
   template and a counter's within template (what a number writes of the
   counters it is numbered within) are written in the [template
   language](crate::guide::concepts_overview#the-template-language-shared-by-features),
   the language of a user definition's template. The format is `#{arabic}`
   where the Python implementation writes `${arabic}`, and the counter a number
   is shown within is `#{flm.headings.section}` (difference 13), or `#{_parent}`
   for the parent that applies, where it writes `${section}`. The whole template
   is parsed once, so markup may enclose the number as well as be placed
   beside it: `\textbf{#{arabic}}` works, where the Python implementation
   substitutes the number into the template and parses the result. The
   single-letter number templates (`(a)`, `1.`), in which the first of the
   characters `a`, `A`, `i`, `I` and `1` is the number, are unchanged.
3. **Zero, negative and very large numbers follow one rule.** Every format
   except `arabic` writes the empty text for a number it cannot write, and
   `arabic` writes the digits of any number. In the Python implementation,
   `alphacounter` returns the empty string for zero only because Python's
   division rounds downwards, `romancounter` returns a meaningless text for a
   negative number, and `customdigitscounter`, on which the superscript and
   subscript formats are built, never returns for one.
4. **The words before a number are keyed `one` and `other`.** Keys for an exact
   count, such as `4: "Quartet of Equations~"`, are not ported: they serve one
   language's special cases, and a configuration file cannot hold an integer
   key. A variant the counter style does not declare still gets the default
   words, as in the Python implementation, but a reference that asks for it is
   reported (see [Labels and references](#labels-and-references)).
5. **A sub-number at a level the counter style does not declare is refused**
   with a typed error, where the Python implementation raises an index error. A
   sub-number of zero is written as nothing in both.
6. **Both delimiters are linked by the same rule.** Both are part of the link
   or neither is: for one value, the word, both delimiters and the number form
   one link. In the Python
   implementation, the opening delimiter is part of the link only when the word
   is, while the closing delimiter is not tested that way.
7. **The part of a number that comes from the counters it is numbered within is
   structural.** The counter record of a reference point holds the ancestors'
   values, and a within-joiner (what a counter numbered within another writes
   of its ancestors before its own number;
   [`Within`](crate::stdcore::features::numbering::Within)) writes them:
   `Within::parent("flm.headings.subsection")` joins the subsection's number and
   a full stop, and a template's
   `#{flm.headings.subsection}` refers to an ancestor and writes its number, with
   that ancestor's own ancestors in front of it, and with no word and no
   delimiters at any level. A joiner therefore refers only to the counter
   immediately above, as the Python configuration's `${subsection}.` does. The
   Python implementation stores that part as a string already formatted, which
   includes the ancestor's own delimiters and makes two ancestors that happen to
   format alike share one counter state. Here the ancestors are carried as the
   chain of their values, outermost first, and the number is composed where it
   is shown. The only difference a reader sees: the ancestor's delimiters are
   not written.
8. **A format, a word and a template are data, not code.** The Python
   implementation accepts a function wherever a number format or a word is
   declared. Here they are declared values, so that a counter style can be
   written in a configuration file and compiled once.
9. **`number_within` is `NumberingOptions::within`.** The Python
   `number_within: {eq: {reset_at: subsection, numprefix: '${subsection}.'}}`
   is `NumberingOptions::new().within("flm.math.equation",
   Within::parent("flm.headings.subsection").template("#{_parent}."))`, or,
   since that is the default joining text,
   `Within::parent("flm.headings.subsection")` alone
   ([`NumberingOptions`](crate::stdcore::features::numbering::NumberingOptions),
   [`Within`](crate::stdcore::features::numbering::Within)). A counter that
   nothing restarts is `within_nothing`. Two things have no Python counterpart:
   a counter may be numbered within whichever of several counters advanced most
   recently (`Within::parents`, `Within::parent_set`, `#{_parent}` in a
   template), and `no_restart()` writes the parent's number without letting the
   parent restart the count.
10. **An ancestor with no value is an error, not a zero.** In the Python
    implementation, as in LaTeX, a subsection written before the first section
    is `0.1`, and a paragraph in a section with no subsection yet is `2.0.1`.
    Here a counter has no value until it first advances, and a point whose
    number would need such a value takes no number and is reported
    (`flm.numbering.parent-uninitialized`, once per counter). A document that
    wants `0.1` says so with
    `NumberingOptions::initial("flm.headings.section", 0)`.
11. **A format is written only inside a template, and what three formats count
    in is fixed.** The Python implementation also accepts a bare format name
    where a template is expected; here that is the template `#{arabic}`, the
    only form. The sequence of footnote symbols and the digits that the
    superscript and subscript formats count in are arguments in the Python
    implementation and are fixed here, because `fnsymbol`, `unicodesuperscript`
    and `unicodesubscript` are three formats, not three functions a caller
    configures.
12. **The identifier built from a number joins the ancestors' values.** Because
    the ancestors are values and not a formatted string, the identifier cannot
    collide as the Python `targetidstr` can, where the prefix `1.` with the
    number 13 and the value `(1, 13)` both give `1-13`.
13. **A counter's name is dotted and belongs to the feature that declares it.**
    The Python implementation gives its counters bare names (`section`,
    `equation`), which share one set of names with every other feature's. Here
    a counter is named like everything the library registers: the headings
    feature's counters are `flm.headings.section` …
    `flm.headings.subparagraph`, the mathematics feature's is
    `flm.math.equation`, and a counter that an extension declares takes the
    extension's own prefix. What a document writes does not change: the heading
    commands, the label types `sec:` and `eq:`, what a reference shows. What
    changes is what a host writes in configuration: the numbering options are
    keyed by the dotted names, and a within template refers to a counter by its
    full name, `#{flm.headings.section}`, or writes `#{_parent}` for the parent
    that applies without naming it.

## The parsing configuration

Three settings decide how source text is read into tokens. They are the fields
of [`FlmParsing`](crate::lang::FlmParsing). The `flm` program reads them from the
block `flm.parsing` of a document's front matter (the block of YAML between a
first line `---` and the next `---` line), under the same names (the
Python implementation's keys have other names; see [the command
line](#the-command-line)):

| key | default | what it does |
|---|---|---|
| `comments` | `true` | whether comments are recognized at all |
| `comment_prefix` | `%%` | what starts a comment: two characters, not LaTeX's single `%`. An empty prefix recognizes no comment. This key also decides whether a lone `%` is a forbidden character: it is, unless the prefix is exactly `%` and comments are on |
| `dollar_math` | `false` | whether `$…$` and `$$…$$` delimit mathematics. When it is off, a `$` in the source is reported as `flm.lang.dollar-math-disabled`. `\(…\)` and `\[…\]` delimit mathematics either way |

A forbidden character is a character that the parser reports where it occurs as
ordinary text; it is kept in the text as written. There is no key that lists the
forbidden characters, and no way to switch one off on its own: these keys alone
decide them. A lone `%` is reported as `flm.lang.percent-forbidden` and a `$` as
`flm.lang.dollar-math-disabled`, in both cases because the character is
ambiguous (a percent sign or a comment, a currency amount or mathematics), and
the language asks for the unambiguous spelling (`\%`, `%%`, `\$`, `\(…\)`)
rather than guessing. The Python implementation derives the same two by the
same rule, so a document that builds there builds here. The Python
implementation also accepts a list of further forbidden characters when a
parsing state is constructed; this library has no counterpart, because the set
of forbidden characters is a property of the language, not of a document.

This library reads no front matter and no configuration file. The `flm`
program reads these keys from a document's front matter, and a program that
embeds the library sets them itself, in
[`RegistryInputs::parsing`](crate::core::registry::RegistryInputs#structfield.parsing),
before the registry is built. They take effect then: the parser's token rules
are derived from them once, when the registry is built, and they are one of the
inputs of the [registry digest](crate::model::digest), so data computed under
one parsing configuration is never read back under another.

## Mathematics is read by a feature of its own

The Python implementation reads no command inside a formula: any command is accepted, and
the source goes to the formula renderer. Here two features share that work. The
[`math`](crate::stdcore::features::math) feature introduces mathematics into the
language (the math groups, the display-math environments, their lines and numbers, the
text commands) and processes none of it; a *processing feature* decides which commands a
formula may hold, checks the formulas and writes them into the output. The library's
math processor, the package
[`math-passthrough`](crate::math_passthrough), is the one that
reproduces the Python implementation's reading: it accepts every command inside a
formula and carries the source to the output. Every name it registers carries
`passthrough` (the element of an unknown command of mathematics,
`flm.math-passthrough.opaque`; the presentation element `flm.pres.math-passthrough`; the
stylesheet `flm.html.style.math-passthrough`). A command that another feature declares
(`\href`, `\section`) is declared for text only, so inside a formula it is an unknown
command of mathematics, kept as written. The extension `math-mathjax` is a second
processing feature, with no counterpart in the Python implementation: it accepts only the
commands of MathJax's TeX packages ([mathematics processed by
MathJax](crate::guide::command_line#mathematics-processed-by-mathjax)). The `flm` program
uses `math-mathjax` by default (see [the command line](#the-command-line)).

## Text inside mathematics is FLM content here

`\text{…}` and the related commands `\textnormal`, `\textup`, `\textrm`,
`\textmd`, `\textbf`, `\textit`, `\textsl`, `\textsc`, `\textsf`, `\texttt` and
`\mbox` ([`COMMANDS`](crate::stdcore::features::math::text::COMMANDS)) are
declared here for mathematics mode, each with one argument read in *text* mode.
The argument is therefore ordinary FLM content: it is parsed as text, its
whitespace is collapsed, the typographic substitutions apply to it, a reference
or a use of a user definition in it works, and each output format escapes it as
it escapes text anywhere else. In the Python implementation the content of a
mathematics region, the argument of such a command included, is passed to the
formula renderer as the source the author wrote, and nothing inside it is FLM
content.

What to expect when moving a document across: `\text{a --- b}` sets an em dash
here and three hyphens there; a command that no feature declares is an error
inside the argument here (text mode reports unknown commands), while
mathematics outside the argument keeps such a command without a report; and the
recomposed LaTeX writes the command back with its argument recomposed as text,
rather than copying the source. The mathematics around the argument is
unchanged in both implementations.

`\ensuremath{…}` is declared here by the `math` feature, with no counterpart in the
Python implementation. Written in text, it is an inline formula, as `\(…\)`; inside a
formula, it stands for its argument.

## Equation lines in display mathematics

An equation line is one line of a display-math environment as its body reads
between two `\\`: its mathematics, its labels, its tag and its no-number mark;
each line is one reference point (see [equation
lines](crate::guide::concepts_overview#equation-lines)). Inside the body of a
display-math environment (`equation`, `align`, `gather` and their starred
forms), the [`math` feature](crate::stdcore::features::math::lines) declares
`\\`, `\label`, `\tag` and `\nonumber` and reads the body as equation lines. The
split into lines is the Python implementation's: a `\label` on a line of its own
belongs to the line that the following `\\` ends; `\notag` is declared by
neither implementation; and a `\label` or a `\tag` on a `\nonumber` line or
anywhere in a starred environment is a parse error in both
(`flm.math.label-without-number` here). Whether the lines are numbered at all
is [`MathConfig::numbered_equations`](crate::stdcore::features::math::MathConfig),
`true` by default and fixed when the registry is built. Six things differ:

1. **An empty line is an error** (`flm.math-passthrough.empty-line`). A line that
   holds no mathematics (nothing at all, or only whitespace, comments, `\label`,
   `\tag` and `\nonumber`) is reported by the check of `math-passthrough`, wherever it is: before
   the first `\\`, between two, or after the last. The Python implementation
   accepts an empty line between two `\\` and silently drops a `\label`, `\tag`
   or `\nonumber` written after the last `\\`. Here both are errors, and such a
   command after the last `\\` begins a further line that holds it.
2. **A final `\\` is dropped.** A `\\` at the very end of a body, followed only
   by whitespace and comments, is not recorded on the last line, and the
   `latex-recomposed` backend does not write it back. The Python implementation
   writes it back, and LaTeX then adds the space of an empty line after the last
   equation.
3. **A comment after the final `\\` is kept.** The whitespace and comments after
   such a `\\` stay in the last line's content as written, so a comment there is
   written into the last line of the `latex-recomposed` output (unless comments
   are dropped), where the source had it. The Python implementation drops it.
4. **A label must be written `type:label`** inside an environment, as every
   label here must (`flm.refs.bad-label` otherwise). The Python implementation
   accepts a label without a type there.
5. **A label belongs to its own line, and a line with no number keeps no
   label.** The environment declares one reference point per line, and a line
   that can be numbered carries the labels written on it, so `\ref{eq:second}`
   resolves to the line that carries `eq:second`. The Python implementation
   registers every label of a multi-line environment under the first line's
   target, so a reference to any of them leads to the first line. What such a
   reference *prints* is the line's number in both. A line that has **no**
   number (a line marked `\nonumber`, and every line of `equation*`, `align*`
   or `gather*`) is still a reference point, so that every line is treated
   alike and a program can walk them, but it keeps no label: writing one there
   is the parse error both implementations report, and here a `\ref` to it is
   also unresolved. A reference here to a label on a *numbered* line resolves to
   that line.

   **A line that has no number takes none.** The Python implementation numbers
   the lines of `equation*`, `align*` and `gather*` too (its loop never asks
   whether the environment is a numbered one), so a starred environment there
   shows `(1)` and the next equation is numbered `(2)`. Here such a line takes
   no number and counts for nothing, and the next equation takes the number the
   starred one did not. A line whose author wrote a `\tag` counts for nothing in
   both.
6. **A `\tag`'s argument is text content, not characters.** The Python
   implementation reads the tag's braces as plain characters and writes them
   into the LaTeX output unchanged. Here the argument is ordinary inline content
   of the document
   ([`Tag::content`](crate::stdcore::features::math::lines::Tag#structfield.content)
   is a run of nodes): a command inside it is read as a command and written by
   each output format, and a character that an output format must escape is
   escaped. `\tag{50\% \& more}` reaches LaTeX as `\tag*{(50\% \& more)}`, where
   the Python implementation's copy, byte for byte, would have made the rest of
   the line a comment. In both implementations an unstarred `\tag{x}` is shown
   as `(x)`.

## Lists

The environments are the Python implementation's three, `itemize`,
`enumerate` and `description`, with its labels by depth (`•`, `-`, `▸`; `1.`,
`(i)`, `a-`), and they are configured as there, per environment name, in
[`ListsConfig`](crate::stdcore::features::lists::ListsConfig). `\item[…]`, and
`\label{item:…}` directly after an `\item`, are read as they are there, except
for a bracket holding `=` (below). The differences:

- **Whether a list is numbered is declared, not read off its labels.** A list
  kind (`itemize`, `enumerate`, `description`, or one a host declares) says
  whether its items are numbered, and its labels by depth are list label
  templates in the [template
  language](crate::guide::concepts_overview#the-template-language-shared-by-features)
  rather than formatter functions. A list kind has a second template per depth,
  the form that a reference to an item writes (`#{arabic}`, `(#{roman})`,
  `#{alph}` for `enumerate`), which the Python implementation does not have. The
  Python `description` list's `<??>` placeholder has no counterpart: a list kind
  with no label template writes none, and an item of it without a tag is an
  error.
- **A list's own label** is `label=` in a key-value argument,
  `\begin{enumerate}[label=(#{alph})]`, or a short form of it made only of the
  characters `[]().:-+*aAiI1`: `[(i)]`, `[1.]`, `[a-]`, and the fixed labels
  `[-]`, `[*]`, `[+]`. The key `ref=`, which the Python implementation does not
  have, sets the form that a reference to the list's items writes; a short form
  sets it to the short form without its trailing full stops, colons and hyphens
  (`[1.]` refers to its items as `1`, `2`). The Python spelling `[(${alph})]` is
  not accepted: the parse reports the dollar sign, and the bracket is neither a
  short form nor a key, which is reported too. A short form holding two of the
  characters `a A i I 1` is an error, rather than read at the first of them, and
  a bracket holding any other character is not a short form: the Python
  implementation read `[Step 1.]` as `Step 1.`, `Step 2.`, taking the first of
  those characters as the number wherever it was. Here the bracket is reported,
  and the label is written `label={Step #{arabic}.}`.
- **An `\item`'s bracket is a key-value argument** whose only key is `tag`. A
  bracket holding no `=` is the tag as a whole, as there; one holding `=` is
  read as keys, so `\item[a=b]`, the tag `a=b` in the Python implementation, is
  an unknown key here, which is reported. Write `\item[tag={a=b}]`; braces
  around the whole bracket, `\item[{a=b}]`, are removed when the bracket is read
  and protect nothing.
- **Items are numbered by the numbering feature**, one count per list. In a
  registry without that feature a numbered list shows no labels and is
  reported, where the Python renderer counted the items itself.
- **A tagged item counts for nothing**: `\item[Custom] \item \item` reads
  `Custom`, `1.`, `2.`, as in LaTeX, where the Python renderer counts by
  position and reads `Custom`, `2.`, `3.`.
- **A reference to an item** shows the word and the item's number through the
  list kind's reference template: `Item 2` for the second item of an
  `enumerate`, `Item (ii)` at the second depth, and `(ii)` under
  `\ref[noprefix]`. The Python implementation showed the label alone, full stop
  included (`2.`), so a sentence ending with the reference ended with two full
  stops. A reference to any other item shows what is written beside the item,
  as there: a reference to a tagged item shows the tag, and a reference to an
  untagged item of an unnumbered list shows the list's own label where that
  label can be shown (`[-]`, `label=*`), and otherwise the bullet (`•`). Braces
  around the value of `tag=` or `label=` change nothing.
- **The depth counts only lists of the same kind**, as LaTeX does: an
  `itemize` inside an `enumerate` shows `•`, where the Python renderer, which
  counts every enclosing list, shows `-`. Nesting is read within one document
  tree. The content of a file brought in with `\input` is part of the tree
  around the command, so a list written there is nested in the list around the
  command, except in a file that opens with a heading of its own, which is a
  tree of its own: a list written there is an outermost list even where the
  command is inside a list item.
- **Content before the first `\item`** is an error and is shown as an item with
  no label, where the Python parse raised an exception.
- **The plain-text output** pads every label of a list on the right to one
  column width and indents the items' text to that column, with no indentation
  of the list itself: `i.  First`, `ii. Second`. The Python output aligned the
  labels on the right in a column two spaces wider than the widest label,
  `   i. First`, `  ii. Second`, and indented a nested list by a fixed amount.

## Footnotes and other xnotes

The Python implementation's `endnotes` feature is the
[`xnotes`](crate::stdcore::features::xnotes) feature here. An xnote is a piece of
text set apart from the running text, marked where it belongs and shown
elsewhere or in place (a footnote, an endnote, a side note); an xnote kind is
one way of being an xnote, declared by the feature's configuration. An xnote
kind takes the place of the Python *category*. `\footnote{…}` is written as
there, and the standard package declares one xnote kind, `footnote`, by
default. The differences:

- **The xnote kinds are declarations in the configuration.** The Python feature
  takes a list of `EndnoteCategory` values; the standard package here takes
  [`XnotesConfig`](crate::stdcore::features::xnotes::XnotesConfig), a list of
  [`XnoteKind`](crate::stdcore::features::xnotes::XnoteKind) values, fixed when
  the registry is built:

  | Python `EndnoteCategory` | `XnoteKind` |
  |---|---|
  | `category_name` | `name` (ASCII letters only) |
  | `endnote_command` (one command, or none) | `commands` (any number, or none) |
  | `counter_formatter` | `numbered`, whether the kind is numbered at all; then `number_format`, the number a reference shows, and `mark`, the text of the mark, each a number template in the [template language](crate::guide::concepts_overview#the-template-language-shared-by-features); the counter is declared to the numbering feature |
  | `heading_title` | `title` (FLM text, the kind's plural word by default, `None` for no heading) and `title_level` |
  | — | `words` (what a reference writes, `Footnote~2`), `label_type` (`fn`), `within`, `placement`, `layout`, `backlink`, `latex_command` |

  A category that a feature added to a document at run time (the Python
  document manager's `add_endnote_category`) is here an xnote kind that another
  feature contributes when the registry is built, through
  [`contribute_kind`](crate::stdcore::features::xnotes::contribute_kind); its
  xnotes are nodes of that feature's own elements rather than calls into the
  xnotes feature. The Python render options of the list of notes (whether a
  heading is written, at which level, a heading above all the lists) are the
  xnote kind's `title` and `title_level` here. There is one list per xnote
  kind, and a render leaves a list out through
  [`PiecesRenderOptions`](crate::stdcore::backends::pieces::PiecesRenderOptions).
- **A footnote can be labeled and referred to.** In
  `\footnote[\label{fn:x}]{…}`, the optional argument holds `\label`s and
  nothing else (anything else is reported and ignored), and `\ref{fn:x}` shows
  `Footnote~1`. The Python command takes one argument, and its notes carry no
  label. A `\label` written after the note, `\footnote{…}\label{fn:x}`, is an
  unknown command here.
- **The mark is the number, raised.** The Python standard features write the
  mark as a letter (`a`, `b`, …, the `alph` formatter) inside a link. Here the
  mark is the number written by the xnote kind's mark style (its `mark`,
  `#{arabic}` by default), set as a superscript: the plain-text output prints
  `¹`, the HTML page writes `<sup>…<a …>1</a></sup>`, and the recomposed LaTeX
  leaves the mark to LaTeX's `\footnote`. A host replaces the mark style as it
  replaces any declared counter style ([the numbering
  chapter](crate::guide::numbering)).
- **The notes are numbered in document order** by the numbering feature, not in
  the order in which a renderer reaches them, so the number is the same in the
  plain-text output and on the page. In the recomposed LaTeX, where LaTeX
  numbers the notes, the number is also the same for an xnote kind whose LaTeX
  command counts its notes as FLM does (`footnote`, passed to `\footnote`),
  including a note nested in another. It is not the same after a note written
  in a list's own label template: LaTeX sets that note beside every item's label
  and inside every `\ref` that prints that template, while the other outputs
  write no mark there. A note in the template of a user definition that is kept
  as a call (a kept definition,
  [`userdefs::kept`](crate::stdcore::features::userdefs::kept)) is written into
  that definition in the style file, so LaTeX sets it wherever the call is, as
  the other outputs do. A `\label` in such a template is an Error at each use
  of the definition (`flm.userdefs.kept-template-declares-point`): a kept
  definition's template is lowered as a [synthesized
  value](crate::guide::concepts_overview#synthesized-values) (a small tree built
  while a run is under way), and a synthesized value cannot declare a reference
  point that the rest of the document refers to. To label something in the
  template, give the definition the expansion policy `always`.
- **Notes nest.** A note written inside another note, of the same xnote kind or
  another, is a note like any other: numbered in document order after the note
  that holds it, and listed in its own kind's list, with its mark inside the
  outer note's content.
- **A reference shows a note in the text it repeats as the note's mark.** A
  reference to an unnumbered heading, an unnumbered float, a defined term or a
  list item's tag shows that construct's text in the plain-text output and on
  the page. A note in that text is shown there as the note's mark, linked to the
  note, which is written and listed once, where it was written; a labeled note
  there is referred to like any other. The recomposed LaTeX leaves the text to
  LaTeX, which sets the note again at each such reference. The Python
  implementation reads a heading's title again from its source to show it, in a
  mode where a footnote is not allowed, so a reference to an unnumbered heading
  whose title holds a footnote stops the render with an error. It shows a list
  item's tag as parsed, without reading it again, so that error does not arise
  there. The difference follows from how this library copies the content a
  reference shows: each element of the copy says what represents it there, and
  a note is represented by its mark.
- **The plain-text output holds the notes.** The mark is written where the note
  was written, and the notes are listed after the content under the xnote
  kind's title (`Footnotes`), each beside its mark. The Python text output
  prints the mark alone and no note.
- **Notes are never moved.** An xnote kind is either listed after the content
  or shown in place (`XnotePlacement::InPlace`): in brackets where it was
  written; a note holding paragraphs then interrupts the paragraph around it, as
  a displayed formula does. A note's content may hold paragraphs and lists; the
  xnote kind's `layout` says how the content argument is laid out.
- **Identifiers**: a numbered note is `fn-1` on the page, and its mark carries an
  identifier of its own, which the list's link back to the mark points at. The
  Python identifier is `footnote-1`, and there is no link back.

The page's markup and the recomposed LaTeX of a footnote are listed with the
other differences of those outputs below: a footnote is written
`\flmXNfootnote{…}` in the recomposed LaTeX (difference 21 under [Recomposing
to LaTeX](#recomposing-to-latex)).

## Hyperlinks

The Python implementation's `href` feature is the
[`href`](crate::stdcore::features::href) feature here, with the same three
commands, `\href{address}{text}`, `\url{address}` and `\email{address}`, and the
same display: `\url` and `\email` show the address without a leading `http://`,
`https://` or `mailto:` and without trailing `/`, `#` and `?` characters. The
address is read as raw text in a document, as there. The differences:

- **Inside a template.** The Python implementation declares the three commands
  again inside a user definition's template, so that a placeholder in the
  address is substituted. Nothing is declared again here: the address argument
  keeps the template's placeholders working by itself, so
  `\href{https://doi.org/#1}{#1}` in a template substitutes the argument, and a
  literal `#` there is written `##`.
- **The plain-text output.** The Python option `display_href_urls` is
  [`TextRenderOptions::link_addresses`](crate::stdcore::backends::text::TextRenderOptions::link_addresses),
  on by default as there: a link prints its address after its text. One
  difference: a `\url` or an `\email` prints its display once,
  `\url{https://example.com}` printing `example.com` where the Python output is
  `example.com <https://example.com>`, because what it shows is already the
  address. The second option,
  [`link_addresses_when_shown`](crate::stdcore::backends::text::TextRenderOptions::link_addresses_when_shown),
  off by default, prints the address after such a link too, which gives the
  Python output back.
- **The recomposed LaTeX.** The Python recomposer writes `\href` and `\url`
  directly, and `\email` as itself, which LaTeX does not define. Here each
  command is written as a LaTeX hook that the style file defines with
  `\ProvideDocumentCommand` over the `hyperref` package, which the style file
  requests: `\flmHref{address}{text}`, `\flmUrl{address}` and
  `\flmEmail{address}`, the last one linked to `mailto:` followed by the
  address. The last two derive the display in LaTeX, as the page shows it, and
  typeset it with `\nolinkurl`, where the Python recomposer's `\url` shows the
  whole address. Every argument is an ordinary LaTeX argument, with `#`, `%`
  and `\` written `\#`, `\%` and `\\`, so the hooks work inside a heading's
  title. The Python feature's `map_macros` option, which renamed the commands
  written, has no counterpart: redefining a hook does what renaming did.
- **The commands are fixed.** The settings of the Python command class have no
  counterpart: `command_arguments`, which chose the arguments a command takes,
  because the three commands are fixed here and a host that wants another adds a
  feature of its own; and `ref_type`, the link type `href`, because every link
  the three commands write is of the kind `url` (`data-flm-kind="url"` on the
  page; [`LINK_KIND`](crate::stdcore::features::href::LINK_KIND)).

## Defined terms

The Python implementation's definition terms are the
[`defterm`](crate::stdcore::features::defterm) feature here:
`\begin{defterm}{term} … \end{defterm}` defines a term, and `\term[ref]{shown}`
links to its definition. What carries over:

- A term link finds a definition by the term as written, exactly, letter case
  and markup included; where the bracket is left out, the text shown is the
  term.
- A definition shows its term before the words of its first paragraph, followed
  by `: ` by default.
- A definition accepts a `topic:` label, by which `\ref` refers to it.

The differences:

- **A term link finds its definition through a label that the library computes
  from the term**, and from each alias; the author never writes it. An author's
  own `\label` on a definition takes the label type `term` or `topic`: `term` is
  the label type for a definition, as `sec` is for a heading and `eq` for an
  equation.
- **Aliases are new.** `\aliasterm{…}`, written after the term beside the
  `\label`s, declares another spelling of the term (a plural, an inflected form)
  under which a term link finds the definition. The Python implementation has no
  counterpart and matches only the term's own spelling.
- **The lead-in is a template.** What a definition shows before its body is
  configured as a template in which `#{term}` is the term
  ([`DeftermConfig`](crate::stdcore::features::defterm::DeftermConfig)),
  `#{term}: ` by default, rather than by the Python feature's two settings
  (which it passes to its environment class as that class's
  `render_with_term` and `render_with_term_suffix`). A suffix
  `render_defterm_with_term_suffix` is the template `#{term}` followed by that
  suffix, and `render_defterm_with_term=False` is `None`, which shows no
  lead-in.
- **The command and the environment are fixed**: `\term` and `defterm`. The
  Python classes' `macroname` and `environmentname`, which set their names,
  have no counterpart; a host that wants another name adds a feature of its own.
- **A term link inside the definition of its own term is not a link**: it is
  shown as the term being defined. This is decided from the document's
  structure, where the Python renderer keeps a stack while rendering.
- **A term link is a reference** resolved by the references feature: one that
  finds no definition is an unresolved reference, reported as any reference is,
  and two definitions of one term are a label written twice.
- **The recomposed LaTeX writes a term link through two LaTeX hooks**, where the
  Python implementation writes one, `\flmTerm{term}{safe label}{ref term}{shown}`.
  Here a term link is `\flmTerm{<safe label>}{<shown>}`, the two arguments
  LaTeX needs (a safe label is the key the recomposed LaTeX writes for a label;
  see [`SafeLabels`](crate::stdcore::features::refs::SafeLabels)), and a term
  inside the definition of its own term is `\flmTermInDefterm{<shown>}`, the
  term being defined set apart with no link, for which the Python
  implementation has no form. The environment `defterm` is defined from the
  configured lead-in.

## Theorems

The Python implementation's `theorems` feature is the
[`theorems`](crate::stdcore::features::theorems) feature here: the environments
`theorem`, `lemma`, `proof` and the others, with their title, their `\label`s,
`\noproofref`, a proof's back-reference `[*lem:x]` or `[**lem:x]`, and the
closing mark. What carries over:

- The three sets of environments, `simpleset`, `defaultset` and `richset`, are
  configuration values:
  [`TheoremsConfig::simpleset`](crate::stdcore::features::theorems::TheoremsConfig::simpleset),
  [`TheoremsConfig::new`](crate::stdcore::features::theorems::TheoremsConfig::new)
  (the default set, which is the default here too) and
  [`TheoremsConfig::richset`](crate::stdcore::features::theorems::TheoremsConfig::richset).
- The shapes the Python sets give their environments are the theorem classes
  here, `theoremlike`, `definitionlike` and `prooflike`
  ([`TheoremClass`](crate::stdcore::features::theorems::TheoremClass)), and the
  settings of a Python environment map to a theorem kind
  ([`TheoremKind`](crate::stdcore::features::theorems::TheoremKind)) and its
  class: whether it is numbered and whether it shares the common counter
  (`shared_numbering: False` is `shared_counter: false`, a counter of the
  kind's own, `flm.theorems.counter.<kind>`), its title forms (`words`), the
  closing mark (`closing_mark`), and whether its title may refer to the
  statement it proves (`relation_ref`).
- The accepted label prefixes, `allowed_ref_label_prefixes`, are the single list
  [`label_types`](crate::stdcore::features::theorems::TheoremsConfig::label_types),
  `thm, prop, cor, lem, rem, def, dfn, x, topic` by default, accepted by every
  numbered theorem kind.
- A theorem reads `Theorem 1 (My theorem).` and a proof `Proof (Theorem 1).` or
  `Proof.`, ending with `□`; all numbered theorem kinds count in one sequence by
  default. A `\label` on a proof is refused, as the Python implementation
  refuses it; here the report is the references feature's
  `flm.refs.label-type-not-accepted`.

The differences:

- **The title forms are written out, never derived.** A theorem kind's words are
  the numbering feature's
  [`PrefixWords`](crate::stdcore::features::numbering::PrefixWords): the
  capitalized pair is the base (`Theorem`, `Theorems`), and named variants
  (`lowercase`, `capital`, `abbreviated`, or a name of the host's) hold the
  other forms, which `\ref[abbreviated]{thm:a}` asks for by name. The Python
  implementation derives an abbreviation it is not given from the first three
  letters of the capitalized form (`The.`). Here nothing is derived: every
  standard theorem kind declares its abbreviation, written out with the Python
  implementation's words (`Thm.`, `Prop.`, `Lem.`, `Conj.`, `Qtn.`, …), except
  the plural of `corollary`, written `corollaries` where the Python
  implementation adds an `s`. Asking a theorem kind for a variant it does not
  declare is the references feature's Error `flm.refs.undeclared-variant`, and
  the usual wording is written.
  [`TheoremKind::from_word`](crate::stdcore::features::theorems::TheoremKind::from_word)
  derives the two regular forms from one word, as the Python implementation
  does, and no abbreviation, because a derived abbreviation is wrong for many
  words.
- **The heading is a template per theorem class.** How the theorem kind's name
  and number join the title is the class's theorem heading template
  ([`TheoremClass::heading`](crate::stdcore::features::theorems::TheoremClass::heading)),
  `#{name}\IfValueT{#{title}}{ (#{title})}` by default, rather than the Python
  feature's two strings `heading_title_pre` (` (`) and `heading_title_post`
  (`)`). The template gives the same default and can also put the title first
  or join it with a word (`Proof of Lemma 4`).
- **No counter formatter.** The Python `counter_formatter` of each environment
  and `shared_counter_formatter` have no counterpart: every theorem kind's
  number is written in the single [number
  format](crate::stdcore::features::theorems::TheoremsConfig::number_format),
  `#{arabic}` by default. A host that wants one kind's number written otherwise
  replaces that kind's counter style, `flm.theorems.style.<kind>`, through
  [`NumberingConfig`](crate::stdcore::features::numbering::NumberingConfig), the
  way every declared counter style is replaced. Numbering theorems within
  sections is a numbering option of the document, as for every counter, on the
  shared counter `flm.theorems.theorem` (the Python `_theorems_shared`).
- **Whether a statement expects a proof is set per theorem kind**
  ([`expects_proof`](crate::stdcore::features::theorems::TheoremKind::expects_proof)):
  `theorem`, `proposition`, `lemma` and `corollary` do; a conjecture does not,
  since it has no proof by nature.
- **`\noproofref` is read.** The Python implementation records `\noproofref`
  and checks nothing. Here a statement that expects a proof, that no proof's
  back-reference (`[*lem:x]` or `[**lem:x]`) refers to, and that carries no
  `\noproofref`, is reported with the Warning `flm.theorems.missing-proof`. The
  check runs under every workflow, rendering or not, as every check of the
  standard features does.
- **A back-reference is a reference** resolved by the references feature: one
  that refers to a label nothing declares is an unresolved reference, reported
  as any reference is and shown as the label's text. A proof's title that
  begins with a star but is not a star followed by a label written as plain
  characters is reported as `flm.theorems.bad-relation-ref` and shown as
  written.
- **Neither the references feature nor the numbering feature is required.**
  Without the references feature no label resolves, and a back-reference shows
  its label's text; without the numbering feature a theorem's heading shows the
  kind's word alone. The Python feature requires its references feature.
- **The text output** writes a no-break space before the closing mark, as the
  Python implementation does, and after the heading the stop that the text
  backend writes after every run-in heading (followed by one space).

## Quotations

The Python implementation's `quotation` feature is the
[`quote`](crate::stdcore::features::quote) feature here: the environments
`quote`, `blockquote` and `address`; the quotation-section commands `\text{…}`,
`\lines{…}`, `\attributed{…}` and `\block{…}`, each of which writes one part of
a quotation (a quotation section); and, inside a lines section, `\\`, which ends
a line, and `\indent`, which indents the line it starts. What carries over:

- The three environments and what each accepts: `quote` accepts the four
  quotation-section commands and no content outside them; `blockquote` accepts
  no quotation-section command, its content being one block section; `address`
  accepts none either, its content being one lines section. A Python
  environment's list of accepted sections and its bare-content setting are a
  [`QuoteEnvironment`](crate::stdcore::features::quote::QuoteEnvironment)'s
  `sections` (a set: the order of the Python list has no meaning) and `bare`
  ([`QuoteSection`](crate::stdcore::features::quote::QuoteSection)), configured
  through [`QuoteConfig`](crate::stdcore::features::quote::QuoteConfig).
- A quotation-section command that an environment does not accept is not a
  command in its body, as in the Python implementation, which adds only the
  accepted sections to the parsing context of the body.
- A lines section is split at every `\\`; the `\indent`s at the start of each
  line are counted, and the whitespace at both ends of the line is removed. An
  `\indent` after the start of its line is an error, as the Python
  implementation raises one.
- The text output prints the quotation sections as blocks, one blank line
  apart, with no prefix: `To be or not to be.` then `Shakespeare`.

The differences:

- **The layout of the body is derived, not configured.** The Python
  environment's third setting, `content_is_block_level`, has no counterpart.
  The body is laid out as paragraphs where the content outside the
  quotation-section commands is read as a block section or not at all, and as
  one run of inline content where that content is read as a passage, lines or an
  attribution. The Python implementation warns when its setting disagrees with
  the bare-content setting, so the setting could only repeat or contradict that
  other setting.
- **An indent is a length in tabs.** Each `\indent` is one tab of indentation, a
  [`Length`](crate::stdcore::backends::pres::Length) whose unit each backend
  gives a width (four columns on the text output,
  [`tab_width`](crate::stdcore::backends::text::TextRenderOptions::tab_width)),
  rather than a count of columns. On the page a line carries its indentation as
  the custom property `--flm-element-indent-left` of its `style` attribute, a
  multiple of `--flm-tab` (`2em` by default), which the stylesheet applies. The
  Python HTML renderer writes an empty spacer element of a fixed width for each
  `\indent`.
- **Every quotation is a `<blockquote>` on the page**, `address` included,
  carrying the environment's name as its kind token (`<blockquote
  class="flm-pres-block flm-quote" data-flm-kind="address">`). A role is a
  feature-owned dotted name on a presentation payload that says what a container
  is, and a kind token is a feature-owned name that classifies it further, so
  that a stylesheet or a host's lowering rule can select on them; the page
  writes the kind tokens in the attribute `data-flm-kind` (see
  [`payload`](crate::stdcore::backends::pres::payload)). Each quotation section
  is a `<div>` of its role's class: `flm-quote-attributed` where the Python
  renderer writes `quote-attributed`. The tag comes from the HTML backend's
  table of tags by role, never from the environment, so a stylesheet selects one
  environment by its kind token (`.flm-quote[data-flm-kind~="address"]`). The
  feature's stylesheet, `flm.quote.style`, sets a quotation apart as the Python
  stylesheet does (a `quote` on a faint background, its passage and lines in
  italics, a block section with a rule on its left, the attribution on its own
  line at the far side after a dash), and also indents every quotation on both
  sides.
- **The recomposed LaTeX writes every quotation section through a LaTeX hook.**
  A quotation is written back as its environment, and each quotation section
  through the hook of its kind, `\flmQuoteText{…}`, `\flmQuoteLines{…}`,
  `\flmQuoteAttributed{…}` and `\flmQuoteBlock{…}`, which the style file defines
  with `\ProvideDocumentCommand` and a user may redefine; a line's indentation
  is `\flmIndent{1\flmTabWidth}` rather than one `\indent` per count. What this
  replaces: there is no `\flmQuoteSetup{<name>}` line at the start of every
  quotation (redefine a hook, or the environment, instead); the passage and the
  lines are written through their hooks rather than bare; and the recomposer
  options have no counterpart, neither `keep_as_is` nor `text_macro`,
  `lines_macro`, `attributed_macro` and `block_macro`, which renamed a
  quotation-section command, since redefining the hook does what renaming did.
- **The lines of a lines section are joined by `\\\relax` and a line break**,
  rather than by `\\` and a line break. After a bare `\\`, LaTeX reads a line
  beginning with `[` as the optional argument of `\\`, which fails to compile,
  and a line beginning with `*` as its star, which loses the `*`. The lines hook
  starts its paragraph before the first line, so a lines section or an `address`
  that begins with `\\` compiles, where the Python recomposer's output stops
  with LaTeX's error that there is no line to end.
- **The style file defines `blockquote` and `address`** for a document holding
  a quotation, where nothing defined them before. An environment that a host
  configures is written under its name, and the author's own LaTeX must define
  it.
- **The errors keep the content.** The Python implementation stops at content
  outside the quotation-section commands of `quote`, or at an `\indent` in the
  middle of a line, and produces nothing. Here each is an Error and the
  document is still processed: `flm.quote.bare-content`, once for each stretch
  of such content between two quotation sections, the content shown where it
  was, inside the quotation; `flm.quote.indent-mid-line`, the `\indent` left
  out; and `flm.quote.nested-section` for a quotation-section command inside
  another section's argument (`\lines{\text{x}}`), shown as its content.

## Annotations

The Python implementation's `annotations` feature is the
[`annotations`](crate::stdcore::features::annotations) feature here: a
reviewer's command for each annotator, with `\phf{…}` and `\phf … \endphf`
highlighting a passage and `\phf[…]` writing a reviewer's comment. What carries
over: the three spellings and their meanings; the annotators, configured with a
command and initials and numbered in configuration order (the number on which
an output keys a color); and `\phf{x}[y]`, a highlight of `x` followed by the
text `[y]`, as in the Python implementation and in the `phfcc` LaTeX package.
The configuration is
[`AnnotationsConfig`](crate::stdcore::features::annotations::AnnotationsConfig),
an ordered list of
[`Annotator`](crate::stdcore::features::annotations::Annotator)s. The
differences:

- **The initials are a decoration beside the text.** The Python implementation
  writes the initials inside the highlight's or the comment's span, as an
  inline span. Here they are a margin decoration at the start of the annotation
  (a decoration is a node whose element carries the
  [`Decoration`](crate::stdcore::backends::pres::Decoration) aspect, which the
  lowering moves out of the content into an attached slot of the node around
  it), so that
  an output may set them beside the line, as `phfcc` does in the margin. The
  page writes the decoration where its mark is, as a
  `<span class="flm-pres-deco-margin">` inside the highlight's or the comment's
  span, and the feature's stylesheet sets it in a column to the right of the
  text; the colors are selected by the kind token `annotator-<n>`, where the
  Python implementation's classes were `annotation-<n>`. The text output prints
  a highlight as `[PhF] passage` and puts a comment's brackets around its
  remark, `[PhF: remark]`.
- **The recomposed LaTeX relies on `phfcc`.** The Python implementation's
  pure-LaTeX recomposer writes the command as typed and leaves `\phf` undefined
  in LaTeX. Here the command is written as typed too, a highlight across
  paragraphs as one command, and the style file requests the `phfcc` package and
  creates each configured annotator's command with `\phfMakeCommentingCommand`,
  guarded on its name, so the recomposed LaTeX compiles and shows the marks as
  the package sets them.
- **A highlight across paragraphs is cut, never a block.** Where the Python
  implementation makes an annotation holding paragraphs one block that holds
  them, here a blank line inside a highlight is a paragraph break of the text
  around it: the highlight is cut into parts that belong to the enclosing
  paragraphs, the initials on the first part, and a list inside it is a block
  of the text around it, still highlighted.
- **Hiding is a render option.** The Python feature's flag that hides every
  annotation is the render option
  [`AnnotationsRenderOptions::hidden`](crate::stdcore::features::annotations::AnnotationsRenderOptions::hidden),
  set per render: a highlight is shown as its passage alone, and a comment not
  at all.
- **Empty or blank initials are refused** when the registry is built; the
  Python implementation reads empty initials as none. To have an annotator whose
  marks show no initials, leave the initials out.
- **An unclosed passage**, `\phf …` with no `\endphf`, is reported
  (`flm.annotations.unterminated`) and highlighted up to the end of the
  document or of its enclosing group.

## Graphics

The Python implementation's `\includegraphics`, which its `floats` feature
offers inside a figure and which its `graphics_resource_provider` feature
describes, is the [`graphics`](crate::stdcore::features::graphics) feature here.
What carries over: the command inside a float's body by default; what is known
about a file comes from the host, here through the graphics resolver and the
inspector that a host contributes
([`GraphicsResolver`](crate::stdcore::features::graphics::GraphicsResolver),
[`GraphicsInspector`](crate::stdcore::features::graphics::GraphicsInspector)),
where the Python implementation's resource provider looked the file up; and the
recomposed LaTeX's default of an image kept within the line's width
(`max width=\linewidth`). The differences:

- **Options in brackets are accepted.** The Python implementation refuses any
  bracket on `\includegraphics`. Here `[alt=…, width=…, height=…]` are accepted,
  and any other key is an Error, the image still shown. Each `width=` or
  `height=` is reported with a Warning: an image file prepared at the size it is
  shown at is better than one resized by the output.
- **A point is 1/72 inch.** `width=10pt` is ten points of 1/72 inch, the point
  of CSS and PostScript, not TeX's point of 1/72.27 inch, and the recomposed
  LaTeX writes it `10bp`. The units are `pt`, `mm`, `cm` and `in`; `em`, `px`,
  `\linewidth` and percentages are refused.
- **One dimension keeps the proportions.** With `width=` alone and a size that
  the host's inspector reported, the height is computed so that the image keeps
  its proportions, in the width's unit, and written beside the width on every
  output.
- **The page writes custom properties, not a size.** Where the Python renderer
  wrote `width` and `height` directly into an image's `style` attribute, the
  page here writes the custom properties `--flm-element-width` and
  `--flm-element-height` (and their ratio, `--flm-element-aspect-ratio`), which
  the core stylesheet applies, so that a page's own rule can restyle an image
  without competing with an inline style.
- **Magnification is a per-output graphics setting.** The renderers' options
  `graphics_raster_magnification` and `graphics_vector_magnification` are the
  render option `graphics.magnification` here, a map from an output's name to
  `{raster: …, vector: …}`
  ([`GraphicsRenderOptions::magnification`](crate::stdcore::features::graphics::GraphicsRenderOptions::magnification)):
  `graphics: {magnification: {html: {raster: 1.5, vector: 1.25}}}`. As in
  Python, every dimension of the image is multiplied, the `width=` and
  `height=` the author gave included, and an image whose type is not known
  takes the vector factor. The size is computed once, by the `graphics`
  feature, for every output. The recomposer's `width_scale` and
  `set_max_width` options have no counterpart: a LaTeX author who wants
  another default redefines the LaTeX hook `\flmIncludeGraphics`.
- **An image of unknown size is reported.** An image given neither `width=`
  nor `height=` whose file was not measured (no inspector, a failed
  inspection, a web address) is reported with the Warning
  `flm.graphics.unknown-size` on each output, and written without a size.
- **The command anywhere, inline outside a float.** The Python implementation
  offers the command inside a float only, as a block, the float's content. Here
  a setting of the configuration (fixed when the registry is built),
  [`GraphicsConfig::global`](crate::stdcore::features::graphics::GraphicsConfig::global),
  makes it an ordinary command too, written wherever text may be written. An
  image is a block only as a direct child of a float's body; everywhere else it
  is inline content, a word of the text around it: in a sentence, in a
  paragraph of its own, in a heading's title, in a caption. An image meant to
  be set apart from the text is a float, left without a `\label` (or given
  `\nonumber`) where no number is wanted, so that there is one way to set a picture apart, and an image in the
  text serves as an icon or a symbol. Inside a float, the Python behavior is
  unchanged.
- **Files are read by the host.** The library opens no file. The `flm` program
  finds, measures, converts and copies image files through the services that its
  feature `graphics-files` contributes ([the command-line
  chapter](crate::guide::command_line)). As in the Python implementation, a path
  is looked for in the folders of a search path (the setting
  `graphics-files.search_path`), each taken relative to the folder of the file
  that refers to the image, `.` by default, with no extension added; the size is
  read from the file (an SVG number with no unit read as points, a PDF file's
  first page); a file shown twice is copied once; and the program's default
  rules turn a PDF drawing into a PNG image at 192 dots per inch for the page,
  and an SVG drawing into PDF and a GIF image into PNG for the recomposed LaTeX.
  The differences in the `flm` program:
  - *The page holds its images.* The Python program copied every image into a
    folder beside the output. The page here writes each image's content into
    itself as a `data:` address, by default, and copies the images under
    `graphics.strategy.html=copy`: one file that shows its images wherever it is
    sent, and nothing written beside the output unless asked.
  - *The recomposed LaTeX refers to its images where they are.* The Python
    program copied them for its LaTeX output too. Here the recomposed LaTeX
    writes each image's path as seen from the output's folder, and copies the
    images, converted by the same two rules, under
    `graphics.strategy.latex-recomposed=copy`.
  - *The name of a copy.* The Python program named the copies `gr1.png`,
    `gr2.png`, from a template (`gr${counter}${ext}`), in a folder
    `_flm_collected_graphics`. Here a copy is named after its file and eight
    digits of a digest of its content and conversion (`plot-3fa9c2d1`), and is
    placed beside the page as every other file of the output is, after the
    stem of the page's name (`paper-plot-3fa9c2d1.png` for `-o paper.html`,
    `index-plot-3fa9c2d1.png` in a folder), because an address that depended on a count
    over the document could change between two parts of one build. There is no
    template.
  - *The settings.* The Python program's `collect_graphics_to_output_folder`
    (the folder of copies, `false` for none) is the setting `strategy` here, per
    output, and its `collect_format_conversion_rules` is the setting `convert`,
    a map from an output's name to its rules. A rule gives its formats by their
    short names, `pdf` where the Python program wrote `.pdf`, and is written as
    a map only: the Python short form `.svg:.pdf`, and formats listed in one text
    separated by commas, are not read; a list is a YAML list. A rule states the
    formats it applies to (`from`), where a Python rule with no `from` applied to
    every format. A rule with several target formats is refused when the
    settings are read, where the Python program used the first and wrote a
    warning to its log. A rule that no converter can perform is an error at the
    image, which is published unconverted, where the Python program wrote a
    warning to its log and tried the next rule.
  - *Converters.* The Python program turned a drawing into pixels with one of
    `cairosvg`, Ghostscript, `pdftocairo` or `magick`, whichever it found. Here
    the converter `raster` turns a raster image into PNG in the program itself,
    and the converter `magick` runs ImageMagick for every other pair of formats,
    a PDF or SVG drawing among them. A machine without ImageMagick reports an
    error at such an image, regardless of what else is installed.
  - *Where the converting program is found.* The Python program looked for
    `magick` in the environment variable `MAGICK` (`false` switching it off),
    then in a few fixed folders, then in the folders of programs. Here the
    variable `MAGICK` gives the program where it is set, else the name `magick`,
    which the system looks for in its folders of programs; no fixed folder is
    searched, and `false` has no special meaning (it would be a program called
    `false`). No setting gives the program: it depends on the machine `flm` runs on,
    and a document cannot choose a program that `flm` starts.
  - *The first page or frame.* The Python program converted only the first
    frame of a GIF or MNG image. Here the first page or frame of every image is
    converted: a PDF file of several pages makes one picture of its first page,
    the page whose size is measured.
  - *No cache of conversions between runs.* The Python program kept its
    conversions in a file beside its copies (`.flm-output-metainfo-cache.json`)
    and reused them at the next run. Here a conversion is kept while the program
    runs (an image shown twice is converted once, and so is an image that did
    not change between two rebuilds of one preview), and a new run converts
    again.
  - *An image at a web address* (`https://…`) is written as it is and is not
    measured; the Python program downloaded it, measured it and copied it. An
    address beginning with `file:` is such an address too, written as it is;
    the Python program looked up the path after `file:` as a file.
  - *A raster file that states no resolution* is taken at 96 dots per inch, with
    a Warning (the Python program refused it, except for a GIF file).

## Floats

The Python implementation's `floats` feature is the
[`floats`](crate::stdcore::features::floats) feature here: `figure`, `table`
and the float types a host configures, each an environment whose body holds its
content, a `\caption{…}` and `\label{…}`s, shown where it is written. What
carries over: the two standard float types and their words (`Fig.~` and
`Figs.~`, `Figure~` under `\ref[S]{…}`, `Tab.~` and `Table~` for `table`, and
`<Name>~` for a configured type); the counter of each type; what each type
admits (exactly one image for `figure`, one table of cells or one image for
`table`, anything for a type that admits `any`), declared through content
handlers (a content handler is a value that accepts a float's content or refuses
it, contributed by the feature that owns that content;
[`FloatContentHandler`](crate::stdcore::features::floats::FloatContentHandler));
and the line under a numbered float's content, `Figure 1: ` followed
by the caption. The configuration is
[`FloatsConfig`](crate::stdcore::features::floats::FloatsConfig), an ordered
list of [`FloatType`](crate::stdcore::features::floats::FloatType)s, each
declaring its content handlers and its accepted label types (its own name by
default: `\label{figure:x}`). The differences:

- **A float type has a numbering policy.** The Python implementation numbers a
  float when it carries a `\label`. Here each float type has a numbering policy
  ([`FloatNumbering`](crate::stdcore::features::floats::FloatNumbering)):
  `AutoNumbered`, the default of every type, `figure` and `table` included,
  numbers a float that carries a `\label` and no `\nonumber`, which is the
  Python behavior; `Numbered` numbers every float unless it carries
  `\nonumber`, as a displayed equation is numbered; `Unnumbered` numbers none. A
  host configures the policy per type. A `\nonumber` that changes nothing is
  reported with a Note.
- **The caption's bracket is `alt=`.** The Python implementation refuses a
  bracket on `\caption`. Here `\caption[alt=Short]{…}` gives the caption an
  alternative text, which a reference to a float with no number shows; any other
  key, a bare `\caption[Short]{…}` included, is reported and ignored.
- **A caption with no number has no type word.** The Python implementation
  writes `Figure: ` before the caption of a float with no number. Here that
  float's line is its caption alone; the caption block carries the type and
  `unnumbered` among its kind tokens, so a stylesheet rule, or a host's own
  lowering rule for the float, can add a word where one is wanted.
- **Every float can be referred to.** A reference to a numbered float shows its
  number; to a float with no number, its alternative text, else its caption; to
  one with neither, the label as written, which is reported
  (`flm.refs.nothing-to-show`).
- **No separator lines in the text output.** The Python text renderer draws a
  line of middle dots above and below a float and centers the image's line.
  Here the text output prints the float's blocks one blank line apart, as it
  prints any blocks.
- **The content is checked where the float is shown.** Content that its type
  does not admit is reported (`flm.floats.content-refused`) and shown as
  written; a second `\caption` is reported and not shown. A content handler that
  a type declares but that no registered feature provides admits nothing, and is
  reported once per run.
- **The recomposed LaTeX shows an unnumbered caption unnumbered.** The Python
  recomposer's `flmFloat` environment, its four cases (`NumCap`, `NumOnly`,
  `CapOnly`, `Bare`) and its redefinable `\flmFloatSetUseConfig<Case>` and
  `\flmFloatPlacementArgs<Case>` are kept, with one change. In the Python
  recomposer, a float with a caption and no number lets LaTeX number the
  caption; here that case sets the caption with `\caption*` of the `caption`
  package, which the style file requests, so the PDF shows what FLM's other
  outputs show. A reference to such a float prints its caption (its `alt` where
  one is written), and each float type the document uses declares its words for
  `zref-clever`, the package behind the LaTeX hook `\flmRef`. The Python
  recomposer's float options `keep_as_is` and `captioncmd_for_num_only` have no
  counterpart: a numbered float with no caption is written with an empty
  `\caption{}`, the default of that option, and a float is set another way by
  redefining `\flmFloatSetUseConfig<Case>`.
- **A configured float type is numbered and named as the standard ones are.**
  The Python implementation numbers a float type it does not ship in capital
  roman numerals and names it by its type as written (`diagram`). Here every
  float type is numbered in arabic numerals and named by its type capitalized
  (`Diagram 1`); `FloatType::number_format` and `FloatType::caption_name` set
  others.
- **A configured float type's LaTeX environment is the author's.** `figure` and
  `table` are LaTeX's own environments. The environment of a type that a host
  configures (`diagram`) is written under its name and must be defined by the
  author's LaTeX, for instance with `\usepackage{newfloat}` and
  `\DeclareFloatingEnvironment{diagram}` before the style file is loaded, as the
  environment of a configured list kind or quotation is. The style file declares
  none.

## Tables of cells

The Python implementation's `cells` feature is the
[`cells`](crate::stdcore::features::cells) feature here, ported step for step:
the `cells` environment; `\cell<styles>[placement]{content}`;
`\celldata<styles>[mapping]{data}` with `\\` and `&`; `\merge{…}`; the small
language of placements and mappings (`ROW;COL`, `2-4`, `2+3`, `1,2`, an open
range, `.` for the current index); the current row and column, advanced as the
Python implementation advances them; and the cell style tokens `H`, `rH`, `l`,
`c`, `r`, `lvert`, `rvert`, `green`, `blue`, `yellow`, `red`. The differences,
each with its reason:

- **Where the environment exists.** The Python implementation declares
  `cells` everywhere in a document. Here it exists inside a `table` float by
  default, and wherever blocks may be written under the setting
  `cells: { global: true }`
  ([`CellsConfig::global`](crate::stdcore::features::cells::CellsConfig::global)),
  which gives the Python behavior: a host decides whether its authors write
  tables only inside a float.
- **Blank lines in the body** are whitespace here, as an author of a long table
  expects. The Python implementation refuses a blank line between two rows, a
  side effect of its check rather than a design.
- **A `\celldata`'s data is inline content**, one paragraph per entry: a blank
  line inside it is an error. An author who wants paragraphs or a list in a cell
  writes `\cell{…}`, whose content is blocks, as in the Python implementation.
  There is one rule (the data of a `\celldata` is inline, the content of a
  `\cell` is blocks), rather than a reader that splits paragraphs and blocks at
  `&` and `\\`.
- **Cell style tokens are declarations.** A token refers to a cell style
  declared in the service slot `flm.cells.styles` (a service slot is the
  declaration, by the feature that owns an interaction, of a dotted name and the
  type of the values that any feature may contribute to it; see
  [`services`](crate::core::services)), which a host extends. A word that no
  declaration covers is an error here, where the Python implementation wrote any
  word as a class. Tokens are split at whitespace (the Python implementation
  split at single spaces, so that `<H  l>` gave an empty token), and a token may
  carry a value in braces: `align{right}` is `r`, `head{row}` is `rH`,
  `rule{above}` draws a rule at the top edge. Of two tokens that give one cell
  an alignment, the first decides on every output (the Python HTML took the
  last, and its LaTeX looked for `l`, `c` and `r` in that fixed order, so that
  `<r l>` was `l` there).
- **A `\\` at the end of a `\celldata`'s data** ends the last row and adds no
  row: `\celldata{A & B \\ C & D \\}\cell{E}` places `E` on the third row. The
  Python implementation counted the empty part after the final `\\` as a row,
  so that `E` went on the fourth row and the third stayed empty; LaTeX, whose
  syntax this one imitates, reads a trailing `\\` as the end of the last row. A
  `\\` on its own inside the data, `\celldata{A & B \\ \\ C}`, still makes an
  empty row.
- **Names for rows and columns**, which the Python implementation declared and
  never filled, are implemented: `\rownames{…}` and `\colnames{…}`, usable
  wherever a number is.
- **A span to the end** (`\merge{2-}`) reaches the last column of the table as
  finally built, regardless of the order in which the cells were written; the
  Python implementation ended it at the last column of the cells written before
  it.
- **Mistakes are errors, and the table is still shown.** Where the Python
  implementation refused a document holding stray content in the body or two
  overlapping cells, here the stray content and the later cell are left out,
  and a dropped mark is placed immediately before the table for each
  (`[dropped: …]` in plain text, a box on a page, a comment line in LaTeX). An
  empty table writes nothing, and a table larger than a million positions
  refuses the cell that would make it so.
- **The plain-text output** lays the table out in columns, through a layout
  table of the [layout engine](crate::layout), each cell in its own alignment,
  the content of a cell that spans several positions in the first position of
  its span, and the rows of column headers above a rule. The Python text
  renderer printed the lines of each cell one under the other and had a
  `cells_column_sep` option, which has no counterpart.
- **The HTML output** writes the table's facts, not its tokens. A cell is a
  `<td>`, or a `<th>` for a header as in the Python implementation, with
  `colspan` and `rowspan` as there. The rest differs:
  - The Python implementation's `cellstyle-<token>` classes (`cellstyle-H`,
    `cellstyle-c`, `cellstyle-green`) become one attribute per fact:
    `data-flm-align` for the alignment, `data-flm-rules` for the edges at which a
    rule is drawn, and `data-flm-style-<name>` for each custom cell style
    (`green`, or a host's own). The reason is that a token is the name of a
    declaration and several tokens state one fact (`r` and `align{right}`). A
    header's token becomes the `scope` attribute (`col` for a column header,
    `row` for a row header), which the Python implementation does not write and
    which a screen reader reads.
  - The edge classes (`celltbledge-top` and the three others), which marked each
    cell touching the table's outline so that the Python stylesheet could draw
    the outline on the cells, have no counterpart: the stylesheet draws the
    outline on the `<table>` element, and a cell's own rules belong to that cell
    alone.
  - The classes `cells`, `cell` and `cell-empty` become the table's class
    `flm-pres-cells`, and nothing on a cell: an empty position is an empty
    `<td>`.
  - The `data-…` attributes that the Python implementation copied onto the table
    from the first cell's information, for its own page machinery, have no
    counterpart.
  - The stylesheet is the feature's own [style
    definition](crate::core::registry::StyleDef) (a stylesheet recorded in the
    registry) `flm.cells.style`, which a page holds only when it holds a table;
    every value in it is a custom property that a host sets (the four colors
    among them).
- **The recomposed LaTeX** writes the Python LaTeX renderer's `tabularray` form
  (`hspan=minimal`, the `cell{r}{c}={r=…,c=…}{…}` entries, the `hline` under a
  column header and the `vline` of `lvert` and `rvert`, the rows between
  `\toprule` and `\bottomrule`, the columns `c` and then `X[-1]`) as one call of
  a LaTeX hook, `\flmCellsTable{<natural>}{<fitted>}{<options>}{<rows>}`. The
  style file defines the hook with the Python renderer's default method: the
  table set at its natural width, and set again with the fitted columns where it
  is wider than `\flmCellsMaxWidth`. The Python renderer wrote that method into
  the document itself (a `\long\def` of the table and the width test around it),
  chose among three methods through its `cells_render_method` attribute, and
  took the maximum width from its `cells_max_width_latexdim` attribute. Here only
  the call is written in the document, the single method belongs to the style
  file, and the width is the LaTeX macro `\flmCellsMaxWidth`, so that a LaTeX
  user changes everything in LaTeX: redefine `\flmCellsTable` to set tables
  another way, or `\flmCellsMaxWidth` for another width. The other two methods
  have no counterpart. A table placed where LaTeX sets a single line, as in
  another table's cell, is set without the centering, which cannot be used
  there. A rule at the right edge (`rvert`) of a cell that spans columns is
  drawn at the right edge of the span, where the Python renderer drew it after
  the span's first column, inside the span.
- **The background colors** are `flmCellsColorgreen`, `flmCellsColorblue`,
  `flmCellsColoryellow` and `flmCellsColorred`, with the Python values, where
  the Python renderer named them `flmTabCellColorGreen` and so on. A color,
  like every custom cell style, reaches a cell as the table command
  `\flmCellsSet{green}` at the start of its text, which applies the keys
  registered under that name (`\flmCellsStyleKeys{green}{bg=flmCellsColorgreen}`),
  rather than as a `bg=` key of the cell's entry, so that a host's own cell
  style reaches LaTeX in the same way with keys of its own. The old names have
  no counterpart. Of two custom cell styles that set one key on a cell (two
  colors, each a `bg=`), the later token wins, its table command setting the key
  last, where the Python renderer chose by a fixed order of its colors: green,
  then red, blue, yellow.
- **The recomposer's `keep_as_is`** option, which wrote `\begin{cells}` back as
  the author typed it, has no counterpart: LaTeX has no `cells` environment, so
  the recomposed LaTeX writes the table's grid.

## Citations

The Python `cite` feature is the [`citations`](crate::stdcore::features::citations)
feature here: `\cite[note]{keys}`; adjacent calls read as one citation chunk;
each cited work numbered in order of first appearance and shown as a citation
mark made of links to it (`[1,3–5]`); and its identifier `citation-<n>`, as in
the Python implementation. The feature is in the standard package, so every
host has it; the Python command line could not turn citations on at all. The
Python settings map to its configuration
([`CitationsConfig`](crate::stdcore::features::citations::CitationsConfig)):

- `citation_delimiters` to the delimiters of the citation counter style (the
  configuration's `style`, a counter style);
- `citation_optional_text_separator` to `note_separator`;
- `references_heading_title` to `heading_title` (`References` by default in
  both);
- `sort_and_compress` to the citation counter style's `ranges` (below);
- `use_endnotes=False` to the in-place placement,
  `placement: CitationPlacement::InPlace`, under which a citation mark shows
  its entries, `[Knuth, 1984][Lamport, 1994; Thm 4]`, as the Python
  implementation's does.

The Python implementation's external citation providers, asked in order through
`get_citation_full_text_flm(prefix, key, resource_info)`, are the citation
providers here: a
[`CitationProvider`](crate::stdcore::features::citations::CitationProvider)
contributed to the service slot `flm.citations.providers`. The providers are
asked in order, the first answer decides, and a provider's error ends the lookup
of the key, as in the Python implementation. The differences:

- The provider receives the citation key whole, as written
  (`arxiv:1234.5678`), and reads the prefix itself, where the Python
  implementation split the key at the first `:` and lowercased and trimmed the
  part before it.
- The provider decides which keys refer to one work: its answer
  ([`CitationAnswer`](crate::stdcore::features::citations::CitationAnswer))
  gives the work's identity, and keys answered with one identity are one work,
  with one number and one citation entry. The Python implementation ignored the
  letter case of the prefix but numbered by the key after the colon as written,
  so that `\cite{doi:10.1103/PhysRevA.86.052329}` and
  `\cite{doi:10.1103/physreva.86.052329}` were two works there, and are one here
  wherever the provider answers them with one identity. A key that no provider
  answers is taken as its work's identity as written, so that a key written
  exactly as a work's identity is a key of that work.
- A provider may be given all the keys of a document in one call
  ([`answer_all`](crate::stdcore::features::citations::CitationProvider::answer_all)),
  so that a provider that fetches over the network asks for many at once.
- A provider answers with a citation entry, parsed from FLM source through
  [`ProviderCx::parse_entry`](crate::stdcore::features::citations::ProviderCx::parse_entry)
  or built from parts through
  [`ProviderCx::build_entry`](crate::stdcore::features::citations::ProviderCx::build_entry),
  where the Python provider answered with a text or a fragment; an entry built
  from parts has no characters to escape.

The citation marks differ as the following table says, each for its reason:

| Python implementation | This library | Reason |
|---|---|---|
| a note on a call citing several keys is an error | valid: the note belongs to every key of the call, `\cite[Thm 4]{a,b}` reads `[1,2; Thm 4]` | the citation chunk is a list of `(key, note)` items, so a note on several keys has an obvious meaning |
| each item with a note written as its own group, in the order written | the keys without a note first, in one group, then one group per distinct note in order of its first appearance, two notes written alike (spaces aside) being one: `[2–4][1,5; Thm 4]` | the citation chunk is sorted and compressed in one step |
| the delimiters inside the link for one number, outside for several | always outside the links, each number its own link | one rule; a link never holds a bracket |
| two spellings of one DOI in one call, `\cite{doi:10.1/X,doi:10.1/x}`, show two numbers, as does a key written twice in one call, `[1,1]` | the numbers of a group are a set: two keys that the provider answers as one work show one number, `[1]`, as does a key written twice | a citation mark lists the works it cites |
| `sort_and_compress=False`: one citation mark per number, `[1][2][3]` | the citation counter style's `ranges` off: the numbers sorted and each written alone, `[1,2,3]`, one link each; every counter style has this switch | the same one link per number, without the delimiters repeated around each |
| the space before `\cite` as the author wrote it; the citation marks raised by the stylesheet (`.citation-marks`) | a `\cite` removes the space written before it, `~` and `\ ` included, and always writes its own, a no-break space before an inline, bracketed citation mark, regardless of what precedes it; the citation mark is raised only in the superscript form, which writes no space | a citation belongs to the text before it, and no rule can tell reliably from the text around it which space is wanted |
| a comment between two calls ends the citation chunk | a comment between two calls does not end the citation chunk | the parser passes over the comments between the calls it absorbs |
| — | `\R\cite{a,b}` writes the citation as a noun with its word, `refs. [1,2]`; `\R[S]` capitalizes the word, `\R[I]` writes the citation mark alone, inline, with no space of its own | new: a citation used as a word of its sentence, where an attached citation mark would bring an unwanted space |

The citation entries differ as this second table says:

| Python implementation | This library | Reason |
|---|---|---|
| a key that no provider knows, or no provider at all, raises an error and stops the render | the key is numbered and marked like any other, its line in the list (or its bracket in place) shows that the citation entry is missing, and an Error is reported; no provider at all is an Error of its own | a missing citation entry does not stop its key from being numbered, and the document is processed despite its mistakes |
| the providers asked while rendering | asked once per distinct key by a stage, also under `check`, which reports a missing citation entry as a render does | whether every cited key has a citation entry is part of what a check of the document reports |
| a provider's answer given as a parsed fragment used as it is, a paragraph or a list included (an answer given as text is read as inline content) | a citation entry holding a paragraph break or a block, such as a list or a heading, is refused with an Error, and its key has no citation entry | a citation entry is inline content: a line of the list, or the inside of a citation mark in place |
| the heading `References` written only under the render option `include_headings_at_level`, at the level it gives (the Python command line's configuration sets it to 1, for every list of notes); `endnotes_heading_level`, 1 by default, is the level of a heading above all the lists of notes | the heading written above the list by default, at the level `heading_level`, 2 by default, the level of the heading above a list of footnotes; `CitationsConfig::without_heading` places the list with no heading, as the Python library does when no level is given | the list is headed as a list of footnotes is, at the same level |

**The standard citation provider of the `flm` program.** The Python package
`flm-citations`, which fetches the details of cited works from the arXiv and
doi.org and reads bibliography files, is the feature `citations-standard` of the
host's package `stdmain` here, part of every document that the `flm` program
renders (the cargo feature `citations` of `flm-main`). Both fetch through the
same crate, `autocitefetch`, so the sources, the cache file
`.flm-citations.jsonl`, its lifetimes, the batching of keys and the waiting
between requests are the same. The differences:

| Python implementation | This library | Reason |
|---|---|---|
| the feature added to a document with `$import: pkg:flm_citations`, its settings under `flm_citations`; the bibliography files listed at the top level of the front matter, `bibliography:` | always present; the settings under `citations-standard`, the files under its key `bibliography` | the settings follow the library's names, one map per feature |
| `sources`, a list of `{name, config}`, each `config` giving its `cite_prefix` | `sources`, a map from the prefix to `{kind, …}`: `{thesis: {kind: bib, files: [theses.yaml]}}`; an extension (a YAML file of settings that a document lists under `use`) adds one, `false` removes one, and the others stay | a collection of named things is combined entry by entry, as every such setting is |
| `drop_csl_fields`, `bibliography_file` | `dropped_fields`, `files` | the library's names |
| `cache_dir` and `cache_base`, settings | no setting: the `flm` program keeps the cache file beside the document; a program that embeds the host library chooses another folder, or none | where a file is written on the machine is decided by the program, not by a document |
| `on_citation_error: fatal` (the default) stops the render at a work with no citation entry; `warn` shows `[missing citation: key]` | no such setting: a work with no citation entry is an Error at its citation, its line in the list showing the entry missing, and `--continue-with-errors` writes the output | the library decides what a missing citation entry shows; the command decides what an error stops |
| a bibliography file named after the document, `<jobname>.bib.json`, read without being listed | only the files that the settings list are read | every input a build reads is listed where a reader finds it |
| the records of a bibliography file kept for 60 seconds (`ttl_seconds`), so that a file changed within that time is not read again | read again at every build, never written into the cache | a file changed between two builds changes the citation entries, as the live preview needs |
| a bibliography source's `format`, giving the format of its files, CSL-JSON or CSL-YAML | no such setting: the content tells the format, JSON where the file parses as JSON, YAML otherwise | the file already says which format it is in; a setting would repeat it |
| a bibliography file given by a web address (`https://…`) fetched | a bibliography file is a file, read as every file of a build is | every file a build reads is on its list of files read |
| a preprint and its published version cited side by side, `\cite{arxiv:…}` and `\cite{doi:…}` where the arXiv gives that DOI, shown as two citation entries and nothing more | two citation entries too, and a Warning at the preprint's first citation that gives the two keys | the author learns that two keys refer to one work, and decides whether both are meant |
| `manual_format`, the name of the markup a `manual:` key is written in | none: a `manual:` key is FLM | one meaning for the key |
| the keys of a `\cite` read as a list separated by commas, the braces of a `manual:` key respected, so that `\cite{manual:{X et al., Journal of Future Results (2034)}}` is one key, which may hold a comma and a command; each key's text taken as written, so that two spellings of a key that differ only in their inner whitespace (`manual:{A   B}` and `manual:{A B}`) are two works | the braces respected too: the keys of a `\cite` are FLM content, split at the commas outside every group, each recomposed to the source written for it with every run of whitespace written as one space, so that two such spellings are one work; a key is read as any other content of the document, under the document's settings | a key is its FLM source, as a defined term's identity is, a run of whitespace counting as one space |
| `ttl_policy`, `retry`, `max_chain_depth`, `refresh_batching`, `prune_cache` | not settings yet: the crate's defaults apply | not implemented yet |
| the progress written to the logger `flm_citations.retrieve`, at its level `INFO` or `DEBUG`; `on_event`, a function receiving each event | a listener that the program embedding the host library gives; `flm render -v` writes each pass, request and wait to standard error | the host library prints nothing; the program decides what to show |
| citation entries set by the Python formatter `citeproc-py` in the package's patched style "American Physical Society - et al." | citation entries set by the Rust formatter `hayagriva` in a copy of the same style file, its information block and one empty `<else>` brought to the form the CSL schema requires | the Rust formatter of the Citation Style Language (CSL); two formatters reading one style may differ in small ways, such as a word with capitals inside it written in title case by a style (`The TeXbook` becomes `The Texbook`), in a title or in a container's name alike |
| `bib_csl_style`, a style file | `style`: `{file: …}`, a style file, or `{name: …}`, a style of the formatter's archive | the archive's styles are available by name |
| the links after a citation entry fixed: the DOI and the arXiv entry, the address only where neither exists | the same by default, each a setting: `doi_link`, `arxiv_link`, `url_link` (`always`, `never`, `only-if-no-other-link`) | new: a small addition, with the defaults unchanged |
| the citation entry written as FLM text and parsed, the characters special to FLM escaped only inside italic and bold runs, so that a `%` in a title drops the rest of the entry | the citation entry built from the formatter's runs, every character shown as it is, a formula between dollar signs, `$x^2$`, read as an inline formula regardless of the document's `dollar_math` setting; a formula that does not parse is an error at the citation | a character of a record cannot break or change a citation entry |
| an author written with `name` alone (no `family`, no `given`) read as having that family name | the same for every list of persons of a record: authors, editors, translators | one rule for every person; an editor written that way would otherwise be lost |
| small capitals, superscript, subscript and underlined runs written in italics | shown as plain characters | structure, not styling: only emphasis and strong importance have a meaning in a citation entry |
| a record whose date is a range given to the formatter with its range; what the citation entry then shows was not checked | no citation entry, and an error at the citation | the Rust formatter cannot set a range |
| `write_csljson_file` and `write_bibtex_file`: the exports written while the page is rendered, under the names set, into the folder the program runs in | each export an output of its own: `flm render -p bibtex` or `-p csl-json`, written where `-o` says | an output is written where the command says and goes through the output's checks |
| the exports name each record `prefix:key` as the prefix is read (lowercased), and the CSL-JSON export adds `key` beside `id` | each record identified by its key as written, `id` alone; two spellings of one key are two records | the LaTeX that the program writes cites the keys as written, and LaTeX must find each |
| the BibTeX file written through a second style file, its entries sorted by author and year | written from the record's fields by a mapping of its own, after the same two corrections (`issued` from `published`, the ISSNs joined), in order of first appearance; the characters special to LaTeX written by their commands, the DOI, the arXiv identifier and the address as they are; a formula written as it is between its dollar signs; a name with no given name braced, so that BibTeX reads it as one name; the same fields, except that a `report` is written `@techreport` with `institution`, where the Python style wrote `@book`, and `number` comes from `issue`, else from `number`, where the Python style wrote the two run together | one serializer, no second style; the two differences in the fields fix two defects of the Python style |
| a cited work with no record left out of the exports, its failure reported once, when it was fetched | left out, and reported again by the export, at its citation | each part of the program reports what it finds |

**The recomposed LaTeX.** The Python recomposer writes a citation chunk as
`\NoCaseChange{\protect\cite{a,b}\protect\cite[{Thm 4}]{c}}`: the keys without a
note first, then one `\cite` per key with a note. Here each call is written as
the author wrote it, through three LaTeX hooks that the style file defines
([`citations::latex`](crate::stdcore::features::citations::latex)):
`\cite[Thm 4]{a,b}\cite{c}` becomes
`\flmCitePreSpace\flmCite[Thm 4]{a,b}\flmCite{c}`, and `\R[S]\cite{a,b}`
becomes `\flmRefCite{Refs.~}{\flmCite{a,b}}`, the keys as typed. `\flmCite`
defaults to `\cite`, `\flmCitePreSpace` to `\unskip~`, and `\flmRefCite` to the
word followed by the citations. In place of the `\NoCaseChange{\protect …}`
wrapper, the style file adds `\flmCite` to the LaTeX kernel's list of commands
whose argument a change of letter case leaves alone, as the kernel does for
`\cite`: in a heading set in capitals the keys stay as written and the note is
uppercased with the heading, except that a note holding a brace group
(`\emph{p.}`) is protected, as it is for `\cite`. No bibliography is written
there: the list of the cited works is LaTeX's own (`thebibliography`, BibTeX,
`biblatex`), built from the author's database.

## Text macros and semantic environments

The Python implementation's `markup` feature has no counterpart feature here.
Its two halves map as follows:

- **A text macro**, a command that wraps its argument in text formats, such as
  `text_macros: {hl: {text_formats: [textbf]}}`, is a [user
  definition](#user-definitions-and-the-template-language): a macro `hl` with
  one mandatory argument and the template `\textbf{#1}`, supplied through
  [`UserdefsConfig`](crate::stdcore::features::userdefs::UserdefsConfig)
  (`Definition::macro_("hl", vec![ArgDef::mandatory("text")]).text(r"\textbf{#1}")`).
  `\hl{word}` then reads as `\textbf{word}` on every output, and the recomposed
  LaTeX may keep it as a LaTeX definition.
- **A semantic environment**, an environment rendered as a block with a role
  of its own, has no counterpart yet; see [What has no counterpart here
  yet](#what-has-no-counterpart-here-yet).

## The package `phfpack`

The Python package `flm-phfpack` (its feature `flm_phfpack.phfqit`) is the crate
`flm-phfpack` here, with its package `phfpack` and its feature `phfqit`. The
differences:

- **How it is turned on.** A document lists the package, `use: [phfpack]`, and
  writes the feature's settings under `phfqit`, instead of giving the Python
  module among the features; the `flm` program includes the package, so there
  is nothing to install.
- **How a macro is declared.** In `macro_definitions`, a declaration is written
  with the keys of a user definition: `args` (the Python `arguments_spec_list`,
  its entries `code` and `name` for `parser` and `argname`, or a structured
  syntax under `syntax`),
  `argument_number_offset` for the marker `SetArgumentNumberOffset`, and
  `content` with `text` and `math` for `textmode` and `mathmode`;
  `delimited_args` is the Python `delimited_arguments_spec_list`. The names
  `SizeArgBacktick`, `SizeArgStar` and `SizeArgOptArg` are unchanged.
- **A declared macro with the name of a built-in macro replaces it**: a
  document's settings take precedence over the package's defaults, as every
  setting does. The Python package keeps its built-in macro there and ignores
  the declaration.
- **`\oproj{A}`** gives what `\oketbra{A}{A}` gives; the Python template had a
  misspelled placeholder, and every use failed.
- **A size** is one of `\big`, `\Big`, `\bigg`, `\Bigg` or a star, given once;
  any other command is reported, where the Python package wrote it as `\hugel`
  and the like. A star after a backtick (``\abs`*{x}``) draws the delimiters as
  tall as the content, as `\abs*{x}` does; the Python package wrote `*l`. A star
  together with `[*]` is two sizes, which is reported, where the Python package
  accepted it.
- **A control word before a letter.** A template that writes a control word
  right before what follows it (the backtick form's `\langle` before the `x` of
  `` `<x> ``) is written with a space between the two, `\langle x`; the Python
  package wrote them joined, `\langlex`, which is another command. The same
  holds for every user definition's template (`\alpha#1` used with `x` gives
  `\alpha x`).
- **The named values** (`spaceKets.Bar` and the five others) are settings that a
  document can set, under the keys `spaceKets`, `spaceOKets` and `space`; in the
  Python package they were constants.
- **The quantity macros are new here**, with no counterpart in the Python
  package: `\HH`, `\Hmin`, `\Hzero`, `\Hmaxf`, `\Hfn`, `\II`, `\DD`, `\Dmax`,
  `\Dminz`, `\Dminf`, `\Dr`, `\DHyp`, `\Dhyp`, `\IdentProc`, `\ee` and
  `\emptysystem`, the entropies and other measures of information of the LaTeX
  package `phfqit`, written as that package renders them. The crate's
  documentation lists their arguments.

## Recomposing to LaTeX

The Python implementation's pure-LaTeX recomposer (`FLMPureLatexRecomposer`,
run by the `flmlatex` workflow) is the
[`latex-recomposed`](crate::stdcore::backends::latex_recomposed) backend here,
run by the workflow of that name. It writes the document back as LaTeX source,
close to what an author would have typed, and leaves numbering and
cross-references to LaTeX. It writes each label as its safe label, the key that
the recomposed LaTeX writes for an FLM label (`ref1`, `ref2`, … in order of
first mention, one sequence for every label type;
[`SafeLabels`](crate::stdcore::features::refs::SafeLabels)), and each reference
through the LaTeX hook `\flmRef`, which the style file defines over `\zcref`
(difference 13 below). A LaTeX hook is a macro, or an environment such as `flmFloat`,
that the recomposed LaTeX writes and the style file defines with a plain default, which
a user redefines in the document's preamble ([hooks](crate::stdcore::backends::latex::preamble)).
The [rendering chapter](crate::guide::rendering#the-recomposed-latex-backend) describes
that output; this section says where the recomposer's options went and where
the recomposed LaTeX differs.

### Where the recomposer's options went

| Python option | This library |
|---|---|
| `recomposer.safe_label_ref_types[domain][type]` | [`RefsRecomposeOptions::safe_label_types`](crate::stdcore::features::refs::RefsRecomposeOptions): per label type, `Safe` or `Raw`; the only domain is `ref` |
| `refs.protect_surround` | [`protect`](crate::stdcore::features::refs::RefsRecomposeOptions): the wrapper is `\NoCaseChange{\protect …}` as there, and the style file requests the `textcase` package; **off by default** here (the Python implementation wraps every reference by default) |
| `refs.emit_flm_macro` | none: its behavior is the default. Every reference is written through a LaTeX hook that the style file defines (`\flmRef` over `\zcref`, and `\flmHyperref` for a `\hyperref`), so a user who wants one macro to redefine already has it, defined rather than left to the user |
| — | none for `ref_command` and `label_command`, which do not exist here: the reference command is written in one place, the definition of `\flmRef` in the style file, and a user redefines that macro instead of rendering the document again |
| — | [`heading_hook`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions), [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions): the hooks, both on by default; with both off, headings, labels and references are written as the Python implementation writes them |
| — | [`drop_math_labels_without_refs`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions): new here, off by default. When it is on, and only in a registry built without the references feature, the labels of an equation line are written nowhere at all; with that feature registered it does nothing. The Python implementation has no counterpart, and no registry without its references feature either |
| `math.surround_display_math` | [`MathRecomposeOptions::surround_display_math`](crate::stdcore::features::math::MathRecomposeOptions) |
| `math.emit_flm_math_environment_macro` | [`MathRecomposeOptions::emit_flm_math_environment_macro`](crate::stdcore::features::math::MathRecomposeOptions); the style file defines `\flmMathEnvironment` |
| `verbatim.use_fvextra` | always on: every inline verbatim command is written through `\EscVerb`, and the style file defines what it needs (difference 5). `fvextra` 1.5 (2022) or later is required, and the style file does not test for it |
| `headings.macroname_mapping` | [`HeadingsLatexOptions::latex_commands`](crate::stdcore::features::headings::latex::HeadingsLatexOptions): the LaTeX command each heading kind is written as, keyed by the kind's name; a kind not listed is written as the command of its own name. A heading is written by its kind, not by the command it was written with, and with a star when it is not numbered |
| — | [`resolved_proof_refs`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions): new here, off by default. Off, a proof's back-reference is written as the author wrote it, `[*ref1]`, as the Python implementation writes it, for the `phfthm` package; on, it is resolved into the proof title hook, `[{\flmProofTitle{\flmRef{ref1}}}]`, for `amsthm` alone (difference 22) |
| `cite.safe_labels` | none: a citation's keys are written as typed, never as safe labels, because they refer to the entries of the author's bibliography database, not to labels of the document (see [Citations](#citations)) |
| `skip_packages` (the workflow's) | [`LatexRecomposedRenderOptions::skip_packages`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions) |
| — | [`unicode`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions), [`unicode_overrides`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions), [`drop_comments`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions), [`style_stem`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions), [`style_file`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions), [`extra_packages`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions), [`extra_preamble`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions), [`bare_names`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions), [`robust_names`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions): new here (below) |

### Differences in the recomposed LaTeX

1. **The output is the recomposed LaTeX and a style file.** The Python
   implementation returns one string per fragment, plus a fixed block of
   definitions that a template places in the preamble, and requests three
   packages. Here the output is two artifacts: the recomposed LaTeX, whose first
   line gives the name of the style file, and the style file
   (`paper-flmrecomposed.1a2b3c4d.sty` under `-o paper.tex`), which holds exactly what
   the recomposed LaTeX needs:
   - every package a lowering rule requires (`amsmath` for the `amsmath`
     environments, `zref-clever`, `fvextra`, `enumitem` for a list's own label,
     the packages that the spellings of characters need);
   - the declarations of Unicode characters;
   - the definition pieces that the rules used (a definition piece is a unit of
     LaTeX definitions registered under an identifier;
     [`LatexPreambleDef`](crate::stdcore::backends::latex::LatexPreambleDef)),
     each one either guarded or a LaTeX hook;
   - the user definitions that the recomposed LaTeX calls, each guarded in the
     same way, except where the render option `bare_names` asked for the
     document's own name;
   - the packages and the three parts of the user's own LaTeX that the render
     options `extra_packages` and `extra_preamble` hold, which have no
     counterpart in the Python implementation either: one part at the top of
     the file, before the packages; one directly below the package block, above
     everything this library defines; one at the end.

   Each fragment's recomposed LaTeX ends with the comment line `%%`, as there.
   The package block holds **one `\RequirePackage` line per package**, its
   options the union of every request's options, each once, in the order first
   requested. The Python implementation needs no such merge, since it requests a
   fixed three packages with no options. One option key given two different
   values is the error `flm.latex-recomposed.package-options-conflict`, and that
   package is then not requested.
2. **Non-ASCII text is declared or replaced.** The Python implementation writes
   every non-ASCII character as UTF-8 and declares nothing. Here the default
   `Declare` mode writes the character and declares it once in the style file
   with `\newunicodechar`; the `Replace` mode writes its LaTeX spelling in its
   place; either mode takes overrides per character.
3. **The escaped characters.** The Python implementation escapes `$ & # ^ _ %`
   in text, writing `\^` for a caret. Here `^` is written `\textasciicircum{}`
   (`\^` is the accent command and would put a circumflex over the next
   character), and `{`, `}`, `\` and `~` are escaped too when they occur in a
   text node (`\{`, `\}`, `\textbackslash{}`, `\textasciitilde{}`), since a text
   node can hold them here. An author's `\{` is written back as `\{` by the
   `literals` feature's own rule either way.
4. **The syntax of a command is normalized.** The Python implementation writes a
   command it does not rebuild as written (`\textbf x`, `\section  {T}`). Here
   every construct is rebuilt from its parts (a mandatory argument always
   braced, no space before an argument), while the content of every argument is
   the author's. A literal's command keeps the whitespace that followed its
   name (`\textbackslash x` stays apart), as there. On an equation line,
   `\nonumber` is followed by a space only where a letter follows it
   (`\nonumber\\`); the Python implementation always writes `\nonumber `.
5. **Verbatim content is always written through `\EscVerb`.** By default the
   Python implementation writes the FLM verbatim commands back for a shipped
   block of definitions to interpret, and passes the environments through,
   losing the first line of a `{verbatimcode}` block (its parser consumes the
   newline after `\begin`). Here inline content is
   `\EscVerb[formatcom=\flmFmtVRB{<kind>}]{…}` in every case, and a block is the
   `flmVerbatimCode` environment with the content on lines of its own. The
   encoding escapes every printable ASCII character that is not a letter or a
   digit, and nothing else; the Python implementation's `use_fvextra` path
   escapes every character that is not alphanumeric, non-ASCII characters
   included (`caf\é`). **`@` is written unescaped** here: `fvextra` refuses `\@`
   and stops the compilation with "Invalid backslash escape", so the Python
   implementation's output does not compile where verbatim content holds an `@`
   (an email address, a decorator). The optional argument of a `{verbatimcode}`
   block is always written, `[<lang>]` or `[]`, because LaTeX's search for an
   absent optional argument would consume the line end after which the verbatim
   content must begin.
6. **A macro or environment that no feature declares is recomposed as
   written**, beside the parse's error, where the Python implementation rejects
   the document when it parses it.
7. **A plan that recomposes checks nothing about labels.** A label whose type
   its element refuses, and a label not written `type:label`, are both given a
   safe label and recomposed like any other label. The latter is keyed by its
   text and belongs to no label type, as the Python implementation keys it under
   the type `None`. The `check` workflow is where either is reported.
8. **The optional argument of a rebuilt command** is written `[{…}]` as soon as
   its content holds a `]` outside a brace group, or a command whose bytes are
   not inspected, and `[…]` otherwise. The Python implementation writes a single
   text `[…]` regardless of what it holds (its test for a safe text accepts
   every text), so a `]` in it ends the argument early there, while it is
   protected here. It also builds a `\ref`'s options as one string,
   `\zcref[cap={\emph{z}}]{ref1}`, where this library, whose list of options
   holds the lowered command, writes `\zcref[{cap={\emph{z}}}]{ref1}`. Options
   holding text alone are written alike, `\zcref[S,cap={Fig\_1}]{ref1}` (LaTeX
   reads both spellings the same way).
9. **A `\ref` is not protected against `\MakeUppercase` by default** (the option
   `protect` above); the Python implementation wraps every reference.
10. **Comments can be dropped** (`drop_comments`). By default they are written
    as written, prefix included, as there. The option applies to a comment
    inside a display-math environment's lines too.
11. **A comment that ends an equation line keeps its newline before the
    label.** Both implementations take the line breaks that end a line's
    content off it and write them back after the line's `\label`, `\tag*`,
    `\nonumber` and `\\`, so that the label ends the line as written. When the
    last node of that content that writes anything other than whitespace is a
    comment, the Python implementation still does so, and writes the label and
    the line break into the comment, where LaTeX never reads them. Here the
    newline stays where it is, and the commands follow it, on the next line.
12. **A `per-plan` user definition is kept: its use sites are written as calls,
    and the definition itself is written into the style file under a new
    name.** This is the largest difference of this chapter. The Python
    recomposer expands every definition and writes the expansion, so its output
    holds no definition and no call: `\mymacro[Hi]{Albert}` becomes
    `\textbf{Hi}, \emph{Albert}`. Here the output keeps the call and the style
    file defines the command, once per document, so the LaTeX reads like a
    document that uses the author's definitions. An `always` definition is
    expanded, as there. The rendering chapter describes each shape ([user
    definitions in the recomposed
    LaTeX](crate::guide::rendering#user-definitions-in-the-recomposed-latex)).

    **The name is not the document's.** Every kept definition is written under a
    new name: a macro whose name is made of ASCII letters as `\flmUD<name>`, an
    environment as `flmUE<name>`, and a macro whose name holds anything else
    under the letters-only encoding `\flmUX…`, with the author's own name given
    in the comment line above it. The render option `bare_names` lists the
    names written as the document wrote them. The Python implementation has no
    counterpart: it writes the expansion, so the question does not arise there.

    **The call is written in one fixed form**: the name, then one brace group
    per argument, in the order the arguments are declared, and nothing else (no
    brackets, no star, no argument left out). So `\mymacro[Hi]{Albert}` is
    written `\flmUDmymacro{Hi}{Albert}`, and `\mymacro{Albert}`, which left the
    optional argument out, is written `\flmUDmymacro{Hello}{Albert}`, with the
    declared default value in the argument's own group. Where the definition's
    template asks whether an argument was *written*, one more group follows the
    values, `{\BooleanTrue}` or `{\BooleanFalse}`. The reason for this form is
    that FLM has already read the document, so LaTeX never has to read a call a
    second time and can never reach a different answer about a bracket or a
    star that follows it.

    **Nearly everything a definition can say reaches the style file**, none of
    it with a counterpart in the Python implementation, which writes no
    definition at all:

    - the definition itself is a plain long expandable macro,
      `\providecommand\flmUDmymacro[2]{\textbf{#1}, \emph{#2}}`, defined unless
      the name is already taken, or a robust command for a name listed in the
      render option `robust_names`;
    - every argument form that can be declared is one brace group: a key-value
      argument as its content, keys included, and a verbatim argument as its
      characters, written as text, `\flmUDcode{a\_b}`;
    - a declared default value is computed at the use site that left the
      argument out (including a default that refers to another argument or tests
      one) and written as that argument's group, so the definition has no
      default at all;
    - all fourteen conditionals of the template language are kept, each as the
      branch its test selects, as LaTeX's own `\IfBooleanTF` over the flag the
      call carries, or as `\flmIfBlankTF` for `\ifblank` and `\notblank`;
    - the number of arguments has no limit: past nine *effective* arguments
      (values and flags counted together), the call passes them in one group,
      `\flmUDbig{{a}{b}{c}…}`, and the body reads argument *n* with `\flmArg`;
    - a name declared once per parsing mode becomes two definitions under two
      names, the per-mode definitions `\flmUTDvec` and `\flmUMDvec` (one name
      declared once for text mode and once for math mode), and every call uses
      the one that applies where it was written, which FLM decides while it
      reads the document; such a name cannot be listed in `bare_names`;
    - an environment's template is cut in two at its body placeholder or, where
      it uses the body somewhere else, twice or not at all, and given the body as
      LaTeX's argument that reads a whole body,
      `\ProvideDocumentEnvironment{flmUEwrap}{+m +b}…`; a template that never
      uses the body never writes that argument, and LaTeX drops the body as FLM
      does;
    - a tack-on argument (the trailing argument that absorbs the field macros
      written after a call's other arguments, as `\thm{Euclid}\label{thm:euclid}`
      absorbs the `\label`; see
      [fields](crate::guide::concepts_overview#fields-data-a-construct-reads))
      may be declared without being used by the template, and nothing is written
      for it. The Python implementation also drops what such a call absorbs when
      the template uses it nowhere. Reaching it through a placeholder is not
      implemented.

    **What is left is expanded, as in the Python implementation.** Three kinds
    of definition are expanded at each use site instead, as the Python
    recomposer expands every definition, and none of them is a limit of the
    recomposition: a definition of a **special character sequence**,
    whose characters are not a LaTeX command at all; a definition marked
    **always expanded**, which asked for it; and a definition the registry
    **refused**, whose refusal was reported when it happened. A definition that
    uses one of the three is expanded with it, and so is a template holding a
    placeholder that is neither an argument nor the body, which no definition
    written in a document can produce. Nothing is reported for these. A fourth
    case is found as the template is lowered: a definition is written as a
    LaTeX definition where its template *typesets* its arguments (`see #1`,
    `\emph{#1}`), and a template that computes something from an argument's
    characters (puts it into an address, a reference key, a term or code:
    `\href{https://doi.org/#1}{#1}`, `\ref{sec:#1}`, `\verbcode{#1}`) has every
    use site expanded, as the Python implementation does, with nothing
    reported. Every use site of a definition written as a LaTeX definition is
    written as a call.

    **A verbatim block causes no problem anywhere**, where the Python
    implementation could not handle it. A code block that a template holds, or
    one that a use site writes in an argument or in the body an environment
    receives, is written as the single call
    `\flmVerbatimBlock[<lang>]{<env>}{<lines>}`, which the style file defines
    and which rebuilds the block where it is; a code block in the document's own
    text is written out in full, as before.

    **Inside mathematics the same holds.** For example, `Test \(\calN(\rho)\)`,
    with `\calN` declared for mathematics mode, gives `\(\mathcal{N}(\rho)\)` in
    the Python implementation and keeps `\(\flmUDcalN(\rho)\)` here, with
    `\providecommand\flmUDcalN{\mathcal{N}}` in the style file. `\calN` is
    declared once, so it keeps the plain prefix; were it declared a second time
    for text mode, the two would be `\flmUMDcalN` and `\flmUTDcalN`.

    One consequence of keeping the call has no counterpart in the Python output:
    a call inside a heading's title or a caption is carried into the table of
    contents and the auxiliary file, where the Python output (the expansion) had
    put plain text. Because the definition is expandable, the same words end up
    there either way, in the PDF bookmark as well; the only difference a reader
    sees is that `\MakeUppercase` over a call uppercases the definition's own
    text together with the argument.

13. **Headings, labels and references go through four LaTeX hooks.** The Python
    implementation writes `\section*{Title}\label{ref1}` and `\zcref{ref1}`.
    Here the recomposed LaTeX writes `\flmHeading[toc=true]`, a newline,
    `\section*{Title}{\flmLabels{\label{ref1}}}`, and `\flmRef{ref1}`; the style
    file defines the three macros. The reason is that two things go wrong in the
    Python output, and neither shows in the source. A reference to a heading
    that the document class did not number (`\section*`, or a level below
    `secnumdepth`) prints the number of the last construct that was numbered. A
    second `\label` on one line of a display-math environment is lost with an
    `amsmath` error. `\flmHeading` gives an unnumbered heading its title as what
    a reference prints, and `\flmLabels` makes every label on a line resolve to
    that line. The render options `heading_hook` and `label_hooks` turn the
    hooks off; with both off, headings, labels and references are written as the
    Python implementation writes them, and what the hooks did is then left to
    the user. The reference hook also keeps FLM's own reference options in the
    recomposed document (difference 19).

    The fourth hook is `\flmHyperref{ref1}{my text}`, for a reference that shows
    the author's own words. The Python implementation writes
    `\NoCaseChange{\protect\hyperref[{ref1}]{my text}}`, which needs the
    `hyperref` package and does not request it; its `refs.emit_flm_macro` option
    writes `\flmRefsHyperref{ref1}{my text}` instead, a macro it leaves the user
    to define. Here the hook is the default, and the style file defines it: its
    definition links the text through `\hyperref` where **the document** loaded
    that package, and prints the text alone where it did not, so the recomposed
    document compiles either way, and whether it carries PDF links remains its
    author's decision. The protection wrapper is the `protect` option here, off
    by default, as for a `\ref`. With `label_hooks` off, the output is
    `\hyperref[ref1]{my text}` and the style file requests the package; the key
    is written without the braces that the Python implementation puts around
    it, because every optional argument is written by the single rule of
    difference 8.

14. **The style file tests nothing about the installation, and `inputenc` is
    never requested.** The Python implementation requests three packages and
    assumes the rest. Here every package a rule needs is requested, but nothing
    this library defines in the style file tests what is installed (no
    definition piece tests the LaTeX kernel or the existence of a command), so
    one recomposed output behaves the same way wherever it is compiled. The only
    tests of the engine in the file are in the package block, for a package
    whose line differs by engine, and in the line that sets the default font
    encoding (difference 15); they change which fonts set the text, not what the
    document says. The only other test the file makes is about the user's
    **own preamble**, which is part of the document and therefore gives the
    same answer on every installation: whether the user loaded `hyperref`,
    tested where a reference shows the author's own words (difference 13) and
    where the file says what such a reference and verbatim content contribute
    to a PDF bookmark. LaTeX has read UTF-8 without `inputenc` since its 2018
    release, under `pdflatex`, `xelatex` and `lualatex`.

15. **A spelling whose commands belong to another font encoding carries the
    switch to that encoding.** The Python implementation writes `\cyrya` bare; a
    document holding one Cyrillic letter then stops with "Command \cyrya
    unavailable in encoding T1", because `T1` is the document's encoding and the
    Cyrillic commands are declared in a Cyrillic font encoding: `T2A` for the
    letters of the modern languages, `X2`, `T2B`, `T2C`, `OT2` or `T2D` for the
    others (difference 17). Here `я` is written
    `{\fontencoding{T2A}\selectfont\cyrya}`, and the style file requests
    `fontenc` with every encoding its characters need, on one `\RequirePackage`
    line per engine (beside `T1` under pdfLaTeX, beside `TU` under LuaLaTeX and
    XeLaTeX, inside a test of the engine), and then writes
    `\renewcommand{\encodingdefault}{T1}` or `\renewcommand{\encodingdefault}{TU}`
    for that engine. The engine's own encoding therefore stays the document's,
    so that Latin text keeps its usual fonts regardless of the order in which the
    encodings were listed.

16. **The style file is written only where it is needed.** A document that
    requires no package, declares no character, needs no definition piece and
    recomposes no user definition gets no style file at all, and its recomposed
    LaTeX holds no comment line giving its name; the render option `style_file`
    set to `Always` writes the file regardless of what it holds. Either of the
    render options `extra_packages` and `extra_preamble` makes the file exist
    regardless of `style_file` (any one of the three parts of `extra_preamble`
    is enough), since what they ask for is written there and nowhere else. The
    Python implementation has no style file to leave out.

17. **Many characters are spelled differently, and the style file holds what the
    spellings need.** Both implementations spell a non-ASCII character from the
    same conversion table, pylatexenc's. Here that table is the one of the crate
    [`untechxt`](crate::untechxt), every entry of which compiles and sets its
    glyph; this required departing from pylatexenc's spelling for about 360
    entries, 242 of them the Cyrillic letters
    ([its table](crate::untechxt::builtin::default_table) lists every one).
    There are three kinds of change:
    - A spelling whose command no installed package defines is replaced by one
      that works: the vulgar fractions `⅓`…`⅞` are `\nicefrac{…}{…}`, not the
      undefined `\textfrac{…}{…}`; 32 mathematical symbols named after their
      Unicode names, `\allequal` and the like, are taken from the STIX fonts as
      `\UnxT…` commands that the style file declares.
    - A spelling whose command is right but whose font has no glyph for the
      character moves to a font that has it: the double-struck lowercase letters
      are `\mathbbm{…}` of `bbm`, the double-struck digits `\UnxTBbold{…}` from
      the `bbold` family, `𝟙` alone `\mathds{1}` of `dsfont`; the lowercase
      script letters are `\UnxTScr{…}` from the `dutchcal` family, the capitals
      staying `\mathscr{…}`.
    - A character that no installed font has at all loses its entry and is
      reported as a character with no known spelling (the combining dot above
      and diaeresis, the combining Cyrillic hundred-thousands and millions
      signs).

    The Cyrillic letters carry the encoding switch of difference 15, each to the
    encoding that declares its own command: `T2A`, `X2`, `T2B`, `T2C`, `OT2` or
    `T2D`. Each entry of the table also says what its spelling needs, so the
    style file holds exactly that: `dsfont` for `𝟙`, `amssymb` for `ℂ`,
    `fontenc` with the right encoding for a Cyrillic letter, and, where what is
    needed is declarations that no package makes, the declarations themselves.
    The Python implementation requests none of this and leaves the preamble to
    the user. An override per character is the user's own: its spelling is
    written as given, and what it needs belongs in the user's template.

18. **An element that the plan request accepts as unrenderable is a comment
    line.** A caller may accept, when the plan is built, that the backend has no
    rule for some element
    ([`PlanRequest::unsupported`](crate::core::plan::PlanRequest::unsupported));
    such an element is an accepted unrenderable element. Each occurrence is
    then recomposed as the line `% flm: unsupported element <name>`, beside one
    error, so that the gap is shown in the recomposed LaTeX rather than hidden.
    The Python implementation has no such notion: every element it knows has a
    recomposition, and one it does not know is a parse error.

19. **FLM's reference options are translated for `zref-clever`, in the hook.**
    A *reference option* is what a document writes in the brackets of
    `\ref[…]{…}`, and the vocabulary is FLM's own: `noprefix`, which turns off
    the word that a reference writes before its target's number or title, and a
    bare key that asks for a wording variant. The Python implementation passes
    them to `\zcref` unchanged, and `zref-clever` has no key `noprefix`, so a
    document that writes `\ref[noprefix]{…}` does not compile there. On every
    other output the options are read where a number is written (`noprefix`
    drops the word before it, `S` asks for the capitalized wording) and change
    nothing for a target that shows its content or its label. Here the
    recomposed LaTeX writes the options as the document wrote them, and the
    default definition of `\flmRef` (difference 13) translates them when it
    calls `\zcref`: `noprefix` becomes that package's `noname`; the capitalized
    variant `S` has the same name in both vocabularies and is passed unchanged;
    the standard variant names `capital`, `lowercase` and `abbreviated` become
    the package's keys for the capitalized, lowercase and abbreviated names; and
    every other option is dropped, the reference printing its default wording
    rather than stopping the compilation. A `zref-clever` key that a Python
    document writes in a `\ref` (`\ref[cap=true]{…}`, `range`) is therefore
    dropped too; FLM's other outputs already report it as an undeclared wording
    variant ([Labels and references](#labels-and-references)), and with
    `label_hooks` off it still reaches `\zcref`. The translation is in the
    hook's default and nowhere else, so redefining `\flmRef` gives back FLM's
    options as the document wrote them; with the render option `label_hooks`
    off there is no hook, and the options reach `\zcref` untranslated, as they
    do in the Python output. The [rendering
    chapter](crate::guide::rendering#references-and-safe-labels) holds the
    table of the translation.
20. **A list's own label is written for the `enumitem` package.** The Python
    implementation writes a list's bracket back as the document wrote it,
    `\begin{enumerate}[(${alph})]`, which no LaTeX list environment reads. Here
    a list's own label template is written as that package's `label=` key, and
    the package is requested: `[(i)]` and `label=(#{roman})` both become
    `[label={(\roman*)}]`, and a fixed `[-]` becomes `[label={-}]`. A reference
    template that differs from the label template is written as `ref=` beside
    it, `[1.]` becoming `[label={\arabic*.}, ref={\arabic*}]`. The template's
    content (its text, escaped characters and markup) is recomposed as the rest
    of the document is, so the recomposed label carries the author's markup,
    `label={\textbf{#{arabic}}.}` becoming `[label={\textbf{\arabic*}.}]`; a
    number format for which the package has no command (`#{fnsymbol}`) is
    reported, and its key is not written. An item's tag is written
    `\item[Tag]`, and `\item[{…}]` only where it holds a command or a `]`, by
    difference 8, where the Python implementation braces every tag. As there,
    the labels that a list kind declares for each depth are not written, so
    LaTeX's own labels are shown beside the items, and a list environment that a
    host configures is written under its own name for the author's LaTeX to
    define.
21. **A footnote is written through a command of its own.** The Python
    implementation writes `\footnote{…}` back as the document wrote it. Here an
    xnote of the kind `footnote` is written `\flmXNfootnote{…}`, and
    `\flmXNfootnote[{\flmLabels{\label{ref1}}}]{…}` where it carries labels (the
    brackets braced by difference 8). The style file defines the command with
    `\ProvideDocumentCommand\flmXNfootnote{+O{} +m}{\footnote{#1#2}}`, so the
    labels come first inside LaTeX's `\footnote` and a `\zcref` to one of them
    prints the footnote's number. The letters `XN` stand for the xnotes
    feature: a second xnote kind `endnote` is written `\flmXNendnote{…}`, passed
    by default to the LaTeX command that the kind's `latex_command` gives. An
    author who wants the notes set another way redefines the command before or
    after loading the style file; an xnote kind shown in place on the other
    outputs is written the same way here.
22. **Theorems are declared in the style file, and a proof's back-reference
    may be resolved into the proof title hook.** The Python implementation
    writes a theorem's environment as written (`\begin{theorem}[{My Theorem}]`,
    then `\noproofref` and the labels, and a proof's back-reference as the
    author wrote it, `[*ref1]` or `[**ref1]`, the syntax of the `phfthm`
    package) and declares no environment, leaving them to the author's
    preamble. Here the environment, the title, the back-reference and
    `\noproofref` are written as there: the title in `[…]`, and in `[{…}]` once
    it holds a command, as every optional argument here; the labels in
    `\flmLabels`, with `\noproofref` after them. The exception is a proof's
    title, which is written inside the proof title hook,
    `[{\flmProofTitle{Sketch}}]`. The style file gives that hook a default for
    each form: in the default form, the title alone, which `phfthm` sets as
    "Proof of Sketch", as for the Python implementation's `[Sketch]`; in the
    resolved form below, "Proof (Sketch)", as FLM's other outputs set it. The
    style file requests `phfthm`, declares each theorem kind the document uses,
    guarded on its name, and defines `\noproofref` as nothing and
    `\flmProofTitle` where no package defined them. The render option
    `resolved_proof_refs`, new here, writes a back-reference in the proof title
    hook instead, `[{\flmProofTitle{\flmRef{ref1}}}]` (a back-reference that is
    not shown writes no optional argument), and declares the theorem kinds with
    `amsthm` alone. The reason for the default: `phfthm` reads exactly the
    syntax that an FLM author writes, so the recomposed LaTeX keeps the relation
    between a proof and the statement it proves as LaTeX source that a person
    can read and edit, and the package can act on it (a statement's heading can
    point to its proof). A host that compiles where that package is not
    installed sets `resolved_proof_refs`, which `amsthm` alone compiles.

## The LaTeX output for LuaLaTeX

The Python implementation's LaTeX renderer (the format `-f latex`) is the
[`latex-rendered`](crate::stdcore::backends::latex_rendered) backend here, run
by the workflow of that name. FLM resolves the numbering, the references, the
table of contents and the notes itself and writes the result as LaTeX. The
output is a new design, and no compatibility with the Python output is offered:

- **The macros differ.** Each element of the document is written as one
  `\flmx…` command or environment of a fixed contract (`\flmxHeading`,
  `\flmxSpan`, `\begin{flmxBlock}`, `\flmxLink`, `\flmxXnote`, …), whose
  key-value argument carries the element's role, kind tokens, anchor and other
  facts. The Python renderer's macros (`\flmThm…`, `\flmDefterm`, and the
  others) are not written, and a preamble written for them does not apply. A
  look is changed by registering LaTeX code under a role or a kind token
  ([the rendered LaTeX contract](crate::guide::rendering#the-rendered-latex-contract)).
- **The preamble differs.** The definitions are in a style file written beside
  the document (`paper-flmx.HASH.sty`), which loads `fontspec`, `amsmath`,
  `xcolor` and `hyperref`, and only what the document's content needs beside
  them.
- **One engine.** The output is for LuaLaTeX alone; the style file stops any
  other engine. Compile with `lualatex` twice, or with `latexmk -lualatex`.
- **The table of contents is FLM's**, built and numbered by FLM, with page
  numbers through `\pageref`; LaTeX's `\tableofcontents` is not used.
- **Running LaTeX** to a PDF file, the Python workflow `runlatexpdf`, is the
  profile `pdf-latex` here, the extension `profile-pdf-latex`, which the program
  carries compiled in: the command is `flm render -p pdf-latex -o paper.pdf paper.flm`,
  and the front matter needs nothing ([PDF through
  LaTeX](crate::guide::command_line#pdf-through-latex)). Both run `latexmk -f
  -lualatex` in a temporary folder holding the LaTeX file and the images; with `-f`,
  `latexmk` goes on running LaTeX after a run that reported an error, so both write the
  PDF that LaTeX produced, its cross-references resolved, even when LaTeX reported an
  error. They differ in four ways:
  - `latexmk` runs with `-norc`, so its configuration files (`~/.latexmkrc`) are not
    read: such a file can change the engine or the folder where the PDF is written,
    and the program would then not find the PDF.
  - A failure is an error (exit status 1) whose message quotes the first error of
    LaTeX's log, and nothing but the PDF is written. The Python program printed
    `latexmk`'s output, warned, and copied the LaTeX file and the log into the
    current folder (`_flm_runlatexpdf_compile_log.latex.tex` and `.log`). The
    reason: a command writes no file beside its output, and the LaTeX files are made
    again, at their place, with `-p latex-rendered`. A run that produces no PDF is an
    error too, where the Python program stopped with an exception.
  - The LaTeX file is named after the output (`paper.tex` for `-o paper.pdf`), not
    `main.tex`, so that the file and line numbers of the log are those of the file
    that `-p latex-rendered -o paper.tex` writes.
  - The PDF is made from the LaTeX for LuaLaTeX of this section, with its style
    file; the Python workflow compiled the output of its format `latex`, whose
    template held the preamble. The Python workflow's format `latex`, which wrote
    the LaTeX instead of the PDF, is the workflow `latex-rendered` here.

## The plain-text output

Both implementations write plain text. Three properties of the whole output
differ, so an output expected from the Python implementation can be compared
with this library's only after accounting for them:

- **Line wrapping.** The Python implementation never wraps lines. Here the
  text backend wraps at a width in columns given by
  [`TextRenderOptions`](crate::stdcore::backends::text::TextRenderOptions),
  80 by default; `width: None` never wraps, and is the setting that makes the
  two outputs comparable line for line.
- **The final newline.** The Python output ends where the content ends. Here
  the output ends with exactly one newline and never with trailing whitespace.
- **An accepted unrenderable element.** A caller may accept, when the plan is
  built, that a backend has no lowering rule for some element
  ([`PlanRequest::unsupported`](crate::core::plan::PlanRequest::unsupported)).
  The Python implementation prints nothing for an element it cannot render;
  here the text backend prints `[unsupported: <element name>]` for each
  occurrence of such an element, so that the gap is shown in the output rather
  than hidden.

The two agree on the non-breaking space: `~` reaches the plain-text output as
the character U+00A0 itself, at which no line is broken, and which is not
replaced by an ordinary space. There is no option to write an ordinary space
there instead.

## The HTML output

Both implementations write HTML, and the HTML this library writes is **not**
the HTML the Python implementation writes. The class names, the identifiers and
the structure of the page of the Python renderer are replaced here by one
design derived from the document's own structure, the [HTML
contract](crate::stdcore::backends::html::CONVENTIONS) (the classes and attributes
that a stylesheet may rely on). A stylesheet written for the Python
implementation therefore does not fit this output. What the two keep in common
is the *content*: which text appears, and in which order.

The differences a Python user would notice:

| construct | Python renderer | This library |
|---|---|---|
| text content | escapes `&`, `<`, `>`, `"` and `'`; writes seven invisible space characters as named references (`&nbsp;` and the others) | escapes `&`, `<` and `>`, and writes the same seven spaces, and the narrow no-break space (U+202F), as numeric references (`&#xA0;` and the others), which an XML parser reads too. A quotation mark needs no escaping between tags |
| an attribute value | escaped partially: `&` only where something like an entity follows, `<` and `>` never | `&`, `<`, `>` and `"`, always, so the output is valid regardless of what the value holds |
| `\textbf`, `\textit`, `\emph` | `<span class="textbf">`; `\emph` is treated as `\textit` | `<strong class="flm-pres-span" data-flm-style="bold">`, `<i … "italic">`, `<em … "emph">`: the tag follows the text style, the class and the attribute belong to the HTML contract, and `\emph` is kept apart from `\textit` |
| a fragment holding one run of inline content | no `<p>` at all: the renderer decides from what it was given | no `<p>` either, because a fragment carries a declared layout, the block-structure pass builds paragraphs accordingly, and the emitter writes what the tree holds. A host that wants a paragraph parses the fragment with the blocks layout |
| headings | `<h1 class="heading-level-1">` … `<h3>`; levels 4 to 6 a `<span class="heading-level-4 heading-inline">` followed by a space | `<hN class="flm-pres-block--heading" data-flm-heading="section" data-flm-level="N" data-flm-toc="true">` under the default root heading level, 1 (the tag counts from the render option `root_heading_level`), `<h6>` for every depth beyond the sixth, with the true level kept in the attribute, and a render option ([`heading_tags`](crate::stdcore::backends::html::HtmlRenderOptions::heading_tags)) that maps a heading kind to a tag of its own. Which heading kinds are run in is set in [`HeadingsConfig`](crate::stdcore::features::headings::HeadingsConfig), and a run-in heading is a decoration span around a `<span class="flm-pres-runin--heading">` carrying the same attributes as a block heading, with the stop written inside the decoration span as text (the render option `run_in_stop`) and the space after it given by the stylesheet |
| heading and label identifiers | `sec--<slug cut at 32 characters>` with `-2` counters, and a second, different sanitizer for other targets | the anchor computed for the reference point ([`ANCHORS`](crate::stdcore::features::refs::ANCHORS)), written as the `id`: one encoder, one identifier per label, and no counter; the same prefix `sec--` before the title of a heading that has no label, with the same `-2`, `-3` numbering when a title repeats, the title read without the footnotes in it. A label whose own text begins with a hyphen, or whose type ends with one, is encoded in the encoder's second way (accents removed, the text turned into a slug, a short hash of the original appended), so that it cannot take the form of an identifier derived from a title or from a position: `\label{sec:-Foo}` is not `sec--Foo` |
| `\ref` and `\hyperref` | `<a href="#…" class="href-ref ref-sec">` | `<a class="flm-pres-link" data-flm-kind="ref" href="#…">` |
| an unresolved reference | stops the render | an error-styled span and one Error, and the page is still computed, as on every backend here |
| verbatim content | `<span>` or `<div>` chosen by an annotation; every backslash wrapped in a `<span>` to keep the formula renderer away; classes `verbatimcode verbatim-lang-python` | `<code>` inline, `<pre><code>` as a block, with `data-flm-kind`, `data-flm-display` and `data-flm-lang`; no wrapping of backslashes, because the formula renderer is restricted to the elements that hold mathematics |
| mathematics | `<span class="display-math env-align">` even for a displayed formula; the source escaped; `\label` kept; a number written into the source as `\tag*{(n)}` in characters; the first line's identifier on the wrapper and no anchor on the other lines | `<span class="flm-pres-math--passthrough">` for every formula, inline and displayed alike, with `data-flm-display` telling them apart and `data-flm-env` giving the environment, the displayed form made a block by the stylesheet as in the Python renderer; the formula passed whole as the LaTeX the author wrote, between the delimiters this backend chose; a numbered line's number written as `\tag*{…}` around page content |
| text inside a formula (`\text{…}`) | part of the source passed to the formula renderer, typeset by it as it can | FLM content: parsed as text, lowered like content anywhere else, and written as HTML *inside* the formula through the formula renderer's `<tex-html>` mechanism, so a reference or a styled span inside a formula is real markup, wherever the command is in the formula |
| an equation line as a link target | the first line's identifier on the whole formula; the other lines cannot be linked to | every line of a display-math environment is a reference point that a `\ref` can refer to, written as an empty element at the start of that line inside the formula, so a link leads to the line itself |
| `\label` inside a display-math environment | written into the source passed to the formula renderer | not written: it is the line's data, which the references feature turns into the line's anchor. `\nonumber` and `\notag` are not written either: a line that takes no number has none to suppress. The `\\` breaks and every alignment mark are the author's own and are written back unchanged; a `\tag` is written back as page content inside the formula |
| the formula renderer | MathJax 3 from a content delivery network; the configuration never reaches the shipped pages, and `\href` and the related commands accept any address an author writes inside a formula | MathJax 4, as one script definition whose delivery the host decides; its configuration is part of the definition and holds what this backend's output needs, among which typesetting restricted to the elements that hold mathematics. The extension that puts HTML inside a formula is on, because it is how content inside a formula reaches the reader. Mathematics is otherwise not restricted, as in the Python implementation; a host that renders mathematics written by authors it does not trust configures the renderer itself |
| comments | dropped | dropped |
| lists | a `<dl>` with floated labels, the item's identifier on its `<dt>` | `<ol>`, `<ul>` or `<dl>` by the list's structure (numbered, unnumbered, or terms with descriptions), each label in a label span, the list laid out as a grid with one column of labels and the browser's own marker switched off, or in a `<dt>` for a description list; the item's identifier on its `<li>`, or on its `<dt>` |
| footnotes | the mark a link `<a href="#footnote-1" class="href-endnote …">a</a>`; the list, where the template places it, a `<dl class="enumeration footnote-list">` with the letter in a `<dt id="footnote-1">` | the mark a `<sup>` carrying the mark's own identifier around `<a href="#fn-1" role="doc-noteref" aria-label="Footnote 1">1</a>`; the list placed inside `<main>` after the trees, a `<div role="doc-endnotes">` holding the title as a heading and an `<ol>` whose entries carry `id="fn-1"`, a raised label and a link back to the mark (`role="doc-backlink"`); an xnote kind shown in place a `<span>` or `<div>` of role `doc-footnote`, its brackets drawn by the stylesheet; no script |
| theorems | written with markup and classes of their own | a block `<div>` of the HTML contract's block class and the role `flm.theorems.theorem`, its `data-flm-kind` the theorem class and the theorem kind (`theoremlike lemma`), its heading a run-in decoration at the start of the first paragraph, the name and the title in spans of the roles `flm.theorems.name` and `flm.theorems.title`, a proof's closing mark a final decoration at the end of the last paragraph; the feature's stylesheet sets the name in bold, a proof's name in italics |
| floats | written with markup and classes of their own | a `<figure>` of the HTML contract's block class and the role `flm.floats.float`, its `data-flm-kind` the float type and `numbered` or `unnumbered` (`figure numbered`), holding the content in a `<div>` of the role `flm.floats.float.content` and, where the float is numbered or captioned, a `<figcaption>` of the role `flm.floats.float.caption` placed last, whose paragraph holds the number in a span of the role `flm.floats.float.number`; the feature's stylesheet sets the float apart and centers it |
| tables of cells | `<table class="cells">`, each cell a `<td>` or a `<th>` carrying `cell`, a `cellstyle-<token>` class per cell style token and a `celltbledge-<edge>` class per edge of the table it touches | `<table class="flm-pres-cells">`, one `<tr>` per row, each cell a `<td>` or a `<th scope="col">` / `<th scope="row">` carrying `colspan`, `rowspan`, `data-flm-align`, `data-flm-rules` and one `data-flm-style-<name>` per custom cell style; no edge class, the outline drawn on the table by the feature's stylesheet ([Tables of cells](#tables-of-cells)) |
| citations | written with markup and classes of their own | the citation mark a span of the role `flm.citations.mark`, its kind token `attached` or `standalone`, holding the numbers as links of kind `cite` to `#citation-<n>` (see [Citations](#citations)); the list of citation entries a block of role `flm.citations.bibliography` holding a heading and a labeled list, each entry carrying `citation-<n>`; the list carries `role="doc-bibliography"` and each number's link `role="doc-biblioref"`, the entries no role; the feature's stylesheet `flm.citations.style` only where a citation is written |
| the whole output | a bare fragment by default; a page only under a template | a complete page, assembled by the HTML backend's final stage (see [output regions and the final stage](crate::guide::concepts_overview#output-regions-and-the-final-stage)). The HTML of each tree is a table that an embedding program reads ([`OUTPUT_TABLE`](crate::stdcore::backends::html::OUTPUT_TABLE)); the `flm` program assembles the same parts through a compose template (by default `simple`, a complete page; `flm render --help` lists the others) |
| renderer options | eighteen attributes on the renderer | eight ([`HtmlRenderOptions`](crate::stdcore::backends::html::HtmlRenderOptions)). Most of the eighteen configured what the HTML contract now fixes |
| the default stylesheet | one stylesheet, always complete, selecting on bare `p`, `a`, `table`, `h1`–`h3` | several style definitions, one per concern, each included only where an element it styles is written, every selector under a class of the HTML contract, so that FLM output embedded in a page restyles nothing of the host's page |

A number is never written into a formula's source as characters. The Python
renderer writes `\tag*{(n)}` into the source with the digits in it. Here the
numbering feature computes the number and the lowering rule of `math-passthrough`
builds it where it is shown, as FLM content inside the formula (`\tag*{…}`
around a content node), so that a counter style that holds markup produces
markup. A `\tag` that the author wrote is written the same way, and in both
implementations it replaces the number.

## What has no counterpart here yet

This library implements part of what FLM covers. The following have no
implementation yet, so a document that needs them cannot be ported yet:

- **Sub-numbered equations.** `(3a)`, `(3b)`, whether written as a
  `subequations` environment or by numbering the lines of one environment that
  way, have no counterpart here: the numbering feature supports sub-numbers,
  but the mathematics feature asks for none. The Python implementation has no
  `subequations` environment either, and nothing in it creates a sub-number.
- **Alternative sources for an image on the page** (`srcset`), and converters
  that turn a drawing (SVG, PDF) into pixels with a program other than
  ImageMagick (the Python program could use `cairosvg`, Ghostscript or
  `pdftocairo`).
- **Semantic environments** (the Python `markup` feature's
  `semantic_environments`): an environment written as a block with a role of
  its own. A feature of one's own provides one: it declares the environment and
  a lowering rule that stages (builds into the lowered tree) a `flm.pres.block`
  with that role; see [extending FLM](crate::guide::extending).
- **Glossaries of terms**: a list of every defined term of a document, placed as
  a list of footnotes is. Defined terms themselves are described under [Defined
  terms](#defined-terms) above.
- **An HTML output of several files**, the counterpart of the Python
  implementation's collection of pages.

What *is* implemented is the language and the framework around it: the parser
and its element declarations, user definitions, block structure and
paragraphs, mathematics, verbatim content, headings, lists, hyperlinks, defined
terms, theorems, quotations, annotations, images, floats, tables of cells,
citations, labels and references (including references between documents,
through an installed external index), the element index, the `check` workflow,
plain-text rendering, the LaTeX for LuaLaTeX and its compilation into a PDF file,
the recomposed LaTeX, and HTML.

## The command line

The `flm` program is built on the host library `flm-main` and the command-line
crate `flm-cli`. `flm render FILE` renders the file, to HTML unless `-p` gives
another profile; `-o` writes a file, and without it the output is printed on
standard output followed by a newline, unless `-n` is given; `-c TEXT` gives the
content on the command line; `-B` reads the content as paragraphs and blocks.
The front matter of a file is the block of YAML between a first line `---` and
the next `---` line: its key `flm` holds the document's settings, and its other
keys are the author's metadata (title, authors, date); positions in messages
count lines from the top of the file. See [the command-line
chapter](crate::guide::command_line). The differences:

- **The program checks the commands of a formula by default.** The `flm` program's
  default math processor is `mathjax`: it uses the package `math-mathjax` after `std`
  and before the packages a document lists under `use`, so a command inside a formula
  that MathJax's TeX packages do not define is an error. The Python program accepts every
  command inside a formula. In both, the HTML page leaves the formulas to MathJax in the
  reader's browser; the setting `mode: mathml` of `math-mathjax` writes each formula into
  the page as MathML instead, typeset with no script.
- **A document chooses its math processor with `math`.** The key `math` under `flm` in
  the front matter, a math processor's name (`mathjax`, `passthrough`) or `false`, has no
  counterpart in the Python program. A Python document's reading of formulas is
  `math: passthrough` in its front matter, or `--math passthrough` on the command line
  ([the packages a document
  uses](crate::guide::command_line#the-packages-a-document-uses)). The program's default
  processor is not guaranteed to stay the same, so a document that needs a stable result
  writes its processor.
- **`flm FILE` is the live preview**, which the Python program's `flm -W FILE`
  was; what the Python program's `flm FILE` did is `flm render FILE`.
  `flm paper.flm > out.html` becomes `flm render paper.flm > out.html`,
  `flm -o out.html paper.flm` becomes `flm render -o out.html paper.flm`, and
  `flm -V -o out.html paper.flm` becomes `flm render -V -o out.html paper.flm`
  (the preview refuses `-o`, `-V`, `-c` and `-`, and its message gives the
  `flm render` command). There is no `-W`. `-V`, which opens the written output
  with the system's default application, belongs to `flm render` and needs
  `-o`, as in the Python program.
- **The preview opens the browser.** The preview (`flm paper.flm`, or
  `flm watch paper.flm`) opens the default browser on its address unless
  `--no-browser` is given, where the Python program opens nothing; the
  environment variable `BROWSER`, when it is set, gives the browser to open
  instead. It serves at port 18910 or the next free port, as the Python program
  does, or at the port that `--port` gives.
- **The preview watches every file the document read**: the files brought in,
  the manifests and files of the extensions, the images, the bibliography files. The Python
  program watches the file given on the command line alone.
- **The preview starts and continues although the document has errors**,
  showing the document as it is now. The Python program stops at a first build
  that fails, and keeps showing the last version that built. Every problem of
  the build is listed, and the elements it occurs in are marked on the page,
  where the Python program shows the first problem.
- **The parts of the page are updated separately.** The title, the list of
  notes, the bibliography and the content are each updated on their own, and
  inside a part that changed, every element that did not change is kept, where
  the Python program replaces the content's container as a whole. A script of a
  page written for the Python preview's global names `window.flmSetup` and
  `window.flmHotReload` listens instead for the event `flm:region-updated`,
  which bubbles from the element of the part that was updated; the preview's
  script defines no global name of its own (see [a page updated in
  place](crate::guide::rendering#a-page-updated-in-place)). An open page
  connects again by itself when the program is started again.
- **The preview has a toolbar.** The Python program's panel of problems is a
  toolbar with *Problems* (each problem with *Open in editor*, which that panel
  does not have), *Rebuild*, *Outputs* and *Settings*, which have no Python
  counterpart. *Settings* shows the document's features with what the front
  matter writes for them, lets the user change those values, and saves them into
  the document's file by editing its front matter in place (see [watching a
  document](crate::guide::command_line#watching-a-document-the-live-preview)).
- **The clicks on the preview's page are the Python program's**: Ctrl or Cmd
  together with Shift opens an element's place in the editor; Ctrl or Cmd copies a
  reference to the nearest element around the click that carries a label. A
  reference to a defined term is copied as `\ref{…}`, where the Python program
  copies `\term{…}`. When the nearest element with an identifier has no label,
  copying goes on to the elements further out, where the Python program copies
  nothing.
- **The editor is chosen in a menu of the preview's toolbar.** The menu lists
  every editor found, and the browser remembers the choice; `--editor NAME`
  selects the editor the menu shows at first (see [from the page to the
  source](crate::guide::command_line#from-the-page-to-the-source)).
- **The editors are `code`, `zed`, `emacsclient`, `subl` and `gvim`**, in this
  order, where the Python program's list has no Zed and ends with `vim`. `vim`
  is not accepted, by the menu or by `--editor`: the preview starts the editor
  without a terminal, where Vim in a terminal cannot show the file.
- **The default output is a complete page.** With no compose template given,
  the Python program prints the bare content; here the compose template
  `simple` is the default. On HTML, `-t none` gives the content and every part
  the backend places beside it (the title, the authors and the date before it,
  the lists of notes and the bibliography after it), each inside a `<div>` that
  identifies it (`id="flm-region-content"`), where the Python program writes the
  content bare. The Python program's bare output held no title: its title,
  authors and date were written by its `simple` template alone. For the content
  without them, leave the metadata's parts out of the render with the `pieces`
  setting:
  `-t none -O 'pieces.exclude=[flm.meta.title, flm.meta.author, flm.meta.date]'`.
- **The Python program's templates are not read**, nor is `-P`. The program's
  compose templates (`flm render --help` lists them) are written in the template
  language of the `minijinja` crate; they place the title, the authors, the date
  and the lists of notes as parts of their own, which the Python templates could
  not. A compose template's settings are given, as in the Python program, under
  the output's name and the template's name:
  `-O 'compose.template_config={latex-rendered: {simple: {documentclass: report}}}'`.
- **No file is read because of the folder the command runs in, and the
  names are this library's.** The Python program reads `flmconfig.yaml` from
  the current folder, with one more file per format and per workflow. Here a
  document gets the settings of the extensions it lists in `use`, and no
  others. An extension is a YAML file `NAME.yaml`, or a folder `NAME/` holding
  a file `flmextension.yaml`, that a document lists under `use`, found on the
  program's search path; an extension written as data holds settings, never
  code. There is no `-C`, no `--inline-config` and no
  `--inline-default-config`: the front matter, the extensions and
  `-O FEATURE.KEY=VALUE` hold every setting, under the names of
  this library's features and keys (`comment_prefix` for `comment_start`,
  `dollar_math` for `dollar_inline_math_mode`, `default_profile` for
  `default_format`), a feature being given by its name, never by a Python module
  path. Every value has one source that can be identified. To keep an
  `flmconfig.yaml`, make it an extension: copy it to a file
  `_flmextensions/mystyle.yaml` beside the documents (or in a folder of
  `FLMPATH`, or of `--extensions-dir`), keep what is under its `flm:` key, rename
  the keys as above, and write one line in each document's front matter:

  ```yaml
  ---
  flm:
    use: [mystyle]
  ---
  ```
- **No marker in a settings file.** `$defaults`, `$merge-config`,
  `$remove-item`, `$import` and `$no-merge` do not exist. An extension lists
  the packages it builds on in its own `use` list, under `flm:`. A `use` entry
  is a name, never a path: the Python program's relative paths in `use`
  (`./shared/base`) have no counterpart, and an extension is put in
  `_flmextensions` or in another folder of the search path instead. A data
  package of the first versions of this program, which wrote its keys at the
  root of its file, moves them under `flm:`, and its folder's file
  `flmpackage.yaml` becomes `flmextension.yaml`; the per-user folder
  `~/.flm/packages` becomes `~/.flm/extensions`. A setting written by a source
  of higher rank replaces the lower one's as a whole, except for the
  collections of named things (below). Settings cannot be imported from a web address or from a
  Python module.
- **A collection of named things is a map, combined entry by entry.** The xnote
  kinds, the float types, the theorem kinds and classes, the environments of
  quotations and of lists, the heading kinds and commands, the annotators, the
  counter styles and the user definitions are maps from a name to an entry,
  where the Python program writes lists. The sources that write one are
  combined one entry at a time, so a document keeps the defaults and adds one
  entry by writing that entry alone, where the Python program's list needed the
  item `$defaults`; an entry written `false` is removed, where the Python
  program needed `$remove-item`. An entry that is written replaces the entry of
  its name as a whole, where `$merge-config` changed a few of its fields: the
  fields it does not write take the values that the feature gives a new entry
  of that name, not those of the entry it replaces, so a document that changes
  the standard footnote's mark writes its words and `commands: [footnote]` too
  (see [the command-line
  chapter](crate::guide::command_line#configuration-files-and-document-front-matter)).
  Any other list is replaced as a whole, as in the Python program.
  `-O FEATURE.KEY.NAME=VALUE` writes one entry from the command line.
- **Two extensions that disagree stop the command, unless one uses the
  other.** The Python program merges its files in order, the first one winning,
  and reports nothing. Here a package of an extension wins over the packages it
  uses, directly or through others, and the order of a `use` list decides
  nothing. Packages of which neither uses the other are equals, and one setting
  given two different values by two of them is reported with both files and
  lines, until the front matter (or `-O`) sets it.
- **One input per command, and a document brings in its own parts.** Several
  files are not joined end to end, and the settings hold no list of the
  document's parts (the Python program's `content_parts`, each with `input`,
  `type` and `label`): a document brings its parts in with `\input{file}` where
  they belong, a command the Python program does not have. A part whose content
  continues the text around it is written `\input{part.flm}` where it goes. A
  part that opens with a heading (the Python program's `type: chapter` with the
  part's `title` and `label`) says so in its own front matter,
  `flm: {input_as: {heading: chapter, label: chap:x}}` beside its `title`, and
  is included with `\input{part.flm}` like any other. A file brought in may set
  `flm.parsing` and `flm.input_as` in its front matter, and nothing else under
  `flm`.
- **Four exit statuses**, where the Python program exits with 1 for every
  failure: 0 for success (with warnings or not); 1 for a run that reported
  errors (the document's, or an output that could not be written); 2 for a
  command that could not start (its arguments, a file, the front matter, a name,
  a setting); 3 for a failure of the program itself.
- **Every problem of a document is reported**, where the Python program stops at
  the first.
- **A settings key that no feature declares is refused**, with status 2, where
  the Python program warns and continues.
- **The program's defaults.** The program sets the values of the Python
  program's defaults where this library's defaults differ: footnotes marked a,
  b, c, the heading above their list at level 1, figures captioned `Fig.` and
  tables `Tab.`, both numbered I, II, III, and the heading above the list of
  cited works at level 1, as the Python program headed every list of notes. The
  Python program marked footnotes with raised numbers on plain text; here a
  footnote's mark is the same on every output (a, b, c), since a setting chosen
  when the program starts applies to every output.
- **Formats, workflows and profiles.** Here `-p` chooses the profile, which runs the
  workflow of the same name (after using the extension `profile-NAME` when one
  exists), and `-f` only selects one of its outputs; the Python program's `-w`,
  which chose the workflow, is `-p` here. The Python format `-f latex` is the profile
  `-p latex-rendered` here, with other macros and another preamble ([The LaTeX
  output for LuaLaTeX](#the-latex-output-for-lualatex)); the Python workflow
  `flmlatex` is `-p latex-recomposed`; the workflow `runlatexpdf` is `-p pdf-latex`
  ([The LaTeX output for LuaLaTeX](#the-latex-output-for-lualatex)); `-f markdown` and
  `-f pdf` have no counterpart (`-f` never chooses the profile here).
- **An empty command line prints the usage**, where the Python program waits
  for content on standard input; standard input is given as `-`.
- **The settings schema is printed by a command**, `flm schema`, where the
  Python program has the option `--print-config-json-schema`. It prints the same
  bytes at every run, on standard output or into the file that `-o` gives: the
  schema of an extension's manifest by default, and that of a front matter with
  `--of front-matter`; its keys and descriptions are this library's.
- **Options with no counterpart**: `--print-merged-config`,
  `--validate-config-only` and `--very-verbose`.
- **The printed messages** carry no symbol and no color; each problem is printed
  with its position in the file first.
- **A hidden record beside an output written with `-o`.** Beside the main file
  of each output, `.<file>.flm-written` lists the files the render wrote for that
  output; the next render of the same output reads it and removes the files it no
  longer produces, so an output that lost a file leaves no outdated copy behind. The Python program wrote no
  such record.
- **The recomposed LaTeX's `simple` compose template numbers sections as the
  document does and loads no package of its own**, where the Python template
  turned section numbering off and loaded `hyperref` and `phfnote`. A heading is
  numbered unless its author writes a star, as on the page; loading packages is
  left to the document's author, in a compose template of their own.
- **`-n` concerns standard output alone.** A file written with `-o` holds the
  output as it was produced, its final newline included, where the Python
  program also left the newline out of the file under `-n`.
- **The recomposed LaTeX of most documents cannot be printed on standard
  output.** It comes with a style file beside the document as soon as the
  document has a heading, a footnote or a label, and standard output receives
  one file only: write `-o paper.tex` (which also writes
  `paper-flmrecomposed.1a2b3c4d.sty`) or `-o paper/`. The Python program
  printed one LaTeX file with the definitions in its preamble.
- **New**: `flm check`, `--continue-with-errors`, `-O`, `--layout inline`,
  `-o DIR/`, which writes into a folder, a workflow of several outputs, all
  written by one command under one stem (`-o paper.html` writes `paper.html`
  and `paper.txt` for the outputs `html` and `text`), and a compose template
  that an extension provides, given with `-t`, which may ship files of its own
  (a stylesheet, a logo).

## Where to look next

- [FLM language syntax](crate::guide::language_syntax): the markup this library
  accepts today.
- [Embedding flm-core](crate::guide::embedding): the steps a Python host
  performs, done with this library.
- [Extending FLM: features and packages](crate::guide::extending): writing the
  counterpart of a Python feature.
- [Concepts overview](crate::guide::concepts_overview): the terms used above, in
  one place.
- [The command line](crate::guide::command_line): the `flm` program's commands,
  settings and extensions.
- [Rendering](crate::guide::rendering): the four outputs in detail.
