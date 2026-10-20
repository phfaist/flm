# FLM language syntax

FLM is a markup language for writing documents in plain text. An author marks
*what each part of the text is* — emphasized, a heading, a reference,
mathematics, code taken exactly as written — with a syntax borrowed from LaTeX: a
command begins with a backslash, `{…}` groups content, and `\begin{…}` …
`\end{…}` encloses an environment. A *backend*, the part of the library that
writes one output format such as HTML ([`BackendDef`](crate::core::registry::BackendDef);
see [backends, lowering and
outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)), then
decides what each construct becomes in that format.

This chapter is the author's reference of the language. It describes what the
parser reads, how the text is laid out in paragraphs and blocks, and every
command and environment of the standard package
([`stdcore`](crate::stdcore)), with what each one produces. The rendered samples
are plain text printed by the [text backend](crate::stdcore::backends::text);
another backend writes the same markup differently — the
[LaTeX backend](crate::stdcore::backends::latex_recomposed), for example, writes
it back as LaTeX; this chapter calls that output the *recomposed LaTeX*.

FLM takes LaTeX's syntax but not its processing model. Parsing never expands a
macro, an author never steps a counter by hand, and there is no page model:
numbering, references and the expansion of user definitions are done by
the library after the document is parsed, from what the document declares.

