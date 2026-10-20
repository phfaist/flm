// tests/js/typst/emit.js: the Typst emitter over the lowered trees: paragraphs,
// headings, the span styles, lists, links, verbatim, a formula's source inside
// Typst's math delimiters, the fixture's callout, and the `[unsupported: …]`
// stand-in for every other presentation element, as the text backend writes it.
//
// What it reads of the prelude: `cx.trees()` (the frozen trees, `[id, root]` in
// document order); `lowered.tree(id)` (a lowered tree's root, or null); and per
// node `element_name()`, `payload()`, `body()`, `children()`, `is_chars()`,
// `chars()`, `span_content()`, `slot_count()`, `slot_content_nodes(index)`.
import { PAR, BLOCK, BLOCK_HEADING, SPAN, LIST, VERBATIM, MATH_PASSTHROUGH, MATH_TEXT, LINK, UNSUPPORTED, DROPPED } from "flm:stdcore/backends/pres";
import { PARBREAK } from "flm:lang/structural";

const CALLOUT = "typst.pres.callout";

// Document text, with Typst's markup characters escaped.
function escape(text) { return text.replace(/[\\*_`#$@<>\[\]~]/g, c => "\\" + c); }

// The document: every tree of the run in document order, one blank line between two.
export function emit(cx, lowered) {
  const parts = [];
  for (const [id] of cx.trees()) {
    const root = lowered.tree(id);
    if (root !== null) parts.push(blocks(root.children()));
  }
  return parts.filter(text => text !== "").join("\n\n") + "\n";
}

// A run of blocks.
function blocks(nodes) { return nodes.map(block).filter(text => text !== "").join("\n\n"); }

// One block-shaped node.
function block(node) {
  const name = node.element_name();
  switch (name) {
    case PAR: return inline(node.body());
    case BLOCK: return blocks(node.body());
    case BLOCK_HEADING: return "=".repeat(Math.max(1, node.payload()?.level ?? 1)) + " " + inline(node.body());
    case LIST: return list(node);
    case VERBATIM:
      return node.payload()?.display === "block" ? "```\n" + raw(node.body()) + "\n```" : inline([node]);
    case MATH_PASSTHROUGH:
      return node.payload()?.display === "display" ? "$ " + source(node.body()) + " $" : inline([node]);
    case CALLOUT: return `#rect[*${escape(node.payload()?.kind ?? "note")}:* ${inline(node.body())}]`;
    default:
      if (!name.startsWith("flm.pres.")) return blocks(node.children());   // a structural container
      return inline([node]);
  }
}

// A list: a `label` and an `item` slot per entry; `-`, `+` for a numbered list,
// `/ term: item` for terms.
function list(node) {
  const kinds = node.payload()?.kinds ?? [];
  const marker = kinds.includes("numbered") ? "+" : kinds.includes("terms") ? "/" : "-";
  const lines = [];
  for (let i = 0; i + 1 < node.slot_count(); i += 2) {
    const label = inline(node.slot_content_nodes(i) ?? []);
    const item = blocks(node.slot_content_nodes(i + 1) ?? []).replace(/\n/g, "\n  ");
    lines.push(marker === "/" ? `/ ${label}: ${item}` : `${marker} ${item}`);
  }
  return lines.join("\n");
}

// An inline run.
function inline(nodes) { return nodes.map(span).join(""); }

// One inline node.
function span(node) {
  if (node.is_chars()) return escape(node.chars());
  const name = node.element_name();
  switch (name) {
    case SPAN: {
      const text = inline(node.body());
      switch (node.payload()?.style ?? null) {
        case "emph": case "italic": return `_${text}_`;
        case "bold": return `*${text}*`;
        case "monospace": return "`" + text + "`";
        case "underline": return `#underline[${text}]`;
        case "small-caps": return `#smallcaps[${text}]`;
        case "superscript": return `#super[${text}]`;
        case "subscript": return `#sub[${text}]`;
        default: return text;                       // a role, or no style: the content
      }
    }
    case LINK: return `#link("${node.payload()?.href ?? ""}")[${inline(node.body())}]`;
    case VERBATIM: return "`" + raw(node.body()) + "`";
    case MATH_PASSTHROUGH: return "$" + source(node.body()) + "$";
    case MATH_TEXT: return inline(node.body());
    case UNSUPPORTED: return `[unsupported: ${node.payload()?.element ?? "?"}]`;
    case DROPPED: return `[dropped: ${node.payload()?.detail ?? "?"}]`;
    case PARBREAK: return " ";
    default:
      if (!name.startsWith("flm.pres.")) return inline(node.children());
      return `[unsupported: ${name}]`;
  }
}

// Verbatim content, byte for byte; a formula's content as written.
function raw(nodes) { return nodes.map(node => node.is_chars() ? node.chars() : node.span_content()).join(""); }
function source(nodes) { return nodes.map(node => node.span_content()).join(""); }
