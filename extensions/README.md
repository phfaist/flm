# Extensions shipped with FLM

This crate is part of the [`flm`](https://github.com/phfaist/flm) project, the
Flexible Latex-like Markup language. FLM's github repo is at
<https://github.com/phfaist/flm>, and you'll find the FLM documentation at
<https://docs.rs/flm/latest/flm/>.

This folder holds a collection of FLM *extensions* that come with the
repository. An extension is a folder of files that the `flm` program finds by
name and that provides a package a document can use: settings of features,
workflows, and compose templates (the page layouts that arrange a document's
parts into a finished output file). The command-line guide
(`docs/command-line.md`, section "Extensions") describes extensions in full.

The folder is sorted by the *engine* of each extension, the part of the program
that turns the extension's files into a package: `data/` holds extensions of the
engine `data` (a manifest `flmextension.yaml`, and compose templates under
`templates/OUTPUT/`, each the folder `NAME/` holding the description
`flmtemplate.yaml` and the template's own files, or the file `NAME.yaml`, a
description alone; the description gives the template's `name` and `output`);
`js/` holds extensions of the engine `js` (a main module in JavaScript); `rust/`
holds extensions written in Rust, whose code is compiled into the program (the
engine `native`) and also builds as a WebAssembly module (the engine `wasm`).

The extensions:

| extension | engine | what it provides |
|---|---|---|
| [`data/lecturenotes`](data/lecturenotes/README.md) | `data` | the compose template `lecturenotes` for the output `html`: a book-length document as one page, with a sticky bar, a sidebar holding the table of contents, and previews of references on hover |
| [`js/letter`](js/letter/README.md) | `js` | the compose template `letter` for HTML and rendered LaTeX: letters with optional logos and page-width letterheads, European and US layouts, and custom CSS and LaTeX preambles |
| [`js/math-mathjax`](js/math-mathjax/README.md) | `js` | mathematics processed by MathJax: the commands and environments of MathJax's TeX packages inside formulas, and each formula typeset by MathJax in the reader's browser (`load-in-browser`) or written as MathML into the page (`mathml`) |
| [`rust/flmdoc`](rust/flmdoc/README.md) | `native` (compiled in), `wasm` | the markup of documentation pages, such as the pages `flm info` shows: code spans between backticks, the documentation of an item (its signature, summary, parameters and facts), and links to other pages, for the `text` and `html` profiles |
| [`rust/math-techxt`](rust/math-techxt/README.md) | `native` (compiled in), `wasm` | mathematics rendered as Unicode text by techxt (`math: techxt`): the commands and environments of techxt's notation inside formulas, and each formula written as Unicode symbols, scripts and alphabets on the text and HTML outputs, with the text inside it rendered by FLM; on the LaTeX outputs, each formula written as LaTeX by the library's rules |
| [`rust/phfpack`](rust/phfpack/README.md) | `native` (compiled in), `wasm` | notation and quantity macros for quantum information in formulas: bras and kets, norms, operators, gates, groups and entropies, with delimiters drawn at the size a use asks for, the backtick form, and macros declared in a document's settings |
| [`rust/profile-pdf-latex`](rust/profile-pdf-latex/README.md) | `native` (compiled in), `wasm` | the workflow `pdf-latex` and its PDF output `pdf-latex`: the LaTeX for LuaLaTeX (`latex-rendered`) compiled by `latexmk -lualatex` in a temporary folder |
| [`rust/profile-tty`](rust/profile-tty/README.md) | `native` (compiled in), `wasm` | the workflow `tty` and its output `tty`: the text output (`text`) with escape sequences for a terminal, showing its headings and bold, italic and underlined text, its links to addresses outside the document as hyperlinks, and its images drawn with colored half-block characters |
| [`rust/syntax-highlight`](rust/syntax-highlight/README.md) | `native` (compiled in), `wasm` | syntax highlighting, by Syntect, of code given a language (the optional argument of `\verbcode` and `verbatimcode`), for the workflows `html` and `latex-rendered` and those that extend them |

## Using an extension from this folder

This folder is also the crate `flm-extensions`, which compiles the extensions into a
program. Each extension has a cargo feature of its own, named after its folder
(`lecturenotes`, `math-mathjax`, `phfpack`, …); the feature `all` turns on every
extension, and no feature is on by default. With an extension's feature on, the crate
holds the extension in two tables, which a program gives to the host library `flm-host`:

- the table of files `EXTENSIONS`: every file of the extension's folder, embedded by the
  build script `build.rs`, except the entries whose name begins with `.` and, for an
  extension written in Rust, its folders `src/` (its code) and `tests/`;
- for an extension written in Rust, the table of code `COMPILED_EXTENSIONS`: its stem and
  its compiled code (see below).

Since everything in an extension's folder is embedded, the folder holds only the files
the extension ships. Some of those files are built ahead of time and committed (the
lecture-notes page's stylesheet and script, the MathJax bundle); the tools that build
them are in the repository's npm workspace, `npm/` (see `npm/README.md`).

The `flm` program compiles in every extension of this folder: its cargo feature
`bundled-extensions`, on by default, turns on the feature `all`. The program finds the
extensions by name, after every folder of its search path, so `cargo install flm`
installs them and nothing else is needed. A document lists the extension under `use` in
its front matter, by the name of the extension's folder:

```yaml
---
title: My lecture notes
flm:
  use: [lecturenotes]
---
```

An extension named `profile-NAME` needs no `use` entry: the profile `NAME`, given with
`-p NAME`, uses it, and runs its workflow `NAME`. `profile-pdf-latex` and `profile-tty`
are such extensions:

```text
flm render -o out/notes.html notes.flm
flm watch notes.flm
flm render -o out/paper.html paper.flm   # use: [math-mathjax]
flm render -p pdf-latex -o out/paper.pdf paper.flm   # the profile pdf-latex: nothing in the front matter
flm render -p tty paper.flm | less -R                # the profile tty: text for a terminal
```

## Using a modified copy

A copy of an extension placed on the search path is found before the compiled one and
is used in its place. To try a change to an extension of this folder, point the search
path at the folder that holds it with `--extensions-dir` (`extensions/data` or
`extensions/js`), which reads the files as they are on the disk:

```text
flm render --extensions-dir extensions/data -o out/notes.html notes.flm
flm render --extensions-dir extensions/js -o out/paper.html paper.flm
```

Any other folder of the search path works as well: copy or link the extension's folder
into the folder `_flmextensions` beside the document, into a folder listed in the
environment variable `FLMPATH`, or into `~/.flm/extensions`. The option `--extensions-dir`
replaces the folder `_flmextensions` beside the document; `FLMPATH` and `~/.flm/extensions`
are still searched after it. A change to a file of this folder reaches the compiled
program when it is built again.

A copy of an extension written in Rust is its WebAssembly form, which the script
`rust/build-wasm.sh` builds (see below): `--extensions-dir target/wasm-extensions` then
reads the module in place of the extension compiled into the program.

## Extensions written in Rust

An extension in `rust/` is a module of the crate `flm-extensions`, compiled under the
extension's cargo feature, whose code is the folder `rust/NAME/src/`. The code is written
against the guest client of the extension interface (the module `flm_ext::guest` of the
crate `flm-ext`, with the cargo feature `guest`). Its folder `rust/NAME/` also holds the
extension's manifest `flmextension.yaml`, its `README.md` and, for some, its tests under
`tests/`, each declared as a `[[test]]` in `Cargo.toml` that requires the extension's
feature. The same code has two forms:

- **compiled into a program.** The module declares the static `FLM_EXTENSION`, the
  extension's code, and the build script adds a row for it to the table of code
  `COMPILED_EXTENSIONS`. A program that gives this table to the host library (the cargo
  feature `native` of `flm-host`, which the feature `bundled-extensions` of `flm` turns
  on) runs the extension with the engine `native`, in the same process, with no sandbox
  and no limits, as it runs the program's own code. The manifest `flmextension.yaml` is
  the manifest of the WebAssembly form; the program reads it with the engine `native` in
  place of `wasm`, and without the module.
- **a WebAssembly module.** The crate built for the target `wasm32-unknown-unknown` is a
  module that holds the code of the extensions whose features are on; the macro
  `flm_ext::guest::extensions!`, invoked in `src/lib.rs` over the table of code, writes
  its exports.

The script `rust/build-wasm.sh` builds the WebAssembly forms, in one of two outputs:

- `rust/build-wasm.sh [--out DIR] [NAME …]` builds, for each extension NAME written in
  Rust (all of them when no name is given), the crate with that extension's feature
  alone, and writes the folder `DIR/NAME/` (by default `target/wasm-extensions/NAME/`):
  the module, under the file name that the manifest gives, and every other file of the
  extension's folder except `src/` and `tests/`. The folder is an extension as the `flm`
  program finds it on its search path, for a program that does not carry the extension
  compiled in, or to try the module itself.
- `rust/build-wasm.sh --collection FILE NAME …` builds the crate once, with the feature
  `collection` and the features of the extensions given, and writes the module as FILE,
  whose name ends in `.flmextensions.wasm`. This module is a *collection*: it holds the
  code of each extension written in Rust among them and, in its custom section
  `flmextensions`, every file of every extension given, of `data/`, `js/` or `rust/`. A
  program loads a collection itself and offers its extensions; the `flm` program reads
  none.

The script needs the Rust target `wasm32-unknown-unknown` (`rustup target add
wasm32-unknown-unknown`). It builds each output in a cargo run of its own, with the
package `flm-extensions` selected alone (`cargo rustc -p flm-extensions
--no-default-features --features NAME --target wasm32-unknown-unknown --profile
wasm-extension --crate-type cdylib`): a cargo run that also builds the program would
compile the extensions for the engine `native`, and the module would lack the functions
a module exports. Neither a cargo build nor an ordinary test run runs the script; the
tests that run it do so only when the environment variable `FLM_TESTS_WASM_GUEST` is
`1`.

Adding an extension written in Rust takes four steps:

1. a folder `rust/NAME/` holding the code under `src/` (its `lib.rs` declares the static
   `FLM_EXTENSION`, as `pub static FLM_EXTENSION: flm_ext::guest::CompiledExtension =
   flm_ext::guest::CompiledExtension::new(define, "NAME", env!("CARGO_PKG_VERSION"));`,
   `define` being the function that declares the extension's packages), the manifest
   `flmextension.yaml` (`engine: wasm`, `wasm: {module: NAME.wasm}`, each `-` of the name
   written `_`) and the `README.md`;
2. a cargo feature `NAME` in `Cargo.toml`, turning on `flm-ext/guest` and the extension's
   dependencies, each declared as an optional dependency of the crate; the feature is
   also added to the list of the feature `all`;
3. a module declaration in `src/lib.rs`, in name order: `#[cfg(feature = "NAME")]`,
   `#[path = "../rust/NAME/src/lib.rs"]` and `pub mod MODULE;`, MODULE being NAME with
   each `-` written `_`;
4. a row of the table of code, which the build script writes from the folder:
   `NamedExtension { stem: "NAME", compiled: &crate::MODULE::FLM_EXTENSION }`. The
   folder's name, the feature's name and the module's name must therefore match as
   above.