**The commands that exist depend on the features a program registers.** Each
command and environment is an *element* (the value that says what a node of the
document tree means; see [`ElementSpec`](crate::model::element::ElementSpec)) that a *feature*
declares: a feature ([`FlmFeature`](crate::core::registry::FlmFeature)) is a
Rust value that contributes a related bundle of elements, stages and rules to a
[registry](crate::core::registry), the record of everything the language and the
processing use, built once before any document is read. The program that
processes a document — the `flm` command-line program, or any other program
built on the library — chooses which features
it registers and how each is configured (see [features and
packages](crate::guide::concepts_overview#features-and-packages)). This chapter
describes the standard package under its default configuration. A program that
registers other features has other commands, written in the same syntax; [the
extension guide](crate::guide::extending) shows how a feature is written.

## A document at a glance

```text
\section{Introduction}\label{sec:intro}

This is \emph{emphasized} text by \myname.

\begin{equation}
  E = mc^2
\end{equation}

See \ref{sec:intro} and \ref{sec:missing}.
```

The text backend prints this document as:

```text
Introduction
============

This is emphasized text by FLM.

  \begin{equation}
    E = mc^2
  \tag*{(1)}\end{equation}

See Introduction and sec:missing.
```

Five things in the sample show how the language works:

- `\emph{…}` is a *macro* with one argument; it marks its argument as
  emphasized, which plain text cannot show.
- `\label{sec:intro}` is a *field* written after the heading it labels: data
  the heading reads, not a command of its own (see [labels and
  references](#labels-and-references)). `\ref{sec:intro}` shows the labeled
  heading's title.
- `\myname` is a *user definition*: a macro that the program processing this
  document declared to stand for the text `FLM` (see [author
  definitions](#author-definitions)). Without such a definition, `\myname` is
  an unknown macro and an error.
- The mathematics is printed as its source, with the equation's number,
  `(1)`, written as a `\tag*` because the text backend has no other way to show
  it (see [mathematics](#mathematics)).
- `sec:missing` is a label no construct defines, so that reference is reported
  and its label text is printed instead.

## How the source is read

An FLM document is UTF-8 text. Every character is ordinary text except the few
the parser reads specially:

| written | meaning |
|---|---|
| `\` followed by letters, or by one other character | a command: a macro, or `\begin`/`\end` of an environment |
| `{` … `}` | a group |
| `%%` up to the end of the line | a comment |
| a blank line | a paragraph break |
| `\(` … `\)`, `\[` … `\]` | inline and display mathematics |
| `~` | a non-breaking space |
| a lone `%`, and `$` | refused as ambiguous (below) |

Three *parsing settings* ([`FlmParsing`](crate::lang::FlmParsing)) govern these
rows; the program sets them when it builds its registry
([`RegistryInputs::parsing`](crate::core::registry::RegistryInputs::parsing)):
whether comments are recognized at all
([`comments`](crate::lang::FlmParsing::comments)), what starts one
([`comment_prefix`](crate::lang::FlmParsing::comment_prefix), `%%` by default),
and whether `$` delimits mathematics
([`dollar_math`](crate::lang::FlmParsing::dollar_math), off by default). The
`flm` program reads them from the key `flm.parsing` of a document's front matter
(see the [command-line guide](crate::guide::command_line#configuration-files-and-document-front-matter)),
and an included file may set its own for its content (see [including
files](#including-files)). The table above shows the defaults. Of the other rows,
only `~` comes from the standard package: it is a special character sequence of
the [`literals`](crate::stdcore::features::literals) feature. Commands, groups,
paragraph breaks and the mathematics delimiters belong to the parser itself.

A comment starts with two characters, `%%`, not with the single `%` that LaTeX
uses. A lone `%` is a **forbidden character** under the default settings: it is
reported as [`PercentForbidden`](crate::lang::PercentForbidden)
(`flm.lang.percent-forbidden`), whose message gives the spellings to use
instead, and it is kept in the text as an ordinary character, so the rest of the
document still parses. A lone `%` is refused because it is ambiguous: in
`add 10% to the total` it is a percent sign, and in `% a note to myself` it is
what a LaTeX author writes for a comment. The language asks for an unambiguous
spelling instead of guessing: `\%` for the percent sign, `%%` for a comment.
Where comments are on and the comment prefix is a lone `%`, the character is
not ambiguous and is not forbidden
([`FlmParsing::percent_forbidden`](crate::lang::FlmParsing::percent_forbidden)).

With `dollar_math` off, the default, a `$` in the source is reported in the same
way, as [`DollarMathDisabled`](crate::lang::DollarMathDisabled)
(`flm.lang.dollar-math-disabled`), and kept in the text as an ordinary
character. The reason is the same ambiguity: a document that writes `$` for an
amount of money would change meaning on the day its program switched
`dollar_math` on. `\$` writes the dollar sign in either setting.

### Writing a character that means something else

Nine spellings each stand for a single character. The
[`literals`](crate::stdcore::features::literals) feature of the standard package
declares them, and each produces exactly its one character as ordinary text:

| written | character |
|---|---|
| `\%` | `%` |
| `\#` | `#` |
| `\&` | `&` |
| `\$` | `$` |
| `\ ` (a backslash and a space) | a space |
| `\{` | `{` |
| `\}` | `}` |
| `\textbackslash` | `\` |
| `~` | a non-breaking space, U+00A0 |

This language makes fewer characters special than LaTeX does, but all nine
spellings exist, so that an author can write any of these characters without
knowing *where in a document* it is special. `&` and `#` are ordinary characters in running
text, but `&` separates the entries of a [table of cells](#tables-of-cells) and
`#` introduces a placeholder in the template of an [author
definition](#author-definitions); `$` is special only where the program switched
on `dollar_math`; a lone `%` is refused, as described above. The escaped
spelling works wherever it is written.

`\ ` is not the same as a typed space. A run of ordinary spaces in the source
becomes one space in the output (see [paragraphs, blocks and
typography](#paragraphs-blocks-and-typography)), whereas the space that `\ `
stands for is added after that step, so the spaces around it do not absorb it.
`~` is a non-breaking space: an output that wraps lines does not break a line
there.

A long stretch of characters that would otherwise be read specially is easier to
write as [verbatim content](#verbatim-content), which also accepts an unbalanced
brace.

One further command, `\@`, produces nothing. It is a hint about whether a period
ends a sentence, which a typesetter that chooses its own spacing after a sentence
can act on: written after a period, as in `cf.\@ the appendix`, it says that the
period ends an abbreviation and not a sentence; written before a period that
follows a capital letter, as in `NASA\@.`, it says that the period ends a
sentence. The recomposed LaTeX writes it back; plain text and HTML have no spelling
for such a hint, and there it disappears.

### Macros

A macro is written `\name`, where the name is a run of ASCII letters, or `\c`,
where `c` is a single character that is not a letter. This is TeX's rule, and it
is why `\\`, `\(` and `\[` are single commands. The whitespace directly after a
name made of letters is part of the macro and disappears with it, as in LaTeX:
`\version.` and `\version .` print the same. Whitespace after a closing brace is
ordinary text, so `\emph{a} b` keeps its space.

A macro's arguments are part of its declaration: nothing about them is inferred
from the source. Each argument is read by one of the *standard argument kinds*
([`ArgStdKind`](crate::lang::mint::ArgStdKind)), or by a parser of the feature's
own for a syntax no standard kind expresses. The commands of the standard
package use these kinds:

| written | argument kind | example |
|---|---|---|
| `{…}`, or a single token | [mandatory](crate::lang::mint::ArgStdKind::Mandatory) | `\emph{text}`, `\emph x` |
| `[…]`, may be left out | [optional](crate::lang::mint::ArgStdKind::Optional) | `\verbcode[python]+…+` |
| `*`, may be left out | [star](crate::lang::mint::ArgStdKind::Star) | `\section*{Title}` |
| `[key=value,…]`, may be left out | [key-value options](crate::lang::mint::ArgStdKind::KeyVals) | `\ref[noprefix]{sec:a}` |
| `{…}` read as plain characters | [character group](crate::lang::mint::ArgStdKind::CharsGroup) | `\label{sec:intro}` |
| raw text between a pair of delimiters | [verbatim](crate::lang::mint::ArgStdKind::Verbatim) | `\verbtext+a{b}\c+` |
| field macros written after the command | [tack-on argument](crate::lang::mint::ArgStdKind::TackOn) | `\section{Title}\label{sec:a}` |

A feature, or a [user definition](#author-definitions), may also declare an
argument of one of the other standard kinds: a marker made of one character
other than `*` (`\m!`), a group between a given pair of characters, mandatory
(`\m(arg)`) or optional (`\m<arg>`), a group between any of the pairs `{…}`,
`[…]`, `(…)` and `<…>`, a braced group that a single token cannot replace, and
*embellishments*, a run of markers each followed by one expression
(`\m_{a}^{b}`). [`ArgStdKind`](crate::lang::mint::ArgStdKind) describes each.

A mandatory argument takes a single token when no brace follows, so `\emph xy`
is `\emph{x}` followed by `y`. Inside a character group, a backslash and a
special character sequence are ordinary characters, so a label is read as the
characters written; comments and nested braces are still recognized there
([`lang::chars_group`](crate::lang::chars_group)). A *tack-on argument* is the
trailing argument through which a construct absorbs the *field* macros written
after its other arguments, such as `\label` (see [fields: data a construct
reads](crate::guide::concepts_overview#fields-data-a-construct-reads)). A field
is not a command: `\label{…}` is read only after a construct that declares it,
and in `\emph{x}\label{a:b}`, `\label` is an unknown macro.

What a mandatory or optional argument holds is also part of the command's
declaration ([`ArgLayout`](crate::lang::mint::ArgLayout)), never something the
author chooses where the command is used. By default an argument holds *inline
content*: running text, with no paragraph inside it. A blank line in such an
argument is read as a single space and reported as
[`ParbreakInInline`](crate::lang::rewrites::blocks::ParbreakInInline), and a
block-level element in it, such as a list, is reported as
[`BlockInInline`](crate::lang::rewrites::blocks::BlockInInline) and kept where it
is written. A command may instead declare an argument that holds *blocks*: there
a blank line separates paragraphs and a list is a block of its own, as in the
body of a document, and text written on a single line is one paragraph. Or the
declaration may let the content decide: the argument then holds blocks when its
content has a blank line or a block-level element in it, and inline content
otherwise. An argument holds what its declaration says regardless of where the
command is written, so an argument that holds blocks does so even inside a
heading's title.

A feature may also declare a command *block-transparent* (see
[block-transparent
elements](crate::guide::concepts_overview#block-transparent-elements)): the
content of its arguments is laid out as if it were written in the text around
the command. A blank line in such an argument is then not an error but a
paragraph break of the surrounding text — what the command marks continues
across it into the next paragraph — and a list or a displayed formula in it is a
block of the surrounding text. Such a command never becomes a block of its own.
Where the surrounding text is itself inline content, as in a heading's title,
the blank line is reported there, as in any inline content.

A declaration may also make a set of commands available inside one argument or
body and nowhere else, as `\item` is available inside a list's body and nowhere
else. Such a set is a *scoped package*
([`ScopedPackageDef`](crate::lang::mint::ScopedPackageDef); see [scoped
packages](crate::guide::concepts_overview#scoped-packages)). Outside the
arguments and bodies that make it available, such a command is an [unknown
macro](#unknown-macros-and-environments), unless another declaration defines
the name there.

### Environments

An environment is written `\begin{name}` … `\end{name}`. The arguments declared
for it follow `\begin{name}`, in the same kinds as a macro's:

```text
\begin{verbatimcode}[python]
def f(x):
    return {x} + 1
\end{verbatimcode}
```

An environment's declaration also states three things about its body
([`BodyDef`](crate::lang::mint::BodyDef)): whether the body holds blocks or
inline content ([`BodyLayout`](crate::lang::mint::BodyLayout)), the parsing
mode the body is read in (the body of a display-math environment is read in
mathematics mode), and whether the body is raw text.

### Special character sequences

A *special character sequence* is a sequence of characters recognized wherever
it is written, with no backslash before it; `~` has this form in LaTeX. Macros,
environments and special character sequences are the three *callable kinds* of
the language ([`CallableKind`](crate::lang::mint::CallableKind)), the forms in
which an element can be written. The standard package declares two special
character sequences: `~`, a non-breaking space everywhere (see [writing a
character that means something
else](#writing-a-character-that-means-something-else)), and `&`, which separates
the entries of a row inside `\celldata{…}` and is declared nowhere else (see
[tables of cells](#tables-of-cells)). Elsewhere `&` and `#` are ordinary
characters of a document. A [user definition](#author-definitions) can declare
a special character sequence of its own; one that reuses `~` must be marked as
explicitly replacing the standard declaration
([`Definition::overriding`](crate::lang::rewrites::userdefs::Definition::overriding)).

### Groups

`{…}` groups content. A group is inline content: it takes part in the paragraph
around it, its content is read like any other content, and a group is where a
command's mandatory argument is usually written. A group does not by itself
change how its content is printed: `three {grouped words} four` prints as
`three grouped words four`. A block-level element inside a group is reported as
[`BlockInInline`](crate::lang::rewrites::blocks::BlockInInline) and kept where it
is written, and a blank line inside a group is read as a single space and
reported as
[`ParbreakInInline`](crate::lang::rewrites::blocks::ParbreakInInline).

### Comments

A comment runs from `%%` to the end of the line, and the line break belongs to
the comment. A comment on a line of its own therefore disappears entirely, and a
comment after text joins that line to the next: `word%% note` followed by a line
holding `next` prints `wordnext`, while `word %% note` keeps the space and prints
`word next`. Nothing in the output records that a comment was there.

## Paragraphs, blocks and typography

Nothing about paragraphs is decided while the source is read. After parsing, the
*block-structure pass* ([`lang::rewrites::blocks`](crate::lang::rewrites::blocks))
lays out the parsed tree and resolves its text. It works on *content regions*:
a content region is a run of sibling nodes laid out together, such as the top
level of a document, an environment's body, an argument's content, or the
content of a `{…}` group. Each element declares a *block role*
([`BlockRoleAspect`](crate::model::aspects::BlockRoleAspect), whose values are
[`BlockRole`](crate::model::aspects::BlockRole)): whether it is a block of its
own, inline content inside a paragraph, or attached to the block it occurs in.
The pass lays out each content region in
one of three ways, its *region layout*:

- **as blocks**: the content is divided into paragraphs, and every element of
  the block role *block* is placed on its own between them;
- **inline**: no paragraph is built, and a blank line or a block found there is
  reported;
- **opaque**: the content is carried over exactly as written. Mathematics and
  verbatim content are opaque (see [opaque
  regions](crate::guide::concepts_overview#opaque-regions)).

Which layout a content region gets comes from declarations only: the block
role each element declares, the layout an environment declares for its body and a command for each of its
arguments (see [macros](#macros) and [environments](#environments)), and the
layout of the document's top level
([`FragmentLayout`](crate::model::doc::FragmentLayout)), which the program
chooses. The same pass performs *text resolution* in every content region that
is not opaque: whitespace is collapsed and the [typographic
substitutions](#typographic-substitutions) are applied. After the pass, the tree
is never changed again (see [frozen
trees](crate::guide::concepts_overview#frozen-trees-and-computed-data)).

### Paragraphs

A blank line separates paragraphs. Inside a paragraph, a single line break is
ordinary whitespace, every run of spaces, tabs and line breaks becomes one
space, and the whitespace at the two ends of the paragraph is removed:

```text
Two   words   with   extra spaces,
and a line break inside the paragraph.

A second paragraph. %% this comment ends at the line break
Still the second paragraph.
```

```text
Two words with extra spaces, and a line break inside the paragraph.

A second paragraph. Still the second paragraph.
```

An element whose block role is *block* is a block of its own: it is not part of
the paragraph around it, and the text before it and the text after it become two
paragraphs. Among the elements of the standard package, the blocks are the
[headings](#headings), the [display-math environments](#mathematics), the
[lists](#lists), the [tables of cells](#tables-of-cells), the
[theorems](#theorems), the [term definitions](#defined-terms), the
[quotations](#quotations), the [floats](#floats), the `verbatimcode`
environment, a [file included with a heading of its
own](#a-file-that-opens-with-a-heading), and the node an [unknown
environment](#unknown-macros-and-environments) becomes. Everything else is
inline, with two cases decided by the position: an image is a block as the
direct content of a float, and an xnote shown in place is a block where its
content holds paragraphs. A [user definition](#author-definitions) declares its
own block role. Whitespace between two blocks is removed.

### Typographic substitutions

The source spelling `---` denotes an em dash in the same way that `\emph{…}`
denotes emphasis, so FLM replaces such spellings in the text itself rather than
leaving them to each backend. Four families of substitutions are applied, all of
them on by default:

| written | becomes | family |
|---|---|---|
| `---`, `--` | `—`, `–` | [`ligature_unicode_dashes`](crate::lang::rewrites::blocks::BlocksConfig::ligature_unicode_dashes) |
| `...` | `…` | [`ligature_unicode_ellipses`](crate::lang::rewrites::blocks::BlocksConfig::ligature_unicode_ellipses) |
| two backticks, `''`, one backtick | `“`, `”`, `‘` | [`ligature_unicode_quotes`](crate::lang::rewrites::blocks::BlocksConfig::ligature_unicode_quotes) |
| `"`, `'` | an opening or a closing quote, decided by position | [`auto_unicode_quotes`](crate::lang::rewrites::blocks::BlocksConfig::auto_unicode_quotes) |

```text
``Quoted'' text --- an em dash, an en dash -- and an ellipsis...
"Straight quotes" and don't forget the apostrophe.
```

```text
“Quoted” text — an em dash, an en dash – and an ellipsis… “Straight quotes” and
don’t forget the apostrophe.
```

A straight quote opens at the start of a text that nothing precedes, and after
whitespace, an opening bracket or a dash; a single quote also opens before a
digit (`x'9` becomes `x‘9`), except directly after a letter, a digit or an
underscore and before two digits and an `s`, a decade abbreviation (`x'90s`
becomes `x’90s`); after a space, `the '90s` becomes `the ‘90s`. A straight quote closes everywhere
else, so `don't` becomes `don’t` and `'hi'` becomes `‘hi’`. Text already written
with curly quotes, accented letters or East Asian characters is not changed.

The program chooses which families apply when it builds its registry
([`BlocksConfig`](crate::lang::rewrites::blocks::BlocksConfig), passed through
[`StdcoreConfig::blocks`](crate::stdcore::StdcoreConfig::blocks)); a document
cannot switch them. Collapsing whitespace is not one of the families and always
happens. Mathematics and verbatim content are exempt from all of it: they are
[opaque regions](crate::guide::concepts_overview#opaque-regions), carried
through exactly as written.

## Inline formatting

Four commands mark their one mandatory argument with a text style. They are the
four values of the element `flm.textstyle`
([`TEXT_STYLE`](crate::stdcore::features::textstyles::TEXT_STYLE)), declared by
the [`textstyles`](crate::stdcore::features::textstyles) feature:

| written | meaning |
|---|---|
| `\emph{…}` | semantic emphasis |
| `\textbf{…}` | bold weight |
| `\textit{…}` | italic shape |
| `\texttt{…}` | a monospaced face |

```text
Some \emph{emphasis}, \textbf{bold}, \textit{italic} and \texttt{monospace}.
```

All four are inline: they take part in the paragraph around them and keep the
whitespace next to them. What each one means is a value of
[`TextStyle`](crate::stdcore::features::textstyles::TextStyle). A backend with no
way to show a style prints the content without it, as the text backend does for
all four:

```text
Some emphasis, bold, italic and monospace.
```

## Headings

The standard package declares five **heading kinds**, one per level, from
`section` (level 1) to `subparagraph` (level 5), each written with a command of
its own name: `\section`, `\subsection`, `\subsubsection`, `\paragraph`,
`\subparagraph`. A heading kind ([`HeadingDecl`](crate::stdcore::features::headings::HeadingDecl))
is what a heading is; the [`headings`](crate::stdcore::features::headings)
feature declares the heading kinds and the commands that write them from its
configuration ([`HeadingsConfig`](crate::stdcore::features::headings::HeadingsConfig)),
so a program may declare other heading kinds and other commands. A level is a number
counted from zero, a smaller level being an outer one, and every number is a
valid level: a program may declare a heading kind at level 0, above the sections, such as
a chapter. Every heading is a node of the element `flm.heading`
([`HEADING`](crate::stdcore::features::headings::HEADING)), and every heading
command takes, in this order: an optional star, an optional bracket of
key-values, the title, and any number of `\label` fields
([`heading_args`](crate::stdcore::features::headings::heading_args)).

```text
\section{Introduction}\label{sec:intro}

\subsection{Formatting}\label{sec:fmt}

\section*{Unnumbered}

\section[toc=false]{Not in the contents}
```

A heading is a block by default: it is placed on its own between the paragraphs
around it, unless the program declared its heading kind as a
[run-in heading](#run-in-headings). Every heading is a reference point (a place a
reference can point at; see [reference points, labels and
anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)),
whether it is labeled or not.

### The star and the key-values

The bracket holds key-values, `key=value` separated by commas; a key written
alone means `key=true`. Three keys are read:

- `numbered=true|false` — whether the heading takes a number, where its heading
  kind is numbered at all (see [numbered headings](#numbered-headings)). The star is
  exactly `numbered=false` and nothing else: `\section*{T}` and
  `\section[numbered=false]{T}` are the same heading.
- `toc=true|false` — whether the heading is listed in the [table of
  contents](#the-table-of-contents). Every heading is listed unless it says
  `toc=false`, **starred or not**: a `\section*{T}` is listed, and
  `\section*[toc=false]{T}` (or `\section[numbered=false,toc=false]{T}`) is
  neither numbered nor listed. A heading kind the program declared as never listed
  ([`skip_toc`](crate::stdcore::features::headings::HeadingDecl::skip_toc))
  ignores `toc` and reports nothing about it. The outputs also carry the fact on
  the heading itself: the HTML output writes it as the attribute `data-flm-toc` on
  every heading, block or run-in, and the recomposed LaTeX passes it to its LaTeX
  hook for headings (`\flmHeading`, a macro the style file defines with a default;
  see [`latex::preamble`](crate::stdcore::backends::latex::preamble) and [the LaTeX
  hooks](crate::guide::rendering#the-latex-hooks)), whose default lists a starred
  heading written with `toc=true` in LaTeX's table of contents.
- `alt={…}` — the heading's **alternative title**, a shorter or plainer form of
  the title, shown by the table of contents in place of the title:
  `\section[alt={Short title}]{A long title with \emph{markup}}`. The value is
  FLM content, like the title: it may hold markup, a reference or a footnote (a
  footnote shows its mark in the table of contents, and its note is listed once).
  The heading itself shows its full title where it stands, and a reference to the
  heading (`\ref`) shows the full title too, where it shows a title. `alt={}` is an
  empty alternative title: the entry shows the heading's number alone, or nothing
  where the heading has no number. `alt` written without a value is the Error
  `flm.heading.missing-value` (`` the heading option `alt` must be given a value ``),
  and the key is ignored. The recomposed LaTeX hands the alternative title to its
  LaTeX hook for headings as the hook's key `alt`, so LaTeX's own table of contents
  shows it too.

The key `pdfbookmark` (the title of a PDF bookmark) is reserved and not
implemented yet: it is ignored and reported as the Warning
`flm.heading.key-not-implemented` (`` the heading option `pdfbookmark` is not
implemented yet ``). Any other key is the Error
`flm.heading.unknown-key`; a value of `numbered` or `toc` that is neither `true`
nor `false` is the Error `flm.heading.bad-value`; and a bracket whose keys are
not plain text, such as `\section[\emph{x}=1]{T}`, is the Error
`flm.heading.unreadable-options`. In each case the key, or the whole bracket, is
ignored and the heading is written. The bracket holds key-values, not LaTeX's
short title: `\section[Short]{A long title}` is the Error
`flm.heading.unknown-key` for the key `Short`; the alternative title is written
`\section[alt={Short}]{A long title}`.

A later key replaces an earlier one. A command that the program declared with
default key-values of its own
([`HeadingCommand`](crate::stdcore::features::headings::HeadingCommand)), for
example a command that writes a `section` left out of the table of contents,
has its defaults replaced in the same way, first by the star and then by the
bracket.

### Numbered headings

**Headings are not numbered unless the registry's configuration says so.** Each
of the five standard heading kinds counts in a *counter* of its own (a dotted
name under which things are counted in document order; see
[`CounterDecl`](crate::stdcore::features::numbering::CounterDecl) and [counters
and counter styles](crate::guide::concepts_overview#counters-and-counter-styles)),
named after the heading kind under the headings feature's prefix:
`flm.headings.section`, `flm.headings.subsection`, and so on to
`flm.headings.subparagraph`. Which heading kinds are numbered is the configuration's
depth,
[`HeadingsConfig::numbering`](crate::stdcore::features::headings::HeadingsConfig::numbering):
no heading kind by default, every heading kind, or every heading kind down to a
level. "Number the
sections and the subsections and nothing deeper" is a depth of two:

```rust
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::features::headings::{HeadingNumbering, HeadingsConfig};

let config = StdcoreConfig::new()
    .headings(HeadingsConfig::new().numbering(HeadingNumbering::UpToDepth(2)));
# let _ = config;
```

The depth is fixed when the registry is built, like the rest of the headings
configuration.

The number is written before the title, followed by `. `: `1. Intro`,
`1.2. Details`. That text is a setting of each heading kind, its title number
format, `#{number}. ` by default
([`HeadingDecl::title_number_format`](crate::stdcore::features::headings::HeadingDecl::title_number_format)).
The first three levels are written in decimal digits, each numbered **within**
the level above it. This means two things: the number restarts whenever the
level above advances, and the number of the level above is written before its
own, so `1`, then `1.2`, then `1.2.3`. `\paragraph` and `\subparagraph` are
written in letters. A paragraph's number restarts at every `\section`,
`\subsection` and `\subsubsection` and shows none of their numbers (`a`, `b`),
and a subparagraph's number shows the number of the paragraph it is in (`a.b`).

A heading written with a star, or with `numbered=false`, takes no number **and
advances no counter**, so the headings after it continue from the last number
given, as in LaTeX:

```text
\section{First}      1. First
\section*{Aside}     Aside
\section{Second}     2. Second
```

A `\ref` to a numbered heading shows its number after the section sign, `§ 1.2`
(with a narrow no-break space), and `\ref[noprefix]{…}` shows `1.2` alone; a
reference to a heading that has no number shows its title. A subsection written
before the first section has no section number to be written with: it takes no
number and is reported (`flm.numbering.parent-uninitialized`). The [numbering guide](crate::guide::numbering)
describes what a document's numbering options can change, such as giving the
sections the value 0 before the first one, so that such a subsection reads
`0.1`, or taking a level out of the hierarchy it was declared in.

### Run-in headings

A **run-in heading** is a heading set at the start of the paragraph it
introduces, on the paragraph's first line, rather than on a line of its own (see
[`headings`](crate::stdcore::features::headings)): where the program declares
`paragraph` as run in, the text backend prints `\paragraph{Note} Some content.`
as `Note. Some content.`, with the stop set by
[`TextRenderOptions::run_in_stop`](crate::stdcore::backends::text::TextRenderOptions::run_in_stop).
The program decides which heading kinds are set that way, not the author: each heading kind
says whether it is run in
([`HeadingDecl::run_in`](crate::stdcore::features::headings::HeadingDecl::run_in);
[`HeadingsConfig::run_in_levels`](crate::stdcore::features::headings::HeadingsConfig::run_in_levels)
sets it by level), and the program passes the configuration through
[`StdcoreConfig::headings`](crate::stdcore::StdcoreConfig::headings). No standard
heading kind is run in, so unless a program says otherwise every heading is a block.

A heading of a run-in heading kind is not a block: the block-structure pass opens a
paragraph at the heading and lets the content that follows join that paragraph,
so the heading and the text it introduces are one paragraph. A run-in heading
that nothing follows, such as one written alone before a blank line, makes a
paragraph holding only itself.

Nothing else about the heading changes: it keeps its level, it is listed in a
table of contents unless it says `toc=false`, and a reference can point at it
and show its number or its title.

### What the text backend prints for a heading

The text backend prints a heading at the root heading level as its title with a
rule of `=` under it, and a heading one level deeper with a rule of `-`. A
deeper heading has no rule: its title is preceded by `#` characters and a
space, three for a heading two levels below the root and one more for each
further level. The root heading level is a render option of the text backend
([`TextRenderOptions::root_heading_level`](crate::stdcore::backends::text::TextRenderOptions::root_heading_level)),
1 by default, the level of a section; the [rendering
guide](crate::guide::rendering) gives the details.

```text
Introduction
============

Formatting
----------

Unnumbered
==========
```

### The table of contents

The **table of contents** is a list of the document's headings, built by the
[`toc`](crate::stdcore::features::toc) feature of the standard package. An author
does not write it in the document (there is no `\tableofcontents` command): the
output places it before the content when the render asks for it.

**Which headings are listed.** Every heading is listed, in document order, unless
it says `toc=false` or its heading kind is declared as never listed (see [the star
and the key-values](#the-star-and-the-key-values)), and provided its level is at
most the setting `depth` of the feature `toc`: 3 by default, which lists the
sections, the subsections and the subsubsections. A starred heading is listed like
any other. The heading of a file placed as a heading input is listed where that
file is placed. The setting is written in the front matter, or with `-O` on the
command line (see the [command-line guide](crate::guide::command_line)):

```text
---
flm: {features: {toc: {depth: 2}}}
---
```

**How it is turned on.** The table of contents is a *piece* (content the output
places beside the document's content; see
[pieces](crate::guide::concepts_overview#pieces)) named `flm.toc`, and it is off
by default. A render places it when the setting `include` of the feature
`pieces` lists it, in the front matter (`flm: {features: {pieces: {include:
[flm.toc]}}}`) or on the command line (`-O 'pieces.include=[flm.toc]'`), or when
the compose template of the output requires it.

**What an entry shows.** Each entry shows the heading's number, where the heading
has one, then its alternative title where `alt=` gives one, and its title
otherwise. The entries are nested by level: the subsections of a section are
listed in a list inside that section's entry. A heading that is not listed is
skipped, and the listed headings under it are nested in the entry of the listed
heading before them. A heading two levels deeper than the one before it (a
subsubsection directly under a section) is nested one list deeper, not two.

**Where it goes on each output.**

- On the HTML page, the table of contents is a `<nav>` of nested lists, each entry a
  link to its heading. The program's compose template `simple` places it between
  the page's header and its main content (see the [rendering
  guide](crate::guide::rendering) for the markup and its stylesheet).
- In the plain text, it is printed before the content, after the title, the
  authors and the date, one entry per paragraph. The entries are printed at the
  left margin; the nesting shows only through the numbers.
- In the recomposed LaTeX, FLM's table of contents writes nothing. LaTeX builds
  its own table of contents, where the LaTeX document asks for one with
  `\tableofcontents`, from the headings it typesets; the recomposed LaTeX gives
  each heading's `toc` and `alt` to LaTeX for that table, through its LaTeX hook
  for headings (on by default; the hook's default does not take an unstarred
  heading written `toc=false` out of LaTeX's table, see [the LaTeX
  hooks](crate::guide::rendering#the-latex-hooks)).

## Labels and references

`\label{type:label}` attaches a *label* to the construct it is written after, and
`\ref{type:label}` refers to that construct. A second reference command,
`\hyperref[type:label]{text}`, refers to the same constructs but shows the text
the author writes as its second argument instead of what the target shows. The
[`refs`](crate::stdcore::features::refs) feature declares all three. A fourth
command, `\eqref`, belongs to the mathematics feature and refers to equation
lines only (see [inside a display-math
environment](#inside-a-display-math-environment)).

A construct a reference can point at is a *reference point*: one place in a
document that can be referred to, numbered, or given an anchor, such as a
heading, an equation line, a theorem or a footnote
([`RefPoint`](crate::model::aspects::RefPoint); see [reference points, labels
and anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)).

```text
\section{Introduction}\label{sec:intro}

\subsection{Formatting}\label{sec:fmt}

See \ref{sec:intro}, \ref[noprefix]{sec:fmt} and \ref{sec:missing}.

Or read \hyperref[sec:fmt]{how to \emph{format} things}.
```

```text
Introduction
============

Formatting
----------

See Introduction, Formatting and sec:missing.

Or read how to format things.
```

### Writing a label

A `\label` is a *field*, not a command: it is read only after a construct that
declares it ([`label_fields_arg`](crate::stdcore::features::refs::label_fields_arg);
see [fields: data a construct
reads](crate::guide::concepts_overview#fields-data-a-construct-reads)). The
construct reads the field as data and attaches the label to its reference point.
Among the standard constructs, the headings, the list items, the term
definitions and the theorems declare it after their arguments. In three places
`\label` is instead a command available inside one body or argument, and the
label it writes is read in the same way: the body of a display-math environment, where it
labels one equation line (see [inside a display-math
environment](#inside-a-display-math-environment)), the body of a float (see
[floats](#floats)), and the optional argument of `\footnote` (see
[xnotes](#xnotes)). A field has no content
of its own, and what it means is decided by the construct that reads it: a user
definition may declare a field named `\label` and give it a meaning of its own;
such a field defines no label of the document.

A construct may carry several labels, each written as a `\label` field, and each
label's text must be spelled `type:label`: the *label type*, which says what sort of construct is
labeled, a colon, and the label itself. A text not spelled that way defines no
label and is reported where it is written (`flm.refs.bad-label`).

Each construct states which label types may be written for it. A heading accepts
`sec` and `topic`
([`LABEL_TYPES`](crate::stdcore::features::headings::LABEL_TYPES)), so
`\section{Sum}\label{eq:sum}` labels nothing: the type `eq` is for an equation,
not a section. An equation line that can be numbered accepts `eq`
([`LABEL_TYPES`](crate::stdcore::features::math::LABEL_TYPES)); one that can
carry no number accepts no type, because a reference to it would have nothing to
show. A construct may also accept every type, which is what a feature declares
for a construct whose label types are not fixed in advance.

### What a reference shows

A reference shows the first of three things that exists for its target: the
**number** this document gave the target, where it has one; else the target's
own **content**, such as a heading's title; else the **label** the reference was
written with, as written. Whether a target is numbered is decided by the
registry's configuration: the headings configuration numbers no heading by
default, so a `\section` shows its title unless the registry numbers sections
(see [the numbering guide](crate::guide::numbering)). What is shown is a link to
the target. A target with neither a number nor content, such as an unnumbered
float without a caption, has nothing to show: a reference to it is the Error
`flm.refs.nothing-to-show`, and the label is shown as written.

`\hyperref` takes the label in *brackets* and the text to show in braces
([`hyperref_args`](crate::stdcore::features::refs::hyperref_args)). The text is
ordinary content (formatting commands, a formula, another reference), and the
output shows it, linked to the target. In every other respect a `\hyperref` is a
`\ref`: the same labels, the same link target, the same reports. Written without
brackets (`\hyperref{text}`), it refers to no label, which is reported like any
other text not spelled `type:label`; the words are still shown, without a link.

### The options of `\ref`

`\ref` takes an optional key-value argument before its target
([`ref_args`](crate::stdcore::features::refs::ref_args)), which says how a
number is introduced. The words before a number are the target's *counter
style*'s (the description of how a counter's value is shown; see
[`CounterStyleDecl`](crate::stdcore::features::numbering::CounterStyleDecl) and
[counters and counter
styles](crate::guide::concepts_overview#counters-and-counter-styles)):

- the key `noprefix` drops the word before the number: `(3)` rather than
  `Eq. (3)`;
- the key `S` asks for the capitalized wording, `Equation (3)`;
- any other key written alone asks for the wording of that name, such as
  `\ref[lowercase]{lem:x}` for a theorem (see [theorems](#theorems)).

`S` is a preference: a counter style that declares a capitalized wording, under
the name `capital`, writes it, and a counter style that does not declare one
writes its usual wording, and nothing is reported: `\ref[S]{sec:intro}` on a numbered
section reads `§ 1`. A wording named explicitly is a requirement: where the
counter style of a numbered target does not declare it — `\ref[capital]{…}` on a
style with no capitalized wording, or a misspelled key — the reference is the
Error `flm.refs.undeclared-variant`, reported once for each such counter style,
and the style's usual wording is written. The options change nothing, and
nothing is checked, for a target that shows its content or its label. The
recomposed LaTeX leaves the wording of a reference to LaTeX: it writes the
options back for its LaTeX hook for references (`\flmRef`), whose default
translates the standard wordings and writes the usual wording for any other, so
the recomposed LaTeX reports no `flm.refs.undeclared-variant` and LaTeX reports
nothing either.

### A reference to several targets

**A `\ref` may refer to several targets at once**, written as one text with
commas between them: `\ref{eq:a,eq:b,eq:c}`. Each target is resolved and linked
on its own, and the whole is written as one list. The targets this document
numbered come first, one item per counter style: the numbers of one style are
written together, consecutive numbers collapsed into a range, after the plural
word the style declares, `Eqs. (1–3)`. Then comes every other target, one item
each, in the order written: a target without a number of its own shows its
content or its label there, and so does a target that resolves to nothing. Two
items are joined by "and", more items by commas and a final ", and":
`\ref{eq:a,thm:big}` reads `Eq. (1) and Thm. 1.1`, and
`\ref{eq:a,thm:big,note:n}` reads `Eq. (1), Thm. 1.1, and A note`. These joining
texts are the configuration of the references feature
([`RefsConfig`](crate::stdcore::features::refs::RefsConfig)), which a program may
change, to write "und" in place of "and" for example.

### Anchors

The link's target is an *anchor*: the identifier a link points at for a
reference point, computed once by the references feature
([`ANCHORS`](crate::stdcore::features::refs::ANCHORS)). Which anchor a reference
point gets depends on what it is:

- a labeled point gets its first label, the type and the label joined by a
  hyphen: `\label{sec:intro}` gives `sec-intro`
  ([`label_anchor`](crate::stdcore::features::refs::label_anchor), which
  encodes the two parts separately with
  [`encode_anchor`](crate::stdcore::features::refs::encode_anchor));
- an unlabeled heading gets its title, after a prefix and a double hyphen:
  `sec--Introduction` (an xnote in the title is not part of it); a second
  unlabeled heading with the same title gets `sec--Introduction-2`, a third
  `-3`;
- an unlabeled equation line gets its position, likewise after a prefix and a
  double hyphen (see [inside a display-math
  environment](#inside-a-display-math-environment)).

No counter takes part in a label's anchor, so one label always gives the same
anchor. A hyphen next to the joining hyphen — a label
whose text begins with a hyphen, or whose type ends with one — could read as one
of the double-hyphen forms, so it is encoded in another way: `\label{sec:-Foo}`
does not give `sec--Foo`.

### Errors in labels and references

- A **reference that resolves to nothing** shows its own label text (for a
  `\hyperref`, the text its author wrote), marked as unresolved. It is the Error
  `flm.refs.unresolved` by default, and a Warning when the program that
  requested the rendering asks for that
  ([`RefsRenderOptions`](crate::stdcore::features::refs::RefsRenderOptions) with
  [`UnresolvedRefs::Allow`](crate::stdcore::features::refs::UnresolvedRefs)).
  Under the [`check`](crate::stdcore::CHECK_WORKFLOW) workflow, which checks a
  document and renders nothing (a *workflow* is a named set of demands together with
  the choices that go with them, a complete request for a kind of processing; see
  [`WorkflowDef`](crate::core::registry::WorkflowDef)),
  it is always a Warning, because a later rendering may supply labels defined in
  other documents.
- A **label written twice** is the Error `flm.refs.duplicate-label`: the first
  occurrence in document order keeps the label. Every workflow of the standard
  package runs the label check, rendering workflows included, so this is
  reported wherever the document is processed.
- A **label whose type the labeled construct does not accept** is the Error
  `flm.refs.label-type-not-accepted`, reported by the same label check. The
  label defines nothing, so a `\ref` to it resolves to nothing, and a construct
  whose every label was refused gets no anchor and cannot be linked to.
- A **text not spelled `type:label`** is the Error `flm.refs.bad-label`, and a
  **target with nothing to show** the Error `flm.refs.nothing-to-show`, both
  described above.

## Mathematics

Mathematics is written between `\(` and `\)` for inline mathematics and between
`\[` and `\]` for display mathematics; `$…$` and `$$…$$` work as well where the
program switched on [`dollar_math`](crate::lang::FlmParsing::dollar_math). Six
environments write display mathematics under their own name: `equation`,
`align`, `gather` and their starred forms
([`ENVIRONMENTS`](crate::stdcore::features::math::ENVIRONMENTS)). The
[`math`](crate::stdcore::features::math) feature declares them.

The `math` feature introduces mathematics into the language and processes none of
it. A second feature, a *processing feature*, decides which commands a formula may
hold, checks the formulas and writes them into the output: the feature of a *math
processor*, a package beside the standard one, chosen for the document as a whole. This
section describes mathematics under the library's math processor,
[`math-passthrough`](crate::math_passthrough); the `flm` program uses the extension
`math-mathjax` by default, which accepts only the commands MathJax knows ([mathematics
processed by
MathJax](crate::guide::command_line#mathematics-processed-by-mathjax)), and a document
chooses its processor with the key `math` of its front matter (`math: passthrough`; see
[the packages a document uses](crate::guide::command_line#the-packages-a-document-uses)).
In a program whose registry holds `math` and no math processor, a document without
formulas renders as any other, and a render reports each formula as an error and writes
its content as ordinary text.

```text
Inline math \(x_1 + \alpha\) and display math \[ \int_0^1 f(x)\,dx = 1 \] in
the same paragraph.

\begin{align*}
  a &= b \\
  c &= d
\end{align*}
```

Under `math-passthrough`, inside mathematics one rule changes: **an unknown command is
accepted without any report and kept as written**
([`OPAQUE`](crate::math_passthrough::OPAQUE)). Mathematical notation uses
an open-ended vocabulary of commands that FLM does not model, so `\alpha`, `\int` and
`\begin{aligned}…\end{aligned}` need no declaration. Text mode stays strict: the same
`\alpha` written outside mathematics is an unknown macro and an error. A command of another
feature written inside a formula, such as `\section` or `\href`, is an unknown command of
mathematics too: every other standard feature declares its own commands for text only (a
user definition applies in the modes it is defined for). Nothing rewrites the content of
mathematics: no typographic substitution, no paragraph, no collapsed whitespace. It reaches
the output exactly as written, except for the argument of a text command, described in the
next section.

The `math-mathjax` extension is a processing feature that reads mathematics
strictly: a formula may hold only the commands and environments of MathJax's TeX
packages, and any other command is an error where it is written (see [mathematics
processed by MathJax](crate::guide::command_line#mathematics-processed-by-mathjax)).

### Text inside mathematics

A word of ordinary text inside a formula is written with a **text command**:

```text
\( x \ge 0 \text{ for every } x \in S \), and \(y = \textbf{2}\).
```

The `math` feature declares these commands **for mathematics mode only**:
`\text`, `\textnormal`, `\textup`, `\textrm`, `\textmd`, `\textbf`, `\textit`,
`\textsl`, `\textsc`, `\textsf`, `\texttt` and `\mbox`
([`COMMANDS`](crate::stdcore::features::math::text::COMMANDS)). Outside
mathematics, `\textbf`, `\textit` and `\texttt` are the [inline formatting
commands](#inline-formatting), and `\text`, `\mbox` and the others are unknown
commands.

The argument of a text command is **read in text mode**, so it holds ordinary
FLM content rather than notation: emphasis, a reference, the use of a user
definition, and mathematics again.

```text
\(x = \text{the value of \ref{sec:intro}, or \(y\)}\)
```

Everything that applies to text elsewhere applies inside the argument.
Whitespace is collapsed and the typographic substitutions are applied, so
`\text{a --- b}` writes an em dash; a blank line in the argument is reported, as
in any argument that holds inline content; and an unknown command in it is an
error, because text mode is strict: `\alpha` is notation outside the argument
and a mistake inside it. The mathematics around the argument is not changed.

The library includes no renderer for mathematics. The capability
[`flm.math.rendered`](crate::stdcore::features::math::RENDERED) is declared with
no stage providing it, for an extension that renders mathematics (a
*capability* is a registered name for a state of processing that a stage brings
about; see [`CapabilityDef`](crate::core::registry::CapabilityDef)). `math-passthrough`
does not check whether the notation means anything either: its check stage
([`flm.math-passthrough.check`](crate::math_passthrough::CHECK))
accepts every formula and reports only an equation line that holds no mathematics
(see below). Under `math-passthrough`, the text backend prints the source, display
mathematics as an indented block of its own:

```text
Inline math \(x_1 + \alpha\) and display math
  \[ \int_0^1 f(x)\,dx = 1 \]
in the same paragraph.

  \begin{align*}
    a &= b \\
    c &= d
  \end{align*}
```

The unstarred environments number their lines: each line takes a number unless
the line says otherwise, and the lines of the starred forms never do
([`MathEnvironment::numbered`](crate::stdcore::features::math::MathEnvironment)).
Equations are numbered unless the registry's configuration says otherwise
([`MathConfig`](crate::stdcore::features::math::MathConfig)); what they are
numbered within is set by the document's numbering options (see [the numbering
guide](crate::guide::numbering)).

### `\ensuremath`

`\ensuremath{…}` writes mathematics wherever it stands, as in LaTeX. The `math`
feature declares it in both modes
([`ensuremath`](crate::stdcore::features::math::ensuremath)):

- **In text**, it is an inline formula: `\ensuremath{x^2}` is read as `\(x^2\)` is,
  its content in mathematics mode, and every math processor and every output
  treats it as that formula (it takes no number; a text command inside it is
  document content). The braces are required: `\ensuremath x` is a missing
  argument.
- **Inside mathematics**, it stands for its argument, as if the argument were
  written in its place: `\(a + \ensuremath{b}\)` is `\(a + b\)`. The formula's
  source that reaches MathJax holds the argument without the command.

The recomposed LaTeX writes `\ensuremath{…}` as the author wrote it, in text and
inside a formula alike, since LaTeX defines it in both.

```text
The value \ensuremath{\alpha + 1} in text, and \(x = \ensuremath{\alpha}\) in a formula.
```

A user definition whose template writes `\ensuremath` therefore produces
mathematics whether it is used in text or inside a formula.

### Inside a display-math environment

Four commands are available inside the body of the six environments, and only
there: the `math` feature declares them in a scoped package that the
environments' bodies make available
([`math::lines`](crate::stdcore::features::math::lines)). Written in `\(…\)`, in
`\[…\]` or inside a nested environment such as `cases`, the same names are
unknown commands of mathematics, kept as written like any other under
`math-passthrough`; written in text, they are unknown macros and errors.

| written | what it is |
|---|---|
| `\\`, `\\[1ex]` | the end of an *equation line*; the optional argument is vertical spacing, kept exactly as written, and must follow the `\\` directly: in `a \\ [b]`, the `[b]` starts the next line |
| `\label{eq:name}` | a label on the line, anywhere in the line; may be repeated |
| `\tag{A}`, `\tag*{A}` | what the line is called, in place of a number: `\tag{A}` shows `(A)`, `\tag*{A}` shows `A`. The text is read in text mode, so it is ordinary document content: a command written there is a command, and a reference to the line shows the tag |
| `\nonumber` | the line takes no number |

All four are *fields*: data the environment reads, not content where they are
written. Their position in a line makes no difference to what they record:
`a \label{eq:x} &= b` and `a &= b \label{eq:x}` record the same label on the same
line.

`\notag`, the second spelling that LaTeX's `amsmath` package gives the no-number
mark, is not one of these commands: written inside a body under `math-passthrough`,
it is an unknown command of mathematics, kept as written in its line.

```text
\begin{align}
  a &= b
  \label{eq:first}\\
  c &= d \tag*{C} \\[1ex]
  e &= f \nonumber
\end{align}
```

These commands record the structure of the environment. Its body is divided
into *equation lines* at each `\\`: an equation line is one line of a
display-math environment as its body reads between two `\\`, with its
mathematics, its labels, its tag and its no-number mark (see [equation
lines](crate::guide::concepts_overview#equation-lines);
[`equation_lines`](crate::stdcore::features::math::lines::equation_lines) returns
one [`EquationLine`](crate::stdcore::features::math::lines::EquationLine) per
line). A `\label` on a line of its own belongs to the line that the following
`\\` ends: `eq:first` above labels the line `a &= b`. A `\\` at the very end of a
body, followed only by whitespace and comments, ends the last line and opens no
further line. It is removed rather than recorded on the last line, so that an
output that writes the lines back never ends the environment with a `\\`, which
would add an empty line's worth of space in LaTeX. The whitespace and comments
after that final `\\` stay in the last line's content exactly as written. A
command or mathematics written after such a `\\` opens a further line instead.

Two forms of a body are errors. A body is either accepted or an error; nothing
here is a Warning.

- **An empty line** (`flm.math-passthrough.empty-line`): a line that holds no
  mathematics. Whitespace, blank lines and comments are not content, and neither
  are `\label`, `\tag` and `\nonumber`, so a line holding only one of them is
  empty too. A `\\` at the start of the body, `\\\\` in the middle, a `\label`
  after the last `\\`, a body holding only `\label{eq:x}` and an entirely empty
  body are all reported, each with the index of the empty line, counted from
  zero. The processing feature reports it, when it checks the formulas, since
  only it reads what a line holds: this is the check of `math-passthrough`
  ([`EmptyLine`](crate::math_passthrough::check::EmptyLine)),
  and `math-mathjax` reports the same condition as `math-mathjax.empty-line`.
- **A label or a tag on a line without a number**
  (`flm.math.label-without-number`): a `\label` or a `\tag` on a line marked
  `\nonumber`, or anywhere in a starred environment (`align*`, `gather*`,
  `equation*`). Reported once for each such command, when the body is parsed.

After either error the tree keeps the body as written: all its nodes are there,
and `equation_lines` still returns every line, an empty line as a line with no
content.

References to a line and the numbering of equations are built on this structure.
Every line is a reference point of its own, so `\ref{eq:first}` resolves to the
line `a &= b`, not to the environment or to its first line.

A line that **can be numbered** carries the labels written on it. A line that
can carry **no number** — one marked `\nonumber`, and every line of `equation*`,
`align*` or `gather*` — is still a reference point, so that every line is
treated alike and a tool that walks a rendered document can walk the lines; but
it keeps no label and accepts no label type, because a reference to it would
have no number to show. Writing a `\label` or a `\tag` there is the error above,
and a `\ref` to that label resolves to nothing, like a `\ref` to any label no
construct kept.

**Every line has an anchor**, labeled or not, numbered or not, so that a link
from another page, from a table of contents or from a script can reach any line
of any document. A line labeled `\label{eq:euler}` has the anchor `eq-euler`.
Every other line has `eq`, a double hyphen, its position among the document's
equation lines counted from one, and four lowercase letters: `eq--8kqzt`. The
letters are computed from the prefix and the position and from nothing else, so
the same position gives the same anchor in every document and on every run. They
are there so that the anchor does not read as a place in the document's own
numbering, since `eq--8` is not equation (8) as soon as one unnumbered equation
comes before it.

A line that can be numbered states what it is numbered by: the counter
`flm.math.equation` and the counter style its number is shown in, both declared
by the `math` feature. A numbered line shows its number where a `\tag` written by
the author would be (the text backend writes it as `\tag*{(1)}`), and a
reference to it shows that number: `\ref{eq:euler}` shows `Eq. (3)` (with a
no-break space) and `\eqref{eq:euler}` shows `(3)`. A line whose author wrote a
`\tag` takes no number and is not counted, and a reference to it shows the tag's
content.

`\eqref{eq:euler}` is the mathematics feature's own reference command
([`math::eqref`](crate::stdcore::features::math::eqref)). It refers to one
target, whose label type must be `eq`, and shows it as `\ref[noprefix]` shows
it: the number alone, in the parentheses of the equation's counter style and
without the word before them. A target of any other type is the Error
`flm.math.eqref-not-an-equation` at the command, and the label is then shown as
it was written.

## Lists

A list is an environment whose body is a sequence of items, each begun by
`\item`. The [`lists`](crate::stdcore::features::lists) feature declares three
*list kinds* by default: `itemize`, whose items are bulleted, `enumerate`, whose
items are numbered, and `description`, whose items are labeled by the term each
one writes. A list's list kind is its environment's name; a program may declare other
list kinds ([`ListsConfig`](crate::stdcore::features::lists::ListsConfig)).

```text
\begin{itemize}
\item Bread.
\item Butter.
\end{itemize}

\begin{enumerate}
\item Mix.
\item Bake.
\end{enumerate}

\begin{description}
\item[Entropy] A measure of disorder.
\item[Energy] What is conserved.
\end{description}
```

```text
• Bread.

• Butter.

1. Mix.

2. Bake.

Entropy A measure of disorder.

Energy  What is conserved.
```

`\item` is available inside a list's body and nowhere else (it belongs to a
scoped package that every list body makes available). Everything after an
`\item`, up to the next `\item` or the end of the list, is the item: paragraphs,
formulas and other lists included. An item may be empty. Only whitespace and
comments may be written before the first `\item`; anything else there is the
Error `flm.lists.content-before-item` and is shown as an item without a label.

### Nesting and the label beside an item

A list written inside an item of another list of the same list kind is *nested*, and
each depth has a label of its own: `•`, then `-`, then `▸` for `itemize`; `1.`,
then `(i)`, then `a-` for `enumerate`; the last label is repeated for every
deeper list. A nested list numbers its own items from one, and the list around
it continues its count after it. Only lists of the same list kind count: an
`enumerate` inside an `itemize` inside an `enumerate` is at the second depth and
reads `(i)`, as in LaTeX.

A list may write its own label template in its optional argument, `label=`, in
the language of a counter style's number template (see [the template language
shared by
features](crate::guide::concepts_overview#the-template-language-shared-by-features)):
every `#{name}` is the item's number in the format `name` (`arabic`, `alph`,
`Alph`, `roman` or `Roman`), and everything else is written as it is.

```text
\begin{enumerate}[label=(#{alph})]   %% (a), (b), (c)
\begin{enumerate}[label=Step #{arabic}:]
\begin{itemize}[label={--}]           %% a dash beside every item
```

A reference to a numbered item writes its number through a second template, the
**reference template**, which leaves out what suits only the label: the items of
an `enumerate` are labeled `1.`, `2.`, while `\ref` reads `Item 2`, so that a
sentence ending on a reference ends on one full stop. Each depth of a list kind
has one; for `enumerate` they are `#{arabic}`, `(#{roman})` and `#{alph}`. A
list may write its own with the key `ref=`, named after the key of LaTeX's
`enumitem` package that does the same; `label=` written alone sets both to the
same text:

```text
\begin{enumerate}[label=#{arabic}:, ref=#{arabic}]   %% 1:, 2:; \ref reads Item 2
\begin{enumerate}[label=(#{alph})]                   %% (a), (b); \ref reads Item (b)
\begin{enumerate}[ref=(#{alph})]                     %% 1., 2.; \ref reads Item (b)
```

The simple cases have a **short form**, the syntax of LaTeX's `enumerate`
package: a bracket made only of the characters `[]().:-+*aAiI1` and holding no
`=`. One of the characters `a`, `A`, `i`, `I`, `1` stands for the number, in the
format it shows: `[(i)]` is `label=(#{roman})`, `[1.]` is `label=#{arabic}.`,
`[a-]` is `label=#{alph}-`. Its reference template is the same text with every
full stop, colon and hyphen at the end removed: `[1.:]` labels the items `1.:`,
`2.:` and refers to them as `Item 1`, `Item 2`; `[a-]` refers to them as
`Item a`; `[(i)]` as `Item (i)`. With none of the five characters, the bracket
is a fixed label, such as `[-]`, `[*]` or `[+]`, and a reference shows that
label. Anything else, a Unicode bullet for example, is written with
`label={…}`, and so is a `]`, protected by the braces. A short form holding two
of the five characters is the Error `flm.lists.bad-label-form`, since it could
be read two ways (`[i.a]`); any key other than `label` and `ref` is the Error
`flm.lists.unknown-key`.

Whether a list's items are numbered is decided by its list kind, never by its
label: `\begin{enumerate}[label={*}]` shows a star beside every item, and every
item still has its number, which a reference to it shows through the same
template, since `label=` sets both. An empty `label={}` shows no label at all. A
label that writes a number on a list whose items are not numbered, such as
`\begin{itemize}[(i)]`, is the Error `flm.lists.label-names-number`, and the
depth's label is used. A `ref=` on such a list is accepted and ignored with the
Note `flm.lists.ref-ignored`, regardless of what it holds, so that a list can
be switched between `enumerate` and `itemize` without its bracket being
rewritten.

### Tags

`\item[My tag]` writes the item's own label, its **tag**, in place of the
list's: any content, in a numbered list as in an unnumbered one. The bracket is
also a key-value argument whose one key is `tag`, so `\item[tag={a=b}]` writes a
tag holding an `=`, which the bracket alone cannot. A tagged item of a numbered
list is not counted, as in LaTeX:

```text
\begin{enumerate}
\item[Custom] one
\item two
\item three
\end{enumerate}
```

reads `Custom`, `1.`, `2.`. Every item of a `description` needs a tag, since the
list writes no label of its own; an item without one is the Error
`flm.lists.missing-tag` and is shown without a label.

### Labels and references to items

`\label{item:…}` written directly after an `\item` and its bracket labels the
item; the label type is `item`, and no other type is accepted there
([`LABEL_TYPES`](crate::stdcore::features::lists::LABEL_TYPES)). A reference to a
numbered item shows the word `Item` and the item's number through the reference
template: `\ref{item:x}` reads `Item 2` for the second item of an outermost
`enumerate`, `Item (ii)` for the second item of a list at the second depth, and
`Item (b)` under `label=(#{alph})`; `\ref[noprefix]{item:x}` reads `(ii)`; a
reference to several items reads `Items 1,3`, or `Items 1–3` for a run. A
reference to any other item shows the label beside the item: a tagged item's
tag; for an untagged item of an unnumbered list, the list's own label where the
list writes one that can be used (`\begin{itemize}[label={--}]`, `[label=*]` or
`[-]`), and otherwise its list kind's bullet, so `\ref{item:x}` on an item of an
`itemize` reads `•`. Braces around a value only delimit it: `\item[tag=Custom]`,
`\item[tag={Custom}]` and `\item[Custom]` are one tag, shown alike beside the
item and by a reference to it.

```text
\begin{enumerate}
\item\label{item:mix} Mix.
\item Bake until golden; if in doubt, repeat \ref{item:mix}
\end{enumerate}
```

In a program whose registry holds no numbering feature, nothing numbers items: a
numbered list shows no labels and is reported as the Error
`flm.lists.no-numbering`, while bullets, fixed labels and tags are still shown.

The mistakes of a list described in this chapter are reported when a backend
converts the list into its output format (the *lowering*; see [backends,
lowering and outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)),
so the [`check`](crate::stdcore::CHECK_WORKFLOW) workflow, which converts
nothing into an output, does not report them.

## Tables of cells

A *table of cells* is a `cells` environment: a grid of rows and columns, filled
one cell at a time with `\cell` or by rows with `\celldata`, declared by the
[`cells`](crate::stdcore::features::cells) feature. By default the environment is
available inside a `table` float and nowhere else (see [floats](#floats)); a
program that sets the feature's switch
[`global`](crate::stdcore::features::cells::CellsConfig::global) makes it an ordinary
environment, written wherever blocks are written. Anywhere else, `\begin{cells}`
is an unknown environment.

```text
\begin{table}
\begin{cells}
  \celldata<H>{Name & City}
  \celldata{John & Berlin \\ Anna & Zurich}
\end{cells}
\caption{Who lives where}
\label{table:people}
\end{table}
```

```text
Name  City
----  ------
John  Berlin
Anna  Zurich

Table 1: Who lives where
```

The body of the environment is a sequence of instructions; blank lines and
comments between them have no effect:

- `\cell<styles>[placement]{content}` adds one cell. Its content holds blocks:
  paragraphs, a list, a displayed formula, a footnote, another table of cells.
  Unless a placement says otherwise, the cell goes in the next column of the
  current row.
- `\celldata<styles>[mapping]{data}` adds one cell per entry of its data,
  written as in LaTeX's `tabular`: `\\` ends a row and `&` separates the entries
  of a row; a `\\` at the end of the data ends the last row and adds no further
  row. Each entry is inline content, one paragraph, without the whitespace
  around it; a blank line inside the data is an error. An entry whose only
  content is a `\cell` is that cell, its own styles and placement taking
  precedence; an empty entry adds no cell and leaves its position empty.
- `\\` ends the current row: the next cell goes at the start of the next row.
- `\rownames{…}` and `\colnames{…}` name the rows and the columns: the *k*-th
  entry of the comma-separated list is the name of row or column *k*, and an
  empty entry gives no name. A name begins with an ASCII letter and holds ASCII letters,
  digits and underscores; it may be written wherever a row or column number may, anywhere in
  the table.

These commands are available in the body and, through it, in the data of
`\celldata`; `&` only in that data, and `\merge` only in a placement or a
mapping.

**Placements.** Rows and columns are numbered from 1. The optional `[…]` of
`\cell` is `[COL]` or `[ROW;COL]`, each a number, a name, `.` or nothing for the
current row or column, or `\merge{RANGE}` for a cell spanning several:
`\cell<H l>[1;\merge{3-4}]{Hi!}` is a left-aligned header cell in row 1 spanning
columns 3 and 4. A range is `2-4` (2 to 4), `2+3` (three from 2, so 2 to 4) or a
comma-separated union such as `1,2,3`, which must be contiguous. In a `\merge`, a
start left out is the first row or column, and an end left out is the last row
or column of the table as finally built.

**Mappings.** The optional `[…]` of `\celldata` is `[COLS]` or `[ROWS;COLS]`,
each a comma-separated list of numbers, ranges and `\merge{…}`s assigned to the
data's entries in order; without `ROWS`, the data's rows go from the current row
on:

- `\celldata[3,2,1]{A & B & C}` places `A` in column 3, `B` in column 2 and `C`
  in column 1;
- `\celldata[2-5]{A & B & C & D \\ E & F & G & H}` places the two rows in
  columns 2 to 5, and `[2-]` continues one column per entry from column 2;
- `\celldata[\merge{1,2},\merge{+2};4,6]{A & AA \\ B & BB}` places `A` and `AA`
  in cells spanning rows 1 and 2, in columns 4 and 6, and `B` and `BB` in cells
  spanning rows 3 and 4.

**Style tokens.** The angle brackets hold *style tokens* separated by
whitespace. A token is a name, or a name with a value in braces, and each states
a fact about the cell
([`CellStyle`](crate::stdcore::features::cells::CellStyle)):

| token | what it states about the cell |
|---|---|
| `l`, `c`, `r`, or `align{left}`, `align{center}`, `align{right}` | its alignment |
| `H`, or `head{column}` | it heads its column |
| `rH`, or `head{row}` | it heads its row |
| `lvert`, `rvert`, or `rule{left}`, `rule{right}`, `rule{above}`, `rule{below}` | a rule at that edge |
| `green`, `blue`, `yellow`, `red` | a custom style, which the standard HTML stylesheet and the recomposed LaTeX show as a background color |

In `\celldata`, commas separate the token lists of the entries of a row, the
last list being repeated: `\celldata<H l, H c>{One & Two & Three}` makes three
header cells, the first left-aligned and the others centered. A `\cell`'s own
tokens come before those of its `\celldata`. Of the tokens giving an alignment,
the first one decides, and likewise for a header; every rule and every custom
style is kept. A program may declare tokens of its own (see [the extension
guide](crate::guide::extending)); a token nothing declares is an error.

**Mistakes.** Every mistake is an Error, and the table is still shown. Content
in the body outside a cell (`flm.cells.content-outside-cell`), and the later of
two cells covering one position (`flm.cells.overlap`), are left out, and for each
a *dropped mark* is placed immediately before the table: a mark that each output
shows where content was left out, `[dropped: …]` in plain text (see
[`lowering::error`](crate::stdcore::backends::lowering::error)). A placement or a
mapping that cannot be read is ignored (`flm.cells.bad-placement`), and a token
that is misspelled or given a value it does not take is ignored
(`flm.cells.unknown-style`, `flm.cells.bad-style-value`). A `\cell` or a
`\celldata` written where it has no table to belong to, such as inside a cell,
is reported (`flm.cells.outside-table`) and its content is shown where the
command is written; a `\\`, an `&`, a `\rownames` or a `\colnames` written there
is reported and left out, with a dropped mark where it was. An empty table is
reported (`flm.cells.empty`) and shows nothing. These mistakes are reported when
a backend converts the table into its output, so the `check` workflow does not
report them.

## Xnotes

An *xnote* ([`xnotes`](crate::stdcore::features::xnotes)) is a piece of text set
apart from the running text, marked where it belongs and shown elsewhere or in
place: a footnote, an endnote, a side note. An *xnote kind* is one way of being
an xnote; the program configures the xnote kinds when it builds its registry
([`XnotesConfig`](crate::stdcore::features::xnotes::XnotesConfig)), and the
default configuration declares one, `footnote`, written `\footnote`. A citation
is not an xnote (see [citations](#citations)).

```text
The claim holds in every case.\footnote{Except the empty one, which we exclude by
convention.} The next sentence goes on.

A note holding paragraphs.\footnote{The first paragraph of the note.

  The second paragraph, and a list:
  \begin{itemize}\item one \item two\end{itemize}}

A labeled note.\footnote[\label{fn:appendix}]{See the appendix.}

See \ref{fn:appendix}, or \ref[noprefix]{fn:appendix}.
```

- `\footnote[<labels>]{<content>}`: the optional argument holds the xnote's
  labels, `\label{fn:…}`, as many as needed, and nothing else; the mandatory
  argument is its content. `\label` is a command in that argument and nowhere
  else: a `\label` written after the xnote, `\footnote{…}\label{fn:x}`, or inside
  its content is an unknown command. Anything other than `\label`, whitespace
  and comments in the optional argument is the Error
  `flm.xnotes.bad-label-argument` and is ignored; the labels written beside it
  still label the xnote.
- The label type is the xnote kind's, `fn` for `footnote`; a label of another
  type labels nothing and is reported (`flm.refs.label-type-not-accepted`).
- The content is inline content when it is written as one run of text, and
  paragraphs when it holds a blank line or a block such as a list or a displayed
  formula. An xnote kind may be configured to take inline content only, where a
  blank line is an error, or paragraphs always. An xnote with no content is
  still written, and reported as the Warning `flm.xnotes.empty`.
- A space written before `\footnote` is not kept: the xnote mark (the mark in
  the running text that shows where the xnote belongs) is attached to the word
  before it.
- `\ref{fn:x}` to an xnote of a numbered xnote kind shows the xnote kind's word and the
  xnote's number, `Footnote 1`; `\ref[noprefix]{fn:x}` shows `1`; several
  targets read `Footnotes 1,2`. To an xnote of an unnumbered xnote kind, a reference
  shows the xnote's content where that content is inline, and the label
  otherwise. In a program whose registry holds no numbering feature, the xnotes
  of a numbered xnote kind have no number, a reference shows the label, and the Error
  `flm.xnotes.no-numbering` is reported once per xnote kind.
- An xnote may be written wherever inline content may: in a paragraph, a
  heading's title, a list item, a `\text{…}` inside a formula, a user
  definition's template. Directly inside a formula, `\footnote` is an unknown
  command of mathematics, kept as written. An xnote written inside another
  xnote, of its own xnote kind or of another,
  is an xnote like any other: numbered in document order after the xnote that
  holds it, listed in its own xnote kind's list, its mark placed inside the outer
  xnote's content.

A second xnote kind is configuration, not syntax: a program may add
`\endnote{…}`, numbered in a counter of its own, or an unnumbered
`\sidenote{…}` shown where it is written. An xnote kind is either *listed* or shown *in
place* ([`XnotePlacement`](crate::stdcore::features::xnotes::XnotePlacement)).
An xnote of a listed xnote kind prints its mark in the running text, and the
xnotes of that kind are shown after the content, in a list of their own under
the xnote kind's title (`Footnotes`), each beside its mark. An xnote of an xnote
kind shown in place is
printed in brackets where it is written, and is a block of its own where its
content holds paragraphs. On an HTML page the mark links to the xnote, and each
xnote of the list links back to its mark.

## Hyperlinks

Three commands link to an **address**, a place outside the document, written as
the reader's program should follow it; the
[`href`](crate::stdcore::features::href) feature declares them:

| written | links to | shows |
|---|---|---|
| `\href{https://example.com}{Click here}` | `https://example.com` | `Click here` |
| `\url{https://example.com/}` | `https://example.com/` | `example.com` |
| `\email{someone@example.com}` | `mailto:someone@example.com` | `someone@example.com` |

**The address is raw text.** Everything up to the `}` that balances the opening
brace is the address, character for character: `%` does not start a comment,
`#`, `~`, `_` and `\` are themselves, and braces inside it are counted and kept.
So `\url{https://example.com/a%20b#top}` links to exactly that address. The
library checks nothing about an address (no scheme, no form) and changes nothing
in it. An output that refuses some addresses reports it when it writes the link:
the HTML output refuses a scheme it does not accept, such as `javascript:`, and
keeps the link's text (`flm.html.refused-address`).

**What `\url` and `\email` show** is the address without a leading `http://`,
`https://` or `mailto:`, and without the `/`, `#` and `?` characters it ends
with: `\url{https://example.com/?#}` shows `example.com`. `\href` shows its
second argument, which is ordinary inline content. In plain text a link prints
the address after what it shows, `Click here <https://example.com>`, except for
a `\url` or an `\email`, which already show their address; the program rendering
the document switches these with
[`link_addresses`](crate::stdcore::backends::text::TextRenderOptions::link_addresses)
and
[`link_addresses_when_shown`](crate::stdcore::backends::text::TextRenderOptions::link_addresses_when_shown).

**In the template of a user definition**, a placeholder inside an address is
replaced by the characters of the use site, and a literal `#` is written `##`,
as everywhere in a template:

```text
A user definition \doi of one argument, whose template is
    \href{https://doi.org/#1}{#1}

\doi{10.1000/xyz}    is    \href{https://doi.org/10.1000/xyz}{10.1000/xyz}
```

What the use site passes must be characters once substituted: `\doi{\emph{x}}`
puts a command into the address, which is the Error
`flm.href.address-not-characters`, and the link is not made (its text is still
shown). An address holding `%` is passed to such a definition through an
argument the definition declares verbatim
([`ArgStdKind::Verbatim`](crate::lang::mint::ArgStdKind::Verbatim)), where `%`
does not start a comment either; every output then links to the use site's
characters.

## Citations

A **citation** refers to a cited work by its **citation key** and shows a
**citation mark** where it is written. The
[`citations`](crate::stdcore::features::citations) feature declares the
commands. Each cited work gets a number, in the order the document first cites
it, and the mark shows the numbers, each a link to the work.

```text
Codes of this family are well studied \cite{knuth84,lamport94}. The bound is
tight \cite[Thm 4]{knuth84}\cite{shor95}. As \R\cite{lamport94} shows, and as
\R[S]\cite{knuth84,shor95} confirm, the proof is short (see, for instance,
\R[I]\cite{shor95}).
```

reads, on the text output,

```text
Codes of this family are well studied [1,2]. The bound is tight [3][1; Thm 4].
As ref. [2] shows, and as Refs. [1,3] confirm, the proof is short (see, for
instance, [3]).
```

Here the space that a `\cite` writes before its mark, and the space after
`ref.` and `Refs.`, are no-break spaces; the space before the last `[3]` is the
one written before `\R[I]`.

- `\cite[<note>]{<keys>}`: the citation keys are separated by commas written
  outside every group, each without the whitespace around it; an empty key is
  skipped (`\cite{a,,b}` cites `a` and `b`), and a call holding no key at all
  (`\cite{}`) is the Error `flm.citations.no-key`. A **citation key** is the FLM code
  written for it, every run of whitespace counting as one space: `knuth84`,
  `arxiv:1234.5678`, `manual:{A. Author, Some title (2024)}`. A comma inside a
  group, inside a command's argument or inside a formula belongs to the key,
  and a key may hold a command. The library reads no meaning into a key:
  whether two keys, such as `arXiv:1211.1037` and `arxiv:1211.1037`, cite the
  same work is decided by the citation provider that supplies the citation
  entries (see *The citation entries and the list* below), and two keys of one
  work share one number and one entry. The optional **note** is text
  shown after the numbers, `[1; Thm 4]`; it belongs to every key of the call, so
  `\cite[Thm 4]{a,b}` reads `[1,2; Thm 4]`.
- **A citation chunk.** `\cite` calls written directly after each other, with
  only whitespace, a single line break or comments between them, are one
  **citation chunk**, shown as one mark. The keys without a note come first,
  their numbers sorted, three or more consecutive numbers written as a range;
  then one group per note, in the order the notes first appear, two notes
  written the same way (spaces aside) counting as one. With `a`, `b`, `c`, `d`,
  `e` cited in that order earlier,
  `\cite{c,b}\cite[Thm 4]{a}\cite{d}\cite[Thm 4]{e}` reads `[2–4][1,5; Thm 4]`.
  A work cited twice in a group (one key written twice, or two keys of one
  work) shows its number once. A paragraph break, or anything else written
  between two calls, makes two chunks.
- **The space before a `\cite`.** A `\cite` is attached to the text before it:
  it removes the space written before it and always writes its own, a no-break
  space before a bracketed mark and nothing before a raised one, so
  `text \cite{a}` and `text\cite{a}` read the same. Nothing about the text
  around it is examined: a `\cite` at the start of a paragraph, of a cell or of
  a line, or after an opening parenthesis, also writes its space. Where a
  citation is a word of its own sentence rather than attached to the text before
  it, write `\R[I]\cite{…}`: `(\R[I]\cite{a})` reads `([1])`.
- `\R[<flags>]\cite[<note>]{<keys>}`: the citation written as a noun, with its
  word before it: `\R\cite{a}` reads `ref. [1]`, `\R\cite{a,b}` reads
  `refs. [1,2]`, the plural chosen by the number of works the mark shows. The
  flags are `S` for the capitalized word, `Ref.` and `Refs.`, at the start of a
  sentence, and `I` for no word at all, the mark alone. `\R` writes no space
  before itself or its mark and removes none: the space written before it is
  kept as before any word. It must be followed directly by `\cite`, which may
  be followed by further calls (`\R\cite{a}\cite{b}` reads `refs. [1,2]`).
  `\R{\cite{a}}` and `\R text` hold no citation, which is the Error
  `flm.citations.no-citation`; an unknown flag is the Error
  `flm.citations.unknown-flag`.
- **Two forms of the mark.** The program chooses how the numbers look when the
  citation entries are listed ([`MarkForm`](crate::stdcore::features::citations::MarkForm),
  the field `mark` of
  [`CitationsConfig`](crate::stdcore::features::citations::CitationsConfig)):
  *bracketed*, the default, with the numbers inline between the delimiters of
  the counter style described at the end of this item, or *superscript*, with
  the numbers raised, without delimiters, and without a space before a `\cite`'s mark (`text¹,²`). A citation written with
  `\R` is never raised: under the superscript form its numbers are written
  without delimiters, `refs. 1,2`, and the list labels its entries `1`, `2`. The
  delimiters, the words, the texts joining the numbers and whether consecutive
  numbers make a range are set by a counter style, the field `style` of
  [`CitationsConfig`](crate::stdcore::features::citations::CitationsConfig) (see
  [counters and counter
  styles](crate::guide::concepts_overview#counters-and-counter-styles)).

The citation keys and the flags of `\R` are not words of the text, and a note
is: a heading written `\section{Results \cite[Thm 4]{a}}` without a label gets
its anchor from the words `Results Thm 4`. A citation shown again away from
where it is written, such as in a heading's title shown by a reference, shows
the same numbers.

**The citation entries and the list.** The program that processes the document
gives each cited work its **citation entry**, what is shown for it, such as
`Knuth, The Art of Computer Programming, 1984`, through sources of entries it
sets up (citation providers,
[`CitationProvider`](crate::stdcore::features::citations::CitationProvider)); the
document itself only writes keys. After the content, under the heading
`References`, the cited works are listed in the order of their numbers, each
labeled with its number (`[1]`), after the list of footnotes where there is
one:

```text
References
----------

[1] Knuth, The Art of Computer Programming, 1984

[2] Lamport, 1994
```

A cited key without an entry is the Error `flm.citations.no-entry`: it keeps its
number and its marks, and its line in the list shows that the entry is missing
(`[2] [dropped: bibliography entry]` on the text output). An entry may not
itself cite a work, directly or through a user definition, and may not hold a
paragraph break or a block such as a list or a heading; such an entry is an
error (`flm.citations.entry-cites`, `flm.citations.entry-not-inline`), and its
key is then without an entry.

**Two placements.** The program chooses where the entries are shown
([`CitationPlacement`](crate::stdcore::features::citations::CitationPlacement),
the field `placement` of `CitationsConfig`): *listed*, the default, as above;
or *in place*, where a mark shows the entries themselves, one bracketed group
per key in the order written, the note after its entry: `\cite{a}\cite[Thm
4]{b}` reads `[Knuth, 1984][Lamport, 1994; Thm 4]`. In place, nothing is linked
and no list is placed. `\R` writes its word before the entries, a `\cite` writes
its no-break space, and a key without an entry shows the missing entry inside
its brackets. The form of the mark has no effect under the in-place placement.

## Defined terms

A **defined term** is a word or a phrase that a document explains in a **term
definition**, the `defterm` environment; a **term link** links to that term
definition from anywhere in the document. The
[`defterm`](crate::stdcore::features::defterm) feature declares both.

```text
\begin{defterm}{Markov chain}\label{topic:markov}\aliasterm{Markov chains}
  A random process whose next state depends on its present state alone.
\end{defterm}

Every \term{Markov chain} forgets its past, as all \term{Markov chains} do;
\term[Markov chain]{such a process} does too. See \ref{topic:markov}.
```

prints

```text
Markov chain: A random process whose next state depends on its present state
alone.

Every Markov chain forgets its past, as all Markov chains do; such a process
does too. See Markov chain.
```

- `\begin{defterm}{<term>} … \end{defterm}` is a **term definition**: its
  argument is the term, inline content, which may hold markup
  (`\(\alpha\)-decay`, `\emph{Markov} chain`), and its body holds the
  definition's paragraphs. The definition opens with a **lead-in** set into its
  first paragraph, the term and a colon by default; the program may configure
  another lead-in, or none
  ([`DeftermConfig`](crate::stdcore::features::defterm::DeftermConfig)).
- After the term, in any order and each as often as needed: `\label{…}`, a label
  of the type `term` or `topic`, by which `\ref` and `\hyperref` refer to the
  definition (a reference to a definition shows its term); and
  `\aliasterm{…}`, an **alias**: another spelling of the term, such as a
  plural, under which a term link finds the same term definition.
- `\term[<ref>]{<shown>}` is a **term link**: a link to the term definition of the
  term `<ref>`, or of the term `<shown>` when the bracket is left out, showing
  the text `<shown>`.
- A term link finds a term definition by the term as written: markup counts, runs of
  spaces and line breaks count as one space, and case counts, so
  `\term{Entropy}` finds no term definition of `entropy`. Other forms of a term are
  found only where the definition declares them with `\aliasterm`.
- A term link that finds no term definition is a reference that resolves to nothing,
  reported as any reference is (`flm.refs.unresolved`) and shown as its text.
  Two term definitions of one term are reported as a label written twice, and
  term links find the first. A term definition or a term link whose term is empty is the
  Error `flm.defterm.empty-term`.
- Inside the term definition of its own term, a term link is not a link: it is shown
  as the term being defined.
- A term link may be written wherever inline content may, a heading's title
  included, where the bracket is not part of the title's words. A term
  definition is a block, written where blocks are written, and term definitions
  may hold one another.

In a program whose registry holds no references feature, a term definition is
written with its lead-in, and a term link is written as its text.

## Theorems

The **theorem environments** are the numbered, titled, labelable blocks of
mathematical writing: theorems, lemmas, definitions, proofs. The
[`theorems`](crate::stdcore::features::theorems) feature declares them.

```text
\begin{theorem}[My theorem]\label{thm:sqrt2}
  The square root of two is irrational.
\end{theorem}

\begin{proof}[*thm:sqrt2]
  We argue by contradiction.
\end{proof}

By \ref{thm:sqrt2}, no fraction squares to two.
```

prints

```text
Theorem 1 (My theorem). The square root of two is irrational.

Proof (Theorem 1). We argue by contradiction. □

By Theorem 1, no fraction squares to two.
```

- **The theorem kinds.** Each theorem kind is an environment of its own name. By
  default there are eight: `theorem`, `proposition`, `lemma`, `corollary` and
  `conjecture`, then `definition` and `remark`, all numbered in **one shared
  sequence** (a theorem, a lemma and a definition in a row are numbered 1, 2
  and 3), and `proof`, which is not numbered and ends with the closing mark `□`.
  The program may declare other theorem kinds (an `exercise`, for example) or
  another set of them
  ([`TheoremsConfig`](crate::stdcore::features::theorems::TheoremsConfig)); the
  three sets the library ships hold six, eight and thirteen theorem kinds
  ([`simpleset`](crate::stdcore::features::theorems::TheoremsConfig::simpleset),
  [`new`](crate::stdcore::features::theorems::TheoremsConfig::new),
  [`richset`](crate::stdcore::features::theorems::TheoremsConfig::richset); the
  largest adds `idea`, `question`, `claim`, `observation` and `problem`).
- **The heading.** Each theorem opens with a heading set into its first
  paragraph: the theorem kind's word and the theorem's number, then the title in
  parentheses where one is written (`Lemma 4 (Key lemma).`), or the word alone
  for a theorem kind that is not numbered. How the word, the number and the
  title are joined is the theorem heading template that the program configures
  for each *theorem class*, a family of theorem kinds written alike; a program
  can change it to write `Proof of Lemma 4`, for example.
- `\begin{<kind>}[<title>]`: the **title** is optional, ordinary inline
  content.
- After the title, in any order: `\label{…}`, repeatable, a label of one of the
  types `thm`, `prop`, `cor`, `lem`, `rem`, `def`, `dfn`, `x` or `topic`, by
  which `\ref` refers to the theorem; and `\noproofref`, which says that no proof
  of the statement follows. A statement of a theorem kind that expects a proof
  (`theorem`, `proposition`, `lemma` and `corollary` by default) that no proof
  refers back to (see *the back-reference* below) and that carries no
  `\noproofref` is reported as the Warning `flm.theorems.missing-proof`. A label
  on a theorem kind that is not numbered, a proof, is refused
  (`flm.refs.label-type-not-accepted`).
- **A reference** to a theorem shows its word and number: `\ref{lem:x}` reads
  `Lemma 4`, and `\ref{lem:x,lem:y}` reads `Lemmas 4 and 5`. The variants
  `\ref[lowercase]{lem:x}` (`lemma 4`), `\ref[abbreviated]{lem:x}` (`Lem. 4`)
  and `\ref[S]{lem:x}` (the capitalized word) choose the form of the word.
- **The back-reference.** A proof states which statement it proves by writing
  that statement's label after a star as its title: `\begin{proof}[*lem:x]`
  reads `Proof (Lemma 4).`, the reference being a link to the lemma;
  `[**lem:x]` records that the proof proves the lemma and shows `Proof.` alone.
  A label no construct declares is a reference that resolves to nothing
  (`flm.refs.unresolved`), shown as its text. A proof title that begins with a
  star but is not a star followed by a label written as plain characters, such
  as `[*\emph{x}]` or `[*nolabel]`, is the Error
  `flm.theorems.bad-relation-ref` and is shown as written. For the other theorem
  kinds, a star is part of the title.
- A theorem is a block, written where blocks are written; its body holds
  blocks.

The number follows the numbering options of the document, so a document that
numbers theorems within sections reads `Lemma 2.3` in the heading and in every
reference (see [the numbering guide](crate::guide::numbering)). In a program
whose registry holds no numbering feature, a theorem's heading shows its theorem
kind's word alone.

## Quotations

A **quotation** is a block set apart from the running text: a quoted passage, a
poem, an address. Its content is made of **quotation sections**, each written
with a section command. The [`quote`](crate::stdcore::features::quote) feature
declares the quote environments and their section commands.

```text
\begin{quote}
  \text{To be or not to be.}
  \attributed{Shakespeare}
\end{quote}

\begin{address}
123 Main St\\
City, ST 12345
\end{address}
```

prints

```text
To be or not to be.

Shakespeare

123 Main St
City, ST 12345
```

- **The quotation sections.** `\text{…}` is a quoted passage, one paragraph;
  `\lines{…}` holds lines set as written (below); `\attributed{…}` is the
  attribution, an author or a source; `\block{…}` holds ordinary paragraphs and
  blocks, such as a list or a displayed formula, or one paragraph where its
  content is written on one line. The quotation sections are shown in the order
  written; whitespace and comments between them have no effect.
- **The three environments.** `quote` accepts the four section commands and
  nothing else. `blockquote` accepts no section command: its whole content is
  one block section, its paragraphs as written. `address` accepts none either:
  its whole content is one lines section.
- **The lines of a lines section.** Inside `\lines{…}`, and in the body of
  `address`, each `\\` ends a line, and each `\indent` written at the start of a
  line indents that line by one tab, a width each output sets (four columns in
  plain text), and `\indent\indent` by two. The whitespace at
  both ends of a line is not part of it, and a line with nothing in it, between
  two `\\` or after the last one, is an empty line. `\\` and `\indent` are
  available only in a lines section. An indent of a written amount, such as
  `\indent[1.5em]`, is not available yet.
- **The section commands are available only in a quotation**, and only outside
  mathematics: `\text{…}` inside a formula is the mathematics feature's own (see
  [text inside mathematics](#text-inside-mathematics)). A section command that
  the environment does not accept is not a command in its body; and a section
  command opens a quotation section only where it is written directly in the
  body, so written inside another command's argument it is part of that
  content. One exception to the rule that a section command the environment
  does not accept is not a command in its body: a quotation nested in another's
  quotation section still has the enclosing quotation's section commands
  available, and one written directly in the nested body that the nested
  environment does not accept is part of that body's content outside the
  quotation sections.
- **What is reported.** Content outside the section commands of an environment
  that accepts only quotation sections, such as text written directly in
  `quote`, is the Error `flm.quote.bare-content`, reported once for each stretch
  of such content between two quotation sections (a paragraph and the list after
  it are one stretch), and shown where it is written. A section command inside
  another section's argument, `\lines{\text{x}}`, is the Error
  `flm.quote.nested-section` and is shown as its content. An `\indent` after the
  start of its line is the Error `flm.quote.indent-mid-line` and is left out.
- A quotation is a block, written where blocks are written; quotations nest, one
  quotation inside another's `\block{…}` section.

**Configuring an environment.** The program may declare other quote
environments
([`QuoteConfig`](crate::stdcore::features::quote::QuoteConfig)), each with the
section commands it accepts and what its content outside them is read as: a
passage, lines, an attribution, a block, or nothing, which makes such content an
error. Examples are an `epigraph` accepting `\text` and `\attributed` alone, or a
`verse` whose content is lines followed by an `\attributed{…}`. Content outside
the section commands is read stretch by stretch: each stretch between two
quotation sections is one quotation section of the type the environment reads it
as. A name the library already declares (`itemize`, `theorem`), or an
environment that accepts no section command and reads its content as nothing,
makes the registry build fail.

## Annotations

**Annotations** are a reviewer's marks in a document: a passage highlighted, a
remark written in the text. Each reviewer, an **annotator**, has a command of
their own, which exists only when the program configures that annotator, with
the annotator's initials if they have any; the
[`annotations`](crate::stdcore::features::annotations) feature declares the
commands
([`AnnotationsConfig`](crate::stdcore::features::annotations::AnnotationsConfig)).
No annotator is configured by default, and `\phf` is then an unknown command. In
this chapter, an annotator writes `\phf` and has the initials `PhF`.

| written | what it is |
|---|---|
| `\phf{a passage}` | a **highlight**: the passage is marked and stays part of the text |
| `\phf a passage\endphf` | a highlight too, ended by `\end` followed by the command's name |
| `\phf[a remark]` | a **reviewer's comment**: a remark set apart from the text |

```text
The result \phf{holds in every case}, as shown in
\phf[cite the lemma here] the appendix.
```

prints

```text
The result [PhF] holds in every case, as shown in [PhF: cite the lemma here] the
appendix.
```

- **One argument.** What follows the command decides its form: a `{` opens a
  highlight, a `[` a reviewer's comment, and anything else starts a passage that
  runs to `\endphf`. Spaces and `%%` source comments may be written before the
  `{` or the `[`. `\phf{x}[y]` is a highlight of `x` followed by the text `[y]`.
  The `\phf … \endphf` form needs no braces to balance, which suits a long
  passage; a `\endphf` counts only at the passage's own level, not inside a
  group or an environment the passage opened. A passage that the document, its
  enclosing group or its enclosing environment ends before `\endphf` is the
  Error `flm.annotations.unterminated` and is highlighted up to that end; the
  group or the environment stays whole.
- **Inline content or paragraphs.** Every form holds inline content or
  paragraphs, as written. An annotation is block-transparent and never makes a
  block of its own: a blank line inside it is a paragraph break of the text
  around it, and the mark continues in the next paragraph, so

  ```text
  Some text \phf[A comment.

  With multiple paragraphs.] More text
  ```

  prints

  ```text
  Some text [PhF: A comment.

  With multiple paragraphs.] More text
  ```

  A list or a displayed formula inside an annotation is a block of the text
  around it, still marked. The initials are shown once, beside the line where
  the annotation starts.
- **A reviewer's comment is not part of the text.** A heading whose title holds
  a reviewer's comment gets its anchor from the title's other words; a
  highlight's passage counts as words of the title.
- **Hiding.** The program rendering the document can leave every annotation out,
  for a clean copy
  ([`AnnotationsRenderOptions::hidden`](crate::stdcore::features::annotations::AnnotationsRenderOptions::hidden)):
  each highlight is then shown as its passage alone and each reviewer's comment
  not at all, with no change to the source. A reviewer's comment written across
  a paragraph break leaves that break in the clean copy: the text around it
  stays two paragraphs.
- Annotations are written in text, and inside `\text{…}` in a formula; an
  annotation written directly inside a formula is not available yet.

## Graphics

`\includegraphics[options]{path}` shows an **image**: a picture file referred to
by its **path**, as written (`figures/plot.png`, `https://example.org/a.svg`).
The [`graphics`](crate::stdcore::features::graphics) feature declares the
command. The library never opens the file: the output refers to it, and what is
known about the file, such as its size and its resolution, comes from the
program rendering the document, where that program supplies the information
([`GraphicsInspector`](crate::stdcore::features::graphics::GraphicsInspector)).

```text
\includegraphics[alt=A plot of the data, width=5cm]{figures/plot.png}
```

prints

```text
[image: A plot of the data]
```

**Where the command may be written.** By default the command is available only
inside the body of a float (a `figure`), and is an unknown command anywhere
else. The program can make it an ordinary command
([`GraphicsConfig::global`](crate::stdcore::features::graphics::GraphicsConfig::global)),
written wherever text may be written: in a sentence, in a paragraph of its own,
in a heading's title, in a caption, in a list item, in a footnote.

**Block or inline.** An image is inline content, part of the text around it
like a word, everywhere except as the direct content of a float's body, where it
is a block: the float's content. So
`See \includegraphics{icon.png} here.` is one sentence holding an image, an image
written in a paragraph of its own is a paragraph holding an image, and the image
of a `figure` is a block on its own, as the figure's content, above the line
under it. A picture set apart from the text is written as a float, with
`\nonumber` where no number is wanted and without `\caption` where no line under
it is wanted.

**The options** are key-value pairs, written as a heading's are:

| key | value |
|---|---|
| `alt` | the **alternative text**, what a reader who cannot see the picture is told; plain text, without formulas or commands |
| `width` | the width the image is shown at |
| `height` | the height the image is shown at |

A width or a height is a **length**: a number followed directly by a unit,
without a space: `5cm`, `2.5in`, `30mm`, `144pt`. The units are `pt` (a point,
1/72 inch: the point of CSS and PostScript, which LaTeX calls a big point,
`bp`), `mm`, `cm` and `in`; a length must be greater than zero. `em`, `tab`
(a width each output sets), `px`, `\linewidth`, a percentage and `bp` are not
units here.

- **One dimension given, the other keeps the proportions.** With `width=` alone,
  the height is the picture's own height scaled by the same factor, in the unit
  of the width (`width=5cm` on a picture of 340pt × 170pt shows it 5cm × 2.5cm),
  and likewise with `height=` alone; this needs the picture's size, which the
  program rendering the document supplies. Without it, the given dimension is
  written alone, and each output keeps the proportions its own way. With both
  options, the picture is shown at exactly that size. With neither, it is shown
  at its own size. Where its size is known neither from the options nor from the
  picture, the Warning `flm.graphics.unknown-size` is reported, and each output
  shows the picture at whatever size it has there.
- **Each output may magnify pictures.** The program rendering the document may
  set, per output, a factor for raster pictures (PNG, JPEG, …) and one for
  drawings (SVG, PDF), which multiply the size decided above, including a
  `width=` or `height=` the author gave.
- **Each `width=` or `height=` that takes effect is reported as the Warning**
  `flm.graphics.resized`: an image file prepared at the size it is shown at, its
  resolution suited to that size, looks better than one an output resizes.
- **A mistake is an Error, and the image is still shown**, the option being
  ignored: a key other than the three (`flm.graphics.unknown-key`), a key
  written twice, the first being kept (`flm.graphics.repeated-key`), a key
  without a value (`flm.graphics.missing-value`), a value without a key such as
  `=5cm` (`flm.graphics.missing-key`), an `alt` holding a formula or a command
  (`flm.graphics.alt-not-text`), a length that is not one of the above
  (`flm.graphics.not-a-length`), a length in `em` or `tab`
  (`flm.graphics.relative-unit`), and a length that is zero or negative
  (`flm.graphics.not-positive`).
- **The path is raw text** up to the balancing `}`: `%`, `#`, `~`, `\` and nested
  braces are characters of the path, taken as written. In the template of a
  [user definition](#author-definitions), a placeholder in a path is replaced by
  the characters of the use site (`\includegraphics{figs/#1.png}`); a command
  there instead is the Error `flm.graphics.path-not-characters`, and no image is
  shown. An empty path (`\includegraphics{}`) is the Error
  `flm.graphics.empty-path`, and no image is shown.
- **The options and the path are not part of the text.** A heading whose title
  holds an image gets its anchor from the title's other words; the `alt` text is
  left out with the rest of the options.

## Floats

A **float** is a block of content such as a figure or a table, with an optional
caption and labels, numbered in its float type's sequence. The name is LaTeX's: on
the plain-text and HTML outputs a float is shown where it is written and never
moved, and the recomposed LaTeX writes a numbered float as a LaTeX float
environment, which LaTeX places. The
[`floats`](crate::stdcore::features::floats) feature declares the float types.

```text
\begin{figure}
  \includegraphics{plot.png}
  \caption[alt=A plot]{A plot of the data, before and after.}
  \label{figure:plot}
\end{figure}

See \ref{figure:plot}.
```

prints

```text
[image: plot.png]

Figure 1: A plot of the data, before and after.

See Fig. 1.
```

- **The float types.** Each float type is an environment of its own name, taking
  no argument, whose body holds paragraphs and blocks: `figure` and `table` by
  default, and the float types the program declares. What a float's body may
  hold is set by its float type: `figure` holds exactly one image, written with
  `\includegraphics`; `table` holds exactly one [table of
  cells](#tables-of-cells) or one image. A float type may also accept any
  content. Content the float type does not accept is the Error
  `flm.floats.content-refused`, whose message says what the type can hold there,
  or that nothing it may hold is available, and the content is shown as written.
- **The fields.** Three commands are available in a float's body and nowhere
  else, and may be written anywhere in it, in any order: `\caption{…}`, the
  caption, inline content; `\label{type:label}`, a label, whose type is by
  default the float type's own name (`\label{figure:plot}`,
  `\label{table:sizes}`); and `\nonumber`, which asks for no number. A second
  `\caption` is the Error `flm.floats.caption-repeated` and is not shown, the
  first being the float's caption. A field written inside something else in the
  body, such as inside a group or a list, is not the float's, and is reported
  (`flm.lowering.field-not-read`).
- **The caption's alternative text.** `\caption[alt=Short]{A long caption}` gives
  the caption an alternative text: what a reference to a float without a number
  shows in place of the full caption. The bracket holds key-values, and `alt` is
  its only key: any other key, a bare `\caption[Short]{…}` included, is the Error
  `flm.floats.caption-unknown-key` and is ignored, as are an `alt` without a
  value and a second `alt=`.
- **The numbering policy.** Each float type says when its floats are numbered.
  *Numbered when labeled*, the default of `figure` and `table`: a float is
  numbered when it carries a `\label` and no `\nonumber`. *Numbered*: every
  float is numbered unless it carries `\nonumber`, as a displayed equation is.
  *Unnumbered*: no float of the type is numbered. A `\nonumber` that changes
  nothing (in a float type never numbered, in a float numbered only when labeled
  that has no label, or written twice) is the Note
  `flm.floats.nonumber-without-effect`. The numbers of a float type form one
  sequence through the document: `Figure 1`, `Figure 2`, and `Table 1` beside
  them.
- **The line under the content.** A float that is numbered or captioned is shown
  with a line under its content: `Figure 1` for a numbered float without a
  caption, `Figure 1: ` followed by the caption, or the caption alone for a
  captioned float without a number. A float with neither shows its content
  alone. The line always follows the content, wherever the caption is written in
  the body.
- **References.** A reference can point at every float, numbered or not.
  `\ref{figure:plot}` shows `Fig. 1`, and `\ref[S]{figure:plot}` shows
  `Figure 1`, the float type's words before the number; a reference to a float
  without a number shows the caption's alternative text where one is written,
  and the caption otherwise. A reference to a float with neither a number nor a
  caption has nothing to show and is reported (`flm.refs.nothing-to-show`).
- A float is a block, written where blocks are written: between paragraphs, in a
  list item, in a footnote, in a quotation's `\block{…}` section. Written inside
  a paragraph's text, it is a block of its own between the two parts of the
  paragraph.

The float's body is read, and its mistakes reported, when a backend converts the
float into its output, so the [`check`](crate::stdcore::CHECK_WORKFLOW) workflow
does not report them.

**Configuring a float type.** The program may declare other float types
([`FloatsConfig`](crate::stdcore::features::floats::FloatsConfig)), for example a
`diagram` holding any content and numbered when labeled, each with its name, the
word written under its content (`Diagram`), its numbering policy, the words a
reference writes (`Diagram`, `Diagrams`), what its body may hold and the label
types it accepts. A name must consist of ASCII letters.

## Verbatim content

Inside verbatim content no character of the markup language has a special
meaning: a backslash, a brace and a percent sign are ordinary characters, braces
need not balance, and the content ends only at its own end delimiter (see
[verbatim regions](crate::guide::concepts_overview#verbatim-regions)). Five
constructs hold verbatim content, three macros and two environments, declared by
the [`verbatim`](crate::stdcore::features::verbatim) feature:

| written | verbatim kind | printed |
|---|---|---|
| `\verbtext+…+` | `text` | inline |
| `\verbcode[lang]+…+` | `code` | inline |
| `\verba[lang]+…+` | `a` (an identifier) | inline |
| `\begin{verbatimtext}…\end{verbatimtext}` | `text` | inline |
| `\begin{verbatimcode}[lang]…\end{verbatimcode}` | `code` | a block of its own |

For the three macros, the delimiters are matched automatically: the first
character after the command opens the content, and the content ends at the
partner of `{}`, `[]`, `<>` or `()`, or, for any other character, at that
character again. So `\verbcode+a{b+` and `\verbcode{a{b}c}` both read what is
written between their delimiters. For the two environments, the content is the
body, and a line break directly after `\begin{name}` is not part of it.

The optional `[lang]` argument states the programming language the content is
written in. Nothing in the standard package interprets it; a backend that
highlights syntax can use it. The text backend prints neither the language nor
the verbatim kind, which have no spelling in plain text.

```text
Inline \verbtext+a{b}\c+ text, the code \verbcode[python]+f(x) = 1+ and the
identifier \verba[rust]+struct+, then \begin{verbatimtext}raw {text}\end{verbatimtext}
in the same paragraph, and a code block:

\begin{verbatimcode}[python]
def f(x):
    return {x} + 1
\end{verbatimcode}
```

```text
Inline a{b}\c text, the code f(x) = 1 and the identifier struct, then raw {text}
in the same paragraph, and a code block:

def f(x):
    return {x} + 1
```

The content is printed byte for byte, and the block form keeps its own line
breaks and indentation. Verbatim content whose end delimiter never comes is
reported (`core.verbatim.unterminated-verbatim`), and what was read up to the
end of the input is kept.

## The document's title, authors and date

A document's title, its authors and its date are not written in its body: they
are the document's *metadata*, which the program sets on the document
([`DocMeta`](crate::stdcore::features::meta::DocMeta)) and which the
[`meta`](crate::stdcore::features::meta) feature shows before the content. The
`flm` program reads them from the root keys `title`, `author` and `date` of the
document's front matter: `title` and `date` one text each, `author` one text or
a list of texts, one per author (see the [command-line
guide](crate::guide::command_line#configuration-files-and-document-front-matter)).
Each is FLM text, not plain text, read as inline content: `The \emph{Art} of
\(x^2\)`. A problem in it is reported at its place in the file that holds it. On
the text output, the title, then the authors separated by commas, then the date
are printed before the content. A document without metadata shows nothing
there.

## Including files

`\input{file}` brings the content of another file into the document where the
command is written; the [`input`](crate::stdcore::features::input) feature
declares the command. The file's name is written as plain characters: a command,
a group or a comment inside the braces is reported
(`flm.input.reference-not-plain`). Finding and reading the file
is the work of the program, not of the language (see
[`lang::input`](crate::lang::input)). The `flm` program reads the name relative
to the folder of the file that holds the command: `\input{parts/intro.flm}` in
`docs/paper.flm` reads `docs/parts/intro.flm`, and a command in that file that writes
`method.flm` reads `docs/parts/method.flm`. Another program built on the library
decides how it finds files; one that provides no way to find them reports each
`\input` (`flm.input.unavailable`).

A file included this way is a *content input*: its content continues the text
around the command, as if it were written there (see [content inputs and
heading inputs](crate::lang::input#content-inputs-and-heading-inputs)):

```text
The results, \input{results-summary.flm} are shown below.
```

is one paragraph when `results-summary.flm` holds `in short, positive,`. A list
written in an included file nests in the list around the command, a label
written in one file can be referred to from the other, and footnotes, equations
and headings are numbered through the files in the order they are read. The
command may be written wherever text may: in a paragraph, in a list item, in an
environment's body.

An included file may begin with a front matter of its own. There it may set how
its own content is read, under `flm.parsing` (the keys of the primary file's
front matter; see the [command-line
guide](crate::guide::command_line#configuration-files-and-document-front-matter)):
those settings apply inside the file and not after the command, and the settings
the file does not write are those that apply where the command is written. The
settings of the document as a whole (the packages it uses, its features, its
layout, its default workflow) belong to the primary file alone, and writing them
in an included file is reported (`flm.input.front-matter`). A problem in an
included file is reported at its
line in that file, with the chain of files that led to it.

A file that includes itself, directly or through other files, is reported at the
command that would include it again (`flm.input.includes-itself`), and so is a
chain of inclusions more than
[`DEPTH_LIMIT`](crate::lang::input::DEPTH_LIMIT) files deep
(`flm.input.too-deep`). Where a file cannot
be brought in, the outputs show a dropped mark that gives the file's name. `\input` cannot be used
inside the template of an [author definition](#author-definitions)
(`flm.input.in-definition`).

### A file that opens with a heading

A file may instead be a part of the document with a heading of its own, such as
a chapter kept in a file: a *heading input*. Its front matter says so under
`flm.input_as`, and its `title` is the heading's title:

```text
---
title: The \emph{Art} of Parsing
flm:
  input_as:
    heading: section
    label: sec:art
---
The section's text.
```

`\input{art.flm}` is then read as the heading `\section{The \emph{Art} of
Parsing}` with the label `sec:art`, followed by the file's content. `heading`
gives the name of any heading command of the document's language; the heading is
exactly what that command makes of the title, numbered by that command like any
other heading and referred to by its label. Headings, equations, notes and
citations are counted in reading order across the files: the including file's
before the command, the file's own, then the including file's after the command.
`numbered: false` writes the command's unnumbered form (`\section*`); `label` may
be left out. The title is FLM text, and a problem in it is reported at its line
and column in the front matter. `heading` must be a command name written without
its backslash, and `label` must hold no brace; either fault is reported at the
command.

Such a file is a block: the command must be written where a heading may be
written, not inside a paragraph's text. A file may be included this way at one
place of a document only. Its content starts a text of its own: a paragraph does
not continue into it or out of it, and a list it holds is an outermost list even
where the command is written inside a list item. It may include other files in
turn, as content inputs or heading inputs. Only a program that assembles a whole document from its
files can include a heading input
([`model::assemble`](crate::model::assemble)); the `flm` program does.

## Author definitions

A **user definition** is a macro, an environment or a special character sequence
that a document's author or the program defines by *substitution*: a name, an
argument syntax, a **template** and the parsing modes it is defined for
([`Definition`](crate::lang::rewrites::userdefs::Definition)). The template is FLM
text with placeholders that stand for the arguments. Every use of the name is
replaced by its template, filled in with what the use site wrote, before the
document is rendered, on every output except the recomposed LaTeX, which can
keep a use site as a call of the definition (see [when a definition is
expanded](#when-a-definition-is-expanded)).

User definitions are declared in the settings of the
[`userdefs`](crate::stdcore::features::userdefs) feature, never in the document's
body, so a document cannot define a command inside itself: FLM has no
`\newcommand`. The `flm` program reads them from its own settings, from a data
package the document uses, or from the document's front matter (see the
[command-line
guide](crate::guide::command_line#configuration-files-and-document-front-matter)).
A program built on the library gives them as a
[`UserdefsConfig`](crate::stdcore::features::userdefs::UserdefsConfig), which the
standard package takes through
[`StdcoreConfig::userdefs`](crate::stdcore::StdcoreConfig::userdefs). User
definitions are fixed when the registry is built, before the document's body is
read, and the body can neither add to them nor switch them off.

FLM keeps an unknown command inside mathematics as written (see
[mathematics](#mathematics)). A `\newcommand` written inside a formula is
therefore kept as part of the formula, and it is not an FLM definition. A user
definition that applies in mathematics mode, as it does by default (see [the
parsing modes a definition applies
in](#the-parsing-modes-a-definition-applies-in)), is expanded inside a formula as
it is elsewhere.

Three callable kinds can be defined; they differ only in how a use site is
written:

| declared with | written in the document |
|---|---|
| [`Definition::macro_`](crate::lang::rewrites::userdefs::Definition::macro_) | `\name`, with its arguments after it |
| [`Definition::environment`](crate::lang::rewrites::userdefs::Definition::environment) | `\begin{name}` … `\end{name}` |
| [`Definition::specials`](crate::lang::rewrites::userdefs::Definition::specials) | the trigger characters, wherever they are written |

A macro and an environment may have the same name: they are two user
definitions.

Here are three user definitions and a document that uses them, rendered by the
text backend:

```rust
# use std::sync::Arc;
# use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
# use flm_core::core::plan::{Overrides, Plan};
# use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
# use flm_core::core::run::{Externals, Run, RunOptions};
# use flm_core::lang::{FlmOrigin, ParseOptions, parse_fragment};
# use flm_core::stdcore::backends::text::{self, TEXT_OUTPUT};
# use flm_core::techy::source::Source;
use flm_core::lang::mint::ArgDef;
use flm_core::stdcore::features::userdefs::{Definition, ExpandPolicy, UserdefsConfig};
use flm_core::stdcore::{self, StdcoreConfig};

let userdefs = UserdefsConfig::new()
    // `\hello{name}` — one mandatory argument, referred to by its name.
    .definition(
        Definition::macro_("hello", vec![ArgDef::mandatory("name")])
            .content("Hello, #{name}!"),
    )
    // `\begin{note}…\end{note}` — the body is `#{body}`.
    .definition(Definition::environment("note", Vec::new()).content("[Note: #{body}]"))
    // `\version` — no argument, and expanded in every plan.
    .definition(
        Definition::macro_("version", Vec::new()).content("1.0").expand(ExpandPolicy::Always),
    );

let mut builder = RegistryBuilder::new();
builder.use_package(stdcore::package(StdcoreConfig::new().userdefs(userdefs)));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;

let markup = "\\hello{World}, \\begin{note}a note\\end{note}, version \\version.\n";

# let source = Arc::new(Source::new(markup).with_origin(FlmOrigin::none()));
# let fragment = parse_fragment(
#     registry.lang(),
#     source,
#     FragmentLayout::default(),
#     &ParseOptions::default(),
# )?;
# let mut content = DocContent::new();
# content.place(Arc::new(fragment));
# let doc = Arc::new(Doc::new(content));
# let plan = Arc::new(Plan::for_workflow(&registry, text::WORKFLOW, &Overrides::none())?);
# let mut run = Run::new(doc, plan, Externals::none())?;
# run.execute(&RunOptions::new())?;
# let output = run.table(TEXT_OUTPUT)?.expect("the `text` workflow writes it");
assert_eq!(output.text, "Hello, World!, [Note: a note], version 1.0.\n");
# Ok::<(), Box<dyn std::error::Error>>(())
```

A user definition declares its arguments with the same argument kinds as any
other element ([`ArgStdKind`](crate::lang::mint::ArgStdKind); see the table under
[macros](#macros)), so it can take an optional argument, a star, a key-value
group or a verbatim argument exactly as a built-in command does. The arguments a
use site writes are *already parsed* when they are copied into the template; a
user definition cannot change how its own arguments were read. A placeholder is
replaced by the argument's *content*, what the use site wrote between the
argument's delimiters, and nothing else, regardless of the argument's kind: the
braces, the brackets or the delimiters of a verbatim argument are the syntax of
the call, not part of the argument. So `\c{x%y}`, where `\c` declares one
verbatim argument and has the template `see #1`, gives the text `see x%y`.

### The template language

A template is ordinary FLM text with one additional character, `#`
([`PLACEHOLDER_ESCAPE`](crate::lang::rewrites::userdefs::template::PLACEHOLDER_ESCAPE)),
which introduces a *placeholder*: a reference to an argument, to the body, or to
a name whose content an expansion hook computes
([`template`](crate::lang::rewrites::userdefs::template)).

| written in a template | stands for |
|---|---|
| `#1` … `#9` | the argument at that position, counted from one |
| `#{name}` | the argument declared under that name |
| `#{12}` | the twelfth argument: the braced form reaches beyond the ninth |
| `#{body}` | an environment's body ([`BODY_REFERENCE`](crate::lang::rewrites::userdefs::template::BODY_REFERENCE)) |
| `##` | a literal `#` |

The library uses the same language where it needs a template itself: a counter
style's number template writes `#{arabic}`, and a counter's within template
writes `#{flm.headings.section}` (see [the numbering
guide](crate::guide::numbering) and [the template language shared by
features](crate::guide::concepts_overview#the-template-language-shared-by-features)).
There a name stands for a number rather than an argument, and the mechanism is
this one: the same spelling, the whole template parsed at once, and content
substituted at each use.

Only one digit follows a bare `#`, as in TeX: in a template of a macro with one
argument, `#12` is the argument followed by the character `2`. The whitespace
after a placeholder is kept (`#1 and` keeps the space), unlike the whitespace
after a macro name.

The numbering can be shifted. A user definition may declare an **argument number
offset**
([`Definition::argument_number_offset`](crate::lang::rewrites::userdefs::Definition::argument_number_offset)),
one value for the whole definition, which makes `#1` refer to the argument at
that index: with an offset of one, `#1` is the *second* declared argument, and
the first one can be reached only by its name. Placeholders written with a name
are never shifted, and a default value keyed by a number follows the same
numbering as the template.

Everything else in a template is read in the registry's own language, so a
template may use any command the registry declares, other user definitions
included. A template is read once, when the registry is built, in the parsing
state that applies at the top level of a document: a template means the same
wherever it is used, and it cannot use a command that is available only where
the definition is used.

A command that is available only inside one environment, such as `\cell` inside
a table of cells, `\item` inside a list or `\\` inside a quotation's lines, is
therefore not available on its own in a template: a user definition whose
template is `\cell<#1>[#2]{#3}` is reported when the registry is built
([`BadDefinition`](crate::lang::rewrites::userdefs::BadDefinition)), regardless
of where it would be used; the registry is still built, and each use site of the
definition is reported (see [errors in author
definitions](#errors-in-author-definitions)). A template that writes the whole
environment may use such commands, since the environment makes them available
inside itself: `\begin{cells}\cell<#1>[#2]{#3}\end{cells}` is accepted where the
registry makes `cells` available at the top level of a document, and
`\begin{itemize}\item #1\end{itemize}` always is.

Two kinds of argument are not read as FLM content: the characters of a label or
of a reference target (`\label{…}`, `\ref{…}`), and verbatim content
(`\verbcode{…}`). Inside both, in a template, `#` still introduces a placeholder
and `##` still writes a literal `#`, so a user definition can pass its argument
into a label, a reference target or raw text. The template
`\section{#{title}}\label{sec:#{key}}` gives each heading it writes a label made
from its `key` argument, and `(see \ref{#1})` refers to the label the use site
writes. In a document, `#` is an ordinary character in both. A template that
declares a label must have the expansion policy `Always` (see [when a definition
is expanded](#when-a-definition-is-expanded)) for a document rendered to the
recomposed LaTeX: there a per-plan definition is written once, as a LaTeX
definition, and a label, a fact about the document, cannot be stated once for
every use; each use site is then the Error
`flm.userdefs.kept-template-declares-point`.

The same mechanism lets a user definition put its argument into code:

```text
\py{f(x)}
```

with the template `\verbcode[python]+#{code}+` prints

```text
f(x)
```

Nothing else of the markup language is recognized inside a template's verbatim
content (a special character sequence written there stays plain text), and in
the braced placeholder form `#{name}`, the name is read as plain characters, as
the example above shows.

What a placeholder puts inside verbatim content is read as characters: text as it
is, and a brace group of an ordinary argument as its characters without the
braces (`abc` for `a{b}c`). A command, a special character sequence or a formula
has no characters to give: the verbatim command or environment reports it as an
error (`flm.verbatim.content-not-characters`) and shows the content written as its source instead, so `\code{\emph{x}}`,
for a template `\verbcode{#1}` over an ordinary argument, shows `\emph{x}` as
code.

One further form exists for extensions: `#[…]{ref}` passes a *substitution
argument* to a placeholder whose content a feature computes; it is not available
inside verbatim content. The built-in placeholders (numbered, named, and `body`)
accept none: writing one on them is reported when the registry is built
([`BadPlaceholder`](crate::lang::rewrites::userdefs::BadPlaceholder)); the
registry is still built, and each use site of the definition is reported. No
standard feature computes such a placeholder.

### Default values

An argument that the use site did not write can be given a default value, itself
a template
([`Definition::default`](crate::lang::rewrites::userdefs::Definition::default),
keyed by the argument's name or by the number a template writes it as). Take a
macro `\unit[u]{n}`, with an optional unit and a mandatory number, whose
template is `#{n} #{u}` and whose argument `u` defaults to `m`. A document writes

```text
A length of \unit{3} and \unit[km]{3}.
```

and gets

```text
A length of 3 m and 3 km.
```

### Conditionals

A template can ask three different questions about an argument, each written as
a macro whose first argument is one placeholder. They are easy to confuse, so
they are listed here side by side
([`ConditionalTest`](crate::lang::rewrites::userdefs::template::ConditionalTest)):

| family | asks | on an absent argument that has a default value |
|---|---|---|
| `\IfProvidedT/F/TF`, `\IfBooleanT/F/TF` | did the use site *write* something here? | false: the default value is not what the author wrote |
| `\IfValueT/F/TF`, `\IfNoValueT/F/TF` | is there anything to substitute? | true: the default value is a value |
| `\ifblank`, `\notblank` | is what would be substituted empty, or comments only? | tests the default value's own content |

The suffix says which branches follow the test: `…T` takes one branch, used when
the test holds, `…F` one, used when it fails, and `…TF` two, the first for
success. `\ifblank{test}{blank}{full}` and `\notblank` always take both.
Whitespace counts as content, so `\m{ }` is not blank while `\m{}` is. The
fourteen conditionals are listed in
[`Conditional::ALL`](crate::lang::rewrites::userdefs::template::Conditional::ALL).

A conditional is evaluated where the document is expanded, so nothing of it
reaches a rendered page. The one output that keeps user definitions rather than
expanding them, the recomposed LaTeX, keeps the conditionals too, and all
fourteen can be written there: each becomes the branch its result selects where
that result cannot change from one use site to the next, or a test of LaTeX's
own over a value the call carries where it can. The [rendering
guide](crate::guide::rendering) shows what each becomes.

A template for `\greet[title]{name}` reading

```text
Hello, \IfValueT{#1}{#{title} }#{name}!
```

turns

```text
\greet{Ada} \greet[Dr]{Ada}
```

into

```text
Hello, Ada! Hello, Dr Ada!
```

Only the selected branch remains, and placeholders inside it are filled in like
any others.

### The parsing modes a definition applies in

A user definition applies in the parsing modes it declares
([`DefinitionModes`](crate::lang::rewrites::userdefs::DefinitionModes)): text
mode, [mathematics](#mathematics) mode, or both, which is the default.
[`Definition::content`](crate::lang::rewrites::userdefs::Definition::content)
sets the template and leaves the modes unchanged;
[`Definition::text`](crate::lang::rewrites::userdefs::Definition::text) and
[`Definition::math`](crate::lang::rewrites::userdefs::Definition::math) set the
template and restrict the definition to that one mode. A user definition has one
template, so calling both of the latter keeps only the later call.

The user definition is *defined* in those modes and in no other: used in another
mode, its name is one the registry does not declare there, and the use site is
treated like any other unknown command. In running text, an unknown command is
reported and kept as written, so a math-only definition used there is reported as
an unknown command. Inside mathematics, an unknown command is accepted without a
report (see [Mathematics](#mathematics)), so a text-only `\ket` used inside
`\(…\)` is kept as the source text `\ket` and nothing is reported; in neither
case is the use site a definition without a template.

A user definition declared for both modes applies in each mode **where its
template parses**. A template holding a command available only in formulas, such
as `\lvert{#1}\rangle`, applies inside `\(…\)`; a use of it in running text is
the Error `flm.userdefs.bad-definition` reported at that use, and the document is
still rendered. Such a definition is reported when the registry is built only
when its template parses in neither mode, and a definition restricted to one mode
when its template does not parse in that mode
([`BadDefinition`](crate::lang::rewrites::userdefs::BadDefinition)); the
registry is still built, and each use site of the definition is reported (see
[errors in author definitions](#errors-in-author-definitions)).

A name that needs a different template in each mode is declared as two user
definitions, one for text mode only and one for mathematics mode only, side by
side in one configuration. Their modes do not overlap, so both apply, each in its
own mode. Two user definitions of one name that *do* apply in a common mode make the
registry build fail, since only one of them could apply there.

### Blocks and paragraph breaks

A user definition declares the [block role](#paragraphs) of what it produces
([`BlockRole`](crate::model::aspects::BlockRole)). The default is inline content;
a definition declared as a block is placed on its own between the paragraphs
around it, and an environment defined as a block lays out its body as blocks
rather than inline. A blank line inside a template divides the paragraph the use
site is written in, because the template's content is inserted in place of the
use site, not wrapped in anything.

So a block environment `principle`, with one optional argument declared under
the name `name` and the template
`\textbf{Principle\IfValueT{#1}{ (#{name})}.} #{body}` turns

```text
Before.

\begin{principle}[Pythagoras]
The square of the hypotenuse.
\end{principle}

After.
```

into

```text
Before.

Principle (Pythagoras). The square of the hypotenuse.

After.
```

Theorems, lemmas and proofs are better written with the standard [theorem
environments](#theorems), which number them, label them and write their
heading.

### When a definition is expanded

A user definition states when it is expanded
([`ExpandPolicy`](crate::lang::rewrites::userdefs::ExpandPolicy)): per plan
(`PerPlan`), the default, or `Always`. An `Always` definition is expanded in the
document tree, before anything is rendered, for every output and in the `check`
workflow. A `PerPlan` definition is expanded in the same way for every output
except the recomposed LaTeX, and in the `check` workflow. The recomposed LaTeX
instead *keeps* its use sites in the tree and writes each as a use of the
definition, in text and inside mathematics alike, with the definition itself in
the output's style file (a *kept definition*; see
[`userdefs::kept`](crate::stdcore::features::userdefs::kept)).

A definition is written that way when its template *typesets* its arguments
(`see #1`, `\emph{#1}`), since LaTeX then puts each use's arguments where the
template puts them. A template that works on an argument's characters instead,
putting the argument into a link's address, into the target of a reference,
into a defined term or into code (`\href{https://doi.org/#1}{#1}`,
`\ref{sec:#1}`, `\term[#1]{…}`, `\verbcode{#1}`), needs characters that a use
supplies, so each of its use sites is written as its expansion there, as for an
`Always` definition, and nothing is reported. The few other definitions that
cannot be written as LaTeX definitions are expanded there too; the [rendering
guide](crate::guide::rendering) lists them. A program may also ask for the use
sites of a `PerPlan` definition to be kept on any output and expanded only when
they are rendered, by preferring the definition's `instantiate-at-lowering`
rule ([`instantiate_at_lowering_rule`](crate::stdcore::features::userdefs::instantiate_at_lowering_rule)).
A template that declares a label cannot be instantiated this way either: its use
sites are the Error `flm.userdefs.kept-template-declares-point` on every output,
and only the expansion policy `Always` avoids it.

### Errors in author definitions

- A template that does not parse, or that refers to an argument the definition
  does not have, is reported **when the registry is built**
  ([`BadDefinition`](crate::lang::rewrites::userdefs::BadDefinition),
  [`BadPlaceholder`](crate::lang::rewrites::userdefs::BadPlaceholder)), once,
  rather than in every document that uses it. The user definition stays
  registered but has no template, so each of its use sites is kept as its
  literal text and reported.
- User definitions that use each other without end stop at a depth limit
  ([`DEFAULT_RECURSION_LIMIT`](crate::lang::rewrites::userdefs::DEFAULT_RECURSION_LIMIT),
  32 nested expansions, which
  [`UserdefsConfig::recursion_limit`](crate::stdcore::features::userdefs::UserdefsConfig::recursion_limit)
  can change). The innermost use
  site is kept as its literal text and reported as one error
  ([`RecursionLimit`](crate::lang::rewrites::userdefs::RecursionLimit)).
- Two user definitions of the same callable kind and name refuse the registry
  build when they apply in a common parsing mode, and so does a user definition
  that claims a name another feature already declares in a mode it applies in,
  unless it is marked as explicitly replacing that declaration
  ([`Definition::overriding`](crate::lang::rewrites::userdefs::Definition::overriding)).

### Differences from the Python implementation

This template language is the one the Python implementation of FLM calls
`substmacros`. Every difference an author can observe is recorded in the
migration guide's section on user definitions ([migrating from Python
FLM](crate::guide::python_flm_migration)); this chapter states the language's
rules on their own.

## Unknown macros and environments

A command no feature declares is an error, and the document is still rendered:
the parse keeps the command as written and reports it once, where it is written.

```text
Text with \nope, and an unknown environment:

\begin{nope}a body

second paragraph\end{nope}

more text.
```

```text
Text with \nope, and an unknown environment:

\begin{nope}
a body

second paragraph
\end{nope}

more text.
```

- An **unknown macro** becomes a node of the element
  [`flm.unknown.macro`](crate::lang::structural::UNKNOWN_MACRO), and the parse
  reports the Error
  [`UnresolvableCommand`](crate::techy::core::constructs::UnresolvableCommand).
  It takes no argument, since nothing is known about its syntax, so what follows
  it is read as ordinary content, and it is inline. The text backend prints it
  back as written, `\nope`
  ([`UnknownMacroRule`](crate::stdcore::backends::lowering::UnknownMacroRule)).
  The usual whitespace rule still applies: the space after the name is part of
  the macro, so `\nope and` prints as `\nopeand`.
- An **unknown environment** becomes a node of the element
  [`flm.unknown.environment`](crate::lang::structural::UNKNOWN_ENVIRONMENT), and
  the parse reports the Error
  [`UnknownEnvironment`](crate::techy::latexlike::UnknownEnvironment). Its body
  *is* read, as blocks, and ends at its own `\end{name}`; the node is a block of
  its own. The text backend prints the `\begin` and `\end` lines around the body
  ([`UnknownEnvironmentRule`](crate::stdcore::backends::lowering::UnknownEnvironmentRule)).

Each occurrence raises exactly one diagnostic: a paragraph break inside an
unknown environment's body, or content that follows an unknown macro, adds no
further one. These two elements are among the *structural elements* (the
elements of the node shapes no feature declares; see
[`StructuralElements`](crate::lang::structural::StructuralElements) and
[structural elements](crate::guide::concepts_overview#structural-elements)). Inside
[mathematics](#mathematics) the rule is different: the mathematics feature
accepts an unknown name there first, without a report, and no error is raised.

One other message can appear in place of content. If a program registers an
element for which the backend it renders to has no lowering rule (the rule that says how
a backend writes an element; see
[`LoweringRule`](crate::core::lowering::LoweringRule)), and the
program asked for that element to be accepted as unrenderable rather than
refused (an *accepted unrenderable element*; see
[`PlanRequest::unsupported`](crate::core::plan::PlanRequest::unsupported)), the
text backend prints `[unsupported: <element name>]` and reports one error
([`UnsupportedElement`](crate::stdcore::backends::lowering::UnsupportedElement)).
That is a mismatch between a program's features and its backend, which a
document cannot cause on its own; without that permission the whole rendering is
refused before any document is read, with a message giving the element that has
no lowering rule.

## Markup not yet available

The library provides no markup today for the following; where a document needs
one of them now, the way to add it is a feature of one's own (see [the extension
guide](crate::guide::extending)):

- the numbering of anything but headings, equations, list items, xnotes,
  theorems, floats and cited works (described under [numbered
  headings](#numbered-headings), [inside a display-math
  environment](#inside-a-display-math-environment), [lists](#lists),
  [xnotes](#xnotes), [theorems](#theorems), [floats](#floats) and
  [citations](#citations));
- links to anything other than a labeled reference point in the same document
  or an address outside it (see [hyperlinks](#hyperlinks));
- a configuration block inside the document: the parsing settings, the
  typographic substitutions and the user definitions are all fixed by the
  program that builds the registry.