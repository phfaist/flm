// The hand-written parts of `flm:core/registry`, spliced into the generated
// stub `api/core/registry.js` by `tests/api_surface.rs` (each marker's text
// up to the next marker): the builder's `feature` (answered in a rehearsal
// without calling Rust), its `stage` and `rewrite_stage` (the protocol for
// stage objects), its `contribute_to`, `contribute_first_to` and `peer_slot`
// (the protocol for handler objects: a citation provider or a template
// module is recorded by its name before the call, so that a rehearsal finds
// it and a template module's `describe` finds it while `register` runs), its
// `attach`, `late_init` and `finalizer` (the protocol for handler objects:
// recorded in every run of `register`, lent and passed on only when the
// builder dispatches), the class `BackendDef` (which hands its stage objects to FLM by the same
// protocol), and the classes of the `flm-core` types `StyleDef`, `ScriptDef`
// and `OutputDef`. An import line here is copied into the stub; the stub
// imports the common names of `flm:_runtime` itself.
//
// The shapes, in argument order:
//
//   b.stage({ decl(), needs(), run(cx, out) })            `needs` optional
//   b.rewrite_stage({ decl(), needs(), restage(node, cx, rs) })
//                                                          `needs` optional; `decl()` a `RewriteDecl`
//   new BackendDef(name, native, lowering, emit, final_stage).docs(text)
//                                                          each stage a stage object or a `LibraryStage`
//                                                          (`new LoweringStage(backend)`)
//   new OutputDef(name, capability, table, media_type, extension).made_from(output).docs(text)
//   b.table_named(name, StandardTable.ArtifactSet, docs)
//   b.presentation_element(name, docs)
//   b.contribute_to(slot, value), b.contribute_first_to(slot, value), b.peer_slot(slot, value)
//                                                          `value` data, a citation provider or a template module;
//                                                          data given to a standard data-valued slot
//                                                          (`flm.numbering.counters`, …) is that slot's data form
//   b.attach(element, aspect)                              `aspect` e.g. `Aspect.Sectioning({ level(node) })`
//   b.late_init({ late_init(lang, diags) }), b.finalizer({ finalize(b) })
import { Needs } from "flm:core/stage";
import { record_rewrite, lend_rewrite, rehearsing, contributed } from "flm:_runtime";
import { aspect_for, record_late_init, record_finalizer, lend_late_init, lend_finalizer } from "flm:_runtime";

// @hand method FeatureBuilder.feature
  // The registering feature's name (in a rehearsal, without calling Rust).
  feature() { return this._token === null ? registering_feature() : call(this, "feature", []); }
// @hand method FeatureBuilder.stage
  // A stage object `{ decl(), needs(), run(cx, out) }`; `needs` may be left out.
  stage(stage) {
    const decl = stage.decl();
    const needs = typeof stage.needs === "function" ? stage.needs() : new Needs();
    record_stage(decl.name, stage);
    if (this._token !== null) return call(this, "stage", [lend_stage(decl, needs)]);
  }
// @hand method FeatureBuilder.rewrite_stage
  // A rewrite stage object `{ decl(), needs(), restage(node, cx, rs) }`;
  // `needs` may be left out. Recorded under its stage name, so that the
  // engine instance of the thread that rewrites finds it again.
  rewrite_stage(stage) {
    const decl = stage.decl();
    const needs = typeof stage.needs === "function" ? stage.needs() : new Needs();
    record_rewrite(decl.name, stage);
    if (this._token !== null) return call(this, "rewrite_stage", [lend_rewrite(decl, needs)]);
  }
// @hand export BackendDef
// A backend: `new BackendDef(name, native, lowering, emit, final_stage)`,
// then `.docs(text)`. Each of the three stages is a stage object `{ decl(),
// needs(), run(cx, out) }`, as `b.stage` takes, or a stage of the library
// (`new LoweringStage(backend)`, an object of the class `LibraryStage`). The
// constructor records each stage object under its name and lends it, as
// `b.stage` does, so it is called while a feature's `register` runs.
export class BackendDef {
  constructor(name, native, lowering, emit, final_stage) {
    this.name = name;
    this.native = native;
    this.lowering = backend_stage(lowering);
    this.emit = backend_stage(emit);
    this.final_stage = backend_stage(final_stage);
    this.docs_ = "";
  }
  // Mirrors `BackendDef::docs`: sets the documentation prose.
  docs(docs) { this.docs_ = docs; return this; }
  toJSON() { return { name: this.name, native: this.native, lowering: this.lowering, emit: this.emit, final_stage: this.final_stage, docs: this.docs_ }; }
}

