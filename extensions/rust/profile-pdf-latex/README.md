# profile-pdf-latex

The extension `profile-pdf-latex` is the profile `pdf-latex` of the `flm` program: it
produces a document as a PDF file. It makes the LaTeX for LuaLaTeX that the program
already produces (the output `latex-rendered`) and compiles it with `latexmk -lualatex`.
The extension is written in Rust and is compiled into the `flm` program: nothing needs to
be installed but LaTeX itself.

## What it needs

The programs `latexmk` and `lualatex` must be in one of the folders of the environment
variable `PATH`. TeX Live and MacTeX install both, and put their folder on `PATH`. The
extension looks for `latexmk` on `PATH` only.

## Producing a PDF

The command gives the profile `pdf-latex`, which loads the extension `profile-pdf-latex`
and runs its workflow `pdf-latex`; the document's front matter needs nothing for it:

```text
flm render -p pdf-latex -o out/paper.pdf paper.flm
```

The command writes `out/paper.pdf` and nothing else: the LaTeX files are made in memory
and handed to LaTeX, not written beside the PDF. To keep the LaTeX files as well, run
`flm render -p latex-rendered -o out/paper.tex paper.flm`.

The workflow `pdf-latex` is the workflow `latex-rendered` with the extension's output
`pdf-latex`, the PDF file, in place of `latex-rendered`, and one setting more: each image
is copied into the LaTeX files (the strategy `copy` of the feature `graphics` for
`latex-rendered`), since LaTeX runs where the files that the document refers to by their
paths cannot be read. As under
`copy` for `latex-rendered`, an SVG drawing is converted into a PDF file by ImageMagick,
which must then be installed. The workflow's settings rank above the front matter, so `-O
graphics.strategy.latex-rendered=reference` changes this strategy and the front matter
cannot.

## How LaTeX runs

For each command, the extension makes a fresh folder under the system's temporary folder,
writes into it the files of the output `latex-rendered` (the main `.tex` file, its style
file, the images), runs

```text
latexmk -norc -lualatex -f -interaction=nonstopmode -file-line-error paper.tex
```

there, reads back the PDF and the log, and removes the folder, whatever happened (Ctrl+C,
or another signal that ends the command at once, may leave it in the system's temporary
folder). The extension runs `latexmk` through the host service `programs` of the `flm`
program, which does this work.

- `-norc` keeps a user's configuration files of `latexmk` (`~/.latexmkrc`, for instance)
  from changing where the PDF is written or which engine runs.
- Without `-halt-on-error`, LaTeX goes on past an error, so a PDF usually results.
- `-f` makes `latexmk` go on running LaTeX after a run that reported an error, as many
  times as the document needs, so that the page numbers of the table of contents and the
  other cross-references of that PDF resolve.
- `-shell-escape`, which would let the document start any program, is never passed.

When `latexmk` exits with a status other than 0, the command reports the error
`pdf-latex.failed`, with the first error that LaTeX's log records (or the last lines of
`latexmk`'s output when the log records none), and exits with status 1. When LaTeX
produced a PDF anyway, the PDF is still written.

When `latexmk` is not found on `PATH`, the command reports the error
`pdf-latex.latexmk-missing` and exits with status 1.

A document with errors stops before LaTeX runs, as every command that writes an output
does: nothing is written and the status is 1. With `--continue-with-errors`, LaTeX runs
and the PDF is written; the status is still 1.

## Compose templates

The compose template given with `-t` (or with the setting `template` of the feature
`compose`) applies to the output `latex-rendered`, from which the PDF is made:
`flm render -p pdf-latex -t NAME -o out/paper.pdf paper.flm` compiles the LaTeX that the
compose template `NAME` of `latex-rendered` makes.

## Settings

The feature `pdf-latex` has one setting:

| setting | what it holds | default |
|---|---|---|
| `timeout_s` | the time, in seconds, that `latexmk` may take; a run that takes longer is stopped and reported as the error `pdf-latex.failed` | 600 |

```yaml
flm:
  features:
    pdf-latex: {timeout_s: 1200}
```

On Linux, macOS and other Unix systems, the programs that `latexmk` started (LuaLaTeX) are
stopped with it at the end of its time, and so they are when a signal ends the command:
the interrupt that Ctrl+C sends, the quit signal that `Ctrl+\` sends, the hang-up sent
when the terminal closes, or `SIGTERM`. On Windows, only `latexmk` itself is stopped at the
end of its time, and a LuaLaTeX run it started goes on until it ends.

## The live preview

`flm watch -p pdf-latex paper.flm` runs LaTeX at each build and serves the PDF at the
preview's address, where the browser shows it with its own viewer. The page is not
updated in place and carries no toolbar: load the address again to see the last build.

## The WebAssembly form

The same crate builds as a WebAssembly module, which a program without the extension
compiled in loads from its search path:

```text
extensions/rust/build-wasm.sh profile-pdf-latex
flm render --extensions-dir target/wasm-extensions -p pdf-latex -o out/paper.pdf paper.flm
```

The script writes `target/wasm-extensions/profile-pdf-latex/`, holding this file, the
manifest `flmextension.yaml` and the module `profile_pdf_latex.wasm`. A copy of the
extension on the search path is used in place of the one compiled into the program.
