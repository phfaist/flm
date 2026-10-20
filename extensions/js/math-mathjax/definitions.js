// The declarations of the math-mathjax extension, read from MathJax's own
// command maps when the extension registers: every macro and environment the
// configured TeX packages define, but for the few `read_commands` leaves out,
// and every command the feature's `preamble` defines that its setting
// `export_commands` lists, becomes an element declaration of the language, in
// math mode, by name alone (no arguments: MathJax reads them).
//
// Written against MathJax 4.1.3 (`vendor/bundle.js` names the version the
// bundle holds). The reading is `MathJax.names(tex)` of the bundle: the TeX
// input's handlers (`tex.parseOptions.handlers`), kind by kind, each a list of
// command maps in MathJax's order of lookup, each map a `Map` from name to
// definition. The `character` kind (patterns for digits, letters and `\`) is
// not read.
import { BodyDef, BodyLayout, ElementDef, Mode, SyntaxDef } from "flm:lang/mint";
import { ElementSpec } from "flm:model/element";
import { TEXT_COMMANDS } from "flm:stdcore/features/math";

// The element of every macro of MathJax's that the extension declares.
export const MACRO = "math-mathjax.macro";
// The element of every environment of MathJax's that the extension declares.
export const ENVIRONMENT = "math-mathjax.environment";

// The delimiters the extension defines beyond MathJax's packages, in math
// mode only, whatever the setting `packages` lists: a command's name (without
// its backslash) -> its character, `\llangle` (U+27EA, an opening delimiter)
// and `\rrangle` (U+27EB, a closing delimiter). Each is defined as MathJax's
// `base` package defines `\langle`, an entry of a delimiter map, so that
// MathJax reads it alone, after a size command (`\bigl\llangle`) and after
// `\left` or `\right`; whether it opens or closes is MathJax's own reading
// of the character.
export const DELIMITERS = Object.freeze({ llangle: "\u27EA", rrangle: "\u27EB" });

// The MathJax TeX package that holds `DELIMITERS`, the extension's own:
// `{ name, map, delimiters }`, its name, the name of its delimiter map, and
// the delimiters. The bundle defines it for every TeX input it configures
// (`MathJax.configure`), and the page's configuration defines it in the
// reader's browser (`page.js`).
export const OWN = Object.freeze({ name: "math-mathjax", map: "math-mathjax-delimiters", delimiters: DELIMITERS });

// The macros of MathJax's that are the language's own syntax, not commands:
// `\begin` and `\end` (an environment is written with them in the language).
const RESERVED = ["begin", "end"];

// The package recorded for a command that the feature's `preamble` defines.
export const PREAMBLE = "preamble";

// MathJax's maps of definitions: the command maps that the macros of
// `DEFINING` (`\DeclareMathOperator`, `\Newextarrow` and the others) write
// into, which the packages holding those macros create empty and which
// MathJax tries before every package's map. Once the preamble is processed,
// they hold what it defines.
const DEFINITION_MAPS = ["new-Command", "new-Delimiter", "new-Environment"];

// The commands of MathJax's that the extension never declares, whatever
// `packages` lists, so that inside a formula each is an unknown command: a
// formula must not depend on another formula, and the `math` feature owns
// equation numbering, labels and references. MathJax's TeX input still knows
// them (`tex.js` writes `\tag*` and `\notag` into the TeX it hands MathJax).
//
// The macros that define something for later formulas: an operator, an
// arrow, a paired delimiter, a boxed-alignment command, a color, a column
// type.
const DEFINING = [
  "DeclareMathOperator", "Newextarrow",
  "DeclarePairedDelimiter", "DeclarePairedDelimiterX", "DeclarePairedDelimiterXPP",
  "DeclarePairedDelimiters", "DeclarePairedDelimitersX", "DeclarePairedDelimitersXPP",
  "MakeAboxedCommand", "definecolor", "newcolumntype",
];
// The macros of equation numbering, labels and references, which the `math`
// feature owns.
const NUMBERING = [
  "label", "ref", "eqref", "refeq", "tag", "notag", "nonumber", "newtagform", "renewtagform", "usetagform",
];
// The macro that changes a setting for later formulas.
const SETTING = ["mathtoolsset"];
// The environments that are display-math structures, never written inside a
// formula: the `math` feature declares its display-math environments in text.
const DISPLAY_MATH = [
  "equation", "equation*", "eqnarray", "eqnarray*", "displaymath", "math", "indentalign",
  "align", "align*", "multline", "multline*", "gather", "gather*", "alignat", "alignat*",
  "xalignat", "xalignat*", "xxalignat", "flalign", "flalign*", "numcases", "subnumcases", "empheq",
];

