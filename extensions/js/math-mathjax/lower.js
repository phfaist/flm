// The lowering rules of the math-mathjax extension, by backend:
//
// | rule | element | backend | what it stages |
// |---|---|---|---|
// | `math-mathjax.formula.lower.html`, `math-mathjax.environment.lower.html` | `flm.math`, `flm.math.environment` | `html` | by mode: in `load-in-browser`, the formula's TeX in a span for MathJax to typeset in the browser (`register_browser_rules`); in `mathml`, the formula's MathML, read from the render stage's table (`render.js`) and converted to HTML constructs (`register_mathml_rules`, `mathml_construct`), with the stylesheet `math-mathjax.style` |
// | `math-mathjax.formula.lower.latex-recomposed`, `math-mathjax.environment.lower.latex-recomposed` | the two | `latex-recomposed` | the formula recomposed as written, through the `math` library and the mathematics writer |
// | `math-mathjax.formula.lower.latex-rendered`, `math-mathjax.environment.lower.latex-rendered` | the two | `latex-rendered` | the formula staged as `flm.pres.math-passthrough` over its source, each text command lowered through the rules, an environment's lines with their anchors and tags in `flm.pres.math-text` nodes |
// | `math-mathjax.formula.lower.text`, `math-mathjax.environment.lower.text` | the two | `text` | the formula's TeX as text, each text command lowered, each line's tag after the line, in a span or block |
// | `math-mathjax.macro.lower.source`, `math-mathjax.environment-command.lower.source` | `math-mathjax.macro`, `math-mathjax.environment` | every backend | the node's source as text (a command of MathJax's is written by the rule of the formula around it; this rule makes the element writable on its own) |
import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
import {
  MATH, ENVIRONMENT, ENSUREMATH, ANCHORS, COUNTERS, line_frames, stage_tag_content,
  group_opening, group_closing, environment_opening, stage_lines, environment_closing,
} from "flm:stdcore/features/math";
import { write_math_run } from "flm:stdcore/features/userdefs";
import { StyleDef, DefContent } from "flm:core/registry";
import { SPAN, BLOCK, MATH_TEXT, MATH_PASSTHROUGH } from "flm:stdcore/backends/pres";
import { TAG } from "flm:stdcore/backends/html";
import { formula_tex, NOTAG, TAG_OPEN, TAG_CLOSE } from "./tex.js";
import { RENDERED_TABLE, FIRST_MARKER } from "./render.js";
import { MACRO, ENVIRONMENT as COMMAND_ENVIRONMENT } from "./definitions.js";
import { SCRIPT, CONFIGURATION, FORMULA_CLASS } from "./page.js";

// The backends the extension serves.
export const HTML = "html";
export const TEXT = "text";
export const LATEX = "latex-recomposed";
export const RENDERED = "latex-rendered";

// The LaTeX family's constructs the recomposition stages.
const LATEX_RAW = "flm.pres.latex.raw";
const LATEX_MACRO = "flm.pres.latex.macro";
const LATEX_DEFINITION = "flm.pres.latex.definition";
// The references feature's table of safe labels.
const SAFE_LABELS = "flm.labels.safe";
// The option types the LaTeX parts of the `math` library read.
const LATEX_OPTIONS = ["MathRecomposeOptions", "LatexRecomposedRenderOptions"];

// The role (and kind) of a formula's span, and of an environment's block, on
// the text backend.
export const FORMULA_ROLE = "math-mathjax.formula";
export const ENVIRONMENT_ROLE = "math-mathjax.environment";

// The frames of the environment `node`'s lines, with its anchors and numbers
// when `tables`.
function frames_of(lw, node, tables) {
  return node.element_name() === ENVIRONMENT ? line_frames(lw, tables, tables, node) : null;
}

// The opening and closing delimiters of a math group written by the
// extension: `\(…\)` inline, `\[…\]` displayed; none around an environment,
// whose TeX holds `\begin` and `\end`.
function delimiters(tex) {
  if (tex.environment !== null) return ["", ""];
  return tex.display ? ["\\[", "\\]"] : ["\\(", "\\)"];
}

// `text` with every `<tex-html` and `</tex-html` broken by `{}` after its
// `<`, so that MathJax does not read the author's characters as its own tag.
function neutralized(text) {
  return text.replace(/<(\/?tex-html)/g, "<{}$1");
}

