// The hand-written parts of `flm:model/aspects`, spliced into the generated
// stub `api/model/aspects.js` by `tests/api_surface.rs`: the data a
// `Referenceable` or a `Sectioning` handler answers, each writing the data
// form the adapter reads (`adapter/aspects.rs`):
//
// - `new RefPoint({ kind, labels, computed, malformed, anchor, number,
//   content, accepts })` writes `{ kind, labels: [{ ref_type, ref_label }],
//   computed, malformed: [{ text, span: { start, end }, position }], anchor,
//   number, content, accepts }`, filling `labels: []`, `malformed: []`,
//   `computed: 0`, `number: null`, `content: null`, `accepts: "any"`;
// - `new NumberSource({ counter, style, advance, instances, number_format })`
//   writes `{ counter, instances: [node], style, advance: [level],
//   number_format }`, filling `instances: []`, `number_format: null`;
// - `AnchorPolicy`: `"from_first_label"`, `{ explicit: id }`, `{ slug: {
//   prefix, content } }`, `{ from_value: { prefix } }`, `{ auto: { prefix } }`;
// - `ContentRef`: `{ argument: i }`, `{ argument_named: name }`, `"body"`,
//   `{ node: node }`, `{ synthesized: node }` (a node handle each);
// - `AcceptedRefTypes`: `"any"` or `{ these: [type] }`.

// @hand export RefPoint
// A reference point of an occurrence: `new RefPoint({ kind, labels, computed,
// malformed, anchor, number, content, accepts })`, the fields left out
// filled with their defaults.
export class RefPoint {
  constructor({ kind, labels = [], computed = 0, malformed = [], anchor, number = null, content = null, accepts = "any" } = {}) {
    this.kind = kind;
    this.labels = labels;
    this.computed = computed;
    this.malformed = malformed;
    this.anchor = anchor;
    this.number = number;
    this.content = content;
    this.accepts = accepts;
  }
  toJSON() {
    return {
      kind: this.kind, labels: this.labels, computed: this.computed, malformed: this.malformed, anchor: this.anchor,
      number: this.number, content: this.content, accepts: this.accepts,
    };
  }
}
// @hand export NumberSource
// Where a reference point's number comes from: `new NumberSource({ counter,
// style, advance, instances, number_format })`, `instances` (nodes of the
// point's tree) and `number_format` (a number template, or `null` for the
// style's) filled with `[]` and `null` when left out.
export class NumberSource {
  constructor({ counter, style, advance, instances = [], number_format = null } = {}) {
    this.counter = counter;
    this.instances = instances;
    this.style = style;
    this.advance = advance;
    this.number_format = number_format;
  }
  toJSON() {
    return { counter: this.counter, instances: this.instances, style: this.style, advance: this.advance, number_format: this.number_format };
  }
}
// @hand export AnchorPolicy
// How the anchor of a reference point is formed, one constructor per variant.
export const AnchorPolicy = Object.freeze({
  Explicit(id) { return { explicit: id }; },
  FromFirstLabel: "from_first_label",
  Slug({ prefix, content }) { return { slug: { prefix, content } }; },
  FromValue({ prefix }) { return { from_value: { prefix } }; },
  Auto({ prefix }) { return { auto: { prefix } }; },
});
// @hand export ContentRef
// Content a reference point names, one constructor per variant.
export const ContentRef = Object.freeze({
  Argument(position) { return { argument: position }; },
  ArgumentNamed(name) { return { argument_named: name }; },
  Body: "body",
  Node(node) { return { node }; },
  Synthesized(root) { return { synthesized: root }; },
});
// @hand export AcceptedRefTypes
// The label types a reference point accepts: any, or these.
export const AcceptedRefTypes = Object.freeze({
  Any: "any",
  these(types) { return { these: types }; },
});
