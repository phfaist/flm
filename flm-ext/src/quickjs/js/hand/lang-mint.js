// The hand-written parts of `flm:lang/mint`, spliced into the generated stub
// `api/lang/mint.js` by `tests/api_surface.rs`: the classes of the `flm-core`
// types `ArgDef` and `BodyDef`, the function `arg_def_from_code` (which wraps
// its answer in an `ArgDef`), and the dual class `ElementDef` (a data form
// with fields, or a helper-made form holding a handle).

// @hand export ArgDef
// An argument: `ArgDef.mandatory(name)`, `ArgDef.optional(name)`, or
// `arg_def_from_code(code)`; written as the map `{ code, name, layout, docs,
// mode, scoped, role }`. `new ArgDef(code)` takes an argument code where Rust's
// `ArgDef::new` takes a parser, which data cannot carry.
export class ArgDef {
  constructor(code) {
    this.code = code;
    this.name = null;
    this.layout_ = ArgLayout.Inline;
    this.docs_ = "";
    this.mode_ = null;
    this.scoped_ = [];
    this.role_ = null;
  }
  static named(name, code) { const arg = new ArgDef(code); arg.name = name; return arg; }
  static mandatory(name) { return ArgDef.named(name, "{"); }
  static optional(name) { return ArgDef.named(name, "["); }
  layout(layout) { this.layout_ = layout; return this; }
  blocks() { return this.layout(ArgLayout.Blocks); }
  docs(text) { this.docs_ = text; return this; }
  // Mirrors `ArgDef::mode`: the argument is parsed in the mode `mode`.
  mode(mode) { this.mode_ = mode; return this; }
  // Mirrors `ArgDef::scoped`: the commands of the scoped package `name` are
  // legal inside the argument.
  scoped(name) { this.scoped_.push(name); return this; }
  // Mirrors `ArgDef::role`: the argument's role, the registered name of what
  // it holds (`flm.refs.target`).
  role(name) { this.role_ = name; return this; }
  toJSON() {
    return { code: this.code, name: this.name, layout: this.layout_, docs: this.docs_, mode: this.mode_, scoped: this.scoped_, role: this.role_ };
  }
}
// @hand export arg_def_from_code
// The argument the code `code` stands for (`"{"`, `"["`, `"*"`, `"r()"`, …).
export function arg_def_from_code(code) {
  const data = call(MODULE, "arg_def_from_code", [code]);
  const arg = new ArgDef(data.code).layout(data.layout).docs(data.docs);
  arg.name = data.name;
  arg.mode_ = data.mode ?? null;
  arg.scoped_ = data.scoped ?? [];
  arg.role_ = data.role ?? null;
  return arg;
}
// @hand export BodyDef
// An environment's body: `new BodyDef()` and the builder methods.
export class BodyDef {
  constructor() { this.layout_ = BodyLayout.FromRole; this.verbatim_ = false; this.mode_ = null; this.scoped_ = []; }
  layout(layout) { this.layout_ = layout; return this; }
  blocks() { return this.layout(BodyLayout.Blocks); }
  verbatim() { this.verbatim_ = true; return this; }
  mode(mode) { this.mode_ = mode; return this; }
  // Mirrors `BodyDef::scoped`: the commands of the scoped package `name` are
  // legal inside the body.
  scoped(name) { this.scoped_.push(name); return this; }
  toJSON() { return { layout: this.layout_, verbatim: this.verbatim_, mode: this.mode_, scoped: this.scoped_ }; }
}
// @hand export ElementDef
// An element declaration: `new ElementDef(owner, names, syntax, element)`
// and the builder methods. A declaration a user definition's helper made
// (`ud.element_def(…)`) holds a token instead of fields: its accessors answer
// `null`, and `docs`/`modes` are asked of Rust.
export class ElementDef extends Handle {
  constructor(owner, names, syntax, element) {
    super(null);
    this.owner_ = owner;
    this.names_ = names;
    this.syntax_ = syntax;
    this.element_ = element;
    this.docs_ = "";
    this.modes_ = null;
  }
  docs(text) {
    if (this._token !== null) { this._token = call(this, "docs", [text])._token; return this; }
    this.docs_ = text;
    return this;
  }
  modes(list) {
    if (this._token !== null) { this._token = call(this, "modes", [list])._token; return this; }
    this.modes_ = list;
    return this;
  }
  owner() { return this._token === null ? this.owner_ : null; }
  names() { return this._token === null ? this.names_ : null; }
  syntax() { return this._token === null ? this.syntax_ : null; }
  element() { return this._token === null ? this.element_ : null; }
  toJSON() {
    if (this._token !== null) return super.toJSON();
    return { owner: this.owner_, names: this.names_, syntax: this.syntax_, element: this.element_, docs: this.docs_, modes: this.modes_ };
  }
}
register_class("element_def", ElementDef);