// A `<tex-html>` construct holding `children` (pres_tree children).
function tex_html(children) {
  return { element: TAG, payload: { tag: "tex-html" }, children };
}

// Whether `node` is a formula: a math group or a display-math environment.
function is_formula(node) {
  const element = node.element_name();
  return element === MATH || element === ENVIRONMENT;
}

// The formula that `node`, a node of a text command's argument, stands for:
// `node` itself when it is a formula; the inline formula of `\ensuremath{…}`
// written in text (its argument's content, a math group); otherwise `null`.
function formula_of(node) {
  if (is_formula(node)) return node;
  if (node.element_name() !== ENSUREMATH) return null;
  return (node.argument_content_nodes(0) ?? []).find(is_formula) ?? null;
}

// The children of a formula's span in `load-in-browser` mode: its TeX as
// text, each hole a `<tex-html>` element holding the hole's lowered content.
function browser_children(tex, cx, lw) {
  const [open, close] = delimiters(tex);
  const children = [];
  const text = (string) => {
    if (string === "") return;
    const last = children[children.length - 1];
    if (last !== undefined && last.text !== undefined) last.text += string;
    else children.push({ text: string });
  };
  text(open);
  write_browser_parts(tex, cx, lw, children, text);
  text(close);
  return children;
}

// Writes the parts of `tex` into `children` (`text` appends characters), the
// holes as `<tex-html>` elements.
function write_browser_parts(tex, cx, lw, children, text) {
  for (const part of tex.parts) {
    if (typeof part === "string") { text(neutralized(part)); continue; }
    if (part === NOTAG) { text("\\notag"); continue; }
    if (part === TAG_OPEN) { text("\\tag*{"); continue; }
    if (part === TAG_CLOSE) { text("}"); continue; }
    const hole = tex.holes[part.hole];
    if (hole.kind === "text") {
      write_text_hole(hole, cx, lw, children, text);
    } else if (hole.kind === "tag") {
      // The tag's content stands in text mode inside `\tag*{…}`, where MathJax
      // reads a `<tex-html>` element only between dollar signs, as the HTML
      // backend writes it.
      text("$");
      children.push(tex_html(stage_tag_content(cx, lw, hole.frame).map((build) => ({ build }))));
      text("$");
    } else {
      children.push(tex_html([{ element: TAG, payload: { tag: "span", attributes: [["id", hole.anchor]] } }]));
    }
  }
}

// Writes a text command's hole. MathJax does not typeset the HTML it inserts
// for a `<tex-html>` element, so a formula standing in the command's argument
// is never written inside the hole: as the HTML backend's emitter does for a
// passthrough formula, the hole is closed before it, the formula's TeX is
// written into the surrounding formula's characters with no delimiters of its
// own (its holes written the same way), and a new hole opens after it, each
// part of the argument a `flm.pres.math-text` of the command; `\ensuremath{…}`
// standing in the argument is its formula (`formula_of`). An argument
// that holds no formula is lowered by the text command's own rule, in one
// hole. A formula deeper inside the argument (inside a styled command inside
// it) is lowered by its own rule inside the hole.
function write_text_hole(hole, cx, lw, children, text) {
  const content = hole.node.argument_content_nodes(0) ?? [];
  if (!content.some((node) => formula_of(node) !== null)) {
    children.push(tex_html(lw.lower_nodes(cx, [hole.node]).map((build) => ({ build }))));
    return;
  }
  let run = [];
  const flush = () => {
    if (run.length === 0) return;
    const staged = lw.lower_nodes(cx, run).map((build) => ({ build }));
    children.push(tex_html([{ element: MATH_TEXT, payload: { command: hole.command }, children: staged }]));
    run = [];
  };
  for (const node of content) {
    const formula = formula_of(node);
    if (formula === null) { run.push(node); continue; }
    flush();
    write_browser_parts(formula_tex(formula, frames_of(lw, formula, true)), cx, lw, children, text);
  }
  flush();
}

