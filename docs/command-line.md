# The flm command-line tool

`flm` is the command-line program of FLM, a LaTeX-like markup language. It renders a
document written in FLM into an HTML page, plain text or LaTeX, checks a document for
problems, shows a live preview of a document in a browser, prints the schema of its
settings files, and describes the commands, settings and workflows a document may use. The program is the binary `flm` of the crate `flm-cli`. The processing
itself is done by two libraries: the library [`flm-core`](crate) turns the text of a
document into outputs, and the host library `flm-main` reads the files, reads the
settings written at the top of a document, builds the library's
[registry](crate::core::registry) from the packages that the document uses, and writes the
outputs.
`flm` turns a command line into calls of the host library, prints the problems found on
standard error, and exits with a status that says how the command ended.

The program has six commands:

- `flm render INPUT` renders a document and writes the output. See
  [Rendering documents from the command line](#rendering-documents-from-the-command-line).
- `flm check INPUT` reports the problems of a document and writes no output. See
  [Checking a document](#checking-a-document).
- `flm watch INPUT`, or `flm INPUT`, is the live preview: it builds the document, shows
  it in the browser, and builds it again whenever a file the document reads changes. See
  [Watching a document: the live preview](#watching-a-document-the-live-preview).
- `flm schema` prints the JSON Schema of the settings files, with which a text editor can
  check them. See [Printing the schema of the settings](#printing-the-schema-of-the-settings).
- `flm info [PATH…]` describes the environment in which a document is processed (the
  packages, the features and the commands they declare, the settings, the workflows, the
  program itself) as pages of documentation, printed as text or as JSON, or written one
  page at a time as an FLM document. See
  [Describing the environment: `flm info`](#describing-the-environment-flm-info).
- `flm dump FILE` is an inspection tool: it processes one file with the library alone and
  prints one view of the result as JSON (a tree at a chosen point of the processing, a
  computed table, or the artifacts of an output). See
  [Inspecting the processing with `flm dump`](#inspecting-the-processing-with-flm-dump).

The settings of a document, written in its front matter and in the packages of the
extensions it uses, are described in
[Configuration files and document front matter](#configuration-files-and-document-front-matter).

## Building and running the program

The program is the binary `flm` of the crate `flm-cli`, the only binary of this
workspace. From the workspace, it is built and run with

```text
$ cargo run -- render paper.flm
```

Once installed, the same command is `flm render paper.flm`; the examples of this chapter
use that form. The shortest command, `flm paper.flm`, starts the live preview.

`flm --version` prints the program's name and version and exits with status 0. `flm
--help` prints the commands, and `flm COMMAND --help` prints a command's options, each
with what it does, the value it takes and, where it has one, its default; both exit with
status 0. An empty command line prints the help on standard error and exits with status 2.

The program runs extensions written in JavaScript and extensions compiled to WebAssembly
(see [Extensions](#extensions)) through the cargo features `js` and `wasm` of `flm-cli`,
both on by default. A program built without one of them (`cargo build -p flm-cli
--no-default-features --features js`, say) refuses an extension of the engine it lacks,
naming the engines it has.

### The compiled packages the program carries

A *package* is a named bundle of features together with the packages it builds on
([`PackageEntry`](crate::core::registry::PackageEntry)); a *feature* is a Rust value
that contributes a related bundle of things, such as commands an author can write,
processing stages and settings, to the library's processing
([`FlmFeature`](crate::core::registry::FlmFeature)). The program carries these
packages:

- `stdcore`, the library's standard package: the language's constructs, the numbering,
  the references, the floats and the other features of the document's content, and the
  standard backends and workflows. It processes no mathematics.
- `math-passthrough`, the library's math processor that reads nothing inside a formula
  (see [Mathematics processed by MathJax](#mathematics-processed-by-mathjax)).
- `stdmain`, the program's own features: `write` (the output's files handed to standard
  output, a file or a folder), `preview` (the live preview), `compose` and
  `compose-standard` (the compose templates), `graphics-files` (the image files) and
  `citations-standard` (the cited works). It uses `stdcore`.
- `std`, a package with no features of its own that uses `stdcore` and `stdmain`.
- `phfpack` (macros for the notation and quantities of quantum information, from the
  crate `flm-phfpack`). The cargo feature `phfpack` of `flm-cli`, which is on by default,
  includes it. A program built without the cargo feature (`cargo build -p flm-cli
  --no-default-features`) refuses a document that lists `phfpack`, as it refuses any
  package it cannot find.

The extensions compiled into the program, `math-mathjax` among them, are packages too
(see [Where extensions are looked for](#where-extensions-are-looked-for)).

### The packages a document uses

A document uses these packages, in this order, each package after the packages it uses,
and each once:

1. the packages of the *initial list*, by default `std`, the program's features;
2. the package of the *math processor*, the package that processes the document's
   mathematics ([Mathematics processed by MathJax](#mathematics-processed-by-mathjax));
3. the packages its front matter lists under `use`, then those given with `--use`;
4. the extension `profile-NAME` of the profile `NAME` the command builds with, when that
   extension exists ([Profiles and workflows](#profiles-and-workflows)).

The command line adds a package with `--use PACKAGE`, as if it were written last in the
front matter's `use` list; the option is repeatable, and the packages are used in the
order given. A package is written as in a `use` entry and found the same way: a compiled
package, or an extension on the search path or compiled into the program. This command
uses the packages of the document's own `use` list, then the extension `syntax-highlight`,
which highlights the code listings:

```text
$ flm render --use syntax-highlight paper.flm
```

Replacing the initial list is rarely needed, since `--use` adds a package.
A document replaces the initial list with the key `initial_use` of its front matter, and
the command line replaces both with `--initial-use`. Either gives the list exactly: a
package it leaves out is not used unless `use` lists it.

```yaml
---
flm:
  initial_use: [std, myext]
---
```

```text
$ flm render --initial-use '[std, myext]' paper.flm
```

The math processor is chosen apart from the initial list, by its name: with the key
`math` of the front matter, or with `--math` on the command line, which takes precedence.
The name `NAME` uses the package `math-NAME`: `mathjax` uses the extension
`math-mathjax`, which typesets formulas with MathJax, and `passthrough` uses the library's
`math-passthrough`, which reads nothing inside a formula. Without `math` or `--math`, the
program uses its default processor, `mathjax`; a program built without the cargo feature
`js` or without `bundled-extensions` has no `math-mathjax`, and its default processor is
`passthrough`. The default processor is not guaranteed to stay the same in later versions
of the program: a document that needs a stable result specifies its processor.

```yaml
---
flm:
  math: passthrough
---
```

```text
$ flm render --math passthrough paper.flm
```

The processor's package follows the initial list, whichever list applies: a document that
writes `initial_use` keeps its math processor. The package is found as an entry of `use`
is, so a copy of `math-mathjax` in `_flmextensions` is used in place of the one compiled
into the program ([Where extensions are looked for](#where-extensions-are-looked-for)). The
value `false` (`math: false`, `--math false`) uses no math processor, for a document that
lists its processor under `use`, such as a processor whose package's name does not have
the form `math-NAME`.

### Building a program that carries other packages

The library of the crate `flm-cli`, `flm_cli`, holds the whole command line:
`flm_cli::run(args, packages)` takes the command-line arguments and the list of compiled
package factories that the program carries. A program of one's own that carries other
compiled packages is a `main` function of a few lines that passes its own list. The
example `own_package` of `flm-cli` carries `phfpack` that way:

```text
$ cargo run -p flm-cli --example own_package -- render paper.flm
```

## Rendering documents from the command line

`flm render INPUT` reads a document, renders it into one output and writes that output.
The input is a file, `-` to read the document from standard input, or the text given with
`-c TEXT` in place of `INPUT`. The output goes to standard output unless `-o` gives a file
or a folder. (`flm INPUT`, with no command, runs the live preview `flm watch INPUT`: see
[Watching a document](#watching-a-document-the-live-preview).)

```text
$ flm render paper.flm > paper.html   # a complete page, on standard output
$ flm render -p text paper.flm        # plain text
$ flm render -p latex-recomposed -o out/paper.tex paper.flm
$ flm render -p latex-rendered -o out/paper.tex paper.flm
$ flm render -p pdf-latex -o out/paper.pdf paper.flm   # a PDF file, compiled by LaTeX
$ flm render -c 'Some \emph{text}.' -p text
$ flm render -V -o paper.html paper.flm   # written, then opened
```

### What to build

These options are shared by `flm render`, `flm check` and `flm watch` (the preview
accepts neither `-` nor `-c`):

| option | what it does |
|---|---|
| `-p PROFILE` | the profile to build with, which determines what kind of output is produced: `html` (a complete HTML page), `text`, `latex-rendered` (LaTeX for LuaLaTeX, with everything FLM resolves already resolved), `latex-recomposed` (the document written back as LaTeX source), `bibtex` or `csl-json` (the records of the cited works as a BibTeX or a CSL-JSON file; see [Citations](#citations), "The exports"), `check` (no output), `pdf-latex` (a PDF file; see [PDF through LaTeX](#pdf-through-latex)), or a profile that an extension provides. The profile PROFILE uses the extension `profile-PROFILE` when one exists, then runs the workflow PROFILE. Every format the workflow provides is produced, unless `-f` selects one. Default: for `flm render` and `flm watch`, the front matter's `default_profile`, else `html`; for `flm check`, `check` (see [Profiles and workflows](#profiles-and-workflows)) |
| `-f FORMAT` | only the format FORMAT, one of the formats that the active workflow provides (the workflow of the profile chosen with `-p`, or of the default one). `-f` does not choose the profile: another kind of output is produced by choosing its profile with `-p`. The program's workflows `html`, `text`, `latex-rendered`, `latex-recomposed`, `bibtex` and `csl-json` each provide the one format of the same name; a workflow that an extension declares may provide several. Default: every format the workflow provides |
| `--use PACKAGE` | use the package PACKAGE in addition to those of the front matter's `use`, as if it were written last in that list: a compiled package, or an extension found on the search path or compiled into the program (`--use syntax-highlight` highlights the code listings). Repeatable; the packages are used in the order given (see [The packages a document uses](#the-packages-a-document-uses)) |
| `--initial-use PACKAGES` | the packages to use before those of the front matter's `use`: one package's name, or a list in YAML's one-line form (`--initial-use '[std, myext]'`), used exactly as given. It takes precedence over the front matter's `initial_use`. Rarely needed, since it replaces the program's own list of packages: a package is added with `--use`. Default: the front matter's `initial_use`, else `std` (see [The packages a document uses](#the-packages-a-document-uses)) |
| `--math PROCESSOR` | the math processor, the package that processes the document's mathematics: a processor's name, which uses the package `math-NAME` (`--math passthrough` uses `math-passthrough`), or `false` for no processor. Its package is used after the packages of the initial list and before those of `use`. It takes precedence over the front matter's `math`. Default: the front matter's `math`, else `mathjax` (`passthrough` in a program built without `js` or `bundled-extensions`); this default is not guaranteed to stay the same (see [The packages a document uses](#the-packages-a-document-uses)) |
| `-t TEMPLATE` | the compose template that assembles the output's main file: one of the program's own, which `flm render --help` lists for each output, or a compose template that an extension the document uses ships. Default: `simple` (see [Compose templates](#compose-templates)) |
| `-O FEATURE.KEY=VALUE` | a setting of a feature, the value written in YAML's one-line form; repeatable (see [Settings on the command line](#settings-on-the-command-line)) |
| `--layout blocks\|inline\|auto`, `-B` | how the document's content is read at its top level: `blocks`, as paragraphs and blocks; `inline`, as one run of text; `auto`, as paragraphs and blocks when the content holds a paragraph break or a block, and as one run of text otherwise. Default: the front matter's `layout`, else `auto`. `-B` is the same as `--layout blocks` |
| `--extensions-dir DIR` | a folder in which extensions are looked for by name (see [Where extensions are looked for](#where-extensions-are-looked-for)); repeatable, and each folder must exist. Default: the folder `_flmextensions` beside the document. The extensions compiled into the program are found after every folder |
| `-v` | print on standard error what the program does: the files it reads, the profile, the packages and the workflow it uses, the requests it sends to fetch the details of cited works (each pass, request and wait), and the files it writes |

`-t TEMPLATE` is the same as `-O compose.template=TEMPLATE` written before every other
`-O`: a later `-O compose.template=…` replaces it, and a problem with the template is
reported as a problem of `-t TEMPLATE`.

### Settings on the command line

`-O FEATURE.KEY=VALUE` sets the settings key `KEY` of the feature `FEATURE`, with the
value written in YAML's one-line form: `-O 'text.width=60'`, `-O
'html.heading_tags={section: h2}'`. A setting given with `-O` takes precedence over the
same setting in the front matter, in a package of an extension or in a workflow (see
[Ranks of the settings](#ranks-of-the-settings)); of two `-O` of one key, the later one
wins.

`-O FEATURE.KEY.NAME=VALUE` writes the value one level below the key, as if the key's
value were a map holding `VALUE` under `NAME`: `-O 'floats.types.table=false'` is the
same as `-O 'floats.types={table: false}'`. For a key that holds a collection of named
entries (see [Collections of named entries](#collections-of-named-entries)), the
assignment therefore sets one entry and leaves the others. Below any other key, it
writes the whole key, as the map form does, so a second `-O` of a path below the same key
replaces the first: `-O html.heading_tags.section=h2 -O html.heading_tags.subsection=h3`
leaves only `subsection`, as a second `-O` of the whole map would. An entry whose name
holds a `.` (the counter style `flm.headings.style.section` under `numbering.styles`, for
example) cannot be reached by a path; write it in the map form.

### Where the output goes

A command writes every file of every output it produces (see [Profiles and
workflows](#profiles-and-workflows)): the main file of each output, such as the page or the LaTeX
document, and the files that go with it, such as the style file of a LaTeX output or a
copied image. The option `-o` gives the destination of all of them:

| option | what it does |
|---|---|
| none, or `-o -` | standard output, which receives exactly one file. The file printed there ends with exactly one newline, or with none under `-n`. A file written with `-o` holds the output exactly as made, with or without `-n` |
| `-o paper.html` | files named after `paper.html` (see below), written in the folder of `paper.html` |
| `-o site/` (or an existing folder) | files named after `index`, written in the folder `site` |
| `-V` | once the files are written, open the main file of the *leading output* (the output `-f` names, else the workflow's first) with the system's default application for it. Requires `-o` with a file or a folder |

**The names of the files.** Every file a command writes shares one *stem*, a file name
without its extension. With `-o NAME`, the stem is `NAME` without its extension when that
extension is the one of an output the command produces (`html` for the page, `txt` for the
text, `tex` for either LaTeX output, `bib` and `json` for the exports), and `NAME` as
written otherwise. With `-o FOLDER/`, the stem is `index`. The main file of each output is
then `STEM.EXT`, `EXT` being the output's extension, and every other file is
`STEM-NAME.EXT`, where `NAME` is the name the output gives the file. For a workflow with the
outputs `html` and `text`:

| `-o` | files written |
|---|---|
| `paper.html`, `paper.txt` or `paper` | `paper.html` and `paper.txt` |
| `out/paper` | `out/paper.html` and `out/paper.txt` |
| `out/` | `out/index.html` and `out/index.txt` |
| `index.htm` | `index.htm.html` and `index.htm.txt` (`htm` is no output's extension) |

A file named with `-o` therefore always carries its output's extension: `-o notes.txt` for
the output `html` alone writes `notes.txt.html`. For the recomposed LaTeX, `-o paper.tex`
writes `paper.tex` and its style file `paper-flmrecomposed.1a2b3c4d.sty`, which the document
loads with `\usepackage{paper-flmrecomposed.1a2b3c4d}`; `-o out/` writes `out/index.tex`
and `out/index-flmrecomposed.1a2b3c4d.sty`. The part `flmrecomposed.1a2b3c4d` is the
render option `style_stem` and a digest of the file's content (see [the recomposed
LaTeX backend](crate::guide::rendering#the-recomposed-latex-backend)). The LaTeX for
LuaLaTeX names its style file the same way, with the stem `flmx`: `-o paper.tex` writes
`paper.tex` and `paper-flmx.1a2b3c4d.sty`, and `-o out/` writes `out/index.tex` and
`out/index-flmx.1a2b3c4d.sty`. A copied image is named the same way
(`paper-plot-3fa9c2d1.png`; see [Images](#images)).

**A name for one output's main file.** The setting `write.output_names` gives the main
file of one output a name of its own. It is a map from an output's name (`html`, `text`,
`latex-recomposed`, `bibtex`, `csl-json`, or an output a package adds) to the path of that
output's main file, relative to the folder the files are written in. The path is taken
exactly as written: no stem applies, and no extension is added or removed (`refs` names a
file `refs`). It may hold a folder (`exports/refs.bib`). The setting can be written in three
places. On the command line, `-O` sets one entry and leaves the others:

```sh
flm render -p both -o out/ -O write.output_names.text=notes.txt paper.flm
# writes out/index.html and out/notes.txt
```

In a front matter:

```yaml
flm:
  features:
    write:
      output_names: {bibtex: refs.bib}
```

In a workflow, under its `settings`:

```yaml
flm:
  workflows:
    named:
      extends: html
      outputs: [html, text]
      settings:
        write:
          output_names: {text: notes.txt}
```

The other files of a renamed output keep the stem that `-o` gives: with
`-o out/ -O write.output_names.html=page.html`, a page whose template ships a stylesheet
writes `out/page.html` and `out/index-style.css`. The reason is the order of the work. The
program names a copied image, the recomposed LaTeX's style file and the files a template
ships while it makes the output, before it knows which output each file belongs to. Only the
last step, which writes the files, names the main file of an output, and it alone knows
the output. A name for an output that the packages in use do not have is an error, and
nothing is written. A path that leaves the output folder, and two outputs that would write
different files at one path, are errors too, as for any other name. Under `-o -`, the name
changes nothing: the one file is printed.

**Standard output.** Standard output receives one file in total. A command whose outputs
hold more than one file (two outputs; the recomposed LaTeX with its style file, which
one heading, footnote or label is enough to need; or the LaTeX for LuaLaTeX, which always
has its style file) reports an error, prints nothing and
exits with status 1. Here `both` is a workflow, declared by an extension, with the outputs
`html` and `text`:

```text
$ flm render -p both paper.flm
error: the output `html` cannot be written: standard output can receive one file, and this command's outputs (`html`, `text`) hold 2 files (`index.html`, `index.txt`): write them with `-o FILE` or `-o FOLDER/`
1 error, 0 warnings
```

Write such a command with `-o`: `flm render -p latex-recomposed -o paper.tex paper.flm`, or
`-o paper/` for a folder, rather than `flm render -p latex-recomposed paper.flm >
paper.tex`.

**A file two outputs share.** Two outputs may make a file at one path: for example, the
compose templates of two outputs may both ship the stylesheet `style` (see [Compose
templates](#compose-templates)), which both name `paper-style.css`. When the two files
hold the same bytes, they are one file, written once. When they differ, the command
reports an error and writes nothing; rename the file in one of the compose templates:

```text
error: the output `text` cannot be written: the output `html` and the output `text` must not write different content at one path, `paper-style.css`
```

**All or nothing.** The files of a command are written together or not at all: when any
file of any output cannot be named or placed, nothing is written, and the command exits
with status 1.

**How the files are written.** Each file is first written in a temporary folder inside the
output's folder, then moved into place, and the temporary folder is removed. Beside the
main file of each output, a hidden file `.<main file>.flm-written` lists the files written
for that output (`.paper.html.flm-written`, `.paper.txt.flm-written`). The next command
that writes that output removes the files the list holds that it no longer writes. A
command narrowed to one output with `-f` reads and rewrites only that output's list, so the
other outputs' files stay. The one exception is a file that two outputs shared and that the
narrowed output no longer writes: it is removed, and rendering the other output again
writes it back.

### Profiles and workflows

A [*workflow*](crate::core::registry::WorkflowDef) is a named set of demands (the outputs
to produce, and what else to ask of the processing) together with the choices that go
with them. The program's workflows are `html`, `text`, `latex-rendered`, `latex-recomposed`,
`bibtex`, `csl-json` and `check`, and an extension the document uses can declare more.

A *profile* is the name a command builds with. The profile `NAME` runs the workflow
`NAME`, and so determines what kind of output is produced. The profile is the one given
with `-p`. Without `-p`, `flm render` and `flm watch` use the profile that the front
matter's `default_profile` gives, else `html`, and `flm check` uses `check`.

A profile can come with packages of its own, in an extension named `profile-NAME`. Before
the command finds the document's packages, it looks for an extension `profile-NAME` where
it looks for every extension: on the search path, then among the extensions compiled into
the program (see [Where extensions are looked for](#where-extensions-are-looked-for)).

- When the extension exists, it is used after the document's packages, as if the front
  matter listed it last under `use`. An extension `profile-NAME` must declare the workflow
  `NAME`. A document may also list an extension `profile-NAME` under `use`, like any
  extension; it is then used once, and with every profile. A front matter that sets the
  settings of a feature of the extension (`features: {pdf-latex: {timeout_s: 1200}}`)
  lists it so: with another profile, `flm check` among them, no package in use has that
  feature, and the settings stop the command with status 2.
- When no extension `profile-NAME` exists, nothing is added: the workflow `NAME` must then
  be one that the packages the document uses declare. The program's own workflows are such
  workflows, and so is a workflow that an extension listed under `use` declares.
- When the extension exists but cannot be read (a manifest that is not valid YAML, for
  example), the command stops with status 2, as for an extension listed under `use`.

The program's profile `pdf-latex` is the extension `profile-pdf-latex`, compiled into the
program, which declares the workflow `pdf-latex` and its output `pdf-latex`, the PDF file
(see [PDF through LaTeX](#pdf-through-latex)): `flm render -p pdf-latex` needs nothing in
the front matter.

When the packages in use declare no workflow `NAME`, the command stops with status 2. The
error lists the workflows that the packages in use declare, which are the names that work
as profiles without an extension of their own:

```text
$ flm render -p nosuch paper.flm
error: profile `nosuch` not found; ensure that the existing packages define a workflow named `nosuch` or that an extension named `profile-nosuch` is in scope; try perhaps `bibtex`, `check`, `csl-json`, `html`, `latex-recomposed`, `latex-rendered`, `text`
```

When the extension `profile-NAME` is used and no workflow `NAME` is declared, the error is
``the package `profile-NAME` fails to define a workflow named `NAME` ``. The front
matter's `default_profile` is looked for and checked only when it is the profile that
runs: with `-p`, the command uses the profile given there, and the front matter's
profile is neither loaded nor checked.

`-f` never chooses the profile or the workflow: it selects one of the formats that the
workflow that runs provides, and a format that this workflow does not provide stops the
command with status 2, with an error that lists the formats it provides:

```text
$ flm render -f text paper.flm
error: the workflow `html` does not provide the format `text`; it provides `html`
```

The plain text is produced by its own profile: `flm render -p text paper.flm`.

A command produces every output of the workflow, or the one `-f` selects. The program's own
workflows have one output each; a workflow that an extension declares may list several
(`outputs: [html, text]`; see [Extensions](#extensions)), and one command then writes
them all (see [Where the output goes](#where-the-output-goes)).

**Outputs that cannot be produced together.** The outputs of one command are made from one
processing of the document, and some outputs need the document's content prepared in ways
that exclude each other: the HTML page and the plain text need the content laid out in
paragraphs, as does the LaTeX for LuaLaTeX (`latex-rendered`), while the recomposed LaTeX
needs it exactly as the author wrote it. A workflow that lists `latex-rendered` together
with `html` or `text` runs, and one processing makes all of them (an extension's manifest
declares one as `flm: {workflows: {pagepaper: {extends: html, outputs: [html,
latex-rendered]}}}`, see the manifest under [Extensions](#extensions) below; a document's
own front matter cannot declare workflows); a workflow that lists `latex-recomposed`
together with `html`, `text` or `latex-rendered` cannot run: the command stops with status
2 and an error saying that the processing cannot be planned.
Produce such outputs with two commands (`-p html`, then `-p latex-recomposed`), or with two
workflows. The exports `bibtex` and `csl-json` go with any output (`outputs:
[latex-recomposed, bibtex]` writes `paper.tex`, its style file and `paper.bib`).

### Compose templates

A *compose template* is a text with named places for the parts of an output's main file
(its *output regions*). The library makes the parts of that file — the content; the title, the
authors and the date written in the front matter; the lists of notes and the
bibliography; and, in the HTML output, the stylesheets and the scripts — and the compose
template places them. The program has compose templates of its own for each output; the
help of the option `-t` lists their names, for each output:

```text
flm render --help
```

The default, `simple`, makes a document that can be opened as it is: in the HTML output, a
complete page with a header holding the title, the authors and the date, the table of
contents where the render builds it (see [The table of contents](#the-table-of-contents)),
then the content, the lists of notes and the bibliography.

In the HTML output, the content and each part placed beside it are written inside a
`<div>` whose `id` is `flm-region-` followed by the part's name: `flm-region-content`,
`flm-region-flm.meta.title`, `flm-region-flm.toc`, `flm-region-flm.xnotes.list.footnote` (in a CSS selector,
each `.` of such an identifier is escaped: `#flm-region-flm\.meta\.title`). In the
recomposed LaTeX, `simple` places the title, the authors and the date, and leaves the
notes to LaTeX. A part that the compose template does not place is not computed at all.

The compose template is the setting `template` of the feature `compose`: one name for
every output, or a map from an output's name to a compose template's name (`-O
'compose.template={html: none}'`); in the front matter, `flm: {features: {compose:
{template: none}}}`. A compose template's own settings are written under
`template_config`, under the output's name and then the compose template's name, over its
defaults: `-O 'compose.template_config={OUTPUT: {NAME: {KEY: VALUE}}}'`. The output's level
keeps apart two compose templates of the same name for two outputs. For instance, `simple`
for `latex-rendered` has the settings `documentclass` (`article` by default) and
`classoptions` (empty by default): `-O 'compose.template_config={latex-rendered: {simple:
{documentclass: report}}}'`. A name that matches no compose template of the output stops the command
(status 2). The feature `compose` can be switched off like any other (`compose: false`);
the output is then assembled by the backend's own
[final stage](crate::guide::concepts_overview#output-regions-and-the-final-stage), the
library's assembly of the output's main file, which writes a complete page in the HTML
output.

Some outputs are made from the files of another output instead of from the document
itself: an output of PDF files, for instance, is made by compiling the files of the output
`latex-rendered` with LaTeX. When the output asked for is made from another output, the
compose template is chosen for that other output as well, as if it were asked for too:
`-t NAME` then chooses the compose template `NAME` of `latex-rendered`, and so does `-O
'compose.template={latex-rendered: NAME}'`. A name that matches no compose template of
`latex-rendered` stops the command (status 2), with an error that mentions
`latex-rendered`.

The page's title and language are set by the library, under every compose template: the
title is the document's, else the option `html.title` (`-O 'html.title=My page'`; by
default `FLM document`), and the language is the option `html.lang` (`-O html.lang=en`).

**The LaTeX for LuaLaTeX.** The compose templates of `latex-rendered` are written in LaTeX
with the template engine `minijinja`, whose delimiters are changed so that they do not
collide with LaTeX's braces: statements stand between `<<*` and `*>>`, values between
`<<<` and `>>>`, comments between `<<!--` and `-->>` (`\documentclass{<<<
config.documentclass >>>}`). The output is compiled with `lualatex`, twice so that the
page numbers of the table of contents resolve, or with `latexmk -lualatex`; the style file
stops any other engine. The profile `pdf-latex` makes the PDF file in one command,
compiling the output with `latexmk` (see [PDF through LaTeX](#pdf-through-latex)). How a
template changes the look of each element (a theorem, a heading, a list) is described in
[the rendered LaTeX backend](crate::guide::rendering#the-rendered-latex-backend).

**Files a compose template ships.** A compose template of an extension may ship files of
its own, such as a stylesheet, a script or a logo, written beside the output's main file
(see [Extensions](#extensions) for how an extension lists them). Each shipped file has a
name, and is written as `STEM-NAME.EXT`, like every file that goes with an output (see
[Where the output goes](#where-the-output-goes)): the file `style` of a compose template,
from `look.css`, is `paper-style.css` under `-o paper.html`, and `index-style.css` under
`-o site/`. The template's text writes the path of a shipped file as `{{ artifacts.NAME }}`
(`<link rel="stylesheet" href="{{ artifacts.style }}">`); the path is relative to the
folder of the main file, where the shipped file is written. A shipped file can itself be a
template, filled in as the main file is, with the same values (`{{ config.color }}`, `{{
artifacts.logo }}`), before it is written. The program's own compose templates ship no
file.

A compose template may also ship a whole folder of the extension, such as a folder of
icons, as one shipped file: the folder is written beside the main file as `STEM-NAME`,
with no extension, and each of its files under it with its own path (`paper-icons/check.svg`,
`paper-icons/more/cross.svg`), at every depth. The template writes the folder's path as `{{
artifacts.NAME }}`, with no `/` at its end, and a file inside it as `{{ artifacts.icons
}}/check.svg`. The names in the folder that begin with `.` (such as `.DS_Store`) are left
out. The record of the written files lists each file of the folder, so a file removed from
the folder is removed from the output folder at the next render.

**Files a compose template includes.** A compose template of the engine `minijinja` may
build its text from other files of its extension, in two ways:

- `{% include "partials/header.html" %}`, the engine's own statement, fills in the named
  file as a template, with the same values as the file that includes it, and writes the
  result in its place; the included file may include further files;
- `{{ include_raw("dist/script.js") }}` writes the named file's text as it is: nothing in
  it is filled in, and in an HTML page nothing in it is escaped. The file must be text in
  UTF-8. The author of the template is responsible for what such a file expands into.

Both work in the template's text and in a shipped file that is filled in as a template.
The name is a path relative to the folder of the compose template's description, with
`/` between its parts, and it must stay inside the extension's folder. The files are read
when the template is filled in, so a name may be computed (`{% include
"partials/" ~ config.variant ~ ".html" %}`), and `flm watch` builds the page again when
an included file changes. A file that is missing, that leaves the extension's folder, or
that is not text stops the render with an error naming the template and the path:

```text
error: the compose template `notes` for the output `html` cannot be applied: in the artifact `script`: the compose template `notes` includes `../../../dist/x.js`, and there is no such file in the extension (in notes:1)
```

The program's own compose templates include no file.

### The lecture-notes template

The repository ships one extension of its own, `lecturenotes`, in the folder
`extensions/data/lecturenotes/`. Its compose template renders a book-length document,
such as a set of lecture notes, as one HTML page: a sticky bar with the document's title,
the chapter being read and arrows to the previous and next chapter; a sidebar on the
right holding the table of contents; previews of references, terms and citations on
hover; and the reader's position kept across visits. The extension is compiled into the
program (see [Where extensions are looked for](#where-extensions-are-looked-for)): a
document lists it under `use`, and nothing needs to be installed:

```text
flm render -o out/notes.html notes.flm
```

A modified copy of the extension, placed on the search path (for instance in
`_flmextensions/lecturenotes/` beside the document), is used in its place.

The page is written with three files beside it: `notes-style.css`, `notes-script.js` and
`notes-fragment.html`, the page's parts without its frame, for placing the document inside
a site's own page. The template's settings (`chapter_level`, `font_family`, `stylesheets`,
`links`) are written under `compose.template_config.html.lecturenotes`. The file
`extensions/data/lecturenotes/README.md` of the repository documents the page, its
settings, and the use of its stylesheet and script outside FLM.

### PDF through LaTeX

The profile `pdf-latex` produces a document as a PDF file. It makes the LaTeX for LuaLaTeX
(the output `latex-rendered`) and compiles it with the program `latexmk`, which runs
LuaLaTeX as many times as the document needs. The profile is the extension
`profile-pdf-latex` (see [Profiles and workflows](#profiles-and-workflows)), written in
Rust, in the folder `extensions/rust/profile-pdf-latex/` of the repository, and compiled
into the program (see [Where extensions are looked
for](#where-extensions-are-looked-for)). The programs `latexmk` and `lualatex` must be in
one of the folders of the environment variable `PATH`; TeX Live and MacTeX install both
and put their folder on `PATH`.

The front matter needs nothing for it; the command gives the profile:

```text
flm render -p pdf-latex -o out/paper.pdf paper.flm
```

The command writes `out/paper.pdf` and nothing else. The LaTeX files are made in memory
and handed to LaTeX: for each command, the extension makes a fresh folder under the
system's temporary folder, writes the files of `latex-rendered` into it (the main `.tex`
file, its style file and the images), runs

```text
latexmk -norc -lualatex -f -interaction=nonstopmode -file-line-error paper.tex
```

there, reads the PDF back, and removes the folder, whatever happened (Ctrl+C, or another
signal that ends the command at once, may leave it in the system's temporary folder). The
option `-norc` keeps the configuration files of `latexmk` (`~/.latexmkrc`, for instance)
from changing the engine or the place where the PDF is written. LaTeX never runs with
`-shell-escape`, the option that would let the document start any program. To keep the
LaTeX files, render them with `-p latex-rendered`.

The extension declares the workflow `pdf-latex`, the profile's workflow: the workflow
`latex-rendered` with the extension's output `pdf-latex`, the PDF file, in place of
`latex-rendered`, and one setting more: the strategy `copy` for the images of
`latex-rendered` (see [Images](#images)). Each image is then among the LaTeX files, as
LaTeX runs in the temporary folder, where an image referred to by its path from the
output's folder cannot be read. The workflow's settings rank above the front matter (see
[Ranks of the settings](#ranks-of-the-settings)), so `-O` can change this strategy and the
front matter cannot. A compose template given with `-t` applies to `latex-rendered`, from
whose files the PDF is made (see [Compose templates](#compose-templates)).

The setting `timeout_s` of the feature `pdf-latex` is the time, in seconds, that `latexmk`
may take, 600 by default: `-O pdf-latex.timeout_s=1200`, or `flm: {features: {pdf-latex:
{timeout_s: 1200}}}` in the front matter. A run that takes longer is stopped and reported
as an error. On Linux, macOS and other Unix systems, the programs that `latexmk` started
(LuaLaTeX) are stopped with it, and so they are when a signal ends the command (see [Exit
status of `render` and `check`](#exit-status-of-render-and-check)); on Windows, only
`latexmk` itself is stopped at the end of its time, and a LuaLaTeX run it started goes on
until it ends.

**When LaTeX fails.** When `latexmk` exits with a status other than 0, is stopped, or is
not found on `PATH`, the command reports an error and exits with status 1. The error of a
failed compilation quotes the first error that LaTeX's log records, with the file and the
line of the `.tex` file (which `-p latex-rendered` writes), or the last lines of the
output of `latexmk` when the log records none:

```text
error: LaTeX must compile `paper.tex` without an error; `latexmk` exited with the status 1; the log records:
    ./paper.tex:3: Undefined control sequence.
    l.3 \foo
```

LaTeX goes on past an error (`latexmk` runs without `-halt-on-error`), and `latexmk` goes
on running LaTeX after a run that reported an error (the option `-f`), as many times as
the document needs, so that the page numbers of the table of contents and the other
cross-references resolve. When LaTeX produces a PDF all the same, the PDF is written. A
document with errors stops before LaTeX runs, as it does for every output (see [Documents
with errors](#documents-with-errors)); with `--continue-with-errors`, LaTeX runs and the
PDF is written.

**In the live preview.** `flm watch -p pdf-latex paper.flm` runs LaTeX at each build and
serves the PDF at the preview's address, where the browser shows it in its own viewer.
The PDF is not updated in place: load the address again to see the last build.

**The WebAssembly form.** The same extension also builds as a WebAssembly module, which a
program that does not carry it compiled in finds on its search path. The script
`extensions/rust/build-wasm.sh` of the repository builds it (see the file
`extensions/README.md`).

### The table of contents

The table of contents lists the document's headings (the [language syntax
guide](crate::guide::language_syntax#the-table-of-contents) says which headings, and what
an entry shows). It is off by default. Turn it on with the setting `include` of the
feature `pieces`, which lists parts to place beside the content in addition to those
placed by default:

```text
flm render -O 'pieces.include=[flm.toc]' paper.flm
```

or in the front matter, `flm: {features: {pieces: {include: [flm.toc]}}}`. The setting
`depth` of the feature `toc` is the deepest heading level listed, 3 by default (sections,
subsections and subsubsections): `-O toc.depth=2`, or `flm: {features: {toc: {depth: 2}}}`
in the front matter, where `-O` takes precedence.

- In the HTML output, the compose template `simple` writes the table of contents between
  the page's header and `<main>`, as a `<nav>` of nested lists, each entry a link to its
  heading, inside `<div id="flm-region-flm.toc">`; the live preview (`flm watch`) updates that
  part in place when the headings change, as it does every placed part. Without a page
  around the parts (`-t none`), it is written before the content, after the title, the
  authors and the date.
- In the text output, it is printed before the content, after the title, the authors and
  the date.
- In the recomposed LaTeX, nothing is written for it: the LaTeX document builds its own
  table of contents with `\tableofcontents`, which no compose template of the program
  writes.

### Documents with errors

A document with errors writes no output: the program stops before writing the output's
files.
`--continue-with-errors` writes the output anyway, and the command still exits with
status 1.

### Links

The address of a link (`\href{appendix.pdf}{the appendix}`) is written in every output
as the author wrote it: it refers to a place that a reader reaches from the published
output, which the program cannot know.

### Images

`\includegraphics{figures/plot.png}` refers to an image file, which the program finds,
measures and publishes in each output.

*Where it is looked for.* The path as written is looked for in each folder of the
setting `search_path` of the feature `graphics-files`, in order; the first file found is
used. Each folder is relative to the folder of the file that refers to the image (the
document, or a file it brings in with `\input`), or absolute; for a text given with `-c`
or on standard input, it is relative to the current folder. The default is that folder
alone, `.`. No extension is added to a path that has none. A list of folders is written
`-O 'graphics-files.search_path=[figures, .]'`, or in the front matter `flm: {features:
{graphics-files: {search_path: [figures, .]}}}`. A file found in no folder is an error
at the image, and the message lists the folders searched. Every path looked for, found
or not, is among the files that `-v` lists as read.

*What is measured.* The size of an image is read from its file: for a PNG, JPEG, GIF or
WebP image, its size in pixels and the resolution its file states (a PNG's physical-size
chunk, a JPEG's JFIF header or else its EXIF data), rounded to 1/128 dot per inch; a file
that states no resolution is taken at 96 dots per inch, with a warning, and one that
states two different resolutions for its two directions is an error. For an SVG drawing,
its `width` and `height`, in `pt`, `px` (1/96 inch), `in`, `cm`, `mm` or `m`, a number
with no unit being points. For a PDF file, the size of its first page. A raster image's
size in points is its size in pixels times 72, divided by its resolution.

*How it is published.* Each output reaches its images by one of three *strategies*, set
by the setting `strategy` of the feature `graphics`, a map from an output's name to a
strategy:

| strategy | the output writes | the program's default for |
|---|---|---|
| `reference` | the file's path as seen from the folder the output is written in: `../figures/plot.png` for `-o site/paper.html` | `text`, `latex-rendered`, `latex-recomposed` |
| `copy` | a copy written beside the output, named after the stem, the file and a digest of its content (`paper-plot-3fa9c2d1.png` for `-o paper.html`, `index-plot-3fa9c2d1.png` for `-o site/`) | — |
| `data-uri` | the file's content, in the page itself (`data:image/png;base64,…`): the page needs no other file | `html` |

`-O graphics.strategy.html=copy` sets one output's strategy and leaves the others'; so
does a front matter's `flm: {features: {graphics: {strategy: {latex-recomposed:
copy}}}}`, which leaves the page's `data-uri`. The plain text is one file and cannot hold
a copy, and only a web page can show a `data:` address: either request is an error at
the image, which is then published by reference. An image given by a web address
(`https://…`) is written unchanged under every strategy, and is neither read nor
measured.

*Conversions.* Under `copy` and `data-uri`, the setting `convert` of the feature
`graphics`, a map from an output's name to a list of rules, turns an image of one format
into another. A rule states the formats it applies to (`from`, one or a list of `png`,
`jpeg`, `gif`, `webp`, `svg`, `pdf`), the format it makes (`to`), optionally the
converters allowed to make it (`via`, by name), and options (`dpi`, the resolution at
which a drawing is turned into pixels; `transparent_bg`). The program's rules are: for
`html`, a PDF drawing into a PNG image at 192 dots per inch; for `latex-rendered` and
`latex-recomposed`, an SVG drawing into a PDF file and a GIF image into a PNG image, which take effect only
where that output's strategy is `copy`, since only a copy can carry new content. A rule
is performed by the first converter, in the order below, that its `via` allows (every
converter when `via` is not written) and that can make its pair of formats; a converter
that another feature contributes is asked before these when the feature order places
that feature after `graphics-files`, and after them otherwise:

| converter | converts | how |
|---|---|---|
| `raster` | a PNG, JPEG, GIF or WebP image into a PNG image | in the program itself |
| `magick` | any format into any other | by running ImageMagick's program `magick`, which must be installed |

So a PDF or SVG drawing is converted by ImageMagick, and a raster image is converted into
a PNG image by the program itself unless a rule says `via: [magick]`. Of a PDF file of
several pages, or of an animated image, the first page or frame is converted. The
environment variable `MAGICK`, where it is set, gives the program to run
(`MAGICK=/opt/imagemagick/bin/magick flm render paper.flm`): a name with no folder, which
the system looks for in its folders of programs, or a path, a relative one being taken
from the current folder, as at a command line. Where it is not set, the program is
`magick`, looked for in the folders of programs. No setting gives the program, so a
document cannot choose a program that `flm` starts. The program is started only for an
image that needs it, and its result is kept while `flm` runs, so an image shown twice is
converted once. On a machine without ImageMagick, or where the program fails, the image
that needed it is an error, whose message shows what the program wrote, and the image is
published unconverted: a PDF figure in the page then shows no picture.

*Magnification.* The setting `magnification` of the feature `graphics`, a map from an
output's name to `{raster: …, vector: …}`, multiplies the size of every image shown on that
output: `raster` for a PNG, JPEG, GIF or WebP image, `vector` for an SVG or PDF drawing and
for an image that was not measured. Each factor is a number greater than zero, 1 where not
given; it multiplies a `width=` or `height=` the author gave too. `-O
'graphics.magnification.html={raster: 1.5}'` shows raster images half as large again on
the page, and leaves the other outputs as they are. An image given neither `width=` nor
`height=` whose file was not measured (a web address, or a file that could not be
measured) is reported with the warning `flm.graphics.unknown-size`, and is written without
a size.

### Citations

`\cite{arxiv:1211.1037}` cites a work by a *citation key*. The part of the key before its
first colon, its *prefix*, trimmed and in any letter case, chooses the *record source*
that gives the work's *record*: its details as data (authors, title, journal, year) in
the format CSL-JSON. The rest of the key is handed to the record source as written. The
record sources are the setting `sources` of the feature `citations-standard`, a map from
a prefix to a record source; there are four by default:

| prefix | the rest of the key | the record |
|---|---|---|
| `arxiv` | an arXiv identifier, `1211.1037` | the arXiv's; where the arXiv gives the DOI of the published version, the published version's, from doi.org |
| `doi` | a DOI, in any letter case | doi.org's |
| `manual` | the citation entry itself, in FLM: `\cite{manual:{A. Author, Some title (2024)}}`; a comma inside braces belongs to the key, and the key may hold FLM commands, `\cite{manual:\emph{Some title}}` | none: the key is shown |
| `bib` | the name (`id`) of a record in the bibliography files | the file's |

A key with no colon, or whose prefix matches no record source, has no citation entry,
which is an error at the citation, unless a citation provider that an extension adds
answers it: an extension written in JavaScript may supply citation entries of its own,
from a source of its own such as a file beside the document (see [A citation provider
from a script](crate::guide::extending#a-citation-provider-from-a-script) in the guide to
extending FLM). Two spellings of one work's key (`arXiv:1211.1037`
and `arxiv:1211.1037`, or a DOI in two letter cases) are one cited work, with one
number. A record source is added under a prefix of its own, written in lowercase and
holding no colon, and removed with `false`, in a package of an extension or in the front matter; the
other record sources stay. A second set of bibliography files under the prefix `thesis`
is written

```yaml
flm:
  features:
    citations-standard:
      bibliography: [refs.yaml]
      sources:
        thesis: {kind: bib, files: [theses.yaml]}
```

Each record source states its `kind` (`arxiv`, `doi`, `manual` or `bib`). A record
source of the kind `arxiv` also takes `chain_dois_to`, the prefix of the record source
of DOIs from which the published version is fetched (the only record source of the kind
`doi` when not written; `null` keeps the arXiv's record); `override_dois`, a map from an
arXiv identifier to the DOI to use for it (`null` for none); and `override_dois_file`,
the same map in a JSON file. A preprint and its published version cited side by side
(`\cite{arxiv:1211.1037}` and `\cite{doi:10.1038/ncomms8669}`, where the arXiv gives that
DOI as the preprint's published version) remain two works with two citation entries,
since an author may mean both; a Warning at the preprint's first citation gives the two
keys.

*Bibliography files.* The setting `bibliography` lists the files that a record source of
the kind `bib` reads when it lists none of its own (`files`), in CSL-JSON or CSL-YAML: a
list of records, each giving its name in its `id`, or a map from a name to a record. For
a record of one name, a file listed later wins over an earlier one. A path is relative
to the folder of the file it is written in (the document, for its front matter; a data
package's file, for that package), or to the current folder for an `-O` setting. The
files are read again at every build, so a file changed between two builds changes the
citation entries, and they are among the files that `-v` lists as read. A record may
carry its citation entry ready-made: in FLM, under the field `_ready_formatted: {flm: …}`
(or the older `_formatted_flm_text`), shown as FLM; under another markup's name alone,
shown as the characters written.

*The cache.* The records fetched from the arXiv and doi.org are kept in the file
`.flm-citations.jsonl` beside the document (in the current folder for a document given
as text), each for ten days (the arXiv's) or 360 days (doi.org's), and fetched again after
that; the records of bibliography files and `manual` keys are never written there. The
file is plain text, one record per line, in an order that keeps its changes small: kept
under version control, it lets the document be rendered without the network. While it
runs, `flm` also writes files whose names begin with `._.flm-citations` there, and
removes them: one rule, `._.flm-citations*`, keeps them out of version control. The
fields `abstract` and `reference` of a record (doi.org's list of every work the article
cites) are left out of what is kept: the setting `dropped_fields`.

*Fetching.* The arXiv is asked for up to a hundred keys in one request, doi.org for one
key per request; the program waits about three seconds between two requests to the
arXiv and one second between two requests to doi.org, as they ask, and tries a request
again when the server does not answer. Every request gives the program's name, the
setting `user_agent` (`flm/<version> (+<address>)` by default), where an address to
write to is welcome. Under `-v` the program prints on standard error each pass over the
keys, each request and each wait. On a machine with no network, a key whose record is
not in the cache has no citation entry, which is an error at the citation that says why;
the keys of bibliography files and `manual` keys still have theirs. On such a machine,
or against a server that refuses requests, the retries can take a few minutes before
each key that is not in the cache fails; `-v` shows each wait.

*The citation entries.* A record that carries no ready-made citation entry is set as one
in a *citation style*, a file of the Citation Style Language (CSL) that says in which
order and in which type the authors, the journal, the volume and the year are written;
the formatter is the Rust crate `hayagriva`. By default the citation style is the one
the program ships, "American Physical Society - et al." as patched for FLM: `A. R.
Calderbank and P. W. Shor, Physical Review A **54**, 1098 (1996).`, with the volume in
bold, the year last, in parentheses, and `et al.` after the first author when there are
more than six. The setting `style` chooses another: `{file: my-style.csl}`, a style file,
relative to the folder of the file it is written in and read again at every build, or
`{name: ieee}`, a style of the formatter's archive by its name (`ieee`,
`american-physics-society` and many more). A name that the formatter does not
know is an error before the document is read; a file that is not a citation style is an
error at each citation, and so is a citation style with no `<bibliography>` part, which
says how an entry is set. A citation entry depends on its record and the citation style
alone, never on the other works cited. The number that a numeric citation style writes
at the head of an entry is left out, since the list shows its own. Italic and bold runs
become emphasized and bold content, and every other kind of type (small capitals,
superscript) shows its characters plainly. The characters of a record are shown as they
are, `%`, `&`, `#` and braces included, except a formula between two dollar signs,
`$x^2$` in a title, which is an inline formula regardless of the document's
`dollar_math` setting; a formula that does not parse is an error at the citation. A
record whose date is a range (`1997–2020`) has no citation entry, which is an error at
its citation: the formatter cannot set one.

*The links.* After the citation entry, each after a space, come a link showing `DOI` to
the work's DOI at `https://doi.org/`, a link showing the arXiv identifier to the work's
page on the arXiv, and a link showing `URL` to the record's address. The settings
`doi_link` and `arxiv_link` (each `true` by default) and `url_link` control them;
`url_link` is `only-if-no-other-link` by default (the address is linked where the record
has neither a DOI nor an arXiv identifier), or `always` or `never`. A citation entry that
the record carries ready-made gets no link.

*The exports.* `flm render -p bibtex paper.flm` writes the records of the works the
document cites as a BibTeX file, the file a LaTeX document's bibliography reads, and `-p
csl-json` writes them as a CSL-JSON file. Each is an output of its own, printed on
standard output without `-o`, and written as `index.bib` or `index.json` in the folder
that `-o out/` gives. Each holds one record per key that the document cites, in the
order of first appearance, under the key as written (`bib:knuth84`), which is the key
that the LaTeX written by the program cites; a work cited under two spellings of its key
has two records holding the same details. The BibTeX file writes each field of a record
that BibTeX knows, with the characters special to LaTeX written as their commands and a
formula between dollar signs written as a formula; the DOI, the arXiv identifier and the
address, which BibTeX hands to LaTeX unchanged, are written character for character. A
record with no `issued` date takes its `published` date. A cited key with no record is
an error at its citation, and its work is left out of the file. Every check of the
document runs too, as for a page.

### The program's defaults

The program sets its own values for a few settings, in place of the library's defaults:
footnotes marked a, b, c, with the heading above their list at level 1; figures captioned
`Fig. I` and tables `Tab. I`; the heading above the list of cited works at level 1, as
the footnotes' is; the image conversion rules above; and the `data-uri` strategy for the
images of the HTML page, so that the page is one file that shows its images wherever it
is sent. Apart from `data-uri`, these values are those of the earlier Python
implementation of `flm`. A program that embeds the
library without the host keeps the library's defaults (every image by reference, no
conversion rule). A document's front matter or `-O` changes any of them.

## Checking a document

`flm check INPUT` checks a document and prints its problems, without writing any output.
It runs every check of the program, the reading of the metadata among them. It takes the
options of [What to build](#what-to-build). Its profile is `check` unless `-p` gives
another, so `flm check -p html paper.flm` runs the checks that a render to HTML
runs.

A few problems are found only while an output is made, and a render reports them where a
plain `flm check` does not: among them, a text of the metadata that the document cannot
show (such as a footnote in the title), and an output that cannot be written. Problems
with images and exports are among them too: a plain `flm check` says nothing of an image
file that is not found or cannot be read, of a conversion that fails or of a converting
program that is missing, nor of a cited work with no record when a bibliography is
exported, so a document whose figure refers to a missing file passes with status 0. `flm
check -p html` (or `-p bibtex`, or any other profile) reports them, as a render with that
profile does.

## Problems reported by render and check

`flm render` and `flm check` print every problem of the document on standard error, in
the order found, duplicates included, and a last line gives the number of errors and of
warnings. Each problem is a [*diagnostic*](crate::guide::concepts_overview#diagnostics):
it reports a problem with a document, with a severity (error, warning or note), a
position, and a message written for a person.

```text
error: cannot resolve command ‘\nosuchmacro’
  at paper.flm:12:7
  inside paper.flm:12:7: unknown macro ‘\nosuchmacro’
1 error, 0 warnings
```

The `at` line gives the first position, which is always in the author's own file: the
place of the problem itself, or, for a problem in content that the program made from the
author's text (the expansion of a macro that the document defines), the place in the
file from which that content was made. Then come the problem's traceback lines, one
`inside …` line each: each group or argument inside which the parser found the problem
(the first is often at the same place as the `at` line, as above), and every place where a copy of the content is shown. Last comes the position in
the content the program made, when the problem is there (`in content made from it: …`).
Positions count lines from the top of the file, the front matter included. Nothing is
printed for a document with no problem.

A problem that stops the command before the document is built (an unreadable front
matter, a missing package, a setting that cannot be read) is printed the same way, as an
`error:` line followed by an `at` line where the problem has a place, with no count line.

### Problems as JSON records: `--json-diagnostics`

The option `--json-diagnostics` makes the program write on standard error one JSON object
per line, a *record*, in place of text for a person, so that an editor or another program
can read the problems. It is accepted by every command but `flm dump`, before or after the
command's name (`flm --json-diagnostics check paper.flm` and `flm check --json-diagnostics
paper.flm` are the same). Standard output and the exit status do not change.

```text
$ flm check --json-diagnostics paper.flm
{"kind":"diagnostics","flm":"0.5.0","format":1}
{"kind":"diagnostic","severity":"error","id":"core.specs.unresolvable-command","data":{"name":"nosuchmacro","escape_char":"\\","detail":null},"message":"cannot resolve command ‘\\nosuchmacro’","place":{"file":"paper.flm","offset":47,"line":4,"column":27,"end":{"offset":59,"line":4,"column":39}},"frames":[{"place":{"file":"paper.flm","offset":47,"line":4,"column":27,"end":{"offset":59,"line":4,"column":39}},"title":"unknown macro ‘\\nosuchmacro’"}]}
{"kind":"diagnostic","severity":"warning","id":"flm.refs.unresolved","data":{"label":"sec:missing"},"message":"the reference points at the label `sec:missing`, which is not defined","place":{"file":"paper.flm","offset":25,"line":4,"column":5,"end":{"offset":42,"line":4,"column":22}},"frames":[]}
{"kind":"summary","errors":1,"warnings":1}
```

The first line is a header: its `kind` is `diagnostics`, `flm` is the program's version, and
`format` is the version of the records' form, a whole number. Each following record has a
`kind`:

| `kind` | what it reports | its other keys |
|---|---|---|
| `diagnostic` | a problem in the document | `severity` (`error`, `warning` or `note`), `id` (the identifier of the problem's condition) and `data` (its details), `message` (for a person; its wording may change), `place`, `frames` (the traceback: each an `inside` line of the text form, with its `place` and `title`), and `made_from` (the place in content the program made, when the problem is there) |
| `problem` | a problem that the program reports outside the document's diagnostics: a problem that stopped the command before the document was processed (the severity `error`, with a `place` where the problem has one); an error of the program or of its arguments, such as a path given to `flm info` that matches no page (`error`, no place); or a warning of the program that does not stop the command, such as the warning of `flm info` about a documentation text that it prints as plain text (`warning`, no place; see [Documentation texts that are not valid FLM](#documentation-texts-that-are-not-valid-flm)) | `severity` (`error` or `warning`), `message`, `place` |
| `summary` | the counts that end a report | `errors`, `warnings` (notes are not counted) |
| `build` | the start of a build of the live preview (`flm watch`) | `number` (from 1), `time` (milliseconds since 1970-01-01 00:00 UTC) |
| `log` | a line that tells what the program does: the files `-v` lists, a line an extension writes on its console (`[js log] …`), the preview's address | `text` |

A *place* gives the `file` as the author wrote it (or as the program found it, for a file
included with `\input`), the `line` counted from 1 at the top of the file, the front matter
included, and the `column` counted from 1 at the start of the line, in bytes. In a
file's content (the text below its front matter), a place also gives the `offset`, the
byte offset counted from 0 at the top of the file, and the `end` of the range it covers.
A place in a front matter or in an extension's manifest has no `offset` and no `end`, and
neither does a place in the document's title, author or date, which come from the front
matter. A place in content the program made gives its `label` alone. A key with no value is
left out.

A few lines stay text: the usage message the program prints for a command line it cannot
read, everything `flm dump` prints, what an extension compiled into the program writes on
its console (`[native log] …`), and the message for a compiled package that cannot be made,
which the program prints before it reads its arguments.

## Exit status of `render` and `check`

| status | when |
|---|---|
| 0 | the command did what was asked, with or without warnings |
| 1 | the run reported errors: errors in the document, an output that could not be written (more than one file sent to standard output, two outputs writing different files at one path, or a file the disk refuses), or a written output that `-V` could not open; the status is 1 whether or not `--continue-with-errors` wrote the output |
| 2 | the command could not start the work: wrong arguments, an input that cannot be read, a front matter that cannot be read, a name that matches no package, profile, output or compose template, a setting that cannot be read, or a registry or plan that cannot be built (including a workflow whose outputs cannot be produced together; see [Profiles and workflows](#profiles-and-workflows)) |
| 3 | a failure of the program itself |

On Linux, macOS and other Unix systems, a signal that asks the program to end (the
interrupt that Ctrl+C sends, the quit signal that `Ctrl+\` sends, the hang-up sent when the
terminal closes, or `SIGTERM`) ends the command by that signal, as it ends a program that
does not handle it: a shell reports 128 plus the signal's number (130 for Ctrl+C), and a
script stops there. The programs that an extension was running, such as `latexmk` under
`-p pdf-latex`, are stopped first. A signal that the program's parent left ignored stays
ignored: under `nohup`, closing the terminal does not end the command.

## Configuration files and document front matter

A document's settings come from the program's defaults, from the packages of the
*extensions* the document uses, from the document's *front matter*, from the workflow that runs and from
`-O` on the command line. This chapter describes where each is written and which one
wins.

### The front matter

A document may begin with a *front matter*: a block of YAML between a first line `---`
and the next `---` line. Its root keys are the author's *metadata*: the title, the
authors, the date, and any other key the author writes there. Its one reserved key,
`flm`, holds the document's settings:

```yaml
---
title: A paper
author: [A. Author, B. Author]
flm:
  use: [std]
  features:
    text: {width: 60}
    theorems: false
  parsing: {dollar_math: true}
  feature_order:
    - headings < toc
  layout: blocks
  default_profile: html
---
```

| key under `flm` | what it holds |
|---|---|
| `use` | the packages the document uses: one entry, or a list of entries, each naming a compiled package or a package of an extension (see [Extensions](#extensions)); they are used after the packages of the initial list and the math processor's, and `--use` adds a package after them |
| `initial_use` | the packages used before those of `use`, in place of the program's initial list `std`: one package's name, or a list, used exactly as written; `--initial-use` takes precedence over it (see [The packages a document uses](#the-packages-a-document-uses)) |
| `math` | the math processor, the package that processes the document's mathematics: a processor's name, `NAME`, which uses the package `math-NAME` (`mathjax`, `passthrough`), or `false` for no processor; without it, the program's default processor, which is not guaranteed to stay the same; `--math` takes precedence over it (see [The packages a document uses](#the-packages-a-document-uses)) |
| `features` | a map from a feature's name to its settings, or to `false` to switch the feature off |
| `parsing` | how the content is read: `comments` (whether comments are recognized; default `true`), `comment_prefix` (what starts a comment; default `%%`), `dollar_math` (whether `$…$` and `$$…$$` delimit mathematics; default `false`) |
| `feature_order` | the order of features: one chain, or a list of chains, each two or more features' names joined by `<` (see "The order of features" below) |
| `layout` | how the content is read at its top level: `blocks`, `inline` or `auto`; `--layout` takes precedence over it |
| `default_profile` | the profile to build with when the command gives none with `-p`: a profile's name; the extension `profile-NAME` is used when it exists, and the workflow `NAME` runs; `-f` then selects one of the formats it provides (see [Profiles and workflows](#profiles-and-workflows)) |

A key under `flm` other than these eight stops the command with status 2, at its place in
the file. A file brought in with `\input` may write only `parsing` and `input_as` under
`flm`.

**The order of features.** The features of the packages a document uses are placed in an
order: the program's own first, then those of each package listed under `use`, each
package after the packages it uses, each package's features in the package's order. That
order decides which definition applies where two features define one command, which
feature's contribution wins where two compete (an image converter, a citation provider),
and the order in which features number things. `feature_order` changes it. It holds
*chains*: each chain is a text of two or more features' names (not packages' names)
joined by `<`, and places each feature it names after the one before it:

```yaml
flm:
  use: [phfpack]
  feature_order:
    - math-mathjax < phfqit
    - headings < mycustomsectioncommands
```

A chain says something about every pair it names: `a < x < b` places `b` after `a`, which
the two chains `a < x` and `x < b` do too, but only through `x`. A feature that is not in
use (switched off, or in no package the document uses, or misspelled) is left out of a
chain, and the rest of the chain still applies: with `x` not in use, `a < x < b` places
`b` after `a`. The chains of the front matter and those of every package in use (a
package of an extension writes its own under `flm.feature_order`) all apply together.
Chains that place features in a circle (`a < b` in a package, `b < a` in the front
matter) stop the command with status 2, with a message naming the chains and where each
is written. The program then places the features one at a time: each time, among the
features whose predecessors in the chains are all placed, the one that comes first in
the order above. Two features that no chain orders may still change places: with the
order X, Y, Z and the chain `Z < X`, the features are placed Y, Z, X.

Where two features define one command in the same parsing mode (text or mathematics),
the definition of the feature placed last applies, provided the chains place that feature
after every other feature that defines the command in that mode, or that feature
declares its definition as an override. Otherwise the command stops with status 2, and
the message names the features and the chain that would choose the last one's
definition.

**Mathematics between dollar signs.** With `dollar_math` off, its default, `\(…\)` and
`\[…\]` delimit mathematics, and a `$` in the content is an error, reported as

```text
error: ‘$’ must not be used here, because the parsing settings do not make it a mathematics delimiter: write \(…\) for inline mathematics or \[…\] for display mathematics
```

Write the formula with `\(…\)` or `\[…\]`, or set `flm: {parsing: {dollar_math: true}}`
in the front matter (or under `flm.parsing` in an extension's manifest), which makes
`$…$` and `$$…$$` delimit mathematics too.

**Switching features off.** Any feature may be switched off, the program's own included.
The feature `write` of the package `stdmain` hands the output's files to standard output, a
file or a folder. With `write: false`, `flm check` reads the document as usual, and `flm
render` stops with status 2 and an error at the place where `write: false` is written,
since it has an output to write. A document chooses the package that processes its
mathematics with `math` ([The packages a document uses](#the-packages-a-document-uses));
switching that package's feature off (`math-mathjax: false`) leaves the document with no
math processor, and every workflow then stops with an error
([Mathematics processed by MathJax](#mathematics-processed-by-mathjax)).

`false` for a feature that no package the document uses has is accepted, with a warning
at the place where it is written, since the name may be misspelled: `` `mathjx` is
switched off, but no package in use has a feature `mathjx` ``. The warning is printed
also when an error stops the work, before that error. Settings written for such a
feature, an empty map included, are an error.

**Metadata.** `title` and `date` are each one text; `author` is one text or a list of
texts, one per author; a number is read as the text it is written as (`date: 2026`). Each
is FLM, not plain text (`title: The \emph{Art} of \(x^2\)`), and a problem in it is
reported at its line in the file. The output shows the title, the authors and the date
before the content. Every other root key is kept, and the output does not show it. In a
YAML text that is not quoted or is quoted with `'`, a backslash is written as it is; in a
text quoted with `"`, it must be doubled (`"\\emph{Art}"`). A title, date or author that
is not a text stops the command with status 2, at its place in the file.

### Extensions

Settings shared by several documents, and features that the program does not carry, come
from *extensions*. An extension is a file, or a folder of files, that the program finds by
its name on the search path (see [Where extensions are looked
for](#where-extensions-are-looked-for)) and that provides one or more packages. A
document uses a package of an extension by listing it under `use`, as it lists a compiled
package; a command adds one with `--use`. The *engine* of an extension is the part of the
program that reads the extension's files and turns them into packages:

- the engine `data` reads an extension written as data: settings of features, parsing
  settings, workflows and compose templates, never behavior; it provides one package;
- the engine `js` runs an extension written in JavaScript, whose features declare
  commands, environments, tables and processing stages, and output formats of their own,
  and which may provide several packages;
- the engine `wasm` runs an extension compiled to one WebAssembly module (from Rust,
  typically), whose features declare the same things as a JavaScript extension's;
- the engine `native` runs an extension written in Rust and compiled into the program
  itself, whose features declare the same things as a WebAssembly module's. The program's
  own extensions of this kind (`profile-pdf-latex`, see [PDF through LaTeX](#pdf-through-latex))
  are found by their names, as the other extensions shipped with FLM are; the same source
  also builds as a WebAssembly module, which the engine `wasm` runs.

A workflow that an extension declares runs as the profile of the same name, as the
program's own workflows do (`flm render -p typst paper.flm`), and `-f` selects one of its
outputs, an output that an extension declares included. An extension named `profile-NAME`
is used by the profile `NAME` alone, without being listed under `use` (see [Profiles and
workflows](#profiles-and-workflows)).

**The two layouts.** An extension named `NAME` is either the file `NAME.yaml`, or the
folder `NAME/` holding the file `flmextension.yaml` and the extension's other files: its
compose templates under `templates/`, the modules of an extension written in
JavaScript, and the module of an extension compiled to WebAssembly. The file `NAME.yaml`, or `NAME/flmextension.yaml`, is the extension's
*manifest*. An extension written as one file has no other files.

**The manifest.** The manifest is YAML. Its root is a map of two keys, each optional:
`extension` says what the extension is and which engine reads it, and `flm` holds the
package of an extension of the engine `data`:

```yaml
# mystyle.yaml
extension:
  description: The house style.
flm:
  use: [journal]
  features:
    text: {width: 72}
    theorems: false
  parsing: {dollar_math: false}
  workflows:
    draft:
      extends: html
      settings:
        html: {heading_tags: {section: h3}}
      docs: The HTML output, with smaller headings.
```

| key | what it holds |
|---|---|
| `extension.description` | a text describing the extension; it is also the title under which the extension's compose templates are listed |
| `extension.author` | a text |
| `extension.flm_version` | the versions of FLM the extension works with, as a version requirement (`>=0.5`, `^0.5.0`); a program whose version does not meet it refuses the extension |
| `extension.engine` | the engine that reads the extension: `data`, `js` or `wasm`. When the key is absent, the engine is `data`. The engine `native` is the engine of the extensions written in Rust that the program carries compiled in; a manifest that specifies it for an extension that the program does not carry is refused |
| `extension.js` | for the engine `js` only: `main`, the path of the main module inside the extension's folder (`{main: main.js}`), and `limits`, optional, the limits of every engine instance that runs the extension's scripts, each in place of the program's: `memory_mib` (memory, in MiB), `stack_mib` (the script's stack, in MiB) and `time_s` (the time one call into a script may take, in seconds) |
| `extension.wasm` | for the engine `wasm` only: `module`, the path of the WebAssembly module inside the extension's folder (`{module: wordcount.wasm}`), and `limits`, optional, the limits of every instance of the module, each in place of the program's: `memory_mib` (memory, in MiB) and `fuel` (the work one call into the module may do, in units of about one instruction) |
| `flm.use` | the packages the extension's package uses: one `use` entry, or a list of them |
| `flm.features` | settings of features, as under `flm.features` in a front matter |
| `flm.parsing` | parsing settings, as under `flm.parsing` in a front matter |
| `flm.feature_order` | chains of the order of features, as under `flm.feature_order` in a front matter: one chain, or a list of chains (`[math-mathjax < phfqit]`) |
| `flm.workflows` | a map from a workflow's name to its fields: `extends` (the workflow it starts from), `outputs`, `demands`, `prefer` (each a list of names), `settings` (settings that are set per render, by feature) and `docs` |

The engine `data` alone reads the key `flm`; a manifest that names another engine and
writes `flm` is refused. Any other key, at the root, under `extension` or under `flm`,
is refused (`layout` and `default_profile` included: a document's front matter alone
writes them). A problem in a manifest stops the command with
status 2, at its place in the manifest.

A workflow that an extension declares under the name of an existing workflow (`html`,
for example) is that workflow, with the fields the package writes replacing its own; any
other name declares a new workflow, which the profile of the same name runs.

**The `use` entries.** An entry under `use` names a package. It is written `LOCATOR` or
`LOCATOR#PACKAGE`:

- the *locator* names the extension (or a compiled package): segments of letters,
  digits, `-` and `_`, joined by `/` (`mystyle`, `journals/aps`). The first segment may
  begin with `@` (`@acme/style`); YAML reads a text that begins with `@` only when it is
  quoted, so such an entry is written `use: ['@acme/style']`;
- `#PACKAGE` names one package of the extension. An entry without it names the
  extension's default package, `_`: `mystyle` is the same as `mystyle#_`. An extension
  of the engine `data` provides the package `_` alone;
- a locator is a name, never a path: an entry that begins with `./`, `../` or `/`, or
  that holds `\`, `.` or `..` as a segment, is refused. To use an extension stored next
  to a document, put it in the folder `_flmextensions` beside the document, or in another
  folder of the search path;
- an entry that holds `:` is refused: the character is reserved.

The same entries are written in a front matter, under `flm.use` in a manifest, and in
the package lists of an extension written in JavaScript.

**Compose templates.** An extension written as a folder may ship compose templates. The
engine `data` reads them in `templates/OUTPUT/`, the folder of the compose templates for
the output `OUTPUT` (`templates/html/`). Each compose template there is written in one of
two forms:

- the folder `NAME/`, holding the compose template's *description* `flmtemplate.yaml`
  and the compose template's own files: its text, the files it ships, the files it
  includes;
- the file `NAME.yaml`, the description alone, for a compose template that has no text
  (such as one of the template engine `content-only`, which writes the output's parts
  with nothing around them).

A compose template that has a text is best written as a folder, so that its files stay
apart from those of the extension's other compose templates. The compose template's name
is `NAME` in both forms: the name of its folder, or the name of its file without the
suffix `.yaml` (the file `fancy.v2.yaml` is the compose template `fancy.v2`, and a file
`flmtemplate.yaml` directly in `templates/OUTPUT/` is the compose template
`flmtemplate`); `-t NAME` selects one. A name written in both forms in one folder `templates/OUTPUT/` (the folder `fancy/`
and the file `fancy.yaml`) stops the command with status 2. A folder that holds no
`flmtemplate.yaml` and a file whose name does not end in `.yaml` are not compose
templates and are left out. A name beginning with `.` is left out in `templates/` and in
each `templates/OUTPUT/` alike.

The description is YAML: `output`, `engine`, `accepts`, and optionally `main`,
`requires`, `config`, `delimiters` and `artifacts`. The key `output` must be the `OUTPUT`
of the folder `templates/OUTPUT/` that holds the compose template, or the command stops
with status 2. The key `main` gives the path of the
file that holds the compose template's text; without `main`, the text is empty. Every
path a description gives, `main` and the paths under `artifacts`, is relative to the
folder of the description, which in the folder form is the compose template's own
folder; so is the path of a file the compose template includes (see [Files a compose
template includes](#compose-templates)). The program reads a file of the folder only
where the description or an include gives its path, so another file in the folder, such
as a backup that an editor leaves, is not read.

The description's key `artifacts` lists the files the compose template ships, as a map
from a file's name to its description: `file`, the file's path relative to the folder of
the description (required, with an extension);
`template`, `true` when the file is itself a template to fill in, which must then be
text (default `false`); and `media_type`, the file's media type, required when the
file's extension is not one the program knows (`html`, `css`, `js`, `txt`, `json`, `svg`,
`png`, `jpg`, `jpeg`, `gif`, `webp`, `pdf`, `tex`, `sty`, `bib`, `woff`, `woff2`, `ico`).
In place of `file`, the key `dir` gives the path of a folder, relative to the folder of
the description, which the compose template ships whole, its files at every depth (see
[Files a compose template ships](#compose-templates)); `template` and `media_type` are
refused beside `dir`, since a folder is never filled in as a template and each of its
files has the media type of its own extension:

```yaml
# _flmextensions/fancy/templates/html/fancy/flmtemplate.yaml
output: html
engine: minijinja
main: fancy.html
accepts: all
config: {color: teal}
artifacts:
  style: {file: look.css, template: true}
  logo: {file: ../../../img/logo.png}
  icons: {dir: ../../../icons}
```

The folder `fancy/` holds the description, the text `fancy.html` and the stylesheet
`look.css`; the logo and the icons are files of the extension that any of its compose
templates may ship, reached from the folder `fancy/` through `../../../`.

A `main` or a listed file that leaves the extension's folder, or that does not exist,
stops the command with status 2; so does a listed folder that leaves the extension's folder,
that is not a folder of the extension, that holds no file (names beginning with `.`
aside), or that holds a file that cannot be read. A compose template's name must be unique among the
packages in use, the program's own compose templates included. An extension that wants
its compose template used by default writes `compose: {template: NAME}` in its
`features` (or a map by output); like every setting of an extension, this ranks above the
program's defaults.

**An extension in JavaScript.** The manifest of an extension written in JavaScript names
the engine `js` and the main module; the main module declares the extension's packages
and their features (see [Extensions](crate::guide::extending#extensions) in the guide to
extending FLM):

```yaml
# _flmextensions/wordcount/flmextension.yaml
extension:
  description: Counts words, collects to-do markers, shouts
  engine: js
  js:
    main: main.js
```

A document then lists `use: [wordcount]`, and sets the settings of the extension's
features under `flm.features`, as it sets those of a compiled feature.

**An extension compiled to WebAssembly.** The manifest names the engine `wasm` and the
module, one WebAssembly file inside the extension's folder; the module declares the
extension's packages and their features (see [A Rust extension compiled to
WebAssembly](crate::guide::extending#a-rust-extension-compiled-to-webassembly) in the
guide to extending FLM). A document uses it as it uses any other extension:

```yaml
# _flmextensions/wordcount/flmextension.yaml
extension:
  description: Counts words, collects to-do markers, shouts
  engine: wasm
  wasm:
    module: ./wordcount.wasm
```

**What an extension may reach.** A script, or a WebAssembly module, reaches no file, no
network and no program by itself. A package of the extension declares, under the key `host` of its
package object, the *host services* its handlers call, and the program offers two:

- `files` reads the text of a file in the extension's own folder, or in the document's
  folder (the folder of the input file; the current folder for a document given with
  `-c` or on standard input). A path that leaves the folder is refused, and so is a file
  that a symbolic link leads outside it; this service reaches neither the rest of the
  machine nor the network;
- `programs` finds a program installed on the machine and runs it in a fresh temporary
  folder, with the arguments, the input and the files the extension gives it, and hands
  back its output and the files it wrote there. The program so run may do anything you
  may, the network included.

An extension whose package declares a service the program does not offer is refused with
status 2. The guide to extending FLM says what each service takes and answers ([Host
services](crate::guide::extending#host-services)).

**Install extensions from trusted sources only.** An extension runs code, and through the
service `programs` it can run any program on the machine, with your rights. The folder
`_flmextensions` beside a document comes with the document: building a document you
received from someone else runs the extensions in its folder.

### Where extensions are looked for

A locator under `use` is looked for among the packages compiled into the program first,
so a file never replaces one of them: the name of a compiled package is never read from
a file. It is then looked for in each folder of the *search path*, in order, and the first
folder that holds the extension, in either layout, wins:

1. the folders given with `--extensions-dir`, or, without that option, the folder
   `_flmextensions` beside the document (in the current folder for a document given with
   `-c` or on standard input);
2. the folders listed in the environment variable `FLMPATH`, separated as the system
   separates the folders of `PATH`;
3. the folder `~/.flm/extensions`, the home folder `~` being the one that the environment
   variable `HOME` gives, else `USERPROFILE`;
4. last, the extensions compiled into the program: the extensions shipped with FLM,
   `lecturenotes` ([The lecture-notes template](#the-lecture-notes-template)),
   `math-mathjax` ([Mathematics processed by MathJax](#mathematics-processed-by-mathjax)),
   `profile-pdf-latex` ([PDF through LaTeX](#pdf-through-latex)), `syntax-highlight`
   (the highlighting of code listings in HTML and in the rendered LaTeX) and `flmdoc` (the
   markup of documentation pages; see
   [Writing documentation pages: the extension `flmdoc`](crate::guide::extending#writing-documentation-pages-the-extension-flmdoc)),
   found by their names (a locator of one segment). A copy of one of them in a folder of the search path
   is found first and is used in its place; for the three written in Rust
   (`profile-pdf-latex`, `syntax-highlight`, `flmdoc`), the copy is their WebAssembly
   form.

A locator of several segments is joined to each folder: `@acme/style` is
`@acme/style.yaml` or `@acme/style/flmextension.yaml` inside the folder. An extension is
identified by the path of its manifest, so an extension that two locators reach (`style`
in one folder of the search path, `shared/style` in its parent folder) is read once, and
two packages that use each other cause no problem. An extension compiled into the program
is identified as `bundled:ENGINE/NAME` (`bundled:js/math-mathjax`), and one written in
Rust as `bundled:rust/NAME` (`bundled:rust/profile-pdf-latex`). A symbolic link inside an extension's
folder must not lead to a file or a folder outside that folder; the extension's folder
itself may be a link. A locator found in no folder and not compiled into the program, a folder that holds an extension in
both layouts, a folder given with `--extensions-dir` that does not exist, and a problem
in an extension's manifest or files stop the command with status 2, at the place of the
problem.

### Ranks of the settings

A setting may be written in several places, called *settings sources*; the source of
the highest rank wins and replaces a lower source's value whole (a collection of named
entries is combined entry by entry: see below). From the lowest rank to the highest:

1. the program's defaults ([The program's defaults](#the-programs-defaults));
2. the packages of the extensions the document uses;
3. the front matter;
4. the workflow, for the settings that are set per render (the `settings` of a
   workflow in an extension's package);
5. `-O` on the command line.

Between packages, the `use` lists decide. A package wins over the packages it uses,
directly or through others: a package `my-article` with `use: [article]` that changes
one setting of `article` gives that setting, silently, regardless of the order in which
a document lists the two. Packages of which neither uses the other, and packages that use
each other, are *peers*: two peers that write different settings are combined, two that
write one setting the same value agree, and two that write one setting different values
stop the command with status 2, and the message gives both files and lines. A source of a
higher rank that writes that setting (the front matter, for example) decides it instead.
A package that uses one of two such peers takes its place: it is the other's peer for
that setting. The same rule applies to the settings that packages give one workflow, and
to the fields of one workflow that packages write.

Switching a feature off follows the same rule: the source of the highest rank that
writes `false` or settings for the feature decides. A front matter that writes
`theorems: {}` therefore switches on a feature that a package switched off; a package
switches back on (or off) a feature that a package it uses switched off (or on); and two
peers of which one writes `false` and the other settings stop the command. A key that no
feature declares, or a value that cannot be read, stops the command with status 2, at
its place in the file.

No file is read that the document does not list: the front matter, the extensions it
uses and `-O` hold every setting.

### Collections of named entries

Some settings are collections of named entries: the kinds of notes, the types of floats,
the kinds of theorems. Each is a map from a name to an entry, and the sources that write
one are combined entry by entry, by the same rule: a source adds an entry and leaves the
others; the source of the highest rank that writes an entry replaces it whole; an entry
written `false` is removed; a package replaces an entry of a package it uses and leaves
that package's other entries; two peer packages that write one entry different values
stop the command, and the message gives that entry. So `my-article`, using `article`, can
change `article`'s footnote alone. The entries are kept in the order of the sources (the
feature's own first, then the program's defaults, the packages of extensions, the front matter,
the workflow and `-O`), each source's entries in the order written; an entry that is
replaced keeps its place. A document that wants the program's float types and one more
writes only the one more:

```yaml
flm:
  features:
    floats:
      types:
        algorithm: {handlers: [any], caption_name: Algorithm}   # added
        table: false                                            # removed
```

| feature | keys combined entry by entry |
|---|---|
| `userdefs` | `macros`, `environments`, `specials` |
| `xnotes` | `kinds` |
| `theorems` | `classes`, `kinds` |
| `floats` | `types` |
| `quote` | `environments` |
| `lists` | `environments` |
| `headings` | `headings`, `commands` |
| `annotations` | `annotators` |
| `numbering` | `styles` |
| `write` | `output_names` |

An entry written over one of the feature's own entries, or over one of the program's,
starts from what the feature gives a new entry of that name, not from the entry it
replaces: the fields it does not write are never those a lower source wrote. A document
that changes one field of the standard footnote, its mark for example, therefore writes
the fields that a kind of note requires (`words`) and every field it wants to keep, such
as the command the footnote is written with. Without `commands: [footnote]`, the kind of
note has no command, and `\footnote` is an unknown command:

```yaml
flm:
  features:
    xnotes:
      kinds:
        footnote:
          commands: [footnote]
          words: {base: {one: Footnote~, other: Footnotes~}}
          mark: {formats: '#{roman}'}
```

Every other setting is replaced whole, a list included. A value of one of these keys that
is not a map stops the command with status 2, at its place in the file.

### Documents of several files

A document brings other files in with `\input{file}`
([the language](crate::guide::language_syntax#including-files)). The file name is
relative to the folder of the file that holds the command: `\input{parts/intro.flm}` in
`docs/paper.flm` reads `docs/parts/intro.flm`, and `..` goes up one folder. Every message
gives a file by that path: the primary file as it was typed, and an included file as the
folder of the file that includes it joined with the name written there, with `.` and `..`
removed (`docs/parts/../common.flm` is `docs/common.flm`). A file name printed can
therefore be opened from where the command ran. `flm check` reads the included files as
`flm render` does.

An included file may have a front matter of its own, holding metadata and, under `flm`,
only two keys:

- `parsing`: how its own content is read, over the parsing settings that apply where it
  is included;
- `input_as`: the file becomes a part with a heading of its own
  ([the language](crate::guide::language_syntax#a-file-that-opens-with-a-heading)). The
  heading is made of the file's `title` by the heading command that `input_as.heading`
  gives, without its backslash (`section`); it carries `input_as.label` as its label;
  and with `input_as.numbered: false`, it is unnumbered.

```yaml
---
title: Methods
flm:
  input_as: {heading: section, label: 'sec:methods'}
---
```

A heading command that is not a command name is an error at the `\input`, reported as
"the heading command of FILE must be a command name without its backslash"; a label that
holds a brace is an error reported as "the heading label of FILE must not hold a brace".
Both refer to the keys `input_as.heading` and `input_as.label` of the included file's
front matter. Any other key under `flm` in an included file is reported at its place, and
the file's content is still brought in. `flm schema --of front-matter` describes
`input_as` among the keys of a front matter.

## Printing the schema of the settings

```text
flm schema [--of extension|front-matter] [DOCUMENT] [--extensions-dir DIR] [-o FILE]
```

`flm schema` prints the *schema* of a settings file: a description, in JSON Schema (a
format that text editors read), of every key the file may hold and of the values each key
accepts. A text editor uses it to check the file while it is written.

Without a document, the schema covers the features of every package the program carries
(`stdcore`, `stdmain`, `std`, `math-passthrough` and the packages compiled into the
program) and of the package of the program's default math processor (`math-mathjax`, an
extension shipped with the program, when the program includes it).
It depends on no document, so one schema serves every file of its kind, and every run
with the same extensions installed prints the same bytes. With a document (`flm schema --of front-matter paper.flm`), the
schema covers the features of the packages that document uses: those of its initial list,
the package of its math processor and those its front matter lists under `use`, compiled
packages and packages of
extensions alike, with the packages they use. The extensions are found as `flm render`
finds them, and the features that an extension written in JavaScript declares are
covered too. A document that cannot be read, or whose packages cannot be found, stops the
command with status 2.

| option | what it does |
|---|---|
| `--of extension` | the schema of an extension's manifest (the default) |
| `--of front-matter` | the schema of a document's front matter: the metadata, and the settings under `flm` |
| `DOCUMENT` | the document whose packages the schema covers (see above) |
| `--extensions-dir DIR` | a folder in which the document's extensions are looked for, as for `flm render` (see [Where extensions are looked for](#where-extensions-are-looked-for)); it requires a document |
| `-o FILE` | writes the schema to `FILE` instead of standard output (`-`, the default, is standard output) |

The schema suggests values for three keys of the `flm` block. An editor offers the
suggested values as completions; they restrict nothing, so a value that is not among them
may still be written, and the editor does not mark it as wrong.

- For an entry of `use` (in a front matter and in an extension's manifest) and of
  `initial_use`, the schema suggests the packages compiled into the program, the
  extensions installed in the folders of the search path, and the extensions shipped with
  the program. An installed extension is suggested by its locator: `style` for
  `style.yaml` or `style/flmextension.yaml` directly in a folder, and `@acme/style` for
  the same layouts inside a folder `@acme`. The schema is made without loading any
  extension, so the packages that an extension provides besides its default one
  (`style#svg`) are not suggested; write them by hand.
- For `math`, the schema suggests `NAME` for each suggested package or extension named
  `math-NAME`: `passthrough`, `mathjax` when the program includes it, and `NAME` for
  each extension `math-NAME` installed.

The folders listed are those of the search path that the command uses (see [Where
extensions are looked for](#where-extensions-are-looked-for)): with a document, the
document's; without a document, those of a document given with `-c`, that is
`_flmextensions` in the current folder, the folders of `FLMPATH` and `~/.flm/extensions`.
A folder that does not exist adds nothing. After installing an extension, run `flm schema`
again so that the schema suggests it.

For each setting, the schema gives its description (the feature's own documentation)
and, under `x-flm-bind`, where it may be set: `registry` for a setting that is fixed when
the registry is built (it cannot vary per document or per render); `document` for a
setting that is set on the document; `render` for a setting that is set per render, which
a workflow's `settings` in an extension's package may also write; `either` for a setting that is
set on the document and that a render may set again. For a collection of named entries, the schema
also says that it is combined entry by entry (`x-flm-marks`) and gives its default
entries. The arguments of a user definition are written as maps that give a structured
syntax (the key `syntax` with the name of a kind of argument, and its parameters) or as
argument codes, their shorthands; the schema describes each kind and each code by the
form of argument it represents (`mandatory` and `{`: a mandatory `{…}` group).

To have an editor check the files while they are written, write the schema to a file and
point the editor to it. With the YAML extension of Visual Studio Code, for example, after
`flm schema -o flm-extension.schema.json`, an extension's manifest refers to the schema
on its first line:

```yaml
# yaml-language-server: $schema=flm-extension.schema.json
flm:
  features:
    text: {width: 72}
```

or the editor's settings map a pattern of file names to it (`"yaml.schemas":
{"./flm-extension.schema.json": "_flmextensions/*.yaml"}`). A front matter is part of a
document, not a file of its own, so its schema serves an editor that checks the front
matter of FLM documents. A file that cannot be written stops the command with status 1.

## Watching a document: the live preview

`flm watch INPUT`, or `flm INPUT` with no command, builds the document into an HTML page,
serves the page to a browser on this machine, opens the default browser on it, and keeps
running. Whenever a file that the document read changes (the document itself, a file
brought in with `\input`, an extension's manifest or one of its files, an image, a
bibliography file, a file beside the document that an extension read while building it),
it builds the document again, and the page in the browser follows. Ctrl+C ends it, with
status 0.

```text
$ flm paper.flm
flm: the preview is at http://127.0.0.1:18910/; Ctrl+C ends it
flm: editors found: `code` (/usr/local/bin/code), `emacsclient` (/opt/homebrew/bin/emacsclient); a place clicked on the page opens in the one chosen in the page's menu, `code` by default
flm: build 1 done
flm: changed paper.flm
flm: build 2 done
```

The preview takes the options of [What to build](#what-to-build) (`-p`, `-f`, `-t`,
`-O`, `--layout`, `--extensions-dir`, `-v`), and these:

| option | what it does |
|---|---|
| `--port PORT` | the port of this machine at which the preview is served; 0 lets the system choose a free port (the address printed gives it). Default: 18910, or else the first free port among the 64 that follow it |
| `--no-browser` | open nothing; the address is still printed. Without this option, starting the program again while the page is open opens a second page; the open page reconnects by itself either way |
| `--editor NAME` | the editor that the page's menu of editors selects at first (see [From the page to the source](#from-the-page-to-the-source)): one of `code` (Visual Studio Code), `zed` (Zed), `emacsclient` (Emacs, in the frame already open), `subl` (Sublime Text) and `gvim` (the graphical Vim). Without the option, or when that editor is not found, the menu selects the first editor found, in this order. A browser that remembers a choice made in the menu selects that editor instead, when it is found. Another name is refused with status 2 |

The environment variable `BROWSER`, when it is set, gives the program that opens the
preview's address, in place of the default browser.

The preview writes no output and accepts no `-o` (the only file it writes is the
document itself, when you save settings from the page: see
[Editing the settings from the page](#editing-the-settings-from-the-page)); it opens the
browser by itself and accepts no `-V`; it watches a file, so it accepts neither
`-c TEXT` nor `-`. Each of these is refused with status 2, and the message gives the
`flm render` command that does what was meant (`flm render -V -o OUTPUT INPUT` writes an
output and opens it).

### What the preview serves

The server listens only on this machine's loopback address (`127.0.0.1`), and serves
from memory the files of the last build that produced them. The preview builds every
output of its workflow, as `flm render` does, and serves the main file of the *leading
output* (the output `-f` names, else the workflow's first) at `/` (and at `/index.html`),
and every other file at its path: the files written beside the page (an image copied
beside the page under `-O graphics.strategy.html=copy`, for example) and the files of the
workflow's other outputs. It serves nothing from the disk. Every
answer tells the browser not to keep it. The page is the one that `flm render` would
write, with the same compose template, plus the preview's script and stylesheet, the
stylesheet of the marks of errors (whether or not the document has such a mark), and, on
each element, its place in the source (the attribute `data-flm-source-span`). Under `-p
text`, `-p latex-recomposed`, `-p bibtex` or `-p csl-json`, the output is served the same
way, and loading the address again shows the last build.

### Building through errors

Each build continues through the document's errors: the page shows the document as it is
now, with the marks the library writes where something could not be rendered, and the
terminal prints the build's problems as `flm render` prints them. When the document
cannot be built at all (its front matter cannot be read, a package it lists is missing),
the page in the browser stays as it was. Before any build has produced a page, the
address shows a page that says the preview is waiting for the document, with the toolbar
and the problems (see [The toolbar](#the-toolbar)); the document's page replaces it once
a build produces one. Under `-v`, the terminal also prints, at each build, the files read
and the progress of fetching the details of cited works, and, at the first build and at
each build that reads the front matter and the packages again, the packages in use and
the workflow, as `flm render -v` does.

### How the page follows the document

The page carries a script of the preview, which keeps a connection to the program open.
At each build, the script updates in place only the parts of the page that changed (the
document's content, the title, the table of contents, the list of notes, the
bibliography, each placed part on its own), and inside a changed part it keeps every element that did not change, so that
the place you are reading stays where it is and a formula whose source did not change
stays typeset. The title shown in the browser's tab follows too; a compose template that
writes a part's plain text elsewhere (in a heading, in a `<meta>` tag) gets the page
loaded again when that text changes. An image written into the page changes with the
part that shows it, and a file served beside the page that changed is loaded again. The
page is loaded again when the compose template's own markup changed, when the page
gained or lost a part, or when it gained or lost a script or a stylesheet (the formula
renderer at the document's first formula, for example). When the program is started
again, an open page reconnects by itself, and loads itself again if the document changed
in the meantime.

### The toolbar

The script places a small toolbar over the bottom right corner of the window; it is not
part of the document, and a page written by `flm render` never holds it. The toolbar
shows the state of the build (*Building…* while a build runs, otherwise the number of
the last build's problems, colored by the worst of them: red for an error, amber for a
warning), one button per tool, and a menu of the editors found on this machine, which
chooses the editor that a place of the document opens in (see [From the page to the
source](#from-the-page-to-the-source)). The tools:

| tool | what it does |
|---|---|
| *Problems* | unfolds the list of the last build's problems, each with its severity, its message as the terminal prints it, and its file, line and column; a click on a problem scrolls the page to the element it is in and flashes that element. The elements that problems are in are outlined in the page, whether the list is unfolded or not; after a build without problems, the count is zero and the outlines are gone |
| *Rebuild* | builds the document again now, as if a file had changed: for a change that the program did not detect (a file other than those the document read, a save on a network drive) |
| *Outputs* | shows the command lines that write this document's outputs, with the file names filled in: `flm render -p html -o paper.html paper.flm` (the complete HTML page), `flm render -p latex-rendered -o paper.tex paper.flm` (the LaTeX for LuaLaTeX, with everything resolved), `flm render -p latex-recomposed -o paper.tex paper.flm` (the LaTeX source, written back close to the original) and `flm check paper.flm` |
| *Settings* | shows the document's features and what its front matter writes for them, lets you change that, and saves it into the document's file (see [Editing the settings from the page](#editing-the-settings-from-the-page)) |

The button at the right end folds the toolbar down to its state alone; a click on the
state unfolds it. The browser tab remembers which tool was open and whether the toolbar
was folded when the page is loaded again. When a build produces no page, the page in the
browser stays as it was and the *Problems* list says why; the page that waits for the
document lists the problems the same way, with the list unfolded.

### Editing the settings from the page

The *Settings* tool lists the features of the packages the document uses, the features
of its extensions written in JavaScript included (those of `flm schema --of front-matter
paper.flm`), as of the latest build that read the document's packages anew; before
that first build, it lists the features of the schema made without a document. It shows one folded
section per feature, with the feature's name, its description and a switch: on, or off
(`false` under `flm.features` in the front matter). Unfolding a section shows a form of
the feature's settings, each key with its description, holding **the values that the
front matter writes**, not the values that the document ends up using: a key that the
front matter does not write shows no value, since the document takes it from its
packages or from the feature's default. To set a key, add it to the form (the button
*Object Properties* lists the keys); a key removed from the form is not written. *Save*
edits the front matter's `flm.features` in the document's file: a feature switched off
becomes `name: false`, a feature with keys set holds those keys and only those, and a
feature that is on with nothing set is not written; a feature that the program does not
have is kept as written. The preview then builds again, as after any save of the file,
and a value that a feature refuses is reported in *Problems* at its line in the front
matter. When the front matter cannot be read (it is not valid YAML, for example, while
you are editing it), nothing is written and the tool says why.

A save changes only the lines of the values you changed, apart from the few layout
details stated below. The save compares what the front matter writes with what the tool
holds and rewrites only the values that differ, in place. The comments, the blank lines,
the quoting and the style of each map and list (on several lines, or `{…}` and `[…]` on
one line) are kept, and so are the two `---` lines and the document below them, byte for
byte. A line that the save adds (a key newly set, a feature newly switched off) is
indented like the lines beside it and, in a file whose lines end with a carriage return
and a line feed, ends with a line feed alone. A text written into a map written on one
line (`{…}`) is written between double quotes. A blank line at the end of a feature's
block of settings goes with the block when the feature is switched off, and a key added
to a block that ends with a blank line comes after that line. A comment at the end of the
front matter's last line is moved to a line of its own below the block of settings that
replaces that line's value. A file with no front matter gains one, between two `---`
lines above the document. When the save cannot write the change exactly as the front
matter is laid out, nothing is written and the tool says so; set the value in your editor
then. Two kinds of value are never saved from the page, however the front matter is laid
out: a text of several lines, and a map of settings placed as an entry in a list written
on several lines (`- …` lines); set them in your editor.

The tool edits the features alone: `use`, `parsing`, `feature_order`, `layout`, `default_profile`, the
metadata and the front matter of a file brought in with `\input` are edited in your
editor. **Save the file in your editor before saving from the page**, since the program
writes the file as it is on the disk at that moment: an editor that watches its files
(most do) shows the new front matter; one that does not may write its own copy over the
saved settings at its next save. The tool reads the front matter's features when you
open it and holds what it read until you close it, and a save from the page writes the
features as the tool holds them: after changing the features in your editor (and saving
there), close the tool and open it again before saving from the page.

The tool shows only the values that the front matter writes, not the values that the
document ends up using, and it edits only the features.

A request of the page that acts (build again, open a place in the editor, save the
settings) is carried out only when it comes from the preview's own page at the address
the program printed; a request from a page of another site open in the same browser is
refused. So the actions work from `http://127.0.0.1:PORT/`, but not from the same page
reached as `http://localhost:PORT/`.

### From the page to the source

Two clicks with modifier keys on the page open the source or copy a reference; the
element concerned flashes:

| click | what it does |
|---|---|
| Ctrl or Cmd, and Shift | opens the file at the place of the clicked element in the editor chosen in the toolbar's menu of editors (below): the line of the file, its front matter counted, and the column in characters. An element with no place of its own takes the place of the nearest element before it or around it that has one |
| Ctrl or Cmd | copies `\ref{LABEL}` for the nearest element around the click that has a label (a section, an equation, a figure), so that it can be pasted where the document refers to it; nothing is copied when no element around the click has a label |

A problem of the *Problems* list that has a place in a file has a button *Open in
editor*, which opens that place the same way. The editor runs separately from the
preview: the preview goes on when the editor ends, and ends without ending the editor.
Only a file that the last build read is opened. When nothing could be opened (no editor
was found, the editor could not be started, or the browser did not let the page copy), a
line above the toolbar says why, until a later click succeeds.

**The editors.** The preview can open a place in five editors: Visual Studio Code
(`code`), Zed (`zed`), Emacs (`emacsclient`, in the frame already open), Sublime Text
(`subl`) and the graphical Vim (`gvim`). When the program starts, it looks for each of
them: first for its program in the folders of `PATH`, then, on macOS, for the
application that installs it in `/Applications` or `~/Applications`
(`Visual Studio Code.app`, `Zed.app` or `Zed Preview.app`, `Emacs.app`,
`Sublime Text.app`), so that an editor whose command-line program was never installed
is found too. Vim in a terminal is
not among the editors: the preview starts the editor without a terminal, where it could
not show the file.

The toolbar's menu of editors lists the editors found, and a place opens in the one
selected there; the menu is hidden when no editor was found. The browser remembers the
editor you select, for the preview's address: the choice holds when the page is loaded
again and when the program is started again at the same port. Without `--port`, the
program uses the port 18910 whenever it is free, so the choice holds from one run to the
next; another port is another address, with a choice of its own. The menu selects
at first the editor the browser remembers, when it is found; otherwise the editor that
`--editor` specifies, when it is found; otherwise the first editor found, in the order
above.

The terminal reports the same requests. When the program starts, it lists the editors
found, each with the path of its program, and the editor the menu selects by default,
or it says that none of the five editors was found and where they were looked for. For
each place that the page asks to open, it prints the file, the line and the column
opened and the editor, or why nothing was opened, in the words of the line above the
toolbar. An editor that ends with a failure after it started (an `emacsclient` with no
Emacs server running, for example) is reported there too, with what the editor wrote on
its standard error.

The click copies `\ref{…}` for every element, a defined term included.

## Describing the environment: `flm info`

`flm info` describes the *environment* in which a document is processed, as pages of
documentation that it prints on standard output. The environment is everything the host
library prepares for a document before it reads the document's content:

- the packages in use, and the features they bring;
- the commands each feature declares (macros, environments, special character sequences),
  with how each is written and what each argument holds;
- the settings each feature takes, with the value in effect and where it was written;
- the workflows, with the plan of the workflow that runs (its stages, in order);
- every other name the [registry](crate::core::registry) holds (stages, tables, outputs,
  argument roles, …);
- the program itself: its commands and options, the environment variables it reads, the
  folders it reads, and the packages and extensions a document may use.

A document changes its environment only through its front matter: the packages it uses,
the settings it writes, the profile it builds with. `flm info` describes the environment,
never the document's content.

```sh
flm info                          # the environment at a glance
flm info features/headings        # one page, by its path
flm info '\section'               # the pages of a command, looked up by name
flm info -d paper.flm settings/   # the settings in effect for paper.flm
flm info --json features/refs     # one page as JSON
flm info --write-flm refs.flm features/refs   # one page as an FLM document
```

### The pages and their paths

Each page has a *path*, which is what `flm info` is given to show that page. The pages form
a tree: each page lists the pages directly below it. Without a path, `flm info` shows `/`,
the environment at a glance (the profile in view, the math processor, the packages, the
features and the workflows), which leads to every other section.

| path | the page | example |
|---|---|---|
| `packages/`, `packages/NAME` | the packages; one package, with where it comes from (compiled into the program, or an extension and its manifest's description, author and engine) | `flm info packages/stdcore` |
| `features/`, `features/NAME` | the features; one feature, with its documentation, its element types, the commands it declares, its scoped packages and its settings keys | `flm info features/headings` |
| `features/NAME/macros`, `…/environments`, `…/specials` | the commands of one kind that the feature declares | `flm info features/headings/macros` |
| `features/NAME/macros/MACRO`, `…/environments/ENV`, `…/specials/TRIGGER` | one command: how it is written, its arguments (each with its role, below), the fields written after it, the feature that declares it, the modes it is available in | `flm info features/headings/macros/section` |
| `features/NAME/scopedpackages/PACKAGE`, and below it `macros/MACRO` and the others | a scoped package (the commands available only inside some argument or body) and its commands, as for a feature | `flm info features/refs/scopedpackages/flm.refs.label` |
| `settings/`, `settings/FEATURE` | the settings keys of a feature, each with its type, its default, the value in effect and where that value was written | `flm info settings/text` |
| `settings/parsing`, `settings/front-matter` | the parsing settings; every key a front matter may hold | `flm info settings/front-matter` |
| `workflows/`, `workflows/NAME` | the workflows; one workflow, with its outputs and, for the profile in view, its plan | `flm info workflows/html` |
| `namespaces/`, `namespaces/NAMESPACE`, `namespaces/NAMESPACE/NAME` | the families of names the registry holds; every name of one family; one name | `flm info namespaces/stage/flm.refs.resolve` |
| `program/`, `program/paths`, `program/extensions` | the program's commands, options and environment variables; the folders it reads (the search path of extensions, the citation cache, the image folders); the packages and extensions a document may use | `flm info program/paths` |

In a path, a macro is written without its backslash (`macros/section`), a special
character sequence as written (`specials/~`), and a scoped package or any other name with
its dots (`namespaces/stage/flm.refs.resolve`). A section's index page may also be given
without its final slash (`flm info features`). A feature, a package, a workflow, a scoped
package and a settings key have their page in their own section, so `namespaces/feature`
lists the features but each links to `features/NAME`.

The paths `settings/parsing` and `settings/front-matter` are fixed, whatever the features
are called. A feature named `parsing` or `front-matter` therefore has no settings page of
its own, and looking up one of its keys gives the fixed page. No standard feature has
either name.

### Finding a page by name

A path that is no page's path is looked up as a name: a command (`'\section'` or
`section`), or the name of a package, a feature, a settings key (`text.width`), a workflow
or anything else the registry holds (`flm.refs.resolve`). Every page found is shown, in
the order of the tree: `flm info section` shows the page of the macro `\section`. A name that finds no page is an error, which lists the
nearest paths:

```text
$ flm info features/heading
error: no page or name `features/heading`; the nearest pages are: `features/headings`, …
```

Several paths may be given; every page asked for is shown, in the order given.

### Which environment is described

Without `-d`, `flm info` describes the environment of a document that specifies nothing
(the program's packages, its math processor and its defaults), as `flm schema` does. With
`-d DOCUMENT`, it describes the environment of that document, prepared as `flm render`
prepares it: the packages its front matter uses, the extensions found beside it, the
settings it writes.

The build options of `flm render` are accepted and shape the environment described exactly
as they shape a build: `-p PROFILE` (the profile in view, which decides the workflow
whose plan the workflow pages show and the package `profile-PROFILE` among the packages),
`--use PACKAGE`, `--initial-use PACKAGES`, `--math PROCESSOR`, `--extensions-dir DIR` and
`-O FEATURE.KEY=VALUE` (see [What to build](#what-to-build) and
[Settings on the command line](#settings-on-the-command-line)). For example,
`flm info -O text.width=60 settings/text` shows `60` as the value in effect of
`text.width`, written by the option `-O`.

These options shape what is described and nothing else: they never change how the pages
are printed. The pages are always printed with the program's defaults (next section).

### The text of the pages

By default, `flm info` prints each page as text. Each page is an FLM document that the
program renders with the profile `text`, using the markup of the extension `flmdoc` (see
[Writing documentation pages: the extension `flmdoc`](crate::guide::extending#writing-documentation-pages-the-extension-flmdoc));
the program carries that extension compiled in, among its bundled extensions. A program
built without its bundled extensions does not carry `flmdoc` and can print the pages only
with `--json`, which renders nothing; asked for text, or for an FLM document with
`--write-flm`, it reports that the page cannot be rendered and exits with status 3. A link to another page is printed as its text followed
by the command that shows that page: `headings <flm info features/headings>`.

The width of the lines depends on where standard output goes:

- when standard output is a terminal, the lines are as wide as the terminal: the value of
  the environment variable `COLUMNS` when it is a whole number above 0, else the width the
  terminal reports;
- otherwise (a file, a pipe), the lines are 80 columns wide, the default of the text
  backend.

No option sets the width of the pages: `-O text.width=…` shapes the environment described,
as above.

With `-r` (`--recursive`), every page below each page follows it, in the order of the tree:
`flm info -r features/headings` prints the feature's page and the page of every command it
declares.

### JSON output

With `--json`, `flm info` prints one JSON object on standard output, whatever the number of
pages: a header, `meta`, then the list `pages`.

```json
{
  "meta": { "kind": "info", "flm": "0.5.0", "format": 1 },
  "pages": [
    {
      "path": "features/headings",
      "kind": "feature",
      "title": "Feature headings",
      "summary": "…",
      "body": { … },
      "children": [
        { "path": "features/headings/macros", "title": "Macros of feature headings", "summary": "…" }
      ]
    }
  ]
}
```

- `meta` comes first. Its `kind` is `info`, `flm` is the program's version, and `format`
  is the version of this JSON form, a whole number that changes when the form changes in a
  way a reader must know about.
- Each page has its `path`, its `kind` (`root`, `package`, `feature`, `command`,
  `settings`, `workflow`, `namespace`, `entity`, `program`, the index kinds such as
  `feature-index`, …), its `title`, its `summary` (left out when empty), its `body` (the
  page's content as data; left out for an index page) and its `children`.
- Without `-r`, each child is an object with its `path`, `title` and `summary`. With `-r`,
  each child is the full page, written the same way, so that one object holds every page
  below the page asked for.

The documentation texts in the JSON are the FLM source the features wrote, not rendered.
A place in a file, where a page gives one (where the value in effect of a setting was
written, for example), is a file, a line counted from 1 and a column counted from 1 in
bytes, as in the records of
[`--json-diagnostics`](#problems-as-json-records---json-diagnostics).

### A page as an FLM document: `--write-flm`

With `--write-flm FILE`, `flm info` writes the page asked for to the file FILE as an FLM
document, in place of printing it. This document is the source that `flm info` renders
to print the page as text (previous sections). `flm render -p text` renders it to the text
that `flm info` prints when its standard output is not a terminal, and `flm render` renders
it to any other output as well:

```sh
flm info --write-flm section.flm '\section'
flm render section.flm -o section.html
```

- The file `-` prints the document on standard output.
- The document's front matter gives the page's title, uses the extension `flmdoc`, in
  whose markup the page is written, and no math processor. `flm render` finds `flmdoc`
  among the program's bundled extensions, unless a folder of extensions holds an extension
  of that name.
- A link to another page points at the command that shows that page, `flm info
  features/headings`, in HTML too. The setting `flmdoc.link_style` changes that address
  when the document is rendered: `flm render section.flm -O flmdoc.link_style=path` makes
  it `features/headings.html` (see
  [Writing documentation pages: the extension `flmdoc`](crate::guide::extending#writing-documentation-pages-the-extension-flmdoc)).
- A documentation text that is not valid FLM is written as plain text, after a warning, as
  for the text of the pages (next section), so that the document renders.
- The paths given must choose exactly one page: several pages are an error (exit status
  2), and `--write-flm` cannot be combined with `-r` or `--json`.
- A file that cannot be written gives the exit status 1.

### Documentation texts that are not valid FLM

Every documentation text a page shows is written in FLM, and the program's own texts are
tested to be valid. A text that an extension wrote, or one written as data in a front
matter or with `-O` (the documentation of a user definition or of a workflow, for
example), may not be. Before rendering a page, or writing it as an FLM document with
`--write-flm`, `flm info` parses each documentation text the page shows; a text that is
not valid FLM is printed as it is written, as plain text, and a warning on standard error
shows the text, the page, whose text it is and the problems found. The exit status stays
0.

```text
warning: the documentation text declared by the feature `loud` (package `loud/_flmextensions/loud/flmextension.yaml#_`) on the page `features/loud/macros/shout` must be valid FLM:
  | Like `\textbf`, but 100% {louder.
  |
  | Its argument is the text.
  error: a lone ‘%’ is not allowed, because it could be a percent sign or the start of a comment: write ‘\%’ for a percent sign, or ‘%%’ to start a comment
    at text:1:24
  error: unclosed group: expected ‘}’ before end of input
    at text:1:26
```

The warning says whose text it is:

- a text that a feature declares is credited to that feature and its package;
- a text written as data in a settings value is credited to where it was written: the
  documentation of a workflow given as data, by the workflow's name, and the
  documentation of a user definition (a macro defined in the settings, for example), by
  the settings entry that holds the same text, with the package and the place in its file,
  the front matter, or the option `-O`;
- when no settings entry of the declaring feature holds the text, or several do, the text
  is credited to the declaring feature and its package.

### Exit status of `flm info`

| status | when |
|---|---|
| 0 | every page asked for was shown, or written with `--write-flm`, with or without warnings (a documentation text shown as plain text is one) |
| 1 | the file given with `--write-flm` cannot be written |
| 2 | the arguments are wrong, the document given with `-d` cannot be read or prepared, the environment cannot be prepared, a path is no page's path and no name it looks up, or the paths given with `--write-flm` do not choose exactly one page |
| 3 | a failure of the program itself, such as a page that does not render |

## Inspecting the processing with `flm dump`

`flm dump FILE` is an inspection tool: it processes one file with the library alone and
prints, as JSON on standard output, one view of what the processing holds: a document
tree at a chosen point of the processing, a computed table, or the artifacts of an
output. It serves a reader who wants to see what the library makes of a document, for
example while writing a feature or a lowering rule of their own. Its arguments, its
output and its exit statuses are its own, independent of the other commands, and it does
not accept `--json-diagnostics`.

The processing and the views are done by the library `flm-info`, whose module `dump` is
the engine of this command: the program parses the command line, reads the file, and
prints what the engine returns. A program of one's own can call the same engine.

### What `flm dump` reads

The document that `flm dump` processes is the text of the file given on the command line
and nothing else. The command reads no settings file and no front matter, and it takes no
option that changes the language: every time, it builds its
[registry](crate::core::registry), the frozen record of everything the language and the
pipeline consult for a set of features, from the library's standard package
([`stdcore::package`](crate::stdcore::package)) under its default configuration and the
library's math processor [`math-passthrough`](crate::math_passthrough). `flm
dump` therefore applies neither the parsing settings nor the
[features](crate::core::registry::FlmFeature) that a document's front
matter or `-O` would give to `flm render` and `flm check` (see
[Rendering documents from the command line](#rendering-documents-from-the-command-line)).
A program of one's own can set both by embedding the library; see
[Embedding flm-core](crate::guide::embedding).

With that registry, one invocation, in order:

1. parses `FILE` into a [fragment](crate::model::doc::Fragment);
2. places that one fragment in a [document](crate::model::doc::Doc);
3. unless only the parsed tree is asked for, builds a [plan](crate::core::plan::Plan)
   for a workflow and executes one [run](crate::core::run::Run) of it;
4. prints one view of what that run holds.

### Synopsis of `flm dump`

```text
flm dump FILE [--at parsed|frozen|lowered:<backend>] [--view compact|wire]
              [--table NAME | --output NAME] [--workflow NAME] [--width N]
```

The options may be given in any order, before or after the file name. Exactly one file
is accepted; a second file name is a usage error.

| option | what it does |
|---|---|
| `--at parsed` | prints the tree exactly as the parser produced it. **The default**; no workflow runs for it |
| `--at frozen` | prints the tree after the plan's rewrite stages have run |
| `--at lowered:<backend>` | prints the presentation tree that the backend `<backend>` built from the frozen tree |
| `--table NAME` | prints the computed table `NAME`, under its name |
| `--output NAME` | prints the artifact set of the output `NAME`, under the name of the table that holds it |
| `--view compact` | prints a tree as the library's projection: indented JSON in the library's own vocabulary. **The default** |
| `--view wire` | prints a tree as the parser toolkit's own serialization, on one line |
| `--workflow NAME` | the workflow to plan and run: `text`, `latex-rendered`, `latex-recomposed`, `html` or `check`. Default: `text` |
| `--width N` | the text backend's line width in columns. Default: 80 |

`--table` and `--output` cannot be given together, and either of them replaces `--at`.

### Choosing what is printed

#### A tree, with `--at`

`--at` chooses a point of the processing and prints the file's tree as it is at that
point.

`parsed`, the default, is the tree exactly as the parser produced it: nothing has been
rewritten yet, so the content is not grouped into paragraphs and the user definitions
are not expanded. For the one-line document

```text
Hello \emph{world}.
```

it prints

```text
$ flm dump hello.flm
[
  {
    "chars": "Hello "
  },
  {
    "elem": "flm.textstyle",
    "name": "emph",
    "args": {
      "content": [
        {
          "chars": "world"
        }
      ]
    }
  },
  {
    "chars": ".\n"
  }
]
```

`frozen` is the tree after the plan's rewrite stages: user definitions expanded, content
grouped into paragraphs and blocks. Every later stage of the run reads this form, and the
tree is never changed again (see
[frozen trees and computed data](crate::guide::concepts_overview#frozen-trees-and-computed-data)).
For a document with a heading and a paragraph, the paragraph is a node of its own:

```text
$ flm dump document.flm --at frozen
[
  {
    "elem": "flm.heading",
    "name": "section",
    …
  },
  {
    "elem": "flm.paragraph",
    "children": [ … ]
  }
]
```

`lowered:<backend>` is the presentation tree that the backend built from the frozen
tree: the same content expressed in the vocabulary that every backend shares
([`crate::stdcore::backends::pres`]), which the backend's emitter then writes out.
`lowered:text` is the tree into which the text output's position marks point. The
emphasis above becomes a presentation span:

```text
$ flm dump hello.flm --at lowered:text
[
  {
    "chars": "Hello "
  },
  {
    "elem": "flm.pres.span",
    "body": [
      {
        "chars": "world"
      }
    ],
    "payload": {
      "style": "emph"
    }
  },
  {
    "chars": ". "
  }
]
```

The backends of the standard package are `text`, `html`, `latex-rendered` and
`latex-recomposed`. A tree is printed only for a backend that the chosen workflow uses:
`lowered:latex-recomposed` needs `--workflow latex-recomposed`, `lowered:latex-rendered`
needs `--workflow latex-rendered`, and `lowered:html` needs `--workflow html`. Under another
workflow, the command fails with status 2 and the message "the document's tree was not
lowered for the backend …"; for a name that is no backend at all, it fails with status 1
and lists the backends that the workflow uses.

#### A table, with `--table`

Everything a run computes is held in typed
[tables](crate::core::data::table::Table), each read by a
dotted name. `--table NAME` prints the table of that name, under its name. Each table
type prints itself, so tables are printed in shapes of their own:

```text
$ flm dump document.flm --table flm.refpoints
{
  "flm.refpoints": {
    "flm.heading@1:1#0": {
      "kind": "section",
      "labels": [
        "sec:intro"
      ],
      "anchor": "from-first-label",
      "display": {
        "content": "argument title"
      }
    }
  }
}
```

`flm.refpoints` holds every [reference point](crate::model::aspects::RefPoint) of the
document (every place that a reference can point at), keyed by the node that carries it
and the point's index there. Each point gives its reference kind, the labels written for it, how
its anchor is chosen, and what a reference to it may show: `number`, the counter it
counts in, where it can be numbered; and `content`, where it has content, given as the
part of its node that holds the content (`argument title`, the heading's title) or, for
content that the construct made itself (such as the bullet beside a list's item), as
that content's text. A point that read a text not written as `type:label` (a label with
no type) carries that text under a key `malformed` of its own, beside `labels`, so that
a tool never mistakes it for a label it can look up. The key is printed only where there
is such a text.

```text
$ flm dump document.flm --table flm.labels
{
  "flm.labels": {
    "labels": {
      "sec:intro": "flm.heading@1:1#0"
    },
    "duplicates": [],
    "refused": []
  }
}
```

`flm.labels` maps each label to the reference point that holds it: `labels` is that map,
and `duplicates` and `refused` list the labels that were not given to any point.

A table that a backend writes is printed the same way. `flm.html.definitions`, the
definitions table of the HTML backend, lists everything an HTML page needs beside its
content: every stylesheet and script that the document requires, once each, in the order
the features registered them. A bundler or a site generator works from it. For a
document with one formula:

```text
$ flm dump math.flm --workflow html --table flm.html.definitions
{
  "flm.html.definitions": [
    {
      "id": "flm.html.style.math-passthrough",
      "kind": "style",
      "owner": "html",
      "provided_by_host": false,
      "parts": [
        {
          "inline": "/* flm.html.style.math-passthrough */\n/* The box around a displayed formula, and the text written inside a …"
        }
      ]
    },
    {
      "id": "flm.html.style.page",
      …
    },
    {
      "id": "flm.html.mathjax",
      "kind": "script",
      "owner": "html",
      "provided_by_host": false,
      "parts": [
        {
          "inline": "/* flm.html.mathjax */\n/* The configuration of the formula renderer …"
        },
        {
          "url": "https://cdn.jsdelivr.net/npm/mathjax@4/tex-chtml.js"
        }
      ]
    }
  ]
}
```

Each entry gives the [style or script definition](crate::core::registry::StyleDef)'s
identifier,
whether it is a stylesheet or a script, the feature that declared it, whether the host
said that it supplies the definition itself (`provided_by_host`: the page then writes
nothing for it, and the entry is there so that a reader still learns that the document
needs it), and the parts to write, in order: `inline` text, a `file` that the host reads,
or a `url` that the page loads.

A name that matches no table fails, and the message lists every table that the registry
declares, which is also how to find out what there is to ask for:

```text
$ flm dump document.flm --table nope
flm: there is no table `nope`; the tables are ["flm.blocks.structured", "flm.element-index",
"flm.refpoints", "flm.counters", "flm.labels", "flm.anchors", …, "flm.output.html.regions"]
```

(The message is one line, wrapped and shortened here.)

The registry declares every table, but a run writes only the tables for which its plan
schedules a stage. The LaTeX tables are written under the `latex-recomposed` workflow
only: `flm.labels.safe`, the safe labels allocated for the document's labels, and
`flm.output.latex-recomposed`, each tree's recomposed LaTeX together with the characters
written as UTF-8 that the style file must declare for it, the preamble definitions that
writing those characters needs, and the identifiers of the LaTeX packages and definition
pieces that the tree requires:

```text
$ flm dump document.flm --workflow latex-recomposed --table flm.labels.safe
{
  "flm.labels.safe": {
    "ref": {
      "labels": {
        "sec:intro": "ref1"
      },
      "names": {
        "ref1": "sec:intro"
      }
    }
  }
}
```

A table asked for under a workflow that does not write it fails, and the message gives
the workflow:

```text
$ flm dump document.flm --table flm.labels.safe
flm: the workflow `text` does not write the table `flm.labels.safe`: choose, with `--workflow`, a workflow that writes it
```

#### An output, with `--output`

An **output** is a named deliverable of a backend ([`OutputDef`](crate::core::registry::OutputDef)),
and its value is an [artifact set](crate::core::artifact::ArtifactSet): the artifacts of
one output, each one file's worth of bytes (a folder artifact holds the files of a
folder), at most one of which is the *entry artifact*, the one that stands for the
output as a whole. `--output NAME` prints the table that holds that set, the same way
as `--table` prints any other table. An artifact's bytes are printed in the JSON form
that the parser toolkit gives binary data, the object `{"$bytes": "<base64>"}`:

```text
$ flm dump document.flm --output text --width 30
{
  "flm.output.text.document": {
    "artifacts": [
      {
        "id": "text:document",
        "entry": true,
        "media_type": "text/plain",
        "data": {
          "$bytes": "SW50cm9kdWN0aW9uCj09PT09PT09PT09PQoKSGVsbG8gd29ybGQsIHNlZSBJbnRyb2R1Y3Rpb24uCg=="
        }
      }
    ],
    "violations": []
  }
}
```

A folder artifact prints its `media_type` as the empty string and its `data` as an object
that maps the path of each of its files, relative to the folder, to the file's bytes in
the same form: `"data": {"check.svg": {"$bytes": "…"}, "more/cross.svg": {"$bytes":
"…"}}`. The standard package's outputs hold none; a folder artifact comes from a compose
template that ships a folder, which `flm dump` does not apply, since it loads no extension.

The outputs of the standard package are `text`, `latex-rendered`, `latex-recomposed` and
`html`, each produced by the workflow of its name. The LaTeX for LuaLaTeX is always two
artifacts, `latex-rendered:document` and its style file `latex-rendered:flmx.HASH`. The HTML output is one artifact, `html:document`,
of media type `text/html`: a complete page. An unknown name fails and lists the outputs
that exist, as `--table` does; an output that the chosen workflow does not produce fails
and gives that workflow, as a table does. The LaTeX output is two artifacts (the
recomposed LaTeX, `latex-recomposed:document`, which is the entry artifact, and the style
file beside it, `latex-recomposed:flmrecomposed.HASH`, named after the render option
`style_stem` and a digest of its content, both of media type `application/x-latex`), or
the recomposed LaTeX alone, for a document that needs nothing in a style file. `flm dump`
gives the style file no prefix, since it writes no file (the file would be
`flmrecomposed.HASH.sty`):

```text
$ flm dump document.flm --workflow latex-recomposed --output latex-recomposed
{
  "flm.output.latex-recomposed.document": {
    "artifacts": [
      {
        "id": "latex-recomposed:document",
        "entry": true,
        "media_type": "application/x-latex",
        "data": { "$bytes": "…" }
      },
      {
        "id": "latex-recomposed:flmrecomposed.1a2b3c4d",
        "entry": false,
        "media_type": "application/x-latex",
        "data": { "$bytes": "…" }
      }
    ],
    "violations": []
  }
}
```

To read the rendered text itself rather than its base64 form, ask for the table that the
text backend writes before it packages the bytes,
[`flm.output.text`](crate::stdcore::backends::text::TEXT_OUTPUT): it holds the printed
text and the position marks that say which byte range of it each element produced.

### The two views

`--view` chooses how a *tree* is printed. Tables and outputs are always printed as
projections, so `--view` has no effect on them.

`--view compact`, the default, prints the library's **projection** of the tree: a
one-way, readable view in a vocabulary that the library owns and keeps stable (`elem`,
`chars`, `args`, `children`, `body`, `payload`), as indented JSON. It cannot be read back
into live objects, and it never shows parsing states or node identifiers. Every example
above uses this view. See [`crate::dump::project`].

`--view wire` prints the serialization of the tree by [techy](crate::techy), the parser
toolkit FLM is built on, as a single JSON line. That form is lossless and reads back into
live objects with their sharing intact, which the projection does not; it is
correspondingly verbose, and carries the sources, the parsing states and the token rules
beside the tree. See [`crate::dump::session`]. Under `--at parsed` it writes the whole
fragment (its source, its tree, its diagnostics and its layout) and records it as the
segment's main entry, so that reading the line back gives a fragment rather than a bare
tree.

```text
$ flm dump hello.flm --view wire
{"version":1,"meta":{},"tables":[{"name":"sources","table":0,"start":0,"entries": …
```

### Selecting the run

`--workflow NAME` chooses the [workflow](crate::core::registry::WorkflowDef) to plan and
run. The default is [`text`](crate::stdcore::backends::text::WORKFLOW), which renders the
document as plain text. The standard package also has
[`latex-recomposed`](crate::stdcore::backends::latex_recomposed::WORKFLOW), which
recomposes the document into LaTeX; [`latex-rendered`](crate::stdcore::backends::latex_rendered::WORKFLOW), which renders it as fully
resolved LaTeX for LuaLaTeX; [`html`](crate::stdcore::backends::html::WORKFLOW),
which renders it as an HTML page (the three are described in
[the rendering chapter](crate::guide::rendering)); and
[`check`](crate::stdcore::CHECK_WORKFLOW), which runs every check of the registry and
produces no output.

The four rendering workflows run the same checks as `check`, because each one also asks
the plan for the collection of checks [`flm.validated`](crate::stdcore::VALIDATED). A
document therefore gets the same diagnostics under `--workflow text`, `--workflow html`,
`--workflow latex-recomposed`, `--workflow latex-rendered` and `--workflow check`, with one exception: an unresolved
reference is a Warning under `check` and an Error under a rendering workflow. The
`check` workflow runs these checks and produces no output. It is worth running beside a LaTeX render for one
thing that the render cannot report: a user definition that never stops expanding is
expanded by `check`, which reports it, while the LaTeX render writes it out as a LaTeX
definition and leaves the endless expansion to `pdflatex`, which stops with "TeX capacity
exceeded" or does not finish (see [the rendering chapter](crate::guide::rendering)).

Under `--workflow latex-recomposed`, `--at lowered:latex-recomposed` prints the tree of
LaTeX construct elements that the backend built, `--table` reaches the tables that the
workflow writes, and `--output latex-recomposed` prints the recomposed LaTeX and its
style file. Under `--workflow html`, the same views are `--at lowered:html`, the tree of
presentation elements that the HTML backend built; `--table flm.output.html`, each tree's
HTML with the identifiers of the definitions it requires; `--table
flm.html.definitions`, the definitions table shown above; and `--output html`, the page.

`--width N` sets the text backend's line width in columns: the
[`TextRenderOptions`](crate::stdcore::backends::text::TextRenderOptions) value that the
run reads, whose default is 80. It is the only render option that `flm dump` sets. The
render options of the other backends (for `latex-recomposed`, the packages its style file
requests, the block of LaTeX written at the end of that file, the Unicode mode, the LaTeX
hooks; for `html`, the page's title and language, whether each element carries its place
in its source, and how each stylesheet and script reaches the page) are set by a program
that embeds the library ([Embedding flm-core](crate::guide::embedding)). `flm dump`
renders with their defaults, so a page it writes carries the default title and loads the
formula renderer from the address that the definition gives. `flm dump` reads no front
matter and takes no `-O`; to render with settings, use `flm render` (see
[Rendering documents from the command line](#rendering-documents-from-the-command-line)).

`--workflow` and `--width` apply to a run, and `--at parsed` performs none. Giving either of them with
`--at parsed`, including when `--at` is left at its default, is therefore a usage error
rather than being silently ignored; otherwise a misspelled workflow name would pass
unnoticed. Use them with `--at frozen`, `--at lowered:…`, `--table` or `--output`.

### Diagnostics of `flm dump`

A [diagnostic](crate::guide::concepts_overview#diagnostics) reports a problem with a
document: a severity (Error, Warning or Note), a position, and a typed condition with
its data, with a message written for a person. `flm dump` prints every diagnostic, from
the parse, the registry build or the run, on standard error, so standard output holds
the requested JSON and nothing else:

```text
$ flm dump bad.flm --at frozen --workflow check
warning: the reference points at the label `sec:nowhere`, which is not defined
  at: @ (line 1, col 5) [bad.flm]
[
  {
    "chars": "See "
  },
  …
]
```

Diagnostics do not decide the exit status: a document that reports errors still prints
its view and exits with status 0. A program that wants to fail a build on them reads the
standard error stream, or embeds the library and asks the run
([`Run::diagnostics`](crate::core::run::Run::diagnostics)).

### Exit status of `flm dump`

| status | when |
|---|---|
| 0 | the requested view was printed |
| 1 | a usage error; a file that cannot be read; a registry, plan or run that cannot be built or fails; a name that matches nothing (an unknown table, output or backend); or a table or output that the chosen workflow does not write |
| 2 | the tree asked for does not exist: the parse produced none, a rewrite stage produced none, or the tree was not lowered for the backend asked for |

Status 2 is what a document nested too deeply to process gives, for example: the parse
refuses it rather than overflowing the stack, prints the message of the condition
[`NestingTooDeep`](crate::lang::NestingTooDeep) (`flm.blocks.nesting-too-deep`) on
standard error, and there is no tree to print.

## Mathematics processed by MathJax

Mathematics in an FLM document is read by a *processing feature*: the feature that
decides which commands a formula may hold, checks the formulas and writes them into the
output (see [mathematics](crate::guide::language_syntax#mathematics)). The program's
default processing feature is that of the extension `math-mathjax`, written in JavaScript, which runs
MathJax itself, so that a formula holds only commands MathJax knows, and an HTML page can
carry each formula as MathML, with no script. The library's package `math-passthrough` is
another processing feature: it reads nothing inside a formula, accepts any command and
carries the source to the output, where MathJax typesets it in the reader's browser.

**Using it.** The extension is compiled into the program (its source is the folder
`extensions/js/math-mathjax/` of the FLM repository), and the document's math processor
is `mathjax` by default ([The packages a document uses](#the-packages-a-document-uses)),
so a document uses it without specifying it, and nothing needs to be installed. This
default is not guaranteed to stay the same in later versions of the program: a document
that needs a stable result specifies its processor with `math: mathjax` in its front
matter. A modified copy, named `math-mathjax`, in the document's `_flmextensions` folder
or another folder of the search path
([Where extensions are looked for](#where-extensions-are-looked-for)), is used in its
place.

**Choosing the other processor.** A document uses `math-passthrough` by choosing the
processor `passthrough`, in the front matter or with `--math passthrough`:

```yaml
flm:
  math: passthrough
```

`math: false` (or `--math false`) uses no processor, for a document that lists one under
`use`. Two processing features at once are an error: a document that lists
`math-passthrough` under `use` and keeps the processor `mathjax` has both. With no
processing feature (`math: false` and no processor under `use`, or the processor's
feature switched off, `math-mathjax: false`), a document without formulas renders as any
other, and a render reports each formula as the error `flm.math.unprocessed` and writes
its content as ordinary text.

**Strict commands.** The commands and environments a formula may hold are those that
MathJax's TeX packages define, read from MathJax when the extension loads, the two
delimiters the extension defines beyond them (`\llangle` and `\rrangle`, whatever
`packages` lists), and the `math` feature's own (the text commands such as `\text{…}`,
`\ensuremath{…}`, and inside a display-math environment the commands of an equation line:
`\\`, `\label`, `\tag`, `\nonumber`). Any
other command inside a formula is an error where it is written: `cannot resolve command
‘\dv’` for `\(\dv{x}{t}\)` without the `physics` package. Some of MathJax's commands are
left out, so that a formula does not depend on another formula and numbering stays the
`math` feature's: those that define something or change a setting for later formulas
(`\DeclareMathOperator`, `\definecolor`, `\mathtoolsset`, …), those of equation numbering,
labels and references outside an equation line (`\label`, `\tag`, `\eqref`, …), and the
display-math environments (`equation`, `align`, …) inside a formula; the file
`README.md` of the extension lists them. No command can be defined
inside a formula (`\newcommand` there is an unknown command); a user definition of the
`userdefs` feature applies inside formulas as usual. A user definition of a command that
a loaded MathJax package also defines makes the program stop with an error naming both
features.

**The two modes.** The setting `mode` chooses how the HTML page carries the formulas:

- `load-in-browser`, the default: the page holds each formula's TeX, and MathJax, loaded
  into the page, typesets it in the reader's browser, as under `math-passthrough`, but
  with the commands checked. Choose it for MathJax's own typesetting in the page.
- `mathml`: MathJax runs while the document is prepared, and the page holds each formula
  as MathML, which the browser lays out with no script. A formula MathJax cannot typeset
  is the error `MathJax cannot typeset the formula: …` with MathJax's reason, and the
  page shows its source as text. Choose it for a page that must show its mathematics
  without running a script.

On both LaTeX outputs and on the text output, the formulas are written as the author
wrote them, with their numbers, in both modes.

**Text inside formulas.** In both modes the argument of `\text{…}` and of the other text
commands is document content placed inside the formula: a reference in it is a link, and
a user definition used in it is rendered as anywhere else in the text. Each equation
line's number or tag is content too, and a reference to a line leads to it.

**The settings.** They are written under `flm.features.math-mathjax` in the front matter,
or with `-O math-mathjax.KEY=VALUE`:

| setting | default | what it sets |
|---|---|---|
| `mode` | `load-in-browser` | `load-in-browser` or `mathml` |
| `packages` | every package the extension holds | the MathJax TeX packages whose commands a document may use; a name the extension does not hold is an error |
| `math_font_url` | none | in `mathml` mode, the address of a web font with mathematical tables, used before the math fonts installed on the reader's system |
| `loader_url` | `https://cdn.jsdelivr.net/npm/mathjax@4/tex-chtml.js` | in `load-in-browser` mode, where the page loads MathJax from |
| `lazy` | `false` | in `load-in-browser` mode, whether MathJax typesets each formula only when it scrolls into view |
| `preamble` | empty | TeX that MathJax processes once, before any formula, for the definitions and settings every formula shares (`\definecolor`, `\DeclareMathOperator`, …); the file `README.md` of the extension describes it |
| `export_commands` | none | the commands the preamble defines that a formula may use, each written without its backslash |

```yaml
flm:
  features:
    math-mathjax:
      mode: mathml
      packages: [base, ams, physics]
```

**The math font.** In `mathml` mode the page's stylesheet `math-mathjax.style` sets the
formulas in the font of `math_font_url` when it is set, then in Latin Modern Math, STIX
Two Math or Cambria Math where the reader's system has one, then in the browser's font
for mathematics. The text inside a formula is set in the page's font.

**Rebuilding MathJax.** The extension carries MathJax in its folder `vendor/`, generated
by a tool of the FLM source repository, in its folder `npm/math-mathjax/`, from the
MathJax version that `npm/math-mathjax/package.json` pins; the file `README.md` of the
extension says how to run the tool, and lists the packages the extension holds.

## Where to look next

- [FLM language syntax](crate::guide::language_syntax): the markup of the files that
  this program reads.
- [Embedding flm-core](crate::guide::embedding): driving the same library from a
  program of your own.
- [Stages, capabilities, and plans](crate::guide::pipeline): what `--at frozen`,
  `--table` and a workflow name refer to.
- [Rendering: backends and outputs](crate::guide::rendering): lowering, emitters,
  outputs and artifacts, which `--at lowered:…` and `--output` show.
- [Concepts overview](crate::guide::concepts_overview): the reference page for every
  term used above.
