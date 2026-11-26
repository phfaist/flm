# The flm command-line tool

`flm` is the command-line program of FLM, a LaTeX-like markup language. It renders a
document written in FLM into an HTML page, plain text or LaTeX, checks a document for
problems, shows a live preview of a document in a browser, prints the schema of its
settings files, and describes the commands, settings and workflows a document may use. The
program is built from the crate of the same name. The processing itself is done by two
libraries: the library [`flm-core`](crate) turns the text of a document into outputs, and
the host library `flm-host` reads the files, reads the settings written at the top of a
document, builds the library's [registry](crate::core::registry) from the packages that
the document uses, and writes the outputs.
`flm` turns a command line into calls of the host library, prints the problems found on
standard error, and exits with a status that says how the command ended.

The program has seven commands:

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
  program itself) as pages of documentation, printed as text or as JSON. See
  [Describing the environment: `flm info`](#describing-the-environment-flm-info).
- `flm dump FILE` is an inspection tool: it processes one file with the library alone and
  prints one view of the result as JSON (a tree at a chosen point of the processing, a
  computed table, or the artifacts of an output). See
  [Inspecting the processing with `flm dump`](#inspecting-the-processing-with-flm-dump).
- `flm cache list` and `flm cache clean` list and remove the data of the program's cache,
  the folder in which `flm render`, `flm check` and `flm watch` keep the results of
  costly work from one run to the next. See [The cache](#the-cache).

The settings of a document, written in its front matter and in the packages of the
extensions it uses, are described in
[Configuration files and document front matter](#configuration-files-and-document-front-matter).

**Process only documents that you trust.** A document that `flm` processes can make the
program read files and run code on your machine, with your rights:

- The document can make the program read any file that you can read: a file that it
  includes with `\input`, an image, or a file given in its settings, such as a
  bibliography file. A path may be absolute, or lead out of the document's folder (see
  [Paths](#paths)).
- The document can use extensions, including the extensions in the folder
  `_flmextensions` beside it (see [Where extensions are looked
  for](#where-extensions-are-looked-for)).
- An extension written in JavaScript or compiled to WebAssembly runs code. Through the
  service `programs`, which `flm` offers to every extension, such an extension can start
  any program on the machine (see [Extensions](#extensions)).

## Building and running the program

The program is the only binary of the crate `flm` and of this workspace. `cargo install
flm` installs it from the crates.io registry, once the crate is published there, and
`cargo install --path flm` installs it from a checkout of the repository. From the repository, the program is also built and run with

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
(see [Extensions](#extensions)) through the cargo features `js` and `wasm` of `flm`,
both on by default. A program built without one of them (`cargo build -p flm
--no-default-features --features js`, say) refuses an extension of the engine it lacks,
naming the engines it has.

### The compiled packages the program carries

A *package* is a named bundle of features together with the packages it builds on
([`PackageEntry`](crate::core::registry::PackageEntry)); a *feature* is a Rust value
that contributes a related bundle of things, such as commands an author can write,
processing stages and settings, to the library's processing
([`FlmFeature`](crate::core::registry::FlmFeature)). The program carries these
packages:

- `std`, the package every document uses unless its front matter says otherwise. It holds
  the library's standard features: the language's constructs, the numbering, the
  references, the floats and the other features of the document's content, and the
  standard backends and workflows. Then it holds the program's own features: `compose`
  and `compose-standard` (the compose templates), `write` (the output's files handed to
  standard output, a file or a folder), `preview` (the live preview), `graphics-files`
  (the image files) and `citations-standard` (the cited works). It processes no
  mathematics.
- `math-passthrough`, the library's math processor that reads nothing inside a formula
  (see [Mathematics processed by MathJax](#mathematics-processed-by-mathjax), *Choosing
  another processor*).

These two packages come with the host library. The `flm` program carries no other
compiled package; a program of one's own can carry more (see [Building a program that
carries other packages](#building-a-program-that-carries-other-packages)).

The extensions compiled into the program, `math-mathjax` and `phfpack` among them, are
packages too (see [Where extensions are looked for](#where-extensions-are-looked-for)).
The cargo feature `bundled-extensions` of `flm`, which is on by default, includes
them: it compiles in every extension of the crate `flm-extensions` (its cargo feature
`all`). A program built without that cargo feature (`cargo build -p flm
--no-default-features`) refuses a document that lists `phfpack`, as it refuses any
package it cannot find.

### The packages a document uses

A document uses these packages, in this order, each package after the packages it uses,
and each once:

1. the packages of the *initial list*, by default `std`, the standard features and the
   program's own;
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
package it leaves out is not used unless `use` lists it. A document takes the whole package
`std` or leaves it out of its initial list; it leaves out one feature of `std` by switching
that feature off (see *Switching features off* in [The front matter](#the-front-matter)).
A name that no package has is an error, reported where the name is written.

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
`math-mathjax`, which typesets formulas with MathJax; `techxt` uses the extension
`math-techxt`, which renders formulas as Unicode text ([Mathematics rendered as Unicode
text](#mathematics-rendered-as-unicode-text)); and `passthrough` uses the library's
`math-passthrough`, which reads nothing inside a formula. Without `math` or `--math`, the
program uses its default processor, `mathjax`; a program built without the cargo feature
`js` or without `bundled-extensions` has no `math-mathjax`, and its default processor is
`passthrough`. A program built without `bundled-extensions` has no `math-techxt`
either. The default processor is not guaranteed to stay the same in later versions
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

The library of the crate `flm` holds the whole command line. The function
`flm::run(args, carried)` takes the command-line arguments and a value `flm::Carried`
that lists what the program carries:

- `packages`, the compiled package factories that the program carries besides `std` and
  `math-passthrough`;
- `bundled`, the tables of the extensions compiled into the program, which
  `flm::shipped_extensions()` returns for the extensions shipped with FLM.

A program of one's own that carries other compiled packages is a `main` function of a few
lines that passes its own list of packages. The
example `own_package` of `flm` carries a package written in the example that way: the
package `shout`, whose one feature declares the macro `\shout{text}` as a user definition
(a macro defined by substitution, [author
definitions](crate::guide::language_syntax#author-definitions)) with the template
`\textbf{#{text}}`. A document that lists `shout` under `use` in its front matter can
write `\shout`:

```text
$ cargo run -p flm --example own_package -- render paper.flm
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
$ flm render -p tty paper.flm | less -R   # text for a terminal
$ flm render -c 'Some \emph{text}.' -p text
$ flm render -V -o paper.html paper.flm   # written, then opened
```

### What to build

These options are shared by `flm render`, `flm check` and `flm watch` (the preview
accepts neither `-` nor `-c`):

| option | what it does |
|---|---|
| `-p PROFILE` | the profile to build with, which determines what kind of output is produced: `html` (a complete HTML page), `text`, `latex-rendered` (LaTeX for LuaLaTeX, with everything FLM resolves already resolved), `latex-recomposed` (the document written back as LaTeX source), `bibtex` or `csl-json` (the records of the cited works as a BibTeX or a CSL-JSON file; see [Citations](#citations), "The exports"), `check` (no output), `pdf-latex` (a PDF file; see [PDF through LaTeX](#pdf-through-latex)), `tty` (the text with escape sequences for a terminal; see [Text for a terminal](#text-for-a-terminal)), or a profile that an extension provides. The profile PROFILE uses the extension `profile-PROFILE` when one exists, then runs the workflow PROFILE. Every format the workflow provides is produced, unless `-f` selects one. Default: for `flm render` and `flm watch`, the front matter's `default_profile`, else `html`; for `flm check`, `check` (see [Profiles and workflows](#profiles-and-workflows)) |
| `-f FORMAT` | only the format FORMAT, one of the formats that the active workflow provides (the workflow of the profile chosen with `-p`, or of the default one). `-f` does not choose the profile: another kind of output is produced by choosing its profile with `-p`. The program's workflows `html`, `text`, `latex-rendered`, `latex-recomposed`, `bibtex` and `csl-json` each provide the one format of the same name; a workflow that an extension declares may provide several. Default: every format the workflow provides |
| `--use PACKAGE` | use the package PACKAGE in addition to those of the front matter's `use`, as if it were written last in that list: a compiled package, or an extension found on the search path or compiled into the program (`--use syntax-highlight` highlights the code listings). Repeatable; the packages are used in the order given (see [The packages a document uses](#the-packages-a-document-uses)) |
| `--initial-use PACKAGES` | the packages to use before those of the front matter's `use`: one package's name, or a list in YAML's one-line form (`--initial-use '[std, myext]'`), used exactly as given. It takes precedence over the front matter's `initial_use`. Rarely needed, since it replaces the program's own list of packages: a package is added with `--use`. Default: the front matter's `initial_use`, else `std` (see [The packages a document uses](#the-packages-a-document-uses)) |
| `--math PROCESSOR` | the math processor, the package that processes the document's mathematics: a processor's name, which uses the package `math-NAME` (`--math passthrough` uses `math-passthrough`), or `false` for no processor. Its package is used after the packages of the initial list and before those of `use`. It takes precedence over the front matter's `math`. Default: the front matter's `math`, else `mathjax` (`passthrough` in a program built without `js` or `bundled-extensions`); this default is not guaranteed to stay the same (see [The packages a document uses](#the-packages-a-document-uses)) |
| `-t TEMPLATE` | the compose template that assembles the output's main file: one of the program's own, which `flm render --help` lists for each output, or a compose template that an extension the document uses ships. Default: `simple` (see [Compose templates](#compose-templates)) |
| `-O FEATURE.KEY=VALUE` | a setting of a feature, the value written in YAML's one-line form; repeatable (see [Settings on the command line](#settings-on-the-command-line)) |
| `--layout blocks\|inline\|auto`, `-B` | how the document's content is read at its top level: `blocks`, as paragraphs and blocks; `inline`, as one run of text; `auto`, as paragraphs and blocks when the content holds a paragraph break or a block, and as one run of text otherwise. Default: the front matter's `layout`, else `auto`. `-B` is the same as `--layout blocks` |
| `--extensions-dir DIR` | a folder in which extensions are looked for by name (see [Where extensions are looked for](#where-extensions-are-looked-for)); repeatable, and each folder must exist. Default: the folder `_flmextensions` beside the document. The extensions compiled into the program are found after every folder |
| `-v` | print on standard error what the program does: the files it reads, the profile, the packages and the workflow it uses, and the files it writes, each list once the step that made it is over; while it fetches the details of cited works, each request it sends and each wait; and the lines that extensions write on their console at the level of information (their warnings and errors are printed without `-v`). See [Progress of a build](#progress-of-a-build) |

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
text, `ansi` for the text for a terminal, `tex` for either LaTeX output, `bib` and `json`
for the exports), and `NAME` as written otherwise. With `-o FOLDER/`, the stem is `index`. The main file of each output is
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
file `refs`). It may hold a folder (`exports/refs.bib`); its `.` and `..` are resolved as
text, each `..` going up one folder (`exports/../refs.bib` is the file `refs.bib` of the
output folder). The setting can be written in three
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
nothing is written. A path that leaves the output folder (an absolute path, or one whose
`..` climbs above the output folder), and two outputs that would write different files at
one path, are errors too, as for any other name. Under `-o -`, the name
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
  feature, and the program prints a warning for it.
- When no extension `profile-NAME` exists, nothing is added: the workflow `NAME` must then
  be one that the packages the document uses declare. The program's own workflows are such
  workflows, and so is a workflow that an extension listed under `use` declares.
- When the extension exists but cannot be read (a manifest that is not valid YAML, for
  example), the command stops with status 2, as for an extension listed under `use`.

The program carries two such extensions compiled in. The profile `pdf-latex` is the
extension `profile-pdf-latex`, which declares the workflow `pdf-latex` and its output
`pdf-latex`, the PDF file (see [PDF through LaTeX](#pdf-through-latex)). The profile `tty`
is the extension `profile-tty`, which declares the workflow `tty` and its output `tty`, the
text for a terminal (see [Text for a terminal](#text-for-a-terminal)). `flm render -p
pdf-latex` and `flm render -p tty` need nothing in the front matter.

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
`flm-region-flm.metadata.title`, `flm-region-flm.toc`, `flm-region-flm.xnotes.list.footnote` (in a CSS selector,
each `.` of such an identifier is escaped: `#flm-region-flm\.metadata\.title`). In the
recomposed LaTeX, `simple` places the title, the authors and the date, and leaves the
notes to LaTeX. A part that the compose template does not place is not computed at all.

The compose template is the setting `template` of the feature `compose`: one name for
every output, or a map from an output's name to a compose template's name (`-O
'compose.template={html: none}'`); in the front matter, `flm: {features: {compose:
{template: none}}}`. A compose template's own settings are written under
`template_config`, under the output's name and then the compose template's name, over its
defaults: `-O 'compose.template_config={OUTPUT: {NAME: {KEY: VALUE}}}'`. The output's
level keeps apart two compose templates of the same name for two outputs. For instance,
`article` for `latex-rendered` has the settings `documentclass` (`article` by default) and
`classoptions` (empty by default): `-O 'compose.template_config={latex-rendered: {article:
{documentclass: report}}}'`. A name that matches no compose template of the output stops
the command (status 2). The feature `compose` can be switched off like any other
(`compose: false`); the output is then assembled by the backend's own [final
stage](crate::guide::concepts_overview#output-regions-and-the-final-stage), the library's
assembly of the output's main file, which writes a complete page in the HTML output.

Some outputs are made from the files of another output instead of from the document
itself: an output of PDF files, for instance, is made by compiling the files of the output
`latex-rendered` with LaTeX. When the output asked for is made from another output, the
compose template is chosen for that other output as well, as if it were asked for too:
`-t NAME` then chooses the compose template `NAME` of `latex-rendered`, and so does `-O
'compose.template={latex-rendered: NAME}'`. A name that matches no compose template of
`latex-rendered` stops the command (status 2), with an error that mentions
`latex-rendered`.

**The settings of a compose template.** Each compose template declares its own settings,
with a type, a description, a default value and marks (see [Extensions](#extensions) for
the declaration in a description). The settings that a document, a package or `-O` write
under `compose.template_config.OUTPUT.NAME` are combined key by key, at each of the three
levels `template_config`, `OUTPUT` and `NAME`, over the declared defaults: a package that
sets `documentclass` and a front matter that sets `classoptions` both take effect. Within
one setting, the marks of its declaration decide how sources combine (see [The marks of a
setting](#the-marks-of-a-setting)); a setting without such a mark is replaced whole by
the source of the highest rank. A key that the compose template does not declare is left
out of what the compose template reads, with a warning at the key `template_config` that
writes it. A setting that the declaration marks `path` names a file: a bare path in it
is relative to the place where it is written (see [Paths with a
root](#paths-with-a-root)). `flm schema --of front-matter` describes the declared settings
of the compose templates of the packages a document uses.

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
artifacts.logo }}`), before it is written. A compose template may also ship a file that
it selects while it is applied (see *Files a compose template ships while it is applied*
below). The program's own compose templates ship no file.

A compose template may also ship a whole folder of the extension, such as a folder of
icons, as one shipped file: the folder is written beside the main file as `STEM-NAME`,
with no extension, and each of its files under it with its own path (`paper-icons/check.svg`,
`paper-icons/more/cross.svg`), at every depth. The template writes the folder's path as `{{
artifacts.NAME }}`, with no `/` at its end, and a file inside it as `{{ artifacts.icons
}}/check.svg`. The names in the folder that begin with `.` (such as `.DS_Store`) are left
out. The record of the written files lists each file of the folder, so a file removed from
the folder is removed from the output folder at the next render.

**Files a compose template includes.** A compose template of the engine `minijinja` may
build its text from other files and texts when it is applied, with four functions. Three
of them, `include_raw`, `include` and `include_artifact_url`, read a file given by a path.
The path follows the rules of [Paths with a root](#paths-with-a-root): it begins with a
root, a word followed by `:` that names the folder of the path (`ext+NAME:`, `std:`,
`doc:`, `cwd:`, `self:`, or the address `https://…`), or it is a *bare path*, with no
root, which is relative to the folder of the file in which it is written. That file is
the compose template's text, a partial it includes, a file it ships and fills in as a
template, or a file of another package that it extends. For instance,
`partials/head.html` written in `ext+notes:templates/html/notes/notes.html` reads
`ext+notes:templates/html/notes/partials/head.html`.

The four functions:

- `{{ include_raw("dist/script.js") }}` writes the file's text as it
  is: nothing in it is filled in, and in an HTML page nothing in it is escaped. The file
  must be text in UTF-8. The author of the template is responsible for what such a file
  expands into.
- `{{ include("doc:footer.html") }}` fills in the file as a template of the
  same engine, with the same delimiters, and writes the result in its place.
- `{{ eval(config.footer) }}` fills in a text as a template in the same way: here, the
  text of the template's setting `footer`, such as `'{% for x in config.items %}{{ x }}
  {% endfor %}'`. A text given to `eval` is read from no file, so it has no folder: a path
  inside it begins with `doc:`, `ext+NAME:` or `std:`, as in `'{{
  include_raw("ext+mytemplate:style.css") }}'`.
- `{{ include_artifact_url("doc:logo.png") }}` ships the file beside the
  output's main file and writes the path of the shipped file (see *Files a compose
  template ships while it is applied* below).

The file that `include` fills in, and the text that `eval` fills in, are called the *inner
template*; the text that holds the call is the *outer text*. An inner template reads:

- the values of the compose template: `regions`, `pieces`, `metadata`, `config` and
  `artifacts`;
- the named values written in the call: `{{ include("box.html", color="teal") }}` gives
  the inner template the value `color`;
- the named values given to its outer text, when that text is itself an inner template:
  a named value is passed on to every inner template below the call that gives it.

A named value of the call takes precedence over a value of the same name given to the
outer text, and both take precedence over a value of the compose template. An inner
template does not read the variables that its outer text sets for itself (with `set`, or
as a loop variable); pass such a variable as a named value. The name `html` is an
argument of the functions and cannot be passed as a value. The functions and filters of
the engine work in an inner template, and inner templates nest to at most eight levels.

The argument `html` of `include` and `eval` states whether the inner template is HTML.
With `html=true`, every value that the inner template writes is escaped for HTML; with
`html=false`, every value is written as it is. Without `html`, the inner template is
escaped as its outer text is: in an HTML page, its values are escaped for HTML. A
stylesheet filled in inside an HTML page is therefore given `html=false`, as in `{{
eval(config.css, html=false) }}`.

The engine's own statement `include` (`{% include "partials/header.html" %}`) also fills
in a file as a template, with the same values as the text that holds it, and the
statements `extends` and `import` read a file in the same way. Their paths follow the same
rules as the functions' paths: `{% extends "ext+basetemplate:templates/html/base/base.html"
%}` fills in a text of another extension, and a bare path written in that text is read in
that text's own folder. The statement `import` gives the variables that the imported file
sets (`{% import "partials/colors.html" as colors %}`, then `{{ colors.accent }}`). The
engine has no statement `macro` or `call`, so a compose template cannot define a macro.
Since a file is read when the template is filled in, its path may be computed (`{%
include "partials/" ~ config.variant ~ ".html" %}`).

The functions and the statements work in the compose template's text, in a shipped file
that is filled in as a template, and in an inner template. `flm watch` builds the page
again when an included file of the document's folder or of an extension found in a
folder changes. A file that a call cannot use stops the render with an error that gives
the compose template and the file by its path with its root (`Missing file
‘ext+notes:dist/x.js’`): a file that is missing, that leaves the extension's folder or that
cannot be read, and a file of `include_raw` or `include` that is not text. A failure
inside an inner template gives the place of the call, then the inner template (its path,
or the text given to `eval`), then the failure with its line in the inner template.

**Files on the network.** `include_raw` and `include_artifact_url` also read an address
`https://…`. The program downloads the file and keeps it in its cache ([The
cache](#the-cache)). `include_raw` writes the downloaded text (`{{
include_raw("https://example.org/fonts.css") }}`), and `include_artifact_url` ships the
downloaded file under the last part of the address's path, without the query. The function
`include` and the statements `include`, `extends` and `import` never fill in a downloaded
text, and stop the render on an address. A template that gives a downloaded text to
`eval`, as in `{{ eval(include_raw("https://example.org/part.html")) }}`, fills it in as
a template: its author wrote that combination on purpose, and is responsible for the text
it downloads. A downloaded file is kept in the cache for seven days, and is downloaded
again after that. When a download fails and the cache holds a copy of the file, the copy
is used, with a warning once it is older than its seven days; without a copy, the failed
download stops the render.

**A package that builds on another package's compose template.** A package can use the
compose template of another package and give it files of its own. For instance, the
extension `basetemplate` ships the compose template `base`, which declares two settings:
`style`, a text that it fills in as a template, and `logo`, the path of an image that it
ships:

```yaml
# basetemplate/templates/html/base/flmtemplate.yaml
name: base
output: html
engine: minijinja
main: base.html
accepts: all
settings:
  type: object
  properties:
    style: {type: string, default: '', description: A stylesheet, filled in as a template.}
    logo: {type: string, default: logo.png, description: The logo., x-flm-marks: [path]}
```

```html
<!-- basetemplate/templates/html/base/base.html -->
<style>{{ eval(config.style, html=false) }}</style>
<img src="{{ include_artifact_url(config.logo) }}" alt="">
{{ regions.content }}
```

The default `logo.png` is read beside the description, as
`ext+basetemplate:templates/html/base/logo.png`. The extension `mytemplate` uses
`basetemplate`, selects `base`, and gives it a stylesheet and a logo of its own:

```yaml
# mytemplate/flmextension.yaml
flm:
  use: [basetemplate]
  features:
    compose:
      template: {html: base}
      template_config:
        html:
          base:
            style: '{{ include_raw("ext+mytemplate:style.css") }}'
            logo: logo.png
```

The setting `logo` is marked `path`, so the bare path `logo.png` written in the manifest
of `mytemplate` is `ext+mytemplate:logo.png`, the file beside that manifest. The text of
`style` is filled in by `eval`, so its path states the root `ext+mytemplate:`. A document
that lists `use: [mytemplate]` gets the compose template `base` with the stylesheet and
the logo of `mytemplate`. Its front matter may change one more setting, or the logo again,
under `compose.template_config.html.base`: a `logo: mylogo.png` written there is
`doc:mylogo.png`, a file of the document's folder.

Among the program's own compose templates, `blogpost` and `notebook` include files of
their own: each writes its stylesheet and its script into the page with `include_raw`, and
`blogpost` builds its page from partial templates with the statement `include`.

**Files a compose template ships while it is applied.** The function
`include_artifact_url` reads a file, by its path as the other functions do, ships it beside the output's main
file, and writes the path of the shipped file, relative to the folder of the main file:

```html
<img src="{{ include_artifact_url("doc:logo.png") }}" alt="Logo">
```

The file need not be text. It is written as `STEM-NAME-DIGEST.EXT`, where `STEM` is the
stem of the main file, `NAME` and `EXT` are the stem and the extension of the file's own
name, and `DIGEST` is eight hexadecimal digits computed from the file's content and media type:
under `-o
paper.html`, `logo.png` is written as `paper-logo-20c3c289.png`. A file shipped twice is
written once, and both calls write the same path. The file's media type is the argument
`media_type` when the call gives it, and otherwise the media type of the file's extension.
The program knows the media types of the extensions `html`, `css`, `js`, `txt`, `json`,
`svg`, `png`, `jpg`, `jpeg`, `gif`, `webp`, `pdf`, `tex`, `sty`, `bib`, `woff`, `woff2`
and `ico`; a file with another extension needs the argument, as in `{{
include_artifact_url("data.bin", media_type="application/octet-stream")
}}`. A file whose name has no extension cannot be shipped this way.

**Writing a text inside a style element, a script element or a comment.** Inside a
`<style>` element, a `<script>` element and an HTML comment, a browser reads no character
entity, so a text written there must not be escaped for HTML; it must only be kept from
ending the element or the comment. The filter `escape_for` writes a value's text encoded
for one of three places:

- `{{ css | escape_for("html-style") }}`, for a stylesheet inside a `<style>` element:
  every `</` is written `<\/`. CSS reads `\/` as `/`, and `</` can stand in a stylesheet
  only in a string, a comment or an address, so the stylesheet keeps its meaning and
  cannot end the element.
- `{{ code | escape_for("html-script") }}`, for a script inside a `<script>` element: in
  every `<!--`, `<script` and `</script`, in any letter case, the `<` is written `\x3C`.
- `{{ title | escape_for("html-comment") }}`, for a text inside a comment: a space is
  written between two adjacent hyphens (`--` gives `- -`), and a space is written before
  a text that begins with `-` or `>`. The text cannot then end the comment, wherever it is
  placed. The text is altered, since HTML has no way to write it unchanged in a comment.

The first two places are for a whole stylesheet or a whole script. A single value placed
inside a script is written with the filter `tojson` (`const data = {{ config | tojson
}};`). Another place stops the render with an error.

### The lecture-notes template

The repository ships the extension `lecturenotes` in the folder
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
a site's own page. The template has five settings, written under
`compose.template_config.html.lecturenotes`:

- `navigation_heading_level` (1 by default): the level of the headings that begin the
  document's units of reading, such as its chapters. The bar's title, the arrows and the
  folding of the sidebar follow the headings of this level.
- `top_heading_level` (1 by default): the level of the document's outermost headings; 0
  in a document with parts, which also sets the render option `html.root_heading_level`
  to 0.
- `font_family`: the font family of the page's text, as a CSS font-family list.
- `css`: a stylesheet of the document's own, filled in as a template and written in a
  `<style>` element of the page's head, after the link to the template's stylesheet (see
  [Compose templates](#compose-templates) for the functions it may call). The values
  `css_selectors.headings.top`, `.major`, `.navigation` and `.minor` select the headings
  and the sidebar's entries of each group of levels: `.flm-toc
  li{{ css_selectors.headings.navigation }}` selects the sidebar's entries at the
  navigation level. The text of `css` is read from no file, so a path written in it
  states its root (`doc:notes.css`, see [Paths with a root](#paths-with-a-root)). A file
  that `css` ships, such as a font shipped with `include_artifact_url`, is written beside
  the page.
- `links`: the links shown in the bar, each with `text` and `href`.

For instance:

```yaml
---
flm:
  use: [lecturenotes]
  features:
    compose:
      template_config:
        html:
          lecturenotes:
            navigation_heading_level: 1
            css: |
              :root { --ln-accent: #a71d5d; }
              {{ include_raw("doc:notes.css") }}
---
```

The file `extensions/data/lecturenotes/README.md` of the repository documents the page,
its settings, the groups of headings, and the use of its stylesheet and script outside
FLM.

### The letter template

The bundled extension `letter` provides letters for HTML and rendered LaTeX, with
European and US layouts and an optional logo. Loading it selects the compose template
`letter` for both outputs. Write one `letter` environment with ordinary FLM paragraphs
as its body:

```flm
---
flm:
  use: [letter]
---
\begin{letter}
\begin{fromaddress}
Alex Morgan\\
Example Organization
\end{fromaddress}
\begin{toaddress}
Dr. Taylor Reed\\
100 Example Street
\end{toaddress}
\letterdate{16 November 2026}
\lettersubject{A letter with FLM}
\salutation{Dear Dr. Reed,}

Thank you for your letter.

\closing{Yours sincerely,}
\begin{signature}
Alex Morgan
\end{signature}
\end{letter}
```

Each field is optional. Fields appear in the layout's order, independently of their
order in the source. `enclosed` adds enclosures as a command or a multiline environment.
The template adds no date, personal details or logo by default. Its settings go under
`compose.template_config.html.letter` and
`compose.template_config.latex-rendered.letter`. For example, use
`{logo: myletterhead.svg, letterhead: page-width}` for HTML and
`{logo: myletterhead.pdf, letterhead: page-width}` for rendered LaTeX. The settings
that name a file (`logo`, `custom_css_file`, `latex_preamble_file`) are paths: a bare
path is relative to the place where it is written, so `myletterhead.svg` in a front matter
is the file `doc:myletterhead.svg` of the document's folder, and the same text in the
manifest of an extension `mystyle` is the file `ext+mystyle:myletterhead.svg` of that
extension (see [Paths with a root](#paths-with-a-root)).
Provide a logo in a format supported by each output; the template copies the files
unchanged. PNG or JPEG can be used for both. A page-width letterhead extends across the side
margins; the body retains its text margins. `logo: placeholder` selects a generic
demonstration image in either output.

The file `extensions/js/letter/README.md` documents the fields, layout and typography
options, image formats, and custom CSS and LaTeX preambles. Use these customization
hooks for an institution's letter design. The profile `text` produces a plain-text
letter; the profile `pdf-latex` compiles the rendered LaTeX with LuaLaTeX.

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
latexmk -norc -lualatex -f -interaction=nonstopmode -file-line-error -auxdir=_aux paper.tex
```

there, reads the PDF back, and removes the folder, whatever happened (Ctrl+C, or another
signal that ends the command at once, may leave it in the system's temporary folder). The
option `-norc` keeps the configuration files of `latexmk` (`~/.latexmkrc`, for instance)
from changing the engine or the place where the PDF is written. LaTeX never runs with
`-shell-escape`, the option that would let the document start any program. To keep the
LaTeX files, render them with `-p latex-rendered`.

The option `-auxdir=_aux` puts LaTeX's auxiliary files (`.aux`, `.toc`, …) and its log
into the folder `_aux` of the temporary folder. That folder is the document's folder in
the program's cache (the cache `flm.ext.profile-pdf-latex.latexmk.aux`, kept for 30 days
after its last use; see [The cache](#the-cache)), so the next build of the same document
finds the auxiliary files of the last one, and `latexmk` runs LaTeX only as many times as
the changes since then need: often once. When the folder is not available (under
`--no-cache`, or while another build of the same document uses it), `_aux` is an empty
folder and LaTeX runs as on a first build.

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

**In the live preview.** `flm watch -p pdf-latex paper.flm` runs LaTeX at each build, and
the preview's tools page shows the pages of the PDF, drawn again after each build at the
same page and the same zoom (see [The tools page](#the-tools-page)). The build type
`pdf-latex` can also be chosen in the tools page's status bar while the preview runs.

**The WebAssembly form.** The same extension also builds as a WebAssembly module, which a
program that does not carry it compiled in finds on its search path. The script
`extensions/rust/build-wasm.sh` of the repository builds it (see the file
`extensions/README.md`).

### Text for a terminal

The profile `tty` produces a document as text for a terminal: the text that `-p text`
writes, with its images drawn and with escape sequences that the terminal interprets. A
heading's title and bold text are shown in bold, emphasized and italic text in italics,
underlined text underlined, and a link to an address outside the document is underlined and
is a hyperlink that the terminal can open. A link to a place inside the document, monospaced text, small capitals and
verbatim content are written plain. The headings, the rules under them, the frames of the
tables and the links are also colored, with the terminal's standard colors (a first-level
heading's title bright cyan, the deeper headings and the rules under the headings cyan, the
frames bright black, the links bright blue); the setting `tty.colors`
chooses `none`, `standard` (the default) or `rich` (`-O tty.colors=none` writes no color).
The profile is the extension `profile-tty` (see [Profiles and
workflows](#profiles-and-workflows)), written in Rust, in the folder
`extensions/rust/profile-tty/` of the repository, and compiled into the program. The
output's file extension is `ansi` (`-o out/` writes `out/index.ansi`).

The escape sequences are shown by a terminal, or by a pager that passes them on to the
terminal, such as `less` with its option `-R` (version 566 or later for the hyperlinks):

```sh
flm render -p tty paper.flm | less -R
```

The hyperlinks follow the OSC 8 convention; a terminal that does not support it shows a
link's text without the hyperlink. In a file or a terminal that does not interpret the
escape sequences, they appear as stray characters; `-p text` writes the same text without
them, and without the drawings of the images.

The text is laid out as `-p text` lays it out: wrapped at 80 columns, and a link to an
address outside the document followed by its address between angle brackets. The program
does not measure the terminal; the settings of the text output change both:

```sh
flm render -p tty -O text.width=120 -O text.link_addresses=false paper.flm | less -R
```

`text.width=null` never wraps the lines. The workflow `tty` also sets two settings of the
text output: the tables are framed with box-drawing characters (`text.table_frame=box`) and
the rules under the headings are drawn with `═` and `─` (`text.heading_rules=unicode`);
`-O text.table_frame=none -O text.heading_rules=ascii` gives the plain look back. These two
are settings of the text output, not of `tty`, because a frame changes where each character
of a table lands, which only the text output's layout decides, and the rules under the
headings are characters of the printed text in the same way; `-p text` takes them too.

The layout is the text output's because the profile `tty` makes no layout of its own: the
text output records which ranges of its printed text are a heading's title, bold or
emphasized text, a link or the frame of a table, and the extension writes those ranges with
escape sequences. The workflow `tty` is the workflow `text` with the output `tty` in place
of `text`, so it runs the same checks; it also sets the graphics settings and selects the
rule that draws the images.

Every line sets its styles again, so a line shown on its own is shown right; the spaces
that indent a line are never styled. The control characters a document can hold (in a
verbatim block, for instance) are written as the replacement character `�`, so the
document cannot send escape sequences of its own to the terminal.

**Images.** An image shown as a block (the content of a figure) prints its label,
`[image: <alternative text or address>]`, as `-p text` does, and under it the image drawn
with the half-block characters `▀`, `▄`, `█` and the space: one character per column, two
pixel rows per line, light pixels drawn and dark ones left blank, so that the drawing
reads without colors too. An image shown inline prints its label alone. Under
`tty.colors` other than `none`, each character takes the colors of the two pixels it
covers: their red, green and blue values under `rich`, the nearest of the sixteen
standard colors under `standard`.

The drawing has the size the image is shown at (see [Images](#images): the author's
`width=` and `height=`, else the size measured from the file, times the setting
`graphics.magnification.text`), divided by the size of a terminal cell, which two
settings give in points: `tty.graphics_cell_width_pt`, 6 by default (a column of the text
output is 6 points wide, so an image `1in` wide takes 12 columns), and
`tty.graphics_cell_height_pt`, 12 by default (a terminal line is about twice as tall as a
column is wide). They are set on the command line, `-O tty.graphics_cell_height_pt=14`, or
by a workflow's settings, not by the front matter; a value that is not a positive number
is an error, and the default applies.

The images are read and converted by the graphics feature, as for every output: the
workflow `tty` sets the text output's strategy to `content`, which keeps each image's
content for the profile, and a rule of `graphics.convert.text` that converts an SVG or PDF
drawing into a PNG image, which needs ImageMagick (see [Images](#images)). A PNG, JPEG, GIF
or WebP image needs no conversion. `-O 'graphics.convert.text=[{from: svg, to: png}]'`
converts SVG drawings only, and `-O graphics.strategy.text=reference` keeps no content, so
only the labels are printed. An image that cannot be drawn (a drawing that could not be
converted, a file that is not a valid image) prints its label alone, with an error.

The printed text holds the characters of a drawing but not their colors, so the profile
finds them again: the text output names, for each range of its text, the part of the
document that printed it, and a verbatim block that stands for an image is drawn again from
the image's content at its printed size; the colors of the drawing whose characters are the
printed ones are written. A verbatim block that an author wrote is never colored.

A document that switches the feature `text` off cannot be built with the profile `tty`,
since the workflow `tty` extends the workflow `text`: the command stops with status 2.

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

- In the HTML output, the compose template `simple` writes the table of contents inside
  `<main>`, between the page's header and the content, as a `<nav>` of nested lists, each
  entry a link to its heading, inside `<div id="flm-region-flm.toc">`; the live preview
  (`flm watch`) updates that part in place when the headings change, as it does every
  placed part. Without a page
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
or on standard input, it is relative to the current folder. The image's path and the
folders are written as every path of a document is ([Paths](#paths)). The default is
that folder alone, `.`. No extension is added to a path that has none. A list of folders is written
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

*How it is published.* Each output reaches its images by one of four *strategies*, set
by the setting `strategy` of the feature `graphics`, a map from an output's name to a
strategy:

| strategy | the output writes | the program's default for |
|---|---|---|
| `reference` | the file's path as seen from the folder the output is written in: `../figures/plot.png` for `-o site/paper.html` | `text`, `latex-rendered`, `latex-recomposed` |
| `copy` | a copy written beside the output, named after the stem, the file and a digest of its content (`paper-plot-3fa9c2d1.png` for `-o paper.html`, `index-plot-3fa9c2d1.png` for `-o site/`) | — |
| `data-uri` | the file's content, in the page itself (`data:image/png;base64,…`): the page needs no other file | `html` |
| `content` | the file's path, as under `reference`; the file's content is kept for an output that draws images itself | — |

`-O graphics.strategy.html=copy` sets one output's strategy and leaves the others'; so
does a front matter's `flm: {features: {graphics: {strategy: {latex-recomposed:
copy}}}}`, which leaves the page's `data-uri`. The plain text is one file and cannot hold
a copy, and only a web page can show a `data:` address: either request is an error at
the image, which is then published by reference. An image given by a web address
(`https://…`) is written unchanged under every strategy, and is neither read nor
measured.

*Conversions.* Under `copy`, `data-uri` and `content`, the setting `convert` of the feature
`graphics`, a map from an output's name to a list of rules, turns an image of one format
into another. A rule states the formats it applies to (`from`, one or a list of `png`,
`jpeg`, `gif`, `webp`, `svg`, `pdf`), the format it makes (`to`), optionally the
converters allowed to make it (`via`, by name), and options (`dpi`, the resolution at
which a drawing is turned into pixels; `transparent_bg`). The program's rules are: for
`html`, a PDF drawing into a PNG image at 192 dots per inch; for `latex-rendered` and
`latex-recomposed`, an SVG drawing into a PDF file and a GIF image into a PNG image, which
change what that output includes only where its strategy is `copy`, since under
`reference` and `content` it writes the path of the original file. A rule is performed
by the first converter, in the order below, that its `via` allows (every converter when
`via` is not written) and that can make its pair of formats; a converter that another
feature contributes is asked before these when the feature order places that feature
after `graphics-files`, and after them otherwise:

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
image that needs it, and its result is kept in the program's cache (the cache
`flm.graphics-files.magick.converted`, for 90 days after its last use; see [The
cache](#the-cache)), so an image shown twice, or rendered again by a later command, is
converted once. The key of a result holds the program's version (the first line that
`magick -version` writes), both formats, the options of the conversion and the image's
content, so a changed image or a new version of ImageMagick is converted again. On a
machine without ImageMagick, or where the program fails, the image
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
a record of one name, a file listed later wins over an earlier one. The setting is marked
`path`: a relative path is relative to the place where it is written (see [Paths with a
root](#paths-with-a-root)), the document's folder for its front matter, the extension's
folder for the manifest of an extension written as a folder, and the current folder for
an `-O` setting. An extension written as one file has no folder of files, so it names a
bibliography file with `doc:PATH`. The
files are read again at every build, so a file changed between two builds changes the
citation entries, and they are among the files that `-v` lists as read. A record may
carry its citation entry ready-made: in FLM, under the field `_ready_formatted: {flm: …}`
(or the older `_formatted_flm_text`), shown as FLM; under another markup's name alone,
shown as the characters written.

*The cache file and the central cache.* The records fetched from the arXiv and doi.org
are kept in two places, each for ten days (the arXiv's) or 360 days (doi.org's), and
fetched again after that; the records of bibliography files and `manual` keys are kept
in neither.

- The file `.flm-citations.jsonl` beside the document (in the current folder for a
  document given as text) holds the records of this document. The file is plain text,
  one record per line, in an order that keeps its changes small: kept under version
  control, it lets the document be rendered without the network. While it runs, `flm`
  also writes files whose names begin with `._.flm-citations` there, and removes them:
  one rule, `._.flm-citations*`, keeps them out of version control.
- The *central cache*, `flm.citations-standard.autocitefetch.items` in the program's cache
  (see [The cache](#the-cache)), holds the records fetched for all documents. A document
  whose file does not hold a record, or holds a copy that expires earlier, takes the record from
  the central cache, with no request, and the record is written into the document's file.
  A document that has no file yet therefore gets every record that another document
  fetched. The central cache receives only the records that `flm` fetched itself: the
  records of a document's file never enter it, since a document may come from someone
  else.

The fields `abstract` and `reference` of a record (doi.org's list of every work the
article cites) are left out of what is kept: the setting `dropped_fields`. A document
that removes one of them from the setting keeps that field in both places.

To have a record fetched again before its lifetime ends, remove it from both places:
delete its line from `.flm-citations.jsonl` (or delete the file), and remove the central
cache with `flm cache clean flm.citations-standard.autocitefetch.items`, which removes the
records of every document. The next build fetches each record that neither place holds.

*Fetching.* The arXiv is asked for up to a hundred keys in one request, doi.org for one
key per request; the program waits about three seconds between two requests to the
arXiv and one second between two requests to doi.org, as they ask, and tries a request
again when the server does not answer. Every request gives the program's name, the
setting `user_agent` (`flm/<version> (+<address>)` by default), where an address to
write to is welcome. While it fetches, the progress line counts the keys to fetch
(`Fetching citation data 3/12`; see [Progress of a build](#progress-of-a-build)), and under `-v` the program prints on standard error
each request and each wait.

*Building without a network.* A record that is due to be fetched again, and whose server
cannot be reached, is taken from the cache file or the central cache, however old it is,
so a document whose records are kept builds without the network. When the record is past
its lifetime, the citation key gets a warning at its first citation that gives the
failure (`Failed to refresh the expired record of ‘doi:10.1038/ncomms8669’: …`); a record
only due for a refresh gets none. A key whose record is kept in neither place has no
citation entry, which is an error at the citation that says why; the keys of bibliography
files and `manual` keys still have theirs. Each request to an unreachable server is tried
again for about 15 seconds, and the program waits between two requests to one server as
it always does, so a build without the network takes that time for each record due;
`-v` shows each wait.

*The citation entries.* A record that carries no ready-made citation entry is set as one
in a *citation style*, a file of the Citation Style Language (CSL) that says in which
order and in which type the authors, the journal, the volume and the year are written;
the formatter is the Rust crate `hayagriva`. By default the citation style is the one
the program ships, "American Physical Society - et al." as patched for FLM: `A. R.
Calderbank and P. W. Shor, Physical Review A **54**, 1098 (1996).`, with the volume in
bold, the year last, in parentheses, and `et al.` after the first author when there are
more than six. The setting `style` chooses another: `{file: my-style.csl}`, a style file,
whose path is relative to the place where it is written, as for `bibliography`, and which
is read again at every build; or
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
implementation of `flm`. A document whose packages lack one of these features (an
`initial_use` without `std`, for example) gets none of its values. A program that embeds the
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

## Progress of a build

While `flm render` or `flm check` builds a document, the program shows how far the
build has gone in one line on standard error, the *progress line*, redrawn in place:

```text
[########------------] 42% Converting graphics figure-3.pdf
```

The line begins with a bar of 20 cells that fills as the build goes on. Then comes the
percentage of the build done, a whole number. Then comes the step under way: its name
(`Converting graphics`), the item it works on when it has one (a file, here the image as
written in the document), and its count `done/total` when it counts its work
(`Fetching citation data 3/12`). The step shown is the most detailed one under way that
has a name for a person; the steps of the build that have none show the step that
contains them, such as `Processing document`.

- The line is shown only when standard error is a terminal and the program writes text
  (not under [`--json-messages`](#messages-as-json-records---json-messages)). No option
  changes this.
- It is first drawn once the build has run for half a second, so a short build shows no
  line, and then redrawn at most ten times a second.
- It is at most 80 columns wide, whatever the width of the terminal: the text is cut to
  fit. On a terminal narrower than 80 columns the line wraps, and it is not redrawn
  cleanly.
- A line that the program prints while the line is drawn (a problem, a line of `-v`) is
  printed above it.
- The line is removed when the build ends, before the problems are printed. It is also
  removed before `flm render` writes the document on standard output (no `-o`), since
  both may be the same terminal.

The percentage is an estimate. The program weighs each step of the build by a table of
expected durations shipped with it, measured on sample documents: the percentage gives
the parts of a build their proportions, not the time left. It never decreases during a
build, and it reaches 100 only when the build has ended. A step that the table does not
know, such as a stage of an extension, gets the median weight of the steps beside it.

The preview (`flm watch`) shows no progress line on the terminal: its tools page shows the
same text in its status bar (see [The tools page](#the-tools-page)).

### Log lines and `-v`

A *log line* tells what the build does, for a person: a line that an extension writes on
its console, each request sent to fetch the details of cited works. The program prints
each log line on standard error, after the name of who wrote it: for an extension, its
name; for a step of the build, the name of that step's kind, such as
`flm.citations.fetch`.

```text
flm.citations.fetch: Fetching https://export.arxiv.org/api/query?id_list=1706.03762&max_results=1
flm.citations.fetch: Waiting 0.5 s before trying https://export.arxiv.org/api/query?id_list=1706.03762&max_results=1 again (attempt 2)
```

A log line has a level: `error` and `warn` lines, printed as `NAME: error: TEXT` and
`NAME: warning: TEXT`, are always printed; `info` lines are printed under `-v` only.
`console.error` and `console.warn` in an extension write lines of the first two levels,
and `console.log`, `console.info` and `console.debug` lines of the level `info`. Under
`-v`, the program also prints, once each step is over, the files it read, the profile,
the packages and the workflow it uses, and the files it wrote.

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

### Messages as JSON records: `--json-messages`

The option `--json-messages` makes the program write on standard error one JSON object
per line, a *record*, in place of text for a person, so that an editor or another program
can read every message of the program: the problems, the log lines, and the progress of a
build. It is accepted by every command but `flm dump`, before or after the
command's name (`flm --json-messages check paper.flm` and `flm check --json-messages
paper.flm` are the same). Standard output and the exit status do not change.

```text
$ flm check --json-messages paper.flm
{"kind":"messages","flm":"0.6.0","format":1}
{"kind":"diagnostic","severity":"error","id":"core.specs.unresolvable-command","data":{"name":"nosuchmacro","escape_char":"\\","detail":null},"message":"cannot resolve command ‘\\nosuchmacro’","place":{"file":"paper.flm","offset":47,"line":4,"column":27,"end":{"offset":59,"line":4,"column":39}},"frames":[{"place":{"file":"paper.flm","offset":47,"line":4,"column":27,"end":{"offset":59,"line":4,"column":39}},"title":"unknown macro ‘\\nosuchmacro’"}]}
{"kind":"diagnostic","severity":"warning","id":"flm.refs.unresolved","data":{"label":"sec:missing"},"message":"Label ‘sec:missing’ is undefined","place":{"file":"paper.flm","offset":25,"line":4,"column":5,"end":{"offset":42,"line":4,"column":22}},"frames":[]}
{"kind":"summary","errors":1,"warnings":1}
```

The first line is a header: its `kind` is `messages`, `flm` is the program's version, and
`format` is the version of the records' form, a whole number. Each following record has a
`kind`:

| `kind` | what it reports | its other keys |
|---|---|---|
| `diagnostic` | a problem in the document | `severity` (`error`, `warning` or `note`), `id` (the identifier of the problem's condition) and `data` (its details), `message` (for a person; its wording may change), `place`, `frames` (the traceback: each an `inside` line of the text form, with its `place` and `title`), and `made_from` (the place in content the program made, when the problem is there) |
| `problem` | a problem that the program reports outside the document's diagnostics: a problem that stopped the command before the document was processed (the severity `error`, with a `place` where the problem has one); an error of the program or of its arguments, such as a path given to `flm info` that matches no page (`error`, no place); or a warning of the program that does not stop the command, such as the warning of `flm info` about a documentation text that it prints as plain text (`warning`, no place; see [Documentation texts that are not valid FLM](#documentation-texts-that-are-not-valid-flm)) | `severity` (`error` or `warning`), `message`, `place` |
| `summary` | the counts that end a report | `errors`, `warnings` (notes are not counted) |
| `preview-link` | the link at which the live preview (`flm watch`) is viewed in a browser, the address of its tools page, written once when the preview starts | `url` |
| `build` | the start of a build of the live preview (`flm watch`). A build that ends is followed by its problems and by a `log` record `build N done`. A cancelled build has neither: a `build` record followed by the next `build` record with no `build N done` between them means that the first build was cancelled, for a change of a file it read, a request to build again (*Build again*) or a change of the build type (see [Watching a document](#watching-a-document-the-live-preview)) | `number` (from 1), `time` (milliseconds since 1970-01-01 00:00 UTC) |
| `log` | a line that tells what the program does: the files `-v` lists, a [log line](#log-lines-and--v) of the build or of an extension | `level` (`error`, `warn` or `info`), `source` (a short name of who writes the line: `flm` for the program itself, an extension's name, or the kind of a step of the build), `text` |
| `progress` | how far the build under way has gone, every half second while a build runs, when it differs from the last `progress` record (see [Progress of a build](#progress-of-a-build)). `flm render` and `flm check` write the first one half a second into the build; the preview writes them for each of its builds, from the start of the build, although its terminal shows no progress line | `fraction` (the estimate of the part done, from 0 to 1), `text` (the text of the progress line, without its bar), `row` (the step shown, when one is: `kind`, and when it has them `title`, `label` and `count`, the count being `unit` (`items` or `bytes`), `total` and `done`) |

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
read, everything `flm dump` prints, and the message for a compiled package that cannot be
made, which the program prints before it reads its arguments.

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

## The cache

The commands that build a document, `flm render`, `flm check` and `flm watch`, keep the
results of costly work in the program's *cache*, a folder shared by all your documents,
so that the next command does not do that work again. A result is kept under a key that
holds everything the result depends on, so a result found in the cache is the result that
the work would give again: a build gives the same output and the same problems with an
empty cache, a full one, or none. Removing the cache costs time only, except for the
records of cited works and the downloaded files while their servers cannot be reached
(see [Building without a network](#citations)).

### Where the cache is

The folder of the cache is the first of these that is given:

1. the option `--cache-dir DIR`;
2. the environment variable `FLM_CACHE_DIR`;
3. the folder `flm` in the folder of the environment variable `XDG_CACHE_HOME`, on every
   system, when the variable holds an absolute path;
4. the folder `flm` in the system's folder of a user's caches: `~/.cache/flm` on Linux and
   other Unix systems, `~/Library/Caches/flm` on macOS, and `flm\cache` in the folder of
   local application data on Windows (`%LOCALAPPDATA%\flm\cache`).

An environment variable set to the empty text counts as not set. The program creates the
folder when it first opens a cache in it, and writes there the file `CACHEDIR.TAG`, by which
backup programs that follow the Cache Directory Tagging Specification leave it out. The
folder `.flm` of the home folder (the viewer settings, the user's extensions) holds no
cache.

**Keep the cache on a local disk.** The citation records are kept in an SQLite database,
which a network file system without file locks can damage. When your folder of caches is
on such a file system, set `FLM_CACHE_DIR` to a folder on a local disk.

### What is kept

The cache holds several *caches*, each under a name that begins with `flm.`:

| cache | what it holds | kept |
|---|---|---|
| `flm.graphics-files.magick.converted` | the images that ImageMagick converted (see [Images](#images)), shared by all documents | 90 days after the last use |
| `flm.citations-standard.autocitefetch.items` | the records of cited works fetched from the arXiv and doi.org for all documents, the *central cache* (see [Citations](#citations)) | until maintenance must make room; a record is fetched again after its lifetime (10 days for the arXiv's, 360 days for doi.org's), and used past it only when its server cannot be reached |
| `flm.ext.math-mathjax.typeset.mathml` | the MathML of each formula that MathJax typeset in the mode `mathml` (see [Mathematics processed by MathJax](#mathematics-processed-by-mathjax)), shared by all documents | 90 days after the last use |
| `flm.host.download.files` | the files downloaded from an `https` address by a compose template ([Compose templates](#compose-templates)) or by an extension through the host service `download` ([Extensions](#extensions)), shared by all documents | seven days before the file is downloaded again; a copy is used past its seven days when a download fails |
| `flm.ext.profile-pdf-latex.latexmk.aux` | LaTeX's auxiliary files of each document built with `-p pdf-latex` (see [PDF through LaTeX](#pdf-through-latex)), one folder per document | 30 days after the last use |

An extension may declare caches of its own, named `flm.ext.EXTENSION.NAME`; its
documentation says what it keeps. A cache kept per document identifies the document by
the absolute path of its file, so a document moved or renamed starts with empty caches
of its own, and a document given as text has none. The cache file of the citation
records, `.flm-citations.jsonl`, stays beside the document; the central cache fills it
(see [Citations](#citations)).

### The options

`flm render`, `flm check` and `flm watch` take three options of the cache:

- `--cache-dir DIR` gives the folder of the cache (see [Where the cache is](#where-the-cache-is)).
- `--no-cache` reads and writes no cache on disk. The results are then kept in memory
  for the time the program runs, so one `flm watch` still converts an image once, but a
  folder kept per document, such as LaTeX's auxiliary files, is empty at every build.
- `--cache-max-size SIZE` sets the size that the cache should not exceed: a whole number
  of bytes, or a whole number followed by `K`, `M` or `G` for KiB, MiB or GiB
  (`--cache-max-size 500M`). Without it, the limit is 2% of the drive that holds the
  cache, between 256 MiB and 2 GiB.

`--no-cache` may not be given with `--cache-dir` or `--cache-max-size`. When none of the
four sources gives a folder, the program keeps the cache in memory, as under
`--no-cache`; so it does, after a warning, when the folder's path is not valid Unicode
text.

After its build, `flm render` and `flm check` carry out the cache's *maintenance*, and
`flm watch` carries it out once, while it serves its first build. Maintenance does its
work at most once a day, for every program that uses the folder: it removes the images,
formulas and LaTeX folders past the time the table above gives, and, when the cache is
larger than its limit, the data that is largest, least recently used and quickest to
produce again. A citation record is removed only in this second case.

A problem of the cache never fails a command and never changes its exit status: a result
that cannot be read or stored is produced as on a first run. The program reports such a
problem as a warning on standard error, under the source `cache`
(`cache: warning: TEXT`), and, under `-v`, the cache's informational lines (see [Log
lines and `-v`](#log-lines-and--v)).

### Listing and removing the cache: `flm cache`

`flm cache list` prints the folder of the cache, then one line for each *instance* of a
cache: the data of the cache for one document (for a cache kept per document), for one
extension, or for one set of settings. Each line gives the cache's name, its version, the
instance's key as JSON text (which identifies the document or the extension), the
instance's size, its last use (in UTC), and `damaged` for an instance whose data cannot
be read. The instances are listed in the order of the caches' names, then of their keys.

```text
$ flm cache list
Cache folder: /home/alice/.cache/flm
flm.ext.profile-pdf-latex.latexmk.aux  1  {"document":"/home/alice/paper.flm","extension":"bundled:rust/profile-pdf-latex"}  2.4 MiB   2026-11-24 13:05
flm.graphics-files.magick.converted    1  null                                                                              15.0 MiB  2026-11-24 13:05
```

`flm cache clean NAME…` removes every instance of each cache named, as `flm cache list`
prints the names, and `flm cache clean --all` removes every instance of every cache. The
command then prints the number of instances removed and their total size (`Removed 3
instances (17.4 MiB)`). An instance that a running program is using, such as the folder
of a document that `flm watch` is building, is left in place, with a warning. Both
commands take `--cache-dir DIR`, and otherwise find the folder as `flm render` does.

`flm cache` exits with status 0 when it did what was asked, also when it left an
instance in use in place; 1 when the cache cannot be read, an instance cannot be
removed, or another program is cleaning the cache or carrying out its maintenance; 2
when no folder of the cache is given or found, the folder's path is not valid Unicode
text, or a name given to `flm cache clean` is not a valid cache name; and 3 when standard
output cannot be written. `flm cache clean` checks every name before it removes anything,
so an invalid name leaves every cache as it was.

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
  use: [phfpack]
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
| `math` | the math processor, the package that processes the document's mathematics: a processor's name, `NAME`, which uses the package `math-NAME` (`mathjax`, `techxt`, `passthrough`), or `false` for no processor; without it, the program's default processor, which is not guaranteed to stay the same; `--math` takes precedence over it (see [The packages a document uses](#the-packages-a-document-uses)) |
| `features` | a map from a feature's name to its settings; the key `enabled: false` among them switches the feature off, and `false` in place of the settings is its shorthand |
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

**Switching features off.** Any feature may be switched off, the program's own included,
with the key `enabled` of its entry:

```yaml
flm:
  features:
    citations:
      enabled: false
      mark: superscript
```

`enabled: false` switches the feature off; `enabled: true` switches it on, as an entry
without the key does. The other settings of the entry stay as written, and they are not in
effect while the feature is off: switching it back on is the change of one key. `NAME:
false` is the shorthand of `NAME: {enabled: false}` and means the same. No feature has a
setting named `enabled`, and a workflow's settings cannot write it (nor `false`).

The feature `write` of the package `std` hands the output's files to standard output, a
file or a folder. With `write: false`, `flm check` reads the document as usual, and `flm
render` stops with status 2 and an error at the place where `write: false` is written,
since it has an output to write. A document chooses the package that processes its
mathematics with `math` ([The packages a document uses](#the-packages-a-document-uses));
switching that package's feature off (`math-mathjax: false`) leaves the document with no
math processor, and every workflow then stops with an error
([Mathematics processed by MathJax](#mathematics-processed-by-mathjax)).

Settings written for a feature that no package the document uses has are accepted and
have no effect: a front matter keeps the settings of `math-mathjax` while the document
tries another math processor (`math: techxt`). Since the name may be misspelled, each
source that writes such a feature (the front matter, a package, the command line) gives
one warning, at the place where it writes the feature: `Unknown feature ‘mathjx’: no
package in use has this feature`. The warning is printed also when an error stops the
work, before that error.

**Metadata.** `title` and `date` are each one text; `author` is one author or a list of
authors. An author is a text, the author's name, or a map:

```yaml
author:
  - name: Ada Lovelace
    affiliation: [University of X, Institute of Y]
    email: ada@example.com
    note: Corresponding author
  - Grace Hopper
```

In an author's map, `name` is required; `affiliation` and `note` are each one text or a
list of texts; `email` is the address; any other key is kept, and the output does not show
it. A number is read as the text it is written as (`date: 2026`). Each text but the email
address is FLM, not plain text (`title: The \emph{Art} of \(x^2\)`), and a problem in it
is reported at its line in the file; the email address is a plain text. The output shows
the title, the authors and the date before the content: each author on lines of its own,
the name, then each affiliation, the email address as a link, and each note. Every other
root key is kept, and the output does not show it. In a YAML text that is not quoted or is
quoted with `'`, a backslash is written as it is; in a text quoted with `"`, it must be
doubled (`"\\emph{Art}"`). A title, date or author that is not of this form, and an
author's map without `name`, stop the command with status 2, at their place in the file.

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
  own extensions of this kind (`flmdoc`, `math-techxt`, `phfpack`, `profile-pdf-latex`,
  see [PDF through LaTeX](#pdf-through-latex), `profile-tty`, see [Text for a
  terminal](#text-for-a-terminal), and `syntax-highlight`) are found by their names, as
  the other extensions shipped with FLM are; the same source also builds as a WebAssembly
  module, which the engine `wasm` runs.

A workflow that an extension declares runs as the profile of the same name, as the
program's own workflows do (`flm render -p typst paper.flm`), and `-f` selects one of its
outputs, an output that an extension declares included. An extension named `profile-NAME`
is used by the profile `NAME` alone, without being listed under `use` (see [Profiles and
workflows](#profiles-and-workflows)).

**The two layouts.** An extension named `NAME` is either the file `NAME.yaml`, or the
folder `NAME/` holding the file `flmextension.yaml` and the extension's other files: its
compose templates under `templates/`, the modules of an extension written in
JavaScript, and the module of an extension compiled to WebAssembly. The file `NAME.yaml`, or `NAME/flmextension.yaml`, is the extension's
*manifest*. An extension written as one file has no other files: a bare path in its
manifest, such as the path of a bibliography, names no file. Such a manifest names a file
of the document's folder with `doc:PATH`; an extension that comes with files of its own
is written as a folder.

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
| `extension.flm_version` | the versions of FLM the extension works with, as a version requirement (`>=0.5`, `^0.5.0`); a program whose version does not meet it refuses the extension. The requirement is read in version order, where a pre-release such as `0.6.0-beta.2` comes after every `0.5` version and before `0.6.0`; a version written without its patch number, such as `0.6`, stands for its whole series, the series' pre-releases included |
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
  that holds `.` or `..` as a segment, is refused. To use an extension stored next
  to a document, put it in the folder `_flmextensions` beside the document, or in another
  folder of the search path;
- an entry that holds `:` is refused: the character is reserved.

The same entries are written in a front matter, under `flm.use` in a manifest, and in
the package lists of an extension written in JavaScript.

**Compose templates.** An extension written as a folder may ship compose templates. The
engine `data` finds their descriptions in the folder `templates/`, which holds a folder
`templates/OUTPUT/` for the compose templates of each output (`templates/html/`). Each
compose template there is written in one of two forms:

- the folder `FOLDER/`, holding the compose template's *description* `flmtemplate.yaml`
  and the compose template's own files: its text, the files it ships, the files it
  includes;
- the file `FILE.yaml`, the description alone, for a compose template that has no text
  (such as one of the template engine `content-only`, which writes the output's parts
  with nothing around them).

A compose template that has a text is best written as a folder, so that its files stay
apart from those of the extension's other compose templates. The description gives the
compose template's name and the output it serves, with its keys `name` and `output`;
`-t NAME` selects a compose template by that name. The names of the folders and files
give neither. A file of `templates/` itself (a `README`), and in `templates/OUTPUT/` a
folder that holds no `flmtemplate.yaml` and a file whose name does not end in `.yaml`, are
not compose templates and are left out. A name beginning with `.` is left out in
`templates/` and in each `templates/OUTPUT/` alike.

The description is YAML: `name`, `output`, `engine`, `accepts`, and optionally `main`,
`requires`, `settings`, `delimiters` and `artifacts`. The key `main` gives the path of the
file that holds the compose template's text; without `main`, the text is empty. The key
`settings` declares the compose template's own settings, which the compose template reads
as `config`: a JSON Schema of an object, each of whose `properties` is one setting, with
its `type`, its `description`, its `default` (which every setting gives) and its marks
(`x-flm-marks`, see [The marks of a setting](#the-marks-of-a-setting)). A default of a
setting marked `path` that is a bare path is relative to the folder of the description. A
key written under `compose.template_config.OUTPUT.NAME` that the compose template does not
declare is left out, with a warning. Every
path a description gives, `main` and the paths under `artifacts`, is relative to the
folder of the description, which in the folder form is the compose template's own
folder; so is the path of a file the compose template reads from its own files, with the
statement `include` or with a function given a path with no root (see [Files a compose
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
name: fancy
output: html
engine: minijinja
main: fancy.html
accepts: all
settings:
  type: object
  properties:
    color: {type: string, default: teal, description: The color of the paragraphs.}
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
network and no program by itself. A package of the extension declares, under the key
`host` of its package object, the *host services* its handlers call, and the program
offers four:

- `files` reads the text of a file given by a path with its root (see [Paths with a
  root](#paths-with-a-root)): `self:PATH` for a file of the extension's own folder,
  `ext+NAME:PATH` for a file of another extension that the document uses, `std:PATH` for
  the program's own files, `doc:PATH` for a file of the document's folder (the folder of
  the input file; the current folder for a document given with `-c` or on standard
  input), or `cwd:PATH` for a file of the folder in which the program runs. A path of an
  extension's folder that leaves the folder is refused, and so is a file that a symbolic
  link leads outside it. This service reaches no network;
- `download` downloads the file at an address `https://…` and keeps it in the program's
  cache for seven days ([The cache](#the-cache)). It answers the file's content, the media
  type that the server states, and a warning when the file is a kept copy that could not
  be downloaded again;
- `cache` reads and stores the values of the caches that the extension's package
  declares, in the program's cache ([The cache](#the-cache)). This service reaches no
  other file and no network;
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
received from someone else runs the extensions in its folder. Building a document can
also contact any server that the document or its packages name: an address in a compose
template's call of `include_raw` or `include_artifact_url`, in a setting, in a
bibliography, or given to the service `download`.

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
   variable `HOME` gives, or `USERPROFILE` where `HOME` is not set or empty;
4. last, the extensions compiled into the program: the extensions shipped with FLM,
   `lecturenotes` ([The lecture-notes template](#the-lecture-notes-template)),
   `letter` ([The letter template](#the-letter-template)),
   `math-mathjax` ([Mathematics processed by MathJax](#mathematics-processed-by-mathjax)),
   `profile-pdf-latex` ([PDF through LaTeX](#pdf-through-latex)), `profile-tty` ([Text
   for a terminal](#text-for-a-terminal)), `syntax-highlight`
   (the highlighting of code listings in HTML and in the rendered LaTeX), `flmdoc` (the
   markup of documentation pages; see
   [Writing documentation pages: the extension `flmdoc`](crate::guide::extending#writing-documentation-pages-the-extension-flmdoc)),
   `phfpack` (notation and quantity macros for quantum information in formulas; see
   [A Rust extension, end to end](crate::guide::extending#a-rust-extension-end-to-end))
   and `math-techxt` (the math processor `techxt`, which renders formulas as Unicode text;
   see [Mathematics rendered as Unicode text](#mathematics-rendered-as-unicode-text)),
   found by their names (a locator of one segment). A copy of one of them in a folder of
   the search path is found first and is used in its place; for the six written in Rust
   (`profile-pdf-latex`, `profile-tty`, `syntax-highlight`, `flmdoc`, `phfpack`,
   `math-techxt`), the copy is their WebAssembly form.

A locator of several segments is joined to each folder: `@acme/style` is
`@acme/style.yaml` or `@acme/style/flmextension.yaml` inside the folder. An extension is
identified by the path of its manifest, so an extension that two locators reach (`style`
in one folder of the search path, `shared/style` in its parent folder) is read once, and
two packages that use each other cause no problem. An extension compiled into the program
is identified as `bundled:FOLDER/NAME`, where `FOLDER` is the folder of the repository's
`extensions/` that holds it: `data`, `js`, or `rust` for one written in Rust
(`bundled:js/math-mathjax`, `bundled:rust/profile-pdf-latex`). A symbolic link inside an extension's
folder must not lead to a file or a folder outside that folder; the extension's folder
itself may be a link. This check, like the refusal of a path that leaves the extension's
folder, is against a broken or misconfigured extension, not a protection. A locator found in no folder and not compiled into the program, a folder that holds an extension in
both layouts, a folder given with `--extensions-dir` that does not exist, and a problem
in an extension's manifest or files stop the command with status 2, at the place of the
problem.

### Ranks of the settings

A setting may be written in several places, called *settings sources*; the source of
the highest rank wins and replaces a lower source's value whole, unless a mark of the
setting's declaration says otherwise (see [The marks of a setting](#the-marks-of-a-setting)).
From the lowest rank to the highest:

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
writes an entry for the feature decides, the entry switching it off with `enabled: false`
(or `false`) and on otherwise. A front matter that writes `theorems: {}` or `theorems:
{enabled: true}` therefore switches on a feature that a package switched off, and the
settings that package wrote beside `enabled: false` are then in effect, unless a higher
source writes them; a package switches back on (or off) a feature that a package it uses
switched off (or on); and two peers of which one switches the feature off and the other
on stop the command. A key that no
feature declares, or a value that cannot be read, stops the command with status 2, at
its place in the file.

No file is read that the document does not list: the front matter, the extensions it
uses and `-O` hold every setting.

### The marks of a setting

The declaration of a setting may attach *marks* to the setting, or to a value at any depth
inside it. A mark tells how the settings sources combine the value, or what the value
means. There are five marks:

- `merge-object-key-values`, on a map: the sources that write the map are combined key by
  key. Each key of the map is decided by the rule of [Ranks of the
  settings](#ranks-of-the-settings): a source adds a key and leaves the others, and the
  source of the highest rank that writes a key gives its value whole. The declared default
  of the map is the lowest source, so a source that writes one key keeps the other
  default keys. A key whose own declaration carries the mark is combined key by key in
  turn.
- `remove-key-when-value-is-false`, on a map: a key whose combined value is `false` is
  removed from the map, a key of the default included. A `false` written elsewhere is the
  value `false`.
- `set-to-default-with-null`, on one setting: a combined value of `null` gives the setting
  its default.
- `path`, on a text or on a value that holds texts: each text is the path of a file. A
  bare path is relative to the place where it is written (see [Paths with a
  root](#paths-with-a-root)).
- `markup`, on a text or on a value that holds texts: each text is written in FLM markup.

`flm schema` prints each mark on the node of the schema that it concerns, under
`x-flm-marks` (see [Printing the schema of the settings](#printing-the-schema-of-the-settings)),
and the Settings tab of the live preview shows the marks of each row (see [Editing the
front matter from the page](#editing-the-front-matter-from-the-page)).

### Collections of named entries

Some settings are collections of named entries: the kinds of notes, the types of floats,
the kinds of theorems. Each is a map from a name to an entry that carries the marks
`merge-object-key-values` and `remove-key-when-value-is-false`, so the sources that write
one are combined entry by entry: a source adds an entry and leaves the
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
| `graphics` | `strategy`, `convert`, `magnification` |
| `citations-standard` | `sources` |
| `write` | `output_names` |

The settings of the compose templates, `compose.template_config`, are combined key by key
too, at each of the levels `template_config`, `OUTPUT` and `NAME` (see [Compose
templates](#compose-templates)).

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

Every other setting is replaced whole, a list included. A value that is not a map where
the mark `merge-object-key-values` stands stops the command with status 2, at its place in
the file.

### Paths

A path written in a document (the file of an `\input`, an image, a file that a setting
names, such as a bibliography) separates its names with `/`, on every system, Windows
included: `\input{parts/intro.flm}`. Every other character belongs to a name, the
backslash `\` among them: `\input{my\folder\file.flm}` names the one file
`my\folder\file.flm`. Such a file cannot exist on Windows, where a name may not hold `\`
or `:`, and the message says so.

A path is relative unless it begins with `/`, is `~`, or begins with `~/`:

- A relative path is relative to the folder of the file in which it is written:
  `figures/plot.png` in `docs/paper.flm` is `docs/figures/plot.png`. In a text given with
  `-c` or on standard input, it is relative to the current folder.
- A path that begins with `/` is absolute. On Linux and macOS it is the system's path as
  is: `/home/me/figures/plot.png`. On Windows it begins with its drive,
  `/C:/figures/plot.png`, or is a network path, `//server/share/figures/plot.png`.
- `~` is the home folder, and a path that begins with `~/` is in the home folder:
  `~/figures/plot.png`. The home folder is the value of the environment variable `HOME`,
  or of `USERPROFILE` where `HOME` is not set or empty. A folder named `~` is written
  `./~`: `./~/plot.png`.

A relative path is joined to the folder as text, and only its `.` names are removed:
`docs/./figures/plot.png` is `docs/figures/plot.png`. A `..` is kept as written:
`../common.flm` in `docs/parts/one.flm` is `docs/parts/../common.flm`, and the system
resolves the `..` when it reads the file (on Linux and macOS, after following symbolic
links). Messages, and the files that `-v` lists as read, give each file by this path, the
same on every system, and a file name printed can be opened from where the command ran.

A path given on the command line or in the environment (the document's file, `-o`,
`--extensions-dir`, `FLMPATH`, `MAGICK`) is written as the system writes it,
`C:\docs\paper.flm` on Windows; the program converts it. Such a path must be valid
Unicode text.

### Paths with a root

A compose template, the code of an extension and a setting marked `path` (see [The marks
of a setting](#the-marks-of-a-setting)) give the path of a file in one syntax, which adds
a *root* to the paths of a document. The root is the word before the first `:` of the
path, and it names the folder in which the path is read: `ext+mytemplate:style.css` is
the file `style.css` of the extension `mytemplate`. A path is read by three rules, in this
order:

1. A path that begins with `/`, is `~`, or begins with `~/` is an absolute path, as in a
   document ([Paths](#paths)), read with the root `doc`: `/home/me/logo.png` is
   `doc:/home/me/logo.png`, a file of the machine. In a file of a package too, an absolute
   path names a file of the machine, never a file of the package: a file of the package
   is written `self:partials/head.html`.
2. Any other path that holds `:` has a root: the text before the first `:`. A use name may
   hold `/`, so the name of `ext+@acme/style:img/logo.svg` is `@acme/style`.
3. Any other path is a *bare path*, which holds no `:`. It is relative to the folder of
   the file in which it is written.

The roots:

| root | the folder | the path after the root |
|---|---|---|
| `self` | the files of the package in which the path is written: the folder of its extension, or the program's own files for a compose template of the program | relative, inside the folder |
| `ext+NAME` | the folder of the extension that the `use` name `NAME` finds (the text of a `use` entry without its `#PACKAGE` part), for any extension that the document uses, directly or through another package | relative, inside the folder |
| `std` | the program's own files, such as `std:templates/html/simple/simple.html` | relative, inside the folder |
| `doc` | the document's folder: the folder of the input file, or the current folder for a text given with `-c` or on standard input | as a path written in the document: relative, absolute, `~`, or climbing with `..` |
| `cwd` | the folder in which the program runs | as a path given on the command line |
| `https` | an address on the network, `https://example.org/fonts.css` | the address |

Under `self`, `ext+NAME` and `std`, a `..` may climb inside the folder, never above it.
A `:` always ends a root: a file of the document's folder whose name holds `:` is written
`doc:a:b.css`. A path written with a drive letter, such as `C:\Users\me\logo.png`, is
refused with the form to write on Windows, the absolute path `/C:/Users/me/logo.png`.

**The place of a text.** A bare path is relative to the file in which it is written,
called its *place*:

- in a file of a compose template, the file itself: `partials/head.html` written in
  `ext+notes:templates/html/notes/notes.html` is
  `ext+notes:templates/html/notes/partials/head.html`. A compose template that extends a
  text of another package reads the bare paths of that text in the other package's folder;
- in the code of an extension, no place: a bare path given to a host service is refused,
  since the program cannot tell in which file of the extension the call is written. A
  script gives the path of a file beside its module with the helper `path`
  (`path(import.meta, "./head.html")`), which returns its path under `self:` (see [Host
  services](crate::guide::extending#host-services) in the guide to extending FLM);
- in a setting marked `path`, the file in which the setting is written. A bare path in
  the front matter is a file of the document's folder (`doc:`); in the manifest of an
  extension `NAME`, a file of that extension's folder (`ext+NAME:`); on the command line
  (`-O`), a file of the current folder (`cwd:`); in the program's defaults, a file of the
  program (`std:`). An extension written as one file has no folder of files, so a bare
  path in its manifest names no file.

A text that no file holds has no place: a bare path or `self:` inside it is refused. This
is the case of a text given to the function `eval` of a compose template (see [Compose
templates](#compose-templates)), whose paths therefore state their root. `self:` is also
refused in the front matter and on the command line, which belong to no package.

### Documents of several files

A document brings other files in with `\input{file}`
([the language](crate::guide::language_syntax#including-files)). The file name is
relative to the folder of the file that holds the command, as every path of a document
is ([Paths](#paths)): `\input{parts/intro.flm}` in `docs/paper.flm` reads
`docs/parts/intro.flm`, and `..` goes up one folder. Every message gives a file by that
path: the primary file as it was typed, and an included file as the folder of the file
that includes it joined with the name written there. Two spellings of one file's path,
such as `docs/common.flm` and `docs/parts/../common.flm`, are two names: a file that
includes itself under another spelling is stopped by the limit on the depth of
inclusions. `flm check` reads the included files as `flm render` does.

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
(`std`, `math-passthrough` and the packages compiled into the program) and of the package of the program's default math processor (`math-mathjax`, an
extension shipped with the program, when the program includes it).
It depends on no document, so one schema serves every file of its kind, and every run
with the same extensions installed prints the same bytes. With a document (`flm schema --of front-matter paper.flm`), the
schema covers the features of the packages that document uses: those of its initial list,
the package of its math processor and those its front matter lists under `use`, compiled
packages and packages of
extensions alike, with the packages they use. The extensions are found as `flm render`
finds them, and the features that an extension written in JavaScript declares are
covered too. A document that cannot be read, or whose packages cannot be found, stops the
command with status 2. Under `features`, the schema also accepts the name of a feature
that it does not cover, with settings of any key, as the program does (see [The front
matter](#the-front-matter)).

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
  `math-NAME`: `passthrough`, `mathjax` and `techxt` when the program includes them, and
  `NAME` for each extension `math-NAME` installed.

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
set on the document and that a render may set again. Under `x-flm-marks`, a node of the
schema lists the marks of the value it describes (see [The marks of a
setting](#the-marks-of-a-setting)); the default of a map combined key by key is its default
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

`flm watch INPUT`, or `flm INPUT` with no command, is the *live preview*. It builds the
document into an HTML page, serves the page to a browser on this machine, opens the
default browser on it, and keeps running. Whenever a file that the document read changes
(the document itself, a file brought in with `\input`, an extension's manifest or one of
its files, an image, a bibliography file, a sibling file of the document that an extension
read while building it), it builds the document again, and the page in the browser
follows. A change made while a build runs cancels that build, which then shows nothing,
and the next build starts with every change made so far: a build that takes longer than
the time between two saves is cancelled each time. Choosing another build type or
*Build again* on the tools page cancels the build under way too. Ctrl+C ends it, with
status 0.

```text
$ flm paper.flm
╭────────────────────────────────────────────────────────────────────────╮
│                                                                        │
│                            FLM live preview                            │
│                                                                        │
│             ==>  http://127.0.0.1:18910/.flm-preview/  <==             │
│                                                                        │
│   Preview your document by opening this link in your browser;          │
│   hit Ctrl+C here to quit.                                             │
│                                                                        │
╰────────────────────────────────────────────────────────────────────────╯
flm: build 1 done
flm: changed paper.flm
flm: build 2 done
```

The box gives the link at which the preview is viewed, the address of its tools page.
Under `-v`, the program also prints, after the box, the editor in which a place clicked
on the page opens and the editors it found (or that it found none, and where it looked),
and for each build what `flm render -v` prints. Under
[`--json-messages`](#messages-as-json-records---json-messages), the link is a record of
the kind `preview-link`, and no box is drawn.

The preview takes the options of [What to build](#what-to-build) (`-p`, `-f`, `-t`,
`-O`, `--layout`, `--extensions-dir`, `-v`), and these:

| option | what it does |
|---|---|
| `--port PORT` | the port of this machine at which the preview is served; 0 lets the system choose a free port (the link printed gives it). Default: 18910, or else the first free port among the 64 that follow it |
| `--no-browser` | open nothing; the link is still printed. Without this option, starting the program again while the page is open opens a second page; the open page reconnects by itself either way |
| `--editor NAME` | the editor in which a place of the document opens, for this run, until another editor is chosen on the page: `code`, `cursor`, `codium`, `zed`, `emacsclient`, `subl`, `gvim`, `mvim`, `vim`, `nvim`, or `custom` for the editor command (see [Opening the source in an editor](#opening-the-source-in-an-editor)). When that editor is not available, the usual choice applies. Another name is refused with status 2 |
| `--editor-command COMMAND` | the command of an editor of your own, for this run, over the `editor_command` of the [viewer settings](#the-viewer-settings), as in `--editor-command "myeditor --goto {file}:{line}"`. A command with a quotation mark left open, or with no word, is refused with status 2 |

The environment variable `BROWSER`, when it is set, gives the program that opens the
preview's address, in place of the default browser.

The preview keeps its output in memory and accepts no `-o`; it opens the browser by itself
and accepts no `-V`; it watches a file, so it accepts neither `-c TEXT` nor `-`. Each of
these is refused with status 2, and the message gives the `flm render` command that does
what was meant (`flm render -V -o OUTPUT INPUT` writes an output and opens it). The preview
writes a file only when you ask for it on the page: a change of the front matter (see
[Editing the front matter from the page](#editing-the-front-matter-from-the-page)), an
export saved (see [Exporting the document](#exporting-the-document)), and the viewer
settings.

### The tools page

The address that the program prints and opens, `http://127.0.0.1:PORT/.flm-preview/`, is
the *tools page*: a page of the preview's own, which shows the document's output beside
the preview's tools. The document's page itself is at `http://127.0.0.1:PORT/`, shown in a
frame of the tools page; opened alone at that address, it is a live preview without the
tools. The tools page has two parts:

- The *status bar*, across the bottom of the window. At its left, it shows the
  document's file name (the whole path as its tooltip); *Showing* and the selector of the
  build type (see [Build types](#build-types)); *Build again*, which builds the document
  now, for a change that the program did not detect (a file the document did not read, a
  save on a network drive); the state of the builds (*Building…* during a build that lasts
  more than a moment, replaced by the build's progress text, as `flm render` shows it on its
  [progress line](#progress-of-a-build) (`42% Converting graphics figure-3.pdf`), *Built in
  …* once it is done, *Last successful build* when the
  last build produced no output and the output shown is that of an earlier one,
  *Disconnected* when the program does not answer). At its right, it shows *Inspect* (see
  [Inspect](#inspect)); the counts of the last build's problems (red for errors, amber for
  warnings, gray for notes, a green check when there is none), which open the *Problems*
  tab; in a wide window, one small icon for each of the tabs *Outline*, *Help*,
  *Metadata*, *Settings* and *Export*, which opens that tab; and, last, the button that
  shows or hides the panel, on the tab it showed last. The counts and the icons open the
  panel when it is closed, show their tab, and move the focus into it.
- The *panel*, docked at the right edge or at the bottom of the window. It has seven tabs:
  *Problems*, *Outline*, *Help*, *Metadata*, *Settings*, *Export* and, at the right end
  of the tab row, *Viewer*, each described below. The buttons beside *Viewer* dock the
  panel at the other edge and close it; the bar between the panel and the output resizes
  it, with the mouse or with the arrow keys.

The browser tab shows the document's name, and its icon shows the state of the preview:
a build under way, errors, warnings, notes, no problem, or the connection lost. When the
program is started again, the tools page reconnects by itself, and loads itself again.

What the tools page shows of the output depends on the main file of the *leading output*
(the output `-f` names, else the first output of the workflow):

- an HTML page: the page, in the frame;
- a PDF file (under `-p pdf-latex`): its pages, with a line above them that gives the page
  read, the number of pages and the zoom, and the buttons that change the zoom. After each
  build, the pages are drawn again, at the same page and the same zoom;
- a file that holds a text, whatever its media type (the output of `-p text`, the LaTeX of
  `-p latex-recomposed`, the BibTeX of `-p bibtex`, for example): its text, with long
  lines wrapped unless the viewer setting `wrap_lines` is off. A file holds a text when it
  is valid UTF-8 without control characters other than tabs, line ends, form feeds and the
  escape character. LaTeX is highlighted; the colors and styles of a terminal's text (the
  output of `-p tty`) are shown, and its links show as their text alone;
- any other file: a line that gives its media type. The file itself is at
  `http://127.0.0.1:PORT/`;
- a build type that produces no output (`-p check`): a line that says so.

### What the preview serves

The server listens only on this machine's loopback address (`127.0.0.1`), and serves from
memory the files of the last build that produced them. The preview builds every output of
its workflow, as `flm render` does, and serves the main file of the leading output at `/`
(and at `/index.html`), and every other file at its path: the files written beside the
page (an image copied beside the page under `-O graphics.strategy.html=copy`, for example)
and the files of the workflow's other outputs. It serves nothing of the document from the
disk. Every answer tells the browser not to keep it. The page is the one that `flm render`
would write, with the same compose template, plus the preview's script and stylesheet,
the stylesheet of the marks of errors (whether or not the document has such a mark), and,
on each element, its place in the source (the attribute `data-flm-source-span`).

A request of the page that changes something (build again, change the build type, open a
place in the editor, change the front matter, export the document, open the exported
file, change the viewer settings) is carried out only when it comes from a page that the
preview's server served, or from a program that is not a browser; a request from a page of
another site open in the same browser is refused. So the tools work from
`http://127.0.0.1:PORT/`, but not from the same page reached as `http://localhost:PORT/`.

### Building through errors

Each build continues through the document's errors: the page shows the document as it is
now, with the marks the library writes where something could not be rendered, and the
terminal prints the build's problems as `flm render` prints them. When the document cannot
be built at all (its front matter cannot be read, a package it lists is missing), the
output shown stays as it was, the status bar says *Last successful build*, and the
*Problems* tab says why. Before any build has produced an output, the frame shows a page
that says the preview is waiting for the document; the document's page replaces it once a
build produces one. Under `-v`, the terminal also prints, at each build, the files read,
each request and each wait of a fetch of the details of cited works, and the lines that
extensions write on their console at the level `info`, and, at the first build and at
each build that reads the front matter and the packages again, the packages in use and the
workflow, as `flm render -v` does ([log lines and `-v`](#log-lines-and--v)).

### How the page follows the document

The document's page carries a script of the preview, which keeps a connection to the
program open. At each build, the script updates in place only the parts of the page that
changed (the document's content, the title, the table of contents, the list of notes, the
bibliography, each placed part on its own), and inside a changed part it keeps every
element that did not change, so that the place you are reading stays where it is and a
formula whose source did not change stays typeset. It then scrolls to the first element
that the build added or changed, unless a part of it is already in the window, and
flashes each such element once; the viewer setting `scroll_to_changes` turns this off. The
title shown in the browser's tab follows too; a compose template that writes a part's
plain text elsewhere (in a heading, in a `<meta>` tag) gets the page loaded again when
that text changes. An image written into the page changes with the part that shows it,
and a file served beside the page that changed is loaded again. The page is loaded again
when the compose template's own markup changed, when the page gained or lost a part, or
when it gained or lost a script or a stylesheet (the formula renderer at the document's
first formula, for example). When the program is started again, an open page reconnects
by itself, and loads itself again if the document changed in the meantime.

### Build types

The *build type* is the profile that the preview builds with: `html`, `text`,
`pdf-latex`, `check`, and the others that the document's packages declare. It starts as
the profile of `-p`, else the front matter's `default_profile`, else `html`. The selector
of the status bar lists the build types of the document, the default one (the build type
the preview starts with) marked with ★; a choice there builds the
document again with that profile, and the preview keeps it until another choice. The
choice is not written into the document, and it ends with the program. When the document
cannot be prepared with the build type chosen, the build stops, its problems say why, and
the output shown stays the last one.

### The problems

The counts of the status bar are those of the last build's problems. The *Problems* tab
lists them by file, one row for problems that are alike, each with its severity, its
message, and its file, line and column. A click on a row scrolls the document's page to
the elements the problem is in and flashes them; the row then shows the line of the source,
the constructs the problem is inside, and the line *Related:* with links to the help pages
of the command or environment at the problem's place and of its feature. *Open in editor*
(or Enter on a row) opens the problem's place in the editor (see [Opening the source in an
editor](#opening-the-source-in-an-editor)). The elements that an error or a warning is in
are outlined on the page, whether the tab is open or not. With the viewer setting
`open_problems_on_errors` on, each build that has an error opens the panel on *Problems*
when the panel is closed.

### Outline, Inspect and the help pages

The *Outline* tab lists the document's headings, indented by level, each followed by the
labels that stand under it: those written after the heading and before the next one (a
heading's own label first), the labels written before the first heading at the top. A
label's row shows what a reference to it shows (`Fig. I`), then the code that refers to it,
with a button that copies that code, to paste where the document refers to it:
`\term{TERM}` for the label that a term definition computes from its term or an alias,
`\ref{LABEL}` for every other label. A click on a heading, or on what a label shows,
scrolls the page to it. The search field above the list filters the headings and the
labels by their text, a label also by its code, and keeps the headings that a row found
stands under. Under the search field, a row of toggles shows or hides all the labels, or
those of one kind (`section`, `equation`, `theorem`, …, with their number); a click with
Shift on a kind shows that kind alone, and, on the kind shown alone, every kind again. The
choice holds until the tools page is loaded again.

#### Inspect

*Inspect*, in the status bar, is offered while the frame shows an HTML page. While it is
on, a click on the document selects the innermost element around the click that has a
place in the source, outlines it, and shows a card beside it. The card shows the construct
written there (a command, an environment or a special character sequence such as `~`) as
written, with its kind and its feature; the constructs it is inside (*Inside:*), each of
which can be inspected in turn; its place as `FILE:LINE:COLUMN`; *Open in editor*; *Help*,
when the construct has a help page; the button that copies the code that refers to the
element, `\ref{LABEL}` or `\term{TERM}` as in the *Outline* tab, when a label refers to
the element;
and the messages of the problems on the element. The card closes on Escape, on a click
elsewhere, when the document scrolls, changes or is resized, and when Inspect is turned
off. While Inspect is on, a click on a link of the document does not follow the link.

#### The help pages

The *Help* tab shows the pages of `flm info -d INPUT` (see [Describing the environment:
`flm info`](#describing-the-environment-flm-info)) for the environment of the document's
last successful preparation: its packages, features, commands, settings and workflows. It
has *Back* and *Forward*; *Home*, which shows the first page, the one that leads to all
the others; a search field that lists the pages whose names match (a page opened from that
list follows the search in the history, so that *Back* shows the list again); and a path
above each page whose parts lead to the pages above it. A page of a feature has the
link *Settings of this feature*, which opens the *Settings* tab at that feature. *Open this
page in a new tab* opens the *help browser*, the help pages alone at
`http://127.0.0.1:PORT/.flm-preview/help#PATH`, where `PATH` is the page's path
(`features/refs/macros/ref`, for example); the help browser follows the document's
environment as the tab does.

### Editing the front matter from the page

Two tabs change the document's [front matter](#the-front-matter) and write it into the
document's file:

- The *Metadata* tab shows the root keys other than `flm`: the title, the authors and the
  date, each in a row of its own, then every other key with a text field, or an editor of
  its YAML text for a value that is not a text, and a button that removes it. *Add a key*
  adds a root key. The authors' row shows the authors' names, each changed, removed or
  added in place; when an author is written as a map, the row opens the editor of the YAML
  text of `author`, so that no part of an author is lost.
- The *Settings* tab shows the keys under `flm`. A search field filters the rows. The
  common settings come first, by topic: the compose template; the default build type
  (`default_profile`); the math processor and `$…$` for mathematics; the numbering of the
  headings and the depth of the table of contents; the typographic substitutions of the
  feature `blocks` (curly quotes for `"…"`, the TeX quotes ``` ``…'' ```, the dashes `--`
  and `---`, the ellipsis `...`), each a checkbox; the citation keys, the bibliography
  files and the citation marks; the settings of the compose template in use; and the
  packages the document uses (`use`). The advanced list follows, with every feature of the
  document's packages (those of `flm schema --of front-matter INPUT`): a switch that turns
  the feature on or off (see [Switching features off](#the-front-matter)), and an arrow
  that unfolds its settings. The field *Filter* beside the list's heading filters the
  advanced list alone. A row shows the value that the front matter writes, else the value
  in effect, else the feature's default; its arrow *Revert to default* removes what the
  front matter writes there. The package icon of a value that a package gives lists, in its
  tooltip, the value that each source writes, the one in effect, and the marks of the
  setting (see [The marks of a setting](#the-marks-of-a-setting)); it also shows before
  the arrow of a value that the front matter writes over a package's. A value that the
  workflow or the command line sets is named so beside its control, which still changes
  the front matter: that value applies under another build type, or without the option.
  A change at a place inside a value that one source gives whole (a setting without the
  mark `merge-object-key-values`, or an entry of a map that carries it) would make the
  front matter's value replace that whole value. The program then copies into the front
  matter, in the same write, the other values in effect inside it, whether they come from
  a package, the program's defaults or the feature's own default, and the page says which
  values it copied. A copied value is the front matter's from then on: a later change of
  the package does not change it. A row is read-only only where writing the front matter
  there would be an error. Before the first build that
  prepares the document, the rows come from the schema made without a document. When the
  math processor chosen, `NAME`, has settings (those of the feature `math-NAME`), the link
  *options…* after its choice shows them in the advanced list.

  *Current template settings* lists the settings of the [compose
  template](#compose-templates) that assembles the output shown, under a line that gives
  the template's name and the output's: one row for each setting that the template
  declares, with the setting's default. The control follows from the setting's declared
  type: a boolean has a switch, a choice a list of options, a number or a text a field, a
  list of texts one chip per text, and any other value an editor of its YAML text; a
  setting that also accepts `null` has a checkbox before its control, which, unchecked,
  gives it no value. A change is written under
  `compose.template_config`, under the output's name and then the template's. For a
  template with more than ten settings, the tab shows the first ten and those that the
  front matter writes; *More…* shows the others. A setting that the front matter writes
  and the template does not declare is listed last. The topic is left out for a template
  without settings.

  The packages that `use` lists are in their order, each with a handle: drag it, or press
  the up and down arrow keys on it, to move the package. Under them, the other packages in
  use, which cannot be removed there, each with a padlock whose tooltip says why it is in
  use: the math processor's package, the build type's, a package of `initial_use`, a
  package given with `--use`, or a package that another package uses. The packages found
  that are in neither list follow under *Available*, each with *Add*; the packages `math-NAME` and
  `profile-NAME` come last there, without *Add*, since the program uses them by the
  choice of the math processor and of the build type (a package of these written in `use`
  is listed and can be removed like any other). In each list, a package's description
  stands under its name, cut to one line, and whole in its tooltip. A feature or a key
  that the front matter writes and that no package in use has is listed with the mark
  *no package in use*.

  When the last build could not prepare the document (a feature switched off that the
  build needs, a value of the wrong type, a package that cannot be found), the tab says
  so at its top, with the reason and a link to the *Problems* tab, and stays usable, so
  that the setting at fault can be repaired from it. What the front matter writes is shown
  as it is now. The values it does not write are those of the last successful build,
  dimmed and marked so; when the packages cannot be found, the lists of the features,
  the settings and the packages are those of that build too. A change that would copy
  such a value into the front matter is not made until a build succeeds: its control is
  disabled, or the change is refused with a line in the status bar.

Each change is written into the file at once: for a text field, when you leave the field
or press Enter; for a switch, a choice or a checkbox, at the change. A green check shows
on the row once the change is written. The preview then builds again, as after any save of
the file, and a value that a feature refuses is reported in *Problems* at its line in the
front matter. When the front matter cannot be read (it is not valid YAML, for example,
while you are editing it), the tabs say why, and their controls are disabled.

A change rewrites only the values that it changes, in place, apart from the few layout
details stated below. The comments, the blank lines, the quoting and the style of each map
and list (on several lines, or `{…}` and `[…]` on one line) are kept, and so are the two
`---` lines and the document below them, byte for byte. A line that a change adds is
indented like the lines beside it and, in a file whose lines end with a carriage return
and a line feed, ends with a line feed alone. A key added goes at the end of its map,
except at the top level, where the metadata (the title, the authors, …) stays before
`flm`, and under `flm`, whose keys are placed in the order `initial_use`, `math`, `use`,
`default_profile`, `layout`, `parsing`, `features`, `feature_order`; the keys already
written keep the order they have. A text written into a map written on one
line (`{…}`) is written between double quotes. A file with no front matter gains one,
between two `---` lines above the document. When a change cannot be written exactly as
the front matter is laid out, nothing is written and the status bar says so; set the
value in your editor then. Two kinds of value are never written from the page: a text of
several lines, and a map of settings placed as an entry in a list written on several lines
(`- …` lines).

**Save the file in your editor before changing it from the page.** The program writes the
file as it is on the disk, and only when its front matter is still the one that the page
read: a change made after the front matter changed on the disk is refused, and the tabs
read the front matter again. An editor that reloads the files it shows (most do) then
shows the change; one that does not may write its own copy over the change at its next
save.

### Exporting the document

The page of the preview cannot be saved as it is: it carries the places of its elements in
the source and the preview's script. The *Export* tab writes the document as `flm render`
writes it, with the options the preview was started with:

- *Build type* selects the profile to export, the build type shown until you choose
  another.
- *Save as sibling* writes the export into the document's folder, under the name that
  `flm render -o STEM.EXT` gives it: `STEM` is the document's file name without its
  extension, and `EXT` the extension of the build type's main file (`paper.html`,
  `paper.pdf`). The files of an earlier export of the same name are replaced; an export
  whose name is that of a file the document reads is refused. The tab then says
  *Successfully wrote “paper.html”.*, with *Open in default app*, which opens the file as
  `flm render -V` does.
- *Download* writes nothing beside the document: the browser receives the export's file,
  or, for an export of several files (such as `paper.tex` with its style file), a zip
  archive `STEM.zip` of them.
- *Terminal command* shows the `flm render` command that writes the same files from a
  terminal, as in `flm render -p html -o paper.html paper.flm --view`, with a button that
  copies it.

The export is a build of its own, which the program runs between two builds of the
preview; a `pdf-latex` export runs LaTeX. As `flm render` does, an export stops at the
document's errors (see [Documents with errors](#documents-with-errors)). A yellow box above
the buttons states the problems of the preview's last build:

- When the document has errors, the box says how many, with *Show problems* and the
  checkbox *Export despite the errors*. The buttons are enabled once the checkbox is
  checked, and the export then continues through the errors, as `flm render
  --continue-with-errors` does (the command shown gains that option).
- When the document has warnings alone, the box gives their count, with *Show problems*,
  and no checkbox.

The build type exported may have errors that the preview's build has not. When an export
is refused for such errors, the box holds the reason of the refusal, the checkbox, and
*Show log*, which shows every error of the export's build; from the next build on, the box
again states the preview's problems. An export that fails for another reason (a LaTeX run
that fails, for example) writes nothing, and the tab shows the reason, with *Show log*.

### Opening the source in an editor

A click on an element of the document's page with Ctrl or Cmd, and Shift, opens its
place in the editor: the file, at the line (its front matter counted) and the column (in
characters) where the element is written; the element flashes. An element with no place
of its own takes the place of the nearest element before it or around it that has one.
*Open in editor*, on a problem of the *Problems* tab or on the card of Inspect, opens the
place the same way. Only a file that the last build read is opened. The editor runs
separately from the preview: the preview goes on when the editor ends, and ends without
ending the editor.

**The editors.** The preview can open a place in ten editors: Visual Studio Code
(`code`), Cursor (`cursor`), VSCodium (`codium`), Zed (`zed`), Emacs (`emacsclient`, in the
frame already open), Sublime Text (`subl`), gVim (`gvim`, the graphical Vim) and MacVim
(`mvim`), both in the window already open or else in a new one, Vim (`vim`) and Neovim
(`nvim`). When the program starts, it looks for each of them: first for its program in the
folders of `PATH`, then, on macOS, for the application that installs it in `/Applications`
or `~/Applications` (`Visual Studio Code.app`, `Cursor.app`, `VSCodium.app`, `Zed.app` or
`Zed Preview.app`, `Emacs.app`, `Sublime Text.app`, `MacVim.app`), so that an editor whose
command-line program was never installed is found too.

**Vim and Neovim in a terminal.** The preview starts the editor without a terminal, so it
reaches a Vim or a Neovim already running, as `emacsclient` reaches an Emacs that runs a
server. Vim must listen as the server `VIM`: start it as `vim --servername VIM`, or call
`remote_startserver('VIM')` in your `vimrc`; this needs a Vim built with the feature
`clientserver` (`vim --version` lists `+clientserver`). Neovim must listen at the address
`/tmp/nvimsocket` (on Windows, `\\.\pipe\nvimsocket`): start it as
`nvim --listen /tmp/nvimsocket`. When no editor listens, the editor started ends with a
failure, which the preview reports.

**An editor of your own** is given as a command, with `--editor-command` or with the key
`editor_command` of the [viewer settings](#the-viewer-settings); it appears as *Custom
command* among the editors, under the name `custom`. The command is split into words at
spaces, without a shell: a part between single or double quotation marks keeps its spaces,
and no other character is special (a backslash stays as it is, as in a path of Windows).
The first word is the program. `{file}`, `{line}` and `{column}` are replaced in each word
by the file's absolute path, the line and the column (`{file}:{line}` makes one word), and
the file is added as the last word when no word holds `{file}`. For instance,
`myeditor --line {line} --column {column}` opens `paper.flm` at line 12, column 5 as
`myeditor --line 12 --column 5 /home/me/paper.flm`. An empty command is no command. The
page never writes a command of its own: choosing *Custom command* there when the viewer
settings have no editor command writes the placeholder `echo {file}:{line}:{column}`, which
opens nothing, for you to replace by your editor's command in the file.

**The editor chosen.** The *Editor* choice of the *Viewer* tab lists every editor, then
*Custom command*; an editor not found is grayed out and cannot be chosen. A place opens in
the one selected there. The choice is written into the viewer settings, so it holds for
every document and every later run. A place opens in the first available editor among, in this
order: the one that `--editor` gives (until an editor is chosen on the page), the one of
the viewer settings, the editor command, and the first editor found.

**Without an editor.** When no editor can be opened (none was found, or the one chosen
cannot be started), the place `PATH:LINE:COLUMN` is copied to the clipboard, so that you
can paste it into an editor's dialog to go to a file, and the status bar says why no editor
opened. When something else prevents the opening (a file that the last build did not
read, for example), the status bar says why.

The terminal reports the same requests. When the program starts, it lists the editors
found, each with the path of its program, then the editor command when there is one, and
the editor in which a place opens; or it says that none of the ten editors was found and
where they were looked for. For each place that the page asks to open, it prints the file,
the line and the column opened and the editor, or why nothing was opened. An editor that
ends with a failure after it started (an `emacsclient` with no Emacs server running, for
example) is reported there too, with what the editor wrote on its standard error.

### The viewer settings

The *viewer settings* say how you want the preview shown, whatever the document: they
hold across documents, runs and browsers, and never affect a build. The program keeps them
in the file `.flm/watch.json` of your home folder (the environment variable `HOME`, or
`USERPROFILE` where `HOME` is not set), a JSON object:

```json
{
  "editor": "code",
  "editor_command": "myeditor --line {line} --column {column}",
  "theme": "dark",
  "scroll_to_changes": true,
  "open_problems_on_errors": false,
  "wrap_lines": true,
  "panel_side": "right",
  "panel_width": 420,
  "panel_height": 300
}
```

| key | what it sets | default |
|---|---|---|
| `editor` | the editor in which a place opens: `code`, `cursor`, `codium`, `zed`, `emacsclient`, `subl`, `gvim`, `mvim`, `vim`, `nvim`, or `custom` for the editor command | the first available among the editor command and the editors found |
| `editor_command` | the command of an editor of your own (see [Opening the source in an editor](#opening-the-source-in-an-editor)) | none |
| `theme` | the theme of the tools page: `system` (light or dark as the system is set), `light` or `dark`. The document's page keeps its own colors | `system` |
| `scroll_to_changes` | whether the page scrolls to what a build changed and flashes it | `true` |
| `open_problems_on_errors` | whether a build with an error opens the closed panel on *Problems* | `false` |
| `wrap_lines` | whether a text output wraps its long lines | `true` |
| `panel_side` | the edge at which the panel is docked: `right` or `bottom` | `right` |
| `panel_width`, `panel_height` | the panel's width at the right edge and its height at the bottom, in pixels | the tools page's own |

The *Viewer* tab changes every key but `editor_command` and the panel's place; docking and
resizing the panel change those. Each change is written into the file at once, and every
open tools page follows it. A change writes only the keys it changes and keeps every other
key of the file as it is. The page never writes `editor_command`, which you write in the
file yourself: a command that the file holds is run on your machine, and the tools page
shares its address with the scripts of the document's page.

A file that is not a JSON object, a key with a value of the wrong kind, or an editor
command with a quotation mark left open makes the file unreadable: the terminal says why
when the program starts, the *Viewer* tab says why and its controls are disabled, the
defaults apply, and the file is never written until you correct it. A key that is not one
of these is kept and ignored. A program that has no home folder keeps the viewer settings
in memory, for the run alone.

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
```

### The pages and their paths

Each page has a *path*, which is what `flm info` is given to show that page. The pages form
a tree: each page lists the pages directly below it. Without a path, `flm info` shows `/`,
the environment at a glance (the profile in view, the math processor, the packages, the
features and the workflows), which leads to every other section.

| path | the page | example |
|---|---|---|
| `packages/`, `packages/NAME` | the packages; one package, with where it comes from (compiled into the program, or an extension and its manifest's description, author and engine) | `flm info packages/std` |
| `features/`, `features/NAME` | the features; one feature, with its documentation, its element types, the commands it declares, its scoped packages and its settings keys | `flm info features/headings` |
| `features/NAME/macros`, `…/environments`, `…/specials` | the commands of one kind that the feature declares | `flm info features/headings/macros` |
| `features/NAME/macros/MACRO`, `…/environments/ENV`, `…/specials/TRIGGER` | one command: how it is written, its arguments (each with its role) and an environment's body, the fields written after it, the feature that declares it, the modes it is available in, its element type | `flm info features/headings/macros/section` |
| `features/NAME/scopedpackages/PACKAGE`, and below it `macros/MACRO` and the others | a scoped package (the commands available only inside some argument or body) and its commands, as for a feature | `flm info features/refs/scopedpackages/flm.refs.label` |
| `settings/`, `settings/FEATURE` | the settings keys of a feature, each with its type, its default, the value in effect and where that value was written | `flm info settings/text` |
| `settings/parsing`, `settings/front-matter` | the parsing settings; every key a front matter may hold | `flm info settings/front-matter` |
| `workflows/`, `workflows/NAME` | the workflows; one workflow, with its outputs and, for the profile in view, its plan | `flm info workflows/html` |
| `namespaces/`, `namespaces/NAMESPACE`, `namespaces/NAMESPACE/NAME` | the families of names the registry holds; every name of one family; one name (the page of an element type lists its values, each with the commands written for it and its aspects) | `flm info namespaces/stage/flm.refs.resolve` |
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
program renders with the profile `text`, or `tty` on a terminal (below), using the markup of
the extension `flmdoc` (see [Writing documentation pages: the extension
`flmdoc`](crate::guide::extending#writing-documentation-pages-the-extension-flmdoc)); the
program carries that extension compiled in, among its bundled extensions. A program built
without its bundled extensions does not carry `flmdoc` and can print the pages only with
`--json`, which renders nothing; asked for text, it reports that the page cannot be
rendered and exits with status 3. A link to another page is printed as its text followed
by the command that shows that page: `headings <flm info features/headings>`.

The width of the lines depends on where standard output goes:

- when standard output is a terminal, the lines are as wide as the terminal: the value of
  the environment variable `COLUMNS` when it is a whole number above 0, else the width the
  terminal reports;
- otherwise (a file, a pipe), the lines are 80 columns wide, the default of the text
  backend.

When standard output is a terminal, the pages are rendered with the profile `tty` (see [Text
for a terminal](#text-for-a-terminal)): the same text, with bold and colored headings,
colored rules under the headings, tables in a frame, italics and hyperlinks shown by the
terminal. When the environment variable `NO_COLOR` is set and not empty, whatever its value
(the convention of [no-color.org](https://no-color.org)), the pages are painted without
colors (the setting `tty.colors=none`): bold, italics, underlining and hyperlinks remain. In
a file or a pipe, a pipe to a pager included, the pages are plain text, rendered with the
profile `text`.

No option sets the width or the look of the pages: `-O text.width=…` shapes the
environment described, as above; only the terminal and `NO_COLOR` do.

With `-r` (`--recursive`), every page below each page follows it, in the order of the tree:
`flm info -r features/headings` prints the feature's page and the page of every command it
declares.

### JSON output

With `--json`, `flm info` prints one JSON object on standard output, whatever the number of
pages: a header, `meta`, then the list `pages`.

```json
{
  "meta": { "kind": "info", "flm": "0.6.0", "format": 1 },
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
[`--json-messages`](#messages-as-json-records---json-messages).

### Documentation texts that are not valid FLM

Every documentation text a page shows is written in FLM, and the program's own texts are
tested to be valid. A text that an extension wrote, or one written as data in a front
matter or with `-O` (the documentation of a user definition or of a workflow, for
example), may not be. Before rendering a page, `flm info` parses each documentation text
the page shows; a text that is not valid FLM is printed as it is written, as plain text,
and a warning on standard error shows the text, the page, whose text it is and the
problems found. The exit status stays 0.

```text
warning: the documentation text declared by the feature `loud` (package `loud/_flmextensions/loud/flmextension.yaml#_`) on the page `features/loud/macros/shout` must be valid FLM:
  | Like `\textbf`, but 100% {louder.
  |
  | Its argument is the text.
  error: A lone ‘%’ is not allowed; use ‘\%’ for a percent sign, or ‘%%’ to start a comment
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
| 0 | every page asked for was shown, with or without warnings (a documentation text shown as plain text is one) |
| 1 | a reserved error exit status |
| 2 | the arguments are wrong, the document given with `-d` cannot be read or prepared, the environment cannot be prepared, or a path is no page's path and no name it looks up |
| 3 | a failure of the program itself, such as a page that does not render |

## Inspecting the processing with `flm dump`

`flm dump FILE` is an inspection tool: it processes one file with the library alone and
prints, as JSON on standard output, one view of what the processing holds: a document
tree at a chosen point of the processing, a computed table, or the artifacts of an
output. It serves a reader who wants to see what the library makes of a document, for
example while writing a feature or a lowering rule of their own. Its arguments, its
output and its exit statuses are its own, independent of the other commands, and it does
not accept `--json-messages`.

The processing and the views are done by the library `flm-info`, whose module `dump` is
the engine of this command: the program parses the command line, reads the file, and
prints what the engine returns. A program of one's own can call the same engine.

### What `flm dump` reads

The document that `flm dump` processes is the text of the file given on the command line
and nothing else. The command reads no settings file and no front matter, and it takes no
option that changes the language: every time, it builds its
[registry](crate::core::registry), the frozen record of everything the language and the
pipeline consult for a set of features, from the library's standard features
([`stdlib::add_features`](crate::stdlib::add_features)) under their default configuration and the
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
([`crate::stdlib::backends::pres`]), which the backend's emitter then writes out.
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

The backends of the standard features are `text`, `html`, `latex-rendered` and
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
"…"}}`. The outputs of the standard features hold none; a folder artifact comes from a
compose template that ships a folder, which `flm dump` does not apply, since it loads no
extension.

The outputs of the standard features are `text`, `latex-rendered`, `latex-recomposed` and
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
[`flm.output.text`](crate::stdlib::backends::text::TEXT_OUTPUT): it holds the printed
text, the position marks that say which byte range of it each element produced, and the
style marks (`styles`) that say which ranges are a heading's title, bold or emphasized text,
a link's content or verbatim content.

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
run. The default is [`text`](crate::stdlib::backends::text::WORKFLOW), which renders the
document as plain text. The standard features also have
[`latex-recomposed`](crate::stdlib::backends::latex_recomposed::WORKFLOW), which
recomposes the document into LaTeX; [`latex-rendered`](crate::stdlib::backends::latex_rendered::WORKFLOW), which renders it as fully
resolved LaTeX for LuaLaTeX; [`html`](crate::stdlib::backends::html::WORKFLOW),
which renders it as an HTML page (the three are described in
[the rendering chapter](crate::guide::rendering)); and
[`check`](crate::stdlib::CHECK_WORKFLOW), which runs every check of the registry and
produces no output.

The four rendering workflows run the same checks as `check`, because each one also asks
the plan for the collection of checks [`flm.validated`](crate::stdlib::VALIDATED). A
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
[`TextRenderOptions`](crate::stdlib::backends::text::TextRenderOptions) value that the
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
warning: Label ‘sec:nowhere’ is undefined
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
carry each formula as MathML, with no script. Two other processing features come with
the program: the library's package `math-passthrough`, which reads nothing inside a
formula, accepts any command and carries the source to the output, where MathJax typesets
it in the reader's browser; and the extension `math-techxt`, which renders each formula as
Unicode text on the text and HTML outputs ([Mathematics rendered as Unicode
text](#mathematics-rendered-as-unicode-text)).

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

**Choosing another processor.** A document uses `math-passthrough` by choosing the
processor `passthrough`, in the front matter or with `--math passthrough`, and
`math-techxt` by choosing `techxt`:

```yaml
flm:
  math: passthrough
```

Under `math-passthrough`, nothing checks the formulas before the program that reads the
output does: MathJax in the reader's browser on the HTML output, LuaLaTeX on the rendered
LaTeX and LaTeX on the recomposed LaTeX. A command that program does not know reaches it
as written. Every run of these three outputs therefore reports the warning
`flm.run.unchecked-output` once, at its start, whatever the document holds: `Unchecked
output for LaTeX: feature ‘math-passthrough’ writes each formula's recomposed LaTeX as is
(backend ‘latex-recomposed’)` on the recomposed LaTeX, for example. The text output
prints each formula's source, and reports nothing of the kind.

`math: false` (or `--math false`) uses no processor, for a document that lists one under
`use`. Two processing features at once are an error: a document that lists
`math-passthrough` under `use` and keeps the processor `mathjax` has both, as has one that
lists `math-techxt` under `use` and keeps `mathjax`. With no
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
  without running a script. The MathML of each formula is kept in the program's cache
  (see [The cache](#the-cache)), so a later build typesets only the formulas that
  changed.

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
| `lazy` | `false` | in `load-in-browser` mode, whether MathJax typesets each formula only when it scrolls into view; a reference to an equation line then leads to the start of the line's formula, which may not be typeset yet |
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

## Mathematics rendered as Unicode text

The extension `math-techxt` is a processing feature that renders each formula as Unicode
text with techxt, a converter from LaTeX to text: Unicode symbols, scripts and math
alphabets, with TeX's spacing. `\(x^2 + \alpha\)` becomes `𝑥² + α`, and a matrix in a
displayed formula is drawn over several lines.

**Using it.** The extension is compiled into the program (its source is the folder
`extensions/rust/math-techxt/` of the FLM repository), so nothing needs to be installed.
A document chooses it with the processor `techxt`, in its front matter or with `--math
techxt`:

```yaml
flm:
  math: techxt
```

**What each output gets.** On the text output, an inline formula is text within its line,
and a displayed formula or a display-math environment stands on lines of its own, indented,
each equation line followed by its number or tag. On the HTML output, each formula is a
`<span>` of the rendered text, set in the font JuliaMono with its spaces kept, so that a
drawing over several lines stays aligned; a displayed formula is a block. On both outputs
the argument of `\text{…}` and of the other text commands is document content placed
inside the formula: a reference in it shows its number, and is a link on HTML. On the two
LaTeX outputs techxt is not used: each formula is written as LaTeX, as under
`math-passthrough`, and every run of these outputs reports the warning
`flm.run.unchecked-output` once, at its start, since nothing checks that LaTeX accepts the
formulas: `Unchecked output for LuaLaTeX: feature ‘math-techxt’ writes each formula's
LaTeX source as is (backend ‘latex-rendered’)` on the rendered LaTeX, and `Unchecked
output for LaTeX: feature ‘math-techxt’ writes each formula's recomposed LaTeX as is
(backend ‘latex-recomposed’)` on the recomposed LaTeX.

**Strict commands.** A formula may hold the commands of techxt's notation (the Greek
letters, operators, relations, arrows, `\frac`, `\sqrt`, the delimiters, the matrix
environments, the math alphabets, the accents, the spacing commands and the decorations
such as `\overline`) and the `math` feature's own, as under `math-mathjax`. A command of
techxt's document library, such as `\eqref`, `\ref` or `\cite`, written directly in a
formula is an error where it is written; inside `\text{…}` it is resolved as anywhere in
the text. The file `README.md` of the extension lists the commands, and the names that
techxt's tables hold and that stay valid inside a formula although they are not notation.

**What a run reports.** A line of a display-math environment that holds no mathematics is
the error `math-techxt.empty-line`, on every output and in `flm check`. What techxt
reports about a formula is reported on the text and HTML outputs only, where the formulas
are rendered: `math-techxt.conversion`, at techxt's severity, and the errors
`math-techxt.conversion-failed`, `math-techxt.private-use-character` and
`math-techxt.too-many-text-commands`. A formula with one of these three errors is written
as its LaTeX source, and on HTML the page then loads MathJax for it.

**The setting.** It is written under `flm.features.math-techxt` in the front matter, or
with `-O math-techxt.font_url=VALUE`:

| setting | default | what it sets |
|---|---|---|
| `font_url` | JuliaMono 0.63.2 on cdnjs | the address of the font file, in the WOFF2 format, that the HTML page loads for the formulas; a font in which every glyph has one width |

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
