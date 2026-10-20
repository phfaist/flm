# flmdoc

The extension `flmdoc` provides the markup of documentation pages, such as the pages that
the command `flm info` shows: a code span written between backticks, the constructs that
lay out the documentation of one item (a command, an environment, a feature, …), and a
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
\begin{docitem}[macro-section]{macro}{\section}
\docsignature{\section*[short]{title}}
\docsummary{Starts a section of the document.}
\begin{docparams}
\docparam{*}{star}{}{The section is not numbered.}
\docparam{{title}}{title}{flm.headings.title}{The title.}
\end{docparams}
\docfield{Declared by}{the feature \docref{features/headings}{headings}}
\end{docitem}
```

- `\begin{docitem}[anchor]{kind}{name} … \end{docitem}` documents one item. `kind` is one
  word (`macro`, `environment`, `feature`, `stage`, any word), `name` is the item's name as
  written (`\section`), shown as code; both are read as raw text. The optional `anchor` is
  the identifier a link to the item points at (in HTML, the `id` of its `<div>`). The body
  holds the item's paragraphs and blocks.
- `\docsignature{…}` shows how the item is written, as a block of code (raw text).
- `\docsummary{…}` states the item's summary, as a paragraph.
- `\begin{docparams}[title] … \end{docparams}` lists the item's parameters under a title,
  `Arguments` when none is given. Its body holds `\docparam` commands, and that command is
  available there only.
- `\docparam{form}{name}{role}{text}` describes one parameter: how it is written (`{title}`,
  `[options]`, `*`), its name, the registered name of what it holds (its role), each read
  as raw text and each but the form may be empty, then what it holds: one line of text, or
  paragraphs.
- `\docfield{key}{value}` states one labeled fact, such as `Declared by` or `Available in`,
  as a paragraph `key: value`.

The arguments read as raw text (the kind and the name of `docitem`, the signature, the
form, name and role of `\docparam`, and the path of `\docref` below) are usually written
between braces, inside which braces pair up. One that holds a brace alone is written
between another pair of delimiters, such as `|…|`: the item `\{` is documented with
`\begin{docitem}{macro}|\{|` and `\docsignature|\{|`.

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
spans, a list, inline and block code, a link) carrying a role named after the construct:
`flmdoc.item`, `flmdoc.item.kind`, `flmdoc.item.name`, `flmdoc.signature`,
`flmdoc.summary`, `flmdoc.params`, `flmdoc.params.title`, `flmdoc.param.form`,
`flmdoc.param.name`, `flmdoc.param.role`, `flmdoc.field.key`, `flmdoc.field.value`. The
HTML output writes each role as a class (`flmdoc-item`, `flmdoc-param-role`, …) and the
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
