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
import { StageDecl, Needs, Effect, Scope } from "flm:core/stage";
import { MATH, ENVIRONMENT, ANCHORS, COUNTERS, RENDERED, line_frames } from "flm:stdcore/features/math";
import { formula_tex, tex_string } from "./tex.js";

// The stage, its table and its diagnostic.
export const RENDER_STAGE = "math-mathjax.render";
export const RENDERED_TABLE = "math-mathjax.rendered";
export const TEX_ERROR = "math-mathjax.tex-error";

// The capabilities of the expansion of user definitions, after which the
// stage runs when the plan holds it, so that MathJax sees expanded content.
const EXPANDED = "flm.userdefs.expanded";
const ALWAYS_EXPANDED = "flm.userdefs.always-expanded";

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
// converts every formula, through a fresh document object each.
export function register_render(b, mathjax, jax) {
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
      for (const node of tree.descendants()) {
        const element = node.element_name();
        if (element !== MATH && element !== ENVIRONMENT) continue;
        const frames = element === ENVIRONMENT ? line_frames(cx, true, true, node) : null;
        const tex = formula_tex(node, frames);
        let mathml;
        try {
          mathml = mathjax.convert(jax, tex_string(tex, hole_tex), tex.display);
        } catch (error) {
          if (!(error instanceof mathjax.TexError)) throw error;
          out.diags({
            id: TEX_ERROR,
            severity: "error",
            message: `MathJax cannot typeset the formula: ${error.message}`,
            at: node,
          });
          continue;
        }
        out.table(RENDERED_TABLE).push({
          addr: cx.addr_of(node),
          mathml: cleaned(mathml),
          holes: tex.holes.map((hole) => ({ kind: hole.kind, command: hole.kind === "text" ? hole.command : null })),
          display: tex.display,
          environment: tex.environment,
        });
      }
    },
  });
}
