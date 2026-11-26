# AI guide: embedding

This page is the AI guide for writing a program that embeds FLM. A program embeds FLM in
one of two ways:

- **`flm-core`** directly: the program hands the library source text and takes values
  back (output bytes in tables, diagnostics, trees). The library performs no input or
  output. Sections 1 to 6 below and the sections after them through "Threads, the
  `parallel` feature and WebAssembly" cover it.
- **`flm-host`**, the host library: the program hands it a session request (a document
  file, an output format, settings) and a file reader, and it reads the document, its
  front matter and the extensions it uses, combines the settings, builds the registry and the
  plan, runs `flm-core` and writes the output files through a sink. Section "The host
  library `flm-host`" covers it. Everything it does is a composition of the `flm-core`
  steps, so read those first.

A third crate, **`flm-info`**, describes what a program embeds rather than processing a
document: the commands, settings, workflows and names of a registry, as typed views, JSON
and documentation pages. Section "Describing an environment: `flm-info`" covers it.

Load this page together with the entry page, [AI guide](crate::guide::ai_guide), whose
[vocabulary](crate::guide::ai_guide#vocabulary) defines every term used here. The pages
for people that cover the same ground with the reasoning are [Embedding
flm-core](crate::guide::embedding) and [The flm command-line
tool](crate::guide::command_line). Writing features of your own is [AI guide: extending
FLM](crate::guide::ai_guide_extending).

## What the host supplies, and what the library guarantees

A *host* is any program that embeds the library.

- **The processing performs no input or output**: no file, no network, no process. The
  host reads the source text, hands it in as an `Arc<`[`FlmSource`](crate::lang::FlmSource)`>`,
  and takes the output as [`Artifact`](crate::core::artifact::Artifact) values out of the
  run's tables. The only part of `flm-core` that touches files is the golden-file test
  helper [`crate::dump::golden`].
- **The host supplies what needs files, programs or the network**, in two ways:
  - an **input resolver**, an implementation of
    [`InputResolver`](crate::lang::InputResolver), which finds and reads the file an
    `\input` command refers to (see [Documents of several
    files](#documents-of-several-files)); without one, every `\input` is reported as the
    Error `flm.input.unavailable` and includes nothing;
  - **service values** contributed to the service slots that features declare, when the
    registry is built (see [Services the host provides](#services-the-host-provides)):
    the naming scheme of output files, the image resolver, inspector and converters, the
    citation providers.
- **Data from outside one document** reaches a run through an external table installed
  when the run is constructed (see [External tables and two-run
  composition](#external-tables-and-two-run-composition)).
- **Reach techy through [`flm_core::techy`](crate::techy)** (and the Unicode-to-LaTeX
  encoder through [`flm_core::untechxt`](crate::untechxt)), never through a dependency of
  your own: two versions of techy in one build are two unrelated sets of types.
- **No document can make the library panic.** Every problem a document causes is a
  diagnostic or a typed error; [`crate::guide::panics`] lists every public item that can
  panic, each under a condition a document cannot bring about.

Minimum Rust version: 1.86 for `flm-core`, 1.88 for `flm-host` and `flm`.

## The six steps

```text
1 registry   RegistryBuilder::build → Arc<FlmRegistry>        once per program
2 parse      parse_fragment, or assemble → Fragment / DocContent   once per source
3 document   DocContent::place, Doc::new → Doc                once per document
4 plan       Plan::for_workflow or Plan::build → Plan         once per plan request
5 run        Run::new, then execute or step → Run             per document and plan
6 read       Run::table, frozen_trees, diagnostics
```

Steps 1 and 4 depend on no document: build the registry and each plan once and use them
for every document. A plan holds its registry (`Plan::registry`), so one `Arc<Plan>` is
enough to run.

```rust
use std::sync::Arc;

use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::stdlib::backends::text::{self, TEXT_DOCUMENT};
use flm_core::stdlib::{self, StdlibConfig};

// 1. Registry: a package of the standard features, and a math processor.
let mut builder = RegistryBuilder::new();
builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
builder.use_package(flm_core::math_passthrough::package());
let registry = builder.build(&RegistryInputs::new())?;

// 2. Parse. The source carries its own origin, the key of everything keyed
//    by source: diagnostics, include-cycle detection, a host's side tables.
let source = Arc::new(
    FlmSource::new("Hello \\textbf{world}.").with_origin(FlmOrigin::new("hello.flm")),
);
let fragment = parse_fragment(
    registry.lang(),
    source,
    FragmentLayout::default(),
    &ParseOptions::default(),
)?;

// 3. Document: one placed fragment.
let mut content = DocContent::new();
content.place(Arc::new(fragment));
let doc = Arc::new(Doc::new(content));

// 4. Plan of the `text` workflow, with nothing changed about it.
let plan = Arc::new(Plan::for_workflow(&registry, text::WORKFLOW, &Overrides::none())?);

// 5. Run.
let mut run = Run::new(doc, plan, Externals::none())?;
run.execute(&RunOptions::new())?;

// 6. Read: the output's artifact set, and the bytes of its entry artifact.
let artifacts = run.table(TEXT_DOCUMENT)?.expect("the `text` workflow writes it");
let bytes = artifacts.entry().and_then(|a| a.as_bytes()).expect("one entry artifact");
assert_eq!(String::from_utf8_lossy(bytes).trim_end(), "Hello world.");
# Ok::<(), Box<dyn std::error::Error>>(())
```

## 1. The registry

The registry is the frozen record of everything the language and the pipeline consult
for a set of features ([`crate::core::registry`]). It is built once, returned as
`Arc<`[`FlmRegistry`](crate::core::registry::FlmRegistry)`>`, and shared by every
parse, plan and run.

### Building it

[`RegistryBuilder`](crate::core::registry::RegistryBuilder) collects, and checks nothing
until [`build`](crate::core::registry::RegistryBuilder::build):

| method | adds |
|---|---|
| [`use_package`](crate::core::registry::RegistryBuilder::use_package)`(PackageEntry)` | the features of a [`PackageEntry`](crate::core::registry::PackageEntry): first those of the packages it uses, then its own; a package name already added is skipped, so no feature is added twice |
| [`feature`](crate::core::registry::RegistryBuilder::feature)`(Arc<dyn FlmFeature>)` | one [`FlmFeature`](crate::core::registry::FlmFeature) after those already added; two features of one name fail the build ([`BuildError::DuplicateFeature`](crate::core::registry::BuildError::DuplicateFeature)) |
| [`contribute`](crate::core::registry::RegistryBuilder::contribute)`(Arc<S>)` | a service value from the host, for the service slot whose value type is `S` |
| [`contribute_to`](crate::core::registry::RegistryBuilder::contribute_to)`(slot, Arc<S>)` | a service value from the host, for the service slot of that dotted name |
| [`workflow_fields`](crate::core::registry::RegistryBuilder::workflow_fields)`(WorkflowDef)` | fields of a workflow from the host: each field the value declares replaces that field of the workflow of that name, after every feature's; with no workflow of that name, the value is the workflow |

Each method returns `&mut RegistryBuilder`. `build(self, &`[`RegistryInputs`](crate::core::registry::RegistryInputs)`)`
consumes the builder and returns `Result<Arc<FlmRegistry>, `[`RegistryBuildError`](crate::core::registry::RegistryBuildError)`>`.
A host's contributions are attributed to [`HOST_FEATURE`](crate::core::registry::HOST_FEATURE)
(`_host`) and come after every feature's contribution.

`flm-core` ships the standard features (module [`stdlib`](crate::stdlib)) and one package,
the math processor `math-passthrough`. A program declares its own package entry and adds
the standard features to it:
[`stdlib::add_features`](crate::stdlib::add_features)`(PackageEntry, `[`StdlibConfig`](crate::stdlib::StdlibConfig)`)`
returns the `PackageEntry` it is given with the thirty-three features listed in the
[`stdlib`](crate::stdlib) module documentation added; they process no mathematics. A
registry that renders adds a math processor:
[`math_passthrough::package`](crate::math_passthrough::package)`()` (the package
`math-passthrough`, which needs no JavaScript and carries each formula's source to the
output), or another, such as the extension `math-mathjax`. Without one, a document
without formulas renders without error, and a render reports each formula as the Error
`flm.math.unprocessed` (no workflow runs a check of the formulas, which only a processor
adds to `flm.validated`); two processors in one registry are an error. The entry page's
[feature map](crate::guide::ai_guide#the-features) lists what each declares.

### The configuration of the standard features

[`StdlibConfig`](crate::stdlib::StdlibConfig) holds the configuration of each of the
sixteen standard features that take one; the other seventeen take none. A feature's
configuration is the plain Rust value the feature is constructed from, fixed when the
registry is built ([settings, options and
configuration](crate::guide::concepts_overview#settings-options-and-configuration)). Each
field has a builder method of the same name (`StdlibConfig::new().theorems(…)`):

| field | type | default |
|---|---|---|
| `blocks` | [`BlocksConfig`](crate::stdlib::features::blocks::BlocksConfig) | the text options (typographic substitutions) |
| `userdefs` | [`UserdefsConfig`](crate::stdlib::features::userdefs::UserdefsConfig) | no user definition; a program that gathers definitions from several sources merges them into this one value |
| `headings` | [`HeadingsConfig`](crate::stdlib::features::headings::HeadingsConfig) | the kinds `section` (level 1) to `subparagraph` (level 5), one command each, none run in, none numbered |
| `numbering` | [`NumberingConfig`](crate::stdlib::features::numbering::NumberingConfig) | no counter style replaced |
| `math` | [`MathConfig`](crate::stdlib::features::math::MathConfig) | equations numbered |
| `refs` | [`RefsConfig`](crate::stdlib::features::refs::RefsConfig) | join texts `A and B`, `A, B, and C` |
| `lists` | [`ListsConfig`](crate::stdlib::features::lists::ListsConfig) | `itemize`, `enumerate` (numbered), `description` |
| `xnotes` | [`XnotesConfig`](crate::stdlib::features::xnotes::XnotesConfig) | one xnote kind, `footnote`, numbered and listed after the content |
| `defterm` | [`DeftermConfig`](crate::stdlib::features::defterm::DeftermConfig) | lead-in template `#{term}: ` |
| `theorems` | [`TheoremsConfig`](crate::stdlib::features::theorems::TheoremsConfig) | eight kinds in the classes `theoremlike`, `definitionlike`, `prooflike` ([`simpleset`](crate::stdlib::features::theorems::TheoremsConfig::simpleset), [`richset`](crate::stdlib::features::theorems::TheoremsConfig::richset) are the other shipped sets) |
| `quote` | [`QuoteConfig`](crate::stdlib::features::quote::QuoteConfig) | `quote`, `blockquote`, `address` |
| `annotations` | [`AnnotationsConfig`](crate::stdlib::features::annotations::AnnotationsConfig) | no annotator, so no command |
| `graphics` | [`GraphicsConfig`](crate::stdlib::features::graphics::GraphicsConfig) | `\includegraphics` only in a float's body (`global: false`) |
| `floats` | [`FloatsConfig`](crate::stdlib::features::floats::FloatsConfig) | float types `figure` and `table`, numbered when labeled |
| `cells` | [`CellsConfig`](crate::stdlib::features::cells::CellsConfig) | `cells` only in a `table` float's body (`global: false`) |
| `citations` | [`CitationsConfig`](crate::stdlib::features::citations::CitationsConfig) | bracketed marks with the citation counter style (`[1]`, words `ref.~`, `refs.~`), the list under `References` at level 2, `; ` before a note |

Each configuration is declared with the binding
[`Bind::Registry`](crate::model::options::Bind::Registry) and enters the registry digest.
The registry build checks every configuration; a refused one fails the build and the
error identifies the part at fault. [Configuring the standard
features](crate::guide::embedding#configuring-the-standard-features) gives a worked
example for each of the configurations a host most often changes.

### The registry inputs

[`RegistryInputs::new`](crate::core::registry::RegistryInputs::new) sets every field to
its default; change only the fields you need:

| field | what it is | enters the digest |
|---|---|---|
| `parsing` | [`FlmParsing`](crate::lang::FlmParsing): `comments`, `comment_prefix` (`%%` by default), `dollar_math` (off by default); they decide the forbidden characters ([`FlmParsing::percent_forbidden`](crate::lang::FlmParsing::percent_forbidden)) | yes |
| `structural` | the specifications of the structural elements ([`StructuralElementSpecs`](crate::lang::structural::StructuralElementSpecs); the registry builds [`StructuralElements`](crate::lang::structural::StructuralElements) from them) | yes |
| `guard` | [`GuardPolicy`](crate::lang::guard::GuardPolicy): `StackBudget` (the default; for normal use) or `DepthLimit(n)` (tests only: it refuses at the same input on every platform, and does not protect the stack) | no |
| `diagnostics_limit` | how many diagnostics one parse keeps (`None`: techy's default) | no |
| `input_resolver` | the input resolver of every parse under this registry | no |
| `build_identity` | a string identifying your program (its version, a hash of its binary): a cache written by one program is never read by another | yes |

The registry digest ([`crate::model::digest`]; [`FlmRegistry::digest`](crate::core::registry::FlmRegistry::digest))
is a fingerprint of everything about the registry that can change what processing
produces; a host that caches results keys them by it. A feature of your own whose
declarations depend on its configuration adds that configuration with
[`FeatureBuilder::digest_input`](crate::core::registry::FeatureBuilder::digest_input).

### When the build fails, and what a successful build reports

A failed build is a [`RegistryBuildError`](crate::core::registry::RegistryBuildError)
`{ phase, errors }`: the first [`BuildPhase`](crate::core::registry::BuildPhase)
(`collect`, `finalize`, `validate`, `language`, `late-init`) that found problems, and
every problem it found, each a [`BuildError`](crate::core::registry::BuildError). Later
phases did not run. [`render`](crate::core::registry::RegistryBuildError::render) gives
the problems one per line, stable enough for a test to compare.

A successful build may still report problems in
[`FlmRegistry::build_diagnostics`](crate::core::registry::FlmRegistry::build_diagnostics):
the warnings of a counter style, the errors and warnings of each user definition given
in `UserdefsConfig` (a refused definition stays registered, and every use of it is
reported again), and one Note per peer declaration that no feature resolves (with the
lowering rules set aside because of it,
[`set_aside_rules`](crate::core::registry::FlmRegistry::set_aside_rules)). **A build
succeeds even when these hold Errors**: report the collection once per build.

### Reading a finished registry

By name: [`workflow`](crate::core::registry::FlmRegistry::workflow),
[`workflows`](crate::core::registry::FlmRegistry::workflows),
[`combined_workflow`](crate::core::registry::FlmRegistry::combined_workflow) (a workflow
with the fields it inherits through `extends`),
[`output`](crate::core::registry::FlmRegistry::output),
[`outputs`](crate::core::registry::FlmRegistry::outputs),
[`backend`](crate::core::registry::FlmRegistry::backend),
[`table_named`](crate::core::registry::FlmRegistry::table_named),
[`external_tables`](crate::core::registry::FlmRegistry::external_tables),
[`stage_named`](crate::core::registry::FlmRegistry::stage_named),
[`capability_named`](crate::core::registry::FlmRegistry::capability_named),
[`element_named`](crate::core::registry::FlmRegistry::element_named),
[`options`](crate::core::registry::FlmRegistry::options) (every option type with its
binding), [`option_decl`](crate::core::registry::FlmRegistry::option_decl),
[`services`](crate::core::registry::FlmRegistry::services),
[`features`](crate::core::registry::FlmRegistry::features),
[`has_feature`](crate::core::registry::FlmRegistry::has_feature),
[`lang`](crate::core::registry::FlmRegistry::lang) (the language to parse with).

### Settings given as data, and packages by name

Settings are a feature's configuration and option values written as data, one map per
feature from a settings key to its value, read into the feature's own Rust types
([`crate::core::settings`]). A host that takes settings from a person (a command line, a
front matter) uses this route instead of building `StdlibConfig` in Rust.

- [`stdlib::add_features_from_settings`](crate::stdlib::add_features_from_settings)`(PackageEntry, &PackageSettings)`
  → `PackageEntry`: the standard features configured from settings, each switched off
  feature left out, called in the build function of a package's factory
  ([`PackageFactory::new`](crate::core::registry::PackageFactory::new)`(name, build)` →
  `Result<`[`PackageFactory`](crate::core::registry::PackageFactory)`, PackageError>`).
  [`features`](crate::core::registry::PackageFactory::features)
  lists each feature, with its [`SettingsDecl`](crate::core::settings::SettingsDecl) when
  it takes settings; [`settings`](crate::core::registry::PackageFactory::settings)`(name)`
  returns one declaration; [`build`](crate::core::registry::PackageFactory::build)`(&`[`PackageSettings`](crate::core::settings::PackageSettings)`)`
  → `Result<PackageEntry, PackageError>`, with each feature switched off by
  [`PackageSettings::switch_off`](crate::core::settings::PackageSettings::switch_off) left
  out.
- [`SettingsDecl::read`](crate::core::settings::SettingsDecl::read)`(&serde_json::Map)` →
  `Result<`[`FeatureSettings`](crate::core::settings::FeatureSettings)`, Vec<`[`SettingsError`](crate::core::settings::SettingsError)`>>`.
  A `FeatureSettings` gives [`config::<C>()`](crate::core::settings::FeatureSettings::config),
  [`document_options()`](crate::core::settings::FeatureSettings::document_options) (set
  them on the document) and [`render_options()`](crate::core::settings::FeatureSettings::render_options)
  (set them in the plan request); insert it into the `PackageSettings` with
  [`insert`](crate::core::settings::PackageSettings::insert). `read` refuses a key the
  feature does not declare and a value of the wrong shape; each error gives the key's
  path from the feature's name (`text.width`), and the host adds the file and line.
- Spelling of the data: keys are field names; an enum value in lower case with hyphens
  (`when-needed`, `in-place`); a variant with a value as `{variant: value}`
  (`{up-to-depth: 2}`). A collection of named things (xnote kinds, float types) is a map
  from each name to its entry, read in the order written; its default entries as data
  are [`SettingsDecl::default_entries`](crate::core::settings::SettingsDecl::default_entries)`(path)`.
  For that order, `flm-core` turns on `serde_json`'s cargo feature `preserve_order`,
  which then applies to every JSON map of your program.
- [`SettingsDecl::json_schema`](crate::core::settings::SettingsDecl::json_schema) is the
  JSON Schema of a feature's settings, with descriptions from the fields' documentation.
- Combining several sources of settings into one map per feature is the host's task,
  done before reading (`flm-host` does it; see [The host library
  `flm-host`](#the-host-library-flm-host)).
- [`PackageCatalog`](crate::core::registry::PackageCatalog) finds a package factory by
  name: [`add_compiled`](crate::core::registry::PackageCatalog::add_compiled),
  [`add_resolver`](crate::core::registry::PackageCatalog::add_resolver) (a
  [`PackageResolver`](crate::core::registry::PackageResolver), asked in order only when
  no compiled factory has the name; `flm-core` ships none),
  [`resolve`](crate::core::registry::PackageCatalog::resolve)`(name)`, and
  [`resolve_all`](crate::core::registry::PackageCatalog::resolve_all)`(&[&str])`, which
  returns the factories in registration order (each after the packages it uses, each
  once) or every problem as a [`UseError`](crate::core::registry::UseError).

[Settings given as data, and packages by
name](crate::guide::embedding#settings-given-as-data-and-packages-by-name) holds a
compiled example of this route.

## 2. Parsing

[`parse_fragment`](crate::lang::parse_fragment)`(&`[`BuiltLang`](crate::lang::BuiltLang)`, Arc<FlmSource>, `[`FragmentLayout`](crate::model::doc::FragmentLayout)`, &`[`ParseOptions`](crate::lang::ParseOptions)`)`
→ `Result<`[`Fragment`](crate::model::doc::Fragment)`, `[`FragmentError`](crate::lang::FragmentError)`>`.

- **The language comes from the registry**:
  [`FlmRegistry::lang`](crate::core::registry::FlmRegistry::lang) is the only way to
  obtain one. It is `Send + Sync`; one value serves any number of parses on any number of
  threads.
- **The caller builds the source.** An [`FlmOrigin`](crate::lang::FlmOrigin)'s canonical
  path identifies the source wherever something about it is recorded: in the diagnostics'
  file name, in the check that no file includes itself, in a host's own tables.
  [`FlmOrigin::new`](crate::lang::FlmOrigin::new)`(path)` for a file,
  [`FlmOrigin::none`](crate::lang::FlmOrigin::none) for an in-memory snippet. When the
  content starts below a front matter, techy's `with_line_column_number_offsets(line, 1)`
  on the source makes positions count from the top of the file (`flm-host` does so).
- **The layout**: [`FragmentLayout`](crate::model::doc::FragmentLayout) says how the top
  level is read: as blocks, as inline content, or as blocks only when it holds a block
  (the default).
- **A parse is tolerant.** A problem in the source is a diagnostic held in the fragment
  ([`Fragment::diags`](crate::model::doc::Fragment::diags)), and the tree covers the whole
  input. An unknown command, a stray `$`, an unbalanced group are diagnostics.
- **[`FragmentError`](crate::lang::FragmentError) lists the only ways a parse yields no
  tree**, none of them about the document's syntax: `NestingTooDeep` (nesting past the
  recursion guard), `Aborted` (an extension's parser broke a requirement of techy's
  parser, or a method of the language failed: an error in the program),
  `WorkerThread` (the operating system refused a worker thread).
  [`FragmentError::diagnostic`](crate::lang::FragmentError::diagnostic) returns the one
  diagnostic to record in place of the missing tree.
- [`ParseOptions`](crate::lang::ParseOptions) has two fields, both overriding the
  language's setting for one parse: `max_diagnostics` (diagnostics past the cap are
  counted and the count is reported) and `input_resolver`.
- [`parse_fragment_with_state`](crate::lang::parse_fragment_with_state) is the same parse
  started from a parsing state derived from the language's initial state with
  [`BuiltLang::derive_state`](crate::lang::BuiltLang::derive_state).

A fragment exposes [`tree`](crate::model::doc::Fragment::tree),
[`source`](crate::model::doc::Fragment::source),
[`diags`](crate::model::doc::Fragment::diags) and
[`layout`](crate::model::doc::Fragment::layout); share it as `Arc<Fragment>`.

### The worker-thread requirement

Parsing, rewriting, lowering and emitting are recursive. The recursion guard
([`crate::lang::guard`]) refuses to descend further once an operation has used
[`STACK_BUDGET`](crate::lang::guard::STACK_BUDGET) (48 MiB) of stack, and reports a
nesting-too-deep condition instead of overflowing the stack (`flm.blocks.nesting-too-deep`
for a parse or a rewrite; the lowering and each backend's emit stage report their own).
The budget limits use; it does not measure the thread's stack. So every guarded operation
must run on a **worker thread**: a thread created with at least
[`WORKER_STACK_SIZE`](crate::lang::guard::WORKER_STACK_SIZE) (64 MiB) of stack ([the
recursion guard and worker
threads](crate::guide::concepts_overview#the-recursion-guard-and-worker-threads)).

- **A host need arrange nothing**: the library's entry points check a thread-local marker
  and create a worker thread when the calling thread is not marked.
- [`on_worker_thread`](crate::lang::guard::on_worker_thread)`(f)` → `std::io::Result<T>`
  runs `f` in place on a marked thread, otherwise on a new worker thread. Wrap a batch of
  work in it to spare each inner entry point a thread of its own (`flm dump` wraps its
  whole command), and wrap **your own** recursive walks of a tree in it: they carry no
  guard, and only their thread's stack limits them.
- [`run_with_worker_stack`](crate::lang::guard::run_with_worker_stack) always creates a
  new worker thread. [`is_worker_thread`](crate::lang::guard::is_worker_thread) reads the
  marker.
- [`mark_worker_thread`](crate::lang::guard::mark_worker_thread) marks the current thread.
  **Call it only on a thread that really has `WORKER_STACK_SIZE` bytes of stack**: marking
  a smaller one allows the stack overflow the requirement prevents.
- When the operating system refuses the thread, the parse is not attempted and reports
  [`WorkerThreadUnavailable`](crate::lang::parse::WorkerThreadUnavailable)
  (`flm.lang.worker-thread-unavailable`, through `FragmentError::WorkerThread`); a run
  returns [`RunError::Worker`](crate::core::run::RunError::Worker).

## 3. The document

A [`Doc`](crate::model::doc::Doc) is its [`DocContent`](crate::model::doc::DocContent)
and the option values bound on it, and nothing else: no front matter, no chapters, no
named parts ([documents, fragments, and the
corpus](crate::guide::concepts_overview#documents-fragments-and-the-corpus)). The content
is an arena that numbers each tree with a [`TreeId`](crate::model::addr::TreeId), the
traversal list (the order in which the trees are walked), and the resolution map (which
node places which tree).

- [`DocContent::place`](crate::model::doc::DocContent::place)`(Arc<Fragment>)` → `TreeId`:
  arena **and** traversal list. The usual call.
- [`DocContent::add_tree`](crate::model::doc::DocContent::add_tree)`(Arc<Fragment>)` →
  `TreeId`: arena only, for a tree that the walk reaches through a node that places it.
- [`DocContent::resolve`](crate::model::doc::DocContent::resolve)`(site, target)` →
  `Result<(), PlacementError>`: records that the node `site` places the tree `target`, so
  that the walk visits that tree at the node (a heading input is placed this way).
- Placing one shared `Arc<Fragment>` twice makes **two trees**, each with computed data of
  its own; the fragment's parse diagnostics are reported once.
- [`Doc::new`](crate::model::doc::Doc::new)`(content)` binds no option value;
  [`Doc::with_options`](crate::model::doc::Doc::with_options)`(content, OptionValues)`
  binds the document's option values (see [Options](#options)).

### Documents of several files

`\input{file}` is the command of the standard [`input`](crate::stdlib::features::input)
feature. The library reads no file: for each `\input`, it calls the host's
[`InputResolver`](crate::lang::InputResolver):

```text
fn resolve(&self, reference: &str, including: Option<&str>, parsing: &FlmParsing)
    -> Result<IncludedFile, InputError>
```

It passes the reference as written, the name of the file that holds the command, and the
parsing settings that apply where the command is written. The resolver finds and reads
the file, splits off its front matter, and returns an
[`IncludedFile`](crate::lang::IncludedFile):

| field | meaning |
|---|---|
| `name` | the name under which the file is known: what messages print, and what tells two files apart; two references to one file must give one name |
| `text` | the content, front matter removed |
| `first_line` | the line of the file on which `text` begins, counted from 1 |
| `parsing` | `Some(settings)` when the file sets parsing settings of its own: the settings passed in, with the file's keys replaced; `None` keeps the state that applies at the command |
| `problems` | the problems found in the front matter ([`IncludedFileProblem`](crate::lang::IncludedFileProblem): line, column, message); reported, and the content is still included |
| `heading` | `Some(`[`InputHeading`](crate::lang::InputHeading)`)` for a **heading input**: the heading command, the title as FLM text and where it is written, the label, whether it is numbered; `None` for a **content input** |

or an [`InputError`](crate::lang::InputError)`::new(message)` when no file is found or it
cannot be read (reported as `flm.input.unreadable` at the command). The resolver alone
decides how a reference becomes a file and what a front matter holds; it needs no memory
of earlier calls.

The library does the rest ([content inputs and heading
inputs](crate::lang::input#content-inputs-and-heading-inputs)): it parses a content
input's text at the command under the parsing state there, and the rewrite stage
`flm.input.splice` later replaces the command by that content, so every later stage sees
one continuous text. It makes a heading input a tree of its own, opening with its
heading, placed at the command. It checks that no file includes itself (by comparing
`IncludedFile::name` with the canonical paths of the including sources) and that a chain
of inclusions is no deeper than [`DEPTH_LIMIT`](crate::lang::input::DEPTH_LIMIT) (32).

[`assemble`](crate::model::assemble::assemble)`(&BuiltLang, Arc<FlmSource>, FragmentLayout, &ParseOptions)`
→ `Result<DocContent, FragmentError>` parses a primary file (the file the host is asked to
process) and every file it includes into a document's content. Only `assemble` can place
a heading input; a plain `parse_fragment` reports one as an Error. Give the resolver either
for one assembly, in [`ParseOptions::input_resolver`](crate::lang::ParseOptions::input_resolver)
(suits a resolver that records the files one build read), or for every parse of a
registry, in [`RegistryInputs::input_resolver`](crate::core::registry::RegistryInputs#structfield.input_resolver).
[Documents of several files](crate::guide::embedding#documents-of-several-files) holds a
compiled example with a resolver over files held in memory.

## 4. The plan

A plan is the list of stages that satisfies a plan request, in dependency order and
grouped in batches; it is a pure value of the registry and the request, with nothing of
any document in it ([`Plan`](crate::core::plan::Plan)). Nobody lists stages by hand: a
plan request demands capabilities, and [`Plan::build`](crate::core::plan::Plan::build)`(&Arc<FlmRegistry>, &PlanRequest)`
derives the stages ([AI guide: pipeline](crate::guide::ai_guide_pipeline) describes the
derivation).

Most callers choose a registered workflow, a named set of demands together with the
choices that go with them ([`WorkflowDef`](crate::core::registry::WorkflowDef)):
[`Plan::for_workflow`](crate::core::plan::Plan::for_workflow)`(&registry, name, &`[`Overrides`](crate::core::plan::Overrides)`)`.
[`workflow_request`](crate::core::plan::workflow_request) returns the plan request
without building it, to inspect or change it before [`Plan::build`](crate::core::plan::Plan::build).

| workflow | demands |
|---|---|
| `text` ([`text::WORKFLOW`](crate::stdlib::backends::text::WORKFLOW)) | the `text` output and [`flm.validated`](crate::stdlib::VALIDATED) |
| `html` ([`html::WORKFLOW`](crate::stdlib::backends::html::WORKFLOW)) | the `html` output (one page) and `flm.validated` |
| `latex-rendered` ([`latex_rendered::WORKFLOW`](crate::stdlib::backends::latex_rendered::WORKFLOW)) | the `latex-rendered` output and `flm.validated`; it reads the trees laid out in paragraphs, so it shares a plan with `text` and `html` |
| `latex-recomposed` ([`latex_recomposed::WORKFLOW`](crate::stdlib::backends::latex_recomposed::WORKFLOW)) | the `latex-recomposed` output and `flm.validated`; its rules need the raw trees, so it never shares a plan with `text`, `html` or `latex-rendered` |
| `check` ([`CHECK_WORKFLOW`](crate::stdlib::CHECK_WORKFLOW)) | `flm.validated` alone: every check, no output |

`flm.validated` is a collection: a name for the set of capabilities that every check of
the standard features adds itself to. A render therefore reports what `check` reports,
with one difference: an unresolved reference is an Error in a render (unless
[`RefsRenderOptions`](crate::stdlib::features::refs::RefsRenderOptions) says Warning) and
always a Warning under `check`. A registry holding a standard backend must hold the
`check` feature ([`check_feature`](crate::stdlib::check_feature)), which declares the
collection; `stdlib::add_features` adds it. `text` and `html` can be demanded in one
plan request, since both want the structured trees.

[`Overrides`](crate::core::plan::Overrides) changes a workflow's request without
restating it ([`Overrides::none`](crate::core::plan::Overrides::none) changes nothing):

| field | combines with the workflow's value |
|---|---|
| `demands` | added |
| `prefer` | stages preferred ahead of the workflow's preferences |
| `lowering` | per backend, a [`LoweringSelection`](crate::core::registry::LoweringSelection) replacing the workflow's whole selection for that backend |
| `unsupported` | accepted unrenderable elements per backend, added (each occurrence lowers to a childless `flm.pres.unsupported` node with one Error; [`PlanRequest::unsupported`](crate::core::plan::PlanRequest#structfield.unsupported)) |
| `render_options` | one option type at a time: a value given here replaces the workflow's value of that type |

[`PlanRequest`](crate::core::plan::PlanRequest) has the same five fields written out in
full (with builder methods `demand`, `prefer`, `lowering`, `unsupported`,
`render_options`); besides the registry, nothing else decides a plan. A sixth field,
`workflow`, names the workflow the request was made from, `None` for a request built by
hand; it decides nothing, and the plan's errors and warnings name it.

```rust
use flm_core::model::options::OptionValues;
use flm_core::core::plan::{Overrides, Plan};
use flm_core::stdlib::backends::text::{self, TextRenderOptions};
# use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
# use flm_core::stdlib::{self, StdlibConfig};
# let mut builder = RegistryBuilder::new();
# builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
# builder.use_package(flm_core::math_passthrough::package());
# let registry = builder.build(&RegistryInputs::new())?;

let overrides = Overrides {
    render_options: OptionValues::new()
        .with(TextRenderOptions { width: Some(40), ..TextRenderOptions::default() }),
    ..Overrides::none()
};
let plan = Plan::for_workflow(&registry, text::WORKFLOW, &overrides)?;

// A plan is a list of stages in the order the runner will run them, and a
// pure function of the registry and the request.
assert!(plan.stage_names().iter().any(|name| name.as_str() == "flm.text.emit"));
assert!(plan.externals().is_empty()); // nothing to install for this one
# Ok::<(), Box<dyn std::error::Error>>(())
```

Reading a plan back: [`stage_names`](crate::core::plan::Plan::stage_names) (run order),
[`rewrite_names`](crate::core::plan::Plan::rewrite_names),
[`batch_names`](crate::core::plan::Plan::batch_names),
[`active_backends`](crate::core::plan::Plan::active_backends),
[`externals`](crate::core::plan::Plan::externals) /
[`external_names`](crate::core::plan::Plan::external_names) (the external tables to
install), [`warnings`](crate::core::plan::Plan::warnings),
[`excluded`](crate::core::plan::Plan::excluded) (the stages ruled out),
[`first_io_batch`](crate::core::plan::Plan::first_io_batch),
[`registry`](crate::core::plan::Plan::registry), [`request`](crate::core::plan::Plan::request),
[`render`](crate::core::plan::Plan::render) (five lines of text: rewrite stages, stage
order, the order of choice, the stages ruled out, external tables; for a report or a test
comparing plans). [`Plan::key`](crate::core::plan::Plan::key) is a
[`PlanKey`](crate::core::plan::PlanKey) computed over the registry digest and the whole
request except the workflow name: cache a built plan under it.

**Warnings.** [`PlanWarning`](crate::core::plan::PlanWarning): `IgnoredPreference`,
`PreferredRuleSetAside` and `PreferredRuleNotCandidate` are a preference the plan could
not honor, each naming the workflow the request was made from ("the plan request for
workflow `html` prefers …"), or saying "the plan request" for one built by hand;
`UncheckedOutput` is the output of the selected lowering rules of one feature on one
backend that nothing checks before the program reading it does
([`LoweringDecl::unchecked_output`](crate::core::lowering::LoweringDecl::unchecked_output)).
Every run of the plan reports each as a Warning at its start
(`flm.run.ignored-preference`, `flm.run.preferred-rule-set-aside`,
`flm.run.preferred-rule-not-candidate`, `flm.run.unchecked-output`). With
`math-passthrough`, every plan of `html`, `latex-rendered` and `latex-recomposed` holds an
`UncheckedOutput`, for its formulas; with `math-techxt`, every plan of `latex-rendered`
and `latex-recomposed`.

**Refusals.** [`PlanError`](crate::core::plan::PlanError): each variant holds the names
of the stages, capabilities, elements or other entities involved, and a variant about an
undeclared name also lists the declared ones (except `UnknownElement`). The variants:
`UnknownDemand`, `UnknownPreference`, `UnknownBackend`, `UnknownRule`, `UnknownElement`,
`UnknownWorkflow`, `UnknownOutput`, `RenderOption` (a render value the option type's
binding does not admit, or an option type no feature declares), `RegistryBoundRead` (a
stage reads a configuration type), `NeedsUndeclared`, `NeedsTableType`, `RewriteReads`,
`RewriteNeedsComputed`, `CollectionRequired`, `SelfExclusion`, `Unsatisfiable`,
`Contested`, `RuleContest`, `Cycle`, `ProducesCycle`, `TwoWriters`, `RewriteTie`,
`IoProductReadByPure`, `Incomplete`. Every variant `Plan::build` returns holds the
workflow the request was made from, and its message names it.

## 5. The run

A run is one execution of one plan over one document ([`Run`](crate::core::run::Run)); it
owns the frozen trees, the store and the diagnostics, so one document can be run several
times under different plans and no run sees another's results.

- [`Run::new`](crate::core::run::Run::new)`(Arc<Doc>, Arc<Plan>, `[`Externals`](crate::core::run::Externals)`)`
  → `Result<Run, RunError>` **executes nothing**: it checks the inputs and resolves every
  scheduled stage's option values, so every run that exists has sound inputs.
- [`Run::execute`](crate::core::run::Run::execute)`(&RunOptions)` →
  `Result<`[`RunOutcome`](crate::core::run::RunOutcome)`, RunError>` runs to the end;
  the outcome is `Finished`, `HaltedAtGate` (the error gate, below, stopped the run) or
  `Cancelled` (the build was cancelled through the progress sheet the run reports on,
  [`Run::with_progress`](crate::core::run::Run::with_progress)). [`Run::step`](crate::core::run::Run::step)`(&RunOptions)` →
  `Result<`[`Step`](crate::core::run::Step)`, RunError>` runs the rewrite stages on the
  first call, then one batch per call: `Step::More` while steps remain, `Step::Done` once
  the run is over. Stepping changes when the work happens, never
  what is computed. [`is_done`](crate::core::run::Run::is_done) says whether the run is
  over; [`outcome`](crate::core::run::Run::outcome) gives its `RunOutcome` once it is, and
  `None` before.

[`RunOptions`](crate::core::run::RunOptions) (pass the same value to every step):

| field | effect |
|---|---|
| `sequential` | all the work on one thread even when the cargo feature `parallel` is compiled in; identical results ([`RunOptions::sequential`](crate::core::run::RunOptions::sequential)`()` builds it) |
| `continue_with_errors` | run the `Io` stages past the error gate (below) |
| `max_diagnostics` | how many of the run's own diagnostics it keeps (the parse has its own limit); read from the **first** step's options only. Diagnostics past it are counted ([`any_recorded`](crate::core::run::DiagnosticsView::any_recorded), [`has_errors`](crate::core::run::DiagnosticsView::has_errors), [`error_count`](crate::core::run::DiagnosticsView::error_count) count them) but not kept |

### The error gate

A stage's [`Effect`](crate::core::stage::Effect) is `Compute` (it produces computed
data), `Check` (diagnostics only) or `Io` (effects outside the store, such as handing an
output's files to the program that writes them). The plan orders every `Io` stage after
every `Compute` and `Check` stage, and **a run that has recorded an Error diagnostic stops
before the first batch holding an `Io` stage**, unless `continue_with_errors` is set. So a
document with errors produces no effect outside the store.

**The standard features declare no `Io` stage**: the backends write their bytes into
tables, and writing files is the host's task. A plan built from the standard features
alone always runs to the end
([`Plan::first_io_batch`](crate::core::plan::Plan::first_io_batch) is `None`), so a
document full of errors still renders; check
[`has_errors`](crate::core::run::DiagnosticsView::has_errors) where errors must stop
something. The host package `std` of `flm-host` declares one `Io` stage,
`flm.write.files`.

### What a run refuses

A [`RunError`](crate::core::run::RunError) means the run was asked for something it
cannot do; a problem with the document's content is a diagnostic, never a `RunError`.
`Run::new` returns `DocumentOption` (an option value on the document whose option type's
binding does not admit the document), `MissingExternal`, `UnknownExternal`,
`NotExternal`, `ExternalType`, `StampMismatch` (see [External tables and two-run
composition](#external-tables-and-two-run-composition)) and `Store`. `step` and `execute` return
`Worker` (no worker thread could be created) and `Store` (an invariant of the store
failed: a defect in the runner or a stage).

A stage that panics propagates the panic out of `step` and `execute`. A panic is a defect
in the stage, never a way to report something about a document. The whole batch is
collected before anything is added to the store, so the store is unchanged and the run
has not advanced.

### Following a build: the progress sheet

[`crate::progress`] ([following a build](crate::guide::embedding#following-a-build)):

- [`Sheet::start`](crate::progress::Sheet::start)`(&root_kind, label, Observer,
  Option<Arc<dyn LogSink>>)` → `(Sheet, Task)`. `Sheet` (host's half, `Clone`, any
  thread): `snapshot()` → [`Snapshot`](crate::progress::Snapshot) (`cancelled`, `rows`
  in id order, serde), `cancel()`, `cancelled()`. The root `Task`: give
  `root.scope()` to the work, `root.finish()` at the end.
- Rows: id, parent, kind ([`Kind`](crate::progress::Kind): `Kind::new(name)`,
  `Kind::titled(name, title)`, consts), label, [`RowState`](crate::progress::RowState)
  (`Pending` → `Running` → `Finished`/`Failed`, or `Pending` → `NotRun`), optional
  [`Count`](crate::progress::Count) (`unit` `Items`/`Bytes`, `total` fixed at start,
  `done` only grows), `started`/`ended` times from the observer. Never removed.
- [`Observer`](crate::progress::Observer) `{ row_started, row_ended, checkpoint }`, each
  `Option<Box<…>>`: row functions get the `RowSnapshot` and return `Option<Duration>` (the
  time to store, from the host's origin; the sheet reads no clock); `checkpoint()` runs
  before each `Scope::cancelled()`. Called on the working thread, outside any lock; may
  snapshot and cancel; never called on a count's advance. A host with its own reading
  thread sets the row functions to its clock and no checkpoint.
- Estimate: [`estimate`](crate::progress::estimate)`(&Snapshot, &Expected, now) -> f64`
  (root's fraction 0..=1, weights = expected durations by kind name);
  [`measure`](crate::progress::measure)`(&Snapshot) -> Expected` (from a finished build
  with times); [`Expected::over`](crate::progress::Expected::over) lays the measured table
  over defaults. The estimate can decrease: a display keeps its highest value, and shows
  100% only once the root has finished.
- Depth: [`Scope::with_depth_limit`](crate::progress::Scope::with_depth_limit),
  [`Pending::with_depth_limit`](crate::progress::Pending::with_depth_limit) (0: no rows
  below); past it, handles declare nothing but still answer `cancelled()` and log,
  attributed to the nearest row.
- [`LogSink`](crate::progress::LogSink) `{ enabled(level), log(&LogRecord) }`
  ([`LogRecord`](crate::progress::LogRecord): `level` `Error`/`Warn`/`Info`, `source` (the
  row's kind name through a scope), `text`, `row`, `label`); independent of a sheet (the
  `flm-ext` engines take one for `console`).
- Cancel: `sheet.cancel()` from any thread or inside the observer → the run ends
  `RunOutcome::Cancelled`, the step under way discarded, rows running → `Failed`, no
  diagnostic.
- One-thread host (WebAssembly): display from `checkpoint` (own clock, snapshot,
  estimate, maybe cancel); in a browser run the library in a worker that posts each text
  to the page. A panic stops the module: no row ends `Failed`, the sheet is unreadable.

## 6. Reading results

- [`Run::table`](crate::core::run::Run::table)`(key)` →
  `Result<Option<&T>, `[`AccessError`](crate::core::data::store::AccessError)`>` reads one
  table through its typed key (a [`TableName`](crate::core::data::table::TableName)
  constant). Unlike a stage, a caller may read **any** table the registry declares.
  `Ok(None)`: nothing wrote it (no scheduled stage writes it, or its batch has not run).
  `Err`: the registry declares no table of that name, or the key's type is not the
  table's.
- [`frozen_tree`](crate::core::run::Run::frozen_tree)`(TreeId)`,
  [`frozen_trees`](crate::core::run::Run::frozen_trees): each tree as the last rewrite
  stage left it, the form every later stage reads ([frozen trees and computed
  data](crate::guide::concepts_overview#frozen-trees-and-computed-data)).
  [`tree_order`](crate::core::run::Run::tree_order): the order in which the run walks the
  trees.
- [`computed`](crate::core::run::Run::computed): the whole store, for walking tables whose
  types you do not know (with [`crate::dump::project`]).
- [`doc`](crate::core::run::Run::doc), [`plan`](crate::core::run::Run::plan),
  [`stamp`](crate::core::run::Run::stamp).

An [`ArtifactSet`](crate::core::artifact::ArtifactSet) holds the artifacts of one output;
[`entry`](crate::core::artifact::ArtifactSet::entry) is the entry artifact, the one that
represents the output as a whole; [`iter`](crate::core::artifact::ArtifactSet::iter)
lists them all. An [`Artifact`](crate::core::artifact::Artifact) has an `id`
([`ArtifactId`](crate::core::artifact::ArtifactId): owner and name, **never a path**),
`entry`, `media_type`, `data` (`ArtifactData`: `Bytes`, `FileRef`, or `Files(Vec<(RelPath,
Vec<u8>)>)` for a *folder artifact*, made with `Artifact::folder(id, files)`: the files of
a folder by their paths relative to it, `media_type` empty and `extension` `None`),
`extension` (the file extension its producer gives, without
the dot, set with `with_extension`: `sty` for the style file, the format's name for a
copied image (`png`, `jpeg`), the file's extension for a file a compose template ships;
`None` leaves it to the host's table over media types), `fixed_path` (a path the artifact
must keep, such as either LaTeX output's style file, which the document loads by name)
and `refs` (the other
artifacts this artifact's bytes link to, set on the entry artifact by the HTML and
recomposed LaTeX outputs);
[`as_bytes`](crate::core::artifact::Artifact::as_bytes) returns its bytes. A host that
writes files decides each path itself; [`ArtifactNamer`](crate::core::artifact::ArtifactNamer)
is the trait for that decision (see [Services the host
provides](#services-the-host-provides)). `core::artifact::media_type_of(extension)` gives
the media type of a file by its extension (`css` → `text/css`, `tex` and `sty` →
`application/x-latex`; `None` for an unknown one).

### Text output

| table | holds |
|---|---|
| [`TEXT_OUTPUT`](crate::stdlib::backends::text::TEXT_OUTPUT) (`flm.output.text`) | a [`TextOutput`](crate::stdlib::backends::text::TextOutput): the printed `text` and its position marks, `marks` ([`TextMark`](crate::stdlib::backends::text::TextMark): the byte and line/column range of the text each lowered node produced; [`slice`](crate::stdlib::backends::text::TextOutput::slice), [`marks_at`](crate::stdlib::backends::text::TextOutput::marks_at)); and its style marks, `styles` ([`TextStyleMark`](crate::stdlib::backends::text::TextStyleMark), [`TextStyle`](crate::stdlib::backends::text::TextStyle): the printed ranges whose content plain text cannot show, a heading's title (with its depth), the rule or `#` prefix of a heading, a span of bold, italic, emphasis, underline, monospace or small capitals, a link's content with its target, verbatim content, the frame of a table; recorded inside pieces too; [`style_slice`](crate::stdlib::backends::text::TextOutput::style_slice)). **Read it for text with positions, or to show the styles in a medium that can.** |
| [`TEXT_DOCUMENT`](crate::stdlib::backends::text::TEXT_DOCUMENT) (`flm.output.text.document`) | the artifact set; its entry artifact `text:document`, media type `text/plain` |

The `flm` program's profile `tty` (the extension `profile-tty`, written in Rust and
compiled into the program, which `-p tty` loads; its workflow `tty` extends `text`) is
the text for a terminal: an output `tty` (`tty:document`, `text/plain`, extension `ansi`)
whose `Compute` stage reads the projection of `flm.output.text` and writes the text with
SGR and OSC 8 escape sequences from the style marks; it lays nothing out
([Text for a terminal](crate::guide::command_line#text-for-a-terminal)). The workflow also
sets the graphics strategy `content` for `text` and a rule `tty.graphics.image` that draws
each block image with half-block characters from its row of `flm.graphics.content.text`;
the stage colors a drawing by finding its node through a position mark (`node_at` on
`flm.lowered.text`) and drawing the image again.

### HTML output

| table | holds |
|---|---|
| [`DOCUMENT`](crate::stdlib::backends::html::DOCUMENT) (`flm.output.html.document`) | the artifact set; its entry artifact `html:document` is a complete page, `text/html`, followed by the artifacts of `flm.output.html.artifacts` (copies of images) |
| [`OUTPUT_TABLE`](crate::stdlib::backends::html::OUTPUT_TABLE) (`flm.output.html`) | [`HtmlFragments`](crate::stdlib::backends::html::HtmlFragments): per tree an [`HtmlFragment`](crate::stdlib::backends::html::HtmlFragment), its HTML in `segments` (bytes, or another tree placed at that point; [`html()`](crate::stdlib::backends::html::HtmlFragment::html) joins the bytes) and `requires`, the identifiers of the style and script definitions it needs. **Read it to place the content in a page of your own.** |
| [`DEFINITIONS_TABLE`](crate::stdlib::backends::html::DEFINITIONS_TABLE) (`flm.html.definitions`) | [`ResolvedDefinitions`](crate::stdlib::backends::html::ResolvedDefinitions): every stylesheet and script the document requires, once each, the stylesheets first and then the scripts, each in the features' registration order; each row has `id`, `kind`, `owner` and `delivery` |
| [`REGIONS`](crate::stdlib::backends::html::REGIONS) (`flm.output.html.regions`) | the page's output regions ([`Regions`](crate::core::region::Regions)): the content, each placed piece, the stylesheets, the scripts, the title and the language, for a host that assembles the page through a template of its own ([output regions and the final stage](crate::guide::concepts_overview#output-regions-and-the-final-stage)) |

**Composing a page of your own.** Write the content from `flm.output.html`; from
`flm.html.definitions`, write the rows whose `kind` is
[`DefinitionKind::Style`](crate::stdlib::backends::html::DefinitionKind::Style) in the
head and those whose `kind` is `Script` at the end of the body, in the table's order
([`of_kind`](crate::stdlib::backends::html::ResolvedDefinitions::of_kind)). A row's
`delivery` is [`DefinitionOverride::Parts`](crate::stdlib::backends::html::DefinitionOverride::Parts)
(texts to write, addresses to load, files the host reads itself) or `ProvidedByHost`
(your own page or bundle supplies it). Leave out the row
[`flm.html.style.page`](crate::stdlib::backends::html::STYLE_PAGE), which styles the
backend's own page and not the content. To have the backend's own page use your
deliveries instead: the render option
[`HtmlRenderOptions::definitions`](crate::stdlib::backends::html::HtmlRenderOptions::definitions)
gives a delivery per definition before the table is written, so that the backend writes
the page with your deliveries; [`required_definitions`](crate::stdlib::backends::html::HtmlRenderOptions::required_definitions)
adds definitions every page must carry (a script of your program's own).

**What the page guarantees.** Every byte the HTML backend writes outside a mathematics
region is escaped or checked by the library; inside a mathematics region that
`math-passthrough` writes the author is trusted, since that feature does not read the
formula ([escaping, and the trust
guarantee](crate::guide::rendering#escaping-and-the-trust-guarantee)). A host that renders
mathematics written by authors it does not trust configures the formula renderer itself,
through `definitions` ([mathematics written by authors the host does not
trust](crate::guide::rendering#mathematics-written-by-authors-the-host-does-not-trust);
[`html::mathjax`](crate::stdlib::backends::html::mathjax)).

### Rendered LaTeX output

[`latex_rendered::DOCUMENT`](crate::stdlib::backends::latex_rendered::DOCUMENT)
(`flm.output.latex-rendered.document`) holds the document (`latex-rendered:document`,
media type `application/x-latex`, extension `tex`; under the backend's final stage a
complete `article` document) and always its style file (`latex-rendered:<stem>.<hash>`,
extension `sty`, its fixed path the one the namer gives, `<stem>.<hash>.sty` without a
namer, `paper-flmx.1a2b3c4d.sty` under the `flm` program's `-o paper.tex`;
`latex_rendered::style_artifact(&set)` finds it; a namer's path that is not a bare file
name ending in `.sty` is the Error `flm.latex-rendered.style-path`; also alone in
[`STYLE_FILE`](crate::stdlib::backends::latex_rendered::STYLE_FILE)), followed by the
image copies. The regions (`flm.output.latex-rendered.regions`) are `content`, one per
piece and `style-package`, the style file's package name, which a host's template loads
with `\usepackage{…}`. The render options are
[`LatexRenderedRenderOptions`](crate::stdlib::backends::latex_rendered::LatexRenderedRenderOptions)
(`style_stem`, `run_in_stop`, `skip_packages`, `extra_packages`, `extra_preamble`). A
template changes a look by registering LaTeX code under a role or kind token after the
style file is loaded ([the rendered LaTeX
contract](crate::guide::rendering#the-rendered-latex-contract)). Compile with `lualatex`
twice or `latexmk -lualatex`. The `flm` program's profile `pdf-latex` (the extension
`profile-pdf-latex`, written in Rust and compiled into the program, which `-p pdf-latex`
loads; its workflow `pdf-latex`) does it in one command: an output `pdf-latex` made
from `latex-rendered` (`OutputDef::made_from`),
whose `Io` stage reads the files of `latex-rendered` and runs `latexmk` through the host
service `programs` ([PDF through LaTeX](crate::guide::command_line#pdf-through-latex)).

### Recomposed LaTeX output

[`latex_recomposed::DOCUMENT`](crate::stdlib::backends::latex_recomposed::DOCUMENT)
(`flm.output.latex-recomposed.document`) holds the LaTeX document
(`latex-recomposed:document`) and, when the document needs one, its style file
(`latex-recomposed:<stem>.<hash>`, extension `sty`, its fixed path the one the namer gives,
`<stem>.<hash>.sty` without a namer; `latex_recomposed::style_artifact(&set)` finds it;
the package name is the path without `.sty`; a namer's path that is not a bare `NAME.sty`
is the Error `flm.latex-recomposed.style-path`; also alone in
[`STYLE_FILE`](crate::stdlib::backends::latex_recomposed::STYLE_FILE)), followed by
copies of images. The render options are
[`LatexRecomposedRenderOptions`](crate::stdlib::backends::latex_recomposed::LatexRecomposedRenderOptions)
([Rendering](crate::guide::rendering) describes the output).

### Diagnostics

A diagnostic reports a problem with a document: a severity (Error, Warning or Note), a
position, and a typed condition with its data; its message is written for a person and is
not a stable interface ([`FlmDiagnostic`](crate::lang::FlmDiagnostic);
[diagnostics](crate::guide::concepts_overview#diagnostics)). **Select diagnostics by
identifier** ([`identifier()`](crate::techy::error::Diagnostic::identifier), such as
`flm.refs.unresolved`) **and the condition's data, never by message text.**

[`Run::diagnostics`](crate::core::run::Run::diagnostics) →
[`DiagnosticsView`](crate::core::run::DiagnosticsView): each fragment's parse diagnostics
(in the order the walk reaches the fragments, once per fragment however many times it is
placed), then the diagnostics the stages produced.
[`pipeline_diagnostics`](crate::core::run::Run::pipeline_diagnostics) holds the stages'
alone. View methods: [`iter`](crate::core::run::DiagnosticsView::iter),
[`has_errors`](crate::core::run::DiagnosticsView::has_errors),
[`error_count`](crate::core::run::DiagnosticsView::error_count),
[`any_recorded`](crate::core::run::DiagnosticsView::any_recorded),
[`retained_len`](crate::core::run::DiagnosticsView::retained_len),
[`render_all`](crate::core::run::DiagnosticsView::render_all) (one per line, as a
command-line program prints them). `flm-host`'s report module lays a diagnostic out for a
person with its leading position (see [Reporting](#reporting)).

## Options

An option type is a plain Rust struct a feature declares, whose values are set on a
document or on a render request; its binding says which. **Its Rust type is its
identity**, so options need no names ([`crate::model::options`]). Values are held in an
[`OptionValues`](crate::model::options::OptionValues) collection
([`new`](crate::model::options::OptionValues::new),
[`with`](crate::model::options::OptionValues::with), [`set`](crate::model::options::OptionValues::set),
[`get`](crate::model::options::OptionValues::get),
[`merge_from`](crate::model::options::OptionValues::merge_from)), given at one of two
places:

- on the document: [`Doc::with_options`](crate::model::doc::Doc::with_options);
- on the render request: [`PlanRequest::render_options`](crate::core::plan::PlanRequest#structfield.render_options),
  or `Overrides::render_options` for a workflow.

A stage reads the render value if one is set, otherwise the document value, otherwise
the type's `Default`. The binding ([`Bind`](crate::model::options::Bind)) says which
places admit a value: `Document`, `Render`, `Either` (a render value then replaces the
document's for that render), or `Registry` (a feature's configuration: neither place
admits it). **A value at a place the binding does not admit is refused, never ignored**:
when the plan is built for a render value (`PlanError::RenderOption`), when the run is
constructed for a document value (`RunError::DocumentOption`), both holding an
[`OptionError`](crate::model::options::OptionError).
[`FlmRegistry::options`](crate::core::registry::FlmRegistry::options) lists every option
type of a registry with its binding.

The option types of the standard features, with their fields, are tabled on the entry page
([option types of the standard
features](crate::guide::ai_guide#option-types-of-the-standard-features)). The ones a host
sets most:

- [`TextRenderOptions`](crate::stdlib::backends::text::TextRenderOptions) (render):
  `width` (80; `None` never wraps), `tab_width`, `run_in_stop`, `link_addresses`,
  `link_addresses_when_shown`, `root_heading_level`, `table_frame` (`none` or `box`),
  `heading_rules` (`ascii` or `unicode`);
- [`HtmlRenderOptions`](crate::stdlib::backends::html::HtmlRenderOptions) (render):
  `title`, `lang`, `source_spans` (each element's source position written into the
  page), `run_in_stop`, `definitions`, `required_definitions`, `heading_tags`,
  `root_heading_level`;
- [`HtmlAllowedUrlSchemes`](crate::stdlib::backends::html::HtmlAllowedUrlSchemes) (render, set by
  Rust code only, not a settings key): the URL schemes the page's links and images may
  have, `Listed` (a list replacing the default `http`, `https`, `mailto`, `ftp`, `tel`)
  or `All`;
- [`RefsRenderOptions`](crate::stdlib::features::refs::RefsRenderOptions) (render): an
  unresolved reference is an Error (default) or a Warning;
- [`AnnotationsRenderOptions`](crate::stdlib::features::annotations::AnnotationsRenderOptions)
  (render): `hidden` produces a clean copy without annotations;
- [`PiecesRenderOptions`](crate::stdlib::backends::pieces::PiecesRenderOptions)
  (render): which pieces (the title, the list of footnotes, the bibliography) are placed;
- [`GraphicsRenderOptions`](crate::stdlib::features::graphics::GraphicsRenderOptions)
  (render): per output, how images are published and converted (see [Image
  services](#image-services));
- [`NumberingOptions`](crate::stdlib::features::numbering::NumberingOptions) (either,
  the only one): per counter, written with its full name (`flm.math.equation`), the
  counter it is numbered within and its value before it is first advanced ([the
  numbering guide](crate::guide::numbering));
- [`DocMeta`](crate::stdlib::features::metadata::DocMeta) (document): the document's
  metadata (see [The document's metadata](#the-documents-metadata)).

Whether something is numbered at all is a configuration (`HeadingsConfig::numbering`,
`MathConfig`, `ListsConfig`), and how a number looks is a counter style, replaced through
`NumberingConfig`; neither is an option.

## Services the host provides

A service is a value that can be shared across threads which one feature supplies for
another feature, or for the embedding program, to use; a service slot is the
declaration, by the feature that owns an interaction, of a dotted name, the type of the
values the slot takes and its arity ([`crate::core::services`]; [services and service
slots](crate::guide::concepts_overview#services-and-service-slots)). The arity
([`Arity`](crate::core::services::Arity)) is `Single` (one value; a second contribution
fails the registry build) or `OrderedMany` (any number: the values contributed with
`contribute_first` or `contribute_first_to` first, then the others, each group in
contribution order).

A host contributes with [`RegistryBuilder::contribute`](crate::core::registry::RegistryBuilder::contribute)
(the slot found by the value's type) or [`contribute_to`](crate::core::registry::RegistryBuilder::contribute_to)
(the slot given by name), passing an `Arc<dyn Trait>`:

```text
builder.contribute_to(INSPECTOR_SLOT, Arc::new(MyInspector) as Arc<dyn GraphicsInspector>);
```

**Services are fixed when the registry is built**: every run shares them, and no slot
receives a value during a run. Data that exists only after some work (an index built by
scanning a collection of documents) is an external table, not a service. The standard
features fill most of their slots themselves (the LaTeX package and definition-piece
slots, [`latex::packages::SLOT`](crate::stdlib::backends::latex::packages::SLOT) and
[`latex::preamble::SLOT`](crate::stdlib::backends::latex::preamble::SLOT), among them).
The slots a host fills are listed below. The library runs with each of them empty, but
under some render options an empty slot makes the publish stage report Errors:

| slot | value type | arity | without it |
|---|---|---|---|
| `flm.artifacts.namer` ([`NAMER_SLOT`](crate::stdlib::backends::lowering::NAMER_SLOT)) | `dyn` [`ArtifactNamer`](crate::core::artifact::ArtifactNamer) (`url_of(&ArtifactRef, &NamingScope) -> RelPath`: `ArtifactRef { id, entry, media_type, extension, output, folder }`, `folder` true for a folder artifact, named with no extension; `NamingScope { outputs: Vec<ScopedOutput { name, extension }> }`, the command's outputs, built by `NamingScope::of(registry, plan.demands())`, the same for every call of one run) | single | no artifact has a path before the host writes it; under the publish strategy `copy`, each image is reported as the Error `flm.graphics.needs-namer` and published by `reference` (the `graphics` feature's publish stage asks the namer for the path of an image it copies beside the output); the recomposed LaTeX names its style file `<stem>.<hash>.sty`; a compose template that ships files reports `flm.compose.template-failed` and makes no output |
| `flm.graphics.resolver` ([`RESOLVER_SLOT`](crate::stdlib::features::graphics::RESOLVER_SLOT)) | `dyn` [`GraphicsResolver`](crate::stdlib::features::graphics::GraphicsResolver) | single | each output writes an image's path as the author wrote it (strategy `reference`); under `copy`, `data-uri` or `content`, each image is reported as the Error `flm.graphics.needs-resolver` and published by `reference` |
| `flm.graphics.inspector` ([`INSPECTOR_SLOT`](crate::stdlib::features::graphics::INSPECTOR_SLOT)) | `dyn` [`GraphicsInspector`](crate::stdlib::features::graphics::GraphicsInspector) | single | no image size is known; an output writes the size the options give, or none and the Warning `flm.graphics.unknown-size` |
| `flm.graphics.converters` ([`CONVERTERS_SLOT`](crate::stdlib::features::graphics::CONVERTERS_SLOT)) | `dyn` [`GraphicsConverter`](crate::stdlib::features::graphics::GraphicsConverter) | any number, asked in order | no conversion; an image that a conversion rule asks to convert is reported as the Error `flm.graphics.no-converter` and published unconverted |
| `flm.citations.providers` ([`PROVIDERS_SLOT`](crate::stdlib::features::citations::PROVIDERS_SLOT)) | `dyn` [`CitationProvider`](crate::stdlib::features::citations::CitationProvider) | any number, asked in order | a document that cites a work reports the Error `flm.citations.no-provider` once, at its first citation, and each cited key as `flm.citations.no-entry` |

### Image services

The `graphics` feature reads no image file ([`graphics`](crate::stdlib::features::graphics)).

- **The resolver** receives the path as the author wrote it and the origin of the source
  that holds the image, and returns an [`ImageFile`](crate::stdlib::features::graphics::ImageFile):
  the file's identity in your own form (two images referring to one file have one
  identity), its format as a media type, and the address an output writes to reach the
  file where it is; it then reads the file's content whenever a stage asks. A path that
  begins with a scheme (`https://…`, `data:…`) is never handed to it. When it cannot
  find or read a file, the image is reported as an Error that gives the path.
- **The inspector** receives one [`GraphicsInspectRequest`](crate::stdlib::features::graphics::GraphicsInspectRequest)
  per image (the path, the origin, and with a resolver the file and its content) and
  returns what the file states
  ([`GraphicsInspectedInformation`](crate::stdlib::features::graphics::GraphicsInspectedInformation):
  vector or raster, pixel size, resolution, physical size) or a
  [`GraphicsInspectError`](crate::stdlib::features::graphics::GraphicsInspectError). It is
  asked again wherever content holding an image is shown a second time (for example a
  heading's title that a reference to the heading repeats), so a slow inspector should keep its results. A raster image with no
  stated resolution is taken at 96 dots per inch, with one Warning. The table
  `flm.graphics.info` ([`INFO`](crate::stdlib::features::graphics::INFO)) holds each
  image's record.
- **The converters** each have a name, say which pairs of formats they convert, and
  convert content.
- **Publishing**, with a resolver, follows the render option
  [`GraphicsRenderOptions`](crate::stdlib::features::graphics::GraphicsRenderOptions),
  given per output: the strategy `reference` (the default: the resolver's address),
  `copy` (a copy of the file becomes an artifact of the output, its path from the
  namer), `data-uri` (the content written into the address) or `content` (the
  resolver's address, the content kept in the table `flm.graphics.content.<backend>` for
  an output that renders images itself); under the last three, its conversion rules turn
  one format into another through the converters. [Images and how an output reaches
  them](crate::guide::rendering#images-and-how-an-output-reaches-them) describes what each
  output writes.

[Image services: resolver, inspector and
converters](crate::guide::embedding#image-services-resolver-inspector-and-converters)
holds a compiled example of an inspector.

### Citation providers

A document writes citation keys only; the providers give each cited work's citation
entry. They are asked in contribution order: the first receives, in one call, every
distinct key the document cites, in order of first appearance
([`CitationProvider::answer_all`](crate::stdlib::features::citations::CitationProvider::answer_all)`(&[CitationRequest], &ProviderCx) -> Vec<CitationResult>`);
each later provider receives the keys every earlier one declined. A provider that handles
one key at a time implements
[`answer`](crate::stdlib::features::citations::CitationProvider::answer)`(&CitationRequest, &ProviderCx) -> CitationResult`
alone (the default `answer_all` calls it per key); one that fetches many keys at once
implements `answer_all` too, returning one result per request in order (a count that
differs fails every key of the call,
[`ProviderFailed`](crate::stdlib::features::citations::ProviderFailed)).

A result is `Ok(Some(`[`CitationAnswer`](crate::stdlib::features::citations::CitationAnswer)`))`,
`Ok(None)` ("not mine"), or an error in the provider's words (reported; ends that key's
lookup). The answer holds the citation entry (inline content, from
[`ProviderCx::parse_entry`](crate::stdlib::features::citations::ProviderCx::parse_entry)
over FLM source or [`ProviderCx::build_entry`](crate::stdlib::features::citations::ProviderCx::build_entry)
over [`EntryPiece`](crate::stdlib::features::citations::EntryPiece)s), optionally an
identity ([`with_identity`](crate::stdlib::features::citations::CitationAnswer::with_identity): one spelling every key of the same work shares, so that those
keys are one cited work), and optionally warnings
([`with_warning`](crate::stdlib::features::citations::CitationAnswer::with_warning)),
each reported at the key's first citation. Every run asks the providers again (`check`
too), so a slow provider should keep its results. The table `flm.citations.entries`
([`ENTRIES`](crate::stdlib::features::citations::ENTRIES)) holds what they returned.
[The citation providers](crate::guide::embedding#the-citation-providers) holds a compiled
example of two providers.

Declaring service slots of your own: [AI guide: extending
FLM](crate::guide::ai_guide_extending).

## External tables and two-run composition

An external table is a table that no stage of the run writes: the caller installs its
value when the run is constructed ([`Externals`](crate::core::run::Externals)). It brings
data from outside one document to the stages of a run.

The standard use is a **corpus**: documents processed separately whose references to
each other must resolve. The `refs` feature declares the external table
[`EXTERNAL_INDEX`](crate::stdlib::features::refs::EXTERNAL_INDEX)
(`flm.refs.external-index`), an [`ExternalIndex`](crate::stdlib::features::refs::ExternalIndex)
mapping a label of *another* document to an
[`ExternalTarget`](crate::stdlib::features::refs::ExternalTarget) (the link target as
you want it written, and the content a reference to it shows). The resolve stage looks a
label up there only when no reference point of the document carries it; a run without the
table resolves no reference to another document. The procedure: run each document once;
read the labels it defines and their anchors from [`LABELS`](crate::stdlib::features::refs::LABELS)
(`flm.labels`) and [`ANCHORS`](crate::stdlib::features::refs::ANCHORS) (`flm.anchors`);
build the index ([`ExternalIndex::with`](crate::stdlib::features::refs::ExternalIndex::with));
run each document again with `Externals::none().install_unstamped(EXTERNAL_INDEX, index)`.

```rust
use flm_core::core::run::Externals;
use flm_core::core::data::table::{Table, TableName};

#[derive(Debug, Default)]
struct KnownLabels(Vec<String>);

impl Table for KnownLabels {
    fn absorb(&mut self, piece: KnownLabels) {
        self.0.extend(piece.0);
    }
}

const KNOWN: TableName<KnownLabels> = TableName::new("example.known-labels");

// Data that did not come from a run of this library: nothing checks it against
// the document, and the caller is responsible for it.
let externals = Externals::none()
    .install_unstamped(KNOWN, KnownLabels(vec!["thm:main".to_string()]));

assert_eq!(externals.len(), 1);
```

- [`Externals::install_unstamped`](crate::core::run::Externals::install_unstamped)`(key, value)`:
  data from elsewhere; nothing checks it.
- [`Externals::install`](crate::core::run::Externals::install)`(key, value, stamp)`: a
  value an earlier run of this library produced, with that run's
  [`Stamp`](crate::core::run::Stamp) (`digest`: the registry digest; `active_rewrites`:
  the rewrite stages that ran; `option_reads`: a digest of the option values they read).
  [`Run::new`](crate::core::run::Run::new) refuses a stamp that differs from its own
  (`RunError::StampMismatch`), so data computed under one reading of a document is never
  used under another without notice. Read a run's stamp with
  [`Run::stamp`](crate::core::run::Run::stamp).
- [`Plan::externals`](crate::core::plan::Plan::externals) lists what a plan needs, so you
  can check before constructing the run. `Run::new` refuses a missing value
  (`MissingExternal`), a table the registry does not declare (`UnknownExternal`), a table
  not declared external (`NotExternal`) and a value of the wrong type (`ExternalType`).

## The document's metadata

A document's metadata (its title, authors, date, and any other key its author writes
about it) is one option value bound on the document,
[`DocMeta`](crate::stdlib::features::metadata::DocMeta), declared by the `metadata` feature. The
title, the date and each author's name, affiliations and notes are FLM text
([`MetaText`](crate::stdlib::features::metadata::MetaText)), each with the place where it was
written when it was read from a file; an author
([`MetaAuthor`](crate::stdlib::features::metadata::MetaAuthor)) also has an optional email
address, a plain text, and other keys kept as data.

- Set it with `Doc::with_options(content, OptionValues::new().with(meta))`.
- From data (a map from key to value):
  [`DocMeta::from_data`](crate::stdlib::features::metadata::DocMeta::from_data)`(&Map, place_of)`
  → `Result<DocMeta, Vec<MetaDataError>>`: `title` and `date` one text each, `author` one
  author or a list, an author a text (the name) or a map (`name` required; `affiliation`
  and `note` one text or a list; `email`; other keys kept), a number or truth value read
  as the text it is written as, every other key kept as data
  ([`DocMeta::other`](crate::stdlib::features::metadata::DocMeta::other), never shown).
  `place_of` gives the place of each value when the host knows it (`|_| None` otherwise),
  by its [`MetaPath`](crate::stdlib::features::metadata::MetaPath) (`author.0.affiliation`);
  each error gives the path at fault.
- The title, the authors and the date are each a piece placed before the content
  (`flm.metadata.title`, `flm.metadata.author`, `flm.metadata.date`), placed by default; a render
  leaves any of them out through `PiecesRenderOptions`. A document with no metadata
  places none. The authors piece holds one `flm.metadata.author.entry` node per author,
  holding `flm.metadata.author.name`, `.affiliation` (numbered across the document,
  `AffiliationPayload`), `.email` (`EmailPayload`) and `.note` nodes; on the text, HTML and
  rendered-LaTeX outputs each author is a `flm.pres.lines`, one line per part in a span of
  the part's role; on the recomposed LaTeX, `name\thanks{note} \\ affiliation \\ email`,
  joined by `\and`.

[The document's metadata](crate::guide::embedding#the-documents-metadata) holds a compiled
example.

## Inspecting, debugging and testing

[`crate::dump`] renders what the library holds in memory for inspection and tests. It is
ordinary public interface behind no cargo feature, so your tests use the same functions
the library's tests use. It keeps two forms apart.

**The projection** ([`crate::dump::project`]): a one-way rendering of a tree, a node, a
table or a diagnostic as a [`SerialValue`](crate::techy::serialize::SerialValue), in a
vocabulary this library owns and keeps stable (`elem`, `chars`, `args`, `children`,
`body`, `payload`; spans as `line:col` on request). It never shows a parsing state or a
node identifier, and **cannot be read back**. Tests compare it; `flm dump --view compact`
prints it.

- [`project_tree`](crate::dump::project::project_tree),
  [`project_node`](crate::dump::project::project_node),
  [`project_nodes`](crate::dump::project::project_nodes),
  [`project_diagnostic`](crate::dump::project::project_diagnostic),
  [`project_diagnostics`](crate::dump::project::project_diagnostics),
  [`project_payload`](crate::dump::project::project_payload), each with
  `&`[`ProjectOptions`](crate::dump::project::ProjectOptions) (`spans` off, `payloads` on,
  `source_maps` off; [`with_spans`](crate::dump::project::ProjectOptions::with_spans),
  [`without_payloads`](crate::dump::project::ProjectOptions::without_payloads),
  [`with_source_maps`](crate::dump::project::ProjectOptions::with_source_maps)).
- A [`Projector`](crate::dump::project::Projector) serves any number of projections and
  keeps the line positions it computes. [`Projector::table`](crate::dump::project::Projector::table)`(name, &table, trees)`
  projects a table under its name; a [`Run`](crate::core::run::Run) is a
  [`TreeSource`](crate::dump::project::TreeSource), so a table's node references resolve
  through the run's frozen trees. For a table whose type you do not know (what
  `flm dump --table` does): take the value from `run.computed().get_erased(id)`, project
  it with the registry's `registry.table(id).def.ty.project(value, &projector.table_projection(&run))`,
  and put it under its name with
  [`project_table`](crate::dump::project::project_table)`(name, projection)`.
- [`to_json_pretty`](crate::dump::to_json_pretty), [`to_json`](crate::dump::json::to_json),
  [`from_json`](crate::dump::json::from_json).

```rust
use flm_core::dump::project::{ProjectOptions, project_tree};
use flm_core::dump::to_json_pretty;
# use std::sync::Arc;
# use flm_core::model::doc::FragmentLayout;
# use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
# use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
# use flm_core::stdlib::{self, StdlibConfig};
# let mut builder = RegistryBuilder::new();
# builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
# let registry = builder.build(&RegistryInputs::new())?;

let source = Arc::new(FlmSource::new("A \\emph{word}.").with_origin(FlmOrigin::none()));
let fragment = parse_fragment(
    registry.lang(), source, FragmentLayout::default(), &ParseOptions::default(),
)?;

let value = project_tree(fragment.tree(), &ProjectOptions::new());
let json = to_json_pretty(&value)?;
assert!(json.contains("flm.textstyle"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

**The wire form** ([`crate::dump::session`]): techy's serialization of a parse, made to
work for FLM by [`crate::lang::serialize`]: complete, verbose, and readable back into
objects with their sharing and identity intact.
[`writer`](crate::dump::session::writer)`(&BuiltLang)`,
[`reader`](crate::dump::session::reader) /
[`reader_with`](crate::dump::session::reader_with) (with payload readers, for trees that
hold presentation nodes) create the sessions;
[`write_fragment`](crate::dump::session::write_fragment),
[`read_fragment`](crate::dump::session::read_fragment),
[`write_tree`](crate::dump::session::write_tree),
[`read_tree`](crate::dump::session::read_tree) move objects through them;
[`segment_line`](crate::dump::json::segment_line) /
[`segment_from_line`](crate::dump::json::segment_from_line) write and read one segment as
one JSON line. A round trip keeps the structure, spans and sources, parsing states,
specifications and elements (by identity), diagnostics with their identifiers, data and
counts, and the fragment's layout. It does not keep node identifiers and tree tags (new
on reading), nor the Rust types of diagnostic conditions (read back as techy's generic
condition with the same identifier and data).

**Test helpers.**

- [`crate::dump::golden`]: [`assert_text_golden`](crate::dump::golden::assert_text_golden)`(path, &str)`,
  [`assert_json_golden`](crate::dump::golden::assert_json_golden)`(path, &SerialValue)`
  compare with a file, and rewrite it instead when the environment variable
  [`UPDATE_GOLDEN`](crate::dump::golden::UPDATE_GOLDEN) is `1`;
  [`check_text_golden`](crate::dump::golden::check_text_golden) and
  [`check_json_golden`](crate::dump::golden::check_json_golden) take the mode
  ([`GoldenMode`](crate::dump::golden::GoldenMode)) explicitly and return a `Result`
  instead of panicking.
- [`crate::dump::diag`]: [`expect`](crate::dump::diag::expect)`(diagnostics, identifier)`
  selects by identifier (panics, listing the identifiers present, when none matches) and
  narrows with [`at`](crate::dump::diag::DiagExpect::at) (covered text),
  [`at_line_col`](crate::dump::diag::DiagExpect::at_line_col),
  [`severity`](crate::dump::diag::DiagExpect::severity),
  [`frames`](crate::dump::diag::DiagExpect::frames),
  [`data`](crate::dump::diag::DiagExpect::data),
  [`count`](crate::dump::diag::DiagExpect::count),
  [`once`](crate::dump::diag::DiagExpect::once);
  [`expect_none`](crate::dump::diag::expect_none) asserts absence; the trait
  [`ExpectDiag`](crate::dump::diag::ExpectDiag) offers both as methods on a fragment's
  diagnostics, a slice or a vector.

## Threads, the `parallel` feature and WebAssembly

- `flm-core` has two cargo features, both **on by default**: `docs` keeps every
  documentation text of the declarations (off: every builder stores an empty text; no
  rendered document and no registry digest changes; the settings JSON Schemas lose their
  top-level descriptions; [the cargo feature
  docs](crate::guide::extending#dropping-the-documentation-texts-the-cargo-feature-docs)),
  and `parallel`. With `parallel`, the per-tree
  work of a run (the rewrite stages over each tree, and each stage of scope
  [`Scope::PerTree`](crate::core::stage::Scope::PerTree)) runs on a thread pool whose
  threads are created with `WORKER_STACK_SIZE` bytes of stack and marked as worker
  threads, so that work never creates a thread of its own.
- **The parallel and sequential paths produce identical results**: results computed in
  parallel are always collected in the same order. The tests run under both, and again
  with the pool restricted to one thread. Choose per run with
  [`RunOptions::sequential`](crate::core::run::RunOptions::sequential), or for the whole
  program by building without the feature.
- The sequential path is always compiled and is the only one under
  `--no-default-features`, the configuration of the **WebAssembly** build: continuous
  integration checks that `flm-core` without default features compiles for
  `wasm32-unknown-unknown`, and so does `flm-host` without its default feature `fs`.
  Nothing else changes there, since the processing performs no input or output.
- Where creating threads may fail, the parse entry points still try to create a worker
  thread when the caller is not on one, and report `flm.lang.worker-thread-unavailable`
  when refused. A host whose own thread has `WORKER_STACK_SIZE` bytes of stack can call
  [`mark_worker_thread`](crate::lang::guard::mark_worker_thread) once, and no thread is
  then created.

## The host library `flm-host`

`flm-host` is the host library shipped with the project: it finds and reads a document,
reads its settings, chooses its packages and workflow, prepares the registry and the plan,
runs `flm-core` and writes the outputs. It parses no command line, prints nothing and sets
no exit status; the `flm` program, built from the crate of the same name, is a command
line over it. Its items are documented in the crate `flm_host` of the same documentation;
this page writes their paths as code (a page of `flm_core` cannot link them). [The flm
command-line tool](crate::guide::command_line) describes the same behavior for a user of
the `flm` program.

### Prepare, then build

A program uses the host in two calls, both in `flm_host::session`:

```text
prepare(request: Request, files: Arc<dyn Files>, compiled: &[PackageFactory], progress: &Scope)
    -> Result<Prepared, PrepareError>
build(prepared: &Prepared, progress: &Scope) -> Result<Built, PrepareError>
```

- **`prepare`** does the work that depends on the request and the front matter, not on
  the content: it reads the input's front matter and the metadata there, finds the
  packages the document uses (`compiled` are the compiled packages your program carries
  besides `std` and `math-passthrough`, such as the factory of the
  package `shout` that the example `own_package` of `flm` writes), combines the settings,
  builds the registry, chooses the workflow and the output, and builds the plan.
- **`build`** reads the input again, parses its content with `assemble` (positions
  counted from the top of the file, front matter included) and the host's input resolver
  `flm_host::inclusion::Inclusion`, makes the document (with the document option values
  and the metadata) and executes one run. When the request gives a sink, the run's last
  stage, `flm.write.files` (an `Io` stage, after the error gate), gives the output's files
  their paths and hands them to the sink.

This example is the doctest of the `flm_host` crate root (shown here, compiled there):

```ignore
use std::sync::Arc;

use flm_core::progress::Scope;
use flm_host::files::MemoryFiles;
use flm_host::output::{MemorySink, Naming, Sink};
use flm_host::seed::MathChoice;
use flm_host::session::{Input, Request, build, prepare};

let files = MemoryFiles::new().with("paper.flm", "---\ntitle: Notes\n---\nHello \\emph{world}.\n");
let sink = Arc::new(MemorySink::new());
let mut request = Request::new(Input::File(String::from("paper.flm")));
// The host's math processor: the library's package `math-passthrough`.
request.default_math = MathChoice::processor("passthrough");
request.profile = Some(String::from("text"));
request.naming = Naming::File { name: String::from("paper.txt") };
request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);

// No progress is shown: the calls report under `Scope::none()`.
let prepared = prepare(request, Arc::new(files), &[], &Scope::none()).map_err(|error| format!("{error:?}"))?;
let built = build(&prepared, &Scope::none()).map_err(|error| format!("{error:?}"))?;
assert!(!built.has_errors());
assert_eq!(sink.files()["paper.txt"].bytes, b"Notes\n\nHello world.\n");
# Ok::<(), String>(())
```

**`Request`** (`Request::new(input)` sets every other field to its default):

| field | meaning (default) |
|---|---|
| `input` | `Input::File(String)` (a path, `/` between names on every system), or `Input::Text(String)` (named `<input>` in messages, `TEXT_INPUT_NAME`; its relative paths resolved against the current folder) |
| `format` | the one output of the workflow to produce (`html`, `text`, `latex-rendered`, `latex-recomposed`, or an output a package registers), which the workflow must deliver; `None` produces every output of the workflow |
| `profile` | the profile: the profile `NAME` uses the extension `profile-NAME` when one exists, after the document's other packages, and runs the workflow `NAME`; `None` runs the front matter's `default_profile`, else `html` (`DEFAULT_PROFILE`; below) |
| `default_initial_use` | the host's initial list, a `GivenList` (`packages`, and `source`: the words a message about one of its entries opens with, after "in "); the packages used before the front matter's `use` when neither the front matter's `initial_use` nor the request's `initial_use` gives a list (`[std]`, `DEFAULT_INITIAL_LIST`) |
| `initial_use` | `Option<GivenList>`, the command line's initial list (the `flm` program's `--initial-use`); it replaces the front matter's `initial_use`, which replaces `default_initial_use`. Each list is used exactly as given (`None`) |
| `default_math` | the host's math processor, a `flm_host::seed::MathChoice`: `Processor(NAME)` uses the package `math-NAME` (`MATH_PACKAGE_PREFIX`, `MathChoice::package()`), `NoProcessor` adds none; used when neither the front matter's `math` nor the request's `math` chooses one; the processor's package comes after the initial list's, whatever its source, and before `use`'s; a problem with it opens with `DEFAULT_MATH_PROCESSOR` (`NoProcessor`: a host that renders sets one; the `flm` program sets `mathjax`, or `passthrough` without the cargo features `js` and `bundled-extensions`, and documents this default as not guaranteed) |
| `math` | `Option<GivenMath>` (`choice`, and `source`: the words a message about its package opens with), the command line's math processor (the `flm` program's `--math`); it takes precedence over the front matter's `math`, which takes precedence over `default_math` (`None`) |
| `uses` | `Option<GivenList>`, the packages given on the command line (the `flm` program's `--use`), each named as a `use` entry: used after the front matter's `use`, as if written last there, and before the profile's package (`None`) |
| `options` | command-line settings, `Vec<CommandLineSetting>`, each `FEATURE.KEY=VALUE` with the words typed (`CommandLineSetting::new(assignment, label)`); the highest rank |
| `layout` | the top-level layout (`Option<FragmentLayout>`); when given, it replaces the front matter's `layout` (`None`: the front matter's, else the library's default) |
| `naming` | the naming scheme of the outputs' files (`Naming::Folder`; `Naming::File { name }`: see "Files, sinks and output names") |
| `output_folder` | `OutputFolder`: the folder the output is written to and the program's current folder; from them the host computes the address of a file the output refers to instead of holding it (`OutputFolder::default()`: the output is written to the current folder, and the program's current folder is not given) |
| `sink` | `Option<Arc<dyn Sink>>`, the receiver of the outputs' files, in one delivery (`None`: nothing is written; read the output from `Built::run`'s tables) |
| `continue_with_errors` | write the output although the document has errors (`false`); passed to `RunOptions::continue_with_errors` |
| `extension_folders` | the search path of extensions, `Vec<ExtensionFolder>` (`path`, `required`: a required folder that does not exist stops `prepare`) (empty) |
| `bundled` | `flm_host::bundled::Bundled`, the extensions compiled into the program: `Bundled::new(files)` (the table of files, `&'static [flm_ext::source::BundledExtension]`: folder `data`/`js`/`rust`, stem, files) and, cargo feature `native`, `.with_code(code)` (the table of code, `&'static [flm_ext::guest::NamedExtension]`: stem, `&CompiledExtension`); searched last; identity `bundled:FOLDER/STEM`; a row with code is given to the engine `native` with its manifest's `engine` set to `native` and `wasm` removed (empty tables: no extension) |
| `collections` | cargo feature `wasm`: `Vec<flm_ext::wasm::Collection>`, each read by the program with `Collection::read(label, bytes)` from a `.flmextensions.wasm` file; searched after the folders and before `bundled`, in order; identity `LABEL:FOLDER/STEM`; the engine `wasm` runs every row with code on one instance of the collection's module per thread, memory limit the sum over those rows (empty; the `flm` program loads none) |
| `log_sink` | `Option<Arc<dyn LogSink>>`, the log sink that `packages::loader` gives every engine for the extensions' `console` output, each line with the extension's name as its source (`None`: the engines keep the sinks their hosts were given; the default hosts have none) |
| `js_host` | cargo feature `js`: `flm_ext::quickjs::JsHost`, the settings of the engine instances of JavaScript extensions and the modules the program offers them (the default settings, no module) |
| `magick_program` | cargo feature `graphics`: the program the converter `magick` runs (`MagickConverter::DEFAULT_PROGRAM`); the `flm` program takes it from the environment variable `MAGICK` |
| `citations` | cargo feature `citations`: `flm_host::citations::CitationsHost` (`fetcher`: network access or `None`; `cache_folder`: the folder of the cache file `.flm-citations.jsonl` or `None`), neither by default; a fetch reports a row `flm.citations.fetch` (`flm_host::citations::FETCH_KIND`) per pass, counted in the keys to fetch, and an `Info` log line per request and per wait |
| `cache` | `flm_host::cache::CacheHost` (`manager`: the `cachette::Manager` that keeps the results of costly work; `document`: the text that identifies the document among those of the manager's root), `Manager::none()` and no document by default; see "Caches and the host services" below |
| `downloads` | `flm_host::download::Downloads`, what downloads the file at an `https` address: `Downloads::new(Arc<dyn Downloader>)` (trait `Downloader`, one method `get(url) -> Result<Response, String>`, `Response { bytes, media_type }`), none by default (`Downloads::default()`: every download refused with the words of `download::NO_NETWORK`, and the loader does not offer the service `download`); cargo feature `download`: `download::network_downloader()` (ureq; 60 s, 32 MiB, five redirects each `https`, the proxy variables `ALL_PROXY`, `HTTPS_PROXY`, `HTTP_PROXY`; a status outside 200..300 fails), which the `flm` program gives; clones share the five-minute record of failed addresses (`RETRY_AFTER`); the citations never use it |

`magick_program`, `citations` and `downloads` depend on the machine the program runs on, so the program
embedding the host gives them; they are never settings, so that no document can choose a
program for the host to start or reach the network on its own.

**`Prepared`**: `registry()`, `plan()`, `profile()`, `profile_package()` (`Some("profile-NAME")` when the profile's extension was found and used), `workflow()` (the profile's name), `outputs()` (the outputs produced, in the workflow's order; the first is the *leading output*; empty for `check`), `packages()` (each after
the packages it uses, each once, starting with the initial list's, then the math processor's, then `use`'s, then the request's `uses`, then the profile's package; a package of an extension as `<identity>#<package>`, the identity being the path of the extension's manifest), `factories()` (their factories), `seed()` (the
front matter as read, `flm_host::seed::Seed`), `files_read()`, `diagnostics()` (the
settings' Warnings `flm.host.unknown-feature` and the registry's build diagnostics), and
`is_current()`: whether what `prepare` made is still valid for the files as they are now
(the input's front matter unchanged, and every other path `prepare` read giving the same
result). A change below the front matter, or to a file only a build reads (an included
file, an image, a bibliography file), needs only a new `build`; any other change needs a
new `prepare`, which makes a new registry; the results kept in the caches of the request's
manager (`Request::cache`), such as converted images, serve the new registry's builds.

**`Built`**: `run` (the `Run`, whose tables hold each output's artifact set),
`diagnostics` (`prepare`'s, then the run's, in the order reported), `files_read`
(`prepare`'s reads, then the build's; each a `FileRead { path, found }`, paths where
nothing was found included), and `has_errors()`.

**`PrepareError`** `{ kind, problems, diagnostics, files_read }` stops the work before the document is
processed; a problem with the content is a diagnostic in `Built::diagnostics`. Each
`Problem` has `file`, `place` (line and column) and `detail`. `diagnostics` holds what
was found before the step that stopped and does not stop the work, in the order found, to
report with the problems (for `build`, `prepare`'s diagnostics among them). `kind` is a
`PrepareErrorKind`: `Input`, `FrontMatter`, `Packages`, `Settings`, `Registry`,
`Workflow`, `Template` (no compose template of the chosen name serves the output),
`Plan` (also: the feature `write` switched off while the request gives a sink), `Run` (a
failure of the program, never the document's errors), `Cancelled` (the build was
cancelled through the progress sheet; no problem). `files_read` lists every path read
up to the failure, so a program that rebuilds on change knows what to watch, including a
an extension listed and not written yet.

### Which profile runs, and its workflow

- The profile is `Request::profile`; without it, the front matter's `default_profile`;
  without that, `html` (`flm_host::session::DEFAULT_PROFILE`). The profile `NAME` runs the
  workflow `NAME`.
- Before the packages are resolved, `prepare` asks the catalog for `profile-NAME`
  (`PROFILE_PACKAGE_PREFIX`). Found: the entry is appended last, after the initial list,
  the math processor's package and `use` (source words "the profile `NAME`"; import-once
  if `use` lists it too). Absent everywhere (no compiled package, no folder of the search
  path, no bundled extension): nothing is added, nothing is reported. A failure (invalid
  manifest, both layouts, a load failure): the entry is appended and reported as any `use`
  entry, `PrepareErrorKind::Packages`. The probe's paths are among `files_read`, found or
  not, so a preview prepares again when `profile-NAME.yaml` appears.
- After the registry is built, the workflow `NAME` must be registered (by any package),
  else `PrepareErrorKind::Workflow`: "the package `profile-NAME` fails to define a workflow
  named `NAME`" when the package was used, else "profile `NAME` not found; ensure that the
  existing packages define a workflow named `NAME` or that an extension named
  `profile-NAME` is in scope; try perhaps `a`, `b`" (the registry's workflows, in its
  order; no suffix when it holds none). Placed at the value of `default_profile` when that
  gave the name, else nowhere. An unchosen `default_profile` is neither probed nor checked.
- `format` never chooses the profile or the workflow: it selects one of the formats (outputs) the workflow
  that runs provides, and a format this workflow does not provide is
  `PrepareErrorKind::Workflow`, the message listing the formats it provides.
- The messages state the condition and refer to no command-line option; a program's own
  help says how its options map onto `profile` and `format`.
- A request produces every output of the workflow, or the one `format` names; with
  `format`, the demands of the workflow's other outputs are dropped from the plan request.
  `compose` prepares each produced output's composer. The outputs must share one plan: a
  workflow listing `latex-recomposed` with `html`, `text` or `latex-rendered` is refused by the planner
  (`PrepareErrorKind::Plan`), since those need the tree laid out in paragraphs and the
  recomposed LaTeX the raw tree.

With the citations cargo feature, the host adds the workflows `bibtex` and `csl-json`.

### Files, sinks and output names

- **Paths** (`flm_host::files`): a path is text (`String`, `&str`) with `/` between names
  on every system; `\` is a character of a name. It is absolute when it begins with `/`
  or is `~` or begins with `~/` (on Windows, `/C:/x` or `//server/share/x`).
  `BuildFiles::resolve(base, path)` joins as text and removes `.` names only (`..` kept);
  `BuildFiles::folder_of(name)` is the text before the last `/`;
  `BuildFiles::resolve_within(folder, path)` refuses an absolute path and a `..` climbing
  above `folder`. `files::system::to_system` and `from_system` convert to and from the
  system's form (`C:\x` on Windows) where the host meets the system: `DiskFiles`, the
  output's writing, the watcher, the programs run; a program holding system paths (the
  command line, the environment, a temporary folder) converts them with `from_system`.
- **Reading** (`flm_host::files`): the trait `Files: Send + Sync` has
  `read(&str) -> Result<Vec<u8>, FileError>` and `entries(&str) -> Result<Vec<String>, FileError>`
  (names in a folder, in name order). `DiskFiles` reads the disk (cargo feature `fs`, on
  by default), converting each path with `to_system`: a path with no system's form
  answers `NotFound`, the reason in `FileError::invalid`; `MemoryFiles::new().with(path, text)` (or `insert(path, bytes)`) reads from
  memory, keyed by the path's text, for tests and for a host with no file system. Implement `Files` to read from
  elsewhere (a browser's storage, an archive). Every module of the host reads through a
  `BuildFiles`, which resolves a relative path against the folder of the file that writes
  it and records every path it looked for; a registry service reads through a
  `ServiceFiles`, to which each build attaches its own `BuildFiles`, so a service's reads
  appear on that build's list.
- **Naming** (`flm_host::output::Naming`, which implements the library's
  [`ArtifactNamer`](crate::core::artifact::ArtifactNamer) and is contributed to
  `flm.artifacts.namer` by the feature `write`): every file of a command shares one
  *stem*. `Naming::File { name }`: the stem is `name` less `.EXT` for the first output of
  the scope whose extension `EXT` ends it (`paper.html`, `paper.txt`, `paper` all give
  `paper` for the outputs `html` and `text`), else `name` as written (also when stripping
  leaves nothing, `.html`); matching is exact (case-sensitive). `Naming::Folder`: the stem
  is `index`. The entry artifact is `STEM.EXT` (the output's declared extension), every
  other artifact `STEM-NAME.EXT` (`NAME` = `ArtifactId::name`; `EXT` = the artifact's
  extension, else the media type's (`image/jpeg` → `jpg`), else none). The scheme does not
  read `ArtifactRef::output`. An artifact with a fixed path keeps it.
  The setting `write.output_names` (`flm_host::stdpkg::write::WriteRenderOptions`,
  `output_names: BTreeMap<String, String>`, `Bind::Render`, merged entry by entry, no
  default entries) maps an output's name to the path of its entry file, taken literally
  (no stem, no extension added or removed; a folder part allowed). The write stage wraps
  the slot's namer in `flm_host::output::EntryNames::new(&names, namer)`, which answers
  an entry artifact whose `ArtifactRef::output` is listed with the map's value and passes
  every other request to the wrapped namer; so a renamed output's other files keep the
  stem (their producers ask during the run with `output: None`). A key that is no output
  of the registry is the Error `flm.write.unknown-output` (`UnknownOutput`), and nothing
  is written; under `flm check` no write stage runs, so it is not reported there.
  `flm_host::output::disk_destination(path)` chooses a `DiskSink` and a `Naming` from a
  path a user gave (a trailing `/` or an existing folder: `Folder`).
- **Placing** (`flm_host::output::place(&RegisteredOutput, &ArtifactSet, &dyn ArtifactNamer,
  &NamingScope) -> Result<Vec<NamedFile>, Vec<OutputError>>`): one output's files; a
  folder artifact (`ArtifactData::Files`, named `STEM-NAME`) gives one `NamedFile` per
  file at `STEM-NAME/PATH`, each with the media type of its own extension
  (`application/octet-stream` for one `media_type_of` does not know); each path resolved
  as text (`.`, empty names and `..` with the name before it left out: `x/../b.css` is
  `b.css`; messages show the path as given); refuses an absolute path, a `..` climbing above
  the output folder, a path naming the folder itself, two files of the output at one
  resolved path, an artifact holding no bytes
  (`FileRef`), a folder artifact holding no file. The feature `write`'s stage places every demanded output (registry order);
  two outputs' files at one path are one file when bytes and media type are equal (listed
  under both outputs), else the Error `flm.write.failed`; any problem: nothing is handed
  to the sink.
- **Sinks** (`flm_host::output::Sink: Send + Sync`,
  `receive(&self, files: &[NamedFile], progress: &Scope) -> Result<(), Vec<OutputError>>`,
  under the scope of the write stage's row) receive every
  output's files in one delivery (`NamedFile { path, output, entry, media_type, bytes }`;
  a shared file appears once per output, same path and bytes). A sink that keeps files
  keeps one *record* per output in the delivery, named by that output's entry path (else
  its first file's path). `DiskSink::new(folder)` writes each distinct path once through a
  temporary folder, counting its files in a row `flm.output.files`
  (`flm_host::output::FILES_KIND`), and keeps each record as `.ENTRY.flm-written`; `MemorySink::new()`
  keeps the files (`files()` → `BTreeMap<String, NamedFile>`, a shared file once). Both
  remove, per output, the paths of its previous record that no file of the delivery
  holds (`stale(previous, files)`): a path the delivery writes is never removed; a
  delivery narrowed with `format` touches only its output's record, but a file it shared
  with another output and no longer writes is removed. `flm_host::output::write_file`
  writes a file that is not a document's output.
- **`OutputFolder`** (`OutputFolder::new(folder).with_current(current)`, both paths as
  text) computes the address, seen from the output folder, of a file the output refers to
  instead of holding it (an image under the graphics strategy `reference`).

Without the cargo feature `fs`, `flm-host` reads and writes no file and compiles for
WebAssembly: read from `MemoryFiles` (or your own `Files`) and write to `MemorySink`. Two
exceptions. Under the cargo feature `citations`, the citations provider reads and writes
its cache file in `CitationsHost::cache_folder` directly. The caches are read and written
by the manager of `Request::cache`, in the form the program chooses: a manager over a
folder on disk (`Manager::open_path`) reads and writes that folder itself, and a manager
in memory or over a file system of the program's own reads and writes no file.

### Front matter, settings sources and extensions

- **Front matter** (`flm_host::seed`, named after `Seed`, the value a front matter is read
  into): the block of YAML between a first line `---` and the next `---` line of a file;
  its root keys are the author's metadata (title, authors, date; read into `DocMeta`),
  and the reserved key `flm` holds the document's settings: `use` (packages),
  `initial_use` (the packages used before those of `use`, in place of the host's initial
  list, exactly as written), `math` (the math processor: a name `NAME` for the package
  `math-NAME`, used after the initial list and before `use`, or `false` for none; read by
  `read_math` into `Seed::math`), `features`
  (a map from a feature's name to its settings; `enabled: false` among them switches the
  feature off and keeps them, and `false` is the shorthand of `{enabled: false}`; the
  block step reads both into one `FeatureBlock { enabled, settings }`), `parsing`
  (`comments`, `comment_prefix`, `dollar_math`), `feature_order` (chains of features'
  names, `NAME < NAME < … < NAME`), `layout` (`blocks`, `inline`, `auto`),
  `default_profile` (a profile's name; read by `read_default_profile` into
  `Seed::default_profile`);
  an included file may set only `parsing` and `input_as` (`heading`, `label`,
  `numbered`: a heading input, with the root key `title`). [The front
  matter](crate::guide::command_line#the-front-matter).
- **Settings sources** (`flm_host::settings`), from the lowest rank to the highest: the
  program's defaults (`flm_host::stdpkg::defaults`), the packages of the extensions the
  document uses (one source per package: the block it writes), the front matter, the
  workflow (settings a package gives the workflow that runs, render keys only), the
  command line, combined by the library's
  [`cascade`](crate::core::settings::cascade). The highest-ranked source that writes a
  key gives its value, whole, unless the marks of the feature's declaration (on the
  schema's nodes, `x-flm-marks`: `merge-object-key-values`, `remove-key-when-value-is-false`,
  `set-to-default-with-null`, `path`, `markup`) say otherwise; a map marked
  `merge-object-key-values` (every collection of named things, and
  `compose.template_config` at its three levels) is combined key by key over its default
  entries; a value marked `path` receives the root of its origin
  (`flm_host::settings::Origin::resolve_path`: `doc:` for the front matter, `cwd:` for the
  command line, `ext+NAME:` for an extension's manifest, `std:` for the defaults; a
  one-file extension has no files, so a bare path in it names no file); a key written
  under `compose.template_config.OUTPUT.NAME` that the compose template does not declare
  is left out with the Warning `flm.host.unknown-setting`;
  between packages, a package wins over the packages it uses, and two packages of which
  neither uses the other are peers whose different values for one key are a problem. Every value
  keeps its origin, so a problem is reported where it is written. Whatever a source writes
  for a feature that no package in use has (a switch or any key) is accepted and read by
  nothing, with one Warning `flm.host.unknown-feature` for each source that writes it, at
  the place where it writes the feature (`flm_host::settings::warning_diagnostic`), also when the
  preparation stops (`PrepareError::diagnostics`). [Ranks of the
  settings](crate::guide::command_line#ranks-of-the-settings).
- **Extensions** (`flm_host::extensions`, `flm_host::packages`, the crate `flm_ext`): a
  YAML file `NAME.yaml`, or a folder `NAME/` holding `flmextension.yaml` and other files,
  found by name on the search path, that provides packages. The manifest's root keys are
  `extension` (`description`, `author`, `flm_version`, `engine`: `data` when absent or
  `js`, `js: {main}`) and `flm` (engine `data` only: `use`, `features`, `parsing`,
  `workflows`, `feature_order`; compose templates of a folder under `templates/OUTPUT/`, each the folder `NAME/` holding `flmtemplate.yaml` or the file `NAME.yaml`, its description giving its `name` and `output`). A `use` entry is
  `LOCATOR` or `LOCATOR#PACKAGE` (`_` when absent), never a path. A locator is looked up
  among the compiled packages first, then in the folders of `Request::extension_folders`
  in order (the `flm` program: `--extensions-dir` or `_flmextensions` beside the document,
  `FLMPATH`, `~/.flm/extensions`), then in the collections of `Request::collections`, then
  in the tables of `Request::bundled` (`flm_host::bundled::BundledSource`). The manifest,
  the files and the code of an extension always come from the same source, the one that
  answered its name: its folder, a collection, or the tables built into the program;
  files from two sources are never combined. The crate `flm-extensions` gives the tables
  of the shipped extensions, `EXTENSIONS` and `COMPILED_EXTENSIONS`: one cargo feature per
  extension, named after it, `all` for every one, none by default (the `flm` program's
  `bundled-extensions` turns on `all` and `flm-host/native`). [The extensions a program
  offers](crate::guide::embedding#the-extensions-a-program-offers).
  `flm_host::packages::loader` makes the preparation's
  `flm_ext::ExtensionLoader` over `flm_host::extensions::ExtensionFinder`, and
  `flm_host::session::document_packages` resolves a document's packages without
  preparing it. `ExtensionFinder::locators` lists the extensions installed on the search
  path (`NAME.yaml`, `NAME/flmextension.yaml`, and the same under a folder `@SCOPE`),
  without loading them and without recording the listing among the paths read;
  `flm_host::packages::suggestions` adds the compiled packages (first, in name order) and
  the stems of `Request::bundled` (last; not those of the collections);
  `Prepared::suggestions` and
  `DocumentPackages::suggestions` carry them, and `front_matter_schema(factories,
  suggestions)` and `extension_schema(factories, suggestions)` write them as `examples`
  of `use` and `initial_use` (and `NAME` of each `math-NAME` for `math`), never as an
  `enum`. [Extensions](crate::guide::command_line#extensions).

### The package `std`, and the program's defaults

The host's package `std` (`flm_host::stdpkg`, `PACKAGE`, `factory(parts)`) holds the
standard features (`flm_core::stdlib::add_features_from_settings`), then six features
of the host; it uses no other package and processes no mathematics. A document takes the
whole package or leaves it out of its initial list; one feature is left out by switching
it off. The host's features of `std`:

- `compose`: assembles an output's entry artifact from its output regions through a
  compose template (a text with named places for those regions), instead of the
  backend's final stage; `prepare` adds the preference for its stage and selects the
  pieces the template accepts. A template ships files and folders (`TemplateArtifact::File`,
  `TemplateArtifact::Folder`; `Template::from_data(.., text, files, folders, place)`
  reads them, and the text the description's `main` gives, through the caller's three readers, and
  `place(path)` gives each file's qualified path) and states the place of its text
  (`Template::source: Option<String>`, `TemplateArtifact::File::source`); the engine reads its
  includes when applied, a path without a root beside the file of the text that writes it, each
  read recorded on the build's list of files read.
  `TemplateEngine::render(template, context, shipper)` and `render_text(template, text,
  place, context, shipper)` receive a `&Arc<dyn TemplateShipper>`: `ship(ShippedFile {stem,
  extension, media_type, bytes})` writes the file beside the main file as the artifact
  `compose:<stem>-<digest>` (once per identity per application) and returns its path.
  The engine `minijinja` (`MinijinjaEngine::new(roots)`, the preparation's `Roots`) offers the
  statements `include`/`extends`/`import`, the functions
  `include_raw(path)`, `include(path, html=…, NAME=…)`,
  `eval(text, html=…, NAME=…)` and `include_artifact_url(path, media_type=…)`
  (a path with its root, `ext+NAME:`, `std:`, `doc:`, `cwd:`, or a bare path read beside the file
  that holds the call; `self:` the base folder of that file's root; a bare path or `self:` in a
  text given to `eval` is refused; `https://…` for `include_raw` and `include_artifact_url`
  only, through `Roots::read` and `Downloads::get`, its warning given to
  `TemplateShipper::warn`; `include` and the statements refuse an address, but
  `eval(include_raw("https://…"))` applies a downloaded text as its author wrote it; reads
  go through `Roots`, so a read of
  the document's folder or of an extension found in a folder is recorded and watched; no
  statement `macro` or `call`: minijinja is built without its feature `macros`, and
  `import` gives the variables a file sets), and the filter `escape_for(place)` (`html-style`,
  `html-script`, `html-comment`). An inner template of `include`/`eval` reads the
  application's values and the named values of its call and of the calls above it, not
  the outer text's own variables; `html` sets its escaping, by default the outer text's.
- `write`: the `Io` stage `flm.write.files`, the naming scheme, and the render-bound
  setting `output_names` (a name of its own for one output's main file).
- `preview`: the live preview's page script and stylesheet.
- `graphics-files` (cargo feature `graphics`): a resolver over a search path, an
  inspector for PNG, JPEG, GIF, WebP, SVG and PDF, and the converters `raster` and
  `magick`.
- `citations-standard` (cargo feature `citations`): a citation provider that fetches from
  the arXiv and doi.org through the crate `autocitefetch`, reads bibliography files, or
  takes a `manual:` key's own text, and formats entries with a citation style (a CSL
  file); it also adds the outputs and workflows `bibtex` and `csl-json`.
- `compose-standard`: the template engine `minijinja` and the program's own compose
  templates.

Any of them can be switched off in the settings (`flm: {features: {preview: false}}`).

The host also sets the `flm` program's own defaults (`flm_host::stdpkg::defaults`) below
every source a document writes, so a document rendered through `flm-host` can differ from
one rendered with the library's defaults: footnotes marked a, b, c with their list's
heading at level 1; figures captioned `Fig.` and tables `Tab.`, both numbered I, II, III;
the list of cited works at level 1; under the cargo feature `graphics`, `html` images
written into the page (`data-uri`, PDF converted to PNG at 192 dots per inch) and
`latex-rendered` and `latex-recomposed` conversions under `copy` (SVG to PDF, GIF to
PNG). [The program's defaults](crate::guide::command_line#the-programs-defaults).

### Reporting

`flm_host::report` returns text and prints nothing: `diagnostic(&FlmDiagnostic)` lays one
diagnostic out as lines (`error: <message>`, an `at` line with the *leading position*,
the place in a file the author wrote that the author can act on, `inside` lines for its
traceback, and an `in content made from it` line when the problem is in content the
library made); `report(&[FlmDiagnostic])` lays out every diagnostic, duplicates included,
with the counts of errors and warnings; `problem(&Problem)` lays out a `PrepareError`'s
problem, printed after its `diagnostics`; `Reported::of_diagnostic(&d, &offsets)` and `Reported::of_problem(&p)`
give what the live preview's stream carries: the record below, the byte range in the parsed content and an excerpt
of the source line.

`report::Record` is the record form, what the program's `--json-messages` writes, one
JSON object per record (`#[serde(tag = "kind")]`): `Record::of_diagnostic(&d, &offsets)`
(`Diagnostic { severity, id, data, message, place, frames, made_from }`, `id` and `data`
as the dump projection spells them), `Record::of_problem(&p)` (`Problem { severity,
message, place }`, an Error that stopped the work; a program writes its own error or a
warning that stops nothing as a `Record::Problem` with `place: None`),
`Record::summary(&diagnostics)` (`Summary { errors, warnings }`),
`Record::build(n, time)` (`Build`, the preview), `Record::Log { level, source, text }`
and `Record::Progress { fraction, text, row }`; the header is
`Record::meta()` (a `flm_core::dump::Meta`: `kind: "messages"`, `flm`, `format: 1`). A
`Place` is `file`, `line` (from 1, top of the file, front matter included), `column` (from
1, in bytes) and, only in a file's parsed content, `offset` (bytes from the top of the
file) and `end`; content the library made gives its `label` alone. The offsets come from
`ContentOffsets`, which `session::Built::content_offsets` holds for the input and every
included file; with `ContentOffsets::new()` no place gets an offset.

`Prepared` also exposes what a describer needs: `profile()`, `profile_package()`,
`workflow()`, `packages()`, `factories()`, `suggestions()` (what a `use` entry may name:
compiled packages, extensions on the search path, bundled stems), `manifest(identity)`
(each loaded extension's manifest), `front_matter_text()`, `settings_in_effect()` (per
feature, per key: the value and its origin, per entry of a merged key its origin, and
whether the feature is switched off), `document_options()`, `layout()`, `search_path()`.
`session::document_packages` reads only `use`, `initial_use`, `math` and
`default_profile`, so a front matter with a problem elsewhere still gives its packages.

### Progress in the host library

`prepare(…, progress: &Scope)` declares `flm.host.prepare` ("Preparing", parts
`flm.host.packages` "Loading packages", `flm.host.registry`, `flm.host.plan`);
`build(…, progress: &Scope)` declares `flm.host.parse` ("Reading document") then
`flm.host.run` ("Processing document", under which the run reports). Kinds are consts of
`flm_host::session`. A cancelled sheet makes either return `PrepareErrorKind::Cancelled`.
Layout used by the `flm` program and the live preview: root (the program's kind) → a row
`session::BUILD_KIND` (`flm.host.build`, no title) declared **before** `prepare`, started
after it, its scope given to `build`, finished after; `prepare` runs under the root's
scope. Declaring it in advance keeps the estimate from reaching 99% during `prepare`.
Other rows: `DiskSink` counts `flm.output.files` ("Writing files",
`output::FILES_KIND`); citations `flm.citations.fetch` ("Fetching citation data",
`citations::FETCH_KIND`, one per pass, total = keys to fetch) plus an `Info` log line per
request and wait; `programs` host service `flm.programs.run` ("Running program", label =
the program). `flm_host::report`: `expected_durations()` (the shipped `Expected` table),
`ShownFraction` (`estimate(&snapshot, &expected, now)` keeps the highest fraction),
`progress_line(&snapshot, fraction)` (`42% Converting graphics figure-3.pdf`: percent
floored, below 100 until the root finishes; the deepest running titled row, its label,
`done/total`), `Record::progress(&snapshot, fraction)`. The request's `log_sink`
receives the extensions' `console` lines.

### Caches and the host services

- **The manager.** `Request::cache` is a `flm_host::cache::CacheHost { manager, document }`.
  `manager` is a `cachette::Manager` (`flm_host::cache::cachette` re-exports the crate):
  `Manager::open_path` (a folder on disk; the `flm` program's), `Manager::new` (a file
  system the program supplies), `Manager::in_memory` (results kept for the process's
  life), `Manager::none()` (the default: keeps nothing). `document`: `Option<String>`,
  the text that identifies the document among all documents of the root (the `flm`
  program: the canonical absolute path of the input file; `None` for a text input);
  caches kept per document keep nothing without it. The manager's clock must read the
  system's time (`cachette::SystemClock`): the citation crate compares the stored expiry
  times with the system's clock.
- **Sessions.** `prepare` makes `flm_host::cache::ServiceCaches` from the request's
  `CacheHost` and gives it to the package `std` (`StdParts::cache`) and to
  `flm_host::packages::loader(finder, request, files, caches, document)`. `build`
  attaches to it: a values session opened during a build is shared by every caller of
  that build and closed when the build ends. Every preparation and build of one request
  (the live preview's included) share the caches. Owners open sessions through
  `ServiceCaches::open_values(&ValuesDeclaration, &Instance)` and `open_folder`.
- **The host's caches.** `flm.graphics-files.magick.converted`
  (`graphics::MagickConverter::CACHE`: converted images, `Files` with one blob, stable,
  90 days, shared; key = SHA-256 of the first line of `magick -version`, both formats,
  the options, the content); `flm.citations-standard.autocitefetch.items`
  (`citations::StandardProvider::CACHE`: the records of all documents, `Sqlite`,
  volatile, the crate's lifetimes, grace period unlimited; instance key
  `{keep_abstract, keep_reference}`; fills the document's `.flm-citations.jsonl`, never
  filled from it); an extension's caches `flm.ext.STEM.NAME` (instance key
  `{"extension": IDENTITY}`, plus `"document"` for the scope `document`);
  `flm.host.download.files` (`download::CACHE`: the downloaded files, `Sqlite` with a
  blob, volatile, seven days, grace unlimited, shared; key = SHA-256 of the address; a
  stale value is requested again and, when that fails, answered, with a warning past its
  hard expiry).
- **Cargo feature `sqlite`** (off in `flm-host`, on in `flm`): turns on
  `cachette/sqlite`. A cache declared with `ValuesEngine::Sqlite` uses SQLite only on a
  manager made by `open_path` in a program with the feature; otherwise the handle opens it
  with `Jsonl` (no blobs) or `Files` (blobs). A program without the feature that shares a
  root with `flm` gets sessions without a cache for the caches that SQLite holds there
  (cachette refuses a recorded engine that differs, and reports it): reuse is lost,
  results are not.
- **Failures.** A session's operations never fail (a failed read answers "not found", a
  failed write stores nothing); events go to the manager's `cachette::Reporter`, never to
  the diagnostics. Maintenance is the program's: `Manager::maintain` (the `flm` program
  calls it after `render` and `check`, and once at the start of `watch`);
  `Manager::instances` and `Manager::remove_instances` back `flm cache list` and `flm cache
  clean`.
- **Host services on the loader.** `packages::loader` offers `files`
  (`host_files::FilesService`) and `cache` (`host_cache::CacheService`: `get`, `put`,
  `remove` on a values cache the caller's package declares under `caches`; keys any JSON,
  hashed; blobs `{text}` or `{base64}`; `get` answers `freshness` and `refresh` for
  volatile values) always, and `programs` (`host_programs::ProgramsService`: `find`,
  `run`; `run`'s argument `cache` links declared folder caches into the run's temporary
  folder) when `Request::offer_programs` (cargo feature `fs`), and `download`
  (`host_download::DownloadService`: `get { url }` answering `{ content, media_type,
  warning? }`, protocol `flm_ext::host::download`) when `Request::downloads` gives a
  downloader. `Roots` (`flm_host::roots`, `Roots::new(extensions_by_name, std_files,
  files, document_folder)`, `.with_downloads(downloads, caches)`) reads every qualified path
  of a preparation for `files`, `minijinja` and the citations; `Roots::read` answers a
  `RootsFile { bytes, media_type, warning }`. A program on `flm-ext`
  alone offers only what it registers with `ExtensionLoader::with_host_service`; the
  shipped `math-mathjax` declares `cache`, so such a program registers a service named
  `cache` (a stand-in answering `null` to every call suffices) to load it, in either
  mode.

### The live preview

`flm_host::watch` holds the parts of the live preview that do not depend on a command
line. Guide: [Watching a document: the live
preview](crate::guide::command_line#watching-a-document-the-live-preview).

- **Loop** (cargo feature `watch`): `Watch::new(request, files, compiled:
  Vec<PackageFactory>)`, `.watching_files()` to watch the disk, `.handle()` for a
  `Handle`, `.run(|event| …)`. It builds the document, then builds again whenever a file
  the last build attempt read changes (found or not, also when the attempt stopped
  early) or the handle asks, one build at a time; it prepares again only when
  `Prepared::is_current` is false. Each build continues through the document's errors,
  writes source positions into the page, requires the preview's script and stylesheet
  and the HTML backend's stylesheet of the error marks (`watch::build_settings` lists
  the settings it adds), and writes no file. `Watch::new` keeps the output in memory and
  resets the request's `output_folder` to its default; the exports use the request as
  given.
- **Handle**: `stop()`, `build_again()`, `set_profile(name)` (the profile of the next
  builds; clears `format`; never written to the document) — each of these three first
  cancels the build under way, as a changed file does (an export is never cancelled, and
  asking for one cancels nothing); `progress()` → `Option<watch::BuildProgress>`
  (`sheet`, `expected`: the last handed-over build's `measure` over
  `expected_durations()`, `origin: Instant`; `None` between builds), `export_name(profile)` (the
  export's name `STEM.EXT`) and `export(ExportOrder)` (`watch::export`: a build as `flm
  render -o STEM.EXT` builds it, run by the loop between two builds;
  `ExportAction::SaveAsSibling` writes beside the document, `Download` returns the files;
  `ExportError` says why nothing was written; its `log()` gives the build's errors).
  Never call `export_name` or `export` from the function given to `run`.
- **Events**: a cancelled build hands no `Built` (it keeps the current output, adds the
  paths it read to the watched set; its number is used, so `update.build` can skip).
  `Building` (the files whose change caused the build), `Built` (a
  `flm_host::watch::Built`, not the session's `Built`: the update `update::Update` with
  the build number, what the page must do, the output regions and files that changed and
  the problems; the output now current; the profile; the last successful `Prepared`; the
  `snapshot::Snapshot`, with the problems and the processed document's headings, labels
  and constructs by place) or `Watcher` (a problem of the file watcher; the loop
  continues). `watch::update`, the comparison of two builds, performs no input or output
  and is compiled without the cargo feature.
- **Server**: `watch::server::Server::start(port, document, handle, editors, openings,
  settings, services)` serves the output from memory on the loopback address, the tools
  page at `/.flm-preview/` and the stream (with `progress {"text"}` messages, at most four
  a second, while a build runs) and requests of the watch client
  (`npm/flm-watchclient/PROTOCOL.md` in FLM's repository); `server.tell(&event)` for each
  event of the loop; `tools_url()` is the address to open. `editors`:
  `watch::editor::Editors::find(preferred, search_path, application_folders)`, then
  `.with_command(Option<EditorCommand>)`; `openings` receives each `Opening`;
  `settings`: `watch::settings::DocumentSettings::on_disk(front_matter_schema,
  document)` (front matter edited in place by `watch::front_matter::edit_front_matter`).
- **Services** (`watch::services::Services`, each method with a default that gives
  nothing; `NoServices` gives none): `help()` (the trait `Help`: `index`, `page`,
  `construct_page`, for the last successful `Prepared`), `export_command(profile, name,
  continue_with_errors)` (the `flm render` line shown), `open_file(path)` (the system's
  default application, for an export saved), `home_folder()` (where `.flm/watch.json`,
  the viewer settings of `watch::viewer`, is read and written; `None`: kept in memory).
  The server calls them on its request threads.
- A program that updates pages its own way follows [a page updated in
  place](crate::guide::rendering#a-page-updated-in-place).

## Describing an environment: `flm-info`

`flm-info` (crate documentation `flm_info`) answers questions about an *environment*: a
registry, an optional plan and the package factories. Dependencies: `flm-core` without
default features, `serde`, `serde_json`, `schemars`; minimum Rust 1.86, builds for
WebAssembly. Cargo feature `flm-host` adds the host layer (needs 1.88, like `flm-host`).
Keep `flm-core`'s feature `docs` on in your own dependency to see documentation texts.

- **Queries** (`flm_info::env`): `Environment::new(registry: Arc<FlmRegistry>, plan:
  Option<Arc<Plan>>, factories: Vec<PackageFactory>)`; then `features()`, `commands(feature)`,
  `command(CallableKind, name) -> Vec<CommandView>` (every declaration of the name:
  signature, arguments with kind, name, role, layout, mode, docs; tack-on fields; owner;
  modes; element), `settings(feature) -> Option<SettingsView>` (keys from the JSON Schema),
  `workflows()`, `workflow(name)` (outputs, settings, and `plan` when the plan was built
  from it), `namespaces()` (every namespace with its names, `argument-role` included).
  `summary_of(text)` gives a text's first paragraph.
- **Description** (`flm_info::describe`): every view is `Serialize + JsonSchema`;
  `describe::schema()` is the schema of all views; header type `describe::Meta`
  (= `flm_core::dump::Meta`: `kind`, `flm` version, integer `format`). Rules: absent key
  for nothing to show, flags written only when `true`, names as dotted strings, commands
  without the backslash, lowercase enumerated words, registry order.
- **Pages** (`flm_info::pages`): `Tree::new(&env, program: Option<ProgramInfo>)`;
  `tree.page(path)`, `tree.lookup(name)` (commands with or without backslash, packages,
  features, settings keys, workflows, any registered name), `tree.walk()`;
  `page.to_json(recursive)` (`path`, `kind`, `title`, `summary`, `body`, `children`);
  `Output` (`{meta: {kind: "info", …}, pages}`); `source(&page, &SourceOptions)` writes
  FLM using `flmdoc` with `math: false` (render it with the host and the profile `text`;
  the crate renders nothing). `texts(&page)` lists the documentation texts the source
  embeds, each with its `TextSource` (declaring feature and package, or where a text
  written as data was written); a text that does not parse goes into
  `SourceOptions::plain_texts` and is then written as plain text. Paths:
  `packages/`, `features/NAME[/macros|environments|specials[/NAME]]`,
  `features/NAME/scopedpackages/PKG/…`, `settings/FEATURE`, `settings/parsing`,
  `settings/front-matter` (feature `flm-host`), `workflows/NAME`,
  `namespaces/NS[/NAME]`, `program/`, `program/paths`, `program/extensions` (only with a
  `ProgramInfo`, which the program fills).
- **Host layer** (feature `flm-host`, module `flm_info::host`):
  `Environment::of_prepared(&Prepared)`, `Environment::of_request(request, files,
  compiled) -> Result<Environment, PrepareError>` (a request over an empty text: the
  environment of a document that specifies nothing); `HostInfo`: profile, math processor,
  each package's origin (compiled, or the extension with its manifest's description,
  author and engine), settings in effect with origins, front matter, search path.
- **Dump engine** (`flm_info::dump`): see the next section.

## The `flm dump` command line

`flm dump` processes one file with `flm-core` alone and prints, as JSON on standard
output, one view of what the run holds. The program's module `flm::dump` parses the
command line into a `flm_info::dump::Request`, reads the file and prints; the engine is
`flm_info::dump::dump(&request, text, origin, &mut print)`, which a program may call
itself. It builds
its registry from the standard features
([`stdlib::add_features`](crate::stdlib::add_features)) under the default
configuration with `RegistryInputs::new()`, every time: no configuration file, no front
matter, no input resolver (an `\input` reports `flm.input.unavailable`), and it writes no
file. It runs on a worker thread it creates once. The commands `flm render` and `flm
check` go through `flm-host` instead. Full treatment: [Inspecting the processing with
`flm dump`](crate::guide::command_line#inspecting-the-processing-with-flm-dump).

```text
flm dump FILE [--at parsed|frozen|lowered:<backend>] [--view compact|wire]
              [--table NAME | --output NAME] [--workflow NAME] [--width N]
```

| option | meaning |
|---|---|
| `--at parsed` | the tree as the parser produced it. **Default**; runs no workflow |
| `--at frozen` | the tree after the plan's rewrite stages |
| `--at lowered:<backend>` | the presentation tree that backend built (`text`, or `latex-rendered`, `latex-recomposed` or `html` under the workflow of that name); `lowered:text` is the tree the text output's position marks point into |
| `--table NAME` | one computed table, under its name, as a projection |
| `--output NAME` | an output's artifact set, under the name of its table; bytes as `{"$bytes": "<base64>"}` |
| `--view compact` | the projection, indented. **Default** |
| `--view wire` | the wire form, one JSON line; under `--at parsed` it writes the whole fragment, recorded as the segment's main entry |
| `--workflow NAME` | the workflow to plan and run. Default `text` |
| `--width N` | the text backend's line width (default 80) |

- The options may come in any order, before or after `FILE`; exactly one file.
- `--table` and `--output` exclude each other and either replaces `--at`.
- `--workflow` and `--width` need a run, and `--at parsed` performs none, so either one
  beside `--at parsed` (including by default) is a **usage error**, never silently
  ignored.
- An unknown table or output fails and the message lists every one that exists; an
  unknown backend fails listing the backends of the plan that ran. A table or output the
  chosen workflow does not write (`--table flm.labels.safe` under the default `text`)
  fails with a message that gives that workflow and points at `--workflow`.
- Every diagnostic (registry build, parse, run) goes to standard error, so standard output
  holds the requested JSON alone. Diagnostics do not decide the exit status.
- Exit status: `0` printed; `1` usage error, unreadable file, registry, plan or run that
  cannot be built or fails, a name that matches nothing, or a table or output the
  workflow does not write; `2` the tree asked for does not exist (the parse produced none,
  a rewrite stage produced none, or the workflow did not lower the tree for that backend).
- To read rendered text rather than base64, ask for
  [`flm.output.text`](crate::stdlib::backends::text::TEXT_OUTPUT) with `--table`.

## Pitfalls

- Build the registry and each plan once and reuse them; do not rebuild per document.
- Give each source a real [`FlmOrigin`](crate::lang::FlmOrigin): an empty origin loses
  the key that diagnostics, include-cycle detection and your own tables use.
- A successful parse is not a clean one: check [`Fragment::diags`](crate::model::doc::Fragment::diags)
  as well as the `Result` (and `Run::diagnostics`, which includes them).
- Report [`build_diagnostics`](crate::core::registry::FlmRegistry::build_diagnostics)
  once per registry build: a successful build can carry Errors.
- Match diagnostics on their identifier and data, never on message text.
- A document full of errors still renders with the standard features alone (no `Io` stage, so no
  error gate): check `has_errors` where errors must stop something.
- Wrap your own tree traversals in [`on_worker_thread`](crate::lang::guard::on_worker_thread):
  they carry no recursion guard. Call `mark_worker_thread` only on a thread with
  `WORKER_STACK_SIZE` bytes of stack.
- `max_diagnostics` is read from the first step's options only.
- Set a render-bound option in `render_options` and a document-bound one on the document:
  the wrong place is refused, not ignored.
- The projection cannot be read back; use [`crate::dump::session`] for a round trip.
- `flm-core` turns on `serde_json`'s `preserve_order` for your whole program.
- With `flm-host`, check `Prepared::is_current` before reusing a preparation, and keep
  machine-dependent values (`magick_program`, `CitationsHost`) out of settings: they go in
  the `Request`.
- `flm-host`'s defaults are not the library's (footnote marks, float captions and numbers,
  image publishing): a document can render differently through the two.

## See also

- [AI guide](crate::guide::ai_guide): vocabulary, the feature map, the option types and
  tables of the standard features, recipes.
- [AI guide: pipeline](crate::guide::ai_guide_pipeline): how a plan is derived and a run
  executes.
- [AI guide: document model](crate::guide::ai_guide_document_model): trees, elements,
  addresses, computed data.
- [AI guide: extending FLM](crate::guide::ai_guide_extending): writing features, stages,
  lowering rules, service slots.
- [Embedding flm-core](crate::guide::embedding): the same ground for a person, with
  compiled examples of each configuration and service.
- [The flm command-line tool](crate::guide::command_line): the `flm` program, front
  matter, extensions, settings ranks, the preview, `flm dump`.
- [Rendering](crate::guide::rendering): what each backend writes.
- [Panics](crate::guide::panics): every public item that can panic.
