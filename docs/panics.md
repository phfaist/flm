# List of panicking items

This page lists every item of the public interface that can panic, states the
condition under which it panics, and links the item's documentation. It covers
every crate of this documentation: the library [`flm_core`](crate), the host
library `flm-main`, the command-line program `flm-cli`, the introspection library
`flm-info`, the extension layer `flm-ext`, the extension crate `flm-phfpack` and the
extension `flmdoc`. Every item listed belongs to `flm_core`, except the one item of
`flm-ext` in the last section; `flm-main`, `flm-cli`, `flm-info`, `flm-phfpack` and
`flmdoc` document no panicking item.

An item that can panic states the condition in a `# Panics` section of its own
documentation; this page restates that condition on the item's line. A change to the
panicking behavior of the public interface (a panic added, removed or changed)
updates this page in the same change.

## The panic policy

No document content can make this library panic. Every problem that markup can
cause — malformed syntax, an unknown command, nesting deeper than the
[recursion guard](crate::lang::guard) allows, a lowering rule that refuses a
node — is reported as a
[diagnostic](crate::guide::concepts_overview#diagnostics) or returned as a typed
error. A host (a program that embeds the library) renders documents it does
not control, so a panic there would crash the host instead of reporting a
problem with the document.

A public item panics only when its caller did not meet a requirement that the
item states in its `# Panics` section. These requirements fall into six groups:

- **Identifiers used with the wrong registry, layout document or text output.**
  A number that a [registry](crate::core::registry) assigned, a node identifier
  of a layout document ([`DocBuf`](crate::layout::doc::DocBuf)), or a position
  mark ([`TextMark`](crate::stdcore::backends::text::TextMark)) of a text output
  ([`TextOutput`](crate::stdcore::backends::text::TextOutput)) is meaningful only
  for the registry, layout document or text output that produced it. Used with
  another one, the identifier is either out of range, which panics, or it
  addresses the wrong entry.
- **Type-erased values paired with the wrong type.** A few methods meant for the
  library's internal use receive a value whose Rust type has been erased (a
  `dyn Any`) together with a description of the type that value must have:
  [`TableType`](crate::core::data::table::TableType)'s methods,
  [`OptionRef::digest`](crate::model::options::OptionRef::digest) and the typed
  reads of `dyn Lowerer`. Passing a value of another type is a programming error
  that the compiler cannot catch, because the type has been erased.
- **A panic in another component's code passing through.** The items that run a
  [stage](crate::core::stage::Stage), a
  [rewrite stage](crate::core::stage::RewriteStage) or a closure that the caller
  passes (in [`core::run`](crate::core::run), [`core::plan`](crate::core::plan)
  and [`lang::guard`](crate::lang::guard)) do not catch a panic. A stage that
  panics has a defect, and continuing the run over data left in an unknown
  condition would turn that defect into a wrong result instead of a clear
  failure.
- **Test helpers whose purpose is to fail a test.** The assertions in
  [`crate::dump`] panic on purpose, with a message describing what they found.
- **Capacity limits that no real document reaches.** Building a layout document
  of more than about four billion entries, or placing more than about two
  billion trees in a document, panics instead of letting the count wrap around.
- **Output written out of order, or into the wrong kind of element, through the
  [HTML sink](crate::stdcore::backends::html::HtmlSink).** The sink is the
  writer through which the HTML backend's emitter writes the bytes of a page. Closing an element the
  sink never opened, taking the output while an element is still open, or writing
  unescaped content into an element whose content a browser does not read as raw
  text is a mistake in the code that writes through the sink.

The lines marked *debug builds only* below describe a check that runs in a
debug build only: a `debug_assert!`, or for `TextSourceMap` the integer-overflow
check that a debug build performs on arithmetic. A release build does not check
the condition and goes on with the consequence stated on that line.

Items whose failure is a typed error or a diagnostic are not listed here, and
neither are private helpers.

