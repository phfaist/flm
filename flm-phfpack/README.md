# flm-phfpack

The FLM package `phfpack`: notation macros for quantum information — bras and kets,
norms, operators, gates, groups — whose delimiters are drawn at the size a use asks for,
the backtick form in formulas, the quantity macros — entropies and other measures of
information — and the macros a document's settings declare.

The crate is an extension of FLM written as any third party's would be: it depends on
the library `flm-core` alone, through its public interface. The `flm` program carries
it by default (its cargo feature `phfpack`). A document uses it by naming it:

```yaml
---
flm:
  use: [phfpack]
---
```

Then, in a formula: `\ket{\psi}`, ``\braket`\Big{\phi}{\psi}``, `\abs*{x}`, `` `(a+b) ``,
`\tr`, `\SU(2)`. The crate's rustdoc lists every macro, the three ways of asking for a
size (after a backtick, in brackets, or a star), the backtick form, how a document
declares macros of its own (under `flm:`, `features:`, `phfqit:`, `macro_definitions:`), and the named values the
templates read (`spaceKets`, `spaceOKets`, `space`).

The macros are written in formulas. A macro whose replacement is valid in running text
(`\Ident`, `\bit{0}`) works there too; any other is an error at a use in running text,
and the document is still rendered. In running text a backtick is a quotation mark.

## Quantity macros

Each stands for a quantity and writes its optional parts only when they are given. The
size of its parenthesis is written after a backtick, at the place the examples show it.
The parenthesis is left out when every part it holds is blank, so that `\Hmaxf{}` writes
the symbol alone:

| write | for |
|---|---|
| ``\Hmin`\Big[\rho][\epsilon]{X}[Y]`` | the smooth min-entropy of `X` given `Y` for the state `\rho`; `\HH`, `\Hzero` and `\Hmaxf` take the same arguments |
| `\Hfn(p_1\rho_1 + p_2\rho_2)` | the entropy as a function of its argument |
| `\II[\rho][\epsilon]{A}{C}[B]` | the mutual information between `A` and `C` given `B` |
| `\DD{\rho}{\sigma}`, `\DD_{\mathrm{Rob}}^{\epsilon}{\rho}{\sigma}` | a relative entropy, its subscript and superscript optional and in either order |
| ``\Dmax[\epsilon]`\big{\rho}{\sigma}`` | the max-relative entropy; `\Dminz`, `\Dminf`, `\Dr`, `\DHyp` and `\Dhyp` take the same arguments, the last two `\eta` when no epsilon is given |
| `\IdentProc[A][A']{\rho}` | the identity process from `A` to `A'`, applied to `\rho` |
| `\ee^{-i\theta}` | the exponential |
| `\emptysystem` | a trivial system |

An entropy with a symbol or a subscript of the document's own is a macro the document
declares with the same arguments and a template of its own.