// The rules of the `html` backend in `load-in-browser` mode.
function register_browser_rules(b) {
  const body = {
    lower(node, cx, lw) {
      const tex = formula_tex(node, frames_of(lw, node, true));
      lw.require(SCRIPT);
      const attributes = [["class", FORMULA_CLASS], ["data-flm-display", tex.display ? "display" : "inline"]];
      if (tex.environment !== null) attributes.push(["data-flm-env", tex.environment]);
      return Lowering.Emit([lw.pres_tree(cx, {
        element: TAG, payload: { tag: "span", attributes }, children: browser_children(tex, cx, lw),
      })]);
    },
  };
  const decl = () => new LoweringDecl().produces(TAG).produces(MATH_TEXT).requires(CONFIGURATION);
  b.lowering_rule(new LoweringRule(
    "math-mathjax.formula.lower.html", MATH, decl(), body, DefaultMark.OnBackends([HTML]),
  ).docs("Inline or displayed mathematics lowers to its TeX, between `\\(…\\)` or `\\[…\\]`, as the text "
    + "of a span that MathJax typesets in the reader's browser; the argument of each text command is "
    + "document content inside a `<tex-html>` element."));
  b.lowering_rule(new LoweringRule(
    "math-mathjax.environment.lower.html", ENVIRONMENT, decl().reads_opt(ANCHORS).reads_opt(COUNTERS), body,
    DefaultMark.OnBackends([HTML]),
  ).docs("A display-math environment lowers to its TeX, rebuilt from its equation lines, as the text of a "
    + "span that MathJax typesets in the reader's browser; each line begins with its anchor and ends "
    + "with its number or tag inside `\\tag*{…}`, or with `\\notag`."));
}

// The stylesheet of `mathml` mode, which every formula lowered in that mode
// requires.
export const STYLE = "math-mathjax.style";
// The class of the cell holding an equation line's tag (or number), the last
// cell of its row (empty on a line without one).
export const TAG_CLASS = "math-mathjax-tag";
// The class of the empty cell that opens each equation line's row, as wide as
// the tag cell, so that the line's own cells stand in the middle.
export const TAG_SPACE_CLASS = "math-mathjax-tag-space";
// The class of the table whose rows are an environment's equation lines.
export const LINES_CLASS = "math-mathjax-lines";
// The font family the stylesheet declares for the `math_font_url` setting.
const MATH_FONT = "math-mathjax-font";
// The math fonts named when no `math_font_url` is set: the common installed
// ones, then the generic family.
const INSTALLED_MATH_FONTS = '"Latin Modern Math", "STIX Two Math", "Cambria Math", math';

// The text of the stylesheet `math-mathjax.style` for the setting
// `math_font_url` (`null` when absent).
export function stylesheet_text(math_font_url) {
  const font_face = math_font_url === null ? ""
    : `  @font-face {\n    font-family: "${MATH_FONT}";\n    src: url("${math_font_url.replace(/["\\]/g, "\\$&")}") format("woff2");\n  }\n`;
  const fonts = math_font_url === null ? INSTALLED_MATH_FONTS : `"${MATH_FONT}", ${INSTALLED_MATH_FONTS}`;
  return `/* ${STYLE} */
/* The formulas the math-mathjax extension writes as MathML: the math fonts,
   a displayed formula as a centered block, the table of an environment's
   equation lines at full width with each line in the middle (between an
   empty cell and the cell of its tag, each half of the width that remains)
   and the tag at the right edge, and the text inside a formula (the content
   of an mtext element) in the page's font. */
@layer flm {
${font_face}  math {
    font-family: ${fonts};
  }
  math[display="block"] {
    display: block math;
    margin: 0.5em 0;
    text-align: center;
  }
  mtable.${LINES_CLASS} {
    width: 100%;
  }
  mtd.${TAG_SPACE_CLASS},
  mtd.${TAG_CLASS} {
    width: 50%;
  }
  mtd.${TAG_CLASS} {
    text-align: right;
    white-space: nowrap;
  }
  math mtext {
    font-family: var(--flm-page-family, serif);
  }
}
`;
}

// The attributes MathML Core does not define, which `mathml_construct` maps
// to CSS.
const COLUMNALIGN = "columnalign";
const COLUMNSPACING = "columnspacing";
const ROWSPACING = "rowspacing";

// The value at `index` of a whitespace-separated list of values, the last
// one repeating (MathML's rule for its table attributes); `null` for an
// absent list.
function nth_value(list, index) {
  if (list === null) return null;
  const values = list.trim().split(/\s+/);
  return values[Math.min(index, values.length - 1)];
}

