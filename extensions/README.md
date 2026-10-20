# Extensions shipped with FLM

This folder holds FLM *extensions* that come with the repository. An extension is a
folder of files that the `flm` program finds by name and that provides a package a
document can use: settings of features, workflows, and compose templates (the page
layouts that arrange a document's parts into a finished output file). The command-line
guide (`docs/command-line.md`, section "Extensions") describes extensions in full.

The folder is sorted by the *engine* of each extension, the part of the program that
turns the extension's files into a package: `data/` holds extensions of the engine
`data` (a manifest `flmextension.yaml`, and compose templates under `templates/OUTPUT/`,
each the folder `NAME/` holding the description `flmtemplate.yaml` and the template's own
files, or the file `NAME.yaml`, a description alone);
`js/` holds extensions of the engine `js` (a main module in JavaScript); `rust/` holds
extensions written in Rust, each a crate that is compiled into the program (the engine
`native`) and that also builds as a WebAssembly module (the engine `wasm`).

The extensions:

| extension | engine | what it provides |
|---|---|---|
| [`data/lecturenotes`](data/lecturenotes/README.md) | `data` | the compose template `lecturenotes` for the output `html`: a book-length document as one page, with a sticky bar, a sidebar holding the table of contents, and previews of references on hover |
| [`js/math-mathjax`](js/math-mathjax/README.md) | `js` | mathematics processed by MathJax: the commands and environments of MathJax's TeX packages inside formulas, and each formula typeset by MathJax in the reader's browser (`load-in-browser`) or written as MathML into the page (`mathml`) |
| [`rust/flmdoc`](rust/flmdoc/README.md) | `native` (compiled in), `wasm` | the markup of documentation pages, such as the pages `flm info` shows: code spans between backticks, the documentation of an item (its signature, summary, parameters and facts), and links to other pages, for the `text` and `html` profiles |
| [`rust/profile-pdf-latex`](rust/profile-pdf-latex/README.md) | `native` (compiled in), `wasm` | the workflow `pdf-latex` and its PDF output `pdf-latex`: the LaTeX for LuaLaTeX (`latex-rendered`) compiled by `latexmk -lualatex` in a temporary folder |
| [`rust/syntax-highlight`](rust/syntax-highlight/README.md) | `native` (compiled in), `wasm` | syntax highlighting, by Syntect, of code given a language (the optional argument of `\verbcode` and `verbatimcode`), for the workflows `html` and `latex-rendered` and those that extend them |

## Using an extension from this folder

The extensions of this folder are compiled into the `flm` program: this folder is also
the crate `flm-extensions`, whose build script embeds every file of every extension under
`data/` and `js/` (except entries whose name begins with `.`), and which compiles in the
extensions written in Rust under `rust/` (see below). Since everything in an extension's
folder is embedded, the folder holds only the files the extension ships. Some of those
files are built ahead of time and committed (the lecture-notes page's stylesheet and
script, the MathJax bundle); the tools that build them are in the repository's npm
workspace, `npm/` (see `npm/README.md`). The program finds the extensions by name, after
every folder of its search path. A `cargo install` of the program installs them; nothing
else is needed. A document lists the extension under `use` in its front matter, by the
name of the extension's folder:

```yaml
---
title: My lecture notes
flm:
  use: [lecturenotes]
---
```

An extension named `profile-NAME` needs no `use` entry: the profile `NAME`, given with
`-p NAME`, uses it, and runs its workflow `NAME`. `profile-pdf-latex` is such an extension:

```text
flm render -o out/notes.html notes.flm
flm watch notes.flm
flm render -o out/paper.html paper.flm   # use: [math-mathjax]
flm render -p pdf-latex -o out/paper.pdf paper.flm   # the profile pdf-latex: nothing in the front matter
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

An extension in `rust/` is a crate of its own, a member of the repository's cargo
workspace, written against the guest client of the extension interface (the module
`flm_ext::guest` of the crate `flm-ext`, with the cargo feature `guest` alone). The same
source has two forms:

- **compiled into the program.** The crate `flm-extensions` depends on the extension's
  crate under its cargo feature `native`, which the program turns on (the feature
  `bundled-extensions` of `flm-main` and of `flm-cli`), and lists it in its table
  `NATIVE_EXTENSIONS`: its name, the text of its manifest and the code the crate's macro
  `extension!` gives. The program runs it with the engine `native`, in the same process,
  with no sandbox and no limits, as it runs the program's own code. The manifest
  `flmextension.yaml` beside the crate is the manifest of the WebAssembly form; the program
  reads it with the engine `native` in place of `wasm`, and without the module.
- **a WebAssembly module.** The script `rust/build-wasm.sh [--out DIR] [NAME …]` builds the
  module of each extension given (all of them when no name is given) and writes the folder
  `DIR/NAME/` (by default `target/wasm-extensions/NAME/`), holding the manifest, the
  `README.md` and the module: an extension as the program finds it on its search path, for
  a program that does not carry the extension compiled in, or to try the module itself.
  The script needs the Rust target `wasm32-unknown-unknown` (`rustup target add
  wasm32-unknown-unknown`). It builds each extension in a cargo run of its own (`cargo
  rustc -p NAME --target wasm32-unknown-unknown --profile wasm-extension --crate-type
  cdylib`): a cargo run that also builds the program would compile the extension for the
  engine `native`, and the module would lack the functions a module exports. Neither a
  cargo build nor an ordinary test run runs the script; one test, run only when the
  environment variable `FLM_TESTS_WASM_GUEST` is `1`, does.

Adding an extension written in Rust takes four steps:

1. a folder `rust/NAME/` holding the crate (`Cargo.toml` with `publish = false` and the
   dependency `flm-ext = { path = "../../../flm-ext", default-features = false, features
   = ["guest"] }`, `src/lib.rs` invoking `flm_ext::guest::extension!` once), its manifest
   `flmextension.yaml` (`engine: wasm`, `wasm: {module: NAME.wasm}`, each `-` of the name
   written `_`) and its `README.md`;
2. a line `"extensions/rust/NAME"` in the `members` of the repository's `Cargo.toml`;
3. an optional dependency of `flm-extensions` on the crate (`extensions/Cargo.toml`),
   added to the cargo feature `native`;
4. a line of the table `NATIVE_EXTENSIONS` in `extensions/src/lib.rs`, in name order.
