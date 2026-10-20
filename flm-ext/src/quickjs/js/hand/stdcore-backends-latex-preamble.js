// The hand-written part of `flm:stdcore/backends/latex/preamble`, spliced into
// the generated stub `api/stdcore/backends/latex/preamble.js` by
// `tests/api_surface.rs`: the class of a definition piece a feature
// contributes to the slot `flm.latex.preamble`, which writes the data form
// the adapter reads (`adapter/slots.rs`): `{ name, content, docs, requires:
// [id] }`.

// @hand export LatexPreambleDef
// A definition piece of the style file: `new LatexPreambleDef(name, content,
// docs)` and `requires(ids)`, the identifiers of the pieces and packages it
// needs.
export class LatexPreambleDef {
  constructor(name, content, docs) { this.name = name; this.content = content; this.docs = docs ?? ""; this.requires_ = []; }
  requires(ids) { this.requires_ = ids; return this; }
  toJSON() { return { name: this.name, content: this.content, docs: this.docs, requires: this.requires_ }; }
}