// One stage of a `BackendDef`: a stage object recorded and lent (kept as it
// is in a rehearsal, whose builder lends nothing); anything else (a
// `LibraryStage`) as it is.
function backend_stage(stage) {
  if (stage === null || typeof stage !== "object" || typeof stage.decl !== "function") return stage;
  const decl = stage.decl();
  const needs = typeof stage.needs === "function" ? stage.needs() : new Needs();
  record_stage(decl.name, stage);
  return rehearsing() ? stage : lend_stage(decl, needs);
}
// @hand export OutputDef
// An output: `new OutputDef(name, capability, table, media_type, extension)`,
// then `.made_from(output)` (once per output whose files the output is made
// from) and `.docs(text)`, as Rust's `OutputDef::new`, `made_from` and `docs`.
export class OutputDef {
  constructor(name, capability, table, media_type, extension) {
    this.name = name;
    this.capability = capability;
    this.table = table;
    this.media_type = media_type;
    this.extension = extension;
    this.made_from_ = [];
    this.docs_ = "";
  }
  made_from(output) { this.made_from_.push(output); return this; }
  docs(docs) { this.docs_ = docs; return this; }
  toJSON() {
    return { name: this.name, capability: this.capability, table: this.table, media_type: this.media_type, extension: this.extension, made_from: this.made_from_, docs: this.docs_ };
  }
}
// @hand method FeatureBuilder.contribute_to
  // Mirrors `FeatureBuilder::contribute_to`: contributes `value` (data, a citation provider or a
  // template module) to the service slot `slot`, after every value contributed to it so far.
  contribute_to(slot, value) {
    const sent = contributed(slot, value, this._token === null);
    if (this._token !== null) return call(this, "contribute_to", [slot, sent]);
  }
// @hand method FeatureBuilder.contribute_first_to
  // Mirrors `FeatureBuilder::contribute_first_to`: contributes `value` (data) to the service slot
  // `slot`, before every value appended to it.
  contribute_first_to(slot, value) {
    const sent = contributed(slot, value, this._token === null);
    if (this._token !== null) return call(this, "contribute_first_to", [slot, sent]);
  }
// @hand method FeatureBuilder.peer_slot
  // Mirrors `FeatureBuilder::peer_slot`: contributes `value` (data, a citation provider or a
  // template module) to the service slot `slot`, which another feature is expected to declare.
  peer_slot(slot, value) {
    const sent = contributed(slot, value, this._token === null);
    if (this._token !== null) return call(this, "peer_slot", [slot, sent]);
  }
// @hand method FeatureBuilder.attach
  // Attaches `aspect` to the element `element`, which another feature may
  // declare. A handler object in it (`{ Expand: … }`, `{ Referenceable: … }`,
  // `{ Sectioning: … }`) is recorded under `element`, and lent.
  attach(element, aspect) {
    const dispatching = this._token !== null;
    const given = aspect_for(element, aspect, dispatching);
    if (dispatching) return call(this, "attach", [element, given]);
  }
// @hand method FeatureBuilder.late_init
  // A late-initialization hook `{ late_init(lang, diags) }`, recorded by its
  // position among the feature's hooks, and lent.
  late_init(hook) {
    const index = record_late_init(hook);
    if (this._token !== null) return call(this, "late_init", [lend_late_init(index)]);
  }
// @hand method FeatureBuilder.finalizer
  // A finalizer `{ finalize(b) }`, recorded by its position among the
  // feature's finalizers, and lent; it is called in the build's second round
  // with the feature's builder.
  finalizer(hook) {
    const index = record_finalizer(hook);
    if (this._token !== null) return call(this, "finalizer", [lend_finalizer(index)]);
  }
// @hand export StyleDef
// A style definition: `new StyleDef(id, content, docs)`, `content` a list of
// `DefContent` parts written in order.
export class StyleDef {
  constructor(id, content, docs) { this.id = id; this.content = content; this.docs = docs ?? ""; }
  toJSON() { return { id: this.id, content: this.content, docs: this.docs }; }
}
// @hand export ScriptDef
// A script definition: `new ScriptDef(id, content, docs)`, as `StyleDef`.
export class ScriptDef {
  constructor(id, content, docs) { this.id = id; this.content = content; this.docs = docs ?? ""; }
  toJSON() { return { id: this.id, content: this.content, docs: this.docs }; }
}