// The commands the configured TeX input `tex` defines, read through the
// bundle `mathjax`, and those of them the extension declares: `{ macros,
// environments, defined }`, where `macros` and `environments` are the declared
// ones and `defined` (`{ macros, environments }`) every one MathJax defines,
// declared or not, each a `Map(name -> package)`, each name without its
// backslash, in MathJax's order of lookup. A name defined by several maps of
// packages is the first map's (the one MathJax finds). The maps of the
// `delimiter` kind are also macro maps: their names that begin with a
// backslash (`\langle`) are macros; the others (`(`, `|`) are characters, not
// commands, and are left out.
//
// A name of MathJax's maps of definitions (`DEFINITION_MAPS`) that no
// package's map holds is one the preamble defines: its package is `PREAMBLE`,
// and it is declared only when `exported` (the setting `export_commands`)
// lists it. A name `exported` lists that MathJax does not define is the
// registration's failure; one a package defines is declared, or left out, as
// if `exported` did not list it.
//
// Left out of the declared ones, by name: the twelve text commands of
// mathematics (`\text` and its relatives), which are the `math` feature's;
// `\begin` and `\end`, which are the language's syntax of an environment
// (`RESERVED`); the macros that define something or change a setting for later
// formulas, and those of equation numbering, labels and references
// (`DEFINING`, `NUMBERING`, `SETTING`); and the display-math environments
// (`DISPLAY_MATH`). `defined` gives the page's configuration the package of a
// name the extension does not declare (`page.js`).
export function read_commands(mathjax, tex, exported) {
  const maps = mathjax.names(tex);
  const delimiter_maps = new Set(maps.filter(({ kind }) => kind === "delimiter").map(({ map }) => map));
  const defined = { macros: new Map(), environments: new Map() };
  const in_definitions = { macros: [], environments: [] };
  for (const { kind, map, package: pkg, names } of maps) {
    const part = kind === "environment" ? "environments" : "macros";
    for (const written of names) {
      let name = written;
      if (kind !== "environment" && written.length > 1 && written.startsWith("\\")) name = written.slice(1);
      else if (kind !== "environment" && delimiter_maps.has(map)) continue;
      if (DEFINITION_MAPS.includes(map)) in_definitions[part].push(name);
      else if (!defined[part].has(name)) defined[part].set(name, pkg ?? map);
    }
  }
  for (const part of ["macros", "environments"]) {
    for (const name of in_definitions[part]) if (!defined[part].has(name)) defined[part].set(name, PREAMBLE);
  }
  for (const name of exported) {
    if (!defined.macros.has(name) && !defined.environments.has(name)) {
      throw new Error(`the command \`\\${name}\` of \`export_commands\` of the feature \`math-mathjax\` must be `
        + "a command that its `preamble` or one of its `packages` defines");
    }
  }
  const left_out_macros = new Set([...TEXT_COMMANDS, ...RESERVED, ...DEFINING, ...NUMBERING, ...SETTING]);
  const left_out_environments = new Set(DISPLAY_MATH);
  const declared = (all, left_out) => new Map([...all].filter(
    ([name, pkg]) => !left_out.has(name) && (pkg !== PREAMBLE || exported.includes(name))));
  return {
    macros: declared(defined.macros, left_out_macros),
    environments: declared(defined.environments, left_out_environments),
    defined,
  };
}

// Declares the commands `commands` (as `read_commands` answers them) on the
// builder `b`: each macro an element of `MACRO` with the macro syntax and no
// arguments, each environment an element of `ENVIRONMENT` with the
// environment syntax, no arguments and a body in math mode, both in math mode
// only, each recording its package in its element value (`{ package }`; the
// package of a delimiter of `DELIMITERS` is `OWN`'s name, that of a command
// the preamble defines `PREAMBLE`).
export function declare_commands(b, commands) {
  const owner = b.feature();
  for (const [name, pkg] of commands.macros) {
    const docs = pkg === OWN.name
      ? `\`\\${name}\`: a delimiter the math-mathjax extension defines, beyond MathJax's packages`
      : pkg === PREAMBLE
        ? `\`\\${name}\`: a command that the \`preamble\` of the feature \`math-mathjax\` defines`
        : `\`\\${name}\`: a command of MathJax's \`${pkg}\` package`;
    b.element(new ElementDef(
      owner, [name], SyntaxDef.Macro([]),
      new ElementSpec(MACRO).with_value({ package: pkg }).docs(docs),
    ).modes([Mode.Math]).docs(docs));
  }
  const body = new BodyDef().layout(BodyLayout.Inline).mode(Mode.Math);
  for (const [name, pkg] of commands.environments) {
    const docs = pkg === PREAMBLE
      ? `\`\\begin{${name}}\`: an environment that the \`preamble\` of the feature \`math-mathjax\` defines`
      : `\`\\begin{${name}}\`: an environment of MathJax's \`${pkg}\` package`;
    b.element(new ElementDef(
      owner, [name], SyntaxDef.Environment([], body),
      new ElementSpec(ENVIRONMENT).with_value({ package: pkg }).docs(docs),
    ).modes([Mode.Math]).docs(docs));
  }
}

