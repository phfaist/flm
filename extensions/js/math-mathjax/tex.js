// The TeX string of a formula, with its holes: what MathJax typesets, built
// from the formula's nodes. Both modes consume it: `load-in-browser` writes it
// into the page, `mathml` hands it to MathJax in the pipeline.
//
// A formula is a math group (`\(…\)`, `\[…\]`, the element `flm.math`) or a
// display-math environment (`flm.math.environment`). Its TeX is answered as
// `{ display, environment, parts, holes }`:
//
// - `display`: whether the formula is displayed; `environment`: the
//   environment's name, or `null` for a math group;
// - `parts`: the TeX in order, each part a string, `{ hole: i }` (the place
//   of the hole `holes[i]`), `NOTAG` (`\notag`, which a writer of the TeX
//   string writes and the text backend leaves out, the formula's source not
//   holding it), or `TAG_OPEN` and `TAG_CLOSE` (`\tag*{` and `}` around a
//   tag's hole, which a writer of the TeX string writes and the text backend
//   leaves out, writing only the tag's content after the line). A math
//   group's parts are its content, without
//   delimiters; an environment's parts are the whole environment, from
//   `\begin{name}` to `\end{name}`;
// - `holes`, numbered in document order, each one of:
//   - `{ kind: "text", command, node }`: the whole argument of a text command
//     (`\text{…}`, `\textbf{…}`, …; `command` is its name, `node` the text
//     command's node). The writer of a mode decides how the hole is written
//     (the command's own style is FLM's, never MathJax's);
//   - `{ kind: "tag", frame }`: what an equation line shows at its tag (its
//     number or the author's tag), standing between `TAG_OPEN` and
//     `TAG_CLOSE`;
//   - `{ kind: "anchor", anchor, frame }`: the anchor of an equation line, at
//     the line's start.
//
// The nodes are written as follows: a node of MathJax's (a macro, an
// environment) and a character run as their source as written; a group as
// its delimiters around its content; `\ensuremath{…}` (the `math` feature's
// `flm.math.ensuremath`) as its argument's content alone, the command left
// out, since inside a formula it stands for its argument; a text command as
// a hole; a comment as
// one space (the comment takes in its line end, so that dropping it would run
// the tokens on either side together: `\alpha%c` then `b` on the next line
// is `\alpha b`, not `\alphab`). Where the TeX written so far ends with a
// control word (a backslash and letters, `\rangle`) and the next text begins
// with a letter, one space stands between them, which TeX skips after a
// control word: the two come from places the source did not write side by
// side (the end of a definition's expansion and the letter after the
// definition's use; `\alpha` and the argument of `\ensuremath{b}`), and the
// letter would otherwise continue the control word (`\rangleA`, `\alphab`).
// An environment of the `math` feature is
// rebuilt from its equation lines and their frames: per line its anchor, its
// content, then `\tag*{<hole>}` (the parts `TAG_OPEN`, the hole, `TAG_CLOSE`)
// where the line shows a number or a tag and `\notag` where it
// shows nothing (MathJax's own numbering is off, so the starred form adds no
// parentheses of its own), then its line break with its spacing as written.
// The labels and `\nonumber` are the `math` feature's data and are not
// written. A line's content is written under the `math` library's newline
// rule (`normalize_line_newlines`), so that a `\label`, `\tag` or
// `\nonumber` taken off a line of its own leaves no blank line, which LaTeX
// refuses inside mathematics: each text part of the content is normalized,
// the run of whitespace that ends the content is written after the line's
// tag and its line break, and the run that begins the next line's content is
// then dropped. A comment, written as a space, does not end a line.
import { ENVIRONMENT, TEXT, ENSUREMATH, formula_display, normalize_line_newlines } from "flm:stdcore/features/math";

// The part standing for `\notag` at the end of a line that shows nothing.
export const NOTAG = Object.freeze({ notag: true });
// The parts standing for `\tag*{` and `}` around the hole of a line's tag.
export const TAG_OPEN = Object.freeze({ tag_open: true });
export const TAG_CLOSE = Object.freeze({ tag_close: true });

// Whether `tex` ends with a control word: a run of letters preceded by an odd
// number of backslashes (`\rangle`; not `\\abc`, which is `\\` then `abc`).
function ends_with_control_word(tex) {
  let start = tex.length;
  while (start > 0 && is_letter(tex[start - 1])) start -= 1;
  if (start === tex.length) return false;
  let backslashes = 0;
  while (start - backslashes > 0 && tex[start - backslashes - 1] === "\\") backslashes += 1;
  return backslashes % 2 === 1;
}

