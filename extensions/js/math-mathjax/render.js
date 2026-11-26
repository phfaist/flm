// The render stage of `mathml` mode: MathJax run in the pipeline, once per
// formula, and its MathML written into a table for the html rules
// (`lower.js`) to turn into the page's elements.
//
// The stage `math-mathjax.render` (`Effect.Compute`, `Scope.PerTree`) walks
// the tree of its call (`cx.my_tree()`: a document's tree, or a synthesized
// value during value catch-up, so that a formula copied into a table of
// contents or a reference's text gets a row of its own) and, for every math
// group and display-math environment the tree holds (inside arguments and key
// values too), builds the formula's TeX string (`tex.js`), converts it with the
// one TeX input configured when the feature registers, through a fresh
// document object, and writes one row into the table `math-mathjax.rendered`,
// keyed by the formula node's address (`cx.addr_of(node)`):
//
//   { addr, mathml, holes, display, environment }
//
// - `mathml`: the formula's MathML as a tree, each element `{ tag,
//   attributes: [[name, value], …], children }` and each text a string, as
//   MathJax's lite parser reads the serialization back; the MathML carries
//   TeX's spacing between the parts of the formula, as MathJax computes it
//   for its own typesetting (an `mspace` before each part MathJax spaces, and
//   `lspace` and `rspace` on every `mo`, `0` unless MathJax's TeX input set
//   them: `MathJax.convert`, the repository's `npm/math-mathjax/entry.mjs`),
//   so that the browser adds no spacing of its own; the `data-latex`
//   attributes MathJax adds (the source a second time) and the whitespace the
//   serialization puts between elements are dropped;
// - `holes`: the formula's holes in order (`tex.js`), each `{ kind, command }`
//   (`command` for a text hole, else `null`); hole `i` is written into the TeX
//   as the private-use character `U+E000 + i` (a text hole as `\text{…}`
//   around it, a tag hole as the character alone inside `\tag*{…}`, an anchor
//   hole as nothing), which MathJax carries into an `mtext` element;
// - `display`: whether the formula is displayed; `environment`: the
//   environment's name, or `null` for a math group.
//
// A TeX error (MathJax's `formatError` throws) is the Error
// `math-mathjax.tex-error` at the formula, with MathJax's message, and no row;
// any other failure (a limit of the engine) fails the stage as any handler
// failure does, the rows written before it staying.
//
// The stage asks the cache `typeset.mathml` of the host service `cache` for
// each formula before it converts it, and stores what `MathJax.convert`
// answers, before `cleaned` drops what the page does not carry, as the
// value's record. The key is the list of: the version of MathJax the bundle
// holds, the settings that configure the TeX input (`packages`, `preamble`,
// and `export_commands`, the commands of the preamble that a formula may
// hold), whether the formula is displayed, and the TeX string given to
// MathJax (`typeset_key`). The other settings change nothing that
// `MathJax.convert` answers, and the row's other members come from the tree.
// A formula MathJax cannot typeset is not stored, so it is converted again,
// with its error, at every build.
//
// For a tree that has formulas, the stage declares on the build's progress
// sheet one row of kind `math-mathjax.formulas` ("Rendering formulas"),
// counting the formulas, and advances it at each formula, found in the cache,
// converted, or not converted.
import { StageDecl, Needs, Effect, Scope } from "flm:core/stage";
import { Unit } from "flm:progress";
import { MATH, ENVIRONMENT, ANCHORS, COUNTERS, RENDERED, line_frames } from "flm:stdlib/features/math";
import { formula_tex, tex_string } from "./tex.js";

// The stage, its table and its diagnostic.
export const RENDER_STAGE = "math-mathjax.render";
export const RENDERED_TABLE = "math-mathjax.rendered";
export const TEX_ERROR = "math-mathjax.tex-error";

// The kind of the progress row that counts the formulas of a tree, and its
// title.
const FORMULAS_KIND = "math-mathjax.formulas";
const FORMULAS_TITLE = "Rendering formulas";

// The capabilities of the expansion of user definitions, after which the
// stage runs when the plan holds it, so that MathJax sees expanded content.
const EXPANDED = "flm.userdefs.expanded";
const ALWAYS_EXPANDED = "flm.userdefs.always-expanded";

// The cache of the MathML that MathJax answers, which the package declares
// (`main.js`), with its declaration.
export const TYPESET_CACHE = "typeset.mathml";
export const TYPESET_DECLARATION = Object.freeze({
  version: 1,
  values: { engine: "sqlite" },
  unused_s: 90 * 24 * 60 * 60,
  cost_ms: 20,
  scope: "shared",
});