// Half the length `length` (`"3pt"` gives `"1.5pt"`), the share of a
// spacing on each side of the line between two rows or two columns.
function half(length) {
  const match = /^(-?[0-9]*\.?[0-9]+)([a-z%]*)$/.exec(length);
  return match === null ? `calc(${length} / 2)` : `${Number(match[1]) / 2}${match[2]}`;
}

// The attribute `name` of an element of the tree, or `null`.
function attribute(tree, name) {
  const found = tree.attributes.find(([key]) => key === name);
  return found === undefined ? null : found[1];
}

// `attributes` with `declarations` (CSS, `"name: value"`) added to its
// `style`.
function with_style(attributes, declarations) {
  if (declarations.length === 0) return attributes;
  const css = declarations.join("; ");
  const style = attributes.findIndex(([name]) => name === "style");
  if (style < 0) return [...attributes, ["style", css]];
  const copy = attributes.slice();
  copy[style] = ["style", `${copy[style][1].replace(/;\s*$/, "")}; ${css}`];
  return copy;
}

// Whether the tree `tree` is an element holding nothing but the marker of a
// hole: the hole's number, or `null`.
function marker_hole(tree, count) {
  if (tree.tag !== "mtext" || tree.children.length !== 1 || typeof tree.children[0] !== "string") return null;
  const text = tree.children[0];
  if ([...text].length !== 1) return null;
  const index = text.codePointAt(0) - FIRST_MARKER;
  return index >= 0 && index < count ? index : null;
}

// The table of a formula's MathML whose rows are its `lines` equation lines:
// the `mtable` that is the `math` element's only element and has `lines`
// rows, or `null` (a math group has no lines; a one-line environment without
// a tag has no such table, and a table it holds is the line's content unless
// it has exactly one row, a case that looks the same either way).
function lines_table(math, lines) {
  if (lines === 0) return null;
  const elements = math.children.filter((child) => typeof child !== "string");
  if (elements.length !== 1 || elements[0].tag !== "mtable") return null;
  const rows = elements[0].children.filter((child) => typeof child !== "string");
  return rows.length === lines ? elements[0] : null;
}

// `attributes` with the class `name` added.
function with_class(attributes, name) {
  const index = attributes.findIndex(([key]) => key === "class");
  if (index < 0) return [...attributes, ["class", name]];
  const copy = attributes.slice();
  copy[index] = ["class", `${copy[index][1]} ${name}`];
  return copy;
}

