// The hand-written parts of `flm:stdcore/backends/latex`, spliced into the
// generated stub `api/stdcore/backends/latex.js` by `tests/api_surface.rs`:
// the classes of the payloads of the LaTeX family's constructs, which a rule
// gives to `lw.pres` and `lw.pres_slots`, each writing the payload's
// projection, which the adapter reads through the payload's
// `from_projection` (`adapter/payloads.rs`):
//
// - `new LatexMacroPayload(name, args)` with `packed()`: `{ name, args:
//   [shape], packed }`;
// - `new LatexEnvironmentPayload(name, args)` with `verbatim()`, `packed()`,
//   `body_argument()`: `{ name, args: [shape], verbatim, packed, body }`;
// - `new LatexRawPayload(text)`: `{ text }`;
// - `new LatexDefinitionPayload(kind, name, args)` with `of_feature`,
//   `with_source_name`, `packed()`, `robust()`, `body_argument()`: `{ kind,
//   name, source, args, packed, robust, body, feature }`, `kind` one of
//   `LatexDefinitionKind`.

// @hand export LatexDefinitionKind
// The kind of a definition written into the style file.
export const LatexDefinitionKind = Object.freeze({ Macro: "macro", Environment: "environment", Declaration: "declaration" });
// @hand export LatexMacroPayload
// The payload of a macro call: `new LatexMacroPayload(name, args)`, `args`
// a list of `LatexArgShape`s.
export class LatexMacroPayload {
  constructor(name, args) { this.name = name; this.args = args; this.packed_ = false; }
  packed() { this.packed_ = true; return this; }
  toJSON() { return { name: this.name, args: this.args, packed: this.packed_ }; }
}
// @hand export LatexEnvironmentPayload
// The payload of an environment: `new LatexEnvironmentPayload(name, args)`.
export class LatexEnvironmentPayload {
  constructor(name, args) {
    this.name = name;
    this.args = args;
    this.verbatim_ = false;
    this.packed_ = false;
    this.body_argument_ = false;
  }
  verbatim() { this.verbatim_ = true; return this; }
  packed() { this.packed_ = true; return this; }
  body_argument() { this.body_argument_ = true; return this; }
  toJSON() {
    return { name: this.name, args: this.args, verbatim: this.verbatim_, packed: this.packed_, body: this.body_argument_ };
  }
}
// @hand export LatexRawPayload
// The payload of raw LaTeX text written byte for byte: `new LatexRawPayload(text)`.
export class LatexRawPayload {
  constructor(text) { this.text = text; }
  toJSON() { return { text: this.text }; }
}
// @hand export LatexDefinitionPayload
// The payload of a definition written into the style file:
// `new LatexDefinitionPayload(kind, name, args)`, `args` the number of
// arguments.
export class LatexDefinitionPayload {
  constructor(kind, name, args) {
    this.kind = kind;
    this.name = name;
    this.source_name = name;
    this.args = args;
    this.packed_ = false;
    this.robust_ = false;
    this.body_argument_ = false;
    this.feature = null;
  }
  of_feature(feature) { this.feature = feature; return this; }
  with_source_name(name) { this.source_name = name; return this; }
  packed() { this.packed_ = true; return this; }
  robust() { this.robust_ = true; return this; }
  body_argument() { this.body_argument_ = true; return this; }
  toJSON() {
    return {
      kind: this.kind, name: this.name, source: this.source_name, args: this.args, packed: this.packed_, robust: this.robust_,
      body: this.body_argument_, feature: this.feature,
    };
  }
}
