// The hand-written parts of `flm:lang/rewrites/userdefs`, spliced into the
// generated stub `api/lang/rewrites/userdefs.js` by `tests/api_surface.rs`:
// the class of the `flm-core` type `Definition`, the class
// `DefinitionBindings` (whose constructor answers a `PlaceholderBindings`),
// `UserDefinition.element_def_with` (the protocol for expansion hooks), the
// data class `TemplateSpec`, the durable class `TemplateLanguage`, and
// `ValueBindings` (a `PlaceholderBindings` whose `with` answers the same
// instance). An import line here is copied into the stub.
import { BlockRole } from "flm:model/aspects";
import { Durable } from "flm:_runtime";

// @hand export Definition
// A user definition: `new Definition(kind, name, args)`, the static
// constructors `macro_`, `environment`, `specials`, and the builder methods.
// `args` is a list of `ArgDef`s or a string of argument codes.
export class Definition {
  constructor(kind, name, args) {
    this.kind = kind;
    this.name = name;
    this.args = args;
    this.content_ = null;
    this.modes_ = DefinitionModes.Both;
    this.defaults = {};
    this.argument_number_offset_ = 0;
    this.block_role_ = BlockRole.DEFAULT;
    this.expand_ = ExpandPolicy.PerPlan;
    this.overrides = false;
    this.docs_ = "";
    this.extra_placeholders = [];
  }
  static macro_(name, args) { return new Definition(DefinitionKind.Macro, name, args); }
  static environment(name, args) { return new Definition(DefinitionKind.Environment, name, args); }
  static specials(trigger, args) { return new Definition(DefinitionKind.Specials, trigger, args); }
  content(template) { this.content_ = template; return this; }
  modes(modes) { this.modes_ = modes; return this; }
  text(template) { return this.content(template).modes(DefinitionModes.Text); }
  math(template) { return this.content(template).modes(DefinitionModes.Math); }
  default(key, template) { this.defaults[key] = template; return this; }
  argument_number_offset(offset) { this.argument_number_offset_ = offset; return this; }
  block_role(role) { this.block_role_ = role; return this; }
  expand(policy) { this.expand_ = policy; return this; }
  overriding() { this.overrides = true; return this; }
  docs(text) { this.docs_ = text; return this; }
  // Mirrors `Definition::extra_placeholder`: adds a placeholder name whose
  // content an expansion hook computes; the templates may write it with a
  // substitution argument when `substitution` is `true`.
  extra_placeholder(name, substitution) { this.extra_placeholders.push({ name, substitution }); return this; }
  toJSON() {
    return {
      kind: this.kind, name: this.name, args: this.args, content: this.content_, modes: this.modes_,
      defaults: this.defaults, argument_number_offset: this.argument_number_offset_, block_role: this.block_role_,
      expand: this.expand_, overrides: this.overrides, docs: this.docs_, extra_placeholders: this.extra_placeholders,
    };
  }
}
// @hand method UserDefinition.element_def_with
  // `hook` is a plain object `{ expand(node, cx, xp), policy() }`, recorded
  // under `element_name` and lent.
  element_def_with(owner, element_name, hook) {
    return call(this, "element_def_with", [owner, element_name, lend_expand(element_name, hook)]);
  }
// @hand export DefinitionBindings
// The bindings of a definition's arguments at a use site:
// `new DefinitionBindings(site, variant)` answers them, as an object of the
// class `PlaceholderBindings` (as Rust's `DefinitionBindings::new` answers a
// `Box<dyn PlaceholderBindings>`), so it is not `instanceof DefinitionBindings`.
export class DefinitionBindings extends PlaceholderBindings {
  constructor(site, variant) { return call(MODULE, "DefinitionBindings::new", [site, variant]); }
}
// @hand export TemplateSpec
// Mirrors `TemplateSpec`: what to compile, `new TemplateSpec({ text, mode,
// vocabulary, definition, label, origin_label, user_definition, offset })`,
// writing `{ text, mode, vocabulary: { arity, names?, body?, extras?: [{
// name, substitution }] }, definition, label, origin_label, user_definition,
// offset }`, with `user_definition` and `offset` filled with `false` and `0`
// when left out.
export class TemplateSpec {
  constructor({ text, mode, vocabulary, definition, label, origin_label, user_definition = false, offset = 0 } = {}) {
    this.text = text;
    this.mode = mode;
    this.vocabulary = vocabulary;
    this.definition = definition;
    this.label = label;
    this.origin_label = origin_label;
    this.user_definition = user_definition;
    this.offset = offset;
  }
  toJSON() {
    return {
      text: this.text, mode: this.mode, vocabulary: this.vocabulary, definition: this.definition, label: this.label,
      origin_label: this.origin_label, user_definition: this.user_definition, offset: this.offset,
    };
  }
}
// @hand export TemplateLanguage
// Mirrors `TemplateLanguage`: the template language of the running handler's
// feature: `new TemplateLanguage(lang)`, `lang` the built language a rule, a
// hook or a late-initialization hook was lent. A durable object: the adapter
// builds the language once per feature and registry and answers the same
// token at every later call; the object crosses as `{"@durable": { kind,
// table, index }}`.
export class TemplateLanguage extends Durable {
  constructor(lang) { return call(MODULE, "TemplateLanguage::new", [lang]); }
}
register_class("template_language", TemplateLanguage);
// @hand export ValueBindings
// Mirrors `ValueBindings`: bindings that give each extra placeholder
// (`#{name}`) of a template the content of a finished tree: `new
// ValueBindings()`, then `with(name, node)` for each placeholder, `node` the
// root node of a tree (a number segment's content, a synthesized tree).
// `with` answers the same instance, now holding the new binding.
export class ValueBindings extends PlaceholderBindings {
  constructor() { return call(MODULE, "ValueBindings::new", []); }
  with(name, node) {
    this._token = call(this, "with", [name, node])._token;
    return this;
  }
}
register_class("value_bindings", ValueBindings);
