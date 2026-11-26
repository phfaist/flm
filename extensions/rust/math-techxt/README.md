# math-techxt

An FLM extension that renders mathematics as Unicode text with
[techxt](https://crates.io/crates/techxt): Unicode symbols, scripts and math alphabets,
with TeX's spacing between the parts of a formula.

On the text and HTML outputs, every formula is written as the text techxt renders for it:
`\(x^2 + \alpha\)` becomes `𝑥² + α`, and a matrix in a displayed formula is drawn over
several lines. The text inside a formula (`\text{…}` and the other text commands) is not
rendered by techxt: it is document content placed inside the formula, so a reference in
it is a link on HTML and shows its number, and its style is the document's. Each
equation's number or tag is document content too, and a reference to an equation line
leads to it.

On the two LaTeX outputs, the rendered and the recomposed LaTeX, techxt plays no part:
each formula is written as LaTeX from its nodes, with the library's rules, as under
`math-passthrough`, for LaTeX to typeset (see *The LaTeX outputs*, below).

The extension is written in Rust against the guest client `flm_ext::guest`. It requires
the standard feature `math`.

## Choosing it

The extension is compiled into the `flm` program, so nothing needs to be installed. A
document chooses it as its math processor, `techxt`, in its front matter:

```yaml
flm:
  math: techxt
```

or on the command line, with `flm render --math techxt paper.flm`. The same source also
builds as a WebAssembly module (`extensions/rust/build-wasm.sh`), which a folder named
`math-techxt` on the search path holds beside its manifest and which is then used in
place of the compiled one.

## The setting

The feature `math-techxt` reads one setting:

| Setting | Values | Default | Meaning |
|---|---|---|---|
| `font_url` | an address, not empty | JuliaMono 0.63.2 on cdnjs (`https://cdnjs.cloudflare.com/ajax/libs/juliamono/0.63.2/JuliaMono-Regular.woff2`) | the font file, in the WOFF2 format, that the HTML stylesheet loads for the formulas |

The font should give every glyph one width, so that a formula drawn over several lines
aligns, and hold the characters techxt writes; JuliaMono does both. For example, a copy
of the font beside the page:

```yaml
flm:
  math: techxt
  features:
    math-techxt:
      font_url: fonts/JuliaMono-Regular.woff2
```

## What a formula may hold

A command is available inside a formula when it belongs to techxt's notation and
techxt's document library does not define it. techxt's definitions come in categories:
seven render notation, and the others render a document (headings, lists, references,
citations, links, graphics, the title, the preamble). The extension renders with the
seven notation categories alone and declares, inside formulas:

- the commands and environments of the categories `mathcore`, `mathenvs`, `fontstyles`,
  `symbols_extra` and `accents` that techxt accepts in mathematics: the Greek letters,
  the operators, relations and arrows, `\frac`, `\sqrt`, `\binom`, the delimiters,
  `\left` and `\right`, the matrix environments, `cases`, `aligned`, `split` and
  `array`, the math alphabets such as `\mathbb`, the font commands, and the accents such
  as `\hat`;
- of the category `base`, which mixes the commands of mathematics with those of running
  text, only these:
  - the line break `\\`, which ends a row of a matrix, `cases`, `aligned`, `split` or
    `array` written in a formula;
  - the escaped characters `\{`, `\}`, `\%`, `\&`, `\#`, `\_` and `\$`, each written as
    its character (`\{x\}` and `\left\{ x \right\}` are `{𝑥}`);
  - the spacing commands `\,`, `\;`, `\:`, `\!`, `\ ` (a backslash and a space), `\quad`
    and `\qquad`;
  - the decorations `\overline`, `\underline`, `\widehat`, `\widetilde`, `\wideparen`,
    `\overleftarrow`, `\overrightarrow`, `\overleftrightarrow`, `\underleftarrow`,
    `\underrightarrow`, `\underleftrightarrow`, `\overleftharpoon`,
    `\overrightharpoon`, `\overbrace`, `\underbrace`, `\overgroup`, `\undergroup`,
    `\overbracket`, `\underbracket`, `\overlinesegment` and `\underlinesegment`;
  - `\phantom`, `\hphantom`, `\vphantom`, `\smash` and `\boxed`.

This list of `base`'s commands is the one list the extension writes by hand; every other
name is read from techxt's tables when the extension loads.

The rest of `base` (paragraphs, footnotes, page breaks, counters, the text symbols) and
techxt's document library are not commands of a formula. A name that the document
library defines is not declared even where `symbols_extra` also holds it, so `\eqref`,
`\ref`, `\cite`, `\section` and `\newcommand` written directly in a formula are unknown
commands, each an error where it is written. A reference goes inside a text command,
`\(a = b \text{ by \eqref{eq:lemma}}\)`, where FLM resolves it.

The `math` feature keeps its own commands: the text commands (`\text`, `\textbf`,
`\mbox`, …) and `\ensuremath`, and, on the lines of a display-math environment such as
`align`, `\label`, `\tag`, `\nonumber` and the `\\` that ends the equation line.
techxt's `\notag` is declared: it renders nothing and does not stop FLM numbering the
line, which `\nonumber` does.

A formula defines nothing: `\newcommand` and the other defining commands of techxt's
document library are unknown commands there, and the few that stay declared
(`\definecolor`, `\colorlet`, `\providecolor` and `\provideenvironment`, listed below)
define nothing. A user definition of the `userdefs` feature applies inside formulas as
usual. A document's own definition of a command or environment that the extension
declares fails the build, with an error naming both features; the definition then takes
another name. A feature that defines such a command clashes in the same way, and the
feature order chooses which definition applies: a chain that places the other feature
after `math-techxt` gives that feature's definition precedence. For example, for a
feature `mymacros` of a package the document uses:

```yaml
flm:
  feature_order:
    - math-techxt < mymacros
```

## The two stages and what they report

The extension reads the formulas in two stages.

- **The check**, the stage `math-techxt.check`, runs on every output and in `flm check`.
  It reports:
  - `math-techxt.empty-line` (an error): a line of a display-math environment holds no
    mathematics (a `\label`, `\tag` or `\nonumber` alone is not mathematics).
- **The rendering**, the stage `math-techxt.render`, runs only on the text and HTML
  outputs, where the formulas are rendered. It converts every formula once, nested
  formulas included (a formula inside a text command of a formula), a math group as one
  formula and a display-math environment one equation line at a time, and writes the
  rendered text into the table `math-techxt.rendered`. It reports:
  - `math-techxt.conversion`, at the severity techxt gives it (an error, a warning or a
    note): what techxt reports about a formula, such as `Problem rendering the formula as
    text: missing mandatory argument ‘denominator’` for `\frac{a}`. techxt's own
    identifier of the problem is in the diagnostic's data, under `condition`;
  - `math-techxt.conversion-failed` (an error): techxt cannot read the formula at all;
  - `math-techxt.private-use-character` (an error): the formula holds a private-use
    character, `U+E000` to `U+F8FF`, outside its text commands (the extension uses these
    characters to mark the places of the text commands in what techxt reads);
  - `math-techxt.too-many-text-commands` (an error): the formula, or an equation line,
    holds more than 6400 text commands, one per such character.

techxt's problems are therefore reported on the text and HTML outputs only, not on the
LaTeX outputs and not by `flm check`. On a LaTeX output a formula is not rendered by
techxt, and the output does not depend on it: an error of techxt's there would stop LaTeX
files that do not need techxt from being written. And on the recomposed LaTeX the plan may
keep the uses of the document's own macros as they are written, for the LaTeX style file
to define, so that techxt, which does not know them, would report each one.

A formula that techxt renders with a problem keeps the text techxt gives (`𝑎/` for
`\frac{a}`). A formula that techxt cannot read, or that holds a private-use character or
too many text commands, is written as its LaTeX source, between its delimiters,
`\(…\)`, `\[…\]` or `\begin{…}…\end{…}`, with its text commands and its numbers placed as
under `math-passthrough`; on HTML the page then loads MathJax, which typesets that
formula in the reader's browser.

## The presentation element

A formula rendered on the text and HTML outputs becomes the presentation element
`math-techxt.rendered-as-text`, which carries the rendered lines, each text command's
content lowered by FLM's rules, and, for an equation line, its anchor and its number or
tag. Each output writes it in its own way.

- **Text.** An inline formula is text within its line. A displayed formula, or a
  display-math environment, stands on lines of its own inside its paragraph, indented by
  two columns, with its spaces kept so that a drawing over several lines stays aligned;
  an equation line's number or tag follows the line's last printed line after one space.
- **HTML.** The formulas are `<span>` elements of the extension's own classes, which are
  not part of FLM's HTML contract. The outermost span of each formula also carries FLM's
  mark of a formula, which a stylesheet or a script of the page reads whatever wrote the
  formula: the class `flm-math-formula`, `data-flm-display` (`inline` or `display`), and
  for a display-math environment `data-flm-env`, its name:

  | What | Written as |
  |---|---|
  | an inline formula | `<span class="math-techxt flm-math-formula" data-flm-display="inline">` |
  | a displayed formula, a display-math environment | `<span class="math-techxt math-techxt-display flm-math-formula" data-flm-display="display">` (and `data-flm-env` for an environment) |
  | an equation line | `<span class="math-techxt-line">`, with the line's anchor as its `id` |
  | an equation line's rendered text | `<span class="math-techxt-text">` |
  | an equation line's number or tag | `<span class="math-techxt-tag">` |

  The page carries the stylesheet `math-techxt.style`: the font JuliaMono, loaded from
  `font_url` by an `@font-face` rule, then the system's monospace font; the white space
  kept as written; a displayed formula as a block, with the space of a displayed formula
  above and below, scrolling sideways when it is wider than the text; an equation line's
  number or tag at the right edge. The page also carries the standard stylesheet
  `flm.html.style.math-passthrough`, which sets the text inside a formula in the page's
  font.

A command of the extension reached outside a formula is written as its source, on every
output.

## techxt's own output

techxt's rendering differs from LaTeX's where an author may not expect it:

- the lowercase Greek letters are upright: `\alpha` is `α`, while a Latin letter is
  italic, `x` being `𝑥`;
- a fraction is written on one line, with a slash: `\frac{1}{2}` is `1/2`, and
  `\frac{a+b}{c}` is `(𝑎 + 𝑏)/𝑐`;
- a matrix, `cases` or another two-dimensional construct in a displayed formula is drawn
  over several lines, with tall delimiters made of pieces (`⎛` and `⎝`); in an inline
  formula, its rows follow each other on one line, separated by `;`
  (`\(\begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}\)` is `( 1 2; 3 4 )` on the text
  output).

## The LaTeX outputs

On the rendered LaTeX (for LuaLaTeX) and on the recomposed LaTeX, the extension
registers the library's own rules of the formulas, those of `math-passthrough`, under
names of its own (`math-techxt.math.lower.latex-rendered`, …): each formula is written
as LaTeX from its nodes, the argument of a text command lowered as document content, an
environment written line by line with its labels, and FLM's numbers as `\tag*{…}` on the
rendered LaTeX. techxt is not called and the table `math-techxt.rendered` is not read.

Nothing checks that LaTeX accepts these formulas: a command techxt knows and LaTeX does
not reaches LaTeX as written. Every run of these two outputs therefore reports the
warning `flm.run.unchecked-output` once, at its start, whatever the document holds:

- on the rendered LaTeX: `Unchecked output for LuaLaTeX: feature ‘math-techxt’ writes
  each formula's LaTeX source as is (backend ‘latex-rendered’)`;
- on the recomposed LaTeX: `Unchecked output for LaTeX: feature ‘math-techxt’ writes
  each formula's recomposed LaTeX as is (backend ‘latex-recomposed’)`.

A document whose formulas use commands that LaTeX does not define provides their
definitions to LaTeX separately.

## What the design intends, and what is not yet done

The extension has these limits. Each says whether it is what the design intends, what the
design gives from techxt's tables, or work not yet done.

- **A text command counts as one column** (not yet done). techxt renders a formula with
  a placeholder of one character where each text command stands, and FLM puts the
  command's content there afterwards. In a drawing over several lines, a matrix for
  example, the columns are aligned as if each text command were one character wide.
- **One equation line at a time** (not yet done). Each line of a display-math environment
  is rendered alone, as a displayed formula of its own: its `&` is dropped, and the lines
  of an `align` are not aligned at their `&`.
- **References directly in a formula** (what the design intends). `\eqref`, `\ref` and
  `\cite` are commands of a formula only inside a text command, where FLM resolves them.
- **Names that are not notation** (what the design gives, from techxt's tables). techxt's
  category `symbols_extra` is generated from another library's tables and holds some
  document and preamble commands. These sixteen are not defined by techxt's document
  library and stay valid inside a formula: the color commands `\color`, `\colorlet`,
  `\definecolor`, `\providecolor`, `\pagecolor`, `\nopagecolor`, `\rowcolors`,
  `\textcolor`, `\colorbox` and `\fcolorbox`, and `\addcounter`, `\addlength`,
  `\boxframe`, `\keywords`, `\provideenvironment` and `\selectlanguage`. Each renders
  nothing, except `\textcolor`, `\colorbox` and `\fcolorbox`, which render their text
  without its color.
- **Display-math environments inside a formula** (what the design gives, from techxt's
  tables). techxt's display-math environments stay valid inside a formula, where each
  renders its content: `align`, `align*`, `alignat`, `alignat*`, `displaymath`, `dmath`,
  `dmath*`, `eqnarray`, `eqnarray*`, `equation`, `equation*`, `flalign`, `flalign*`,
  `gather`, `gather*`, `math`, `multline`, `multline*` and `subequations`.
- **techxt's known defects** (not yet done, in techxt). `\xrightarrow` and `\xleftarrow`
  render nothing, their label included. A relation of `symbols_extra` gets no relation
  spacing: `a \lessapprox b` is `𝑎⪅𝑏`, where `a \leq b` is `𝑎 ≤ 𝑏`.

## The files

| File | What it holds |
|---|---|
| `flmextension.yaml` | the manifest of the WebAssembly form; the program reads it with the engine `native` for the compiled form |
| `src/lib.rs` | the package `_` and its feature `math-techxt` |
| `src/settings.rs` | the setting `font_url` and its reader |
| `src/vocabulary.rs` | the converter's definitions and the commands declared inside formulas |
| `src/feature.rs` | the registration of the feature |
| `src/check.rs` | the check stage |
| `src/render.rs` | the rendering stage and its table |
| `src/rules/` | the lowering rules: of the formulas on the text and HTML outputs, of the element on each of them with the stylesheet, and the library's rules on the LaTeX outputs |
| `tests/` | the tests, which load the extension through the extension interface |

techxt is licensed under the MIT License or the Apache License, Version 2.0; JuliaMono,
which the page loads and the extension does not carry, under the SIL Open Font License.