## `model::addr`

- [`TreeId::tree`](crate::model::addr::TreeId::tree) — `index` is 2³¹ or greater.
  The highest bit of a [`TreeId`](crate::model::addr::TreeId) is the tag that
  tells a document tree from a
  [synthesized value](crate::guide::concepts_overview#synthesized-values).
- [`TreeId::value`](crate::model::addr::TreeId::value) — the same bound, for the
  same reason.

## `model::doc`

- [`DocContent::add_tree`](crate::model::doc::DocContent::add_tree) — the
  document already holds 2³¹ trees, the largest number of trees a
  [`TreeId`](crate::model::addr::TreeId) can tell apart from synthesized values.
- [`DocContent::place`](crate::model::doc::DocContent::place) — the same
  condition as `add_tree`.

## `model::element`

- [`ElementSpec::with`](crate::model::element::ElementSpec::with) — an
  implementation of the [aspect](crate::guide::concepts_overview#elements-and-aspects)
  type `A` is already attached to the element.
  Attaching one aspect twice while building an element is a mistake in the
  feature that builds it. Code that attaches aspects from several sources uses
  the fallible [`AspectMap::insert`](crate::model::element::AspectMap::insert)
  instead.

## `model::options`

- [`OptionRef::digest`](crate::model::options::OptionRef::digest) — `value` is
  not a value of the option type that this `OptionRef` (a declared read of one
  option type) refers to. The library always passes a value of the matching
  type.

## `core::data::table`

The three methods below take a table value whose type has been erased and
convert it back to the value type that the
[`TableType`](crate::core::data::table::TableType) describes. A caller working
through the store or through a typed key cannot cause these panics: the
registry checked the value type when the table was declared.

- [`TableType::absorb`](crate::core::data::table::TableType::absorb) — `acc` or
  `piece` is not a value of this table type's value type.
- [`TableType::project`](crate::core::data::table::TableType::project) — `table`
  is not a value of this table type's value type.
- [`TableType::piece_class`](crate::core::data::table::TableType::piece_class) —
  the same condition as `project`.

## `core::data::store`

- [`ComputedView::get`](crate::core::data::store::ComputedView::get) — the table
  installed under the key's name holds a value of a type other than the declared
  one. A stage cannot cause this, because the declared type is checked at this
  read and when the table is written, and
  [`Run::new`](crate::core::run::Run::new) checks the type of every external
  table's value before it installs it (a mismatch is refused with
  [`RunError::ExternalType`](crate::core::run::RunError::ExternalType)). The
  panic is reachable only through a
  [`Computed`](crate::core::data::store::Computed) that a caller filled directly
  with [`Computed::install`](crate::core::data::store::Computed::install), which
  does not check the value type.
- [`ComputedView::get_singleton`](crate::core::data::store::ComputedView::get_singleton)
  — the same condition as `get`.
- [`fold_outs`](crate::core::data::store::fold_outs) — the value of a
  [table piece](crate::core::data::table::PieceClass) (the part of one table
  that one stage invocation produced) is not of
  the value type its write declaration gives. Neither
  [`StageOut`](crate::core::data::store::StageOut) nor the store can produce
  such a table piece.

## `core::registry`

Each of these accessors reads an entry of a
[registry](crate::core::registry) by a number that the registry
assigned. A number that another registry assigned is either beyond this
registry's lists, which panics, or within them, in which case the accessor
returns the wrong entry. A number obtained from this registry, from one of its
entries, or from a plan built with it is always valid.

- [`FlmRegistry::capability`](crate::core::registry::FlmRegistry::capability) —
  the [`CapId`](crate::core::registry::CapId) is beyond this registry's
  capabilities.
- [`FlmRegistry::default_refinement`](crate::core::registry::FlmRegistry::default_refinement)
  — the [`CapId`](crate::core::registry::CapId) is not an identifier of this
  registry.
- [`FlmRegistry::table`](crate::core::registry::FlmRegistry::table) — the
  [`TableId`](crate::core::data::table::TableId) is not an identifier of this
  registry.
- [`FlmRegistry::stage`](crate::core::registry::FlmRegistry::stage) — the
  [`StageIdx`](crate::core::registry::StageIdx) is not an index of this
  registry.
- [`FlmRegistry::rule`](crate::core::registry::FlmRegistry::rule) — the
  [`RuleIdx`](crate::core::registry::RuleIdx) is not an index of this registry.

## `core::settings`

A [`SettingsDecl`](crate::core::settings::SettingsDecl) describes the settings
a feature reads; the feature's code builds it. A declaration that contradicts
itself is a mistake in that code, and the methods below panic on it.

- [`SettingsDecl::config`](crate::core::settings::SettingsDecl::config) and
  [`SettingsDecl::config_from`](crate::core::settings::SettingsDecl::config_from)
  — a field of the configuration type has the name of a key already declared,
  the declaration already has a configuration, or the type is not read from a
  map of named fields.
- [`SettingsDecl::options`](crate::core::settings::SettingsDecl::options) —
  `bind` is [`Bind::Registry`](crate::model::options::Bind::Registry) (a value
  fixed when the registry is built belongs in the feature's configuration), a
  field of the option type has the name of a key already declared, or the option
  type is not read from a map of named fields.
- [`SettingsDecl::merge_entries`](crate::core::settings::SettingsDecl::merge_entries)
  — `key` is a nested path rather than one settings key (it contains a `.` or a
  `*`), or
  `default_entries` fails or returns something other than a map.

## `core::plan`

- [`Plan::build`](crate::core::plan::Plan::build) — the
  [`Stage::needs`](crate::core::stage::Stage::needs) or
  [`RewriteStage::needs`](crate::core::stage::RewriteStage::needs) method of a
  registered stage panics. Building a plan calls that method on every registered
  stage, rewrite stages included, and the panic propagates out of this call.
- [`Plan::for_workflow`](crate::core::plan::Plan::for_workflow) — the same
  condition as [`Plan::build`](crate::core::plan::Plan::build).

## `core::run`

- [`Run::step`](crate::core::run::Run::step) — a stage or a rewrite stage of the
  batch of stages it runs panics, and the panic propagates out of this call. The whole
  batch is collected before anything is folded into the store, so when the panic
  propagates, the store is untouched and the run has not advanced.
- [`Run::execute`](crate::core::run::Run::execute) — the same condition as
  `step`, for any batch it runs.
- [`StageCx::resolve_value`](crate::core::run::StageCx::resolve_value) — a
  rewrite stage or another stage panics while it runs over the synthesized value
  during [value catch-up](crate::guide::concepts_overview#value-catch-up), and
  the panic propagates out of this call.
- [`StageCx::resolve_value_with`](crate::core::run::StageCx::resolve_value_with)
  — the same condition as `resolve_value`.

## `core::lowering`

Two typed reads of a [`Lowerer`](crate::core::lowering::Lowerer) convert the
value that a type-erased method of the trait returned. They are inherent methods
of `dyn Lowerer`, not methods of the trait, so no intra-doc link resolves to
them; they appear on the trait's page, under its implementations for
`dyn Lowerer`.

- `get`, the method `get` of `dyn Lowerer` in `flm_core::core::lowering` — the
  [`Lowerer`](crate::core::lowering::Lowerer) implementation's
  [`Lowerer::get_erased`](crate::core::lowering::Lowerer::get_erased) returns a
  value of a type other than the one the key designates. The implementation that
  this library ships cannot: the registry compares the table's value type with
  the key's before the read. Another implementation of the trait could.
- `option`, the method `option` of `dyn Lowerer` in `flm_core::core::lowering` —
  the implementation's
  [`Lowerer::option_erased`](crate::core::lowering::Lowerer::option_erased)
  returns a value that is not of the requested option type. The implementation
  that this library ships cannot: an option value is stored under its own type.

## `lang::ext`

- [`TextSourceMap::total_len`](crate::lang::TextSourceMap::total_len) — *debug
  builds only*: the lengths of the map's text source pieces sum to more than a
  32-bit unsigned integer holds. A map describes one text node's text, so this
  would take a text node of more than four gigabytes; a release build lets the
  sum wrap around.
- [`TextSourceMap::piece_at`](crate::lang::TextSourceMap::piece_at) — *debug
  builds only*: the same overflow condition as `total_len`.

## `lang::guard`

Neither of these functions raises a panic of its own; both let a panic of the
caller's closure through.

- [`run_with_worker_stack`](crate::lang::guard::run_with_worker_stack) — a panic
  inside the caller's closure is caught on the worker thread and resumed on the
  calling thread, so it reaches the caller as if the closure had run there.
- [`on_worker_thread`](crate::lang::guard::on_worker_thread) — a panic inside the
  caller's closure propagates to the caller, whichever thread the closure ran on.

## `lang::rewrites::userdefs::template`

Both functions parse argument specifications (the codes `o` and `m`) that are
constants of that module, and panic only if those constants fail to parse; no
document input can reach either panic. `Default::default` on either type calls
`new` and panics under the same condition.

- [`PlaceholderSpec::new`](crate::lang::rewrites::userdefs::template::PlaceholderSpec::new)
  — the placeholder's argument codes, which are constants of that module, fail
  to parse.
- [`PlaceholderProvider::new`](crate::lang::rewrites::userdefs::template::PlaceholderProvider::new)
  — the same condition, since it builds a
  [`PlaceholderSpec`](crate::lang::rewrites::userdefs::template::PlaceholderSpec).

## `stdcore::backends::html::sink`

The [HTML sink](crate::stdcore::backends::html::HtmlSink) is the writer through
which the HTML backend's emitter writes the bytes of a page. Code that writes through it
must close every element it opens, write unescaped content only into an element
whose content a browser reads as raw text, and check every tag and attribute
name before writing it
([`is_valid_tag_name`](crate::stdcore::backends::html::is_valid_tag_name),
[`is_valid_attribute_name`](crate::stdcore::backends::html::is_valid_attribute_name)).
No document input can reach any of the conditions below: a tag or attribute name
written by a lowering rule reaches the sink only through the
[HTML emitter](crate::stdcore::backends::html::emit), which checks it, reports a
name that fails as `flm.html.invalid-name`, and writes, in place of that node,
an empty element marked as unsupported (`<span class="flm-pres-unsupported">`).

- [`HtmlSink::close`](crate::stdcore::backends::html::HtmlSink::close) — no
  element is open.
- [`HtmlSink::finish`](crate::stdcore::backends::html::HtmlSink::finish) — an
  element is still open, so the HTML handed over would be incomplete; the panic
  message gives every element left open.
- [`HtmlSink::raw_text_element`](crate::stdcore::backends::html::HtmlSink::raw_text_element)
  — the tag is neither `style` nor `script`, the two elements whose content a
  browser reads as raw text. The content is written unescaped, so in another
  element a browser would read it as markup. This check runs in every build.
  *Debug builds only*, additionally: an attribute name fails the check above.
- [`HtmlSink::open_block`](crate::stdcore::backends::html::HtmlSink::open_block),
  [`HtmlSink::open_inline`](crate::stdcore::backends::html::HtmlSink::open_inline),
  [`HtmlSink::void_block`](crate::stdcore::backends::html::HtmlSink::void_block),
  [`HtmlSink::void_inline`](crate::stdcore::backends::html::HtmlSink::void_inline)
  — *debug builds only*: the tag name fails the check above. A release build
  writes the name unchanged, which may produce markup the caller did not intend.
- [`TagBuilder::attr`](crate::stdcore::backends::html::TagBuilder::attr),
  [`TagBuilder::attr_json`](crate::stdcore::backends::html::TagBuilder::attr_json),
  [`TagBuilder::flag`](crate::stdcore::backends::html::TagBuilder::flag) —
  *debug builds only*: the attribute name fails the check above, with the same
  consequence in a release build.

## `stdcore::backends::latex::preamble`

Both functions panic when the constant data of untechxt's builtin
Unicode-to-LaTeX table breaks an invariant: the chunks of its builtin profiles
([`PROFILES`](crate::untechxt::builtin::needs_profiles::PROFILES)), as
[`table_chunks`](crate::stdcore::backends::latex::packages::table_chunks) lists
them. The tests of that module check the invariant over the table as shipped;
no document input can reach either panic.

- [`chunk_pieces`](crate::stdcore::backends::latex::preamble::chunk_pieces) — a
  chunk of declarations (a chunk that loads no LaTeX package; see
  [`is_package_chunk`](crate::stdcore::backends::latex::packages::is_package_chunk))
  is not one block of declarations for every engine: it has more than one case
  (a variant of the chunk for a set of engines), or its one case does not apply
  to every engine listed in
  [`ENGINES`](crate::stdcore::backends::latex::packages::ENGINES); or the chunk
  declares no name (no math alphabet, math symbol, text symbol or symbol font)
  that the [definition piece](crate::stdcore::backends::latex::LatexPreambleDef)
  made from it can test before writing the declarations (the piece writes them
  only when no command of that name exists).
- [`standard_pieces`](crate::stdcore::backends::latex::preamble::standard_pieces)
  — the same condition, since it calls `chunk_pieces`.

## `stdcore::backends::lowering`

Most of these items read a lowering rule selection
([`SelectedRules`](crate::core::plan::SelectedRules): the rule a plan chose for
each element on one backend) together with a registry. The selection holds rule
numbers of the registry the plan was built against. Paired with another
registry, a rule number is out of range for that registry's list of rules, and
the item panics. A plan and the registry it was built from always match, so
only values from two registries paired by hand can reach this panic.

- [`lower_tree`](crate::stdcore::backends::lowering::lower_tree) —
  `inputs.selected` was built against a registry other than `inputs.registry`
  (the fields of [`LoweringInputs`](crate::stdcore::backends::lowering::LoweringInputs)).
- [`lower_value`](crate::stdcore::backends::lowering::lower_value) — the same
  condition as `lower_tree`. A panic of a rewrite stage, or of a stage run again
  during [value catch-up](crate::guide::concepts_overview#value-catch-up), while
  the synthesized value given to this call is brought up to date also propagates
  out of this call, as out of [`StageCx::resolve_value`](crate::core::run::StageCx::resolve_value).
- [`rank`](crate::stdcore::backends::lowering::rank) — `selected` was built
  against a registry other than `registry`.
- [`selection_requires`](crate::stdcore::backends::lowering::selection_requires)
  — the same condition as `rank`.
- [`rules_needs`](crate::stdcore::backends::lowering::rules_needs) — the rule
  selection that the plan context `cx` ([`PlanCx`](crate::core::plan::PlanCx))
  carries was built against a registry other than the one `cx` holds.
- [`LoweringStage`](crate::stdcore::backends::lowering::LoweringStage), in its
  [`Stage::needs`](crate::core::stage::Stage::needs) and
  [`Stage::run`](crate::core::stage::Stage::run) implementations — the rule
  selection that the plan carries was built against a registry other than the
  one that the plan context ([`PlanCx`](crate::core::plan::PlanCx)) or the run
  holds.

## `stdcore::backends::pieces`

- [`PiecesStage`](crate::stdcore::backends::pieces::PiecesStage), in its
  [`Stage::needs`](crate::core::stage::Stage::needs) and
  [`Stage::run`](crate::core::stage::Stage::run) implementations — the condition
  of [`rules_needs`](crate::stdcore::backends::lowering::rules_needs) above. Its
  `run` also lets through a panic of a rewrite stage or of a stage run again
  while a [piece](crate::core::piece::Piece) (a document-level value a backend
  places beside the content) is brought up to date, as
  [`StageCx::resolve_value`](crate::core::run::StageCx::resolve_value) does.

## `stdcore::backends::text`

- [`TextOutput::slice`](crate::stdcore::backends::text::TextOutput::slice) — the
  byte range of the given [`TextMark`](crate::stdcore::backends::text::TextMark)
  lies outside the output's text or does not start and end on character
  boundaries. A mark taken from this output's own `marks`, with the output
  unchanged since printing, never panics; a mark from another output, a mark
  built or changed by hand, or an output whose `text` was changed after printing
  can.

## `dump::diag`

These helpers assert on the diagnostics of a test and fail the test by
panicking. [`expect`](crate::dump::diag::expect) selects the diagnostics that
carry an identifier, and each method of
[`DiagExpect`](crate::dump::diag::DiagExpect) narrows that selection; a
narrowing method that leaves no diagnostic panics with a message listing the
candidates it had (identifier, severity, position, covered text and data).

- [`expect`](crate::dump::diag::expect) — no diagnostic of the collection
  carries the identifier; the message lists the identifiers present.
- [`expect_none`](crate::dump::diag::expect_none) — a diagnostic of the
  collection carries the identifier; the message describes it.
- [`ExpectDiag::expect`](crate::dump::diag::ExpectDiag::expect) — the same
  condition as [`expect`](crate::dump::diag::expect), as a method.
- [`ExpectDiag::expect_none`](crate::dump::diag::ExpectDiag::expect_none) — the
  same condition as [`expect_none`](crate::dump::diag::expect_none), as a
  method.
- [`DiagExpect::at`](crate::dump::diag::DiagExpect::at) — no diagnostic in the
  selection covers exactly that source text.
- [`DiagExpect::at_line_col`](crate::dump::diag::DiagExpect::at_line_col) — no
  diagnostic in the selection starts at that line and column.
- [`DiagExpect::severity`](crate::dump::diag::DiagExpect::severity) — no
  diagnostic in the selection has that severity.
- [`DiagExpect::frames`](crate::dump::diag::DiagExpect::frames) — no diagnostic
  in the selection carries exactly that many traceback frames.
- [`DiagExpect::data`](crate::dump::diag::DiagExpect::data) — the expected value
  cannot be converted to a serialized value (the cases that
  [`from_serde`](crate::dump::json::from_serde) lists, such as a floating-point
  number); or no diagnostic in the selection carries that data.
- [`DiagExpect::count`](crate::dump::diag::DiagExpect::count) — a number of
  diagnostics other than the one asked for remains.
- [`DiagExpect::once`](crate::dump::diag::DiagExpect::once) — a number of
  diagnostics other than one remains.

## `dump::golden`

Both functions panic with the message of the
[`GoldenError`](crate::dump::golden::GoldenError) that the matching `check_`
function of the same module returns; the `check_` functions never panic.

- [`assert_text_golden`](crate::dump::golden::assert_text_golden) — the golden
  file is missing, differs from the actual text, or cannot be read or written.
- [`assert_json_golden`](crate::dump::golden::assert_json_golden) — the golden
  file is missing, differs in value from the actual value, cannot be read or
  written, or holds text that is not the JSON form of a serialized value (the
  cases that [`from_json`](crate::dump::json::from_json) lists); or the actual
  value has no JSON form (it holds a map key beginning with `$`).

## `dump::session`

- [`writer`](crate::dump::session::writer) — only if a new serialization session
  refused the table of fragments that `writer` registers, which cannot happen: a session holding just the standard
  tables accepts one more table with a distinct name and a non-empty identifier.
  The `expect` in `writer` states an invariant of that module, and no requirement on the
  caller is involved. This is why `writer` returns the session directly while
  [`reader`](crate::dump::session::reader) returns a `Result`.

## `layout::doc`

- [`DocBuf`](crate::layout::doc::DocBuf) — the type's own `# Panics` section
  states the condition that all of its node-creating methods share: one of the
  pools of the layout document (the lists of nodes, children, rows, cells,
  columns, tables and text that a `DocBuf` holds) would grow past 2³²−1 entries
  (about four billion entries, or four gigabytes of text), a size no document
  reaches. The panic message gives the pool; each method below
  lists the pools it can overflow.
- [`DocBuf::text`](crate::layout::doc::DocBuf::text) — the node or text pool.
  *Debug builds only*, additionally: the string contains a newline or a tab. A
  release build writes such a string as it is, and the column count of that
  line is then wrong.
- [`DocBuf::line`](crate::layout::doc::DocBuf::line) — the node pool.
- [`DocBuf::softbreak`](crate::layout::doc::DocBuf::softbreak) — the node pool.
- [`DocBuf::hardline`](crate::layout::doc::DocBuf::hardline) — the node pool.
- [`DocBuf::blocksep`](crate::layout::doc::DocBuf::blocksep) — the node pool.
- [`DocBuf::nest`](crate::layout::doc::DocBuf::nest) — the node pool.
- [`DocBuf::align`](crate::layout::doc::DocBuf::align) — the node pool.
- [`DocBuf::group`](crate::layout::doc::DocBuf::group) — the node pool.
- [`DocBuf::mark`](crate::layout::doc::DocBuf::mark) — the node pool.
- [`DocBuf::concat`](crate::layout::doc::DocBuf::concat) — the node or children
  pool.
- [`DocBuf::prefix`](crate::layout::doc::DocBuf::prefix) — the node or text pool.
  *Debug builds only*, additionally: the prefix contains a newline or a tab. A
  release build writes such a prefix as it is, and the column count of the lines
  it prefixes is then wrong.
- [`DocBuf::raw`](crate::layout::doc::DocBuf::raw) — the node or text pool.
- [`DocBuf::table`](crate::layout::doc::DocBuf::table) — the node, column, row,
  cell or table pool.
- [`DocBuf::fill_words`](crate::layout::doc::DocBuf::fill_words) — the node, text
  or children pool.

## `layout::printer`

- [`print`](crate::layout::printer::print) — `root` was not created by the
  layout document `buf` (a [`DocBuf`](crate::layout::doc::DocBuf)), or was
  created before a call to [`DocBuf::clear`](crate::layout::doc::DocBuf::clear),
  and its index is beyond `buf`'s nodes. An identifier from another layout
  document whose index falls within range refers to the node at that index of
  `buf` instead, which the layout engine cannot detect.
- [`print_full`](crate::layout::printer::print_full) — the same condition as
  `print`.

## `untechxt`

The Unicode-to-LaTeX encoder is the external crate
[`untechxt`](crate::untechxt), re-exported at that path; its own documentation
states what panics in it.

## `flm-ext`

`flm-ext` is the extension layer: it loads extensions (sets of files, found by name, that
provide packages written as data, in JavaScript, compiled to WebAssembly or compiled into
the program) and lets
their features reach the library. Its items are named here without a link, since this page belongs to the
documentation of `flm_core`.

- `flm_ext::adapter::node::NodeFacade::shared(tree, id)` makes the façade through which an
  extension reads the node `id` of the shared tree `tree`. `id` must be a node of `tree`:
  a façade made with the id of a node of another tree panics at its first read, as
  techy's [`NodeTree::node`](crate::techy::core::node::NodeTree::node) does. Every façade
  that the adapter hands an extension is made from its tree's own ids, so only Rust code
  of a host can make one that panics.
