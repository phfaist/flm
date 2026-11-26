# math-mathjax

An FLM extension that processes mathematics with MathJax.

With it, the commands and environments that MathJax's TeX packages define are
the commands available inside a formula (but for a few, listed below), read
from MathJax itself when the extension loads. A command no package defines is
an unknown command, reported where it stands when the document is parsed.
Every formula is then written for MathJax, in one of two modes.

- **`load-in-browser`** (the default): the page carries each formula's TeX,
  and MathJax, loaded into the page, typesets it in the reader's browser. The
  text of `\text{…}` and the other text commands, and each equation's number
  or tag, are document content placed inside the formula.
- **`mathml`**: MathJax runs while the document is prepared and the page
  carries each formula as MathML, with no script. The spacing between the
  parts of a formula is TeX's, as MathJax computes it for its own typesetting,
  written into the MathML as `mspace` elements and as each operator's `lspace`
  and `rspace`, so that the browser adds no spacing of its own. The text of
  `\text{…}` and the other text commands is document content placed inside the
  formula's `mtext` elements, laid out by the browser; each equation's number
  or tag is the last cell of its line's row, and each line's row carries the
  line's identifier, so that a reference to it leads there.

On the recomposed LaTeX and on the text output the formulas are written as the
author wrote them, with their numbers, in both modes. On the text output, a
displayed formula or a display-math environment stands on lines of its own,
indented, inside its paragraph, as the standard features set one.

Beyond MathJax's packages, the extension defines two commands inside a
formula, whatever `packages` lists: the delimiters `\llangle` (⟪, U+27EA, an
opening delimiter) and `\rrangle` (⟫, U+27EB, a closing delimiter). As with
`\langle` and `\rangle`, each may stand alone, after a size command
(`\bigl\llangle`, `\Bigr\rrangle`) or after `\left` and `\right`. They are
defined in MathJax's own way, as entries of a delimiter map in a TeX package of
the extension's own, `math-mathjax`, which MathJax reads beside the packages
listed, in both modes.

`\ensuremath{…}` is the `math` feature's: in text it is an inline formula,
processed as `\(…\)` is, and inside a formula the extension hands MathJax its
argument in place of the command.

The extension requires the standard feature `math`. No command
can be defined inside a formula: MathJax's `newcommand` package is never
offered, so `\newcommand` inside a formula is an unknown command.

Some of MathJax's commands are never declared, whatever `packages` lists, so
that each is an unknown command inside a formula:

- the commands that define something for later formulas, because a formula must
  not depend on another formula (they go in the setting `preamble`, described
  below): `\DeclareMathOperator`, `\Newextarrow`,
  `\DeclarePairedDelimiter`, `\DeclarePairedDelimiterX`,
  `\DeclarePairedDelimiterXPP`, `\DeclarePairedDelimiters`,
  `\DeclarePairedDelimitersX`, `\DeclarePairedDelimitersXPP`,
  `\MakeAboxedCommand`, `\definecolor` and `\newcolumntype`;
- `\mathtoolsset`, which changes a setting for later formulas, for the same
  reason;
- the commands of equation numbering, labels and references, because the `math`
  feature handles them: `\label`, `\ref`, `\eqref`, `\refeq`, `\tag`, `\notag`,
  `\nonumber`, `\newtagform`, `\renewtagform` and `\usetagform` (on the lines of
  a display-math environment, `\label`, `\tag` and `\nonumber` are the `math`
  feature's own, and `\eqref` is the `math` feature's in text);
- the display-math environments, because they are never written inside a
  formula (the `math` feature declares `equation`, `align` and `gather`, with
  their starred forms, in text): `equation`, `equation*`, `eqnarray`,
  `eqnarray*`, `displaymath`, `math`, `indentalign`, `align`, `align*`,
  `multline`, `multline*`, `gather`, `gather*`, `alignat`, `alignat*`,
  `xalignat`, `xalignat*`, `xxalignat`, `flalign`, `flalign*`, `numcases`,
  `subnumcases` and `empheq`. The environments made for use inside a formula
  (`aligned`, `gathered`, `split`, `multlined`, `cases`, the matrices, `array`
  and the others) remain.

A feature that defines a command MathJax also defines (for example `\Im` or
`\Re`) clashes with MathJax's definition. The feature order chooses which
definition applies: a chain that places the other feature after `math-mathjax`
gives that feature's definition precedence. For example, for a feature
`mymacros` of a package the document uses:

```yaml
flm:
  feature_order:
    - math-mathjax < mymacros
```

## Installing it

Copy this folder, named `math-mathjax`, into a document's `_flmextensions`
folder or into the user's extensions folder, and name it in the document's
front matter:

```yaml
flm:
  use: [math-mathjax]
```

