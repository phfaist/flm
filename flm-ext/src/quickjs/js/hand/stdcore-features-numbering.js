// The hand-written parts of `flm:stdcore/features/numbering`, spliced into the
// generated stub `api/stdcore/features/numbering.js` by `tests/api_surface.rs`:
// the classes of the declarations a feature contributes to the numbering
// slots, each writing the data form the adapter reads (`adapter/slots.rs`),
// and the options of a number's formatting:
//
// - `new CounterDecl(name, default_style)` with `docs`, `within`, `nesting`:
//   `{ name, default_style, docs, within, nesting }` (`flm.numbering.counters`);
// - `new CounterStyleDecl(name)`, its fields set by assignment:
//   `{ name, number_format, sub_levels, prefix_words, delimiters, join,
//   name_in_link, repeat_parents_in_range, ranges, number_writer }`, a field
//   left `null` taking the constructor's default (`flm.numbering.styles`);
// - `new ParentSetDecl(name, counters)`: `{ name, counters }`
//   (`flm.numbering.parent-sets`);
// - `Within.parent(name)`, `Within.parents([name])`, `Within.parent_set(name)`
//   and the builder methods: `{ parents: { one } | { any } | { set }, joiner:
//   { joined } | { template } | "bare" | { named }, no_restart }`;
// - `new PluralWords(one, other)`: `{ one, other }`; `new PrefixWords(one,
//   other)`: `{ base: { one, other }, variants: { name: { one, other } } }`;
//   `new Delimiters(open, close)`: `{ open, close }`;
//   `NumberTemplate.formats(text)`: `{ formats }`, `.initial(text)`: `{ initial }`;
// - `new FormatOptions()` with `variant`, `without_word`, `without_delimiters`:
//   `{ variant, with_word, with_delimiters }`.

// @hand export CounterDecl
// A counter: `new CounterDecl(name, default_style)` and the builder methods.
export class CounterDecl {
  constructor(name, default_style) {
    this.name = name;
    this.default_style = default_style;
    this.docs_ = null;
    this.within_ = null;
    this.nesting_ = null;
  }
  docs(text) { this.docs_ = text; return this; }
  within(within) { this.within_ = within; return this; }
  nesting(spec) { this.nesting_ = spec; return this; }
  toJSON() {
    return { name: this.name, default_style: this.default_style, docs: this.docs_, within: this.within_, nesting: this.nesting_ };
  }
}
// @hand export CounterStyleDecl
// A counter style: `new CounterStyleDecl(name)`, each other field `null` (the
// default) until assigned.
export class CounterStyleDecl {
  constructor(name) {
    this.name = name;
    this.number_format = null;
    this.sub_levels = null;
    this.prefix_words = null;
    this.delimiters = null;
    this.join = null;
    this.name_in_link = null;
    this.repeat_parents_in_range = null;
    this.ranges = null;
    this.number_writer = null;
  }
  with_number_writer(name) { this.number_writer = name; return this; }
  toJSON() {
    return {
      name: this.name, number_format: this.number_format, sub_levels: this.sub_levels, prefix_words: this.prefix_words,
      delimiters: this.delimiters, join: this.join, name_in_link: this.name_in_link,
      repeat_parents_in_range: this.repeat_parents_in_range, ranges: this.ranges, number_writer: this.number_writer,
    };
  }
}
// @hand export ParentSetDecl
// A parent set: `new ParentSetDecl(name, counters)`.
export class ParentSetDecl {
  constructor(name, counters) { this.name = name; this.counters = counters; }
  toJSON() { return { name: this.name, counters: this.counters }; }
}
// @hand export Within
// The place of a counter within others: `Within.parent(name)`,
// `Within.parents(names)`, `Within.parent_set(name)`, and the builder methods
// (the joiner `.` and restarting by default).
export class Within {
  constructor(parents) { this.parents = parents; this.joiner = { joined: "." }; this.no_restart_ = false; }
  static parent(name) { return new Within({ one: name }); }
  static parents(names) { return new Within({ any: names }); }
  static parent_set(name) { return new Within({ set: name }); }
  no_restart() { this.no_restart_ = true; return this; }
  joined_by(text) { this.joiner = { joined: text }; return this; }
  template(text) { this.joiner = { template: text }; return this; }
  displayed_bare() { this.joiner = "bare"; return this; }
  joiner_named(name) { this.joiner = { named: name }; return this; }
  toJSON() { return { parents: this.parents, joiner: this.joiner, no_restart: this.no_restart_ }; }
}
// @hand export PluralWords
// A word in the singular and the plural: `new PluralWords(one, other)`.
export class PluralWords {
  constructor(one, other) { this.one = one; this.other = other; }
  toJSON() { return { one: this.one, other: this.other }; }
}
// @hand export PrefixWords
// The words written before a number: `new PrefixWords(one, other)` and
// `with_variant(name, words)`.
export class PrefixWords {
  constructor(one, other) { this.base = new PluralWords(one, other); this.variants = {}; }
  with_variant(name, words) { this.variants[name] = words; return this; }
  toJSON() { return { base: this.base, variants: this.variants }; }
}
// @hand export Delimiters
// The delimiters written around a number: `new Delimiters(open, close)`.
export class Delimiters {
  constructor(open, close) { this.open = open; this.close = close; }
  toJSON() { return { open: this.open, close: this.close }; }
}
// @hand export NumberTemplate
// A counter style's number template: `formats(text)` or `initial(text)`.
export const NumberTemplate = Object.freeze({
  formats(text) { return { formats: text }; },
  initial(text) { return { initial: text }; },
});
// @hand export FormatOptions
// The options of a number's formatting: `new FormatOptions()` (no variant,
// the word and the delimiters written) and the builder methods.
export class FormatOptions {
  constructor() { this.variant_ = null; this.with_word = true; this.with_delimiters = true; }
  variant(name) { this.variant_ = name; return this; }
  without_word() { this.with_word = false; return this; }
  without_delimiters() { this.with_delimiters = false; return this; }
  toJSON() { return { variant: this.variant_, with_word: this.with_word, with_delimiters: this.with_delimiters }; }
}