// The conversion of MathJax's MathML (a row's `mathml`) into the tree of
// construct nodes `lw.pres_tree` stages, under the HTML backend's construct
// `flm.pres.html.tag`:
//
// | MathJax's MathML | what is staged |
// |---|---|
// | an element | a `flm.pres.html.tag` of the same name, with its attributes (`math` with `display="block"` block-level) |
// | text | a text leaf |
// | an `mtext` holding nothing but the marker of hole `i` | an `mtext` with the same attributes over `holes[i]` (build identifiers) |
// | an `mlabeledtr` (a line with a tag; MathML Core has none) | an `mtr` with its first cell (the tag) moved to the end, that cell carrying the class `math-mathjax-tag` and losing MathJax's `id` |
// | the table of an environment's equation lines: the `mtable` that is the `math` element's only element and has one row per line (`anchors` has one entry per line) | an `mtable` with the class `math-mathjax-lines`; each row, padded with empty `mtd` cells up to the widest row's own cell count (its tag excluded), then opens with an empty cell of class `math-mathjax-tag-space`, and a row without a tag ends with an empty cell of class `math-mathjax-tag`, so that these two cells are the first and the last column of every row |
// | the i-th row (`mtr`, `mlabeledtr`) of that table | its `id` is `anchors[i]` where that is not `null`; a formula without that table and with one line puts `anchors[0]` on the `math` element |
// | `columnalign` (on an `mtable`, `mtr`, `mtd`) | `text-align` in the `style` of each cell of its column (a cell's own value first, then its row's, then its table's) |
// | `columnspacing` on an `mtable` | `padding-left` and `padding-right` in the `style` of each cell, half the spacing on each side of the line between two columns, none at the table's sides |
// | `rowspacing` on an `mtable` | `padding-top` and `padding-bottom` in the `style` of each cell, half the spacing on each side of the line between two rows, none at the table's top and bottom |
// | a `data-*` attribute | dropped |
// | anything else | passed through |
//
// The three table attributes are not written as attributes, since MathML Core
// does not define them; a table that gives a spacing replaces the browser's
// own padding of its cells on that axis.
export function mathml_construct(math, holes, anchors) {
  const table = lines_table(math, anchors.length);
  const lines = new Map();
  if (table !== null) {
    table.children.filter((child) => typeof child !== "string").forEach((row, index) => {
      if (index < anchors.length && anchors[index] !== null) lines.set(row, anchors[index]);
    });
  }
  const own_id = table === null && anchors.length === 1 ? anchors[0] : null;
  const kept = (attributes) => attributes.filter(([name]) =>
    !name.startsWith("data-") && name !== COLUMNALIGN && name !== COLUMNSPACING && name !== ROWSPACING);
  const tag = (name, attributes, children) => ({
    element: TAG,
    payload: { tag: name, attributes, block: name === "math" && attributes.some(([key, value]) => key === "display" && value === "block") },
    children,
  });
  const convert = (tree) => {
    if (typeof tree === "string") return { text: tree };
    const hole = marker_hole(tree, holes.length);
    if (hole !== null) return tag("mtext", kept(tree.attributes), (holes[hole] ?? []).map((build) => ({ build })));
    if (tree.tag === "mtable") return convert_table(tree);
    let attributes = kept(tree.attributes);
    if (tree === math && own_id !== null) attributes = [...attributes, ["id", own_id]];
    return tag(tree.tag, attributes, tree.children.map(convert));
  };
  const convert_table = (mtable) => {
    const align = attribute(mtable, COLUMNALIGN);
    const column_spacing = attribute(mtable, COLUMNSPACING);
    const row_spacing = attribute(mtable, ROWSPACING);
    const rows = mtable.children.filter((child) => typeof child !== "string");
    // A row's own cells, its tag excluded (an `mlabeledtr`'s first cell).
    const own_cells = (row) => {
      const cells = row.children.filter((child) => typeof child !== "string");
      return row.tag === "mlabeledtr" ? cells.slice(1) : cells;
    };
    // The lines table's rows are padded to the widest row's own cell count.
    const width = mtable === table ? Math.max(0, ...rows.map((row) => own_cells(row).length)) : 0;
    const converted = rows.map((row, row_index) => {
      const labeled = row.tag === "mlabeledtr";
      let cells = own_cells(row);
      while (cells.length < width) cells = [...cells, { tag: "mtd", attributes: [], children: [] }];
      const columns = cells.length;
      if (labeled) {
        const first = row.children.find((child) => typeof child !== "string");
        if (first !== undefined) cells = [...cells, first];
      }
      const row_align = attribute(row, COLUMNALIGN);
      const staged = cells.map((cell, column) => {
        const is_tag = labeled && column === columns;
        const css = [];
        if (!is_tag) {
          const cell_align = attribute(cell, COLUMNALIGN) ?? nth_value(row_align, column) ?? nth_value(align, column);
          if (cell_align !== null) css.push(`text-align: ${cell_align}`);
          if (column_spacing !== null) {
            css.push(`padding-left: ${column === 0 ? "0" : half(nth_value(column_spacing, column - 1))}`);
            css.push(`padding-right: ${column === columns - 1 ? "0" : half(nth_value(column_spacing, column))}`);
          }
        }
        if (row_spacing !== null) {
          css.push(`padding-top: ${row_index === 0 ? "0" : half(nth_value(row_spacing, row_index - 1))}`);
          css.push(`padding-bottom: ${row_index === rows.length - 1 ? "0" : half(nth_value(row_spacing, row_index))}`);
        }
        let attributes = kept(cell.attributes);
        if (is_tag) {
          attributes = [...attributes.filter(([name]) => name !== "id"), ["class", TAG_CLASS]];
        }
        return tag(cell.tag, with_style(attributes, css), cell.children.map(convert));
      });
      if (mtable === table) {
        staged.unshift(tag("mtd", [["class", TAG_SPACE_CLASS]], []));
        if (!labeled) staged.push(tag("mtd", [["class", TAG_CLASS]], []));
      }
      let attributes = kept(row.attributes);
      if (lines.has(row)) attributes = [...attributes.filter(([name]) => name !== "id"), ["id", lines.get(row)]];
      return tag("mtr", attributes, staged);
    });
    const attributes = kept(mtable.attributes);
    return tag("mtable", mtable === table ? with_class(attributes, LINES_CLASS) : attributes, converted);
  };
  return convert(math);
}