The extension's package declares the host service `cache` (see below), so a
program loads the extension, in either mode, only when it offers a service
named `cache`. The `flm` program, and every program built on the host library
`flm-host`, offers it. A program built on the extension layer `flm-ext` alone
registers a service of that name; a service that answers `null` to every call
is enough, and the extension then typesets every formula at every build.

## Settings

The feature `math-mathjax` reads these settings:

| Setting | Values | Default | Meaning |
|---|---|---|---|
| `mode` | `load-in-browser`, `mathml` | `load-in-browser` | how the formulas are written into the page |
| `packages` | a list of package names | every package of the bundle | the MathJax TeX packages whose commands a document may use; a package the bundle does not hold is refused |
| `math_font_url` | an address | none | in `mathml` mode, a web font with a MATH table for the formulas |
| `loader_url` | an address | `https://cdn.jsdelivr.net/npm/mathjax@4/tex-chtml.js` | in `load-in-browser` mode, where the page loads MathJax from |
| `lazy` | `true`, `false` | `false` | in `load-in-browser` mode, whether MathJax typesets each formula only when it scrolls into view (MathJax's `ui/lazy` component) instead of all at once when the page loads; the anchors of an environment's lines then stand before its TeX, which MathJax replaces until it typesets the formula, so that a link to a line leads to the start of its formula |
| `preamble` | TeX | empty | TeX that MathJax processes once, before any formula, in both modes: what it defines and sets holds for every formula |
| `export_commands` | a list of command names, each without its backslash | none | the commands the preamble defines that a formula may use; each is declared as an FLM command in math mode |

For example, a document whose formulas use the `physics` package, written into
the page as MathML:

```yaml
flm:
  use: [math-mathjax]
  features:
    math-mathjax:
      mode: mathml
      packages: [base, ams, physics]
```

The setting `preamble` is the place for the commands that define something or
change a setting for every formula, which a formula itself may not hold:
`\definecolor`, `\newcolumntype`, `\mathtoolsset`, `\DeclareMathOperator`,
`\DeclarePairedDelimiter`, `\Newextarrow` and the others listed above. MathJax
processes it once, before any formula, and what it outputs for it is
discarded; the extension hands it to MathJax as written. A command the
preamble defines may be used inside a formula only when `export_commands`
lists it, and is then declared as MathJax's own commands are. A name in
`export_commands` that a MathJax package already defines changes nothing, so
the commands listed above stay unknown inside a formula whatever it lists. A
preamble that MathJax rejects, and a name in `export_commands` that neither the
preamble nor a package defines, fail the loading of the extension. `\newcommand`
is not available in the preamble, since MathJax's `newcommand` package is never
offered: a document defines its own commands with FLM's definitions (the
`userdefs` feature). The preamble is MathJax's alone: the text output and the
two LaTeX outputs do not receive it, so a document that uses its definitions
provides them to LaTeX separately. For example, a color and an arrow, written
`\(\color{myred} a \xMyarrow{f} b\)` in a formula:

```yaml
flm:
  use: [math-mathjax]
  features:
    math-mathjax:
      preamble: |-
        \definecolor{myred}{RGB}{200,0,0}
        \Newextarrow{\xMyarrow}{5,5}{0x21C0}
      export_commands: [xMyarrow]
```

The bundled packages are `base`, `ams`, `amscd`, `boldsymbol`, `braket`,
`bbox`, `cancel`, `cases`, `centernot`, `color`, `empheq`, `enclose`,
`extpfeil`, `gensymb`, `mathtools`, `mhchem`, `physics`, `textcomp`,
`textmacros`, `unicode`, `upgreek`, `units`, `verb` and `dsfont`
(`vendor/bundle.js`).

A document's own definition of a command or environment that a loaded package
also defines fails the build: for example, a document that defines `\ket`
while the `physics` package is loaded meets the error ``macro `ket` is defined
by features `userdefs` and `math-mathjax` in mode Math``. The author then
leaves that package out of `packages`, or gives the definition another name.
An empty `packages` list declares no MathJax command.

In both modes, the outermost element of each formula carries FLM's mark of a formula,
which a stylesheet or a script of the page reads whatever wrote the formula: the class
`flm-math-formula`, `data-flm-display` (`inline` or `display`), and for a display-math
environment `data-flm-env`, its name. In `load-in-browser` mode that element is the span
holding the formula's TeX (`<span class="math-mathjax-formula flm-math-formula" …>`), in
`mathml` mode the `math` element itself, and for a formula MathJax cannot typeset, the span
holding its source.

In `mathml` mode, MathJax converts each formula once (and once more for each
copy of it, such as a section title's formula shown in the table of contents,
when the program offers no cache; see below), in the engine limits the
manifest sets. One TeX input, configured when the
feature registers, converts every formula, after it has processed the
preamble. A formula MathJax cannot
typeset (for example `\frac{a}`, with an argument missing) is the error ``MathJax cannot
typeset the formula: Missing argument for \frac``, and the page shows the
formula's source as text.