// Whether the character `ch` is one of TeX's letters.
function is_letter(ch) {
  return (ch >= "a" && ch <= "z") || (ch >= "A" && ch <= "Z");
}

// Collects the parts and the holes of one formula.
class Builder {
  constructor() { this.parts = []; this.holes = []; }
  text(text) {
    if (text === "") return;
    const last = this.parts.length - 1;
    if (last >= 0 && typeof this.parts[last] === "string") {
      const previous = this.parts[last];
      const space = is_letter(text[0]) && ends_with_control_word(previous) ? " " : "";
      this.parts[last] = previous + space + text;
    } else {
      this.parts.push(text);
    }
  }
  hole(hole) { this.parts.push({ hole: this.holes.length }); this.holes.push(hole); }
  // The parts of the nodes `nodes` in a list of their own (their holes are
  // this builder's), for the caller to add.
  apart(nodes) {
    const parts = this.parts;
    this.parts = [];
    this.nodes(nodes);
    const written = this.parts;
    this.parts = parts;
    return written;
  }
  // Writes the nodes `nodes` (a list of `Node`).
  nodes(nodes) {
    for (const node of nodes) this.node(node);
  }
  node(node) {
    if (node.is_comment()) {
      this.text(" ");
      return;
    }
    if (node.element_name() === TEXT) {
      this.hole({ kind: "text", command: node.name(), node });
      return;
    }
    if (node.element_name() === ENSUREMATH) {
      this.nodes(node.argument_content_nodes(0) ?? []);
      return;
    }
    const delimiters = node.group_delimiters();
    if (delimiters !== null) {
      this.text(delimiters[0]);
      this.nodes(node.children());
      this.text(delimiters[1]);
      return;
    }
    if (node.is_list()) {
      this.nodes(node.children());
      return;
    }
    const body = node.body();
    if (body !== null) {
      this.text(`\\begin{${node.name()}}`);
      this.nodes(body);
      this.text(`\\end{${node.name()}}`);
      return;
    }
    this.text(node.span_content());
  }
}

// The TeX of the formula `node`; `frames` are the frames of its lines when it
// is a display-math environment (`line_frames`, read by the caller with the
// tables it declared), ignored for a math group.
export function formula_tex(node, frames) {
  const built = new Builder();
  if (node.element_name() !== ENVIRONMENT) {
    built.nodes(node.children());
    return { display: formula_display(node), environment: null, parts: built.parts, holes: built.holes };
  }
  const name = node.name();
  built.text(`\\begin{${name}}`);
  // Whether the output written so far ends a line: the previous line's
  // content ended with a run of whitespace, written after its line break.
  let after_line_end = false;
  for (const frame of frames) {
    if (frame.anchor !== null) built.hole({ kind: "anchor", anchor: frame.anchor, frame });
    // The builder joins adjacent strings, so the content's parts alternate
    // strings and holes; a hole is not a line's end.
    const content = built.apart(frame.line.content);
    let trailing = "";
    content.forEach((part, index) => {
      if (typeof part !== "string") {
        built.parts.push(part);
        after_line_end = false;
        return;
      }
      const line = normalize_line_newlines(part, after_line_end, index < content.length - 1);
      built.text(line.text);
      trailing = line.trailing;
    });
    if (frame.shows.kind === "nothing") {
      built.parts.push(NOTAG);
    } else {
      built.parts.push(TAG_OPEN);
      built.hole({ kind: "tag", frame });
      built.parts.push(TAG_CLOSE);
    }
    const linebreak = frame.line.linebreak;
    if (linebreak !== null) built.text(linebreak.spacing === null ? "\\\\" : `\\\\[${linebreak.spacing}]`);
    built.text(trailing);
    after_line_end = trailing !== "";
  }
  built.text(`\\end{${name}}`);
  return { display: true, environment: name, parts: built.parts, holes: built.holes };
}

// The parts of `tex` joined into one string, each hole written by
// `write(hole, index)`.
export function tex_string(tex, write) {
  const written = (part) => {
    if (typeof part === "string") return part;
    if (part === NOTAG) return "\\notag";
    if (part === TAG_OPEN) return "\\tag*{";
    if (part === TAG_CLOSE) return "}";
    return write(tex.holes[part.hole], part.hole);
  };
  return tex.parts.map(written).join("");
}