// The rules of the `html` backend in `mathml` mode, and their stylesheet
// (`settings`: the feature's configuration as read).
function register_mathml_rules(b, settings) {
  b.style_def(new StyleDef(STYLE, [DefContent.Inline(stylesheet_text(settings.math_font_url))],
    "The formulas written as MathML: the math fonts, the displayed formulas, the tags, the text inside a formula."));
  const body = {
    lower(node, cx, lw) {
      const frames = frames_of(lw, node, true);
      const tex = formula_tex(node, frames);
      lw.require(STYLE);
      const [row] = lw.get_at(RENDERED_TABLE, node) ?? [];
      let children;
      if (row === undefined) {
        children = [{ text: node.span_content() }];
      } else {
        const holes = tex.holes.map((hole) => {
          if (hole.kind === "text") return lw.lower_nodes(cx, [hole.node]);
          if (hole.kind === "tag") return stage_tag_content(cx, lw, hole.frame);
          return null;
        });
        const anchors = frames === null ? [] : frames.map((frame) => frame.anchor);
        children = [mathml_construct(row.mathml, holes, anchors)];
      }
      const tree = tex.environment === null
        ? { element: SPAN, payload: { role: FORMULA_ROLE, kinds: [tex.display ? "display" : "inline"] }, children }
        : { element: BLOCK, payload: { role: ENVIRONMENT_ROLE, kinds: [tex.environment] }, children };
      return Lowering.Emit([lw.pres_tree(cx, tree)]);
    },
  };
  const decl = () => new LoweringDecl().produces(TAG).reads(RENDERED_TABLE);
  b.lowering_rule(new LoweringRule(
    "math-mathjax.formula.lower.html", MATH, decl().produces(SPAN), body, DefaultMark.OnBackends([HTML]),
  ).docs("Inline or displayed mathematics lowers to the MathML MathJax made of it, in a span; the argument "
    + "of each text command is document content inside its `mtext` element. A formula MathJax could not "
    + "typeset lowers to its source as text."));
  b.lowering_rule(new LoweringRule(
    "math-mathjax.environment.lower.html", ENVIRONMENT,
    decl().produces(BLOCK).reads_opt(ANCHORS).reads_opt(COUNTERS), body, DefaultMark.OnBackends([HTML]),
  ).docs("A display-math environment lowers to the MathML MathJax made of it, in a block; each line's row "
    + "carries the line's anchor, and its last cell the line's number or tag."));
}

// The rules of the `text` backend: the formula's TeX as text inside a span
// (a math group) or a block (an environment), each text command written as
// its command around its lowered argument, each line's tag content after the
// line (without `\tag*{…}`, which is MathJax's input, not the source).
function register_text_rules(b) {
  const body = {
    lower(node, cx, lw) {
      const tex = formula_tex(node, frames_of(lw, node, true));
      const [open, close] = delimiters(tex);
      const children = [{ text: open }];
      for (const part of tex.parts) {
        if (typeof part === "string") { children.push({ text: part }); continue; }
        if (part === NOTAG || part === TAG_OPEN || part === TAG_CLOSE) continue;
        const hole = tex.holes[part.hole];
        if (hole.kind === "text") {
          children.push({ text: `\\${hole.command}{` });
          children.push(...lw.lower_nodes(cx, [hole.node]).map((build) => ({ build })));
          children.push({ text: "}" });
        } else if (hole.kind === "tag") {
          children.push(...stage_tag_content(cx, lw, hole.frame).map((build) => ({ build })));
        }
      }
      children.push({ text: close });
      const tree = tex.environment === null
        ? { element: SPAN, payload: { role: FORMULA_ROLE, kinds: [tex.display ? "display" : "inline"] }, children }
        : { element: BLOCK, payload: { role: ENVIRONMENT_ROLE, kinds: [tex.environment] }, children };
      return Lowering.Emit([lw.pres_tree(cx, tree)]);
    },
  };
  b.lowering_rule(new LoweringRule(
    "math-mathjax.formula.lower.text", MATH, new LoweringDecl().produces(SPAN), body, DefaultMark.OnBackends([TEXT]),
  ).docs("Inline or displayed mathematics lowers to its TeX as text inside a span, the argument of each "
    + "text command lowered as document content."));
  b.lowering_rule(new LoweringRule(
    "math-mathjax.environment.lower.text", ENVIRONMENT,
    new LoweringDecl().produces(BLOCK).reads_opt(ANCHORS).reads_opt(COUNTERS), body, DefaultMark.OnBackends([TEXT]),
  ).docs("A display-math environment lowers to its TeX as text inside a block, rebuilt from its equation "
    + "lines, each line's number or tag after the line."));
}

