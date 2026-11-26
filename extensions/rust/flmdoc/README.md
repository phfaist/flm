# flmdoc

The extension `flmdoc` provides the markup of documentation pages, such as the pages that
the command `flm info` shows: a code span written between backticks, the constructs that
lay out the documentation of one item (a command, an environment, a feature, …) with its
facts, lists of named things and inline lists, the type a JSON Schema describes, and a
link to another documentation page. The constructs are written for the `text` and `html`
profiles. The extension is written in Rust against the guest client `flm_ext::guest` and
is compiled into the `flm` program: nothing needs to be installed.

## Using it

A document uses the extension by listing it in its front matter's `use` list, or with
`--use flmdoc` on the command line:

```text
---
flm:
  use: [flmdoc]
---
The command `\section` starts a section; see \docref{features/headings}{the feature}.
```

## Code spans

A code span begins with a run of backticks and ends at the next run of as many backticks.
Its content is shown as inline code, exactly as written: a backslash, a brace or a percent
sign is an ordinary character there. `` `x` `` shows `x`. To show a backtick, open the span
with a longer run: ``` ``a`b`` ``` shows ``a`b``. The content is then adjusted in two
steps, as in CommonMark. First, each line ending becomes a space, so that a span may be
wrapped across lines of the source. Then, when the content both begins and ends with a
space and is not all spaces, one space is removed from each end: ``` `` `a` `` ``` shows
`` `a` ``. Code spans are written in text; inside mathematics a backtick is an ordinary
character.

