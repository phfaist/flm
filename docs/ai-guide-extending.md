# AI guide: extending FLM

This page is a condensed reference for an agent that adds functionality to FLM: a
feature (elements and their syntax, aspects, tables, stages, capabilities, lowering
rules, options, services, build hooks), a backend and its output, a lowering rule for
another feature's element, or an extension of one of the standard features (numbering,
heading kinds, pieces (document-level values a backend places beside the content),
block-transparent elements, stylesheets, LaTeX packages and
definition pieces). It gives the exact items, signatures and refusals; the public guide
[Extending FLM](crate::guide::extending) covers the same ground with the reasoning, and
[Rendering](crate::guide::rendering) describes lowering and the backends in full.

Load it together with the entry page, [AI guide](crate::guide::ai_guide), whose
[vocabulary](crate::guide::ai_guide#vocabulary) gives the canonical definition of every
term used here (feature, registry, element, aspect, stage, table, capability, lowering
rule, peer declaration, service slot, piece, and the others).

Everything the library can do comes from features: a feature
([`FlmFeature`](crate::core::registry::FlmFeature)) is a Rust value that contributes a
related bundle of things to a registry (elements, stages, tables, lowering rules,
backends, services, settings) through the feature builder. The standard features in
[`crate::stdlib`] register through the same interface as a feature of yours and use no
method that your feature cannot call, so their source is a worked example of every
extension point (see [The standard features as worked
examples](#the-standard-features-as-worked-examples)).

## A minimal feature, end to end

One element (`\shout{…}`), one lowering rule (a bold span), registered beside the
standard features and rendered through the `text` workflow. Every extension follows this
shape: an element value, its syntax, and a rule.

```rust
use std::sync::Arc;

use flm_core::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use flm_core::model::element::ElementSpec;
use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_core::model::names::ElementName;
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry, RegistryBuilder, RegistryInputs,
};
use flm_core::lang::Flm;
use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_core::stdlib::backends::pres::{self, SpanPayload, SpanStyle};
use flm_core::techy::core::node::NodeRef;

const SHOUT: &str = "shout.shout";
const SHOUT_RULE: &str = "shout.shout.lower.default";

struct ShoutRule;

impl Lower for ShoutRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let body = lw.lower_arg_named(cx, &node, "text")?;
        let payload = SpanPayload { style: Some(SpanStyle::Bold), ..SpanPayload::default() };
        let span = lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)?;
        Ok(Lowering::Emit(vec![span]))
    }
}

struct Shout;

impl FlmFeature for Shout {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("shout", "Shouting").docs("The `\\shout` command.")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();

        // What a `\shout` node means; its block role says it is inline content.
        let element = Arc::new(
            ElementSpec::new(SHOUT)
                .docs("Loud inline content.")
                .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT))),
        );

        // How it is written: `\shout` with one mandatory argument named `text`.
        b.element(ElementDef::new(
            owner,
            ["shout"],
            SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] },
            element,
        ))?;

        // How it renders: the default rule for this element, on every backend.
        b.lowering_rule(LoweringRule::new(
            SHOUT_RULE,
            SHOUT,
            LoweringDecl::new().produces(pres::SPAN),
            Arc::new(ShoutRule),
            DefaultMark::Everywhere,
        ))?;

        Ok(())
    }
}

let mut builder = RegistryBuilder::new();
builder.use_package(flm_core::stdlib::add_features(PackageEntry::new("std"), flm_core::stdlib::StdlibConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
builder.feature(Arc::new(Shout));
let registry = builder.build(&RegistryInputs::new())?;

# use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
# use flm_core::core::plan::{Overrides, Plan};
# use flm_core::core::run::{Externals, Run, RunOptions};
# use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
# use flm_core::stdlib::backends::text::{self, TEXT_OUTPUT};
let source =
    Arc::new(FlmSource::new("Please \\shout{listen} now.").with_origin(FlmOrigin::none()));
let fragment =
    parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())?;
let mut content = DocContent::new();
content.place(Arc::new(fragment));

let plan = Arc::new(Plan::for_workflow(&registry, text::WORKFLOW, &Overrides::none())?);
let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none())?;
run.execute(&RunOptions::new())?;

// Plain text cannot show boldness, so the emitter prints the content alone.
assert_eq!(run.table(TEXT_OUTPUT)?.expect("written").text, "Please listen now.\n");
# Ok::<(), Box<dyn std::error::Error>>(())
```

The three parts stay separate in every feature: the **element**
([`ElementSpec`](crate::model::element::ElementSpec)) says what a node means; the **element declaration** ([`ElementDef`](crate::lang::mint::ElementDef))
says how the construct is written; the **lowering rule**
([`LoweringRule`](crate::core::lowering::LoweringRule)) says how it renders.
`ElementDef::new` takes the owner's name from
[`FeatureBuilder::feature`](crate::core::registry::FeatureBuilder::feature), which
returns the name the feature was registered under.

**A Rust extension, end to end.** The extension `phfpack` (folder
`extensions/rust/phfpack/`, its code the module `flm_extensions::phfpack` of the crate
`flm-extensions` under the cargo feature `phfpack`; the default package `_`, so
`use: [phfpack]`; the feature `phfqit`) is written against `flm_ext::guest` alone:
`flm-ext` with `default-features = false, features = ["guest"]`, plus `serde`,
`serde_json` and `schemars` for its settings. The paths below are under
`flm_ext::guest`, whose modules mirror flm-core's under the same paths.

- *Package*: `pub static FLM_EXTENSION: CompiledExtension = CompiledExtension::new(define,
  "phfpack", env!("CARGO_PKG_VERSION"))`, its row of the crate's table of code;
  `define` calls `ext.package("_",
  Package::new().flm(json!({ "feature_order": ["math-mathjax < phfqit"]
  })).feature(feature::FEATURE, Feature::new().title(…).docs(…).settings(feature::settings()).read(settings::read).register(feature::register)))`,
  `feature::FEATURE` being `"phfqit"`.
- *Macros as data*: rows of tables (name, arguments, argument number offset, template,
  modes, defaults) and the macros declared under the setting `macro_definitions`, each
  turned into a guest `Definition`
  ([`Definition`](crate::lang::rewrites::userdefs::Definition)): a row's codes read by
  `ArgDef::from_code(code)?` or `ArgDef::named_from_code(name, code)?`, a size slot
  declared by its kind (`ArgDef::new(ArgStdKind::Star)`); a declared argument (a code, or a map with `code` or a
  structured `syntax`, as a user definition's `args`) held in the settings types as the
  guest's `lang::mint::ArgDef`, which serde reads as
  [`ArgData`](crate::lang::mint::ArgData) and schemars describes with `ArgData`'s schema;
  the list of `delimited_args` held as the guest's `lang::mint::ArgListData`, read as
  [`ArgListData`](crate::lang::mint::ArgListData) and described with its schema
  (`arg_defs()` reads the codes of one text);
  a declared `args` entry may also be a
  size slot's name (`SizeArgBacktick`/`SizeArgStar`/`SizeArgOptArg`), and
  `delimited_args` puts the three slots first (offset +3, as a row built by `delimited`);
  four extra placeholders (`DelimLeft`, `DelimMiddle`, `DelimRight`, `config`, written
  `#[…]{name}`) by `Definition::extra_placeholder(name, true)`; a `content` of
  `{text, math}` gives one definition per mode.
- *Size slots*, declared by their standard kinds (the codes in parentheses):
  `Embellishments { markers: vec!['`'] }` (``e{`}``; a size after a backtick: the
  [`Embellishments`](crate::lang::mint::ArgStdKind::Embellishments) argument whose marker
  matches the raw backtick even where the backtick is a specials trigger; the size is the
  text of the children of its wrapper group), `Star` (`*`), `Optional` (`[`). The backtick
  form (specials `` ` ``, formulas only) declares the
  [`Marker`](crate::lang::mint::ArgStdKind::Marker) arguments `\Bigg`, `\Big`, `\bigg`,
  `\big` (`t{\Bigg}`, …; longest first: a marker matches the raw characters), `Star`,
  then `AnyDelimited`; a size that matches no marker
  (`` `\huge(x) ``) leaves the group missing, a parser error. The hook reads a slot with
  `node.argument_content_nodes(i)?` (`Some` when written) and `span_content()?`; a bad
  size is `phfpack.phfqit.bad-size`.
- *Register* `(b, settings)`: per macro, `m.definition.expand(ExpandPolicy::Always)`,
  `element_name_under(PREFIX, &definition)?` (`PREFIX` = `"phfpack.phfqit"`),
  `b.lowering_rule(expand_in_tree_rule_for(&element, &definition)?)?` (requires
  [`ALWAYS_EXPANDED`](crate::stdlib::features::userdefs::ALWAYS_EXPANDED): the
  `userdefs` feature is necessary), `let value = UserDefinition::new(definition)?;
  b.element(value.element_def_with(&owner, &element, hook)?)?` with `owner =
  b.feature()?` and `hook: Arc<dyn Expand>` (`MacroHook`: the size slots and the named
  values); the order is fixed (table rows, minus those a declared macro replaces, then the
  declared macros by name). No late-init hook: the extension interface compiles the
  registered definitions' templates once the language is built.
- *The hook at each use*: `cx.definition()?` (a `UserDefinition` handle of `register` is
  stale in a later call); `.variant_for_use(node)`, whose `Err(ExtError::Core { kind,
  detail, data })` of kind `flm.userdefs.bad-definition` is reported with
  `xp.recover(cx, node, Diagnostic::error(kind, detail).with_data(data))`; the keys from
  `placeholders()` of `variant.template()?` and of each template of
  `variant.defaults()?` (`PlaceholderTarget::Extra(name)`, `reference.substitution`); each
  text (the delimiter at the use's size, or the named value; an unknown one is
  `phfpack.phfqit.unknown-value` through `xp.recover`) compiled in the use's mode by
  `compile_template_with(&spec, &lang, &language)`, `lang` being `xp.lang()?` and
  `language` `TemplateLanguage::new(&lang)?` (durable, deduplicated by its specification;
  an `ExtError::Core` of kind `compile_template` means the text does not compile in that
  mode: left out, so the placeholder is
  `flm.userdefs.bad-placeholder` at the use); from `ExtraValues::new()`,
  `values.template(name, substitution.as_deref(), compiled)`
  ([`ExtraValues`](crate::lang::rewrites::userdefs::ExtraValues));
  `values.over(&DefinitionBindings::new(node, &variant)?)?`; `xp.instantiate(cx, node,
  &template, &bindings)`.
- *Settings as data*: `Feature::settings` of the value `{config: {schema, docs}}`, the
  schema by `schemars::schema_for!` over the crate's own types, with the marks written on
  its nodes (`x-flm-marks`: `merge-object-key-values` and
  `remove-key-when-value-is-false` on the four keys, whose `default` gives the default
  entries; `markup` on the templates and named values), registered as
  [`SettingsDecl::config_data_read`](crate::core::settings::SettingsDecl::config_data_read),
  which reads the marks from the nodes; `Feature::read(settings::read)`,
  the configuration reader, reads the configuration value key by key (each declaration
  with serde), and answers it unchanged or a `ReadError` (the path of the value at fault
  below the feature's name, `macro_definitions.abs.args`, and the condition it violates),
  which the program reports where the document wrote the value, with no place when the
  document's settings do not name the feature; the program calls the reader whenever it
  reads a document's settings, for every feature of a package the document uses that
  declares a reader and is not switched off in the document's settings, with the
  configuration written under the feature (`{}` when nothing is written under the
  feature, or none of the configuration's own keys), before the registry is built;
  `register` receives the reader's answer and reads it the same way. A feature in
  JavaScript gives `read(settings)`, answering `{ value }` or
  `{ error: { path, detail } }`.
- *Digest*: `b.digest_input(&digest)`, `digest` being `data.digest_bytes()`, the
  settings as binary data (counts as little-endian 64-bit numbers, map keys sorted);
  bytes pass as they are.
- *Two forms*: compiled in (its rows in `flm_extensions::EXTENSIONS` and
  `flm_extensions::COMPILED_EXTENSIONS`, which the build script writes; the `flm`
  program's cargo feature `bundled-extensions` gives both tables to `flm-host`; the tests
  load it with `ExtensionLoader`, `NativeEngine::new([NamedExtension { stem, compiled }])`
  and a `MemorySource` answering `phfpack` with `engine: native`), and the module of
  `flm-extensions` built with the feature `phfpack` alone, which
  `extensions/rust/build-wasm.sh phfpack` writes, with the extension's files, to
  `target/wasm-extensions/phfpack/`.

The public guide describes it under [A Rust extension, end to
end](crate::guide::extending#a-rust-extension-end-to-end); the crate root
`extensions/rust/phfpack/src/lib.rs` lists the guest items each step uses.

## Where the items are

| subject | path |
|---|---|
| feature interface, feature builder, registry build, packages | [`crate::core::registry`] |
| declaration types (capability, collection, ordering edge, backend, native-set implication, output, workflow, style and script definition, build hooks) | [`crate::core::registry::entities`] |
| element declarations and their syntax | [`crate::lang::mint`] |
| element values and aspects | [`crate::model::element`], [`crate::model::aspects`] |
| tables | [`crate::core::data::table`] |
| stages | [`crate::core::stage`] |
| what a stage reads and writes at run time | [`StageCx`](crate::core::run::StageCx), [`StageOut`](crate::core::data::store::StageOut) |
| lowering rules | [`crate::core::lowering`] |
| presentation vocabulary | [`crate::stdlib::backends::pres`] |
| the shared lowering stage and its driver | [`crate::stdlib::backends::lowering`] |
| the LaTeX family: construct elements, escaping, the typed sink, package requests, definition pieces, the style file, the names of recomposed user definitions | [`crate::stdlib::backends::latex`] |
| the HTML backend: escaping, already-formed markup, the typed sink, the naming functions and the conventions table of the HTML contract, the address policy, the tag construct element, the emitter, the stages and the page | [`crate::stdlib::backends::html`] |
| option types, settings, services | [`crate::model::options`], [`crate::core::settings`], [`crate::core::services`] |
| names | [`crate::model::names`] |
| the standard features | [`crate::stdlib`], [`crate::stdlib::features`], [`crate::stdlib::backends`] |
| a feature written in JavaScript: the package and feature objects, settings from data, imports, the stem rule | [Extensions](crate::guide::extending#extensions); the module page of `flm_ext::quickjs` (sandbox, limits, how each item appears in JavaScript; "Editor support": the TypeScript declarations `flm-ext/flm.d.ts` and the `jsconfig.json` an editor checks a script with) |
| reading nodes, trees and table rows from an extension, and passing content back: traversals (`walk`, `descendants`, `trees`) with `elements(names)` and `kinds(kinds)`, the sequences `Nodes`, `StagedNodes`, `Rows`, joined lists, `[...nodes]` and `to_vec()` | [Traversals and sequences in an extension](#traversals-and-sequences-in-an-extension); [Traversals and sequences](crate::guide::extending#traversals-and-sequences); the module pages of `flm_ext::quickjs` and `flm_ext::guest` (mapping rule 9) |
| a feature in JavaScript that lowers: `new LoweringRule(name, element, decl, body, mark)`, the lowerer `lw` (`lower_arg_named`, `lower_nodes`, `get_at`, `pres_tree`), presentation trees as data, `StyleDef`/`ScriptDef` with `DefContent.Table`, `definition_text_table`, `peer_backend`/`peer_table`/`peer_element`, `table_refining`, `flm:stdlib/features/math`, the manifest's `extension.js.limits` (`memory_mib`, `stack_mib`, `time_s`) | [What a feature in JavaScript adds to the output](crate::guide::extending#what-a-feature-in-javascript-adds-to-the-output); the module page of `flm_ext::quickjs`; `flm-ext/flm.d.ts` (every `flm:` module's declarations: the lowerer's methods, the presentation names, the `math` library, with each item's first sentence); the extension `extensions/js/math-mathjax/` as the worked example |
| host services for a feature in JavaScript: the package object's `host` list, `cx.host(name)` on a stage's, a citation provider's and a template module's context, the service `files` (`read_text(path)`, a qualified path: `self:` the extension's folder, `ext+NAME:` an extension the document uses, `std:`, `doc:`, `cwd:`; a bare path and `https:` refused; `path(import.meta, "./x")` of `flm:host` gives `self:` and the module's folder joined with `x`), the service `download` (`call("get", { url })` → `{ content: {text}\|{base64}, media_type, warning? }`, `https` only, kept seven days in `flm.host.download.files`, offered when the host gives a downloader; `flm_ext::host::download`), the `flm` program's services `cache` (`get`, `put`, `remove`) and `programs` (`find`, `run`, with its argument `cache`), the package object's `caches` (the declared caches, `flm_ext::host::CacheDeclaration`, Rust guest `Package::caches`), the refusal at load of an undeclared or unoffered service, the trust rule (an extension may run any program through `programs`, and a build may contact any server that the document or its packages name); for a host, `ExtensionLoader::with_host_service` and the trait `flm_ext::host::HostService` | [Host services](crate::guide::extending#host-services); [Host services for extensions](crate::guide::embedding#host-services-for-extensions); [Caches of an extension](crate::guide::extending#caches-of-an-extension); the module pages of `flm_ext::host`, `flm_ext::paths`, `flm_host::host_files`, `flm_host::host_download`, `flm_host::host_cache` and `flm_host::host_programs`; the module page of `flm_ext::quickjs` ("Host services") |
| a citation provider written in JavaScript: the object (`name`, `answer_all` or `answer`) contributed to `PROVIDERS_SLOT` (`flm.citations.providers`), the request and answer shapes, `cx.build_entry`, `cx.parse_entry`, what a failure gives; service slots of a script's own (`service_slot`, `contribute_to`, `peer_slot`, `cx.service`, `cx.services`) | [A citation provider from a script](crate::guide::extending#a-citation-provider-from-a-script); [Services and service slots](crate::guide::extending#services-and-service-slots) ("In JavaScript"); the module page of `flm_ext::quickjs` ("A citation provider"); the extension `flm-ext/tests/js/bibfile/` as the worked example |
| a compose template written as a JavaScript module: the object (`name`, `describe(settings)` → `{ accepts, requires?, config }`, `config` the default of every declared setting and no other key, `files(data, cx)` → `{ name, extension, media_type, text }` or `{ name, file, media_type? }` with `file` a qualified path (an `https` file through the service `download`), `render(data, cx)`) contributed to `TEMPLATES_SLOT` (`flm.compose.templates`); its declaration under the key `templates` of the feature object, `{ NAME: { output, settings } }`, `settings` a JSON Schema of an object with marks and no `default` (declared as the entry `template_config.OUTPUT.NAME` of `compose`, `compose::declare_template_settings`; an undeclared key is left out with a warning); `path(import.meta, "./head.html")` for a file beside the module; the shapes of `data`; the composer's hook `TemplateEngine::artifacts` through which an engine chooses a template's files when it is applied | [A compose template written as a module](crate::guide::extending#a-compose-template-written-as-a-module); [`TemplateEngine::artifacts`](crate::stdlib::features::compose::TemplateEngine::artifacts); the module page of `flm_ext::quickjs` ("A compose template written as a module"); the extension `flm-ext/tests/js/tinytpl/` as the worked example |
| a feature in JavaScript with numbered and referenceable constructs: handler aspects `Aspect.Referenceable({ points })` and `Aspect.Sectioning({ level, in_toc?, toc_title? })` (in `ElementSpec.with` or `b.attach`; a failing handler is silent: no point, level 0, not listed), `flm:model/aspects` (`RefPoint`, `NumberSource`, `AnchorPolicy`, `ContentRef`, `AcceptedRefTypes`), `flm:stdlib/features/refs` (label helpers, `lw.get(ANCHORS)` → `{ anchors }`), `flm:stdlib/features/numbering` (`lw.get(COUNTERS)` → `{ counters }`: `record`, `format`), the standard data-valued slots through `contribute_to`/`contribute_first_to`/`peer_slot` (counters, styles, parent sets, `latex.preamble.SLOT`, `latex.packages.SLOT`: the data read as the slot's type, its own name under the extension's stem), `flm:stdlib/backends/latex` (payload classes, `lw.pres_slots`, `lw.require(piece_id)`), `scoped_package`, `collection_member` (case: `wordcount.counted` in `flm.validated`), `late_init`/`finalizer` objects, templates: `compile_template_with(new TemplateSpec({…}), lang, new TemplateLanguage(lang))`, `instantiate(node, template, new ValueBindings().with(name, tree))`, durable objects (`{"@durable": …}`, valid for the registry's life, stale under another, deduplicated by specification) | [What a feature in JavaScript adds to the output](crate::guide::extending#what-a-feature-in-javascript-adds-to-the-output); the JavaScript twin in [Number a construct of your own](crate::guide::extending#number-a-construct-of-your-own); the module page of `flm_ext::quickjs` (mapping rules 4 and 7, the forms of `lw.get`, the rehearsal, Failures); the test extensions `flm-ext/tests/js/exercises/` and `flm-ext/tests/js/myhead/` |
| a feature that processes mathematics: what to declare (a fallback or `[Mode::Math]` elements), the check providing `flm.math.content` and added to `flm.validated`, the rules of `flm.math` and `flm.math.environment` built with the `math` library | [A feature that processes mathematics](crate::guide::extending#a-feature-that-processes-mathematics); [`crate::math_passthrough`] (Rust); `extensions/js/math-mathjax/` (JavaScript) |
| a feature in JavaScript that adds an output format or rewrites trees: `BackendDef`, `OutputDef`, `LoweringStage`, `lowered_table_name`, `StandardTable` with `table_named`, `Artifact`/`ArtifactId`, the lowered trees read by a stage (`.lowered`), `node.payload()` and the slots by index, `flm:lang/structural`, `presentation_element` with a data payload, `rewrite_stage` with `RewriteDecl`, `Restage` and the restager `rs` | [What a feature in JavaScript adds to the output](crate::guide::extending#what-a-feature-in-javascript-adds-to-the-output); [Adding an output format](crate::guide::extending#adding-an-output-format); the module page of `flm_ext::quickjs` ("A backend of your own", "A rewrite stage"); the test extensions `flm-ext/tests/js/typst/` and `flm-ext/tests/js/smartdash/` |
| a Rust extension compiled to WebAssembly: the manifest (`engine: wasm`, `extension.wasm.module`, `extension.wasm.limits` with `memory_mib` and `fuel`), the guest client `flm_ext::guest` (cargo feature `guest`: the API's modules under `flm_ext::guest::…`, façades as structs forwarding to the host and answering `Result<_, ExtError>`, the handler traits taken as `Arc<dyn Trait>`, plain data re-exported), the macro `flm_ext::guest::extension!(f)` and the builders `Extension`, `Package`, `Feature`, the crate (`cdylib`, `flm-ext` with `default-features = false, features = ["guest"]`, `panic = "abort"`), the build (`cargo build --target wasm32-unknown-unknown --release`), the move from a crate written against `flm_ext::api`, what a module reaches (the host's imports and its declared host services), the limits (fuel per handler call, memory per instance) | [An extension in WebAssembly](crate::guide::extending#an-extension-in-webassembly); [A Rust extension compiled to WebAssembly](crate::guide::extending#a-rust-extension-compiled-to-webassembly); the module pages of `flm_ext::guest` (the mapping rules, the rehearsal, the places where the move is not mechanical) and `flm_ext::wasm` (the interface between the host and the module, the limits, the failures); the test crate `flm-ext/tests/wasm/wordcount/` as the worked example |
| a Rust extension compiled into a program: the same crate as its WebAssembly form (`flm-ext` with the feature `guest` alone; the default library type for the program, `cargo rustc --crate-type cdylib` for the module), the static `FLM_EXTENSION` the macro writes on every target, the engine `native` (cargo feature `native`: `NativeEngine::new([NamedExtension { stem, compiled: &crate::FLM_EXTENSION }])`, a table of code, added with `ExtensionLoader::with_engine`; the program's source answers the stem with its manifest, `engine: native`, and its files; a stem the engine does not hold is refused, listing those it holds; a program on `flm-host` gives `flm_host::bundled::Bundled` in its request, and `flm-host` makes the source and the engine), what differs (no sandbox, no limits and no bound on a value's depth, every JSON value passed as it is, in a call's arguments and answer and in a template module's data (the engines `js` and `wasm` refuse a number that is not whole or a reserved map key in them; on every engine the API itself refuses one where it keeps the value as data: a table item, a data payload, a diagnostic's data, a service slot's data), every call a direct, typed call in the same process with no JSON value written or read (the requests, the lending of handler objects and FLM's calls of the handlers alike; only a feature's settings, given to `register` and to a template module's `describe`, and the package declarations stay JSON values), a panic fails the handler and replaces the thread's instance, a panic in an object's `Drop` reported by the panic hook alone, a call from a thread the extension started answers `ExtError::Unsupported`, the digest from the extension's name and version, those its `CompiledExtension` holds, not its code), and how the form is chosen (by `flm-ext`'s feature `native`, not by the target: a program built for any target, `wasm32` included, carries extensions compiled in; an extension's `.wasm` module is built in a cargo run where no crate turns on `native`, such as `cargo rustc -p NAME`, or it lacks its exports and the engine `wasm` refuses it, saying which export it lacks) | [A Rust extension compiled into a program](crate::guide::extending#a-rust-extension-compiled-into-a-program); the module pages of `flm_ext::native` (a compiled example, the instances, dropping an instance, the limits and failures) and `flm_ext::guest`; the tests `flm-ext/tests/native_case1.rs` and `flm-ext/tests/native_handlers.rs` as the worked examples; the shipped extension `phfpack`, in both forms ([A Rust extension, end to end](crate::guide::extending#a-rust-extension-end-to-end)) |
| several Rust extensions in one crate, and collections: one module per extension, each declaring `pub static FLM_EXTENSION: CompiledExtension = CompiledExtension::new(define, NAME, env!("CARGO_PKG_VERSION"))`; the table of code `pub static COMPILED_EXTENSIONS: &[NamedExtension]` (`stem`, `compiled`) at the root, and `flm_ext::guest::extensions!(COMPILED_EXTENSIONS)` once (`extension!(f)` is the same over one row whose stem is the crate's name); the module's entry `define(stem, module)` picks the row by stem (one row: any stem; several and no match: an error listing the stems); every entry is `{"entry", "extension": STEM, "args"}`, so one instance runs several extensions, each with its own state; two WebAssembly outputs: the module of one extension in its folder (code alone; manifest and files beside it), and a *collection*, one module holding the code and, in its custom section `flmextensions` (`flm_ext::collection`, macro `record!`), every file of several extensions (`data`, `js` and Rust), named `*.flmextensions.wasm`, never found by name (a module found by name that holds the section is refused) but loaded by a program with `flm_ext::wasm::Collection::read(label, bytes)` and offered through `flm_host::session::Request::collections` (identity `LABEL:FOLDER/STEM`; one instance per thread and host for the whole collection; its memory limit the sum over the rows with code; fuel per extension); the manifest, the files and the code of an extension always come from the same source (its folder, a collection, or the tables built into the program), and files from two sources are never combined | [A crate that holds several Rust extensions](crate::guide::extending#a-crate-that-holds-several-rust-extensions); [The extensions a program offers](crate::guide::embedding#the-extensions-a-program-offers); the crate `flm-extensions` (`extensions/`: one cargo feature per extension, `all`, `collection`; its build script writes `EXTENSIONS`, `COMPILED_EXTENSIONS` and the records) and `extensions/rust/build-wasm.sh` (`[--out DIR] [NAME …]`, `--collection FILE NAME …`); the module pages of `flm_ext::wasm` (section Collections) and `flm_ext::collection`; the test guests `flm-ext/tests/wasm/pair/` and `flm-ext/tests/wasm/collection/` |
| where the `flm` program looks for extensions | [Where extensions are looked for](crate::guide::command_line#where-extensions-are-looked-for) |
| settings declared from data, the settings cascade | [`SettingsDecl::config_data`](crate::core::settings::SettingsDecl::config_data), [`SettingsDecl::options_data`](crate::core::settings::SettingsDecl::options_data), [`crate::core::settings::cascade`] |
| end-to-end tests of wiring | `flm-core/tests/wiring_*.rs`, `flm-core/tests/stdlib_*.rs`, `flm-core/tests/pipeline_*.rs` |

## The feature interface

[`FlmFeature`](crate::core::registry::FlmFeature) has two required methods and one
optional one:

- [`info`](crate::core::registry::FlmFeature::info) returns a
  [`FeatureInfo`](crate::core::registry::FeatureInfo): the registered name (unique in a
  registry; by convention the middle segment of the names of everything the feature
  declares, `refs` in `flm.refs.resolve`), a title, documentation and an optional
  version. The version is an input of the [registry digest](crate::model::digest).
- [`register`](crate::core::registry::FlmFeature::register)`(&self, b: &mut
  FeatureBuilder<'_>) -> Result<(), BuildError>` makes every declaration, by calling
  methods of [`FeatureBuilder`](crate::core::registry::FeatureBuilder).
- [`settings`](crate::core::registry::FlmFeature::settings) (optional) returns the
  declaration of the feature's settings (see [Options, settings and
  services](#options-settings-and-services)).

Rules that hold everywhere:

- **The builder only records.** Nothing is resolved or checked while a feature
  registers, so registration order never limits what a feature may declare: refer to
  another feature's capability, table or stage in any order. The checks run in
  [`RegistryBuilder::build`](crate::core::registry::RegistryBuilder::build) (see [The
  registry build](#the-registry-build)).
- **A feature never writes its own name into a declaration.** The builder attributes
  every declaration to the feature being registered, regardless of an `owner` field;
  [`b.feature()`](crate::core::registry::FeatureBuilder::feature) returns the
  [`FeatureName`](crate::model::names::FeatureName) it was registered under.
- **Configuration is a Rust value.** A feature's configuration is the plain Rust value the
  feature is constructed from; it is fixed when the registry is built and cannot vary per
  document or per render ([settings, options and
  configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
  Implement the trait on a value holding it (`Blocks::new(BlocksConfig { .. })`), so the
  compiler checks it. Configuration that changes what processing produces in a way the
  declarations do not show (the text of a template) is passed to
  [`digest_input`](crate::core::registry::FeatureBuilder::digest_input); otherwise a
  cache keyed on the registry digest is reused after it changes.
- **Two registration rounds, exactly**: `register`, then every
  [`Finalizer`](crate::core::registry::Finalizer) (see [Build hooks](#build-hooks)).

## The `FeatureBuilder` methods

Every method that records a declaration returns `Result<_, BuildError>`. Most never
fail at the call; the problems
of a declaration are reported when the registry is built. The methods that only the
first round may call fail with
[`BuildError::ContributionAfterAssembly`](crate::core::registry::BuildError::ContributionAfterAssembly)
when a finalizer calls them (marked "round one" below).

The third column says whether the extension API (`flm_ext::api`, the builder `b` of a
feature written in JavaScript) offers the method, and in what form; besides these, it
offers `table_refining(name, parent, docs)` (a table whose capability refines another) and
`definition_text_table(name, docs)` (a table holding a text for a style or script
definition's part).

| call | declares | from JavaScript |
|---|---|---|
| [`element`](crate::core::registry::FeatureBuilder::element)([`ElementDef`](crate::lang::mint::ElementDef)) | an element value, the names it is written under and their syntax | yes |
| [`element_type_text`](crate::core::registry::FeatureBuilder::element_type_text)(element type name, docs) | the documentation text of an element type this feature declares: what its nodes mean, whatever their element value | yes |
| [`attach::<dyn A>`](crate::core::registry::FeatureBuilder::attach)(element type name, `Arc<A>`) | an aspect on every element value of that element type name, other features' and the structural elements included | — |
| [`fallback`](crate::core::registry::FeatureBuilder::fallback)([`FallbackDef`](crate::lang::mint::FallbackDef)) | what a command name means in a parsing mode where no declaration defines it | yes |
| [`scoped_package`](crate::core::registry::FeatureBuilder::scoped_package)([`ScopedPackageDef`](crate::lang::mint::ScopedPackageDef)) | a scoped package: element declarations available only inside the bodies and arguments that refer to the package (`BodyDef::scoped`, `ArgDef::scoped`) | — |
| [`override_callable`](crate::core::registry::FeatureBuilder::override_callable)([`CallableKind`](crate::lang::mint::CallableKind), name) | that this feature's definition of a command replaces another feature's; it counts only in a parsing mode in which this feature registers after every other feature defining the name, and is not consulted elsewhere | yes |
| [`capability`](crate::core::registry::FeatureBuilder::capability)([`CapabilityDef`](crate::core::registry::CapabilityDef)) | a capability, only for a state that no table represents (a marker, a check's result, the parent of refinements); `unprovided` gives what a plan error says when no stage provides it | yes (name, docs, unprovided) |
| [`collection`](crate::core::registry::FeatureBuilder::collection)([`CollectionDef`](crate::core::registry::CollectionDef)) | a name for a set of capabilities that any feature may add to; demanded, never provided | — |
| [`collection_member`](crate::core::registry::FeatureBuilder::collection_member)(collection, member) | adds a capability to a collection; dropped without a diagnostic when no feature declares the collection | — |
| [`table`](crate::core::registry::FeatureBuilder::table)([`TableName<T>`](crate::core::data::table::TableName), docs) → [`TableHandle<T>`](crate::core::data::table::TableHandle) | a table, and a capability of the same name | yes, by name: a table of data items |
| [`table_named`](crate::core::registry::FeatureBuilder::table_named)`::<T>`(name, docs) | the same, for a name computed at registration (`flm.lowered.<backend>`) | yes, for the types of `StandardTable` |
| [`external_table`](crate::core::registry::FeatureBuilder::external_table) / [`external_table_named`](crate::core::registry::FeatureBuilder::external_table_named) | an external table: no stage writes it; the caller installs its value when the run is constructed | — |
| [`stage`](crate::core::registry::FeatureBuilder::stage)(`Arc<dyn `[`Stage`](crate::core::stage::Stage)`>`) | a stage that runs after the trees are frozen; its `decl()` is read once, here | yes |
| [`rewrite_stage`](crate::core::registry::FeatureBuilder::rewrite_stage)(`Arc<dyn `[`RewriteStage`](crate::core::stage::RewriteStage)`>`) | a rewrite stage, and the table of trees it publishes | yes |
| [`ordering`](crate::core::registry::FeatureBuilder::ordering)([`OrderingDef`](crate::core::registry::OrderingDef)) | ordering edges on any registered stage, this feature's or another's | — |
| [`lowering_rule`](crate::core::registry::FeatureBuilder::lowering_rule)([`LoweringRule`](crate::core::lowering::LoweringRule)) | a lowering rule for one element | yes |
| [`presentation_element`](crate::core::registry::FeatureBuilder::presentation_element)`::<P>`(name, docs) → `Arc<`[`ElementSpec`](crate::model::element::ElementSpec)`>` | a presentation element whose nodes carry a payload of type `P`; returns the element value | yes, the payload being data |
| [`backend`](crate::core::registry::FeatureBuilder::backend)([`BackendDef`](crate::core::registry::BackendDef)) | a backend: its native set and its lowering, emit and final stages, registered as this feature's stages in that order | yes |
| [`native_implication`](crate::core::registry::FeatureBuilder::native_implication)([`NativeImplication`](crate::core::registry::NativeImplication)) | a requirement on every backend's native set: an element always, or an element whenever the set holds an element carrying a given aspect | — |
| [`peer_backend`](crate::core::registry::FeatureBuilder::peer_backend)(name, docs) | a peer declaration of a backend that this feature's rules are the default on (`DefaultMark::OnBackends`) | yes |
| [`peer_element`](crate::core::registry::FeatureBuilder::peer_element)`::<P>`(name, docs) | a peer declaration of a presentation element this feature's rules produce; `P` is checked against the registered payload type | yes, by name |
| [`peer_table`](crate::core::registry::FeatureBuilder::peer_table)`::<R>`(name, docs) | a peer declaration of a table this feature's rules read or its stages write; `R` is checked against the declared value type | yes, by name |
| [`peer_scoped_package`](crate::core::registry::FeatureBuilder::peer_scoped_package)(name, docs) | a peer declaration of a scoped package this feature's bodies or arguments refer to | — |
| [`peer_element_value`](crate::core::registry::FeatureBuilder::peer_element_value)([`ElementDef`](crate::lang::mint::ElementDef)) | a peer declaration of one more value, with its commands, of an element type another feature declares (a float type of your own, for example) | — |
| [`output`](crate::core::registry::FeatureBuilder::output)([`OutputDef`](crate::core::registry::OutputDef)) | an output: a name a caller asks for, with its capability, its artifact-set table, its entry artifact's media type and extension, and the outputs whose files it is made from (`made_from`) | yes |
| [`workflow`](crate::core::registry::FeatureBuilder::workflow)([`WorkflowDef`](crate::core::registry::WorkflowDef)) | a workflow: a named set of demands together with the choices that go with them | in the package's `flm` block |
| [`workflow_fields`](crate::core::registry::FeatureBuilder::workflow_fields)([`WorkflowDef`](crate::core::registry::WorkflowDef)) | fields that replace the same fields of the workflow of that name (or define it when no feature registers one), applied in registration order | in the package's `flm` block |
| [`options`](crate::core::registry::FeatureBuilder::options)`::<T>`([`Bind`](crate::model::options::Bind)) / [`options_with_docs`](crate::core::registry::FeatureBuilder::options_with_docs) | an option type and where its values may be set; not needed for an option type the feature's settings declaration lists | declared as data in the feature object's `settings` |
| [`service_slot`](crate::core::registry::FeatureBuilder::service_slot)`::<S>`(name, [`Arity`](crate::core::services::Arity), docs) | a service slot (round one) | — |
| [`contribute`](crate::core::registry::FeatureBuilder::contribute) / [`contribute_to`](crate::core::registry::FeatureBuilder::contribute_to) / [`contribute_first`](crate::core::registry::FeatureBuilder::contribute_first) / [`contribute_first_to`](crate::core::registry::FeatureBuilder::contribute_first_to) | a value for a service slot, found by value type or by name, after or before the other contributions (round one) | — |
| [`peer_slot`](crate::core::registry::FeatureBuilder::peer_slot)`::<S>`(name, `Arc<S>`) | a peer contribution: a value for a service slot another feature declares, dropped with a Note when none does (round one) | — |
| [`require_service`](crate::core::registry::FeatureBuilder::require_service)`::<S>` / [`require_service_named`](crate::core::registry::FeatureBuilder::require_service_named) | that an unfilled slot fails the registry build rather than a render (round one) | yes, by name |
| [`finalizer`](crate::core::registry::FeatureBuilder::finalizer)(`Arc<dyn `[`Finalizer`](crate::core::registry::Finalizer)`>`) | the second-round build hook (round one) | — |
| [`late_init`](crate::core::registry::FeatureBuilder::late_init)(`Arc<dyn `[`LateInit`](crate::core::registry::LateInit)`>`) | a build hook that runs once the language is built; either round | — |
| [`style_def`](crate::core::registry::FeatureBuilder::style_def)([`StyleDef`](crate::core::registry::StyleDef)) / [`script_def`](crate::core::registry::FeatureBuilder::script_def)([`ScriptDef`](crate::core::registry::ScriptDef)) | a style or script definition that a rule or an emitter requires by identifier | yes |
| [`digest_input`](crate::core::registry::FeatureBuilder::digest_input)(bytes) | extra bytes for the registry digest | yes |
| [`feature`](crate::core::registry::FeatureBuilder::feature)() | (reads) the feature's registered name | yes |
| [`services`](crate::core::registry::FeatureBuilder::services)() | (reads) the assembled [`Services`](crate::core::services::Services): `Some` in round two only | — |

## Names

- Every entity a feature registers is identified by a lowercase dotted
  [`Name`](crate::model::names::Name) whose first segment is its owner: `flm` for the
  standard features, a prefix of your own otherwise ([names and
  identity](crate::guide::concepts_overview#names-and-identity)). An element also has a
  name, its type name, but it is identified by its element value.
- Each kind of name has a newtype, so that one cannot be passed for another:
  [`ElementName`](crate::model::names::ElementName),
  [`CapName`](crate::model::names::CapName),
  [`StageName`](crate::model::names::StageName),
  [`RuleName`](crate::model::names::RuleName),
  [`FeatureName`](crate::model::names::FeatureName),
  [`BackendName`](crate::model::names::BackendName),
  [`OutputName`](crate::model::names::OutputName),
  [`WorkflowName`](crate::model::names::WorkflowName), and the others of
  [`crate::model::names`].
- **Capabilities, tables and collections share one set of names.** A table's name is a
  capability's name; a table and a separately declared capability of one name are
  [`BuildError::TableCapabilityClash`](crate::core::registry::BuildError::TableCapabilityClash).
  Features, stages, lowering rules, backends, outputs and workflows each have a set of
  names of their own.
- Usual forms: `<owner>.<thing>` for an element, `<owner>.<element>.lower.<variant>` for
  a lowering rule, `flm.<feature>.<verb>` for a standard stage.
- The name of a presentation element and a **role** (a dotted name on a presentation
  payload that says what a container is) are a public interface for stylesheets: the
  HTML backend derives a CSS class from each, every hyphen doubled and every dot turned
  into a hyphen ([`class_of_role`](crate::stdlib::backends::html::class_of_role):
  `flm.heading.number` → `flm-heading-number`). Choose such a name once and keep it.

## The registry build

```text
let mut builder = RegistryBuilder::new();
builder.use_package(PackageEntry);       // a bundle, registered once per package name
builder.feature(Arc<dyn FlmFeature>);    // one feature
builder.contribute::<S>(Arc<S>);         // a service value from the host (also contribute_to)
builder.workflow_fields(WorkflowDef);    // workflow fields from the host, applied last
let registry = builder.build(&RegistryInputs)?;  // Result<Arc<FlmRegistry>, RegistryBuildError>
```

The methods that collect packages, features, contributions and workflow fields take
`&mut self` and return `&mut RegistryBuilder`, so they can be chained; `build` takes the
builder by value, so it is called on the builder variable, as above.
[`build_registry`](crate::core::registry::build_registry) builds from a list of features
alone, with the default inputs.

[`RegistryBuilder::build`](crate::core::registry::RegistryBuilder::build) runs five
phases ([`BuildPhase`](crate::core::registry::BuildPhase)), in order:

1. `Collect`: the features register (round one), then the service slots are assembled.
2. `Finalize`: the finalizers run (round two).
3. `Validate`: the peer declarations are resolved, the checks over all the declarations
   run, and the aspect attachments are merged.
4. `Language`: the language, which documents are parsed with
   ([`FlmRegistry::lang`](crate::core::registry::FlmRegistry::lang)), is built from the
   element declarations.
5. `LateInit`: the late-initialization hooks run with the finished language.

A phase collects every problem it finds; the first phase that finds one stops the build
and returns all of its problems as one
[`RegistryBuildError`](crate::core::registry::RegistryBuildError)`{ phase, errors }`
([`render`](crate::core::registry::RegistryBuildError::render) writes them one per
line). The later phases do not run.

[`RegistryInputs`](crate::core::registry::RegistryInputs) fields: `parsing`
([`FlmParsing`](crate::lang::FlmParsing): comments, `$` as a math delimiter),
`structural` (the structural elements the language starts from), `guard`
([`GuardPolicy`](crate::lang::guard::GuardPolicy)), `diagnostics_limit` (per parse),
`input_resolver` (the host's [`InputResolver`](crate::lang::InputResolver) for `\input`;
`None` includes no file), `build_identity` (a string identifying the program, an input of
the registry digest).

The [`BuildError`](crate::core::registry::BuildError) variants: `Feature` (a feature's
`register` or finalizer refused, or a workflow sets a value of an option type that no
feature declares or that a render request cannot set), `DuplicateFeature`, `Duplicate`
(two declarations of one name in one namespace; for a peer declaration, one feature
declaring a name twice with two signatures), `TableCapabilityClash`, `Undeclared` (with
a [`Site`](crate::core::registry::Site) saying where the reference is written),
`TableTypeMismatch`, `RefinesCycle`, `TwoDefaults`, `DefaultWithoutParent`,
`CheckWrites` (a `Check` stage declares a table written), `ExternalTableWritten`,
`CollectionAsCapability`, `TwoDefaultRules`, `ExtendsCycle`, `OutputCapabilityMismatch`,
`OutputTableNotArtifactSet`, `AspectCollision`, `NativeSetIncomplete`,
`PeerElementMismatch`, `PeerTableMismatch`, `Service` (a problem found when the service
slots are assembled, [`ServiceError`](crate::core::services::ServiceError)),
`ContributionAfterAssembly`, `Lang` (the language build: two features defining one
command, a scoped package no feature declares, and the other `MintError`s), `LateInit`
(a hook failed).

**A successful build may still carry diagnostics**
([`FlmRegistry::build_diagnostics`](crate::core::registry::FlmRegistry::build_diagnostics)):
what a late-initialization hook reported about the configuration it prepared (a user
definition whose template does not parse; the definition stays registered and each use
reports its own diagnostic), and one Note per unresolved peer declaration. An embedding
program can report these build diagnostics once per registry build, so that each cause
appears next to the diagnostics a run reports because of it.

**Peer declarations.** A peer declaration is a feature's forward declaration of an
element, element type, table, service slot, scoped package or backend that another
feature owns, with the element's payload type or the table's value type as the
signature; the registry build resolves it after every feature has registered
([`PeerName`](crate::core::registry::PeerName)). A resolved declaration with a wrong
signature is `PeerElementMismatch` or `PeerTableMismatch`. An **unresolved** one is not
an error: the build succeeds, one Note `flm.registry.unresolved-peer`
([`UnresolvedPeer`](crate::core::registry::UnresolvedPeer)) joins the build diagnostics,
and what depends on the name is handled per form:

| form | unresolved |
|---|---|
| `peer_backend`, `peer_element` | every rule of the feature that is the default on that backend, or produces that element, is **set aside**: excluded from rule selection and from the completeness check, listed by [`FlmRegistry::set_aside_rules`](crate::core::registry::FlmRegistry::set_aside_rules) |
| `peer_table` | a rule that reads the table with `reads` is set aside; a rule that reads it with `reads_opt` runs and finds the table absent; a stage that writes it is not registered, and the ordering edges placed on that stage are dropped |
| `peer_slot` | the value is dropped; nothing is set aside |
| `peer_scoped_package` | the bodies and arguments that refer to the package get nothing from it: its commands are unknown commands there (without the peer declaration, an unknown package is `MintError::UnknownScopedPackage`) |
| `peer_element_value` | the declaration is dropped: its commands are unknown commands, so no node of it exists and no plan is refused for lack of a rule |

A repeated identical peer declaration from one feature is merged into the first; two
features may each declare the same name. Read the declarations back, each marked
`resolved` or not, with
[`peer_backends`](crate::core::registry::FlmRegistry::peer_backends),
[`peer_elements`](crate::core::registry::FlmRegistry::peer_elements),
[`peer_tables`](crate::core::registry::FlmRegistry::peer_tables),
[`peer_slots`](crate::core::registry::FlmRegistry::peer_slots),
[`peer_scoped_packages`](crate::core::registry::FlmRegistry::peer_scoped_packages) and
[`peer_element_values`](crate::core::registry::FlmRegistry::peer_element_values): a
misspelled peer name is a Note and a set-aside rule, not an error.

The result is a frozen [`FlmRegistry`](crate::core::registry::FlmRegistry), shared as
`Arc` and never modified. It numbers its entities in registration order; the dense ids
([`CapId`](crate::core::registry::CapId),
[`StageIdx`](crate::core::registry::StageIdx),
[`RuleIdx`](crate::core::registry::RuleIdx),
[`ElemIdx`](crate::model::element::ElemIdx),
[`TableId`](crate::core::data::table::TableId)) belong to the registry that issued them
and mean nothing in another.

## Elements and their syntax

An element is the value that says what a node means: a declaration with a stable dotted
type name, identified by the element value itself (its allocation) and not by its name
([`ElementSpec`](crate::model::element::ElementSpec)). An element declaration
([`ElementDef`](crate::lang::mint::ElementDef)) is the plain data a feature gives for an
element: the syntax it is written in (macro, environment or special character sequence),
its arguments (each with a parser, a layout, a parsing mode and the scoped packages
available inside it) and its body. An `ElementDef` holds the names the construct is
written under, a [`SyntaxDef`](crate::lang::mint::SyntaxDef), and the
`Arc<ElementSpec>` its nodes carry. From the declarations of every feature,
[`crate::lang::mint`] builds the techy packages and specifications a parse resolves
against (this step is called minting), so every parsed node carries its element directly.

[`SyntaxDef`](crate::lang::mint::SyntaxDef) has five forms:

| variant | source form |
|---|---|
| `Macro { args }` | `\name` followed by its arguments |
| `Environment { args, body }` | `\begin{name}…\end{name}`, the arguments after `\begin{name}` |
| `Inclusion { args }` | a macro that brings in another file's content (`\input{…}`), read by the language's own inclusion parser ([`crate::lang::input`]); one argument must be named [`REFERENCE_ARGUMENT`](crate::lang::input::REFERENCE_ARGUMENT), else minting reports [`MintError::NoReferenceArgument`](crate::lang::mint::MintError::NoReferenceArgument) |
| `Specials { trigger, args }` | a special character sequence (`~`, `--`) followed by its arguments |
| `Synthesized` | no syntax: the element is never parsed; only a rewrite stage or a lowering stage creates nodes of it (a heading's number, a pointer to another node's argument); the declaration lists no name |

A declaration is available in text mode only unless it lists its parsing modes with
[`ElementDef::modes`](crate::lang::mint::ElementDef::modes): `[Mode::Math]` for a command
of mathematics, `[Mode::Text, Mode::Math]` for one of both modes (the JavaScript
`ElementDef` and the extension data shape follow the same default). Two features that
define one name in modes that do not overlap do not clash.

**Arguments.** Each is an [`ArgDef`](crate::lang::mint::ArgDef): the parser that reads
it ([`kind`](crate::lang::mint::ArgDef::kind): a standard kind or a parser of the
feature's own, [`FlmArgumentParser`](crate::lang::mint::FlmArgumentParser)), an optional
name, the parsing mode, the [`layout`](crate::lang::mint::ArgDef::layout), the
[`scoped`](crate::lang::mint::ArgDef::scoped) packages available inside it, and
documentation. **Name every argument that anything reads**: a reader that finds an
argument by name survives a change to the list. Constructors: `ArgDef::mandatory(name)`,
`ArgDef::optional(name)`, `ArgDef::named(name, kind)`.

The standard kinds, [`ArgStdKind`](crate::lang::mint::ArgStdKind), with their structured
syntax (the kind as data: `syntax` holds the name below, the parameters stand beside it, as
in `{syntax: delimited-group, delimiters: ["(", ")"]}`; [the kind as
data](crate::lang::mint::ArgStdKind#the-kind-as-data)) and their argument codes (the
shorthands settings may write in its place; table in
[`lang::mint::codes`](crate::lang::mint::codes)):

| kind | `syntax` and parameters | reads | code |
|---|---|---|---|
| [`Mandatory`](crate::lang::mint::ArgStdKind::Mandatory) | `mandatory` | a `{…}` group; a single token also counts, as in TeX | `{`, `m` |
| [`Optional`](crate::lang::mint::ArgStdKind::Optional) | `optional` | an optional `[…]` group | `[`, `o` |
| [`Star`](crate::lang::mint::ArgStdKind::Star) | `star` | an optional `*` | `*`, `s`, `t*` |
| [`Marker`](crate::lang::mint::ArgStdKind::Marker) | `marker`; `marker`: one or more characters | an optional marker other than a lone `*`, its characters matched as written (`\name!`, `\name\big`) | `t<c>`, `t{<chars>}` |
| [`DelimitedGroup`](crate::lang::mint::ArgStdKind::DelimitedGroup) | `delimited-group`; `delimiters`: `[<c1>, <c2>]` | a group between a prescribed pair (`\name(arg)`); no single token | `r<c1><c2>` |
| [`OptionalDelimitedGroup`](crate::lang::mint::ArgStdKind::OptionalDelimitedGroup) | `optional-delimited-group`; `delimiters` | the same, optional | `d<c1><c2>` |
| [`AnyDelimitedGroup`](crate::lang::mint::ArgStdKind::AnyDelimitedGroup) | `any-delimited-group` | a group in `{}`, `[]`, `()` or `<>`, whichever the use site opens; the node records which | `AnyDelimited` |
| [`OptionalAnyDelimitedGroup`](crate::lang::mint::ArgStdKind::OptionalAnyDelimitedGroup) | `optional-any-delimited-group` | the same, optional | `AnyDelimitedOptional` |
| [`BracedOnly`](crate::lang::mint::ArgStdKind::BracedOnly) | `braced-only` | a `{…}` group and nothing else | `BracedOnly` |
| [`Embellishments`](crate::lang::mint::ArgStdKind::Embellishments) | `embellishments`; `markers`: one character each, at least one | marker–expression pairs in any order, each marker at most once (`\name_{a}^{b}`); absent when no marker follows; techy's `split_embellishments` reads the pairs back by marker | `e{<chars>}` |
| [`KeyVals`](crate::lang::mint::ArgStdKind::KeyVals) | `key-vals` | an optional `[key=value,…]` group, parsed as `Optional`; the element reads the pairs itself | none |
| [`CharsGroup`](crate::lang::mint::ArgStdKind::CharsGroup) | `chars-group` | a `{…}` group read as plain characters (`\label{sec:intro}`); comments and nested groups are recognized, and a template's placeholder stays one | none |
| [`Verbatim`](crate::lang::mint::ArgStdKind::Verbatim) | `verbatim`; `delimiters`, absent for the closer matched to the opener | raw text between delimiters (`\verbcode\|…\|`); nothing is recognized but a template's placeholder | `v`, `v<c1><c2>` |
| [`TackOn`](crate::lang::mint::ArgStdKind::TackOn) | `tack-on`; `fields`: `[{name, args, repeatable, docs}]`, distinct names, `args` an argument list (none by default), `repeatable` `false` and `docs` empty by default | the fields ([`TackOnField`](crate::lang::mint::TackOnField)) absorbed at the argument's position in a call, after the arguments declared before it, as `\section{…}` absorbs `\label{…}` | none |
| [`Terminated`](crate::lang::mint::ArgStdKind::Terminated) `{ terminator, repeat_char, opener, head, raw }` | `terminated`; `terminator`; `repeat_char` (absent: none), `opener` and `head` (empty by default), `raw` (`false` by default) | everything up to a terminator, matched before any other markup: prefix = run of `repeat_char` (any length) + `opener`; terminator = `head` + the same run + `terminator`; content parsed (`raw: false`) or raw text in a verbatim-class group (`raw: true`); nothing trimmed; techy's `TerminatedArgumentParser`; `MissingArgumentTerminator`, `ExpectedArgumentOpener` | `u<term>` (content), `U<term>` (raw), `f<c><term>` (content after a run of `<c>`), `F<c><term>` (raw); the code span is a specials `` ` `` with one argument ``` F`` ```; no code sets `opener` or `head` |

An argument's syntax in data is read by one reader only:
[`arg_def_from_code`](crate::lang::mint::arg_def_from_code) (one code; an
[`ArgCodeError`](crate::lang::mint::ArgCodeError) otherwise),
[`ArgData`](crate::lang::mint::ArgData) (one argument: a code, or a map
[`ArgDataFields`](crate::lang::mint::ArgDataFields) holding exactly one of `syntax` (with
the kind's parameters) and `code`, beside `name, layout, opaque_wrapper, docs, mode,
scoped, role`; an [`ArgDataError`](crate::lang::mint::ArgDataError) otherwise) and
[`ArgListData`](crate::lang::mint::ArgListData) (a list in its three shapes: `"[{"`,
`["[", "r()"]`, or entries, each a code or a map; an
[`ArgListError`](crate::lang::mint::ArgListError) names the entry). A feature's settings field of
arguments is `Vec<ArgDef>` with `#[serde(deserialize_with = "deserialize_arg_list")]` and
`#[schemars(with = "ArgListData")]`
([`deserialize_arg_list`](crate::lang::mint::deserialize_arg_list)); never read a code or
a structured syntax by hand. Written back (`ArgDataFields::from(&arg)`, which flm-ext uses
for every argument it gives a JavaScript extension or a WebAssembly module), a declaration
read from a code gives its `code` ([`ArgDef::code`](crate::lang::mint::ArgDef::code)); any
other declaration whose parser is a standard kind gives its `syntax`; a feature's own
parser gives neither and does not read back. A Rust extension compiled into the program
receives no data form: its guest `ArgDef` is made from flm-core's argument by the same
rule ([`ArgDef::arg_code`](crate::lang::mint::ArgDef::arg_code), else the parser's
standard kind, else an error). JavaScript guests: `new ArgDef(syntax)` and
`ArgDef.named(name, syntax)` take a code or a structured syntax (a map typed `ArgStdKind`
in `flm.d.ts`); `arg_def_from_code(code)` asks FLM. Rust guests: `ArgDef::new(kind)` and
`ArgDef::named(name, kind)` take the guest's own `ArgStdKind`, whose `TackOn { fields }`
holds `TackOnField`s of guest `ArgDef`s; `ArgDef::from_code(code)` and
`ArgDef::named_from_code(name, code)` read a code locally with flm-core's reader (in
`define` too; a text that is no code is `ExtError::Core` of kind `arg-code`, flm-core's
message); the guest has no `arg_def_from_code`. An argument crosses as data both ways:
`label_fields_arg()` (and, in JavaScript, `arg_def_from_code`) answers one written with its
code or its structured syntax.

**An argument's role** ([`ArgDef::role`](crate::lang::mint::ArgDef::role())): a
registered dotted name for what the argument holds (a label, a file path), shown by
`flm info`; parsing, processing and the [digest](crate::model::digest) ignore it. Declare
it once with [`b.argument_role(name, docs)`](crate::core::registry::FeatureBuilder::argument_role)
(namespace *argument roles*; a second declaration of the name is `BuildError::Duplicate`),
set it with `.role(name)` (data: `role:` in an argument entry; guest and JavaScript:
`.role(name)`, `b.argument_role`). Setting another feature's role needs that feature in
the registry, or [`b.peer_argument_role(name)`](crate::core::registry::FeatureBuilder::peer_argument_role)
(unresolved: one Note `flm.registry.unresolved-peer`); neither is
[`BuildError::UndeclaredArgumentRole`](crate::core::registry::BuildError::UndeclaredArgumentRole).
Read back: [`FlmRegistry::argument_roles`](crate::core::registry::FlmRegistry::argument_roles);
the arguments that have a role: `registry.declarations().role_uses(name)`
([`Declarations::role_uses`](crate::core::registry::Declarations::role_uses)).
The eight standard roles: `flm.refs.target`, `flm.refs.defined-label`,
`flm.citations.key`, `flm.defterm.defined-term`, `flm.defterm.target`,
`flm.graphics.path`, `flm.input.path`, `flm.href.address` (constants such as
[`refs::TARGET_ARG_ROLE`](crate::stdlib::features::refs::TARGET_ARG_ROLE)).

**Documentation texts** (`.docs(…)` on every declaration; settings descriptions from the
settings type's rustdoc via the JSON Schema, after `settings::plain_text`): FLM read with
the extension `flmdoc`, code between backticks (`` `\section{title}` ``), braces paired,
no lone `%`, no bare `\name` outside backticks; the first paragraph is the summary; a
command's text says what it does (the page shows the signature). `flm-core`'s cargo
feature `docs` (default) keeps them; off, every builder stores `""`
([the cargo feature docs](crate::guide::extending#dropping-the-documentation-texts-the-cargo-feature-docs)).

**An argument's layout** ([`ArgLayout`](crate::lang::mint::ArgLayout)) says how the
block-structure pass lays out its content: `Inline` (the default; a blank line inside is
reported and read as a space), `Blocks` (`.blocks()`: paragraphs and blocks inside), or
`InlineOrBlocks` (blocks when the content holds a blank line or a block-level node, inline
otherwise; a `Displayed` node, such as a display environment, is not block-level). An
argument declared with an opaque wrapper
([`ArgDef::with_opaque_wrapper`](crate::lang::mint::ArgDef::with_opaque_wrapper): its text
is kept as typed, for a key or a label) takes no layout other than inline
([`MintError::LayoutOnOpaqueWrappedArgument`](crate::lang::mint::MintError::LayoutOnOpaqueWrappedArgument)),
and `Blocks` is refused in mathematics mode
([`MintError::BlocksOnMathArgument`](crate::lang::mint::MintError::BlocksOnMathArgument)).
A layout declared on a verbatim argument or a characters-only group has no effect and is
not reported. A scoped package may be declared on any argument.
Read a declared layout back with [`arg_layout_of`](crate::lang::mint::arg_layout_of). An
element whose block role or rendering depends on whether an argument holds blocks calls
[`argument_holds_blocks`](crate::lang::rewrites::blocks::argument_holds_blocks) (for a
slot, [`slot_holds_blocks`](crate::lang::rewrites::blocks::slot_holds_blocks)), which
returns the same result before and after block structuring; never write your own check
of whether an argument holds blocks.

**Environment bodies.** A [`BodyDef`](crate::lang::mint::BodyDef) gives the body's
[`BodyLayout`](crate::lang::mint::BodyLayout) (`Inline`, `Blocks`, or `FromRole`, the
default: blocks for the block role `Block`, inline for every other role), an optional body
parser ([`BodyDef::parser`](crate::lang::mint::BodyDef::parser), a
[`FlmBodyParser`](crate::lang::mint::FlmBodyParser);
[`BodyDef::verbatim`](crate::lang::mint::BodyDef::verbatim) installs FLM's raw-text
reader), the parsing mode, whether its content is kept as typed under an opaque wrapper
([`BodyDef::with_opaque_wrapper`](crate::lang::mint::BodyDef::with_opaque_wrapper)), and
the scoped packages available inside it. A body declared with an opaque wrapper takes no
layout (`MintError::LayoutOnOpaqueWrappedBody`).

**Scoped packages.** A scoped package
([`ScopedPackageDef`](crate::lang::mint::ScopedPackageDef)) is a set of element
declarations that can be written only inside the environment bodies and arguments whose
declarations refer to the package by name, for instance `\item` inside a list's body or
`\caption` inside a float's body. Its declarations take precedence over every other
declaration of the same name there; two packages declaring one name never clash, and the
inner one wins where one is added inside the other. A package may add as little as one
special character (`&` inside the `cells` feature's data argument,
[`cells::DATA_PACKAGE`](crate::stdlib::features::cells::DATA_PACKAGE)). Refusals:
an unknown package not peer-declared (`MintError::UnknownScopedPackage`), packages that
refer to each other in a circle (`MintError::ScopedPackageCycle`).

**Element values and element type names.** A family of related commands is declared as
one element type with one element value per command: the type name says what kind of
thing a node is, and the data of each element value tells the commands apart
([`ElementSpec::with_value`](crate::model::element::ElementSpec::with_value); [elements
and aspects](crate::guide::concepts_overview#elements-and-aspects)). `\section` and
`\subsection` are two values of `flm.heading`, and one lowering rule serves them all.
Compare values with
[`ElementSpec::same_value`](crate::model::element::ElementSpec::same_value). One
`ElementDef` may list several names for one value: every command of one heading kind is a
name of that kind's value.

**Fallbacks and replacements.** A [`FallbackDef`](crate::lang::mint::FallbackDef) says
what a command name means in a parsing mode where no declaration defines it: the
`math-passthrough` feature declares one for macros and one for environments in mathematics
mode, so an unknown command there becomes a node of `flm.math-passthrough.opaque` that
keeps its source and is not reported. Where several features define one command name in a
parsing mode (each mode decided on its own), the definition of the feature placed last in
the feature order applies, which is allowed when the chains of the feature order place that
feature after every other definer in that mode, or when that feature declares a replacement
with [`override_callable`](crate::core::registry::FeatureBuilder::override_callable);
otherwise the build fails, naming the features and the chain that would choose the last
one's definition. Only the last feature's override is consulted.

**Declaring a command.** An `ElementDef` whose syntax lists its arguments, each an
`ArgDef` carrying a standard kind:

```rust
use std::sync::Arc;
use flm_core::lang::mint::{ArgDef, ArgLayout, ArgStdKind, ElementDef, SyntaxDef};
use flm_core::model::element::ElementSpec;

let note = ElementDef::new(
    "notes",
    ["note"],
    SyntaxDef::Macro {
        args: vec![
            ArgDef::named("star", ArgStdKind::Star),
            ArgDef::mandatory("text").layout(ArgLayout::InlineOrBlocks),
        ],
    },
    Arc::new(ElementSpec::new("notes.note")),
);
assert_eq!(note.names.len(), 1);
```

**An argument read by a parser of one's own.** Where no standard kind expresses the
syntax, the argument carries a parser object: a type implementing techy's
`ArgumentParser<Flm>` (the reading) and
[`FlmArgumentParser`](crate::lang::mint::FlmArgumentParser) (what FLM needs beyond it:
the bytes of its identity for the registry digest, its tack-on fields, its shape for a
definition written back, a short description of its syntax for the settings' schema). The
name, mode, layout, scoped packages, opaque wrapper and documentation stay on the `ArgDef`; minting applies the mode
and the packages around the parser, which finds them in `cx.state`, and places what the
parser read under the opaque wrapper when the declaration asks for one. The parser never panics or
loops: reading nothing and meeting the end of the input are ordinary outcomes. Whether a
use site may leave the argument out is techy's `can_match_empty`, `true` unless the
parser overrides it; a parser that always reads something overrides it to return
`false`.

```rust
use std::sync::Arc;
use flm_core::lang::{Flm, FlmGroupType};
use flm_core::lang::mint::{ArgDef, DefinitionShape, FlmArgumentParser};
use flm_core::techy::core::constructs::{
    ArgumentParser, ConstructParserResult, OptionalGroupArgumentParser, ParseContext,
    ParsedArgumentNodes,
};
use flm_core::techy::core::specs::ArgumentSpec;
use flm_core::techy::core::token::GroupRule;

#[derive(Debug)]
struct Angles(OptionalGroupArgumentParser<Flm>); // `<…>`, optional

impl ArgumentParser<Flm> for Angles {
    fn parse_argument(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
        spec: &ArgumentSpec<Flm>,
    ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
        self.0.parse_argument(cx, spec)
    }
}

impl FlmArgumentParser for Angles {
    fn identity(&self) -> Vec<u8> { b"notes.angles".to_vec() } // digest bytes, stable
    fn definition_shape(&self) -> DefinitionShape { DefinitionShape::NoShape { token: "angles" } }
    fn description(&self) -> String { String::from("an optional `<…>` group") }
}

let rule = Arc::new(GroupRule {
    group_type: FlmGroupType::Content,
    open: String::from("<"),
    close: String::from(">"),
});
let arg = ArgDef::named("aside", Angles(OptionalGroupArgumentParser::new(rule))).scoped("notes.inner");
assert_eq!(arg.kind.description(), "an optional `<…>` group");
```

**A body read by a parser of one's own.** An environment's `BodyDef` carries a
[`FlmBodyParser`](crate::lang::mint::FlmBodyParser), set with
[`BodyDef::parser`](crate::lang::mint::BodyDef::parser): techy's
`EnvironmentBehavior<Flm>` (the reading; only `make_body_parser` is used) plus what FLM
needs beyond it (the bytes of its identity for the registry digest, a short description
of how it reads the body). The layout, mode, scoped packages and opaque wrapper stay on the
`BodyDef`; minting applies the mode and the packages around the parser, which finds them
in `cx.state`, and places what the parser read under the opaque wrapper when the
declaration asks for one. Meeting the end of the input inside the body is an ordinary
outcome. The `math` feature's line check is a full example ([`lines::LinesBehavior`](crate::stdlib::features::math::lines::LinesBehavior)).

```rust
use flm_core::lang::{Flm, FlmGroupType};
use flm_core::lang::mint::{BodyDef, FlmBodyParser};
use flm_core::techy::core::ParsingStateDelta;
use flm_core::techy::core::constructs::{
    ConstructParser, ConstructParserResult, EnvironmentBody, EnvironmentBodyParser, ParseContext,
};
use flm_core::techy::latexlike::{EnvironmentBehavior, EnvironmentInvocation, Mode};

#[derive(Debug)]
struct Checked;

impl EnvironmentBehavior<Flm> for Checked {
    fn make_body_parser<'p>(
        &'p self,
        invocation: EnvironmentInvocation<'p, Flm>,
    ) -> Box<dyn ConstructParser<Flm, Output = EnvironmentBody<Flm>> + 'p> {
        Box::new(CheckedBody { invocation })
    }
}

impl FlmBodyParser for Checked {
    fn identity(&self) -> Vec<u8> { b"notes.checked".to_vec() } // digest bytes, stable
    fn description(&self) -> String { String::from("content, checked as it is read") }
}

struct CheckedBody<'p> {
    invocation: EnvironmentInvocation<'p, Flm>,
}

impl ConstructParser<Flm> for CheckedBody<'_> {
    type Output = EnvironmentBody<Flm>;

    fn parse(
        &mut self,
        cx: &mut ParseContext<'_, '_, Flm>,
    ) -> ConstructParserResult<Flm, (EnvironmentBody<Flm>, Option<Box<ParsingStateDelta<Flm>>>)> {
        let invocation = &self.invocation;
        let (body, delta) = EnvironmentBodyParser::new(
            invocation.trigger_span.clone(),
            invocation.name,
            invocation.end_command_name,
            FlmGroupType::Content,
        )
        .with_invocation_name_span(invocation.name_span.clone())
        .parse(cx)?;
        // check `body` here; report a problem with `cx.recover(condition, span)?`
        Ok((body, delta))
    }
}

let body = BodyDef::new().blocks().mode(Mode::Text).scoped("notes.inner").parser(Checked);
assert!(body.parser.is_some());
```

**Reading an argument as characters.** A label, a reference target or an address (any
argument an element reads as characters) is declared so that its text is kept as typed:
a characters-only group (`CharsGroup`) is placed under an opaque wrapper by
construction, a verbatim argument (`Verbatim`) yields a verbatim group, and any other
kind is declared `.with_opaque_wrapper()`. It is read with
[`read_chars`](crate::lang::text::read_chars), the only reader of content as
characters. It returns the content as [segments](crate::lang::text::CharsSegment): runs
of characters, and the placeholder nodes of a user definition's template that was not
expanded (only a template lowered unexpanded holds one: a kept definition's recomposed
LaTeX, or a list's own label template). It refuses a command or mathematics
([`NotCharacters`](crate::lang::text::NotCharacters)). A caller with no use for a
placeholder calls [`chars_text`](crate::lang::text::chars_text) (the characters, or
`None`); never techy's `content_as_chars`, which fails on a placeholder. A **lowering
rule** reads characters with
[`chars_at_lowering`](crate::core::lowering::chars_at_lowering) and returns its
[`LowerError::PlaceholderCharacters`](crate::core::lowering::LowerError::PlaceholderCharacters)
unchanged (`?`): the rule that spliced the template then acts on it (the recomposed LaTeX
expands the use sites of that kept definition). A rule that computes on content as
written, markup included, calls
[`spelling_at_lowering`](crate::core::lowering::spelling_at_lowering) before it computes
and returns its error the same way.

**Fields.** A field is a macro that a construct absorbs as data rather than as content
(`\label` after a heading; `\\`, `\label`, `\tag` and `\nonumber` in a display-math
body); a construct takes its fields through its tack-on argument, the argument that
absorbs the field macros written at its position in a call, after the arguments declared
before it ([fields](crate::guide::concepts_overview#fields-data-a-construct-reads)). Every field
node, a tack-on field or a field a scoped package declares, is a macro node carrying the
element type [`FIELD`](crate::lang::structural::FIELD) (`flm.field`), and its macro name
is the field's name ([`field_name`](crate::lang::mint::field_name)). A tack-on field
([`TackOnField`](crate::lang::mint::TackOnField)) is data only (name, arguments,
`repeatable`, docs; settings declare one with the structured syntax `tack-on`) and its
node carries the language's own field element, a structural element; a scoped package's
field builds its element with [`field_element`](crate::lang::mint::field_element). A field is a macro that a construct
absorbs as data rather than as content: the construct that declared the field reads it by
its name, and nothing across the document is keyed on a field's name. A field node that
reaches a backend is the Error `flm.lowering.field-not-read`
([`FieldNotRead`](crate::stdlib::backends::lowering::FieldNotRead), reported by
[`FieldRule`](crate::stdlib::backends::lowering::FieldRule)): the construct that
absorbed it did not read it.

## Aspects

An aspect is a piece of behavior attached to an element when the feature registers it and
read back through a trait object; it answers per occurrence of the element and reads no
options ([elements and aspects](crate::guide::concepts_overview#elements-and-aspects)).
A stage or a backend learns what an element does by asking for an aspect, never by
matching the element's name. Any object-safe trait with `Send + Sync` as supertraits can
be an aspect, and no aspect trait is registered. An element value carries at most one
implementation per trait: attach it with
[`ElementSpec::with`](crate::model::element::ElementSpec::with) when building the value,
read it with [`aspect`](crate::model::element::ElementSpec::aspect) and
[`has_aspect`](crate::model::element::ElementSpec::has_aspect).

Two rules hold for every aspect:

1. **Per occurrence.** The node is a parameter of every method, so one element returns
   different results at different places.
2. **No options.** Aspects are consulted during the rewrite stages and while a plan is
   built, before option values are bound. A fact that varies per document belongs to the
   stage that uses the aspect's result; a fact fixed by the configuration belongs to the
   element's data or the aspect's implementation.

The aspects in [`crate::model::aspects`]:
[`BlockRoleAspect`](crate::model::aspects::BlockRoleAspect) (block, inline, a displayed
item set on lines of its own inside its paragraph, or a decorator attached to its block;
[`FixedBlockRole`](crate::model::aspects::FixedBlockRole)
for a constant role, [`BlockRole::DEFAULT`](crate::model::aspects::BlockRole::DEFAULT)
for inline content), [`Referenceable`](crate::model::aspects::Referenceable),
[`Referencing`](crate::model::aspects::Referencing),
[`Sectioning`](crate::model::aspects::Sectioning) (a heading's level and its table of
contents entry), [`ApartFromText`](crate::model::aspects::ApartFromText) (the content of
an occurrence that is not part of the text around it),
[`InCopy`](crate::model::aspects::InCopy) (what replaces an occurrence in a copy of
content shown a second time), [`Whitespace`](crate::model::aspects::Whitespace) (each
occurrence is whitespace to the layout of paragraphs, as `~` and `\ ` are: removed where
the block-structure pass removes whitespace, and at the edges of a paragraph a rule writes
itself, which [`stage_trimmed`](crate::stdlib::backends::lowering::stage_trimmed) trims as
the pass does; whitespace to the standard features' tests for blank content, except those
of text read as typed or with its markup (a citation key, a term: `\term{~}` is not empty);
kept elsewhere as a node of its own; a marker, attached as
[`IsWhitespace`](crate::model::aspects::IsWhitespace)). Read when a document is rendered, declared with the
presentation vocabulary: [`Decoration`](crate::stdlib::backends::pres::Decoration),
[`OpaqueContent`](crate::stdlib::backends::pres::OpaqueContent),
[`ContentInOpaque`](crate::stdlib::backends::pres::ContentInOpaque). Declared beside
the rewrite pass that reads them, each called through an interface of its own (as a
lowering rule works through its [`Lowerer`](crate::core::lowering::Lowerer)):
[`Expand`](crate::lang::rewrites::userdefs::Expand) (through an
[`Expander`](crate::lang::rewrites::userdefs::Expander)),
[`BlockStructure`](crate::lang::rewrites::blocks::BlockStructure) (through a
[`BlockStructurer`](crate::lang::rewrites::blocks::BlockStructurer)) and
[`BlockTransparent`](crate::lang::rewrites::blocks::BlockTransparent) (through a
[`BlockCutter`](crate::lang::rewrites::blocks::BlockCutter); see [Make an element
block-transparent](#make-an-element-block-transparent)).

**Attaching to someone else's element.**
[`attach`](crate::core::registry::FeatureBuilder::attach) gives an aspect to every
element value of a given element type name, other features' elements and the language's
structural elements included. The aspect is given by its **trait-object type**
(`b.attach::<dyn Referenceable>("flm.heading", …)`); a concrete type does not compile. An
element type name no feature declares is `BuildError::Undeclared`; attaching an aspect
the element already carries is
[`BuildError::AspectCollision`](crate::core::registry::BuildError::AspectCollision),
never a silent replacement. A block role that depends on the position (the `cells`
feature's `\cell` is a block directly in a `cells` body, inline elsewhere) is decided by
one predicate that the rule's reader also follows
([`cells::stands_in_body`](crate::stdlib::features::cells::stands_in_body)).

**Templates with values a feature computes.** Declare the macros as
[`Definition`](crate::lang::rewrites::userdefs::Definition)s with extra placeholders
([`Definition::extra_placeholder`](crate::lang::rewrites::userdefs::Definition::extra_placeholder),
`#{name}` or `#[…]{name}`); compile them with
[`UserDefinition::compile_with`](crate::lang::rewrites::userdefs::UserDefinition::compile_with)
in a late-initialization hook; register each element with an `Expand` of your own through
[`UserDefinition::element_def_with`](crate::lang::rewrites::userdefs::UserDefinition::element_def_with)`(owner,
name, hook)`, its type name given by
[`element_name_under`](crate::stdlib::features::userdefs::element_name_under)`(prefix,
&definition)` and lowered by the rule
[`expand_in_tree_rule_for`](crate::stdlib::features::userdefs::expand_in_tree_rule_for)`(name,
&definition)`. In the hook, take the templates of
[`variant_for_use`](crate::lang::rewrites::userdefs::UserDefinition::variant_for_use)`(&site)`
(its `Err` is the condition to report at the use site) and instantiate them with bindings
that give the extra names' content and pass every other reference to
[`DefinitionBindings::new(site, &variant)`](crate::lang::rewrites::userdefs::DefinitionBindings):
[`ExtraValues`](crate::lang::rewrites::userdefs::ExtraValues) keyed by name and
substitution text (`placeholders()` and `substitution_text(node)` of `variant.template()`
and of each template of
[`variant.defaults()`](crate::lang::rewrites::userdefs::ModeVariant::defaults) list the
keys; `.template`, `.tree`, `.no_value`; a pair not held is `Unresolvable`) and
`values.over(&base)`, or an implementation of `PlaceholderBindings`.
A computed value is a finished tree
([`PlaceholderValue::Tree`](crate::lang::rewrites::userdefs::PlaceholderValue::Tree)),
which a feature may compile once with
[`compile_template_with`](crate::lang::rewrites::userdefs::compile_template_with)
through [`compile_in_modes`](crate::lang::rewrites::userdefs::compile_in_modes) (kept in
each parsing mode where it compiles). A template with extra placeholders only (no use
site's arguments) is instantiated from a lowering rule with
[`instantiate`](fn@crate::lang::rewrites::userdefs::instantiate)`(&span, &template,
&ValueBindings::new().with(name, tree), GuardPolicy::StackBudget)`
([`ValueBindings`](crate::lang::rewrites::userdefs::ValueBindings): an extra name
without a tree is `NoValue`, an argument or the body `Unresolvable`), on a worker thread.
The public guide describes it under [Templates with values the feature
computes](crate::guide::extending#templates-with-values-the-feature-computes).

### Reference points, labels and references

A reference point is one place in a document that can be referred to, numbered, or given
an anchor (a heading, an equation line, a theorem, a footnote); an element declares its
reference points per occurrence through the `Referenceable` aspect, and the `refpoints`
feature records them per tree ([`RefPoint`](crate::model::aspects::RefPoint)). The
[`refpoints`](crate::stdlib::features::refpoints) feature asks each node once per run
and writes the points to `flm.refpoints`, which the `refs` and `numbering` features read
and require; a tree the aspect builds is therefore built once per run, and a tree that is
the same for many points (a configured bullet) is better built once, when the registry is
built, and shared.

A point states the two things a reference to it may show:
[`number`](crate::model::aspects::RefPoint::number), a
[`NumberSource { counter, instances, style, advance, number_format }`](crate::model::aspects::NumberSource)
where the construct can be numbered, and
[`content`](crate::model::aspects::RefPoint::content), a
[`ContentRef`](crate::model::aspects::ContentRef) where it has content of its own (an
argument, the body, a node, or
[`ContentRef::Synthesized`](crate::model::aspects::ContentRef::Synthesized) for content
no node holds by itself); either may be `None`. A reference shows the number where the
document numbered the point, else the content, else the label it was written with. The
`numbering` feature numbers every point that states a number source and has no switch to
turn it off, so whether a point states one is the element's decision, from the node and
from its feature's configuration: the headings feature states one for the kinds its
numbering depth admits (`HeadingsConfig::numbering`, none by default); the mathematics
feature states one for every equation line without `\nonumber` or `\tag` unless its
configuration numbers no equations (`MathConfig::numbered_equations`, `true` by default),
and a tagged line states its tag as content.

A point declares the label types it accepts
([`accepts`](crate::model::aspects::RefPoint::accepts)):
`AcceptedRefTypes::these(["thm", "lem"])`, or
[`AcceptedRefTypes::Any`](crate::model::aspects::AcceptedRefTypes). A written label of
another type defines nothing, and the label check reports
`flm.refs.label-type-not-accepted`. Labels the element computes itself come first and
are counted in [`computed`](crate::model::aspects::RefPoint::computed) (`0` when every
label is written); the acceptance does not apply to them. Headings accept
[`headings::LABEL_TYPES`](crate::stdlib::features::headings::LABEL_TYPES) (`sec`,
`topic`), equation lines
[`math::LABEL_TYPES`](crate::stdlib::features::math::LABEL_TYPES) (`eq`).

**A labeled construct**, in three steps: declare
[`label_fields_arg`](crate::stdlib::features::refs::label_fields_arg) as the element's
last argument; read the fields with
[`label_uses`](crate::stdlib::features::refs::label_uses) and split them with
[`point_labels`](crate::stdlib::features::refs::point_labels) (the labels, and the
texts that are not `type:label`, which the `refs` feature reports); put both on the
point. A rule of your own on the recomposed LaTeX writes the labels with
[`stage_label`](crate::stdlib::features::refs::stage_label) and
[`stage_label_group`](crate::stdlib::features::refs::stage_label_group) and declares
what they read and produce (their documentation lists it; among others the safe labels
table [`SAFE_LABELS`](crate::stdlib::features::refs::SAFE_LABELS)).

**An element that refers to a point** through the `refs` feature (the
[`defterm`](crate::stdlib::features::defterm) feature's `\term` is the model):
implement [`Referencing`](crate::model::aspects::Referencing), returning one
[`RefSite`](crate::model::aspects::RefSite) per target, a label the target's point
carries; make a peer declaration of the resolutions table
([`peer_table::<RefResolutions>`](crate::stdlib::features::refs::RefResolutions),
`flm.refs.resolutions`) and read it optionally in the rule
(`LoweringDecl::reads_opt(REF_RESOLUTIONS)`), looking the site up by
[`SiteId { addr: lw.addr(), index }`](crate::stdlib::features::refs::SiteId):
[`Resolution::Resolved`](crate::stdlib::features::refs::Resolution) carries the `href`,
and an absent table means no `refs` feature (write the content alone). The `refs` stages
resolve and report (`flm.refs.unresolved`); never look a label up yourself.

## Tables and stages

A table is typed data that one stage writes beside the frozen trees and other stages
read; its identity is its registered dotted name, never its Rust type, and that name is
also a capability ([`Table`](crate::core::data::table::Table)). Implement `Table`
(`Any + Send + Sync + Default`) on the value type:
[`absorb`](crate::core::data::table::Table::absorb) is required (it adds a [table
piece](crate::core::data::table::PieceClass), the part of a table that one stage
invocation produced; a per-tree stage's table pieces arrive in the run's order of the
trees);
[`project`](crate::core::data::table::Table::project) (the value for dumps and tests)
and [`piece_class`](crate::core::data::table::Table::piece_class) (whether a table
piece states a fact about the document as a whole, which [value
catch-up](crate::guide::concepts_overview#value-catch-up) needs) have defaults. A value
type used by exactly one table may implement
[`SingletonTable`](crate::core::data::table::SingletonTable), so that readers and
writers refer to the table by the Rust type
([`StageCx::get_singleton`](crate::core::run::StageCx::get_singleton),
[`StageOut::table_singleton`](crate::core::data::store::StageOut::table_singleton)).

The two kinds of stage:

| | [`RewriteStage`](crate::core::stage::RewriteStage) | [`Stage`](crate::core::stage::Stage) |
|---|---|---|
| when | before the trees are frozen | after |
| does | one tree to one tree | reads the frozen trees and the tables it declared, writes tables |
| declaration | [`RewriteDecl`](crate::core::stage::RewriteDecl) (`name`, `publishes`, `refines`, `default`, `docs`) | [`StageDecl`](crate::core::stage::StageDecl) (name, [`Effect`](crate::core::stage::Effect), [`Scope`](crate::core::stage::Scope), `provides`, `writes`) |
| reads computed data | never (refused when the plan is built) | the tables it declared |
| output | a [`TreesTable`](crate::core::stage::TreesTable) under the published capability | table pieces through [`StageOut`](crate::core::data::store::StageOut) |
| failure | [`RewriteError`](crate::core::stage::RewriteError): that tree is absent from every later table | none: a problem with the document is a diagnostic |

The standard features provide four rewrite stages: the two expansions of user definitions
(`flm.userdefs.expand-always` for the definitions expanded always and
`flm.userdefs.expand` for the ones expanded per plan, both of the `userdefs` feature), the
insertion of included files (`input`), and the layout in paragraphs and blocks
(`blocks`).

- [`Effect`](crate::core::stage::Effect): `Compute` (produces data), `Check`
  (diagnostics only; declaring a table written is
  [`BuildError::CheckWrites`](crate::core::registry::BuildError::CheckWrites)), `Io`
  (effects outside the store, such as handing files to the program that writes them;
  ordered after every `Compute` and `Check` stage; only another `Io` stage may read what
  it writes; not run when the run has recorded
  an Error, unless
  [`RunOptions::continue_with_errors`](crate::core::run::RunOptions::continue_with_errors)
  is set: the run's error gate).
- [`Scope`](crate::core::stage::Scope): `PerTree` (once per tree, possibly in parallel)
  or `Document` (once for the whole document).
- **The declaration is fixed at registration; the needs are computed per plan.**
  [`Stage::needs`](crate::core::stage::Stage::needs)`(&self, cx: &`[`PlanCx`](crate::core::plan::PlanCx)`)`
  returns [`Needs`](crate::core::stage::Needs):
  [`requires`](crate::core::stage::Needs::requires),
  [`reads`](crate::core::stage::Needs::reads),
  [`reads_opt`](crate::core::stage::Needs::reads_opt),
  [`option`](crate::core::stage::Needs::option)`::<T>`,
  [`after`](crate::core::stage::Needs::after),
  [`before`](crate::core::stage::Needs::before). Most stages return a constant;
  `PlanCx` gives the plan request (demands, preferences, lowering selections, render
  option values) and nothing about any document.
- **Declaring a table written provides its capability, and declaring a table read
  requires it**, so a stage lists its reads and writes and nothing else.
- At run time a stage reads through [`StageCx`](crate::core::run::StageCx):
  [`get`](crate::core::run::StageCx::get) (declared tables only),
  [`option`](crate::core::run::StageCx::option),
  [`service`](crate::core::run::StageCx::service) /
  [`services`](crate::core::run::StageCx::services),
  [`trees`](crate::core::run::StageCx::trees),
  [`frozen_tree`](crate::core::run::StageCx::frozen_tree),
  [`walk`](crate::core::run::StageCx::walk),
  [`placement_of`](crate::core::run::StageCx::placement_of),
  [`resolve_value`](crate::core::run::StageCx::resolve_value) (value catch-up of a
  synthesized value), [`registry`](crate::core::run::StageCx::registry); it writes
  through [`StageOut`](crate::core::data::store::StageOut)
  ([`table`](crate::core::data::store::StageOut::table), and the field `diags` for
  diagnostics). [`Stage::run`](crate::core::stage::Stage::run) has no error return: a
  problem with the document is a diagnostic. A panic is a bug in the stage and propagates
  out of the run.
- An external table is a table that no stage of the run writes: the caller installs its
  value when the run is constructed ([`Externals`](crate::core::run::Externals);
  [`external_table`](crate::core::registry::FeatureBuilder::external_table)). The `refs`
  feature declares one for the labels of the other documents of a corpus
  ([`EXTERNAL_INDEX`](crate::stdlib::features::refs::EXTERNAL_INDEX)).

```rust
use flm_core::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_core::core::data::table::{Table, TableName};

#[derive(Debug, Default)]
struct WordCount(usize);

impl Table for WordCount {
    fn absorb(&mut self, piece: WordCount) {
        self.0 += piece.0;
    }
}

const WORDS: TableName<WordCount> = TableName::new("example.words");

let decl = StageDecl::new("example.words.count", Effect::Compute, Scope::PerTree)
    .writes(WORDS)
    .docs("counts the words of each tree");
let needs = Needs::new().requires("flm.blocks.structured");

// Writing a table provides its capability, so nothing else has to be declared.
assert!(decl.provides.is_empty());
assert_eq!(needs.requires.len(), 1);
```

The [`index`](crate::stdlib::features::index) feature is one table and one `Compute`
stage of `PerTree` scope, and nothing else: the smallest example to read.

**Recipe: a rewrite stage in JavaScript.** `b.rewrite_stage(stage)` with `stage` an object
`{ decl(), needs(), restage(node, cx, rs) }` (`needs` optional). The library drives the
tree (techy's restaging driver); the handler is called once per node and answers what
becomes of that node:

```js
import { Needs, RewriteDecl } from "flm:core/stage";
import { Restage } from "flm:techy/transform";

b.rewrite_stage({
  decl: () => new RewriteDecl("smartdash.dash", "smartdash.dashed"),          // name, published capability
  needs: () => new Needs().after("flm.userdefs").before("flm.blocks.structured"),
  restage(node, cx, rs) {
    if (node.is_chars() && node.chars().includes("--"))
      return Restage.Emit([rs.stage_chars(node.chars().replace(/---/g, "—").replace(/--/g, "–"), node)]);
    if (node.element_name() === "smartdash.quote") {                           // a call rebuilt from parts
      const content = [];
      for (const child of node.argument_content_nodes_named("text") ?? []) content.push(rs.restage_subtree(child));
      const argument = rs.restage_argument_with_content(node, 0, content);
      return Restage.Emit([rs.stage_chars("“", node), rs.restage_invocation(node, [argument], []), rs.stage_chars("”", node)]);
    }
    return Restage.Descend;                                                    // keep the node; its children are visited
  },
});
```

- `Restage.Descend` copies the node and visits its children; `Restage.Emit([...])` puts
  the nodes this call created through `rs` in its place, each placed once (in the `Emit`
  or as content given to `restage_*_with_content`).
- `rs` (the mirror of techy's `RestageContext`): `stage_chars(text, like)` (a characters
  node with the span and parsing state of `like`); `restage_subtree`, `restage_children`,
  `restage_argument`, `restage_argument_named`, `restage_slot` (each copies a region, its
  nodes going through `restage` again, nested calls during which the outer call's objects
  are stale); `restage_argument_with_content`, `restage_slot_with_content` and
  `restage_invocation(node, arguments, slots)` (a call copied with new content);
  `diagnose(diagnostic)`. `cx`: `tree_id()`, `layout()`, `option(name)`.
- A rewrite reads no table (`PlanError::RewriteReads` when the plan is built) and runs
  only when its table is demanded: declare a workflow demanding it in the package's `flm`
  block (`{ workflows: { smartdash: { extends: "text", demands: ["smartdash.dashed"] } } }`).
  A scripted `needs()` cannot test whether a capability is registered; order by names that
  exist in every registry holding the standard features.
- Failures: an exception or a bad answer from `restage` is the Error
  `flm.ext.handler-failed` at the node, and the node is kept (as `Descend`); an operation
  refused for its arguments (an index out of range, an unknown argument name) throws an
  `FlmError` of kind `"core"` that the handler may catch; a refusal after which the new
  tree cannot be built (the root replaced by other than one node, an argument's content
  inside a removed node, content nested too deeply) abandons that tree's rewrite with
  `flm.ext.rewrite-failed` (or `flm.blocks.nesting-too-deep`) at its root, and the tree is
  absent from the output.
- `restage_subtree` and `restage_children` return a `StagedNodes` sequence (next
  section); `Restage.Emit`, `restage_argument_with_content` and
  `restage_slot_with_content` take a `StagedNodes` or an array of staged nodes and
  `StagedNodes`, so the recipe pushes each child's sequence into `content` as it is.
- Worked example: `flm-ext/tests/js/smartdash/main.js`.

## Traversals and sequences in an extension

An extension (JavaScript, WebAssembly, or Rust through the guest client) never receives a
list whose length grows with the document. Each such collection is one handle; the
binding fetches its items from the host a bounded group at a time. Every item the engine
holds costs engine memory (64 MiB by default, `extension.js.limits` /
`extension.wasm.limits`), and every call into the host costs time.

| what | returns | use |
|---|---|---|
| `cx.walk()` (`{ node, addr }`), `node.descendants()` (nodes, document order, the node excluded), `cx.trees()` and the `trees()` of a trees table or of `.lowered` (`[id, tree]`) | a *traversal* (`Walk`, `Descendants`, `Trees`) | iterate once with `for … of` (Rust: `Iterator`, items `Result<_, ExtError>`); no length, no index; calling the method again starts over |
| `.elements(names)` / `.kinds(kinds)` on `walk()` and `descendants()` | the traversal restricted, on the host, to nodes whose `element_name()` is in `names` / whose `kind()` is in `kinds` (`NodeKindName.Chars`, …) | always restrict a stage that looks for a few elements or for characters: a non-matching node costs no call and no memory; descendants of any node are still visited; both restrictions combine (AND); an unknown name matches nothing |
| a list of nodes: `children()`, `body()`, `argument_nodes(i)`, `argument_content_nodes(i)` and the `_named` forms, `slot_content_nodes(i)`, a math line's `content`, `Tag.content`, `NumberSource.instances` | `Nodes`, a *sequence* | `len()`, `is_empty()` (no call), `get(i)` (`null` past the end), repeatable `for … of`, spread, destructuring; `null` where the Rust method returns `None` (`body()` of a node without a body) |
| staged nodes: `lw.lower_children`/`lower_body`/`lower_arg`/`lower_arg_named`/`lower_nodes`/`default`/`splice`, `rs.restage_subtree`/`restage_children`, `xp.instantiate`/`recover`, the `math` library's parts (`group_opening`, `stage_lines`, `write_math_run`, …), `stage_trimmed`, `stage_label_group`, `pres_tree` | `StagedNodes`, a sequence | as `Nodes` |
| the rows of an extension's own table: `cx.get(name).ext.items()` | `Rows`, a sequence of data rows | as `Nodes`; a row is copied when read |

Passing content on: wherever the interface takes several nodes (`chars_text`,
`read_chars`, `lw.lower_nodes`, `write_math_run`, `line_tex`, …) pass a `Nodes`, or a
*joined list*: an array of nodes and `Nodes`, joined in order. Wherever it takes several
staged nodes (`lw.pres` children, `lw.pres_slots` slots, `Lowering.Emit`, `Expansion.Emit`,
`Restage.Emit`, `restage_*_with_content`, `stage_label_group`) pass a `StagedNodes` or an
array of staged nodes and `StagedNodes`; `{ build }` in a `pres_tree` takes one staged node,
a `StagedNodes` or such an array. A sequence passed back crosses as one handle whatever its
length, and its items never become engine objects:

```js
return Lowering.Emit([lw.text(cx, "["), lw.lower_nodes(cx, node.body()), lw.text(cx, "]")]);
```

`[...nodes]` (JavaScript) and `to_vec()?` (Rust) create one engine object per item: use them
only on a short list the handler needs whole (one argument's content), never on a body or a
traversal. A sequence has no array methods (`.map`, `.filter`, `.length`); use `len()`,
`get(i)` and `for … of`.

In the guest client (Rust): `Nodes` and `StagedNodes` are built empty (`Default`) or from a
`Vec`, extended with `push(item)` and `push_all(&other)` without a crossing, and passed as
`&Nodes` / `StagedNodes`; a host sequence inside one crosses back as its handle.
`elements` takes any list of strings, `kinds` a `&[NodeKindName]`; both answer
`Result<…>`. A Rust extension compiled into a program has no memory limit; the same code
serves both forms.

Module pages: `flm_ext::quickjs` (mapping rule 9, the limits), `flm_ext::guest` (mapping
rule 9), `flm_ext::adapter::dispatch` (the value shapes); the guide's [Traversals and
sequences](crate::guide::extending#traversals-and-sequences).

## Capabilities, collections, ordering edges and workflows

A capability is a registered dotted name for a state of processing that a stage brings
about, such as `flm.blocks.structured`: a stage declares the capabilities it provides and
the capabilities it requires, and a caller demands the capabilities it wants
([`CapabilityDef`](crate::core::registry::CapabilityDef)). How a plan is built from
them: [AI guide: pipeline](crate::guide::ai_guide_pipeline).

- [`CapabilityDef`](crate::core::registry::CapabilityDef): declare one only for a state
  no table represents. [`refines`](crate::core::registry::CapabilityDef::refines) makes
  it a stronger form of another: `flm.blocks.structured` and `flm.blocks.raw` refine
  `flm.blocks`, so providing either satisfies a demand for `flm.blocks`, and a plan
  demanding both is refused. At most one refinement may be marked
  [`default`](crate::core::registry::CapabilityDef::default); a demand for the parent
  alone selects it when nothing else decides.
- [`CollectionDef`](crate::core::registry::CollectionDef): a name for a set of
  capabilities that any feature may add to with
  [`collection_member`](crate::core::registry::FeatureBuilder::collection_member);
  demanding it demands every member. Nothing provides a collection, and a stage requiring
  one is [`PlanError::CollectionRequired`](crate::core::plan::PlanError::CollectionRequired).
  A member added to a collection no feature declares is dropped without a diagnostic, so
  a feature that ships a check stays registrable alone.
  [`VALIDATED`](crate::stdlib::VALIDATED) (`flm.validated`) is the standard one: the
  [`CHECK_WORKFLOW`](crate::stdlib::CHECK_WORKFLOW) demands it alone, and every
  rendering workflow of the standard features demands it beside its output, so a check
  you add to it runs in every standard workflow.
- **Ordering edges** say only "runs after (or before) the stage that provides this
  capability". An edge never causes a stage to be scheduled, and has no effect when the
  providing stage is not scheduled. A stage declares its own in
  [`Needs::after`](crate::core::stage::Needs::after) /
  [`before`](crate::core::stage::Needs::before); an
  [`OrderingDef`](crate::core::registry::OrderingDef) places edges on any registered
  stage from outside.
- [`WorkflowDef`](crate::core::registry::WorkflowDef): a workflow is a named set of
  demands together with the choices that go with them: `outputs`, extra `demands`,
  preferred stages (`prefer`), [`LoweringSelection`](crate::core::registry::LoweringSelection)s
  per backend (`lowering`), accepted unrenderable elements per backend (`unsupported`),
  render option values (`render_options`). It may extend another with
  [`extends`](crate::core::registry::WorkflowDef::extends), inheriting the fields it does
  not declare (a cycle is `BuildError::ExtendsCycle`). A caller selects it by name
  ([`Plan::for_workflow`](crate::core::plan::Plan::for_workflow); the `flm` program's
  profile, `-p text`). [`workflow_fields`](crate::core::registry::FeatureBuilder::workflow_fields)
  changes some fields of a workflow another feature may register.

## Lowering rules

Lowering rewrites each frozen tree into a presentation tree for one backend, by applying
the lowering rule selected for each element, in rounds, until only elements the backend
writes natively remain; the result is computed data in the table `flm.lowered.<backend>`
([backends, lowering and
outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)). A lowering rule
is a named registry entity written for one element: it declares what it requires, reads
and produces, and implements `Lower`, which stages the element's presentation for one
backend. To stage a node is to build it into the tree that a lowering rule or a rewrite
hook is producing, through the context the rule or hook is handed (for a rule, its
[`Lowerer`](crate::core::lowering::Lowerer)); the staged nodes exist in that tree only. The full mechanism: [Rendering](crate::guide::rendering#lowering).

[`FeatureBuilder::lowering_rule`](crate::core::registry::FeatureBuilder::lowering_rule)
takes [`LoweringRule`](crate::core::lowering::LoweringRule)`::new(rule name, element
name, `[`LoweringDecl`](crate::core::lowering::LoweringDecl)`, Arc<dyn
`[`Lower`](crate::core::lowering::Lower)`>, `[`DefaultMark`](crate::core::lowering::DefaultMark)`)`,
with `.docs(…)`.

- **A rule is written for an element, not for a backend**, and serves every backend
  whose plan selects it. A rule specific to one format is a second rule for the same
  element marked [`DefaultMark::OnBackends`](crate::core::lowering::DefaultMark::OnBackends);
  [`DefaultMark::Everywhere`](crate::core::lowering::DefaultMark::Everywhere) is the
  general default; [`DefaultMark::Never`](crate::core::lowering::DefaultMark::Never)
  marks a rule that is never a default: it is chosen where a plan request prefers it by
  name ([`LoweringSelection::prefer`](crate::core::registry::LoweringSelection::prefer)),
  or where it is the element's only candidate. Two
  defaults for one element on one backend, or two general defaults, are
  `BuildError::TwoDefaultRules`.
- **Lowering rule selection** chooses one rule per element and backend in a fixed order,
  over the rules not set aside: the rule the request prefers by name, else the backend's
  native emission, else a rule marked default on that backend, else a rule marked
  default everywhere, else the sole candidate
  ([`SelectedRules`](crate::core::plan::SelectedRules)). Two candidates with nothing to
  decide between them are [`PlanError::RuleContest`](crate::core::plan::PlanError::RuleContest).
  An element no rule lowers fails the plan unless the request accepts it as
  unrenderable: an accepted unrenderable element is an element the plan request lists,
  per backend, as one the backend need not render; the plan builds anyway, and each
  occurrence lowers to a childless `flm.pres.unsupported` node with one Error diagnostic
  ([`PlanRequest::unsupported`](crate::core::plan::PlanRequest::unsupported)).
- **A rule for a backend the feature does not register** makes peer declarations of
  that backend ([`peer_backend`](crate::core::registry::FeatureBuilder::peer_backend)),
  of each presentation element it produces and does not own
  ([`peer_element`](crate::core::registry::FeatureBuilder::peer_element)`::<P>`) and of
  each table another feature owns that it reads
  ([`peer_table`](crate::core::registry::FeatureBuilder::peer_table)`::<R>`). In a
  registry without them the rule is set aside (see [The registry
  build](#the-registry-build)), except that a table read with `reads_opt` leaves the rule
  running with the table absent. Never import the backend's feature to make such a rule
  build. Example:
  [`headings::latex::register`](crate::stdlib::features::headings::latex::register).
- [`Lower::lower`](crate::core::lowering::Lower::lower)`(&self, node: NodeRef<'_, Flm>,
  cx: &mut `[`LowerCx`](crate::core::lowering::LowerCx)`<'_>, lw: &mut dyn Lowerer)`
  returns [`Lowering::Default`](crate::core::lowering::Lowering::Default) (copy the node,
  lower its children) or `Lowering::Emit(Vec<BuildId>)` (these nodes replace it; an
  empty list removes it). The implementation is registered once and may run on many
  nodes in parallel: it keeps no state of its own.
- **Everything a rule reads or builds goes through the
  [`Lowerer`](crate::core::lowering::Lowerer)**, the only interface a lowering rule has to
  the run:
  - lowering parts of its node:
    [`lower_children`](crate::core::lowering::Lowerer::lower_children),
    [`lower_body`](crate::core::lowering::Lowerer::lower_body),
    [`lower_arg`](crate::core::lowering::Lowerer::lower_arg),
    [`lower_arg_named`](crate::core::lowering::Lowerer::lower_arg_named),
    [`lower_nodes`](crate::core::lowering::Lowerer::lower_nodes) (a run of the tree it
    works in), [`default`](crate::core::lowering::Lowerer::default) (what
    `Lowering::Default` does, as one part of a larger result);
  - building nodes: `pres_with` and `pres_slots_with` (a presentation node with a
    payload, and one body or several named content slots),
    [`text`](crate::core::lowering::Lowerer::text),
    [`splice`](crate::core::lowering::Lowerer::splice) (lowers a synthesized value where
    the node being lowered is placed, after bringing it up to date),
    [`lower_region`](crate::core::lowering::Lowerer::lower_region) (lowers an argument or
    content slot of a node of another tree, a document node a value points at, as that
    tree's own content; the argument or content slot is a [node
    region](crate::core::lowering::NodeRegion); **the caller must lower each node region
    once**, and the lowering driver ([`crate::stdlib::backends::lowering`], the code
    that applies the selected rules) does not check it);
  - reading: `get` (a table the rule declared), `option` (an option type it declared),
    [`producer_of`](crate::core::lowering::Lowerer::producer_of),
    [`backend`](crate::core::lowering::Lowerer::backend),
    [`rule`](crate::core::lowering::Lowerer::rule),
    [`lang`](crate::core::lowering::Lowerer::lang) (to compile a text of the document
    written in a language of its own, as a list's label template),
    [`addr`](crate::core::lowering::Lowerer::addr) /
    [`addr_of`](crate::core::lowering::Lowerer::addr_of),
    [`tree_id`](crate::core::lowering::Lowerer::tree_id),
    [`layout`](crate::core::lowering::Lowerer::layout),
    [`placement`](crate::core::lowering::Lowerer::placement),
    [`comment_start`](crate::core::lowering::Lowerer::comment_start);
  - reporting: [`require`](crate::core::lowering::Lowerer::require) (records a
    requirement of this node's output: a requirement is an identifier a lowering rule or
    an emitter records to say that its output needs a definition, a style or script
    definition on HTML or a package request or a definition piece on LaTeX; the backend
    that writes the output resolves the requirements recorded for a lowered tree), `error`, `warning` and `note` (a diagnostic at a
    span), [`diagnose`](crate::core::lowering::Lowerer::diagnose).

  The methods that take a type parameter (`get`, `option`, `pres_with`,
  `pres_slots_with`, `error`, `warning`, `note`) are defined on `dyn Lowerer`, since the
  trait is object-safe.
- [`LoweringDecl`](crate::core::lowering::LoweringDecl) is read **when the plan is
  built**: [`requires`](crate::core::lowering::LoweringDecl::requires),
  [`reads`](crate::core::lowering::LoweringDecl::reads),
  [`reads_opt`](crate::core::lowering::LoweringDecl::reads_opt) and
  [`option`](crate::core::lowering::LoweringDecl::option) join the needs of the
  backend's lowering stage (a rule reading the resolutions table schedules the stage that
  writes it). A `reads_opt` table becomes a required read whenever the registry declares
  it and a stage writes it; it stays optional for a table no stage writes (an external
  table), and is left out for a table no feature declares (an unresolved peer table).
  [`produces`](crate::core::lowering::LoweringDecl::produces) lists every presentation
  element the rule may build; building another is
  [`LowerError::UndeclaredProduces`](crate::core::lowering::LowerError::UndeclaredProduces),
  except `flm.text` on a backend that writes it natively.
  [`selection_requires`](crate::core::lowering::LoweringDecl::selection_requires) lists
  the requirements the rule has whenever it is selected (style or script definitions, LaTeX
  package requests, definition pieces).
  [`unchecked_output`](crate::core::lowering::LoweringDecl::unchecked_output)`(backend,
  written, reader)` declares output that nothing checks before `reader` reads it on
  `backend`: a plan that selects the rule there keeps one
  [`PlanWarning::UncheckedOutput`](crate::core::plan::PlanWarning::UncheckedOutput) per
  backend and feature, reported at the start of every run as `flm.run.unchecked-output`
  ("Unchecked output for *reader*: feature ‘…’ writes *written* as is (backend ‘…’)").
  The rules of `math-passthrough` declare it on `html`, `latex-rendered` and
  `latex-recomposed`, and keep it under another feature's names
  ([`math_rule_named`](crate::math_passthrough::math_rule_named) and the other
  constructors), the warning then naming that feature.
- Lowering runs **rounds** until no node the backend does not write natively is left,
  bounded by [`rank`](crate::stdlib::backends::lowering::rank), computed when the plan
  is built; exhausting the bound is `flm.lowering.rounds-exceeded`
  ([`RoundsExceeded`](crate::stdlib::backends::lowering::RoundsExceeded)) and means a
  rule's `produces` is wrong.
- **A failure never panics and never stops the run**
  ([`lowering::error`](crate::stdlib::backends::lowering::error)). Recovered at one
  node, which lowers to a childless `flm.pres.unsupported` node with one Error:
  [`UnsupportedElement`](crate::stdlib::backends::lowering::UnsupportedElement),
  [`RuleFailed`](crate::stdlib::backends::lowering::RuleFailed) (the rule returned a
  [`LowerError`](crate::core::lowering::LowerError)),
  [`RegionNotFound`](crate::stdlib::backends::lowering::RegionNotFound),
  [`NoRule`](crate::stdlib::backends::lowering::NoRule). The whole tree fails, with no
  entry in [`LoweredTrees`](crate::stdlib::backends::lowering::LoweredTrees) and one Error
  at the root: [`RoundsExceeded`](crate::stdlib::backends::lowering::RoundsExceeded),
  [`LoweringFailed`](crate::stdlib::backends::lowering::LoweringFailed),
  [`NestingTooDeep`](crate::stdlib::backends::lowering::NestingTooDeep). A rule that
  leaves out part of the content it lowers may stage a `flm.pres.dropped` node, the
  dropped mark ([`DroppedPayload`](crate::stdlib::backends::pres::DroppedPayload)),
  where that content was.

### The presentation vocabulary

The presentation elements the standard backends write, `flm.pres.<name>`, all in
[`crate::stdlib::backends::pres`] ([`ALL`](crate::stdlib::backends::pres::ALL) lists
them): [`PAR`](crate::stdlib::backends::pres::PAR),
[`BLOCK`](crate::stdlib::backends::pres::BLOCK),
[`BLOCK_HEADING`](crate::stdlib::backends::pres::BLOCK_HEADING),
[`RUNIN_HEADING`](crate::stdlib::backends::pres::RUNIN_HEADING),
[`SPAN`](crate::stdlib::backends::pres::SPAN),
[`LIST`](crate::stdlib::backends::pres::LIST),
[`CELLS`](crate::stdlib::backends::pres::CELLS),
[`LINES`](crate::stdlib::backends::pres::LINES) /
[`LINE`](crate::stdlib::backends::pres::LINE),
[`VERBATIM`](crate::stdlib::backends::pres::VERBATIM),
[`MATH_PASSTHROUGH`](crate::stdlib::backends::pres::MATH_PASSTHROUGH),
[`MATH_TEXT`](crate::stdlib::backends::pres::MATH_TEXT),
[`LINK`](crate::stdlib::backends::pres::LINK),
[`IMAGE`](crate::stdlib::backends::pres::IMAGE),
[`UNSUPPORTED`](crate::stdlib::backends::pres::UNSUPPORTED),
[`DROPPED`](crate::stdlib::backends::pres::DROPPED),
[`UNKNOWN`](crate::stdlib::backends::pres::UNKNOWN),
[`DECO_RUNIN`](crate::stdlib::backends::pres::DECO_RUNIN) /
[`DECO_FINAL`](crate::stdlib::backends::pres::DECO_FINAL) /
[`DECO_MARGIN`](crate::stdlib::backends::pres::DECO_MARGIN),
[`DECO_MARK`](crate::stdlib::backends::pres::DECO_MARK),
[`SUBTREE`](crate::stdlib::backends::pres::SUBTREE).

- Each node carries exactly one typed **payload**, of the type its element declares
  ([`crate::stdlib::backends::pres::payload`]):
  [`SpanPayload`](crate::stdlib::backends::pres::SpanPayload) with
  [`SpanStyle`](crate::stdlib::backends::pres::SpanStyle);
  [`LinkPayload`](crate::stdlib::backends::pres::LinkPayload) (`href` is `#anchor`
  inside the document, an address otherwise; `label`, the link's accessible name;
  [`shows_target`](crate::stdlib::backends::pres::LinkPayload::shows_target));
  [`LinePayload`](crate::stdlib::backends::pres::LinePayload) with its two
  indentations as [`Length`](crate::stdlib::backends::pres::Length)s (an exact amount
  and a closed unit, `1tab`, `1.5em`; never a bare number) and
  [`LineAlign`](crate::stdlib::backends::pres::LineAlign);
  [`CellsPayload`](crate::stdlib::backends::pres::CellsPayload) (a whole grid: `rows`,
  `cols`, one [`CellSpec`](crate::stdlib::backends::pres::CellSpec) per entry in
  row-major order, entry *i*'s content in the node's content slot *i*, named `cell`;
  [`grid()`](crate::stdlib::backends::pres::CellsPayload::grid) gives each position as
  a [`GridPosition`](crate::stdlib::backends::pres::GridPosition), and
  [`cells_of`](crate::stdlib::backends::pres::cells_of) reads the payload with the
  slots); [`ListPayload`](crate::stdlib::backends::pres::ListPayload) (`kinds`,
  `anchor`, and per entry in order `entry_anchors` and `entry_levels`: HTML writes an
  entry's anchor as the `id` and its level as `data-flm-level` of its `<li>` or `<dt>`;
  only the table of contents gives levels, each entry its heading's);
  [`DroppedPayload`](crate::stdlib::backends::pres::DroppedPayload)
  (`condition`, `detail`; the text backend prints `[dropped: <detail>]`); and the
  others of the module. No property bag, no handler per backend on a node.
- A role is a feature-owned dotted name on a presentation payload that says what a
  container is (a theorem's heading, a quotation's attribution), and kinds are
  feature-owned names that classify it further, so that a stylesheet or a host's lowering
  rule can select on them ([`pres::payload`](crate::stdlib::backends::pres::payload);
  [decorations, roles and
  kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)). The payload's
  anchor field is a plain `String`, the anchor already computed by the `refs` feature
  ([`ANCHORS`](crate::stdlib::features::refs::ANCHORS)). **An emitter that cannot
  express a payload field ignores it**: presentation may be lost, content never.
- A document role ([`DocRole`](crate::stdlib::backends::pres::DocRole): `Endnotes`,
  `Footnote`, `NoteRef`, `BackLink`, `Bibliography`, `BiblioRef`) is one of a small fixed
  set of structural roles, held in the field `doc_role` of `BlockPayload`, `SpanPayload`
  and `LinkPayload`. The HTML backend writes it as `role="doc-…"`, and `LinkPayload::label`
  as `aria-label`; the text backend ignores both.
- The text backend writes a `Superscript` or `Subscript` span's text through the Unicode
  forms of [`superscript`](mod@crate::stdlib::backends::text::superscript) (`¹⁰`, `ᵃ`),
  a character with no such form unchanged; it drops every other span style.
- Emitters and rules read a presentation tree with
  [`crate::stdlib::backends::pres::read`]:
  [`is_presentation`](crate::stdlib::backends::pres::is_presentation),
  [`payload_of`](crate::stdlib::backends::pres::payload_of),
  [`body_of`](crate::stdlib::backends::pres::body_of),
  [`list_entries`](crate::stdlib::backends::pres::list_entries),
  [`plain_text`](crate::stdlib::backends::pres::plain_text),
  [`recompose_source`](crate::stdlib::backends::pres::recompose_source),
  [`attached_slots`](crate::stdlib::backends::pres::attached_slots),
  [`decoration_at`](crate::stdlib::backends::pres::decoration_at).
- **Decorations.** A decoration is a node whose element carries the `Decoration` aspect
  (a run-in heading, a margin note, a closing mark): when a rule stages the presentation
  node that contains it, the lowering driver moves it out of the content run into an
  attached slot of that node and leaves a `flm.pres.deco-mark` node where it was
  ([`Decoration`](crate::stdlib::backends::pres::Decoration)). The mark's
  [`DecoMarkPayload`](crate::stdlib::backends::pres::DecoMarkPayload) gives the slot
  index. A rule stages a decoration as an ordinary child; the driver decides by the
  aspect, not by the three standard names. Four standard features stage decorations:
  `headings` (a run-in heading), `defterm`, `theorems` and `annotations`. A backend that
  writes any element carrying the aspect natively must write `flm.pres.deco-mark`
  natively too (a native-set implication, below).
- **Opaque content.** [`OpaqueContent`](crate::stdlib::backends::pres::OpaqueContent)
  marks an element whose content is source text: the driver copies the subtree
  unchanged. [`ContentInOpaque`](crate::stdlib::backends::pres::ContentInOpaque) marks
  the exception, an element that holds content even inside such a subtree, which the
  driver lowers in the ordinary rounds: `flm.pres.math-text` (payload
  [`MathTextPayload`](crate::stdlib::backends::pres::MathTextPayload)) is the one
  element of the vocabulary that carries it, among the direct children of a
  `flm.pres.math-passthrough` body. An emitter that prints opaque content as source writes
  such a node's content itself ([opaque
  regions](crate::guide::concepts_overview#opaque-regions)).
- [`presentation_element`](crate::core::registry::FeatureBuilder::presentation_element)`::<P>`
  declares an element of your own: needed only when no element of the vocabulary carries
  what your element means, and useful only once a backend writes it natively.

### The LaTeX construct elements

A construct element is a presentation element that mirrors one fundamental construct of
an output format (an HTML tag; a LaTeX macro call, environment, raw text or definition),
so that format-specific output is structured data in the lowered tree rather than a
string ([backends, lowering and
outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)). The LaTeX
family's four ([`crate::stdlib::backends::latex::construct`]) are registered by the
`latex` feature, written natively by every LaTeX backend, and not in
[`ALL`](crate::stdlib::backends::pres::ALL); a rule that must write LaTeX that no shared
element expresses stages one of them, never a string.

| element | payload | content slots |
|---|---|---|
| [`MACRO`](crate::stdlib::backends::latex::construct::MACRO) `flm.pres.latex.macro` | [`LatexMacroPayload`](crate::stdlib::backends::latex::LatexMacroPayload) `{name, args, packed}`: one [`LatexArgShape`](crate::stdlib::backends::latex::LatexArgShape) per argument written (`Star`, `Optional`, `Mandatory`, `EscVerb`: verbatim content in the `\EscVerb` encoding); `packed` hands every argument over in one brace group, one pair of braces around each | one [`ARG_SLOT`](crate::stdlib::backends::latex::construct::ARG_SLOT) per argument with content, in order |
| [`ENVIRONMENT`](crate::stdlib::backends::latex::construct::ENVIRONMENT) `flm.pres.latex.environment` | [`LatexEnvironmentPayload`](crate::stdlib::backends::latex::LatexEnvironmentPayload) `{name, args, verbatim, packed, body_argument}`: `verbatim` writes the body byte for byte on lines of its own; `body_argument` for an environment declared to take its body as an argument (`+b`) | the argument slots, then `body` |
| [`RAW`](crate::stdlib::backends::latex::construct::RAW) `flm.pres.latex.raw` | [`LatexRawPayload`](crate::stdlib::backends::latex::LatexRawPayload) `{text}`, written byte for byte: a blank line, a parameter, a rebuilt equation line, a literal's spelling; never document text | none |
| [`DEFINITION`](crate::stdlib::backends::latex::construct::DEFINITION) `flm.pres.latex.definition` | [`LatexDefinitionPayload`](crate::stdlib::backends::latex::LatexDefinitionPayload) `{kind, name, source_name, args, packed, robust, body_argument, feature}`: a macro or environment definition, or a feature's declaration ([`LatexDefinitionKind`](crate::stdlib::backends::latex::LatexDefinitionKind): `Macro`, `Environment`, `Declaration`), written into the style file and never where the node occurs in the tree, once per name. `name` is the name written ([`recomposed_name`](crate::stdlib::backends::latex::names::recomposed_name) for a user definition), `source_name` the name the document declared; `args` the number of arguments the definition takes, all mandatory, `packed` handing them over in one group (how a definition of more than nine is written, its body reading argument *n* with `\flmArg`); `robust` written through the document command interface (the render option `robust_names`); `body_argument` an environment taking its body as a `+b` argument; `feature` the feature that writes the definition from its configuration (then always guarded) | `body` for a macro or a declaration; `begin` and `end` for an environment |

**Requirements beside them.** A rule requires (records a requirement for) a LaTeX
package as `flm.latex.pkg.<name>` ([`packages`](crate::stdlib::backends::latex::packages),
constants such as [`AMSMATH`](crate::stdlib::backends::latex::packages::AMSMATH)) and a
definition piece as `flm.latex.def.<name>`
([`preamble`](crate::stdlib::backends::latex::preamble), such as
[`VERBATIM`](crate::stdlib::backends::latex::preamble::VERBATIM)), through
[`LoweringDecl::selection_requires`](crate::core::lowering::LoweringDecl::selection_requires)
or [`Lowerer::require`](crate::core::lowering::Lowerer::require). To contribute your own,
see [LaTeX package requests and definition
pieces](#latex-package-requests-and-definition-pieces).

**The typed sink.** An emitter writes through
[`LatexSink`](crate::stdlib::backends::latex::LatexSink):
[`text`](crate::stdlib::backends::latex::LatexSink::text) escapes document text under an
[`EscapeConfig`](crate::stdlib::backends::latex::EscapeConfig) with a
[`UnicodeMode`](crate::stdlib::backends::latex::UnicodeMode);
[`raw`](crate::stdlib::backends::latex::LatexSink::raw) takes a
[`LatexRaw`](crate::stdlib::backends::latex::LatexRaw), which only a raw payload or the
document's own source can produce, and refuses a text a rule assembled that holds a `%`
starting a comment
([`RawCommentRefused`](crate::stdlib::backends::latex::RawCommentRefused);
[`comment_percent`](crate::stdlib::backends::latex::comment_percent));
[`comment`](crate::stdlib::backends::latex::LatexSink::comment) is the only way a LaTeX
comment reaches the output, and writes the newline that ends it (the caller writes the
source's following whitespace minus that newline);
[`escverb`](crate::stdlib::backends::latex::LatexSink::escverb) encodes for
`\EscVerb`; [`verbatim_block`](crate::stdlib::backends::latex::LatexSink::verbatim_block)
writes a verbatim environment's content, where `%` is an ordinary character.

**The style file.** A document-scope regions stage assembles the style file from the
resolved requirements, the sink's [`Declared`](crate::stdlib::backends::latex::Declared)
record, the [`RecomposedDefinition`](crate::stdlib::backends::latex::RecomposedDefinition)s
the emitter recorded, and the three texts of LaTeX the document's author gives for before
the packages, after them and the end of the file
([`ExtraPreamble`](crate::stdlib::backends::latex::style::ExtraPreamble); the order of
the file is in [`style`](crate::stdlib::backends::latex::style)). Every request for one
package is merged into one `\RequirePackage` line per engine, its options the union in
the order first found, under a comment line listing every identifier behind it. The
`latex-recomposed` backend calls
[`style_body`](crate::stdlib::backends::latex::style::style_body), then
[`content_hash`](crate::stdlib::backends::latex::style::content_hash) to name the file
after its content, then
[`with_header`](crate::stdlib::backends::latex::style::with_header);
[`assemble_style`](crate::stdlib::backends::latex::assemble_style) does both steps for a
name already chosen.

### The HTML construct element

[`TAG`](crate::stdlib::backends::html::construct::TAG) (`flm.pres.html.tag`;
[`crate::stdlib::backends::html::construct`]) is registered by the `html` feature,
written natively by the HTML backend, and not in
[`ALL`](crate::stdlib::backends::pres::ALL): one HTML tag, for a rule that must put in
the page something no shared element carries. Its payload,
[`HtmlTagPayload`](crate::stdlib::backends::html::HtmlTagPayload) `{tag, attributes,
body, block}`, holds the tag name, the attributes as name–value pairs in the order
written, the content ([`TagBody`](crate::stdlib::backends::html::TagBody): `Children`,
the node's one `body` slot, or `Markup`, already-formed HTML, and no children), and whether the tag is block-level, which decides the line breaks around it.
Constructor: `HtmlTagPayload::new(tag).attributes([…]).markup(…)`.

A tag construct is outside the HTML contract (the classes and attributes the emitter
derives from the presentation vocabulary; [the HTML
contract](crate::guide::rendering#the-html-contract)) and outside the policy that checks
the addresses a link or an image may carry: its rule writes its own classes, attributes
and addresses and is responsible for them. A `Markup` body is created with
[`Markup::trusted`](crate::stdlib::backends::html::Markup::trusted), whose documentation
states what the caller guarantees: the bytes are safe to place in a page, and anything
from an input the author does not control is sanitized first. Bytes reach a page
unescaped in two ways only: that constructor, and
[`HtmlSink::raw_text_element`](crate::stdlib::backends::html::HtmlSink::raw_text_element)
(the content of a `<style>` or a `<script>`).

**The writing layer.** An emitter writes through
[`HtmlSink`](crate::stdlib::backends::html::HtmlSink):
[`text`](crate::stdlib::backends::html::HtmlSink::text) escapes document text;
[`open_block`](crate::stdlib::backends::html::HtmlSink::open_block) and
[`open_inline`](crate::stdlib::backends::html::HtmlSink::open_inline) return a
[`TagBuilder`](crate::stdlib::backends::html::TagBuilder) whose
[`attr`](crate::stdlib::backends::html::TagBuilder::attr) escapes and quotes every
value and [`attr_json`](crate::stdlib::backends::html::TagBuilder::attr_json) does the
same in single quotation marks for a value that is JSON;
[`close`](crate::stdlib::backends::html::HtmlSink::close) closes the innermost element;
[`raw_text_element`](crate::stdlib::backends::html::HtmlSink::raw_text_element) writes
a `<style>` or a `<script>` (those two tags only) and refuses content holding that
element's closing tag. Every entry point that takes a tag or attribute name requires the
caller to have checked it. The contract's names come from
[`class_of_element`](crate::stdlib::backends::html::class_of_element),
[`class_of_role`](crate::stdlib::backends::html::class_of_role),
[`data_attribute`](crate::stdlib::backends::html::data_attribute) and
[`style_property`](crate::stdlib::backends::html::style_property) (a payload's length
field as a custom property in `style`, valued by
[`css_length`](crate::stdlib::backends::html::conventions::css_length), or a ratio of
two length fields,
[`Written::Ratio`](crate::stdlib::backends::html::conventions::Written::Ratio); nothing
else enters `style`); a block's tag comes from its role through
[`BLOCK_TAGS`](crate::stdlib::backends::html::conventions::BLOCK_TAGS) (a feature never
chooses a tag); a name taken from a payload is checked with
[`is_valid_tag_name`](crate::stdlib::backends::html::is_valid_tag_name) and
[`is_valid_attribute_name`](crate::stdlib::backends::html::is_valid_attribute_name), an
address with [`check_address`](crate::stdlib::backends::html::check_address)
([`AddressUse`](crate::stdlib::backends::html::AddressUse)) under the value of the
render option [`HtmlAllowedUrlSchemes`](crate::stdlib::backends::html::HtmlAllowedUrlSchemes) that the
rule declares a read of.

### Recipe: stage an HTML tag from a lowering rule

A rule for the `html` backend alone, staging a tag whose content is markup this feature
vouches for. The feature registers neither the backend nor the element, so it makes peer
declarations of both: a registry that holds the HTML backend (one built with the standard
features does) takes the rule like any other, and one that does not still builds, with
the rule set aside.

The element here, `flm.math`, already has a rule marked as the default on this backend
(`math-passthrough`'s [`MATH_RULE`](crate::math_passthrough::MATH_RULE), marked for every backend), and one
element may have **one** default per backend, or the registry build fails with
`TwoDefaultRules`. So this rule is marked
[`DefaultMark::Never`](crate::core::lowering::DefaultMark::Never), and the plan request
prefers it by name, which comes first in lowering rule selection and wins over every
default. A rule for an element that has no default on that backend is marked
`DefaultMark::OnBackends` instead and needs nothing from the caller.

```rust
use std::sync::Arc;

use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, LoweringSelection, PackageEntry, RegistryBuilder,
    RegistryInputs,
};
use flm_core::lang::Flm;
use flm_core::lang::structural::MATH;
use flm_core::model::names::ElementName;
use flm_core::stdlib::backends::html::{self, HtmlTagPayload, Markup};
use flm_core::stdlib::{self, StdlibConfig};
use flm_core::techy::core::node::NodeRef;

const RULE: &str = "picture.math.lower.html";

struct MathAsPicture;

impl Lower for MathAsPicture {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        // What this feature's own stage typeset, as HTML this feature vouches
        // for: `Markup::trusted` places these bytes in the page unescaped, so
        // sanitizing them is this feature's responsibility.
        let picture = Markup::trusted("<svg viewBox=\"0 0 16 16\"></svg>");
        let payload = HtmlTagPayload::new("span")
            .attributes([("class", "picture-math"), ("role", "math")])
            .markup(picture);
        let tag = lw.pres_with(cx, &ElementName::from(html::TAG), payload, Vec::new())?;
        Ok(Lowering::Emit(vec![tag]))
    }
}

struct Picture;

impl FlmFeature for Picture {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("picture", "Mathematics as a picture")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.peer_backend(html::BACKEND, "the HTML backend this rule is written for")?;
        b.peer_element::<HtmlTagPayload>(html::TAG, "the HTML tag this rule stages")?;
        b.lowering_rule(LoweringRule::new(
            RULE,
            MATH,
            LoweringDecl::new().produces(html::TAG),
            Arc::new(MathAsPicture),
            // `flm.math` already has a default rule on this backend, so this
            // rule is chosen only where a plan request prefers it by name.
            DefaultMark::Never,
        ))?;
        Ok(())
    }
}

let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
builder.feature(Arc::new(Picture));
let registry = builder.build(&RegistryInputs::new())?;

// The element resolves: the `html` feature registers it.
assert!(
    registry
        .peer_elements()
        .iter()
        .any(|entry| entry.name.as_str() == html::TAG && entry.resolved)
);
// The standard features include the `html` backend, so the rule takes part like
// any other: it is not set aside. In a registry built without that backend the
// same rule would be set aside instead, with nothing about the rule to change.
assert!(!registry.set_aside_rules().any(|entry| entry.rule.name.as_str() == RULE));

// The plan request prefers the rule by name on this backend, and it is
// selected for the element in place of `math-passthrough`'s own.
let request = PlanRequest::new()
    .demand(html::DOCUMENT.name())
    .lowering(html::BACKEND, LoweringSelection::new().prefer(RULE));
let plan = Plan::build(&registry, &request)?;
let selected = plan.selection(html::BACKEND).expect("the html backend is active");
let chosen = selected.rule_for(MATH).expect("a rule for mathematics");
assert_eq!(registry.rule(chosen).rule.name.as_str(), RULE);
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Adding a backend and an output

A backend is the registration bundle for one output format: the elements its emitter
writes natively, a lowering stage, an emit stage and a final stage
([`BackendDef`](crate::core::registry::BackendDef)).

- [`BackendDef::new`](crate::core::registry::BackendDef)`(name, native elements,
  lowering, emit, final_stage)`, registered with
  [`backend`](crate::core::registry::FeatureBuilder::backend), which registers the three
  stages as this feature's own, in that order.
- **Do not write the lowering stage**: create
  [`LoweringStage`](crate::stdlib::backends::lowering::LoweringStage) for the backend's
  name and declare the table it writes with
  [`table_named`](crate::core::registry::FeatureBuilder::table_named) under
  [`lowered_table_name`](crate::stdlib::backends::lowering::lowered_table_name)
  (`flm.lowered.<backend>`). Write the emitter (an emitter is a backend's emit stage: it
  walks the lowered trees and writes the output's bytes into a table, from which the
  backend's final stage makes the artifact set) and the final stage.
- The native set must satisfy every
  [`NativeImplication`](crate::core::registry::NativeImplication) of the registry, else
  [`BuildError::NativeSetIncomplete`](crate::core::registry::BuildError::NativeSetIncomplete)
  `{ backend, because, must_claim }`. The `lowering` feature declares two
  ([`native_implications`](crate::stdlib::backends::lowering::native_implications)),
  because the driver stages these elements with no rule declaring them:
  `NativeImplication::always(UNSUPPORTED)` (every backend writes
  `flm.pres.unsupported`) and `NativeImplication::aspect::<dyn Decoration>(DECO_MARK)` (a
  backend writing any decoration element writes the decoration mark). Declare your own
  with [`native_implication`](crate::core::registry::FeatureBuilder::native_implication)
  for anything your code stages without a rule declaring it; read a registry's with
  [`FlmRegistry::native_implications`](crate::core::registry::FlmRegistry::native_implications).
- [`OutputDef::new`](crate::core::registry::OutputDef)`(name, capability, artifact-set
  table, media type, extension)`: the table must hold an
  [`ArtifactSet`](crate::core::artifact::ArtifactSet)
  (`BuildError::OutputTableNotArtifactSet`), and the capability must be the table's or a
  refinement of it (`OutputCapabilityMismatch`). A
  [`WorkflowDef`](crate::core::registry::WorkflowDef) that lists the output makes it
  available by name.
- An output made from the files of another output (a PDF compiled from the files of
  `latex-rendered`): no backend; a stage reading the other output's table (declared in its
  `Needs`, which schedules that table's writers) and writing the output's artifact set;
  [`OutputDef::made_from`](crate::core::registry::OutputDef::made_from)`(other)` on the
  declaration, so a host prepares the other output too (the `flm` program sets up its
  compose template: `-t NAME` reaches `latex-rendered` under a PDF output); a name no
  feature declares as an output: `BuildError::Undeclared` (site `OutputMadeFrom`). The
  stage writes each file at [`path_of`](crate::core::artifact::path_of)`(artifact,
  Some(output), namer, scope)`: the fixed path, else the namer of the slot
  `flm.artifacts.namer` under `NamingScope::of(registry, plan demands)`, the path the
  host's writer uses, so the links between the files hold.
- **An output made of regions.** An output region is one named part of an output's entry
  artifact (the document's content, each piece placed beside it, and on HTML the
  stylesheets, the scripts, the title and the language), which the backend writes once
  into the table `flm.output.<backend>.regions` ([`Regions`](crate::core::region::Regions)).
  A backend whose entry artifact a host may assemble its own way splits its final work as
  the four standard backends do: a stage `flm.<backend>.regions` writes the regions,
  and the backend's final stage assembles the entry artifact from them in a layout of its
  own ([output regions and the final
  stage](crate::guide::concepts_overview#output-regions-and-the-final-stage)). It then
  contributes a [`RegionsOutput`](crate::core::region::RegionsOutput) to the service slot
  `flm.output.regions`
  ([`REGIONS_SLOT`](crate::stdlib::backends::lowering::REGIONS_SLOT)), so that a host's
  own assembler can serve the output. The final stage adds the files written beside the
  entry with [`ArtifactSet::insert_entry_with`](crate::core::artifact::ArtifactSet::insert_entry_with).
- A backend registers its own rules only (the `latex-recomposed` backend registers two,
  for paragraphs and paragraph breaks). A feature that wants its element written in the
  new format ships the rule itself, with peer declarations of the backend and of the
  format's presentation elements; the backend depends on no content feature.
- Worked example, in one `register`:
  [`TextBackend`](crate::stdlib::backends::text::TextBackend).

**Recipe: a backend and an output in JavaScript** (worked example:
`flm-ext/tests/js/typst/`, `main.js` and `emit.js`):

1. Names: the backend's and the output's name is the extension's stem or begins with
   `STEM.` (for an extension `profile-NAME`, also `NAME` or `NAME.…`, as for every name it
   declares); `lowered_table_name(backend)` (`flm.lowered.<backend>`) is accepted by
   `table_named`. Call the API's functions inside `register`, not at a module's top level.
2. Tables: `b.table_named(lowered_table_name(B), StandardTable.LoweredTrees, docs)`; a table
   of your own for the emitted text (`b.table`); `b.table_named(DOC, StandardTable.ArtifactSet, docs)`.
3. Emit stage, a stage object: `Effect.Compute`, `Scope.Document`, `needs().reads(lowered)`;
   `cx.get(lowered).lowered` answers `trees()`, `tree(id)` (a tree or `null`; its
   `root()` is its root node), `requires(id)`, `rounds(id)`, `node_at(addr)` (the node at a
   position mark's address of the text output, `tree#0/5`, or `null`); walk `cx.trees()` in document
   order and read each node's `element_name()`, `payload()` (data or `null`), `body()`,
   `children()`, `is_chars()`, `chars()`, `span_content()`, `slot_count()`, `slot_name(i)`,
   `slot_content_nodes(i)`; push one text. Escape everything the format reserves.
4. Final stage, a stage object reading that text and pushing
   `Artifact.bytes(new ArtifactId(owner, name), media_type, content).as_entry()` (or
   `Artifact.folder(id, [[path, content], …])`) into the artifact set; a content is a text,
   `{ text }` or `{ base64 }` (standard alphabet, padded), the forms of the host service
   `programs`; an artifact's data form is documented on `flm_ext::api::core::artifact`. A
   stage that reads an artifact set gets `cx.get(name).projection` = `{ artifacts,
   violations }`, each artifact in that data form (contents `{ text }` when UTF-8, else `{
   base64 }`) plus `path`, where the host writes it (`null` without a fixed path and a
   namer); a push ignores `path`. An output made from such a set:
   `new OutputDef(…).made_from(name)`.
5. `b.backend(new BackendDef(B, native, new LoweringStage(B), emitStage, finalStage).docs(text))`.
   `native`: `ALL` (`flm:stdlib/backends/pres`), the structural names your emitter handles
   (`TEXT`, `GROUP`, `OPAQUE_WRAPPER`, `COMMENT`, `CONTAINER`, `PARBREAK` from
   `flm:lang/structural`), your
   own presentation elements.
6. Coverage: every element the documents can hold must reach the native set through the
   selected rules, else `PlanError::Incomplete`. The standard features register their rule
   for `flm.graphics.image` only for their own backends: add a rule of your own
   (`DefaultMark.OnBackends([B])`), for example one staging `flm.pres.unsupported`.
7. `b.output(new OutputDef(name, capability, table, media_type, extension).docs(text))`;
   `media_type_of(extension)` answers the library's media type for a known extension
   (`null` for `typ`, so the example declares `application/x-typst`).
8. A workflow in the package's `flm` block: `{ workflows: { typst: { outputs: ["typst"],
   demands: ["flm.validated"] } } }`; then `flm render -p typst`.
9. A presentation element of your own: `b.presentation_element(name, docs)`; its payload is
   any data value given to `lw.pres(cx, element, payload, children)` by a rule listing it in
   `produces`; mark that rule `OnBackends([B])` and give the element of the document a
   second rule marked `Everywhere` producing shared-vocabulary nodes for other backends.
10. Not available to a script: regions and `RegionsOutput` (no compose template for the
    output), the pieces stage, `native_implication`, `ordering`, `collection_member`.

### Style and script definitions

A style definition (`StyleDef`) or a script definition (`ScriptDef`) is a stylesheet or a
script that a feature's output depends on, recorded in the registry under an identifier; a
lowering rule or an emitter *requires* it by identifier, and the backend that writes the
output decides its *delivery*: written into the output, loaded from an address, or left
to the host ([`StyleDef`](crate::core::registry::StyleDef); [style and script
definitions](crate::guide::concepts_overview#style-and-script-definitions)).

- A rule requires one with
  [`LoweringDecl::selection_requires`](crate::core::lowering::LoweringDecl::selection_requires)
  (needed whenever the rule is selected) or
  [`Lowerer::require`](crate::core::lowering::Lowerer::require) (needed because of this
  node); both record an identifier in the lowered tree's requirements.
- A feature registers the definitions with
  [`style_def`](crate::core::registry::FeatureBuilder::style_def) /
  [`script_def`](crate::core::registry::FeatureBuilder::script_def); read them back with
  [`FlmRegistry::styles`](crate::core::registry::FlmRegistry::styles) and
  [`scripts`](crate::core::registry::FlmRegistry::scripts). The content is an ordered
  list of parts ([`DefContent`](crate::core::registry::DefContent)): `Inline` text, a
  `File` the host reads, or a `Url` the output loads, written in that order.
- On the HTML backend, the stage `flm.html.resolve-definitions`
  ([`ResolveDefinitionsStage`](crate::stdlib::backends::html::ResolveDefinitionsStage))
  collects the requirements of every tree and of every document-level piece
  ([`Piece`](crate::core::piece::Piece)), adds the host's
  ([`required_definitions`](crate::stdlib::backends::html::HtmlRenderOptions::required_definitions))
  and the page's own stylesheet, resolves each identifier, applies the render option
  [`definitions`](crate::stdlib::backends::html::HtmlRenderOptions::definitions) (what
  the host delivers instead of the declared content, or `ProvidedByHost`), and writes
  `flm.html.definitions`
  ([`DEFINITIONS_TABLE`](crate::stdlib::backends::html::DEFINITIONS_TABLE)): every
  required definition once, the stylesheets first and then the scripts, each in
  registration order, with its kind, its owner and its delivery. The page writes the
  stylesheets in the head and the scripts at the end of the body.
- A rule requires its style identifier with no condition on the backend. Each backend
  resolves the identifier through its own definitions: the HTML backend through the
  stylesheet, the rendered LaTeX through a definition piece that delivers it; the text
  backend records requirements and reads none.
- Conditions: `flm.html.unknown-requirement` (Error: an identifier no feature declares;
  the requirement is dropped), `flm.html.unknown-definition` (Warning: an entry of the
  render option `definitions` that no feature declares; it is ignored),
  `flm.html.definition-not-inlinable` (Error: a file part, or a text holding the
  element's own closing tag, cannot be written into the page; that part is left out).
- A script that transforms elements of the page in the browser follows two rules so that
  a host can update one output region in place: [a page updated in
  place](crate::guide::rendering#a-page-updated-in-place).

### Recipe: ship a stylesheet with a feature

A feature whose content needs style rules of its own registers a style definition and has
its rule require it. The requirement is an identifier; where the stylesheet comes from is
the host's decision, and the feature says nothing about it.

```rust
use std::sync::Arc;

use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_core::core::registry::{
    BuildError, DefContent, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry, RegistryBuilder,
    RegistryInputs, StyleDef,
};
use flm_core::lang::Flm;
use flm_core::lang::structural::MATH;
use flm_core::model::names::{ElementName, Name};
use flm_core::stdlib::backends::html;
use flm_core::stdlib::backends::pres::{self, SpanPayload};
use flm_core::stdlib::{self, StdlibConfig};
use flm_core::techy::core::node::NodeRef;

/// The stylesheet's identifier: the feature's own name, then what it is for.
const STYLE: &str = "mathframe.style.frame";

struct AsFramedMath;

impl Lower for AsFramedMath {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        // Required with no condition on the backend: each backend resolves the
        // identifier through its own definitions.
        lw.require(&Name::from(STYLE));
        // The role `mathframe.box` becomes the class `mathframe-box` on the HTML
        // backend (`html::class_of_role`), the class the stylesheet selects on.
        let body = vec![lw.text(cx, "a formula")?];
        let payload =
            SpanPayload { role: Some(Name::from("mathframe.box")), ..SpanPayload::default() };
        Ok(Lowering::Emit(vec![lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)?]))
    }
}

struct MathFrame;

impl FlmFeature for MathFrame {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("mathframe", "Framed mathematics")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.style_def(StyleDef {
            id: Name::from(STYLE),
            // One cascade layer, every selector under a class this feature's
            // own output carries, every adjustable value a custom property:
            // the same three rules the standard stylesheets follow.
            content: vec![DefContent::Inline(String::from(
                "@layer flm {\n\
                 :root { --mathframe-border: 1px solid #999 }\n\
                 .mathframe-box { border: var(--mathframe-border) }\n\
                 }\n",
            ))],
            docs: String::from("the frame around a formula"),
        })?;
        b.peer_backend(html::BACKEND, "the HTML backend this rule is written for")?;
        // `DefaultMark::Never`: `math-passthrough` already has a default rule
        // for this element on this backend, and two default rules for one
        // element on one backend clash. A render request asks for this one by
        // name with `LoweringSelection::prefer`.
        b.lowering_rule(LoweringRule::new(
            "mathframe.math.lower.html",
            MATH,
            LoweringDecl::new().produces(pres::SPAN),
            Arc::new(AsFramedMath),
            DefaultMark::Never,
        ))?;
        Ok(())
    }
}

let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
builder.feature(Arc::new(MathFrame));
let registry = builder.build(&RegistryInputs::new())?;

// The definition is on the registry, under the feature that declared it.
let (owner, style) =
    registry.styles().iter().find(|(_, style)| style.id.as_str() == STYLE).expect("declared");
assert_eq!(owner.as_str(), "mathframe");
assert_eq!(style.content.len(), 1);
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

A document lowered with this rule (preferred by name, since it is registered
`DefaultMark::Never`) carries `mathframe.style.frame` in its tree's requirements; the
stage `flm.html.resolve-definitions` puts it in the definitions table after the
stylesheets of the features registered before this one, and the page writes it. A host
that delivers it itself gives the identifier in the render option `definitions`.

## Options, settings and services

A feature takes values in three forms that differ by when they are fixed: its
configuration (a Rust value fixed when the registry is built), its option values (set on
a document or on a render request), and its settings (the same values written as data)
([settings, options and
configuration](crate::guide::concepts_overview#settings-options-and-configuration)).

**Option types.** An option type is a plain Rust struct a feature declares once; its
values are set on a document or on a render request (the type's binding says which), and
a stage reads the value for an option type it declared a read of: the render value if one
is set, otherwise the document value, otherwise the type's default
([`crate::model::options`]). Declare it with
[`options`](crate::core::registry::FeatureBuilder::options)`::<T>(bind)` or
[`options_with_docs`](crate::core::registry::FeatureBuilder::options_with_docs); its
Rust type is its identity (option types have no names), and a second declaration of one
type is a build error. `T` implements [`Options`](crate::model::options::Options):
`Any + Send + Sync + Default + Hash + Debug` (the hash is part of a plan's identity and of
the record of how a tree was produced).

[`Bind`](crate::model::options::Bind):
[`Document`](crate::model::options::Bind::Document) (set with
[`Doc::with_options`](crate::model::doc::Doc::with_options)),
[`Render`](crate::model::options::Bind::Render) (set in
[`PlanRequest::render_options`](crate::core::plan::PlanRequest::render_options)),
[`Either`](crate::model::options::Bind::Either) (both; the render value replaces the
document's), [`Registry`](crate::model::options::Bind::Registry) (nowhere: the value is
part of the configuration and the feature holds it).

A stage or rewrite stage declares what it reads with
[`Needs::option`](crate::core::stage::Needs::option)`::<T>`, a rule with
[`LoweringDecl::option`](crate::core::lowering::LoweringDecl::option)`::<T>`, and reads
the value with [`StageCx::option`](crate::core::run::StageCx::option) or `option` on the
`Lowerer`. The lookup order is the same everywhere. Refused: reading an option type not
declared ([`OptionError`](crate::model::options::OptionError)); a render value whose
binding does not admit a render request (when the plan is built,
[`PlanError::RenderOption`](crate::core::plan::PlanError::RenderOption)); a document
value whose binding does not admit a document (at
[`Run::new`](crate::core::run::Run::new),
[`RunError::DocumentOption`](crate::core::run::RunError::DocumentOption)); a stage that
declares a read of a `Bind::Registry` option type
([`PlanError::RegistryBoundRead`](crate::core::plan::PlanError::RegistryBoundRead)), since
the read could only return the default.

**Settings.** Settings are a feature's configuration and option values written as data,
one map per feature from key to value, which the library reads into the feature's own
Rust types; a settings key is one field of those types ([`crate::core::settings`]). A
feature that takes settings implements
[`FlmFeature::settings`](crate::core::registry::FlmFeature::settings), returning a
[`SettingsDecl`](crate::core::settings::SettingsDecl):

- [`config::<C>(docs)`](crate::core::settings::SettingsDecl::config) declares the
  configuration type (or
  [`config_from::<D, C>(docs)`](crate::core::settings::SettingsDecl::config_from), a
  data type read and converted to the configuration; both panic for a field whose name
  is already a key, for a second configuration, and for a type not read from a map of
  named fields);
- [`options::<O>(bind, docs)`](crate::core::settings::SettingsDecl::options) declares
  each option type with its binding (it panics for `Bind::Registry`, for a field whose
  name is already a key, and for a type not read from a map of named fields; [the
  panicking items](crate::guide::panics)). The registry build declares these option types
  before `register` runs: `register` does not declare them again, and a package factory
  learns the declaration from the feature;
- the marks ([`Mark`](crate::core::settings::Mark): `merge-object-key-values`,
  `remove-key-when-value-is-false`, `set-to-default-with-null`, `path`, `markup`) belong
  to the schema's nodes: `json_schema` writes each node's marks under `x-flm-marks`
  ([`MARKS_KEYWORD`](crate::core::settings::MARKS_KEYWORD)), and a part declared from data
  (`config_data`, `options_data`, a feature in JavaScript's `settings`) writes them there
  itself, the default entries of a map combined key by key being its node's `default`
  (`additionalProperties` and `items` add the segment `*`, a `$ref` into `$defs` is
  followed). In Rust, the five mark calls take a path at any depth (form in
  [`KeyMark`](crate::core::settings::KeyMark)) and panic when it reaches no node of the
  declaration's schema;
- [`merge_entries(path, default_entries)`](crate::core::settings::SettingsDecl::merge_entries)
  marks an object combined key by key, such as a collection of named things read from a
  map with [`named::deserialize`](crate::core::settings::named::deserialize);
  `default_entries` is a `fn() -> Result<Value, String>` writing the default entries as
  data (usually `|| named::to_data(&Config::default().kinds, "name")`, with
  [`named::to_data`](crate::core::settings::named::to_data)); it also panics when
  `default_entries` fails or returns something other than a map. A host that combines
  several settings sources then combines the entries one at a time;
- [`remove_false(path)`](crate::core::settings::SettingsDecl::remove_false) marks an
  object in which a key whose value is `false` is removed (a collection of named things
  carries it beside `merge_entries`), and
  [`default_on_null(path)`](crate::core::settings::SettingsDecl::default_on_null) a
  setting that `null` gives its default;
- [`markup(path)`](crate::core::settings::SettingsDecl::markup) marks FLM text, and
  [`path(path)`](crate::core::settings::SettingsDecl::path) a path to a file.

Each type derives `serde::Deserialize` with `#[serde(default, deny_unknown_fields)]`
(an empty map reads as the default, a misspelled key is refused) and
`schemars::JsonSchema` (the schema, with each field's documentation as its description:
write its first paragraphs for a person editing a settings file). A crate outside the
library lists `serde` and `schemars` among its dependencies; no cargo feature of the
library is involved. [`SettingsDecl::read`](crate::core::settings::SettingsDecl::read)
reads a map; [`json_schema`](crate::core::settings::SettingsDecl::json_schema) returns
the schema. Example: [settings given as
data](crate::guide::extending#settings-given-as-data).

**Services.** A service is a value that can be shared across threads (behavior behind a
trait object, or plain data) which one feature supplies for another feature, or for the
embedding program, to use. A service slot is the declaration, by the feature that owns an
interaction, of a dotted name, the type of the values the slot takes and its arity
(at most one value, or any number); any feature, and the embedding program through the
registry builder, may contribute a value to it, and the registry build assembles all
contributions once ([`crate::core::services`]).

- Declare: [`service_slot`](crate::core::registry::FeatureBuilder::service_slot)`::<S>(name,
  `[`Arity`](crate::core::services::Arity)`, docs)`, `S` usually a trait-object type.
  `Arity::Single` takes at most one value (a second is a build error); `OrderedMany` any
  number, the `contribute_first*` values first, then the others, each group in
  contribution order.
- Contribute: `contribute` (by value type; when several slots take that type it fails with
  `ServiceError::AmbiguousSlot`, and `contribute_to` is needed), `contribute_to` (by name;
  a slot no feature declares is a build error, `ServiceError::UndeclaredSlot`), `contribute_first`,
  `contribute_first_to`; the embedding program uses
  [`RegistryBuilder::contribute`](crate::core::registry::RegistryBuilder::contribute) /
  [`contribute_to`](crate::core::registry::RegistryBuilder::contribute_to), recorded
  under the owner [`HOST_FEATURE`](crate::core::registry::HOST_FEATURE) and placed after
  every feature's contribution. A peer
  contribution, [`peer_slot`](crate::core::registry::FeatureBuilder::peer_slot)`::<S>(name,
  Arc<S>)`, fills a slot another feature owns and may not be registered: present, it
  fills it as `contribute_to` would; absent, the value is dropped with one Note
  (`flm.registry.unresolved-peer`).
- Require: [`require_service`](crate::core::registry::FeatureBuilder::require_service)
  makes an unfilled slot fail the registry build rather than a render.
- Read: [`StageCx::service`](crate::core::run::StageCx::service)`::<S>() -> Option<&S>`
  and [`services`](crate::core::run::StageCx::services)`::<S>()`; an absent service is
  never a failure in the middle of a render: report it as a diagnostic. A finalizer reads
  them through [`FeatureBuilder::services`](crate::core::registry::FeatureBuilder::services)
  and [`Services::shared_named`](crate::core::services::Services::shared_named).
- **The slots are assembled once, between the two registration rounds, and never
  change.** A value that exists only after some work (an index over a corpus) is not a
  service: install it in an external table, so a run's result cannot depend on when a
  stage read it.

The service slots of the standard features:

| slot | owner | value type | arity | for |
|---|---|---|---|---|
| `flm.numbering.counters` ([`COUNTERS_SLOT`](crate::stdlib::features::numbering::COUNTERS_SLOT)) | `numbering` | [`CounterDecl`](crate::stdlib::features::numbering::CounterDecl) | many | the counters a feature numbers by |
| `flm.numbering.styles` ([`STYLES_SLOT`](crate::stdlib::features::numbering::STYLES_SLOT)) | `numbering` | [`CounterStyleDecl`](crate::stdlib::features::numbering::CounterStyleDecl) | many | counter styles |
| `flm.numbering.within-joiners` ([`WITHIN_JOINERS_SLOT`](crate::stdlib::features::numbering::WITHIN_JOINERS_SLOT)) | `numbering` | [`WithinJoinerDecl`](crate::stdlib::features::numbering::WithinJoinerDecl) | many | code writing the ancestors' numbers before a counter's own |
| `flm.numbering.number-writers` ([`NUMBER_WRITERS_SLOT`](crate::stdlib::features::numbering::NUMBER_WRITERS_SLOT)) | `numbering` | [`NumberWriterDecl`](crate::stdlib::features::numbering::NumberWriterDecl) | many | code writing a counter style's numbers |
| `flm.numbering.parent-sets` ([`PARENT_SETS_SLOT`](crate::stdlib::features::numbering::PARENT_SETS_SLOT)) | `numbering` | [`ParentSetDecl`](crate::stdlib::features::numbering::ParentSetDecl) | many | named lists of counters a counter may be numbered within (the headings feature contributes `flm.headings.levels`) |
| `flm.xnotes.kinds` ([`KINDS_SLOT`](crate::stdlib::features::xnotes::KINDS_SLOT)) | `xnotes` | [`XnoteKind`](crate::stdlib::features::xnotes::XnoteKind) | many | xnote kinds; contribute with [`contribute_kind`](crate::stdlib::features::xnotes::contribute_kind) |
| `flm.floats.types` ([`TYPES_SLOT`](crate::stdlib::features::floats::TYPES_SLOT)) | `floats` | [`FloatType`](crate::stdlib::features::floats::FloatType) | many | float types; contribute with [`contribute_type`](crate::stdlib::features::floats::contribute_type) |
| `flm.floats.content` ([`CONTENT_SLOT`](crate::stdlib::features::floats::CONTENT_SLOT)) | `floats` | `dyn` [`FloatContentHandler`](crate::stdlib::features::floats::FloatContentHandler) | many | content handlers a float type refers to |
| `flm.cells.styles` ([`STYLES_SLOT`](crate::stdlib::features::cells::STYLES_SLOT)) | `cells` | [`CellStyle`](crate::stdlib::features::cells::CellStyle) | many | cell styles ([a cell style of your own](crate::guide::extending#a-cell-style-of-your-own)) |
| `flm.citations.providers` ([`PROVIDERS_SLOT`](crate::stdlib::features::citations::PROVIDERS_SLOT)) | `citations` | `dyn` [`CitationProvider`](crate::stdlib::features::citations::CitationProvider) | many | the citation entries of cited works, asked in order; never required |
| `flm.graphics.resolver` ([`RESOLVER_SLOT`](crate::stdlib::features::graphics::RESOLVER_SLOT)) | `graphics` | `dyn` [`GraphicsResolver`](crate::stdlib::features::graphics::GraphicsResolver) | single | finds and reads an image file |
| `flm.graphics.inspector` ([`INSPECTOR_SLOT`](crate::stdlib::features::graphics::INSPECTOR_SLOT)) | `graphics` | `dyn` [`GraphicsInspector`](crate::stdlib::features::graphics::GraphicsInspector) | single | an image's kind and size |
| `flm.graphics.converters` ([`CONVERTERS_SLOT`](crate::stdlib::features::graphics::CONVERTERS_SLOT)) | `graphics` | `dyn` [`GraphicsConverter`](crate::stdlib::features::graphics::GraphicsConverter) | many | image format conversions, asked in order |
| `flm.pieces` ([`pieces::SLOT`](crate::stdlib::backends::pieces::SLOT)) | `pieces` | [`PieceDecl`](crate::core::piece::PieceDecl) | many | the pieces features offer ([Contribute a piece](#contribute-a-piece)) |
| `flm.latex.packages` ([`packages::SLOT`](crate::stdlib::backends::latex::packages::SLOT)) | `latex` | [`LatexPackageDef`](crate::stdlib::backends::latex::LatexPackageDef) | many | package requests |
| `flm.latex.preamble` ([`preamble::SLOT`](crate::stdlib::backends::latex::preamble::SLOT)) | `latex` | [`LatexPreambleDef`](crate::stdlib::backends::latex::LatexPreambleDef) | many | definition pieces |
| `flm.artifacts.namer` ([`NAMER_SLOT`](crate::stdlib::backends::lowering::NAMER_SLOT)) | `lowering` | `dyn` [`ArtifactNamer`](crate::core::artifact::ArtifactNamer) | single | the host's naming of each artifact's path |
| `flm.output.regions` ([`REGIONS_SLOT`](crate::stdlib::backends::lowering::REGIONS_SLOT)) | `lowering` | [`RegionsOutput`](crate::core::region::RegionsOutput) | many | the outputs made of output regions |

A feature of your own that contributes to one of these slots uses `peer_slot`, so that it
still registers where the owning feature is absent.

## LaTeX package requests and definition pieces

A package request is a LaTeX package that a lowering rule or an emitter requires by
identifier (`flm.latex.pkg.<name>`), resolved through the service slot
`flm.latex.packages` to a package definition and merged with every other request for that
package into one `\RequirePackage` line per package and engine
([`packages`](crate::stdlib::backends::latex::packages)). A definition piece is a unit
of LaTeX definitions that a feature registers under an identifier `flm.latex.def.<name>`,
together with the identifiers it needs in turn; a lowering rule requires it by
identifier, and the style file holds the text of every piece the document requires, in
registration order ([`LatexPreambleDef`](crate::stdlib::backends::latex::LatexPreambleDef)).
A feature whose LaTeX rules need either contributes a
[`LatexPackageDef`](crate::stdlib::backends::latex::LatexPackageDef) to
[`packages::SLOT`](crate::stdlib::backends::latex::packages::SLOT) and a
`LatexPreambleDef` to [`preamble::SLOT`](crate::stdlib::backends::latex::preamble::SLOT)
(both owned by the `latex` feature), then requires their identifiers from its rules. A
definition piece's `requires` lists packages and other definition pieces alike, told apart
by their prefix;
the recomposed LaTeX resolves the closure.

A definition piece follows the rules of
[`preamble`](crate::stdlib::backends::latex::preamble) ("How a piece is written"): written for a LaTeX system of 2022 or later; no test of the
installation (no `\ifdefined` around a command, no test of the engine; what it needs, it
requires as a package); one guarded form per kind of definition
(`\ProvideDocumentCommand` for a macro offered to a user, `\ProvideDocumentEnvironment`
for an environment, `\providecommand` for an expandable macro, `\def` for an internal
macro whose name holds `@`, `\ifcsname … \else … \fi` around a counter or another
declaration), so a definition the user made first wins; comment lines in its text that
say what to redefine to change what; and, where its behavior is meant to be changed, a
LaTeX hook: a macro, or an environment such as `flmFloat`, that the recomposed LaTeX
writes and the style file defines with a plain default, which a user redefines in the
document's preamble ([LaTeX package requests, definition pieces and
hooks](crate::guide::concepts_overview#latex-package-requests-definition-pieces-and-hooks)).

```rust
use std::sync::Arc;

use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry, RegistryBuilder, RegistryInputs,
};
use flm_core::stdlib::backends::latex::{
    LatexPackageDef, LatexPreambleDef, packages, preamble,
};
use flm_core::stdlib::{self, StdlibConfig};

struct Boxes;

impl FlmFeature for Boxes {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("boxes", "Boxes")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        // `flm.latex.pkg.tcolorbox`, requested as `\RequirePackage[most]{tcolorbox}`.
        b.contribute_to(
            packages::SLOT,
            Arc::new(LatexPackageDef::new("tcolorbox", "framed boxes").options(["most"])),
        )?;
        // `flm.latex.def.box`, needing that package; guarded, with a hook.
        b.contribute_to(
            preamble::SLOT,
            Arc::new(
                LatexPreambleDef::new(
                    "box",
                    "% \\flmBox{<content>}: a framed box; redefine \\flmBoxStyle to restyle it.\n\
                     \\ProvideDocumentCommand\\flmBoxStyle{}{colframe=black}\n\
                     \\ProvideDocumentCommand\\flmBox{m}{\\begin{tcolorbox}[\\flmBoxStyle]#1\\end{tcolorbox}}\n",
                    "`\\flmBox{...}`: a framed box",
                )
                .requires([packages::package_id("tcolorbox")]),
            ),
        )?;
        Ok(())
    }
}

let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
builder.feature(Arc::new(Boxes));
let registry = builder.build(&RegistryInputs::new())?;

let services = registry.services();
let package = packages::packages_named(services, "flm.latex.pkg.tcolorbox")
    .next()
    .expect("contributed");
assert_eq!(package.requirepackage_line(), "\\RequirePackage[most]{tcolorbox}");
let piece = preamble::piece_named(services, "flm.latex.def.box").expect("contributed");
assert_eq!(piece.requires[0].as_str(), "flm.latex.pkg.tcolorbox");
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

An identifier no feature contributed is reported when the document is rendered
(`flm.latex-recomposed.unknown-requirement`), not when the registry is built.

A contributed request is not a line of the style file by itself. Every request for one
package (this feature's, another feature's, and the ones the document's author adds
through the render option `extra_packages`) is merged into the single `\RequirePackage`
line the file writes for that package under each LaTeX engine, its options the union of
theirs in the order first found, under a comment line listing every identifier behind it.
A request applies to every engine unless
[`.engines(…)`](method@crate::stdlib::backends::latex::LatexPackageDef::engines)
narrows it; where a package's lines differ by engine, the file writes them inside a test
of the engine (`\iftutex … \else … \fi`, or a three-way test, with the package `iftex`
requested first). So `requirepackage_line()` above shows what this request asks for, not
what the file will hold. Two requests that give one option key two different values
cannot be merged: `flm.latex-recomposed.package-options-conflict`, and no line is written
for that package under the engines where they conflict.

## Progress and cancellation in stages and services

Guide: [reporting progress from a stage or a
service](crate::guide::extending#reporting-progress-from-a-stage-or-a-service). The
build's progress sheet ([`crate::progress`]) is written by the work, read by the host.

- **Where the scope comes from**: a stage, [`StageCx::progress`](crate::core::run::StageCx::progress)
  (its row, owned by the run, shared by the per-tree calls); a citation provider,
  [`ProviderCx::progress`](crate::stdlib::features::citations::ProviderCx::progress) (the
  retrieve stage's row); a converter, the `progress: &Scope` argument of
  [`GraphicsConverter::convert`](crate::stdlib::features::graphics::GraphicsConverter::convert)
  (the row `flm.graphics.convert` declared for that conversion: declare nothing more
  unless the conversion has parts); an `flm-ext` host service, the last argument of
  `HostService::call`. A build reporting nowhere gives `Scope::none()`: call the methods
  anyway, never test "enabled".
- **Rows**: `scope.child(&KIND, label)` → `Pending`; `.start()` → `Task`,
  `.start_counted(Unit::Items, total)` → `Counted` (`advance(n)`, capped at the total);
  `finish()` ends as `Finished`; a dropped `Task`/`Counted` → `Failed`; a dropped `Pending`
  → `NotRun`. Only the declarer ends a row; a stage never ends its own stage row.
- **Rules**: a row per heavy item (a program run, a conversion, a fetch), an increment per
  light one (one of many small files); a total counts the slow units only (not cache
  hits) and never changes; declare possible work in advance and drop the `Pending` if
  unneeded (a late row moves the estimate back); per-tree stage: one row per call for its
  tree's items, labeled with the tree's source where the stage can tell it; document
  stage: one row for its items.
- **Kinds**: `const K: Kind = Kind::titled("acme.pages", "Rendering pages")` in your
  namespace; the name keys the host's table of expected durations (renaming loses it).
  Title: capital, "-ing" verb, no article, no period; an untitled row is skipped by
  displays.
- **Cancellation**: check `scope.cancelled()` once per unit (tree, image, formula, key),
  not in inner loops; on `true` return at once: the step's output is discarded, so it
  need not be meaningful, and report no diagnostic. Effects outside the store are not
  undone: check where what you leave is whole.
- **Log lines**: `scope.log(LogLevel::Info, text)` after `scope.log_enabled(level)` when
  the text is costly; text for a person, never a document problem. The `flm` program
  prints `Info` under `-v` only.
- **Extensions**: module `flm:progress` (JavaScript; `flm_ext::guest::progress` for Rust
  to WebAssembly, `flm_ext::api::progress` for compiled-in Rust): `cx.progress()` →
  `ProgressScope { child(kind, title, label), cancelled() }`, then `start()`,
  `start_counted(Unit.Items, n)`, `advance(n)`, `scope()`, `finish()`. Handles live for
  the call (unfinished → `Failed`, unstarted → `NotRun`); no depth limit, no `log`:
  `console.error`/`warn` → `Error`/`Warn`, `console.log`/`info`/`debug` → `Info`, source =
  the extension's name. On cancellation the engine stops a JavaScript handler, a
  WebAssembly module at its next host call; compiled-in Rust checks `cancelled()`. The
  `programs` service runs a program under a row `flm.programs.run` and stops it on
  cancellation (the call fails). Example: `extensions/js/math-mathjax/render.js`
  (`math-mathjax.formulas`, "Rendering formulas", advanced per formula).

## Caches of an extension

Guide: [Caches of an extension](crate::guide::extending#caches-of-an-extension). Types:
`flm_ext::host::CacheDeclaration` (and `CacheKind`, `CacheEngine`, `CacheSort`,
`CacheExpiry`, `CachePercent`, `CacheTime`, `CacheScope`); host side
`flm_host::cache::ExtensionCache`, `flm_host::host_cache`, `flm_host::host_programs`.

- **Declaration.** Package object key `caches` (Rust guest: `Package::caches([(name,
  CacheDeclaration)])`): a map from a name (segments of `[a-z0-9_-]` joined by `.`) to
  `{ version (≥ 1), values: { engine: jsonl|files|sqlite (default sqlite), blobs (default
  false) } | folder: {}, unused_s (stable; seconds or "unlimited") | lifetime_s (volatile,
  values only) with optional expiry { stale_percent (80), jitter_percent (15), grace_s
  (14 days, or "unlimited") }, cost_ms (≥ 1), scope: shared|document }`. Exactly one of
  `values`/`folder` and of `unused_s`/`lifetime_s`; `null` counts as absent; unknown keys
  and wrong shapes fail the load (message names extension, package, cache). Read with
  the package report (`PackageReport::caches`), given to every service call
  (`HostCaller::caches`), not in the digest. `HostService::check_package` (default:
  accept) runs at load for each declared service; `flm-host`'s `cache` and `programs`
  refuse a declaration cachette refuses (whole name too long, `jsonl` with blobs).
- **Naming and instances (flm-host).** Cache name `flm.ext.STEM.NAME` (stem lowercased,
  `@` dropped, `/` → `.`); instance key `{"extension": IDENTITY}` plus `"document"` for
  scope `document` (without a document text: no cache). A `sqlite` cache falls back to
  `jsonl`/`files` on a manager without SQLite.
- **Service `cache`** (`flm-host` always offers it): `get { cache, key, blobs? }` →
  `null` | `{ record, blobs, freshness, refresh }`; `put { cache, key, record, blobs?,
  cost_ms?, lifetime_s? }` → `null`; `remove { cache, key }` → `null`. `cache` must name a
  declared values cache. Key: any JSON, stored under SHA-256 of its JSON with object
  members sorted by name. Blobs: map name → `{ text }` | `{ base64 }` (standard alphabet,
  padding); `get` answers `{ text }` for UTF-8 bytes, else `{ base64 }` — read either as
  bytes. `freshness`: `fresh`/`stale`/`expired` for volatile values, `null` for stable;
  `refresh`: whether to produce and store again. Never refused for age: a handler that
  cannot produce a value again uses the kept one and warns when it is `expired`. Only a
  wrong argument shape is refused; cache failures make `get` answer `null` and `put`
  store nothing. One session per build: a value stored earlier in a build is read later
  in it.
- **`programs.run`'s argument `cache`**: map relative path → `{ name, cost_ms? }`, `name`
  a declared folder cache; the service links the cache folder there (symlink on Unix,
  junction on Windows), `files`/`collect` may lie under it, the links and the temporary
  folder are removed after the run without touching the cache folder; an unavailable
  folder (no folder caches, another build holds it, link refused or leading to no
  folder) is an empty folder.
  Two paths of `cache` may not coincide or nest. No absolute cache path reaches the
  extension.
- **Rule.** A cache saves work and never changes the result: the key holds everything the
  value depends on; an empty cache must give the same output.
- **Examples.** `math-mathjax` (`typeset.mathml`, values, `sqlite`, 90 days, shared;
  key `[MathJax.VERSION, packages, preamble, export_commands, display, TeX]`);
  `profile-pdf-latex` (`latexmk.aux`, folder, 30 days, per document, at `_aux` with
  `latexmk -auxdir=_aux`).

## Build hooks

- [`Finalizer`](crate::core::registry::Finalizer): the build's **second registration
  round**. It runs after every feature's `register`, once the service slots are
  assembled, so it may read the services
  ([`b.services()`](crate::core::registry::FeatureBuilder::services) is `Some` there) and
  declare further entities (an element, a stage, a rule) derived from what other features
  contributed. It may not declare a service slot, contribute, require one, or register a
  finalizer: [`BuildError::ContributionAfterAssembly`](crate::core::registry::BuildError::ContributionAfterAssembly)
  with the attempted [`RoundTwoAction`](crate::core::registry::RoundTwoAction). There
  are exactly two rounds; a feature may register several finalizers, run in order.
- [`LateInit`](crate::core::registry::LateInit): runs once the **language is built**;
  registered in either round. Its usual use is compiling author-written templates, which
  may use commands of other features and so cannot be parsed earlier. A problem it finds
  in the configuration it prepares goes to
  [`build_diagnostics`](crate::core::registry::FlmRegistry::build_diagnostics) and does
  not fail the build; returning
  [`LateInitError`](crate::core::registry::LateInitError) means the hook could not do its
  own work, and fails the build. It is also the place for a fact about the whole
  configuration (whether another definition has the same name), worked out once and
  recorded on the element values it initializes. A value built against the language (a
  compiled template, a parsed tree) that an element's data or aspect reads goes in a
  [`LangOwned`](crate::lang::LangOwned) cell (`cell.set(lang, value)`, `cell.get()`): the
  language owns it. An element that owned it (`OnceLock<Arc<…>>` in its data) would form
  a reference cycle that keeps the language allocated after the registry is dropped. A
  rule or a stage may own such a value directly.

## Packages and registration order

A package is a named bundle of features together with the packages it builds on
([`PackageEntry`](crate::core::registry::PackageEntry)): its name, the packages it
`uses`, and its own features in the order they register.
[`RegistryBuilder::use_package`](crate::core::registry::RegistryBuilder::use_package)
adds the features of the packages it uses first, then its own, and skips a package whose
name it has already added, so two packages can build on a third.

```rust
use std::sync::Arc;
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry, RegistryBuilder,
    RegistryInputs,
};
use flm_core::stdlib::{self, StdlibConfig};

# struct Shout;
# impl FlmFeature for Shout {
#     fn info(&self) -> FeatureInfo { FeatureInfo::new("shout", "Shouting") }
#     fn register(&self, _b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> { Ok(()) }
# }
let mine = PackageEntry::new("mine")
    .uses(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()))
    .feature(Arc::new(Shout));

let mut builder = RegistryBuilder::new();
builder.use_package(mine);
let registry = builder.build(&RegistryInputs::new())?;

assert!(registry.has_feature("shout"));
assert!(registry.has_feature("headings")); // came in with `std`
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

**Registration order** matters in a few places only, because the checks run once every
feature has registered: the registry numbers capabilities, tables, stages and rules in
declaration order, and that order breaks ties when a plan has nothing else to decide
between two stages (two builds from the same features in the same order give the same
numbering and the same plans); contributions fill a service slot in the order they were
made, the embedding program's after every feature's, and a read for precedence
(`service_named`, `services_by_precedence_named`) takes the feature placed last first; the fields several features register for one workflow apply in registration
order; and where several features define one command, the definition of the feature
placed last applies. Ordering features by dependency does nothing.

**The feature order.** The registration order is the order the packages give, changed by
*chains*: [`FeatureChain`](crate::core::registry::FeatureChain), read from a text
`NAME < NAME < … < NAME` of features' names by
[`FeatureChain::parse`](crate::core::registry::FeatureChain::parse). A package carries its
chains ([`PackageEntry::feature_order`](crate::core::registry::PackageEntry::feature_order)),
a host adds its own
([`RegistryBuilder::feature_order`](crate::core::registry::RegistryBuilder::feature_order));
an extension writes them under `feature_order` in its package's `flm` block (a data
extension's manifest, `flm: { feature_order: ["math-mathjax < phfqit"] }` in a JavaScript
package object), and a document under `flm.feature_order` in its front matter. Every
chain applies; a feature that is not in use drops out of a chain; chains that place
features in a circle fail the build
([`BuildError::FeatureOrderCycle`](crate::core::registry::BuildError::FeatureOrderCycle)).
Declare a chain only when the package needs the order (the reverse order is then an
error); one chain settles every collision between two features, and a finer order needs
the feature split into several.

**Packages chosen at run time.** A
[`PackageCatalog`](crate::core::registry::PackageCatalog) finds a package's
[`PackageFactory`](crate::core::registry::PackageFactory) by name: first among the
factories compiled into the program, then through the
[`PackageResolver`](crate::core::registry::PackageResolver) implementations added
([`resolve_all`](crate::core::registry::PackageCatalog::resolve_all) returns them in
registration order, each after the packages it uses, each once). flm-core ships no
resolver and reads no file; the crate `flm-ext` provides the loader of *extensions*
(`flm_ext::ExtensionLoader`): sets of files found by name, written as data (settings,
workflows, compose templates) or in JavaScript (features with behavior), each providing
one or more packages; see [Extensions](crate::guide::extending#extensions). A package of
an extension lists, among the packages it uses, the packages of other extensions it builds
on; it need not list the standard features, which a host loads into every document by
default (the `flm` program, through its package `std`). A factory holds the package's
name, the names of the packages it uses, and the function that builds the entry from the
final settings ([`PackageSettings`](crate::core::settings::PackageSettings)), leaving out
a feature the settings switch off; it learns each feature's settings declaration by
building the package once from empty settings
([`PackageFactory::new`](crate::core::registry::PackageFactory::new)).
A package that holds the standard features builds them in its build function with
[`stdlib::add_features_from_settings`](crate::stdlib::add_features_from_settings).

## Contribute a piece

A piece is a document-level synthesized value that a backend places before or after the
document's content (the title, the list of footnotes, the bibliography), built once by
its builder stage into a table named for the piece and rendered by each backend with its
own rules ([`Piece`](crate::core::piece::Piece); [pieces](crate::guide::concepts_overview#pieces)).
A feature offers one with three declarations:

1. a table of type [`Piece`](crate::core::piece::Piece) named for the piece
   (`table_named::<Piece>(decl.name, docs)`);
2. a `Document`-scope builder stage writing that table: a small tree of document
   elements, made from the tables the stage declares it reads, or
   `Piece { content: None }` for nothing to place;
3. a [`PieceDecl`](crate::core::piece::PieceDecl)`::new(name, place, docs)` (its
   `table`, its [`PiecePlace`](crate::core::piece::PiecePlace), and
   [`default_on`](crate::core::piece::PieceDecl::default_on)) contributed with
   `peer_slot` to [`flm.pieces`](crate::stdlib::backends::pieces::SLOT), so that the
   feature registers without the `pieces` feature.

Nothing else: each backend that places pieces has a
[`PiecesStage`](crate::stdlib::backends::pieces::PiecesStage) (`flm.<backend>.pieces`)
that lowers the selected pieces with the rules the plan selected, so a piece is written
on every backend by the rules of the elements it holds. The render option
[`PiecesRenderOptions`](crate::stdlib::backends::pieces::PiecesRenderOptions) selects the
pieces: every piece declared on by default that `exclude` does not list, plus every piece
`include` lists; the builder stage of a piece left out is not scheduled. Its field
`backends` limits, per backend, the selected pieces that backend places: a backend with an
entry places only the selected pieces the entry lists. A backend without an entry places
every selected piece, except a backend whose pieces stage places only the pieces listed
for it
([`PiecesStage::placing_only_named`](crate::stdlib::backends::pieces::PiecesStage::placing_only_named)):
the recomposed LaTeX is such a backend, and without an entry it places no piece.

**Point at the document's content; never copy it.** A synthesized value states no fact
about the document: a labeled reference point inside it is refused when the value is
brought up to date (`flm.pieces.lowering-failed`, the piece left out), and a point it
states itself has no number and no anchor (`flm.run.missing-entry`). Build the value's
own structure (a list, its entries) with
[`copy_pieces_into_value`](crate::lang::copy::copy_pieces_into_value)
([`SynthesizedPiece`](crate::lang::copy::SynthesizedPiece)), and in place of each
content a synthesized node of an element of your own whose payload is a content pointer:
the payload of a synthesized node that shows, where the node is placed, content written
elsewhere in the tree without copying it, the address of the node the content belongs to
and the region of that node that holds it
([`ContentPointer`](crate::stdlib::backends::lowering::ContentPointer)). That node's
rule calls
[`lower_content_pointer`](crate::stdlib::backends::lowering::lower_content_pointer)
with a check of your feature's
([`PointerCheck`](crate::stdlib::backends::lowering::PointerCheck): whether the node is
one whose region your element may show; report your own condition where it is not) and
[`lower_region_content`](crate::stdlib::backends::lowering::lower_region_content) as the
shape. The content is lowered as the document's own, through
[`Lowerer::lower_region`](crate::core::lowering::Lowerer::lower_region): its points keep
their numbers and anchors. **Lower a region once**: the element whose content is shown
elsewhere writes nothing of it where it is written (the xnote writes its mark alone), and
exactly one node points at it; the driver checks neither. A pointer whose node or region
cannot be found is `flm.lowering.region-not-found`, at the pointer. Copying nodes
([`copy_run_into_value`](crate::lang::copy::copy_run_into_value)) remains the way to
make a value of content not shown as the document's own (a tag's text).

**To replace another feature's builder**, register a `Document`-scope stage of your own
that writes the same table (`writes(decl.table)`) and prefer it
([`WorkflowDef::prefer`](crate::core::registry::WorkflowDef::prefer) in a workflow of
your own, the field `prefer` of [`Overrides`](crate::core::plan::Overrides) for a
workflow, or [`PlanRequest::prefer`](crate::core::plan::PlanRequest::prefer)); without a preference, two writers of one table are
[`PlanError::Contested`](crate::core::plan::PlanError::Contested), which lists both. The
public guide shows it for the xnotes lists: [building a kind's list your own
way](crate::guide::extending#building-a-kinds-list-your-own-way).

```rust
use std::sync::Arc;

use flm_core::core::data::store::StageOut;
use flm_core::core::piece::{Piece, PieceDecl, PiecePlace};
use flm_core::core::plan::PlanCx;
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry, RegistryBuilder, RegistryInputs,
};
use flm_core::core::run::StageCx;
use flm_core::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::model::doc::FragmentLayout;
use flm_core::stdlib::backends::pieces;

/// The piece: a closing line saying how many parts the document has.
fn closing() -> PieceDecl {
    PieceDecl::new("closing.line", PiecePlace::AfterContent, "a closing line")
}

/// The builder: a `Document`-scope stage writing the piece's table.
struct BuildClosing;

impl Stage for BuildClosing {
    fn decl(&self) -> StageDecl {
        StageDecl::new("closing.build", Effect::Compute, Scope::Document)
            .writes(closing().table)
            .docs("writes the closing line")
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let text = format!("\\emph{{{} parts.}}", cx.trees().count());
        let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
        let parsed = parse_fragment(
            cx.registry().lang(),
            source,
            FragmentLayout::default(),
            &ParseOptions::default(),
        );
        let content = parsed.ok().map(|fragment| Arc::clone(fragment.tree()));
        if let Ok(table) = out.table(&closing().table) {
            *table = Piece { content };
        }
    }
}

struct Closing;

impl FlmFeature for Closing {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("closing", "A closing line")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.table_named::<Piece>(closing().name, "the closing line")?;
        b.stage(Arc::new(BuildClosing))?;
        b.peer_slot(pieces::SLOT, Arc::new(closing()))?;
        Ok(())
    }
}

let mut builder = RegistryBuilder::new();
builder.use_package(flm_core::stdlib::add_features(PackageEntry::new("std"), flm_core::stdlib::StdlibConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
builder.feature(Arc::new(Closing));
let registry = builder.build(&RegistryInputs::new())?;

# use flm_core::model::doc::{Doc, DocContent};
# use flm_core::core::plan::{Overrides, Plan};
# use flm_core::core::run::{Externals, Run, RunOptions};
# use flm_core::stdlib::backends::text::{self, TEXT_OUTPUT};
let mut content = DocContent::new();
for text in ["One.", "Two."] {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(
        registry.lang(), source, FragmentLayout::default(), &ParseOptions::default(),
    )?;
    content.place(Arc::new(fragment));
}
let plan = Arc::new(Plan::for_workflow(&registry, text::WORKFLOW, &Overrides::none())?);
let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none())?;
run.execute(&RunOptions::new())?;

// The piece is lowered by the `\emph` rule like content, and printed last.
assert_eq!(run.table(TEXT_OUTPUT)?.expect("written").text, "One.\n\nTwo.\n\n2 parts.\n");
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Number a construct of your own

The [`numbering`](crate::stdlib::features::numbering) feature numbers every reference
point that states a number source; it declares no command and no rule, so the feature
that owns the construct declares the counter, states the number source and shows the
number. A counter is a dotted name under which things are counted in document order,
declared by a feature together with the counter it is numbered within
([`CounterDecl`](crate::stdlib::features::numbering::CounterDecl)); a counter style says
how a counter value is shown
([`CounterStyleDecl`](crate::stdlib::features::numbering::CounterStyleDecl); [counters
and counter styles](crate::guide::concepts_overview#counters-and-counter-styles)). The
compiled example (a `\step{…}` command numbered `(11)`, `(12)` from a document option) is
the public recipe [Number a construct of your
own](crate::guide::extending#number-a-construct-of-your-own); the four steps:

1. **Declare a counter style and a counter**, under your own prefix, with `peer_slot`:
   `b.peer_slot::<CounterStyleDecl>(STYLES_SLOT, …)` and
   `b.peer_slot::<CounterDecl>(COUNTERS_SLOT, Arc::new(CounterDecl::new(counter, style)))`
   ([`STYLES_SLOT`](crate::stdlib::features::numbering::STYLES_SLOT),
   [`COUNTERS_SLOT`](crate::stdlib::features::numbering::COUNTERS_SLOT)). A counter
   numbered within another takes
   [`CounterDecl::within`](crate::stdlib::features::numbering::CounterDecl::within)
   (below). Without the `numbering` feature both are dropped with a Note per slot.
2. **State a number source** on the point your `Referenceable` returns:
   `number: Some(NumberSource { counter, instances: Vec::new(), style, advance: vec![0],
   number_format: None })` (`advance` lists the levels of the counter the point
   advances; `[0]` for a plain number). Leave it `None` for an occurrence not to be
   numbered (a starred form); the `numbering` feature has no setting of its own that turns
   numbering off.
3. **Show the number in the rule**: declare the counter table a peer table
   (`b.peer_table::<Counters>(COUNTERS.name(), …)`) read optionally
   (`LoweringDecl::reads_opt(COUNTERS)`); in the rule,
   [`Counters::format`](crate::stdlib::features::numbering::Counters::format)`(PointId {
   addr: lw.addr(), index }, &FormatOptions::new())` returns the point's number as
   [`NumberSegment`](crate::stdlib::features::numbering::NumberSegment)s
   (`Some(Ok(segments))`), `None` for a point the document did not number, and
   `Some(Err(`[`FormatError`](crate::stdlib::features::numbering::FormatError)`))` only
   for a fault in the program; stage each segment's content with
   [`Lowerer::splice`](crate::core::lowering::Lowerer::splice) (take the segments out of
   the table first: splicing needs the `Lowerer` mutably). Where the `numbering` feature
   is registered the optional read becomes a required one and the numbering stage is
   scheduled; where it is not, the rule finds no table and shows the construct without a
   number.
4. **Hold the `refpoints` feature** in the registry: the numbering stage reads the points
   from [`flm.refpoints`](crate::stdlib::features::refpoints::REFPOINTS). The standard
   package holds it; a registry assembled from single features must list it, or every
   plan that schedules the numbering stage fails.

A `\ref` to the point then shows the number with nothing more from your feature. A
document changes the numbering through its numbering options
([`NumberingOptions`](crate::stdlib::features::numbering::NumberingOptions), bound on
the document and on a render request): which counter a counter is numbered within, and
the value it has before its first advance.

**Forms the declarations offer**, each described in the [numbering
guide](crate::guide::numbering):

- [`Within`](crate::stdlib::features::numbering::Within) ([numbering within another
  counter](crate::guide::numbering#numbering-within-another-counter)):
  `Within::parent("flm.headings.section")` (`1.2`), `.joined_by(text)` (another text
  than the full stop), `.template(text)` (a within template, FLM in which
  `#{flm.headings.section}` or `#{_parent}`, the parent that applies, writes that
  counter's number; keys `#[bare]{…}` and `#[Roman]{…}`), `.displayed_bare()` (nothing of
  the parent written, the count still restarted), `.no_restart()` (written, not
  restarting), `Within::parents([...])` and `Within::parent_set(name)` (within whichever
  of several counters was advanced last; a point reached before any of them has a value
  takes no number and is `flm.numbering.parent-uninitialized`), and
  `.joiner_named(name)`.
- A **within-joiner** of your own, for a form none of these writes: implement
  [`WithinJoiner`](crate::stdlib::features::numbering::WithinJoiner), contribute a
  [`WithinJoinerDecl`](crate::stdlib::features::numbering::WithinJoinerDecl) to
  `flm.numbering.within-joiners`, and select it with `Within::joiner_named(name)`; an
  unknown name in a document's options is `flm.numbering.unknown-within-joiner`.
- A **number writer** of your own, for what a counter style's data cannot describe (a
  number that states the section it is in, one range across two parents): implement
  [`NumberWriter`](crate::stdlib::features::numbering::NumberWriter), contribute a
  [`NumberWriterDecl`](crate::stdlib::features::numbering::NumberWriterDecl) to
  `flm.numbering.number-writers`, and select it on the style with
  [`CounterStyleDecl::with_number_writer`](crate::stdlib::features::numbering::CounterStyleDecl::with_number_writer)
  ([the number writer](crate::guide::numbering#the-number-writer)).
- A **parent set** ([`ParentSetDecl`](crate::stdlib::features::numbering::ParentSetDecl),
  `flm.numbering.parent-sets`): a named list of counters a counter may be numbered
  within, as the headings feature contributes `flm.headings.levels`.
- **Scoped instances** ([scoped instances](crate::guide::numbering#scoped-instances)):
  a construct that numbers its parts afresh in every occurrence of an enclosing construct
  (the items of each list from one) lists, in
  [`NumberSource::instances`](crate::model::aspects::NumberSource::instances), the node
  ids of the enclosing nodes that each hold a run of its counter, outermost first (found
  by walking up from the node), and the numbering stage keeps one count per instance;
  [`CounterDecl::nesting`](crate::stdlib::features::numbering::CounterDecl::nesting)
  says what a nested number writes of the enclosing instance's value. Instances that do
  not enclose one another are `flm.numbering.instance-not-enclosing`.
  `flm-core/tests/numbering_feature.rs` holds a worked list of this kind
  (`ListFeature`); the [`lists`](crate::stdlib::features::lists) feature is the one in
  the library.
- The other ways a rule writes a number: [showing a number from a lowering
  rule](crate::guide::numbering#showing-a-number-from-a-lowering-rule).

## Heading kinds and commands of your own

Heading kinds and heading commands are configuration of the `headings` feature, so
declaring one needs no feature of your own. A heading kind
([`HeadingDecl`](crate::stdlib::features::headings::HeadingDecl)) is a declared heading
(section, subsection, …): `HeadingDecl::new(name, level)` with the defaults of the
standard kinds, then one builder per field: `counter` (`flm.headings.<name>` by default;
`no_counter()` for none), `style` (`flm.headings.style.<name>`), `within`,
`number_format` (`#{arabic}`), `title_number_format` (the heading title template, written
before a numbered heading's title: `#{number}` the number, `#[bare]{number}` the kind's
own number alone, `#[Roman]{number}` it in another number format; `#{number}. ` by
default), `skip_toc`, `run_in`, and the reference point's `ref_kind`, `label_types`,
`anchor_prefix`. The level is any number counted from zero; the shipped kinds have levels
1 to 5, and level 0 is free for a kind above the sections. A heading command
([`HeadingCommand`](crate::stdlib::features::headings::HeadingCommand)) is a macro that
writes a heading of one kind, with default key-values (`numbered`, `toc`) read before the
occurrence's star and bracket. The occurrence's bracket also takes `alt={…}`, the
alternative title its entry in the table of contents shows; `alt` is not a command
default.
[`HeadingsConfig::headings`](crate::stdlib::features::headings::HeadingsConfig::headings)
replaces the kinds and gives each a command of its own name; `.command(…)` adds one;
`.commands(…)` replaces them. Which kinds are numbered is the numbering depth
([`HeadingNumbering`](crate::stdlib::features::headings::HeadingNumbering): nothing, by
default). [`HeadingsConfig::validate`](crate::stdlib::features::headings::HeadingsConfig::validate)
refuses, and the registry build with it: two kinds or two commands of one name, a kind no
command writes, a command for an undeclared kind, two kinds sharing a counter or a
counter style, a kind with a counter and no counter style, a command default whose key is not
`numbered` or `toc` or whose value is not `true` or `false`; a title template that does not parse, uses
another placeholder or gives `number` a key it does not take also fails the build.

```rust
use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::stdlib::features::headings::{HEADING, HeadingCommand, HeadingDecl, HeadingsConfig};
use flm_core::stdlib::features::numbering::Within;
use flm_core::stdlib::{self, StdlibConfig};

let mut kinds = HeadingsConfig::new().headings;
// A chapter above the sections, at level 0, and the sections' counter
// numbered within the chapter's (`2.1` where the configuration numbers
// headings, which it does not by default). The shipped kinds keep their
// levels, 1 to 5.
kinds.insert(0, HeadingDecl::new("chapter", 0));
kinds[1] = kinds[1].clone().within(Within::parent("flm.headings.chapter"));
let headings = HeadingsConfig::new()
    .headings(kinds)
    .command(HeadingCommand::new("quietsection", "section").default_value("toc", "false"));

let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new().headings(headings)));
let registry = builder.build(&RegistryInputs::new())?;
assert_eq!(registry.elements_named(HEADING).len(), 6);
# Ok::<(), flm_core::core::registry::RegistryBuildError>(())
```

The LaTeX command a kind is recomposed as is a render option,
[`HeadingsLatexOptions`](crate::stdlib::features::headings::latex::HeadingsLatexOptions)
(the kind's name by default; a kind whose name is not a sectioning command, such as
`appendix`, must be mapped to one). On HTML, the tag is
[`HtmlRenderOptions::heading_tags`](crate::stdlib::backends::html::HtmlRenderOptions::heading_tags)
(render option; `<h1>` to `<h6>` by level otherwise, counted from
[`HtmlRenderOptions::root_heading_level`](crate::stdlib::backends::html::HtmlRenderOptions::root_heading_level),
1 by default). The chapter of the example needs a root of 0 to be `<h1>`; under the
default it is the Error `flm.pres.heading-above-root` and is written `<h1>`. Every block
heading carries `data-flm-heading` (the kind), `data-flm-level` and `data-flm-toc`
regardless of its tag; a run-in heading carries them on the `<span>` inside its run-in
decoration. Each entry of the table of contents carries its heading's level as
`data-flm-level` on its `<li>`, so `[data-flm-level="N"]` selects the headings of level N
and their entries alike.

Every kind is listed in the table of contents of the [`toc`](crate::stdlib::features::toc)
feature (the table `flm.toc.entries`, the piece `flm.toc`) at its level, unless the kind is
`skip_toc`, the occurrence says `toc=false`, or the level is deeper than
[`TocOptions::depth`](crate::stdlib::features::toc::TocOptions::depth) (3). The feature
reads every element carrying [`Sectioning`](crate::model::aspects::Sectioning), so a
heading element of an extension's own is listed when it implements that aspect: `level`,
`in_toc` (default `true`) and
[`toc_title`](crate::model::aspects::Sectioning::toc_title), the content its entry shows
(default `None`: the entry shows the number alone). [Heading kinds and commands of your
own](crate::guide::extending#heading-kinds-and-commands-of-your-own) is the public
recipe.

## Make an element block-transparent

A block-transparent element is one whose content is laid out as if it were written
directly in the content around it ([block-transparent
elements](crate::guide::concepts_overview#block-transparent-elements)): a blank line in
its argument is a paragraph break of the text around it, and the mark continues into the
next paragraph; a block in it (a list) is a block of the text around it, and a displayed
item in it (a displayed formula) stays in the inline content it spans; the element never
becomes a block of its own. Block structuring
([`structure`](crate::lang::rewrites::blocks::structure)) does this in its first walk, the
*cut step*: it calls the element's
[`BlockTransparent`](crate::lang::rewrites::blocks::BlockTransparent) aspect with the
whole parsed node, and puts the staged nodes it returns in the node's place among its
siblings, without a wrapper. The layout step then lays out each **cut part** (a node that
holds one stretch of the content: inline content between two paragraph breaks, or one
block) by its block role, like any node. Where the content around is inline (a heading's
title), the layout reports a break (`flm.blocks.parbreak-in-inline`, written as a space)
or a block cut part (`flm.blocks.block-in-inline`) as for any inline content.

The element does the work; the cut step provides, through the
[`BlockCutter`](crate::lang::rewrites::blocks::BlockCutter) it hands the aspect:

| operation | what it does |
|---|---|
| [`restage_and_classify`](crate::lang::rewrites::blocks::BlockCutter::restage_and_classify)`(cx, run)` → `Vec<`[`CutItem`](crate::lang::rewrites::blocks::CutItem)`>` | stages a run (an argument's content) in order: a paragraph-break marker as `Marker`; a node whose block role (read on the *input* node) is `Block` as `Block` (a `Displayed` node is `Inline`); a comment, text that is only whitespace, or a node whose element carries [`Whitespace`](crate::model::aspects::Whitespace) (`~`) as `Blank`; anything else `Inline`. A block-transparent node inside is cut on the way and its items returned with their variants (the cut is recursive). A run in mathematics mode is copied unchanged, as `Inline` and `Blank` items only |
| [`rebuild`](crate::lang::rewrites::blocks::BlockCutter::rebuild)`(cx, &node, vec![(`[`CutPart`](crate::lang::rewrites::blocks::CutPart)`, content)…], Some(payload))` → `BuildId` | a copy of the node (same span, parsing state, element, way of writing) in which each listed part (`Argument(i)`, `ArgumentNamed(name)`, `Body`) holds the given staged nodes inside its own delimiters; the other parts are staged through the cut step; the payload is set as [`FlmNodeExt::payload`](crate::lang::FlmNodeExt::payload) (`None` keeps the node's own). A part listed twice, a missing part or an unwritten argument is [`StructureError::Cut`](crate::lang::rewrites::blocks::StructureError) |
| [`carry`](crate::lang::rewrites::blocks::BlockCutter::carry)`(cx, &node)` | an unchanged copy of a node and its subtree, nothing cut |
| [`synthesize_callable`](crate::lang::rewrites::blocks::BlockCutter::synthesize_callable)`(&node, &element)` | a childless node of a synthesized element at the node, to stage with `carry` |
| [`diagnose`](crate::lang::rewrites::blocks::BlockCutter::diagnose)`(diagnostic)` | reports a problem in the document without ending the pass |
| [`new_cut_number`](crate::lang::rewrites::blocks::BlockCutter::new_cut_number)`()` → `usize` | a number unique in the tree, in walk order (the same every time the tree is structured) |

The recipe:

1. **Declare** each argument the element cuts with `.layout(ArgLayout::InlineOrBlocks)`,
   and give the element a [`BlockRoleAspect`](crate::model::aspects::BlockRoleAspect)
   that follows the content: `Block` when
   [`argument_holds_blocks`](crate::lang::rewrites::blocks::argument_holds_blocks) says
   so, inline otherwise. After the cut, a cut part around inline content is inline and
   one around a block is a block; before it, an inner node holding a break returns
   `Block`, which is how an outer element sees a cut point.
2. **Leave the node** ([`Cutting::Default`](crate::lang::rewrites::blocks::Cutting),
   staging nothing first) when it carries the element's own continuation record, or when
   no run it cuts holds a cut point
   ([`run_holds_blocks`](crate::lang::rewrites::blocks::run_holds_blocks)). The record
   test is needed because the aspect is called at every node of the element, cut parts
   included, when a tree is structured again (or a cut part copied into a synthesized
   value is), and a block cut part holds a block, so `run_holds_blocks` returns `true`
   for it; only the element knows its record's type.
3. Otherwise **take a cut number** and **classify** each run it cuts with
   `restage_and_classify`. Never pass the node itself (the aspect would be called on it
   again); pass its parts' content.
4. **Group**: a maximal stretch of `Inline` and `Blank` items holding at least one
   `Inline` is one cut part; each `Block` is a cut part of its own; `Marker`s pass
   through; a stretch of `Blank` items alone makes no cut part and passes through. No cut
   part holds a marker, or a block beside other content.
5. **Build** each cut part with `rebuild`, the items as the part's content and the
   continuation record (a type of the element's own implementing
   [`PresPayload`](crate::lang::PresPayload), such as `{cut, index, count}`) as payload.
   The record is plain data; the feature supplies a reader for it under the element's
   name ([`PayloadReaders`](crate::dump::session::PayloadReaders), passed to
   [`reader_with`](crate::dump::session::reader_with)) so that a frozen tree holding
   cut parts reads back from its serialized form. Return each cut part as
   `CutItem::Inline` or `CutItem::Block` by what it holds, in
   [`Cutting::Emit`](crate::lang::rewrites::blocks::Cutting).
6. **Read the record back** in the element's rules with
   [`payload_of::<Record>(&node)`](crate::stdlib::backends::pres::read::payload_of):
   `None` on a node that was never cut. A copy into a synthesized value
   ([`crate::lang::copy`]) keeps it.

A failure of the aspect itself is
[`StructureError::cut`](crate::lang::rewrites::blocks::StructureError::cut) and ends block
structuring with no tree; a problem in the document goes through `diagnose`. The
[`annotations`](crate::stdlib::features::annotations) feature's element is a complete
user: one argument cut, cut parts around inline stretches and blocks, the record
[`AnnotationPiece`](crate::stdlib::features::annotations::AnnotationPiece) read by its
rules to show the initials on the first cut part only.

```rust
use std::sync::Arc;
use flm_core::lang::mint::{ArgDef, ArgLayout, ElementDef, SyntaxDef};
use flm_core::lang::rewrites::blocks::{
    argument_holds_blocks, run_holds_blocks, structure, BlockCutter, BlockTransparent,
    BlocksConfig, CutItem, CutPart, Cutting, StructureCx, StructureError,
};
use flm_core::lang::{
    build_language, parse_fragment, Flm, FlmDiagnostics, FlmSource, LangInputs, ParseOptions,
    PresPayload,
};
use flm_core::model::aspects::{BlockRole, BlockRoleAspect};
use flm_core::model::doc::FragmentLayout;
use flm_core::model::element::ElementSpec;
use flm_core::stdlib::backends::pres::read::payload_of;
use flm_core::techy::core::node::{BuildId, NodeRef};
use flm_core::techy::serialize::SerialValue;

/// The continuation record: which cut, which cut part, how many cut parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CutRecord { cut: usize, index: usize, count: usize }

impl PresPayload for CutRecord {
    fn project(&self) -> SerialValue {
        SerialValue::Str(format!("{}/{}/{}", self.cut, self.index, self.count))
    }
}

struct Mark;

impl BlockRoleAspect for Mark {
    fn block_role(&self, node: &NodeRef<'_, Flm>) -> BlockRole {
        if argument_holds_blocks(node, 0) { BlockRole::Block } else { BlockRole::DEFAULT }
    }
}

enum Group { Part(Vec<BuildId>, bool), Through(CutItem) }

impl BlockTransparent for Mark {
    fn cut(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut StructureCx<'_>,
        cutter: &mut dyn BlockCutter,
    ) -> Result<Cutting, StructureError> {
        let run: Vec<NodeRef<'_, Flm>> =
            node.argument_content_nodes(0).map(|c| c.iter().collect()).unwrap_or_default();
        if payload_of::<CutRecord>(&node).is_some() || !run_holds_blocks(&run) {
            return Ok(Cutting::Default);
        }
        let cut = cutter.new_cut_number();
        let flush = |stretch: &mut Vec<CutItem>, groups: &mut Vec<Group>| {
            if stretch.iter().any(|item| matches!(item, CutItem::Inline(_))) {
                groups.push(Group::Part(stretch.drain(..).map(CutItem::id).collect(), false));
            } else {
                groups.extend(stretch.drain(..).map(Group::Through));
            }
        };
        let (mut groups, mut stretch) = (Vec::new(), Vec::new());
        for item in cutter.restage_and_classify(cx, &run)? {
            match item {
                CutItem::Inline(_) | CutItem::Blank(_) => stretch.push(item),
                CutItem::Marker(_) => {
                    flush(&mut stretch, &mut groups);
                    groups.push(Group::Through(item));
                }
                CutItem::Block(id) => {
                    flush(&mut stretch, &mut groups);
                    groups.push(Group::Part(vec![id], true));
                }
            }
        }
        flush(&mut stretch, &mut groups);
        let count = groups.iter().filter(|group| matches!(group, Group::Part(..))).count();
        let (mut items, mut index) = (Vec::new(), 0);
        for group in groups {
            match group {
                Group::Through(item) => items.push(item),
                Group::Part(content, block) => {
                    let record = Arc::new(CutRecord { cut, index, count });
                    let replacement = vec![(CutPart::Argument(0), content)];
                    let id = cutter.rebuild(cx, &node, replacement, Some(record))?;
                    items.push(if block { CutItem::Block(id) } else { CutItem::Inline(id) });
                    index += 1;
                }
            }
        }
        Ok(Cutting::Emit(items))
    }
}

let mark = Arc::new(Mark);
let mut inputs = LangInputs::new();
inputs.elements.push(ElementDef::new(
    "t",
    ["mark"],
    SyntaxDef::Macro { args: vec![ArgDef::mandatory("text").layout(ArgLayout::InlineOrBlocks)] },
    Arc::new(
        ElementSpec::new("t.mark")
            .with::<dyn BlockRoleAspect>(mark.clone())
            .with::<dyn BlockTransparent>(mark),
    ),
));
let lang = build_language(inputs);
let source = Arc::new(FlmSource::new("Some text \\mark{a remark\n\ncontinued} and more."));
let fragment = parse_fragment(&lang, source, FragmentLayout::Blocks, &ParseOptions::default())
    .expect("a parse");
let mut diags = FlmDiagnostics::new();
let tree = structure(fragment.tree(), FragmentLayout::Blocks, &BlocksConfig::new(), &mut diags)?;
// Two paragraphs, each holding one cut part of the mark.
assert_eq!(tree.root().child_count(), 2);
let records: Vec<CutRecord> =
    tree.root().descendants().filter_map(|node| payload_of::<CutRecord>(&node).copied()).collect();
assert_eq!(
    records,
    [CutRecord { cut: 0, index: 0, count: 2 }, CutRecord { cut: 0, index: 1, count: 2 }],
);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## The standard features as worked examples

Every extension point is used by a standard feature; read the one that uses the point you
need.

| feature | read it for |
|---|---|
| [`textstyles`](crate::stdlib::features::textstyles) | the smallest complete feature: one element type with four values, a general rule and a rule on the recomposed LaTeX under a peer declaration of the backend, style names of its own mapped onto the presentation vocabulary |
| [`literals`](crate::stdlib::features::literals) | a family of commands declared from one list, a macro and a special character sequence in one element type, a rule that emits a text node and a rule that emits nothing |
| [`blocks`](crate::stdlib::features::blocks) | a rewrite stage, a capability with two refinements, an ordering edge declared only when the registry holds the capability it refers to, a configuration |
| [`index`](crate::stdlib::features::index) | one table and one `Compute`/`PerTree` stage, nothing else |
| [`refpoints`](crate::stdlib::features::refpoints) | one table and one `Compute`/`PerTree` stage that asks an aspect once per node, whose table two other features require rather than declare as a peer table |
| [`numbering`](crate::stdlib::features::numbering) | five service slots other features contribute to as peer contributions (counters, counter styles, within-joiners, number writers, parent sets), a finalizer that checks the declarations against one another, a late-initialization hook that compiles every declared text, an option type bound on the document and on a render request ([`Bind::Either`](crate::model::options::Bind::Either)), one `Compute`/`Document` stage, and a table that carries the compiled data so that a consumer writes a number from the table alone |
| [`math`](crate::stdlib::features::math) | declarations restricted to mathematics mode, a scoped package (the commands of an equation line), a body parser of its own that checks a body as it is parsed ([`lines::LinesBehavior`](crate::stdlib::features::math::lines::LinesBehavior)), an argument parser of its own (the spacing of `\\`), a reader of a body's structure for other features and backends ([`lines`](crate::stdlib::features::math::lines)), a capability declared for another feature to provide (`flm.math.content`), and a library of plain functions that processing features' rules call ([`frame`](crate::stdlib::features::math::frame), [`math::latex`](crate::stdlib::features::math::latex)); one rule on a formula, the rule of last resort of the display-math environments ([`unprocessed`](crate::stdlib::features::math::unprocessed)) |
| [`math_passthrough`](crate::math_passthrough) | the library's math processor, a package of its own beside the standard features: two fallbacks in mathematics mode and their opaque element, a check stage providing `math`'s capability, rules marked `DefaultMark::Everywhere` for `flm.math`, `flm.math.environment` and the opaque element, two rules for `latex-recomposed` built from `math`'s library, and peer declarations of the tables its rules read |
| [`refs`](crate::stdlib::features::refs) | six stages, six tables and an external table, a check added to `flm.validated`, two render option types, a late-initialization hook that compiles the join texts, two referencing elements (`\ref`, `\hyperref`) with a general rule and a recomposition rule each, content built into a synthesized value that a rule splices |
| [`headings`](crate::stdlib::features::headings) | element data built from the configuration (one element value per heading kind, written under the names of its commands; [`HeadingDecl`](crate::stdlib::features::headings::HeadingDecl), [`HeadingCommand`](crate::stdlib::features::headings::HeadingCommand)), three aspects on one element, a rule reading the counter and anchor tables as optional peer tables, counters and counter styles contributed as peer contributions ([`headings::numbers`](crate::stdlib::features::headings::numbers)), a number written from a table and spliced, the peer declarations of a rule on the recomposed LaTeX ([`headings::latex::register`](crate::stdlib::features::headings::latex::register)) |
| [`verbatim`](crate::stdlib::features::verbatim) | verbatim arguments and verbatim environment bodies |
| [`lists`](crate::stdlib::features::lists) | environments built from the configuration (one element value per list kind), a scoped package whose command declares a reference point of its own counting through scoped instances found by walking up the tree, a body read into parts at lowering by one public reader ([`list_items`](crate::stdlib::features::lists::list_items)), a counter and per-depth counter styles contributed as peer contributions, a late-initialization hook compiling configured templates, a rule compiling a document's own template against the language the `Lowerer` gives ([`Lowerer::lang`](crate::core::lowering::Lowerer::lang)) |
| [`xnotes`](crate::stdlib::features::xnotes) | one element with one value per configured xnote kind, whose optional argument refers to another feature's scoped package through a peer declaration ([`labels_of`](crate::stdlib::features::xnotes::labels_of)); a block role read from an argument's layout; two reference points per node built by one public function ([`ref_points`](crate::stdlib::features::xnotes::ref_points)); a service slot of xnote kinds filled by one public function that makes every per-kind declaration and its peer declarations ([`contribute_kind`](crate::stdlib::features::xnotes::contribute_kind)); an aspect of its own that another feature's element implements ([`XnoteSource`](crate::stdlib::features::xnotes::XnoteSource)); a `Compute`/`Document` stage collecting points by kind; public staging functions that another feature's rule calls, one call per xnote, identified by its position among the node's xnotes of its kind ([`stage_mark`](crate::stdlib::features::xnotes::stage_mark), [`stage_in_place`](crate::stdlib::features::xnotes::stage_in_place), [`stage_latex`](crate::stdlib::features::xnotes::stage_latex)); one public implementation of the copy aspect for all of them ([`copied_as`](crate::stdlib::features::xnotes::copied_as)); per kind a piece built by a `Document` stage ([`ListBuilder`](crate::stdlib::features::xnotes::ListBuilder)), which another stage writing [`piece_table`](crate::stdlib::features::xnotes::piece_table) replaces when preferred, and its list rule ([`ListRule`](crate::stdlib::features::xnotes::ListRule)) with rules on the `html` and `latex-recomposed` backends; a stylesheet of its own ([`xnotes::html::stylesheet`](crate::stdlib::features::xnotes::html::stylesheet)). Public recipes: [an xnote kind of your own](crate::guide::extending#an-xnote-kind-of-your-own), [several xnotes on one node](crate::guide::extending#several-xnotes-on-one-node) |
| [`href`](crate::stdlib::features::href) | three commands as three values of one element; a braced verbatim argument read as characters ([`read_chars`](crate::lang::text::read_chars)), a command or mathematics there being the feature's own Error; an [`ApartFromText`](crate::model::aspects::ApartFromText) implementation returning that argument; a payload field the feature states rather than a backend guessing it ([`LinkPayload::shows_target`](crate::stdlib::backends::pres::LinkPayload::shows_target)); a recomposition rule writing a LaTeX hook of its own, defined by a definition piece contributed as a peer contribution and requiring a package, its argument read with [`chars_at_lowering`](crate::core::lowering::chars_at_lowering) and written with [`escape_url`](crate::stdlib::backends::latex::escape_url) |
| [`defterm`](crate::stdlib::features::defterm) | an element referring to a point through the `refs` feature ([`Referencing`](crate::model::aspects::Referencing), the resolutions table read optionally as a peer table); a point carrying labels its element computes beside the written ones; a repeatable tack-on field of its own beside the `\label` field; a configured template compiled by a late-initialization hook and instantiated at lowering with a placeholder bound to a synthesized node carrying a content pointer, lowered through [`Lowerer::lower_region`](crate::core::lowering::Lowerer::lower_region); a run-in decoration staged at the head of a body's first paragraph ([`lower_blocks_with_edges`](crate::stdlib::backends::lowering::lower_blocks_with_edges)); a LaTeX definition written from its configuration |
| [`theorems`](crate::stdlib::features::theorems) | environments built from the configuration, grouped into theorem classes ([`TheoremsConfig`](crate::stdlib::features::theorems::TheoremsConfig)); a counter and one counter style per kind contributed as peer contributions, a kind's words given as [`PrefixWords`](crate::stdlib::features::numbering::PrefixWords); an element that is a reference point and, for some kinds, a reference site read from an ordinary argument ([`title_reading`](crate::stdlib::features::theorems::title_reading)), with a site not shown ([`RefSite::shown`](crate::model::aspects::RefSite::shown)); a per-class template with two placeholders computed by an expansion hook, one possibly unbound so that `\IfValueT` tests it, compiled by a late-initialization hook; synthesized inline elements carrying [`ContentPointer`](crate::stdlib::backends::lowering::ContentPointer)s, one writing a reference from inside the pointed region ([`lower_reference`](crate::stdlib::features::refs::lower_reference)); a run-in and a final decoration placed as a body's two edges; a check of document scope in `flm.validated` ([`CHECK_PROOFS`](crate::stdlib::features::theorems::CHECK_PROOFS)) reading the resolutions only where the registry declares them; a stylesheet of its own ([`theorems::html::stylesheet`](crate::stdlib::features::theorems::html::stylesheet)) |
| [`quote`](crate::stdlib::features::quote) | environments built from the configuration ([`QuoteConfig`](crate::stdlib::features::quote::QuoteConfig)) whose body layout derives from it; one scoped package per environment holding exactly the commands it accepts, one element declared under one name in several packages ([`sections_package`](crate::stdlib::features::quote::sections_package)); a body read into quotation sections at lowering with nothing stored, a lines section split at a field node into lines indented by a [`Length`](crate::stdlib::backends::pres::Length); a rule for a command met where it opens nothing, which reports it and lowers its content ([`SectionRule`](crate::stdlib::features::quote::SectionRule)) |
| [`annotations`](crate::stdlib::features::annotations) | one element value per configured annotator and none without configuration; an argument parser of its own ([`AnnotationParser`](crate::stdlib::features::annotations::AnnotationParser); the form read back by [`form_of`](crate::stdlib::features::annotations::form_of)); a block-transparent element cut into cut parts with a continuation record and its payload reader ([`AnnotationPiece`](crate::stdlib::features::annotations::AnnotationPiece), [`payload_readers`](crate::stdlib::features::annotations::payload_readers)); an `ApartFromText` result depending on the form; a margin decoration; a general rule requiring the feature's style identifier, which the `html` backend delivers as a stylesheet ([`annotations::html`](crate::stdlib::features::annotations::html)) and the `latex-rendered` backend as a definition piece ([`annotations::latex_rendered`](crate::stdlib::features::annotations::latex_rendered)); rules of their own on the `text` and `latex-recomposed` backends, the LaTeX one declaring each annotator's command from the configuration ([`annotations::latex`](crate::stdlib::features::annotations::latex)); a render option read by the rules ([`AnnotationsRenderOptions`](crate::stdlib::features::annotations::AnnotationsRenderOptions)) |
| [`graphics`](crate::stdlib::features::graphics) | one element declared in a scoped package of its own (`flm.graphics.includegraphics`, which another feature's body refers to) and, under a configuration setting, as an ordinary command too, both sharing one `ElementSpec`; key-values read at lowering ([`graphics::options`](crate::stdlib::features::graphics::options)); a braced verbatim argument read with `chars_at_lowering` by rules and stages alike; three service slots a host fills and nothing requires; a `Compute`/`PerTree` stage whose table is keyed by node address ([`INFO`](crate::stdlib::features::graphics::INFO)), so that value catch-up runs it again; one stage type created per backend, writing under the `copy` strategy into the backend's artifact table declared as a peer table, the stage not registered where the backend is absent ([`PublishStage`](crate::stdlib::features::graphics::PublishStage), [`published_table`](crate::stdlib::features::graphics::published_table)); a rule created per backend ([`image_rule`](crate::stdlib::features::graphics::image_rule)) computing lengths with [`Length::scaled`](crate::stdlib::backends::pres::Length::scaled); a LaTeX hook with an optional key-value argument ([`graphics::latex`](crate::stdlib::features::graphics::latex)) |
| [`floats`](crate::stdlib::features::floats) | environments built from the configuration and from other features through one public function that makes every per-type declaration, as peer declarations where another feature owns them ([`contribute_type`](crate::stdlib::features::floats::contribute_type)); a service slot of trait objects ([`FloatContentHandler`](crate::stdlib::features::floats::FloatContentHandler)), given to the rule by a finalizer through [`Services::shared_named`](crate::core::services::Services::shared_named); a body whose scoped package holds the `refs` feature's own `\label` declaration beside fields of its own ([`label_package`](crate::stdlib::features::refs::label_package)), read at lowering by one public reader ([`read_float`](crate::stdlib::features::floats::read_float)); a document-scope check stage required by the rule ([`CHECK_HANDLERS`](crate::stdlib::features::floats::CHECK_HANDLERS)) |
| [`cells`](crate::stdlib::features::cells) | an environment in a scoped package of its own and, under a configuration setting, everywhere; a block role decided per node by one predicate ([`stands_in_body`](crate::stdlib::features::cells::stands_in_body)) that the reader follows; an argument parser of its own over techy's optional group ([`CellStylesParser`](crate::stdlib::features::cells::CellStylesParser)); a scoped package adding one special character (`&`); a service slot of declarations holding no code, collected by a finalizer that refuses a name declared twice ([`CellStyle`](crate::stdlib::features::cells::CellStyle)); one reader of the presentation payload ([`read_grid`](crate::stdlib::features::cells::read_grid)); the dropped mark for what a rule leaves out; requirements chosen per backend ([`Lowerer::backend`](crate::core::lowering::Lowerer::backend)); a float content handler contributed as a peer contribution ([`CellsContent`](crate::stdlib::features::cells::CellsContent)). Public recipe: [a cell style of your own](crate::guide::extending#a-cell-style-of-your-own) |
| [`citations`](crate::stdlib::features::citations) | two elements whose last argument is a tack-on argument holding further calls of the same shape; a document-scope stage over the element index writing a table keyed by citation key; a citation counter style compiled by a late-initialization hook without the `numbering` feature; one staging function two rules share; a service slot of providers asked by a compute stage; a check stage in `flm.validated`; a piece built by one list function |
| [`input`](crate::stdlib::features::input) | a command declared as an inclusion ([`SyntaxDef::Inclusion`](crate::lang::mint::SyntaxDef::Inclusion)), read by the language's own parser, which asks the host's [`InputResolver`](crate::lang::InputResolver) and parses a content input where the command is written, or records a heading input that the assembly places as a tree of its own; a rewrite stage ordered before the other rewrites by `before` edges alone, scheduled in every standard plan because its capability is a member of `flm.validated`; a rule lowering a placing node to `flm.pres.subtree`, which identifies the tree the run places there ([`Lowerer::placement`](crate::core::lowering::Lowerer::placement)), and a command whose file was not brought in to the dropped mark |
| [`userdefs`](crate::stdlib::features::userdefs) | author-written user definitions, a late-initialization hook compiling them and recording what the build decided about each ([`RegistryBuildFacts`](crate::lang::rewrites::userdefs::RegistryBuildFacts): recomposable or why not, the effective signature, whether the name is shared by a definition for the other parsing mode, the per-mode definitions), three rules created per definition (`expand-in-tree`, `emit-call`, `instantiate-at-lowering`; [`kept`](crate::stdlib::features::userdefs::kept)) |
| [`LoweringFeature`](crate::stdlib::backends::lowering::LoweringFeature) | registering the presentation vocabulary and the structural elements' default rules; two service slots and the native-set implications |
| [`metadata`](crate::stdlib::features::metadata) | an option type bound on the document ([`DocMeta`](crate::stdlib::features::metadata::DocMeta)) whose texts are parsed by one `Document` stage writing three piece tables, a capability added to `flm.validated`, three piece declarations contributed as peer contributions, and rules on the recomposed LaTeX under peer declarations |
| [`PiecesFeature`](crate::stdlib::backends::pieces::PiecesFeature) | a service slot other features contribute to as peer contributions ([`PieceDecl`](crate::core::piece::PieceDecl)) and a render option read by a stage the feature does not register: each backend that places pieces creates a [`PiecesStage`](crate::stdlib::backends::pieces::PiecesStage) itself |
| [`TextBackend`](crate::stdlib::backends::text::TextBackend) | a whole backend in one `register`: native set, stages, tables, render options, an output and a workflow, the shared pieces stage created for it, and its output regions |
| [`LatexFamily`](crate::stdlib::backends::latex::LatexFamily) | presentation elements of a family's own (the construct elements), two service slots and the standard contributions to them, and one rule, the one that writes a template placeholder as raw LaTeX ([`placeholder_rule`](crate::stdlib::backends::latex::placeholder_rule)), registered under a peer declaration of the backend it is the default on |
| [`LatexRecomposed`](crate::stdlib::backends::latex_recomposed::LatexRecomposed) | a backend over a typed sink: a native set of construct elements, rules requiring another tree state (`flm.blocks.raw`), a per-tree emit stage writing three tables, a pieces stage placing only the pieces a render lists for this backend, a document-scope regions stage (`flm.latex-recomposed.regions`) that lowers those pieces, assembles the style file from the requirement identifiers into its own table and writes the output's regions, and the backend's final stage assembling the entry artifact from the regions; each content feature's `latex.rs` registers its rule for this backend (`DefaultMark::OnBackends`) under peer declarations of the backend, the constructs and the tables it reads optionally, so a registry without the backend still builds |
| [`HtmlBackend`](crate::stdlib::backends::html::HtmlBackend) | a backend over a typed sink whose HTML contract is data ([`conventions`](crate::stdlib::backends::html::conventions)): a per-tree emit stage writing the HTML with the requirements of each tree, the shared pieces stage and a document-scope stage writing each piece's HTML, the stage resolving definitions, a document-scope regions stage (`flm.html.regions`) writing the page's regions (content, pieces, stylesheets, scripts, title, language), and the backend's final stage assembling the page from them (a host's compose template is a second provider of the same table, reading the same regions); a capability refined by how the page is written (`flm.output.html.singlefile`); the construct element `flm.pres.html.tag`; the four names `html` (feature, backend, output, workflow) a feature imports to render on this backend ([`html::construct`](crate::stdlib::backends::html::construct); rules of a content feature for this backend: [`math_passthrough::MathPassthrough`](crate::math_passthrough::MathPassthrough), [`xnotes::html`](crate::stdlib::features::xnotes::html)) |
| [`CheckWorkflow`](crate::stdlib::CheckWorkflow) | a feature declaring only a collection and a workflow over it |

[`stdlib::add_features`](crate::stdlib::add_features) adds them in this order: `textstyles`,
`literals`, `blocks`, `index`, `refpoints`, `numbering`, `math`, `refs`,
`headings`, `verbatim`, `lists`, `xnotes`, `href`, `defterm`, `theorems`, `quote`,
`annotations`, `graphics`, `floats`, `cells`, `citations`, `input`, `userdefs`, `lowering`,
`metadata`, `toc`, `pieces`, `text`, `latex`, `latex-recomposed`, `latex-rendered`, `html`,
`check`.
[`StdlibConfig`](crate::stdlib::StdlibConfig) holds the configuration of the sixteen
that take one: `blocks` ([`BlocksConfig`](crate::stdlib::features::blocks::BlocksConfig)),
`userdefs` ([`UserdefsConfig`](crate::stdlib::features::userdefs::UserdefsConfig)),
`headings` ([`HeadingsConfig`](crate::stdlib::features::headings::HeadingsConfig)),
`numbering` ([`NumberingConfig`](crate::stdlib::features::numbering::NumberingConfig)),
`math` ([`MathConfig`](crate::stdlib::features::math::MathConfig)), `refs`
([`RefsConfig`](crate::stdlib::features::refs::RefsConfig)), `lists`
([`ListsConfig`](crate::stdlib::features::lists::ListsConfig)), `xnotes`
([`XnotesConfig`](crate::stdlib::features::xnotes::XnotesConfig)), `defterm`
([`DeftermConfig`](crate::stdlib::features::defterm::DeftermConfig)), `theorems`
([`TheoremsConfig`](crate::stdlib::features::theorems::TheoremsConfig)), `quote`
([`QuoteConfig`](crate::stdlib::features::quote::QuoteConfig)), `annotations`
([`AnnotationsConfig`](crate::stdlib::features::annotations::AnnotationsConfig)),
`graphics` ([`GraphicsConfig`](crate::stdlib::features::graphics::GraphicsConfig)),
`floats` ([`FloatsConfig`](crate::stdlib::features::floats::FloatsConfig)), `cells`
([`CellsConfig`](crate::stdlib::features::cells::CellsConfig)) and `citations`
([`CitationsConfig`](crate::stdlib::features::citations::CitationsConfig)).

## Pitfalls

- Begin every name you declare with your own owner segment; a name without one will
  eventually collide.
- Ordering features by dependency does nothing; registration order matters only where
  [Packages and registration order](#packages-and-registration-order) says.
- An aspect reads no options and is evaluated per occurrence; a fact that varies per
  document belongs to a stage.
- `b.attach::<dyn Trait>(…)`: a concrete type does not compile, and attaching an aspect
  an element already carries is a build error.
- A `Check` stage writes no table; a rewrite stage reads no computed data and requires no
  computed capability.
- List every presentation element a rule may build in `produces`: the `Lowerer` refuses
  to stage another, and the bound on lowering rounds is computed from it.
- A rule that refers to a backend, produces an element or reads a table its feature does
  not declare needs a peer declaration, else `BuildError::Undeclared`; a misspelled peer
  name is a Note and a set-aside rule, not an error: read the build diagnostics.
- A value that exists only after some work is not a service; use an external table.
- Feed configuration the declarations do not show into `digest_input`, or caches keyed
  on the registry digest are reused after it changes.
- Read arguments by name (`lower_arg_named`), not by position.
- A rule that shows content written elsewhere points at it and lowers its region once;
  it never copies the document's content into a synthesized value.
- In an extension, an unrestricted `walk()` or `descendants()` that tests each node
  itself makes one call into the host per node, which is slow on a long document:
  restrict the traversal (`elements`, `kinds`)
  ([Traversals and sequences in an extension](#traversals-and-sequences-in-an-extension)).
- In an extension, a `[...nodes]` of a body or of a traversal keeps one engine object per
  node and can make a long document fail at the engine's memory limit: pass the sequence
  on as it is.

## See also

- [Extending FLM](crate::guide::extending): the same ground for a human reader, with the
  reasoning and the recipes for xnote kinds and cell styles.
- [Rendering: backends and outputs](crate::guide::rendering): rules, emitters, artifacts,
  the HTML contract and the LaTeX hooks in full.
- [Numbering](crate::guide::numbering): counters, counter styles and the numbering
  feature in full.
- [AI guide: pipeline](crate::guide::ai_guide_pipeline): how the plan that schedules
  your stage is built.
- [AI guide: document model](crate::guide::ai_guide_document_model): the trees and
  elements your stage reads.
- [AI guide: embedding](crate::guide::ai_guide_embedding): building the registry and
  running it from a program.
- [Concepts overview](crate::guide::concepts_overview): the concepts, one heading each.