// The rules of the recomposed LaTeX: the formula as written, through the
// `math` library's parts and the mathematics writer; an environment's lines
// through the library's line writer (`stage_lines`), which writes each line's
// content under the library's newline rule.
function register_latex_rules(b) {
  const decl = () => {
    const decl = new LoweringDecl().produces(LATEX_RAW).produces(LATEX_MACRO).produces(LATEX_DEFINITION);
    for (const option of LATEX_OPTIONS) decl.option(option);
    return decl;
  };
  b.lowering_rule(new LoweringRule(
    "math-mathjax.formula.lower.latex-recomposed", MATH, decl(),
    {
      lower(node, cx, lw) {
        return Lowering.Emit([
          ...group_opening(cx, lw, node), ...write_math_run(cx, lw, node.children()), ...group_closing(cx, lw, node),
        ]);
      },
    },
    DefaultMark.OnBackends([LATEX]),
  ).docs("Inline or displayed mathematics lowers to its delimiters around its content written from its "
    + "nodes, as the author wrote it."));
  b.lowering_rule(new LoweringRule(
    "math-mathjax.environment.lower.latex-recomposed", ENVIRONMENT, decl().reads_opt(SAFE_LABELS),
    {
      lower(node, cx, lw) {
        return Lowering.Emit([
          ...environment_opening(cx, lw, node),
          ...stage_lines(cx, lw, line_frames(lw, false, false, node)),
          ...environment_closing(cx, lw, node),
        ]);
      },
    },
    DefaultMark.OnBackends([LATEX]),
  ).docs("A display-math environment lowers to the LaTeX environment of the same name over its equation "
    + "lines rebuilt one by one, each line's labels written at the end of the line."));
}

// The rules of the LaTeX for LuaLaTeX: the formula staged as a
// `flm.pres.math-passthrough` node over the formula's source, each text
// command lowered through the rules (its own rule stages a
// `flm.pres.math-text` node). An environment's node carries its name; per
// line, an empty `flm.pres.math-text` carrying the line's anchor, the line's
// source, `\tag*{…}` around a `flm.pres.math-text` marked `at_tag` holding the
// line's number or tag, and the line break. A line that shows nothing gets
// nothing: the backend writes the environment so that it numbers no line.
function register_rendered_rules(b) {
  const body = {
    lower(node, cx, lw) {
      const tex = formula_tex(node, frames_of(lw, node, true));
      const children = [];
      const parts = [...tex.parts];
      if (tex.environment !== null) {
        // The environment's name is the node's payload, which the backend
        // writes as `\begin{…}` and `\end{…}`: the two texts are taken off.
        const begin = `\\begin{${tex.environment}}`;
        const end = `\\end{${tex.environment}}`;
        parts[0] = parts[0].slice(begin.length);
        const last = parts.length - 1;
        parts[last] = parts[last].slice(0, parts[last].length - end.length);
      }
      for (let index = 0; index < parts.length; index += 1) {
        const part = parts[index];
        if (typeof part === "string") {
          if (part !== "") children.push({ text: part });
          continue;
        }
        if (part === NOTAG || part === TAG_OPEN || part === TAG_CLOSE) continue;
        const hole = tex.holes[part.hole];
        if (hole.kind === "text") {
          children.push(...lw.lower_nodes(cx, [hole.node]).map((build) => ({ build })));
        } else if (hole.kind === "anchor") {
          children.push({ element: MATH_TEXT, payload: { anchor: hole.anchor }, children: [] });
        } else if (hole.kind === "tag") {
          const content = stage_tag_content(cx, lw, hole.frame).map((build) => ({ build }));
          if (content.length > 0) {
            children.push({ text: "\\tag*{" });
            children.push({ element: MATH_TEXT, payload: { at_tag: true }, children: content });
            children.push({ text: "}" });
          }
        }
      }
      const payload = tex.environment === null
        ? { display: tex.display ? "display" : "inline" }
        : { display: "display", env: tex.environment };
      return Lowering.Emit([lw.pres_tree(cx, { element: MATH_PASSTHROUGH, payload, children })]);
    },
  };
  b.lowering_rule(new LoweringRule(
    "math-mathjax.formula.lower.latex-rendered", MATH, new LoweringDecl().produces(MATH_PASSTHROUGH), body,
    DefaultMark.OnBackends([RENDERED]),
  ).docs("Inline or displayed mathematics lowers to `flm.pres.math-passthrough` over its source, with the "
    + "argument of every text command inside it lowered as document content."));
  b.lowering_rule(new LoweringRule(
    "math-mathjax.environment.lower.latex-rendered", ENVIRONMENT,
    new LoweringDecl().produces(MATH_PASSTHROUGH).produces(MATH_TEXT).reads_opt(ANCHORS).reads_opt(COUNTERS),
    body, DefaultMark.OnBackends([RENDERED]),
  ).docs("A display-math environment lowers to a displayed `flm.pres.math-passthrough` that carries its "
    + "name, over its equation lines; each line begins with an empty `flm.pres.math-text` carrying its "
    + "anchor and ends with its tag (the author's tag or the line's number) and its line break; labels are "
    + "not written."));
}