In a document that uses the extension, every backtick in text opens a code span. The
quotation marks that FLM otherwise writes with backticks, ``` ``…'' ``` and `` `…' ``, are
therefore not available: write “…” and ‘…’ directly. A backtick that no later run of as
many backticks closes is an error. Since a span runs on to the next such run, even across
paragraphs, the error of a stray backtick may be reported at a later backtick of the
document rather than at the stray one.

## Documenting an item

```text
\begin{docitem}[anchor=macro-section, declared-by=\docref{features/headings}{`headings`}, modes=`text`]{macro}{\section}
\docsignature{\section*[short]{title}}
\docsummary{Starts a section of the document.}
\begin{docdetails}
The section's number appears in `\ref`.
\end{docdetails}
\begin{docparams}
\docparam{*}{star}{}{The section is not numbered.}
\docparam{{title}}{title}{flm.headings.title}{The title.}
\end{docparams}
\docfield{Element type}{\docref{namespaces/element/flm.heading}{`flm.heading`}}
\end{docitem}
```

- `\begin{docitem}[facts]{kind}{name} … \end{docitem}` documents one item. `kind` is one
  word (`macro`, `environment`, `feature`, `stage`, any word), `name` is the item's name as
  written (`\section`), shown as code; both are read as raw text. The optional argument
  holds the item's facts as key–values (below); its key `anchor` is the identifier a link
  to the item points at (in HTML, the `id` of its `<div>`). The body holds the item's
  paragraphs and blocks; the facts are written after it.
- `\docsignature{…}` shows how the item is written, as a block of code (raw text).
- `\docsummary{…}` states the summary of a documentation text, as a paragraph.
- `\begin{docdetails} … \end{docdetails}` holds the rest of a documentation text after
  its summary: its paragraphs and blocks.
- `\begin{docparams}[title] … \end{docparams}` lists the item's parameters under a title,
  `Arguments` when none is given. Its body holds `\docparam` commands, and that command is
  available there only.
- `\docparam[facts]{form}{name}{role}{text}` describes one parameter: how it is written
  (`{title}`, `[options]`, `*`), its name, the registered name of what it holds (its
  role), each read as raw text and each but the form may be empty, then what it holds: one
  line of text, or paragraphs. Its facts are written after the text.
- `\docfield{key}{value}` states one labeled fact, such as `Element type` or `Type`, as a
  paragraph `key: value`.
- `\docjsontype{schema}` shows the type a JSON Schema describes, in words, as inline code:
  `\docjsontype{{"type": "array", "items": {"type": "string"}}}` shows `list of string`.
  The schema is raw text; text that is not JSON is an error, shown as written.

The arguments read as raw text (the kind and the name of `docitem`, the signature, the
form, name and role of `\docparam`, the schema of `\docjsontype`, and the path of
`\docref` below) are usually written between braces, inside which braces pair up. One
that holds a brace alone is written between another pair of delimiters, such as `|…|`:
the item `\{` is documented with `\begin{docitem}{macro}|\{|` and `\docsignature|\{|`.

## Lists

```text
\begin{docentries}[title={Element values}, intro={The values of the type, with their commands.}]
\docentry[written=\begin{docinlinelist}\item \docref{features/textstyles/macros/emph}{`\emph`}\end{docinlinelist}]{}{Emphasis of its content.}
\docentry[unwritten]{}{A value no command writes.}
\end{docentries}
```

- `\begin{docentries}[options] … \end{docentries}` lists named things, one `\docentry`
  each, marked `•`. Its options are key–values: `title`, the heading of the page's part
  that the list is; `intro`, a paragraph under it; `level`, the heading's level (a
  whole number from 0 to 255, 1 by default).
- `\docentry[facts]{name}{summary}` lists one thing: its name (usually a link to its
  page; it may be empty), its facts, and its summary. It is available in the body of
  `docentries` only.
- `\begin{docinlinelist} \item[facts] … \item[facts] … \end{docinlinelist}` lists things
  inline, in a sentence or in a value: each `\item` begins an item, which runs up to the
  next `\item` or the end of the list, and `flmdoc` writes the commas between the items.
  `\item` is available in the body of `docinlinelist` only.

## Facts

A fact that the documentation composes from structure is given as a key–value of the
construct of the thing it describes, and `flmdoc` writes its label and its words. A value
is document content (a link, code, an inline list, text); a value written as one group,
`{…}`, is the group's content, so that it may hold a comma. Some keys take one word, with
no whitespace, and a flag takes no value. A key a construct does not read, and a value a key does not accept,
are errors at the construct.

On `docitem` and `\docentry`, written as labeled facts:

| keys | words |
|---|---|
| `declared-by=X`, with `declarer=feature` (the default), `language`, `program`, `package` or `data` | Declared by: the feature X; the language itself (X); the program, as data (X); the package X; written as data by X |
| `within=X`, `switched-off=X`, `page=X` (each with `declared-by`) | …, in the scoped package X; …, which is switched off, and Switched off: in X; …; see X |
| `modes=X`, `places=X` | Available in: X mode (X modes for an inline list of several), inside X |
| `origin=compiled`, `extension` or `bundled`, with `extension=E` for the last two | Origin: compiled into the program; from the extension E; from the extension E, bundled into the program |

On `docitem` alone: `anchor=…`, and a stage's status in a plan, `plan=scheduled`
(`batch=N`), `plan=ruled-out` (`by=B`, `over=O`) or `plan=not-scheduled`, with
`workflow=W`: “In the plan of the workflow W” (or “In the plan in view”): “scheduled, in
batch N” (“scheduled, as a rewrite stage” without a batch), “ruled out by B, which
provides O as well”, “not scheduled: nothing the plan needs requires it”.

On `\docentry` alone, in its line: `origin` as above, `owner=X` (X), `data` (given as
data), `outputs=X` (outputs: X) and `from=X` (X) in parentheses after the name; `absent`
(, which does not exist), `by=B` with `over=O` (, by B, which provides O as well) and
`profile=X` ( provides the profile X) after them; `rule=X` (: X); and after the summary
`written=X` (Written as X.), `unwritten` (No command is written for this value.) and
`aspects=X` (Aspects: X.).

On `\docparam`, in parentheses after the text: `layout=W` (laid out as `W`), `mode=W`
(read in `W` mode), `scoped=X` (with the scoped package X available inside), `kept`
(content kept as written), `field=X` (an argument of the field X), `default=X` (default:
X), `reader=X` (read as X); then `repeatable` (It may be written more than once.).

On `\item` of `docinlinelist`: `package` (the package …), `body` (the body of …), `argument=N` (argument N of …) or
`argument=name` (the argument `name` of …), and `field=X` (… the field X of …), before the
item; `writer=X` (written by X), `optional` (may be absent) and `missing` (not in the
registry) in parentheses after it.

## Links to other pages

`\docref{path}{text}` links to another documentation page, specified by its path
(`features/headings`, read as raw text). The address the link points at depends on the
setting `flmdoc.link_style`, which a render request sets (`-O flmdoc.link_style=path`):

| `link_style` | address of `\docref{features/headings}{…}` | for |
|---|---|---|
| `command` (the default) | `flm info features/headings` | reading the pages at a terminal |
| `path` | `features/headings.html` | a folder of HTML pages |

The address `path` writes is the page's path followed by `.html`, counted from the root of
the page tree. It is not relative to the page holding the link: it reaches the page from a
page at the root of the tree, and not from a page in a folder below it.

The `text` profile prints a link's text followed by its address, as it prints every link:
`headings <flm info features/headings>`. Any other value of `link_style` is an error,
reported at each `\docref`.

## What the output holds

On both backends, each construct becomes ordinary presentation nodes (paragraphs, blocks,
headings, spans, lists, inline and block code, a link) carrying a role named after the
construct: `flmdoc.item`, `flmdoc.item.kind`, `flmdoc.item.name`, `flmdoc.signature`,
`flmdoc.summary`, `flmdoc.details`, `flmdoc.params`, `flmdoc.params.title`,
`flmdoc.param.form`, `flmdoc.param.name`, `flmdoc.param.role`, `flmdoc.entries`,
`flmdoc.entries.intro`, `flmdoc.entry.name`, `flmdoc.entry.summary`,
`flmdoc.inline-list`, `flmdoc.inline-item`, `flmdoc.json-type`, `flmdoc.field.key`,
`flmdoc.field.value`; the words of each fact are in a span of the role
`flmdoc.fact.<key>`. The title of `docentries` is a heading. The HTML output writes each
role as a class (`flmdoc-item`, `flmdoc-param-role`, `flmdoc-inline--list`, …) and the
item's kind as the attribute `data-flm-kind`; the extension ships no stylesheet. The text
output prints the role of a parameter in parentheses, `{title} title (flm.headings.title):
The title.`, and each parameter as a paragraph of its own, with no list marker.

## LaTeX

The extension has no rules of its own for the LaTeX backends. The profile `latex-rendered`
(and `pdf-latex`, which compiles its output) writes the paragraphs, blocks, spans, lists,
code and links the constructs become, as it writes every such node, with the roles as
options (`\flmxBlock[role=flmdoc.item, kinds={macro}, …]`). The profile `latex-recomposed`
does not: a document that uses the extension cannot be rendered with it, and the program
stops before rendering, listing the constructs that the backend cannot render.

## The WebAssembly form

The extension is compiled into the `flm` program, like `profile-pdf-latex` and
`syntax-highlight`. To build its standalone WebAssembly form:

```sh
extensions/rust/build-wasm.sh flmdoc
```
