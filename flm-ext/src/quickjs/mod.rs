//! The QuickJS binding: extensions written in JavaScript, run by the QuickJS
//! engine through `rquickjs` (cargo feature `quickjs`).
//!
//! # What the binding is
//!
//! The binding is the engine `js` ([`JsEngine`]), which a program adds to its
//! loader of extensions with
//! [`ExtensionLoader::with_js`](crate::ExtensionLoader::with_js), over a
//! [`JsHost`]: the settings every engine instance is created with
//! ([`JsSettings`]), the modules the program offers to every extension, and
//! where `console` output goes. Each thread that runs a handler of a loaded
//! extension gets an *engine instance* of its own for that extension (a
//! QuickJS runtime with one context), shared by every package of the
//! extension, created on first use with the host's limits. A script reaches
//! the library through the API modules, which mirror [`crate::api`] item for
//! item, and nothing else outside the engine.
//!
//! # An extension in JavaScript
//!
//! The extension's manifest names the engine and the main module
//! (`flmextension.yaml`):
//!
//! ```yaml
//! extension:
//!   engine: js
//!   js:
//!     main: main.js
//! ```
//!
//! The main module's default export is a function, called once with the
//! extension builder `ext`; each `ext.package(name, object)` declares one
//! package of the extension (`_` is the default one). Each feature's
//! `register(b, settings)` declares what the feature contributes; `settings`
//! is the feature's configuration (`{}` when nothing is written). The
//! example declares one package with one feature, a setting of each kind, a
//! macro whose replacement a hook computes, and a stage reading an option,
//! with a helper imported from a file beside the main module (`main.js`):
//!
//! ```js
//! import { StageDecl, Needs, Effect, Scope } from "flm:core/stage";
//! import { ArgDef } from "flm:lang/mint";
//! import { chars_text } from "flm:lang/text";
//! import { Definition, UserDefinition, Expansion } from "flm:lang/rewrites/userdefs";
//! import { element_name_under, expand_in_tree_rule_for } from "flm:stdcore/features/userdefs";
//! import { words_of } from "./helpers.js";
//!
//! export default function (ext) {
//!   ext.package("_", {
//!     uses: ["stdcore"],
//!     features: {
//!       wc: {
//!         title: "Words",                       // optional, as `docs` and `version`
//!         settings: {                           // optional
//!           config: { schema: { type: "object", properties: { mark: { type: "string" } } } },
//!           options: {
//!             "wc.counting": { bind: "render", schema: { type: "object", properties: { min: { type: "integer" } } } },
//!           },
//!         },
//!         register(b, settings) {
//!           // A macro whose replacement a hook computes, with the configured mark.
//!           const shout = Definition.macro_("shout", [ArgDef.mandatory("text")]);
//!           const element = element_name_under("wc", shout);
//!           b.element(new UserDefinition(shout).element_def_with(b.feature(), element, {
//!             expand(node, cx, xp) {
//!               const text = chars_text(node.argument_content_nodes_named("text")) ?? "";
//!               return Expansion.Emit([cx.stage_chars(text.toUpperCase() + (settings.mark ?? ""))]);
//!             },
//!           }));
//!           b.lowering_rule(expand_in_tree_rule_for(element, shout));
//!           // A table and a stage writing it, reading an option.
//!           b.table("wc.words", "The number of words of each tree.");
//!           b.stage({
//!             decl() { return new StageDecl("wc.count", Effect.Compute, Scope.PerTree).writes("wc.words"); },
//!             needs() { return new Needs().option("wc.counting"); },
//!             run(cx, out) {
//!               const min = cx.option("wc.counting").min ?? 1;
//!               let words = 0;
//!               for (const node of cx.my_tree().descendants())
//!                 if (node.is_chars()) words += words_of(node.chars(), min);
//!               out.table("wc.words").push({ tree: cx.tree_id(), words });
//!             },
//!           });
//!         },
//!       },
//!     },
//!   });
//! }
//! ```
//!
//! and `helpers.js`:
//!
//! ```js
//! export function words_of(text, min) { return text.split(/\s+/).filter(word => word.length >= min).length; }
//! ```
//!
//! # The package object and the feature object
//!
//! `ext.package(name, object)` declares one package: `name` is what a `use`
//! entry writes after `#` (letters, digits, `-` and `_`; `_` is the default
//! package), `object` the package object.
//!
//! | Key of the package object | Shape | Optional | Meaning |
//! |---|---|---|---|
//! | `uses` | a list of `use` entries | yes | the packages it uses: `stdcore`, another extension, `.#core` (a package of this extension) |
//! | `flm` | a map | yes | its block: `features`, `parsing`, `workflows`, `feature_order` (as a data extension's manifest writes them under `flm`; `use` is written as `uses`) |
//! | `templates` | a list of paths | yes | the compose templates it ships, each the path of a description, `templates/OUTPUT/NAME.yaml` or `templates/OUTPUT/NAME/flmtemplate.yaml`, the compose template's name being `NAME`; a path of neither shape is refused |
//! | `host` | a list of texts | yes | the host services its handlers call ([Host services](#host-services)) |
//! | `features` | a map from a feature's name to its feature object | yes | its scripted features, in the order written |
//!
//! | Key of the feature object | Shape | Optional | Meaning |
//! |---|---|---|---|
//! | `title`, `docs`, `version` | texts | yes | as a compiled feature's information |
//! | `settings` | a map: `config` (`{ docs, schema }`), `options` (name → `{ bind, docs, schema }`, `bind` being `document`, `render` or `either`), `merge_entries`, `markup`, `path` | yes | the settings it declares, as data ([`SettingsReport`](crate::engine::SettingsReport)) |
//! | `register(b, settings)` | a function | no | declares what the feature contributes, with its configuration |
//!
//! The feature's name is its key in `features`; any other key of the feature
//! object is refused, and so is, at any depth below a key other than
//! `register`, a value that is a function, a symbol or a BigInt, or a key
//! that is a symbol (JSON would drop it); the refusal names the key path
//! (`settings.options.x`). A key whose value is `null` is read as absent.
//!
//! A stage reads an option it declares with `cx.option(name)`, after naming
//! it in its needs: `new Needs().option(name)`. The value comes as data (a
//! map of the option's settings).
//!
//! # Imports
//!
//! | Specifier | Names |
//! |---|---|
//! | `flm:core/stage`, … | a module of the extension API (the table below) |
//! | `./helpers.js`, `../lib/x.js` | a file of the extension, joined to the importing module's folder; it may not climb above the extension's folder |
//! | `lodash`, `mathjax/core` (a bare name) | a module the program offers ([`JsHost::with_module`]) |
//!
//! The extension written in a specifier is required (`./helpers.js`, not
//! `./helpers`), as ES modules require; a bare name is never a file of the
//! extension. A module imported from two places is one module. A module the
//! program offers imports another by its name or by a path relative to its
//! own name. A dynamic `import()` is refused wherever it is written: every
//! module is imported by an `import` declaration at the top of a module.
//!
//! # Loading and the engine instances
//!
//! The extension's files are read once, at load, on the loading thread: the
//! main module and every module of its static imports, which the loading
//! instance records. Every other instance, on any thread, is built from the
//! recorded modules and never touches the files. Once the static imports of
//! an instance are loaded (before any of its code runs), its module resolver
//! is closed. The queued promise jobs run at the end of the outermost entry
//! into an instance (its creation, the default export's call, each handler
//! call that no other call on the instance is running beneath): a handler's
//! promise callbacks therefore run when the outermost call returns, while
//! its objects are valid; a handler call nested in another on the same
//! instance never runs the callbacks of the call it interrupts. The default
//! export's packages are read after its callbacks ran, so an `async` default
//! export declares every package it declares after an `await`. The default
//! export must finish while the extension loads: one whose promise is still
//! pending once those callbacks ran fails the load, and a later call of
//! `ext.package` (by a handler that kept `ext`) is refused. What the
//! jobs meet fails the outermost entry: a refused `import()`, even when the
//! script catches the rejection, and a promise rejected with no handler (an
//! exception thrown in a callback or in an `async` function). A refused
//! `import()` rejects its promise with the refusal's own error object, and a
//! promise rejected with that object is the refusal, reported once; another
//! error of the same text is a rejection of its own. A job reaching the
//! memory limit fails the entry with the limit's text and discards the
//! instance, as the handler's own call reaching it does. The jobs run at the
//! end of a failed outermost entry too, and what they meet follows the
//! failure, whatever its kind ([`ExtError::Followed`](crate::ExtError::Followed):
//! the failure's kind and data, its text followed by theirs); only after an
//! exceeded limit, which discards the instance, do they not run.
//!
//! A call of the extension API is answered only from a feature's `register`
//! or from a handler that `register` declared, while that call is the
//! innermost handler call of the extension running on the thread. Made
//! elsewhere (at a module's top level, in the default export, or while an
//! instance is created inside a handler call of another extension, whose
//! frame is then the innermost one), it throws "the extension `S` must ACT
//! from a feature's `register` or from a handler that `register` declared",
//! ACT naming the call ("call `chars` of the extension API"). Importing a
//! module of `flm:` and writing to `console` are not such calls.
//!
//! An instance is kept on its thread for as long as its extension is held:
//! by the registry, the factories and the handlers of the load that made it.
//! Once they are all dropped (a preparation replaced by a new one, which
//! loads every extension again), the next entry into any instance on that
//! thread drops the replaced loader's instances there; a thread that runs no
//! handler again keeps them until it ends.
//!
//! # The mapping rules
//!
//! How the extension API of [`crate::api`] appears in JavaScript.
//!
//! 1. **Module paths.** The module `flm_ext::api::a::b` is the JavaScript
//!    module `flm:a/b`, and every item is exported under its Rust name
//!    verbatim, `snake_case` included:
//!    `import { StageDecl, Effect } from "flm:core/stage";`. The modules are
//!    listed below ([The API modules](#the-api-modules)).
//! 2. **A struct or enum of plain data** (`StageDecl`, `Needs`, `ArgDef`,
//!    `BodyDef`, `ElementSpec`, `ElementDef`, `FallbackDef`, `Definition`,
//!    `RewriteDecl`, `BackendDef`, `OutputDef`, `ArtifactId`, `Artifact`) is
//!    a class whose `toJSON()` answers the value's JSON shape. `X::new(a, b)`
//!    is `new X(a, b)`; a builder method keeps its name and answers the same
//!    instance:
//!    `new StageDecl("wc.count", Effect.Compute, Scope.PerTree).writes("wc.words")`.
//!    Another associated constructor is a static method of the same name:
//!    `Definition.macro_("shout", [ArgDef.mandatory("text")])`,
//!    `Artifact.bytes(new ArtifactId("typst", "document"), "application/x-typst", text).as_entry()`. A field whose
//!    name a builder method takes is held under the name with a trailing
//!    underscore (`def.docs_`); `toJSON()` answers the Rust names. A value
//!    read back from Rust (a span, a walk entry, a node's address, the map
//!    `ud.definition()` answers) is a plain object of its shape:
//!    `node.span()` is `{ start, end }`. The constructors `UserDefinition::new`
//!    and `DefinitionBindings::new` answer objects held in Rust:
//!    `new UserDefinition(definition)` and `new DefinitionBindings(site, variant)`
//!    answer such an object (rule 4). A declaration a helper made
//!    (`ud.element_def(owner, name)`) is an `ElementDef` holding such an
//!    object: its accessors `owner()`, `names()`, `syntax()` and `element()`
//!    answer `null`, as Rust's answer `None`.
//! 3. **An enum of unit variants** is a frozen object of string constants
//!    named by the variants: `Effect.Compute === "compute"`,
//!    `Scope.PerTree === "per-tree"`, `Severity.Note === "note"`,
//!    `ArgLayout.InlineOrBlocks === "inline-or-blocks"`. An enum with data
//!    variants has one static constructor per variant answering its JSON
//!    shape: `BlockRole.Inline(InlineSpacing.CONTENT)` is
//!    `{ inline: { absorb_before: false, absorb_after: false } }`,
//!    `BlockRole.Block` is `"block"`, `SyntaxDef.Macro(args)` is
//!    `{ macro: { args } }`, `Expansion.Emit(staged)` is `{ emit: staged }`,
//!    `Aspect.Expand(hook)` is `{ Expand: hook }`. An associated constant is
//!    a property of the same name (`BlockRole.DEFAULT`, `InlineSpacing.CONTENT`).
//! 4. **A reference to an object with methods** (`FeatureBuilder`, `StageCx`,
//!    `StageOut`, `TablePiece`, `Node`, `UserDefinition`, `Expander`, …) is an
//!    instance of the class of that name, holding a token `{ kind, call, index }`
//!    and nothing else; every method of the Rust trait is a method of the
//!    class, with the same name and arguments:
//!    `node.argument_content_nodes_named("text")`. An instance is valid for
//!    the handler call that received it, while that call is the innermost one
//!    running; kept and used later (or by a nested hook), every method throws
//!    `FlmError` of kind `"stale_handle"`. An expansion hook therefore reaches
//!    its definition through `cx.definition()`, not through a `UserDefinition`
//!    made in `register`, whose object belongs to that call:
//!    `cx.definition().variant_for_use(node)`. Such an instance writes itself
//!    as `{"@handle": { kind, call, index }}` when it crosses back to Rust.
//!
//!    **A durable object** is the one exception: a template language
//!    (`TemplateLanguage`) or a compiled template that
//!    `compile_template_with` answers is kept by FLM in a table of the
//!    feature that made it, one table per feature and registry, and its
//!    instance holds a token `{ kind, table, index }` that writes itself as
//!    `{"@durable": { kind, table, index }}`. A template language's class
//!    extends the class `Durable` (the per-call classes extend `Handle`); a
//!    compiled template is a `CompiledTemplate` in either form, and each
//!    instance writes back the form it came in. A durable object is
//!    valid on every engine instance and every thread, in every later call
//!    of that feature, for as long as the registry it was made under is in
//!    use: a hook may keep it in module scope, and a lowering rule may use a
//!    template that the feature's late-initialization hook compiled on
//!    another thread. Under another registry (the same engine instance may
//!    serve two), every use of it throws `FlmError` of kind
//!    `"stale_durable"`. A handler need not keep one: a second
//!    `compile_template_with` with an equal specification answers the same
//!    token without compiling the text again. Only a call that has the
//!    feature's table can use a durable object: the builder, a stage's
//!    context, an expansion hook's expander, a rule's lowerer, and a
//!    late-initialization hook's language; an aspect handler (rule 7) has
//!    none.
//! 5. **A method generic over a type** takes the registered name:
//!    `cx.get("flm.blocks.structured")`.
//! 6. **A free function** is exported by its module and called directly:
//!    `chars_text(nodes)`, `element_name_under("wc", definition)`.
//! 7. **A trait an extension implements** (`Stage`, `Expand`, `Lower`,
//!    `RewriteStage`) is a plain object with the trait's methods, passed where
//!    Rust passes an `Arc<dyn Trait>`: `b.stage({ decl() {…}, needs() {…}, run(cx, out) {…} })`,
//!    `b.rewrite_stage({ decl() {…}, needs() {…}, restage(node, cx, rs) {…} })`,
//!    `new ElementSpec("wc.badge").with({ Expand: hook })`,
//!    `ud.element_def_with(owner, name, hook)`. A stage object may leave out
//!    `needs` (it then needs nothing); an expansion hook
//!    `{ expand(node, cx, xp) {…} }` may leave out `policy` (it is then
//!    `"per-plan"`); a rewrite stage object may leave out `needs` likewise.
//!    A rule's body `{ lower(node, cx, lw) {…} }` is passed to
//!    `new LoweringRule(name, element, decl, body, mark)`; a backend's emit
//!    and final stages, stage objects as `b.stage` takes them, to
//!    `new BackendDef(name, native, lowering, emit, final_stage)`. `run`
//!    answers nothing; `expand` answers an `Expansion`, `lower` a `Lowering`,
//!    `restage` a `Restage`; a thrown value is the handler's failure. A
//!    citation provider (`CitationProvider`) and a template module
//!    (`TemplateModule`) are such objects too, given to `b.contribute_to` or
//!    `b.peer_slot`; the Rust method `name()` of these two traits is a
//!    property of the object, `name`, by which the binding finds the object
//!    again and the messages name it ([A citation provider](#a-citation-provider),
//!    [A compose template written as a module](#a-compose-template-written-as-a-module)).
//!
//!    `Expansion.Default` keeps the use site: with `expand_in_tree_rule_for`
//!    as the element's rule, the render then reports
//!    `flm.lowering.rule-failed`; with a lowering rule of the feature's own
//!    for the element, that rule lowers the node.
//!
//!    The **handler aspects** `Referenceable` and `Sectioning` are such
//!    objects too, given in an element's specification
//!    (`new ElementSpec(name).with(Aspect.Referenceable({ points(node) {…} }))`)
//!    or attached to an element another feature declares
//!    (`b.attach(element, Aspect.Sectioning({ level(node) {…} }))`).
//!    `points(node)` answers a list of `RefPoint`s; a `Sectioning` object has
//!    `level(node)` (a number from 0 up) and may leave out `in_toc(node)` (a
//!    boolean; listed when absent) and `toc_title(node)` (a `ContentRef` or
//!    `null`; no title when absent). Whether the object has `in_toc` and
//!    `toc_title` is read when `register` gives it to FLM: either one added
//!    to the object later is never called. An aspect handler is called on the thread that runs the
//!    stage asking for it (the reference points and the table of contents),
//!    and is lent the node alone: it has no context, no lowerer and no
//!    durable table. A build hook is an object as well:
//!    `b.late_init({ late_init(lang, diags) {…} })`, called once the
//!    language is built, with the built language and a `diags` whose
//!    `push(diagnostic)` adds to the registry's build diagnostics (rule 11);
//!    `b.finalizer({ finalize(b) {…} })`, called in the build's second
//!    round with a builder of that round.
//! 8. **`Result<T, E>`** is `T`, or a thrown `FlmError`: `error.kind` is the
//!    `ExtError` variant's name (`"handler"`, `"stale_handle"`,
//!    `"argument"`, `"name_outside_extension"`, `"value"`, `"core"`,
//!    `"host"`, `"unsupported"`, `"stale_durable"`; for a failure followed by what the
//!    queued jobs met, the failure's own), `error.message` its text, `error.data` its
//!    [`data`](crate::ExtError::data) map: `read_chars(nodes)` over a node
//!    that is not characters throws kind `"core"` with that node's index in
//!    its tree (not its position in `nodes`) at `error.data.data.node`. A handler may catch it; uncaught, the handler
//!    fails and the diagnostic's data holds it under `data.flm_error`.
//! 9. **`Option<T>`** is `T` or `null`. A `Vec<T>` is an array; a tuple is an
//!    array of its elements: `cx.trees()` answers `[[id, node], …]`.
//! 10. **Values are JSON.** Every argument and result crosses as JSON text: a
//!     number must be an integer (`1.5` is refused with kind `"value"`); an
//!     `undefined` inside data is left out, as `JSON.stringify` does (write
//!     `null` for "no value"); a map key must not begin with `$` nor be
//!     `@handle`. A byte string (`b.digest_input(text)`) is a string, whose
//!     UTF-8 bytes are taken.
//! 11. **A diagnostic** a handler reports is the plain object
//!     `{ id, severity, message, data, at }` (`data` and `at` optional, `at` a
//!     node): `out.diags({ id: "wc.empty", severity: Severity.Warning, message: "the tree has no words" })`.
//!     Its `id` must begin with the extension's stem and a dot (or with `X`
//!     and a dot, for an extension whose stem is `profile-X`).
//! 12. **A `const` name** is a constant of the same name and value:
//!     `MATH === "flm.math"` in `flm:stdcore/features/math`,
//!     `BLOCK === "flm.pres.block"` and `BODY_SLOT === "body"` in
//!     `flm:stdcore/backends/pres`, `TAG === "flm.pres.html.tag"` in
//!     `flm:stdcore/backends/html`. A constant holding a list is a frozen
//!     array: `ALL` (`flm:stdcore/backends/pres`) holds the names of the
//!     standard presentation elements, and `ENVIRONMENTS`
//!     (`flm:stdcore/features/math`) is
//!     `[["equation", true], ["equation*", false], …]`.
//!
//! # A lowering rule
//!
//! A feature's `register` builds a lowering rule with `new LoweringRule(name,
//! element, decl, body, mark)`, the arguments in the order of
//! [`LoweringRule::new`](crate::api::core::lowering::LoweringRule::new), and
//! registers it with `b.lowering_rule(rule)`. The body is called on the thread
//! that lowers, on that thread's engine instance, under the extension's
//! limits; it builds the node's presentation through the lowerer `lw` and
//! answers a `Lowering` (a fragment of `register`):
//!
//! ```javascript
//! import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
//! import { BLOCK, SPAN } from "flm:stdcore/backends/pres";
//!
//! b.lowering_rule(new LoweringRule(
//!   "box.box.lower.default", "box.box",
//!   new LoweringDecl().produces(BLOCK).produces(SPAN).reads("box.rows"),
//!   {
//!     lower(node, cx, lw) {
//!       const content = lw.lower_arg_named(cx, node, "text");
//!       const rows = lw.get_at("box.rows", node) ?? [];
//!       return Lowering.Emit([lw.pres_tree(cx, {
//!         element: BLOCK, payload: { role: "box.box" },
//!         children: [{ element: SPAN, payload: { kinds: ["inner"] },
//!                      children: [...content.map(build => ({ build })), { text: ` (${rows.length})` }] }],
//!       })]);
//!     },
//!   },
//!   DefaultMark.Everywhere,
//! ).docs("A box."));
//! ```
//!
//! The lowerer's methods that stage take `cx` first; `get_at(name, node)`
//! answers the items of the extension's own table `name` whose field `addr`
//! is the node's address (no counterpart in `flm-core`, see
//! [`ExtTableRead::items_at`](crate::api::core::data::table::ExtTableRead::items_at));
//! `pres_tree(cx, tree)` stages a tree of presentation nodes written as data
//! ([`pres_tree`](crate::api::core::lowering::pres_tree)).
//!
//! `lw.get(name)` reads a table the rule declared (`reads` or `reads_opt`)
//! and answers `null` when the table is not there, or one of these forms
//! ([`TableValue`](crate::api::core::data::table::TableValue)): `{ ext }`,
//! the items of an extension's own table; `{ trees }`, a trees table;
//! `{ projection }`, a standard table written as data; `{ counters }`, the
//! numbering feature's counter table `flm.counters`, an object with the
//! methods of [`Counters`](crate::api::stdcore::features::numbering::Counters)
//! (`record(point)`, `format(point, options)`); and `{ anchors }`, the
//! references feature's anchors table `flm.anchors`, an object with the
//! method [`Anchors::id`](crate::api::stdcore::features::refs::Anchors::id).
//! A point is written `{ addr, index }`: the address of a node (`lw.addr()`
//! for the node the rule lowers) and the point's index among its reference
//! points. `lw.lang()` answers the built language, from which
//! `new TemplateLanguage(lw.lang())` is made (rule 4).
//!
//! The documentation
//! of [`crate::api::core::lowering`] describes every method of the lowerer,
//! and the documentation of [`crate::api::stdcore::backends::pres`] and
//! [`crate::api::stdcore::backends::html`] gives the payload shape of each
//! presentation element.
//!
//! # A rewrite stage
//!
//! A feature's `register` declares a rewrite stage with
//! `b.rewrite_stage(stage)`, `stage` a plain object whose `decl()` answers a
//! `RewriteDecl` (`new RewriteDecl(name, publishes)`, the stage's name and
//! the capability whose table of trees it publishes), whose `needs()`
//! answers the stages it runs after and before, and whose
//! `restage(node, cx, rs)` decides what becomes of one node of a tree before
//! the trees are frozen. The library drives the tree: it calls `restage`
//! once for each node it reaches, on the thread that rewrites, on that
//! thread's engine instance, under the extension's limits. The handler
//! answers `Restage.Descend` (the node is copied, its children restaged
//! through the handler in turn) or `Restage.Emit([…])`, the nodes it staged
//! in its place through `rs` (a fragment of `register`):
//!
//! ```javascript
//! import { Needs, RewriteDecl } from "flm:core/stage";
//! import { Restage } from "flm:techy/transform";
//!
//! b.rewrite_stage({
//!   decl: () => new RewriteDecl("dash.dash", "dash.dashed"),
//!   needs: () => new Needs().after("flm.userdefs").before("flm.blocks.structured"),
//!   restage(node, cx, rs) {
//!     if (node.is_chars() && node.chars().includes("--"))
//!       return Restage.Emit([rs.stage_chars(node.chars().replace(/--/g, "–"), node)]);
//!     if (node.element_name() === "dash.quote") {
//!       const content = (node.argument_content_nodes_named("text") ?? []).flatMap(child => rs.restage_subtree(child));
//!       const argument = rs.restage_argument_with_content(node, 0, content);
//!       return Restage.Emit([rs.stage_chars("“", node), rs.restage_invocation(node, [argument], []), rs.stage_chars("”", node)]);
//!     }
//!     return Restage.Descend;
//!   },
//! });
//! ```
//!
//! The methods of `rs` that restage a region (`restage_subtree`,
//! `restage_children`, `restage_argument`, `restage_argument_named`,
//! `restage_slot`) call the handler again for each node of the region, on
//! the same engine instance, before they answer: nested handler calls, each
//! with objects of its own. The outer call's `node`, `cx` and `rs` are stale
//! while a nested call runs and valid again once the method has answered.
//! A staged node is placed once, by this same call: in its `Emit`, or as the
//! content given to `restage_argument_with_content`. Each argument or slot
//! object a method of `rs` answers is given once to `rs.restage_invocation`,
//! which takes them one at a time, the arguments first, then the slots. After
//! a refusal, a bundle given may be spent: those before the one at fault when
//! a bundle is refused (given before, or of the wrong kind), all of them when
//! techy refuses the call; the handler stages new ones.
//! A failing handler keeps its node, copied as `Restage.Descend` copies it;
//! a refusal after which the tree cannot be built abandons the tree's
//! rewrite ([Failures](#failures)). The stage publishes
//! the trees in its table; it runs when that table is demanded (a workflow's
//! `demands`). The documentation of
//! [`Restager`](crate::api::techy::transform::Restager) describes every
//! method of `rs`.
//!
//! # A backend of your own
//!
//! A feature's `register` declares a whole output format: a backend (its
//! native set and its three stages) with `b.backend(new BackendDef(name,
//! native, lowering, emit, final_stage))`, and an output a caller asks for by
//! name with `b.output(new OutputDef(name, capability, table, media_type,
//! extension))`. A backend's name and an output's name are the extension's
//! stem itself (`typst`) or begin with the stem and a dot; an extension whose
//! stem is `profile-X` may also use `X` in place of its stem. The three
//! stages:
//!
//! - the lowering stage is the library's, `new LoweringStage(backend)`
//!   (`flm:stdcore/backends/lowering`), which applies the lowering rules and
//!   writes the table `lowered_table_name(backend)` (`flm.lowered.typst`);
//!   the feature declares that table with
//!   `b.table_named(name, StandardTable.LoweredTrees, docs)`;
//! - the emit stage is a stage object, as `b.stage` takes one: it reads the
//!   lowered trees (`cx.get(name).lowered`, an object with `trees()`,
//!   `tree(id)`, `requires(id)` and `rounds(id)`), walks each tree's nodes
//!   (`element_name()`, `payload()`, `slot_count()`, `slot_content_nodes(i)`,
//!   …) and pushes the text it writes into a table of the extension's own;
//! - the final stage is a stage object too: it reads that text and pushes the
//!   output's files into a table declared with `StandardTable.ArtifactSet`, one
//!   push per file (`Artifact.bytes(new ArtifactId(owner, name), media_type,
//!   content).as_entry()` for the entry file, `Artifact.folder(id, [[path,
//!   content], …])` for a folder), each content a text, `{ text }` or `{
//!   base64 }` (see [the data form of an
//!   artifact](crate::api::core::artifact#the-data-form-of-an-artifact)).
//!
//! A stage of another output can read those files: it declares a read of the
//! table, and `cx.get(name).projection` is `{ artifacts, violations }`, each
//! artifact a plain object in the same data form with its `path`, where the
//! host writes it (see [reading the files of an
//! output](crate::api::core::artifact#reading-the-files-of-an-output)). Such
//! an output declares the outputs it is made from with
//! `new OutputDef(…).made_from(name)`.
//!
//! The native set lists the elements the emit stage writes itself: the
//! presentation vocabulary (`ALL`, `flm:stdcore/backends/pres`, among which
//! the stand-in `flm.pres.unsupported` every backend must write), the
//! structural elements a lowered tree keeps (`flm:lang/structural`), and the
//! extension's own presentation elements. Every element a document using the
//! backend can hold must reach the native set, by itself or through a rule:
//! the library registers some of its rules for its own backends only (the
//! image of the `graphics` feature, `flm.graphics.image`), so a backend of an
//! extension declares a rule of its own for such an element, which may
//! stage the stand-in. An element that no rule brings into the native set
//! makes the plan fail.
//!
//! A presentation element of the extension's own is declared with
//! `b.presentation_element(name, docs)`; its payload is any data value, which
//! a rule gives in `lw.pres(cx, element, payload, children)` (the rule lists
//! the element in its `produces`) and the emit stage reads back with
//! `node.payload()`. The rule that stages it is marked as the default on the
//! backend alone (`DefaultMark.OnBackends(["typst"])`); a second rule for the
//! same element of the document, marked as the default everywhere, lowers it
//! into the shared vocabulary for every other backend, which does not write
//! the extension's element.
//!
//! The registration, abridged from the test extension `tests/js/typst/` of
//! this crate (`emit` walks the trees and writes Typst markup):
//!
//! ```javascript
//! const lowered = lowered_table_name("typst");                  // "flm.lowered.typst"
//! b.table_named(lowered, StandardTable.LoweredTrees, "the lowered trees");
//! b.table("typst.output", "the document as Typst markup");
//! b.table_named("typst.document", StandardTable.ArtifactSet, "the Typst file");
//! const emitStage = {
//!   decl: () => new StageDecl("typst.emit", Effect.Compute, Scope.Document).writes("typst.output"),
//!   needs: () => new Needs().reads(lowered),
//!   run(cx, out) { out.table("typst.output").push(emit(cx, cx.get(lowered).lowered)); },
//! };
//! const finalStage = {
//!   decl: () => new StageDecl("typst.final", Effect.Compute, Scope.Document).writes("typst.document"),
//!   needs: () => new Needs().reads("typst.output"),
//!   run(cx, out) {
//!     const [text] = cx.get("typst.output")?.ext.items() ?? [];
//!     if (text !== undefined)
//!       out.table("typst.document").push(
//!         Artifact.bytes(new ArtifactId("typst", "document"), "application/x-typst", text).as_entry());
//!   },
//! };
//! b.backend(new BackendDef("typst", [...ALL, TEXT, GROUP, COMMENT, CONTAINER, PARBREAK, "typst.pres.callout"],
//!                          new LoweringStage("typst"), emitStage, finalStage));
//! b.output(new OutputDef("typst", "typst.document", "typst.document", "application/x-typst", "typ"));
//! ```
//!
//! A workflow that lists the output, written in the package's `flm` block
//! (`flm: { workflows: { typst: { outputs: ["typst"], demands: ["flm.validated"] } } }`),
//! makes it available by name: the `flm` program then renders the document
//! with `-p typst` and names the file with the output's
//! extension. An emit stage writes the document's text into the output's
//! syntax: a real backend escapes every character the format reserves, in
//! link targets and verbatim content too, where the test extension does not.
//! An output written this way is made of no regions, so a compose template
//! cannot assemble it.
//!
//! # Host services
//!
//! A host service is a named object the program offers to extensions
//! ([`ExtensionLoader::with_host_service`](crate::ExtensionLoader::with_host_service)),
//! through which a handler reaches what the sandbox keeps out; the one this
//! crate defines is `files` ([`crate::host::files`]), and a program may offer
//! others (the `flm` program also offers `programs`, which finds and runs a
//! program installed on the machine; see [Trust](crate#trust)). A package declares the
//! services its handlers call in its package object's `host` list: a package
//! declaring a service the program does not offer does not load, and the
//! list enters the digest.
//!
//! A stage's context, a citation provider's context and a template module's
//! context answer `cx.host(name)`: the service `name` as an object valid for
//! the handler call (a `Files` for `files`, a `HostService` with
//! `call(method, args)` for any other, from `flm:host`). A name the package
//! did not declare is refused, even when the program offers it.
//!
//! The `files` service reads the text of a file in one of two roots, its
//! *scope*: `files.read_text("extension", path)` reads a file of the
//! extension's own folder, `files.read_text("document", path)` a file of the
//! document's folder (`FilesScope.Extension`, `FilesScope.Document`). Each
//! answers `null` when nothing is at the path; a path that leaves its root is
//! refused. The document's files are read through the program's file reader,
//! so a program that watches the files a build read (a live preview) builds
//! again when one changes. Text and "nothing there" cross; bytes do not (a
//! template module's shipped files are read by the binding, see below). A
//! failure of the service is a thrown `FlmError` of kind `host`, whose text
//! is "`files.read_text`: DETAIL".
//!
//! # A citation provider
//!
//! A feature contributes a citation provider to the slot `PROVIDERS_SLOT`
//! (`flm.citations.providers`, from `flm:stdcore/features/citations`): an
//! object with `name` (beginning with the extension's stem and a dot, or with
//! `X` and a dot for an extension whose stem is `profile-X`) and
//! `answer_all(requests, cx)` or `answer(request, cx)`. The retrieve stage
//! asks each provider, in the order of the slot, about the cited keys no
//! earlier provider claimed, in one call; the binding calls `answer_all`
//! when the object has it, else `answer` once per request (a fragment of
//! `register`):
//!
//! ```javascript
//! import { PROVIDERS_SLOT } from "flm:stdcore/features/citations";
//!
//! b.peer_slot(PROVIDERS_SLOT, {
//!   name: "bibfile.json",
//!   answer_all(requests, cx) {
//!     const text = cx.host("files").read_text("document", "bib.json");
//!     if (text === null) return requests.map(() => ({ error: "the file `bib.json` must hold the entries" }));
//!     const records = JSON.parse(text);
//!     return requests.map(({ key }) => {
//!       const record = records[key.toLowerCase()];
//!       if (record === undefined) return null;
//!       return { entry: cx.build_entry([{ text: `${record.author}, ` }, { emphasized: [{ text: record.title }] }]),
//!                identity: key.toLowerCase() };
//!     });
//!   },
//! });
//! ```
//!
//! A request is `{ key }`. The answer holds one item per request, in order:
//! `null` (not this provider's key), `{ error: text }` (its key, which it could
//! not answer), or `{ entry, identity, warnings }`, `identity` and `warnings`
//! optional. The entry is made by the context: `cx.parse_entry(source,
//! origin)` parses FLM text, `cx.build_entry(pieces)` builds it from pieces
//! (`{ text }`, `{ source }`, `{ emphasized: [pieces] }`, `{ strong: [pieces]
//! }`, `{ link: { address, content: [pieces] } }`); two keys with one
//! identity are one work.
//!
//! When the provider's call fails (an exception, a limit, an answer of
//! another shape), every key of the call answers the error
//! "flm.ext.handler-failed: the citation provider `NAME` of the feature `F`
//! of the package `P` of the extension `S` failed: TEXT", which the retrieve
//! stage reports at each key's first citation; those keys then have no entry
//! and no later provider is asked about them.
//!
//! # A compose template written as a module
//!
//! A compose template may be a module whose default export is an object with
//! `name` (the template's name, which a document chooses, `-t NAME`; it is
//! not under the stem rule), `describe(settings)`, `render(data, cx)` and,
//! optionally, `files(data, cx)`. A feature contributes it to the slot
//! `TEMPLATES_SLOT` (`flm.compose.templates`, from
//! `flm:stdcore/features/compose`) with `b.peer_slot(TEMPLATES_SLOT,
//! module)`:
//!
//! ```javascript
//! export default {
//!   name: "tiny",
//!   describe(settings) {
//!     return { output: "html", accepts: "all", requires: [], config: { heading: settings.heading ?? "Tiny", banner: null } };
//!   },
//!   files(data, cx) {
//!     const files = [{ name: "style", extension: "css", media_type: "text/css", text: "h1 { color: teal; }\n" }];
//!     if (data.config.banner !== null) files.push({ name: "banner", file: data.config.banner, scope: "document" });
//!     return files;
//!   },
//!   render(data, cx) {
//!     const head = cx.host("files").read_text("extension", "templates/html/head.html") ?? "";
//!     const paths = Object.fromEntries(data.artifacts);
//!     const regions = Object.fromEntries(data.regions);
//!     return `<!doctype html>\n<html><head>${head}<link rel="stylesheet" href="${paths.style}"></head>\n` +
//!       `<body><h1>${data.config.heading}</h1>\n${regions.content}</body></html>\n`;
//!   },
//! };
//! ```
//!
//! - `describe(settings)` runs inside the feature's `register`, given the
//!   feature's configuration, and answers `{ output, accepts, requires,
//!   config }` (`accepts` `"all"` or a list of piece names; `requires` and
//!   `config` optional): the compose template the binding contributes, with
//!   an engine of its own that calls `files` and `render`.
//! - `files(data, cx)` answers the files the template ships, each `{ name,
//!   extension, media_type, text }` (a text it computed) or `{ name, file,
//!   scope, media_type }` (a file of the extension's folder or of the
//!   document's folder, read by the binding; `media_type` optional, given
//!   by the file's extension otherwise). The program names them and writes
//!   them beside the page.
//! - `render(data, cx)` answers the page, a text. `data` is `{ html, regions:
//!   [[name, text], …], pieces: [{ name, place, text, words }, …], meta,
//!   config, artifacts: [[name, path], …] }`, `artifacts` holding the paths of
//!   the shipped files (empty for `files`); a number of `meta` or `config`
//!   that is not an integer is refused, the refusal naming its key path.
//!
//! The shipped files of the two scopes and the reads of `cx.host("files")`
//! need `host: ["files"]` in the package object. A failure of `describe`
//! fails the feature's `register`, hence the registry build; a failure of
//! `files` or `render` is the template's failure: the Error
//! `flm.compose.template-failed`, whose text holds "flm.ext.handler-failed:
//! the template module `NAME` of the feature `F` … failed: TEXT", and the
//! output is not made.
//!
//! # The API modules
//!
//! Each module of [`crate::api`] that holds items a script can reach is a
//! JavaScript module under `flm:` (rule 1 of the mapping rules). The table
//! lists each module, the module of [`crate::api`] it mirrors, and its
//! exports:
//!
#![doc = include_str!("api_modules.md")]
//!
//! **How the modules are made.** The JavaScript source of these modules (the
//! *prelude*, under `src/quickjs/js/api/` in this crate) is generated from the
//! Rust sources under `src/api/`, the sources of [`crate::api`]. One
//! generator writes, from the same reading of those sources: the prelude; the
//! table above; the declarations file `flm.d.ts` ([Editor
//! support](#editor-support)); and the part of the dispatcher
//! [`call`](crate::adapter::dispatch::call) that serves each method and free
//! function by name. The generator is the test `tests/api_surface.rs` of this
//! crate: run as `UPDATE_GOLDEN=1 cargo test -p flm-ext --test api_surface`,
//! it rewrites every generated file; run without the variable, it fails when
//! a generated file differs from what the sources give. Each generated file
//! says so in its first lines, with the Rust file it was generated from, and
//! is never edited by hand.
//!
//! **How a method reaches Rust.** A class of a module stands for a Rust
//! object or a Rust data type. A method of a class that stands for an object
//! (rule 4) forwards: it sends its name and its arguments, as JSON, to the
//! dispatcher, which calls the Rust method of the same name on the object the
//! instance's token designates and answers the method's result. A free
//! function forwards the same way on its module. Neither computes anything
//! itself. The comment above a generated item in the prelude is the first
//! sentence of the mirrored Rust item's documentation.
//!
//! **What stays hand-written.** Three helper modules: `flm:_runtime` (the
//! base class of the handle classes, the class `FlmError`, the forwarding
//! call, and the record of the objects a script hands to FLM), `flm:_entry`
//! (the entry points the binding calls, which import every API module), and
//! the sandbox script ([The sandbox](#the-sandbox)). And the fragments under
//! `src/quickjs/js/hand/`, which the generator copies into the generated
//! modules where forwarding does not apply:
//!
//! - the methods that hand a script's object to FLM: `FeatureBuilder.stage`,
//!   `FeatureBuilder.rewrite_stage`, `FeatureBuilder.attach`,
//!   `FeatureBuilder.late_init`, `FeatureBuilder.finalizer`,
//!   `ElementSpec.with`, `UserDefinition.element_def_with`, the constructors
//!   of `LoweringRule` and `BackendDef` (the latter for its stage objects),
//!   and `FeatureBuilder.contribute_to`, `contribute_first_to` and
//!   `peer_slot` (the slot's name decides: any object given to
//!   `flm.citations.providers` is a citation provider, any object given to
//!   `flm.compose.templates` a template module, and a value given to any
//!   other slot is data), which record the object so that another engine
//!   instance finds it again ([The rehearsal](#the-rehearsal));
//! - `FeatureBuilder.feature`, which answers the registering feature's name
//!   during a rehearsal without calling Rust;
//! - the classes of the `flm-core` data types `ArgDef`, `BodyDef`,
//!   `Definition`, `StyleDef`, `ScriptDef`, `OutputDef`, `ArtifactId` and
//!   `Artifact`, whose fields and builder methods are `flm-core`'s and have
//!   no source under `src/api/`, with the function `arg_def_from_code`,
//!   which answers an `ArgDef`;
//! - the classes of the data types re-exported from `flm-core` without a
//!   source under `src/api/`: in `flm:model/aspects`, `RefPoint`,
//!   `NumberSource`, `AnchorPolicy`, `ContentRef` and `AcceptedRefTypes`; in
//!   `flm:stdcore/features/numbering`, the declarations `CounterDecl`,
//!   `CounterStyleDecl`, `ParentSetDecl`, `Within`, `PluralWords`,
//!   `PrefixWords`, `Delimiters`, `NumberTemplate` and `FormatOptions`; in
//!   the LaTeX modules, `LatexPreambleDef`, `LatexPackageDef` and the
//!   payloads of the LaTeX construct elements;
//! - the class `ElementDef`, which holds either a declaration as data or a
//!   declaration a helper made, the class `DefinitionBindings`, the class
//!   `LoweringStage`, whose constructor answers a `LibraryStage`, and the
//!   classes `TemplateSpec` (data), `TemplateLanguage` and `CompiledTemplate`
//!   (durable objects, rule 4) and `ValueBindings`. Rust's
//!   `ValueBindings::with` consumes the bindings and answers new ones;
//!   `ValueBindings.with(name, node)` changes the instance it is called on
//!   and answers that instance, so `new ValueBindings().with("number", root)`
//!   reads as in Rust.
//!
//! An argument crosses as its argument code (the codes
//! [`arg_def_from_code`](crate::api::lang::mint::arg_def_from_code) reads),
//! never as a parser object: `new ArgDef(code)` and `ArgDef.named(name, code)`
//! take a code where Rust's `ArgDef::new` and `ArgDef::named` take a parser
//! kind; `ArgDef.mandatory(name)` and `ArgDef.optional(name)` are as in Rust.
//! `ArgDef.mode(mode)`, `ArgDef.scoped(name)`, `BodyDef.scoped(name)` and
//! `Definition.extra_placeholder(name, substitution)` are builder methods as
//! in Rust. Not offered, since the data shape of a declaration cannot carry
//! a parser object: `BodyDef::parser`. `FixedBlockRole` is not
//! a class: the aspect `{ BlockRoleAspect: role }` holds the block role
//! itself.
//!
//! # Editor support
//!
//! The file `flm.d.ts`, at the root of the `flm-ext` crate in the
//! repository, declares every module under `flm:` in TypeScript: each class
//! with its constructor and methods, each trait an extension implements as an
//! interface, each enum and constant with its values, and the shape of the
//! data that crosses (a diagnostic, a span, a node's address). A
//! declaration that mirrors a documented Rust item carries the first sentence
//! of its documentation. An editor that reads TypeScript declarations completes the
//! names a script imports from `flm:` modules, shows each item's sentence,
//! and reports a misspelled method or a wrong number of arguments, once a file
//! `jsconfig.json` beside the extension's manifest points at the
//! declarations (replace `PATH` with the path of `flm.d.ts`;
//! `"strict": false` keeps the checker from reporting every function
//! parameter that has no declared type):
//!
//! ```json
//! { "compilerOptions": { "checkJs": true, "strict": false, "module": "es2022",
//!                        "moduleResolution": "bundler" },
//!   "files": ["PATH/flm.d.ts"], "include": ["**/*.js"] }
//! ```
//!
//! The declarations follow the mapping rules above: `Option<T>` is
//! `T | null`, `Result<T, E>` is `T`, and data that crosses as free JSON (a
//! table's item, an option's value, a presentation payload) has the type
//! `any`. They describe the
//! API of the `flm-ext` version written in the file's first line: an
//! extension is checked against the file of the version it runs on. The file
//! declares the `flm:` modules only, not the modules a program offers. FLM
//! itself reads neither `flm.d.ts` nor `jsconfig.json`: the checks are the
//! editor's.
//!
//! # The sandbox
//!
//! An engine instance reaches nothing outside itself: no timers, files, network or
//! process (`setTimeout`, `fetch`, `require`, `process` are undefined); the
//! host services its package declares are the one way out ([Host
//! services](#host-services)). The
//! two ambient sources of difference are fixed: `new Date()`, `Date()` and
//! `Date.now()` answer the epoch (`new Date(x)` and the parsing and formatting
//! methods are unchanged), `performance.now()` answers `0`, and
//! `Math.random()` is a fixed sequence restarted at every handler call.
//! `console.log`, `info`, `debug`, `warn` and `error` write their arguments,
//! joined by spaces, to the host's [`ConsoleSink`] (standard error by
//! default), not to the run's diagnostics. `eval` and `Function` stay: they
//! reach nothing more than the script already does.
//!
//! # The limits
//!
//! | Limit ([`JsSettings`]) | Default | Exceeded |
//! |---|---|---|
//! | `time_budget`, per handler call | 10 s | the script is interrupted (it cannot catch it); the handler fails; the engine is replaced after the call |
//! | `memory_limit`, per engine instance | 64 MiB | the handler fails; the engine is replaced after the call |
//! | `stack_size`, per engine instance | 1 MiB | a `RangeError` the script may catch; uncaught, the handler fails; the engine is kept |
//!
//! An extension may set its own limits in its manifest, under
//! `extension.js.limits` (`memory_mib`, `stack_mib`, `time_s`;
//! [`JsLimits`](crate::manifest::JsLimits)): each limit given replaces the
//! host's for every engine instance of that extension
//! ([`JsSettings::with_limits`]). The `math-mathjax` extension of the FLM
//! repository sets `stack_mib: 16` and `memory_mib: 64`, which MathJax needs,
//! and `time_s: 120`, a 120 s time budget per call, because one call converts
//! every formula of a document.
//!
//! A nested handler call (a hook run while another hook's `instantiate`
//! runs) has a budget of its own, and the outer call's resumes after it.
//! Creating an engine instance (evaluating the extension's modules and
//! calling its default export) has the time budget too.
//!
//! # Strict mode
//!
//! With [`JsSettings::strict`], every outermost handler call runs on a fresh
//! engine instance, dropped after the call: module-level state never
//! survives a call. It is meant for tests of an extension, to show that nothing it computes
//! depends on what an earlier call left behind.
//!
//! # The rehearsal
//!
//! The package objects, and the objects a feature hands to FLM (a stage, an
//! expansion hook, a rule's body, a rewrite stage, a handler aspect, a
//! late-initialization hook, a finalizer, a citation provider, a template
//! module), are JavaScript objects of the instance that made them. An
//! instance other than the loading one calls the default export once when it
//! is created, so that it holds the package objects. When an instance that
//! has not run a feature's `register` with the configuration of the
//! registration at hand must run one of the feature's handlers, it first
//! runs `register` again, with that configuration and with a builder that
//! declares nothing (its `feature()` answers the feature's name; every other
//! method records or does nothing), and then each finalizer that `register`
//! registered, with such a builder too: a *rehearsal*, after which the
//! instance holds the objects. A stage, a hook, a rule's body and a rewrite
//! stage are found again by name; a citation provider and a template module
//! by their `name`; a handler aspect by its element and its kind
//! (`Referenceable` or `Sectioning`); a late-initialization hook and a
//! finalizer by their position among the feature's calls of `late_init` and
//! `finalizer`. A template module's `describe` is the exception: it runs
//! inside the `register` that contributes the module, on that instance, and
//! finds the module among what that `register` recorded so far. The
//! constructors and free functions still run in a rehearsal, so `register`
//! sees the same values; two registries built from one package with two
//! configurations each get their own rehearsal. Hence the contract: the
//! default export declares the same packages every time it runs, and a
//! feature's `register` (and each of its finalizers) declares the same
//! things under the same names, in the same order, every time it runs with
//! one configuration. A finalizer's code therefore runs once per engine
//! instance that rehearses the feature, besides the build's own call.
//!
//! Engine instances and runs: the library runs each batch of stages on a
//! thread of its own unless the calling thread is one of its marked worker
//! threads, so one render may create a fresh instance per batch (evaluating
//! the modules and rehearsing the feature again), and with the library's
//! `parallel` feature one run's batches land on different threads'
//! instances. Module-level state
//! of a script is therefore read-only after the module loaded: it is never a
//! channel between handler calls, nor between runs.
//!
//! # Failures
//!
//! | Failure | What the handler's caller gets | What the library does |
//! |---|---|---|
//! | an exception escapes a handler | the failure's text: an `Error`'s message, any other value by `String()`; the data hold the error's `name`, its `stack`, and for an `FlmError` its data under `flm_error` | `register`: the registry build fails; a stage: the Error `flm.ext.handler-failed` at the stage, what it pushed before kept; a hook: the use site kept as its source text, with that Error |
//! | an exception escapes a lowering rule's body, or its answer is not a `Lowering` | as above | the Error `flm.ext.handler-failed` at the node, whose message names the rule; a childless `flm.pres.unsupported` stand-in in the node's place, the rest of the tree lowered |
//! | an exception escapes a rewrite stage's `restage`, or its answer is not a `Restage` | as above | the Error `flm.ext.handler-failed` at the node, whose message names the stage; the node copied as if the handler had answered `Restage.Descend`, the rest of the tree rewritten |
//! | a method of `rs` meets a refusal after which the tree cannot be rewritten (content nested too deeply; a refusal of the new tree's builder; an argument's content inside a node the handler removed) | a thrown `FlmError`, kind `"core"`, which the handler may catch | the tree's rewrite is abandoned whatever the handler answers: the Error `flm.ext.rewrite-failed` at the tree's root, or `flm.blocks.nesting-too-deep`; the tree is absent from the output |
//! | the library meets such a refusal after the handler answered, while it places the answer or finishes the tree (the tree's root replaced by other than one node; an argument's content inside a node the handler removed; a refusal of the new tree's builder) | nothing: the handler has returned | the same: the tree's rewrite is abandoned, the Error `flm.ext.rewrite-failed` at the tree's root, the tree absent from the output |
//! | a limit is exceeded, by the handler's call or by a job it queued | the limit's text, the data `{ "limit": "time" \| "memory", … }` | the same |
//! | the engine instance cannot be created (on a thread other than the loading one) | the reason | the same |
//! | a stage, a hook, a rule's body or a rewrite stage is not found after the rehearsal | "the stage `NAME` must be declared by `register` of the feature `F` of the package `P` of the extension `S`" ("the rewrite stage `NAME` …" for a rewrite stage); for a handler aspect, "the reference points of the element `E` must be answered by an aspect attached by `register` of …" (or "the sectioning of the element `E` …"); for a build hook, "the late-initialization hook N must be registered by `register` of …" (or "the finalizer N …") | the same |
//! | `register` contributes two citation providers, or two template modules, under one name | "the citation provider `NAME` must be contributed once by `register` of the feature `F` of the package `P` of the extension `S`" (likewise "the template module `NAME` …") | as for any failure of `register` (the first row) |
//! | a citation provider or a template module is not found after the rehearsal (a second run of `register` declared another name) | "the citation provider `NAME` must be contributed by `register` of the feature `F` of the package `P` of the extension `S`" (likewise "the template module `NAME` …") | as the two rows below |
//! | a citation provider fails (an exception, a limit, an answer of another shape, a provider not found) | — | every key of the call answers "flm.ext.handler-failed: the citation provider `NAME` of the feature `F` … failed: TEXT", reported by the retrieve stage at each key's first citation; those keys have no entry |
//! | a template module's `files` or `render` fails | — | the Error `flm.compose.template-failed` holding "flm.ext.handler-failed: the template module `NAME` of the feature `F` … failed: TEXT"; the output is not made (a failing `describe` fails `register`, as in the first row) |
//! | an exception escapes a handler aspect (`points`, `level`, `in_toc`, `toc_title`), or its answer cannot be read | — | nothing is reported (the aspect methods of the library take a node and answer a value, with no place to report to): the aspect answers as if the element carried none, so the occurrence has no reference point (a `\ref` to it is unresolved, and it has no number and no anchor), or the level 0, is not listed in the table of contents, and has no title |
//! | an exception escapes a late-initialization hook or a finalizer | the failure's text | the registry build fails, as when `register` fails |
//! | a durable object is used under another registry than the one it was made under | a thrown `FlmError` of kind `"stale_durable"`, whose data hold the token under `durable` | uncaught, the same as an exception |
//! | a call of the API fails (a stale object, a bad argument, a value that cannot cross, a name outside the extension's stem) | a thrown `FlmError` the script may catch | uncaught, the same as an exception |
//! | the handler calls `import()` | the refusal's text, the data `{ "refusals": [...] }`, even when the script caught the rejection | the same |
//! | a promise is left rejected with no handler (a callback or an `async` handler throws) | "WHAT must leave no promise rejected without a handler: MESSAGE", the data `{ "rejections": [...] }` | the same |
//! | the extension does not load (a module that does not parse, an import refused, a default export that fails, a package object that is not one) | — | every problem reported where the `use` entry stands; no package of the extension is used |
//!
//! The memory limit is held by the instance's allocator, which marks the
//! instance when it refuses an allocation. A failure is read as the limit
//! reached only after such a refusal during the call: QuickJS then throws
//! its out-of-memory error, or `null` when it cannot allocate that error
//! either. A script's own `throw null` or `Promise.reject(null)` is an
//! ordinary exception.
//!
//! # The digest
//!
//! What the registry digest receives for every feature of a JavaScript
//! extension, after the manifest: the bytes `flm-ext quickjs ` and
//! [`ENGINE_VERSION`]; then, for each recorded module in path order and for
//! each module the host offers in name order, a NUL byte, the path or name,
//! a NUL byte, the text; then a final NUL byte. The host's settings do not
//! enter: they change whether a handler fails, not what a succeeding handler
//! answers.

mod engine;
mod glue;
mod host;
mod js_engine;
mod memory;
mod modules;
mod package;

pub use crate::host::{ConsoleLevel, ConsoleSink};
pub use host::{HostModuleError, JsHost, JsSettings};
pub use js_engine::JsEngine;

#[doc(hidden)]
pub use engine::{eval_for_test, set_test_packages, TestPackage};

/// The engine's version, as it enters the digest of every JavaScript
/// extension.
///
/// Keep it in step with the `rquickjs` line of `flm-ext/Cargo.toml`: rquickjs
/// exposes no version at run time.
pub const ENGINE_VERSION: &str = "rquickjs 0.14.0";

/// The message QuickJS throws when an engine exceeds its memory limit
/// (`quickjs.c:8559` in rquickjs-sys 0.14.0, `JS_ThrowOutOfMemory`): the one
/// fixed text of the engine the binding reads. Check it with
/// [`ENGINE_VERSION`].
pub(crate) const OUT_OF_MEMORY_MESSAGE: &str = "out of memory";
