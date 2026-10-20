// The hand-written part of `flm:stdcore/backends/latex/packages`, spliced
// into the generated stub `api/stdcore/backends/latex/packages.js` by
// `tests/api_surface.rs`: the class of a package request a feature
// contributes to the slot `flm.latex.packages`, which writes the data form
// the adapter reads (`adapter/slots.rs`), every field present: `{ id,
// package, options: [text], docs }`.

// @hand export LatexPackageDef
// A package request: `new LatexPackageDef(name, docs)`, its identifier
// `package_id(name)` until `id(id)` gives another (an extension's own
// request takes an identifier beginning with its stem), and `options(list)`.
export class LatexPackageDef {
  constructor(name, docs) { this.id_ = package_id(name); this.package = name; this.options_ = []; this.docs = docs ?? ""; }
  id(id) { this.id_ = id; return this; }
  options(list) { this.options_ = list; return this; }
  toJSON() { return { id: this.id_, package: this.package, options: this.options_, docs: this.docs }; }
}