In `mathml` mode, the extension keeps the MathML that MathJax gives for each
formula in the cache `typeset.mathml` of the program that runs it (a values
cache that all documents share, through the host service `cache`; the `flm`
program names it `flm.ext.math-mathjax.typeset.mathml`): a later build, or a
copy of the formula in the same build, reads the formula's MathML from there,
and MathJax does not convert it again. A formula's value
is kept for 90 days after its last use. `flm cache list` shows the cache's
size, and `flm cache clean flm.ext.math-mathjax.typeset.mathml` empties it. The
key of a formula holds:

- the version of MathJax that the bundle holds (`vendor/bundle.js`);
- the settings `packages` and `preamble`, which configure MathJax's TeX input;
- the setting `export_commands`, which selects the commands of the preamble
  that a formula may use; a change to it makes every formula typeset again;
- whether the formula is displayed;
- the TeX that the extension gives MathJax for the formula.

The other settings change only how the page shows the MathML, or apply only in
`load-in-browser` mode, so they are not part of the key.

The extension's own package (the two delimiters) is fixed with the extension's
code; a change to it, or to anything else in how the extension calls MathJax,
comes with a new version of the cache, which starts empty. A formula that
MathJax cannot typeset is not kept, so its error is reported at every build.

A page that holds a formula carries the stylesheet
`math-mathjax.style`: the font list of the `math` elements (the font of
`math_font_url` first, when set, then Latin Modern Math, STIX Two Math, Cambria
Math and the browser's `math` family), each displayed formula a centered
block, the table of an environment's equation lines at full width with each
line in the middle and its tag at the right edge, and the
text inside a formula in the page's font (the HTML backend's
`--flm-page-family`). MathJax's table attributes that MathML Core does not
define (`columnalign`, `columnspacing`, `rowspacing`) are written as the cells'
styles, and MathJax's `mlabeledtr`, which MathML Core lacks, as an `mtr` whose
last cell holds the tag. Each row of an environment's lines is padded with
empty cells to the widest line's cell count, so that the empty cell before
each line and the tag's cell after it are the first and the last column of
every row.

In `load-in-browser` mode, the page's MathJax configuration lists the packages
the document uses (with `base`, and MathJax's `texhtml`, which reads the
document content inside a formula) and the extension's own package, which it
defines when MathJax is ready and before MathJax reads its TeX configuration
(`startup.ready`), and the page keeps its formulas typeset when a script
updates parts of it in place or shows a copy of part of it, as the HTML
backend's own MathJax configuration does. With a preamble, the page's MathJax processes it once, before it
typesets the page, and the configuration lists every package of `packages`,
whatever the document uses, since the extension does not read the preamble to
know which packages it uses.

## The files

| File | What it holds |
|---|---|
| `flmextension.yaml` | the manifest: a JavaScript extension, its main module, and its engine limits: the 16 MiB stack and 64 MiB of memory that MathJax needs, and a 120 s time budget per call, because one call converts every formula of a document |
| `main.js` | the package `_`, its host service and its cache, and its feature `math-mathjax`, its settings, and its check |
| `definitions.js` | the reading of MathJax's command maps and the declarations made from it, and the two delimiters the extension defines |
| `tex.js` | the TeX of a formula, built from its nodes, with its holes (the places of document content) |
| `render.js` | the render stage of `mathml` mode: MathJax run on each formula that the cache does not hold, its MathML kept in the cache and written into a table |
| `lower.js` | the lowering rules, by backend, the conversion of MathJax's MathML into the page's elements, and the stylesheet of `mathml` mode |
| `page.js` | the script definition of `load-in-browser` mode and the stage that writes its configuration |
| `vendor/mathjax.js`, `vendor/bundle.js` | generated by the repository's `npm/math-mathjax`: the MathJax bundle, and the packages and version it holds |

## Rebuilding the bundle

`vendor/` is generated by the member `math-mathjax` of the repository's npm workspace
(`npm/math-mathjax/bundle.mjs`), under node (version 18 or later), from the versions that
`npm/math-mathjax/package.json` pins (`@mathjax/src` 4.1.3, `esbuild` 0.28.2) and that the
workspace's `npm/package-lock.json` locks:

```sh
cd npm
npm install
npm run build -w math-mathjax
```

It writes `vendor/mathjax.js` (one minified ES module, about 355 KiB) and
`vendor/bundle.js`, each beginning with a line naming `npm/math-mathjax/bundle.mjs` and
the MathJax version. The bundle holds MathJax's TeX input with the packages
above, its MathML serialization, extended to write TeX's spacing into the
MathML, and its DOM-free adaptor; no output jax and no font. Its `configure` defines the extension's own package from the delimiters
`definitions.js` gives it. The files of `vendor/` are never edited by hand.

MathJax is copyright The MathJax Consortium and licensed under the Apache
License, Version 2.0.