// The host service that keeps it.
export const CACHE_SERVICE = "cache";

// The key of the formula whose TeX string is `input`, displayed or not, under
// the configuration `config`: everything that `MathJax.convert` answers
// depends on, beside the extension's own package (`OWN` of `definitions.js`),
// which changes only with the version of the cache's declaration.
function typeset_key(mathjax, config, display, input) {
  return [mathjax.VERSION, config.packages, config.preamble, config.export_commands, display, input];
}

// The first private-use character: the marker of hole 0.
export const FIRST_MARKER = 0xE000;

// The marker of hole `index`.
export function marker(index) {
  return String.fromCodePoint(FIRST_MARKER + index);
}

// The hole `index` of a formula, as MathJax's input writes it.
function hole_tex(hole, index) {
  if (hole.kind === "text") return `\\text{${marker(index)}}`;
  if (hole.kind === "tag") return marker(index);
  return "";
}

// The MathML elements whose text is content (MathML's token elements): the
// whitespace inside them is the formula's, never the serialization's.
const TOKENS = new Set(["mi", "mn", "mo", "mtext", "ms", "mspace"]);

// The tree `tree` (as `MathJax.convert` answers it) without the `data-latex`
// attributes and without the whitespace the serialization writes between the
// children of an element that is not a token element.
function cleaned(tree) {
  if (typeof tree === "string") return tree;
  const token = TOKENS.has(tree.tag);
  return {
    tag: tree.tag,
    attributes: tree.attributes.filter(([name]) => name !== "data-latex"),
    children: tree.children
      .filter((child) => token || typeof child !== "string" || child.trim() !== "")
      .map(cleaned),
  };
}

// Registers the table and the stage, in `mathml` mode. `mathjax` is the
// bundle's object, `jax` the TeX input configured when the feature registers
// (with the TeX packages the feature's configuration lists and the
// extension's own delimiters, `OWN` of `definitions.js`): this one TeX input
// converts every formula, through a fresh document object each. `config` is
// the feature's configuration, as `main.js` reads it.
export function register_render(b, mathjax, jax, config) {
  b.table_refining(RENDERED_TABLE, RENDERED,
    "The MathML of each formula, as MathJax converted it, keyed by the formula's address.");
  b.stage({
    decl() {
      return new StageDecl(RENDER_STAGE, Effect.Compute, Scope.PerTree).writes(RENDERED_TABLE).docs(
        "Converts every formula of the tree to MathML with MathJax, and writes the MathML into "
        + "`math-mathjax.rendered`, with the places where the formula's text and tags go.");
    },
    needs() {
      return new Needs().reads_opt(ANCHORS).reads_opt(COUNTERS).after(ALWAYS_EXPANDED).after(EXPANDED);
    },
    run(cx, out) {
      const tree = cx.my_tree();
      if (tree === null) return;
      // The formulas are counted with a first traversal, since the row's
      // total is fixed when it starts, and converted with a second one.
      const formulas = () => tree.root().descendants().elements([MATH, ENVIRONMENT]);
      let count = 0;
      for (const _ of formulas()) count += 1;
      if (count === 0) return;
      const cache = cx.host(CACHE_SERVICE);
      const progress = cx.progress().child(FORMULAS_KIND, FORMULAS_TITLE, null).start_counted(Unit.Items, count);
      for (const node of formulas()) {
        const element = node.element_name();
        const frames = element === ENVIRONMENT ? line_frames(cx, true, true, node) : null;
        const tex = formula_tex(node, frames);
        const input = tex_string(tex, hole_tex);
        const key = typeset_key(mathjax, config, tex.display, input);
        const kept = cache.call("get", { cache: TYPESET_CACHE, key });
        let mathml;
        if (kept !== null) {
          mathml = kept.record;
        } else {
          try {
            mathml = mathjax.convert(jax, input, tex.display);
          } catch (error) {
            if (!(error instanceof mathjax.TexError)) throw error;
            out.diags({
              id: TEX_ERROR,
              severity: "error",
              message: `MathJax cannot typeset the formula: ${error.message}`,
              at: node,
            });
            progress.advance(1);
            continue;
          }
          cache.call("put", { cache: TYPESET_CACHE, key, record: mathml });
        }
        out.table(RENDERED_TABLE).push({
          addr: cx.addr_of(node),
          mathml: cleaned(mathml),
          holes: tex.holes.map((hole) => ({ kind: hole.kind, command: hole.kind === "text" ? hole.command : null })),
          display: tex.display,
          environment: tex.environment,
        });
        progress.advance(1);
      }
      progress.finish();
    },
  });
}