// The rules of MathJax's own commands on every backend: the node's source.
// The rule of a kind is registered where the kind's element is declared, that
// is where the packages define at least one command of that kind (`commands`,
// as `read_commands` answers them; an empty `packages` defines none).
function register_command_rules(b, commands) {
  const body = { lower(node, cx, lw) { return Lowering.Emit([lw.text(cx, node.span_content())]); } };
  if (commands.macros.size > 0) b.lowering_rule(new LoweringRule(
    "math-mathjax.macro.lower.source", MACRO, new LoweringDecl(), body, DefaultMark.Everywhere,
  ).docs("A macro of MathJax's lowers to its source; the rule of the formula around it writes it otherwise."));
  if (commands.environments.size > 0) b.lowering_rule(new LoweringRule(
    "math-mathjax.environment-command.lower.source", COMMAND_ENVIRONMENT, new LoweringDecl(), body,
    DefaultMark.Everywhere,
  ).docs("An environment of MathJax's lowers to its source; the rule of the formula around it writes it "
    + "otherwise."));
}

// Declares the peers the rules read and registers the rules for the
// feature's configuration `settings` (as read; its `mode` chooses the html
// rules) and the commands `commands` (as `read_commands` answers them).
export function register_rules(b, settings, commands) {
  b.peer_backend(HTML, "the HTML backend, on which the extension's rules write the formulas");
  b.peer_backend(TEXT, "the text backend, on which the extension's rules write the formulas as text");
  b.peer_backend(LATEX, "the recomposed-LaTeX backend, on which the extension's rules recompose the formulas");
  b.peer_backend(RENDERED, "the LaTeX for LuaLaTeX, on which the extension's rules write the formulas as written");
  b.peer_table(ANCHORS, "the references feature's anchors, read for the anchor of each equation line");
  b.peer_table(COUNTERS, "the numbering feature's counters, read for the number of each equation line");
  b.peer_table(SAFE_LABELS, "the references feature's safe labels, read by the LaTeX recomposition of the lines");
  b.peer_element(LATEX_RAW, "the LaTeX family's raw construct, which the recomposition stages");
  b.peer_element(LATEX_MACRO, "the LaTeX family's macro construct, which the recomposition stages");
  b.peer_element(LATEX_DEFINITION, "the LaTeX family's definition construct, which the recomposition stages");
  b.peer_element(TAG, "the HTML backend's construct, which the HTML rules stage");
  if (settings.mode === "load-in-browser") register_browser_rules(b);
  else register_mathml_rules(b, settings);
  register_text_rules(b);
  register_latex_rules(b);
  register_rendered_rules(b);
  register_command_rules(b, commands);
}
