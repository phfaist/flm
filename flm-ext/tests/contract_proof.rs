//! Plan A, A22: the compile-only proof of the contract, reduced to what the
//! real code does not cover.
//!
//! The binding's half of the proof (an `Expand::expand` and a `Stage::run`
//! lending their arguments into one `CallTable<'c>`, A18) is the JSON double's
//! (`tests/common/json_double.rs`), and the dispatcher's half (a child node, a
//! table piece, a variant, a template and bindings lent back; nodes passed to
//! `chars_text` through `clone_box`) is `adapter::dispatch`, both driven by
//! `tests/boundary.rs`. What stays is a property of the Rust API alone: case
//! 1's hook, written in Rust, keeps relatives beyond the borrow of the node it
//! asked them on (A9, A16).
//!
//! E2 (plan A, section 4) adds the first and the last sketch of its proof over
//! the real types: a memory source's extension resolving through the loader in
//! a catalog, and a host's template engine and template reaching the compose
//! slots beside the library's.
//!
//! E6 adds one consumer of each new façade (a citation provider's context,
//! the entries it makes, a template module's context): each answers values
//! that own what they point at, which the consumers keep beyond the borrow of
//! the context they asked.
//!
//! E4 (its plan, section 6.5) adds the six items of its proof, in the module
//! `e4`: the durable handles, the handler aspects and a point read back from
//! data.
//!
//! The lowering façade needs no sketch of its
//! own: a rule's body lending its node, its `LowerCx` and its `Lowerer` into
//! one `CallTable<'c>` is the JSON double's `JsonLower`, and the dispatcher's
//! half (staged nodes, a node copied out of the table before the lowerer and
//! the context are taken together, a table read lent as an owned value) is
//! `adapter::dispatch`, both driven by `tests/lowering.rs`.
//!
//! E5 adds the compile-only proof of the restage façade's contract: a rewrite
//! stage written in Rust against the façade traits alone, written before the
//! adapter implemented them, so that the compiler checked the shapes before
//! an adapter was built on them. It is never run and asserts nothing; the
//! rewrite stages that run are in `tests/rewrites.rs`.

mod common;

use std::sync::Arc;

use flm_ext::api::lang::mint::ArgDef;
use flm_ext::api::lang::rewrites::userdefs::{Definition, Expand, ExpandCx, Expander, Expansion, UserDefinition};
use flm_ext::api::lang::text::chars_text;
use flm_ext::api::techy::error::Diagnostic;
use flm_ext::api::techy::node::Node;
use flm_ext::ExtError;

/// Case 1's hook (`\shout`) in Rust.
struct Shout;

impl Expand for Shout {
    fn expand<'c>(
        &self,
        node: &'c dyn Node<'c>,
        cx: &'c mut dyn ExpandCx,
        xp: &'c mut dyn Expander,
    ) -> Result<Expansion, ExtError> {
        // Relatives asked on a temporary parent outlive it.
        let nodes = {
            let parent = node.parent();
            let _ = parent.map(|parent| parent.children());
            node.argument_content_nodes_named("text")?.unwrap_or_default()
        };
        match chars_text(&nodes) {
            Some(text) => Ok(Expansion::Emit(vec![cx.stage_chars(&text.to_uppercase())?])),
            None => {
                let diagnostic = Diagnostic::error("wc.shout.not-characters", "`\\shout` takes plain text").at(node);
                Ok(Expansion::Emit(xp.recover(cx, node, diagnostic)?))
            }
        }
    }
}

/// `\shout{text}`, expanded by `hook`, in the package `wc`.
fn shouting(hook: Arc<dyn Expand>) -> Arc<flm_core::core::registry::FlmRegistry> {
    let shout = UserDefinition::new(Definition::macro_("shout", vec![ArgDef::mandatory("text")]).content("<#1>"));
    common::registry_with([common::double::defining("wc", vec![(shout, Some(hook))]).package()])
}

#[test]
fn a22_case_one_s_hook_keeps_relatives_beyond_their_borrow() {
    let registry = shouting(Arc::new(Shout));
    let (text, diagnostics) = common::text_of(&registry, "A \\shout{hey}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A HEY.\n");
    let (text, diagnostics) = common::text_of(&registry, "A \\shout{\\emph{hey}}.");
    assert_eq!(diagnostics, ["wc.shout.not-characters: `\\shout` takes plain text"]);
    assert_eq!(text, "A \\shout{\\emph{hey}}.\n");
}

// --- E2 -----------------------------------------------------------------------------------

/// Section 4's first sketch: a memory source answering an extension whose files
/// are a map, a loader resolving an entry through it in a catalog, a factory;
/// the factory name and both entry forms answer one factory, and the catalog
/// answers what the factories reach.
fn e2_consumer() -> Result<(), Vec<flm_core::core::registry::ResolveError>> {
    use flm_core::core::registry::{PackageCatalog, PackageResolver, ResolveError};
    use flm_ext::{ExtensionLoader, LoadedBlock, MemoryFiles, MemorySource};
    use serde_json::json;

    let source = MemorySource::new().with(
        "myext",
        "_flmextensions/myext.yaml",
        json!({ "extension": { "description": "Mine" }, "flm": { "use": ["stdcore"] } }),
        MemoryFiles::new().with_value(
            "templates/html/fancy.yaml",
            json!({ "output": "html", "engine": "content-only", "accepts": "all" }),
        ),
    );
    let loader = Arc::new(ExtensionLoader::new(vec![Arc::new(source)]));
    let mut catalog = PackageCatalog::new();
    catalog.add_compiled(flm_core::stdcore::factory().map_err(|e| vec![ResolveError::new(e.to_string())])?);
    catalog.add_resolver(Arc::clone(&loader) as Arc<dyn PackageResolver>);
    let factory = catalog.resolve("myext")?.expect("found");
    assert_eq!(factory.name().as_str(), "_flmextensions/myext.yaml#_");
    assert_eq!(catalog.resolve("myext#_")?.expect("found").name(), factory.name());
    assert_eq!(catalog.resolve("_flmextensions/myext.yaml#_")?.expect("found").name(), factory.name());
    let factories = catalog.resolve_all(&["myext"]).map_err(|_| Vec::new())?;
    assert_eq!(factories.len(), 2);
    let _reach: flm_core::core::settings::cascade::Reach = catalog.reach(&factories);
    let blocks: Vec<LoadedBlock> = loader.blocks();
    assert_eq!(blocks.len(), 1);
    Ok(())
}

#[test]
fn e2_a_memory_source_resolves_through_the_loader() {
    e2_consumer().expect("the sketch runs");
}

/// A host's template engine.
struct HostEngine;

impl flm_core::stdcore::features::compose::TemplateEngine for HostEngine {
    fn name(&self) -> &str {
        "host"
    }

    fn render(
        &self,
        template: &flm_core::stdcore::features::compose::Template,
        _: &flm_core::stdcore::features::compose::TemplateContext,
    ) -> Result<String, String> {
        Ok(template.text.clone())
    }
}

/// A host's feature contributing its engine and its templates, as `std`'s
/// `compose-standard` does.
struct ComposeStandard {
    templates: Vec<flm_core::stdcore::features::compose::Template>,
}

impl flm_core::core::registry::FlmFeature for ComposeStandard {
    fn info(&self) -> flm_core::core::registry::FeatureInfo {
        flm_core::core::registry::FeatureInfo::new("compose-standard", "A host's templates")
    }

    fn register(
        &self,
        b: &mut flm_core::core::registry::FeatureBuilder<'_>,
    ) -> Result<(), flm_core::core::registry::BuildError> {
        use flm_core::stdcore::features::compose::{Template, TemplateEngine, ENGINES_SLOT, TEMPLATES_SLOT};
        b.peer_slot::<dyn TemplateEngine>(ENGINES_SLOT, Arc::new(HostEngine) as Arc<dyn TemplateEngine>)?;
        for template in &self.templates {
            b.peer_slot::<Template>(TEMPLATES_SLOT, Arc::new(template.clone()))?;
        }
        Ok(())
    }
}

#[test]
fn e2_a_host_engine_and_template_reach_the_compose_slots() {
    use flm_core::core::registry::{build_registry, FlmFeature};
    use flm_core::stdcore::features::compose::{ComposeFeature, Template, TemplateEngine, ENGINES_SLOT, TEMPLATES_SLOT};
    use flm_ext::{ExtensionFiles, MemoryFiles};
    use serde_json::json;

    // The text and the files a description lists are read through the extension's files.
    let files = MemoryFiles::new().with_text("simple.html", "<html>{{ content }}</html>");
    let template = Template::from_data(
        "std",
        "simple",
        json!({ "output": "html", "engine": "host", "main": "simple.html", "accepts": "all" }),
        &|path| files.read_text(path).map_err(|error| error.detail),
        &|path| files.read_bytes(path).map_err(|error| error.detail),
        &|_| Ok(None),
        None,
    )
    .expect("a template");
    let registry = build_registry([
        Arc::new(ComposeFeature::new()) as Arc<dyn FlmFeature>,
        Arc::new(ComposeStandard { templates: vec![template] }),
    ])
    .expect("the registry builds");
    let engines: Vec<&dyn TemplateEngine> = registry.services().services_named::<dyn TemplateEngine>(ENGINES_SLOT).collect();
    let templates: Vec<&Template> = registry.services().services_named::<Template>(TEMPLATES_SLOT).collect();
    assert_eq!((engines.len(), templates.len()), (2, 1));
}

// --- E5 -----------------------------------------------------------------------------------

/// The proof's rewrite stage: dashes in characters (the first shape a rewrite
/// takes), and `\sdquote{text}{kept}` rebuilt from its parts between quotes
/// (the second). Written against the façade traits alone; never run.
#[allow(dead_code, reason = "a compile-only proof: never run")]
struct Dashes;

#[allow(dead_code, reason = "a compile-only proof: never run")]
fn is_copy<T: Copy>() {}

impl flm_ext::api::core::stage::RewriteStage for Dashes {
    fn decl(&self) -> flm_ext::api::core::stage::RewriteDecl {
        flm_ext::api::core::stage::RewriteDecl::new("sd.dash", "sd.dashed").docs("Dashes")
    }

    fn needs(&self) -> flm_ext::api::core::stage::Needs {
        flm_ext::api::core::stage::Needs::new().after("flm.userdefs").option("sd.options")
    }

    fn restage<'c>(
        &self,
        node: &'c dyn Node<'c>,
        cx: &'c dyn flm_ext::api::core::run::RewriteCx,
        rs: &'c mut dyn flm_ext::api::techy::transform::Restager,
    ) -> Result<flm_ext::api::techy::transform::Restage, ExtError> {
        use flm_ext::api::techy::transform::{Restage, RestagedArgument, RestagedSlot, Staged};

        // The three methods of the context.
        let _: (String, String, serde_json::Value) = (cx.tree_id(), cx.layout(), cx.option("sd.options")?);
        // Staged tokens are `Copy`.
        is_copy::<Staged>();

        // Shape 1: characters, replaced by a characters node staged in their place.
        if let Some(text) = node.chars() {
            return Ok(Restage::Emit(vec![rs.stage_chars(&text.replace("--", "\u{2013}"), node)?]));
        }

        // Shape 2: a call rebuilt from its parts.
        if node.element_name() == "sd.quote" {
            // Relatives asked on a temporary outlive it, and are restaged
            // beyond its borrow.
            let content: Vec<Staged> = {
                let children = node.argument_content_nodes_named("text")?.unwrap_or_default();
                let mut staged = Vec::new();
                for child in &children {
                    staged.extend(rs.restage_subtree(&**child)?);
                }
                staged
            };
            let siblings = node.parent().map(|parent| parent.children()).unwrap_or_default();
            if let Some(first) = siblings.first() {
                let _: Vec<Staged> = rs.restage_children(&**first)?;
            }
            // A bundle answered by one method is passed to another by value:
            // it borrows nothing of the restager.
            let argument: Box<dyn RestagedArgument> = rs.restage_argument_with_content(node, 0, content)?;
            let kept: Box<dyn RestagedArgument> = rs.restage_argument(node, 1)?;
            let _: bool = kept.is_provided();
            let named: Box<dyn RestagedArgument> = rs.restage_argument_named(node, "kept")?;
            let slot: Box<dyn RestagedSlot> = rs.restage_slot(node, 0)?;
            let _: Option<String> = slot.name();
            let emptied: Box<dyn RestagedSlot> = rs.restage_slot_with_content(node, 0, Vec::new())?;
            drop((named, emptied));
            let call = rs.restage_invocation(node, vec![argument, kept], vec![slot])?;
            let open = rs.stage_chars("\u{201c}", node)?;
            let close = rs.stage_chars("\u{201d}", node)?;
            return Ok(Restage::Emit(vec![open, call, close]));
        }

        // Elsewhere: the children restaged, the result unused, and the node kept.
        let _ = rs.restage_children(node)?;
        rs.diagnose(Diagnostic::error("sd.kept", "kept").at(node))?;
        Ok(Restage::Descend)
    }
}

/// The registration of the proof's stage: the stage is a `dyn RewriteStage`
/// held in an `Arc`, which `FeatureBuilder::rewrite_stage` takes.
#[allow(dead_code, reason = "a compile-only proof: the stage is never registered")]
fn e5_consumer(b: &mut dyn flm_ext::api::core::registry::FeatureBuilder) -> Result<(), ExtError> {
    let stage: Arc<dyn flm_ext::api::core::stage::RewriteStage> = Arc::new(Dashes);
    b.rewrite_stage(stage)
}

// E6: one consumer of each new façade. Each is written against the
// extension API alone and proves, by compiling, that what a façade answers
// owns what it points at: an entry a provider's context made, and a host
// service a context lent, outlive the borrow of the context, so an answer
// can carry them and a binding can lend them into the call's table beside
// the context.

/// A provider's body over [`ProviderCx`]: entries made through the context,
/// kept beyond its borrow, answered with the requests' keys.
fn e6_provider_consumer(
    cx: &dyn flm_ext::api::stdcore::features::citations::ProviderCx,
    requests: &[flm_ext::api::stdcore::features::citations::CitationRequest],
) -> Result<Vec<flm_ext::api::stdcore::features::citations::CitationResult>, ExtError> {
    use flm_ext::api::stdcore::features::citations::{CitationAnswer, CitationEntry, EntryPiece};
    let made: Vec<Box<dyn CitationEntry>> = {
        let borrowed = cx;
        let mut made = Vec::new();
        for request in requests {
            made.push(borrowed.parse_entry(&request.key, "refs.json")?);
        }
        made.push(borrowed.build_entry(&[EntryPiece::Text("x".to_string())])?);
        made
    };
    let _files = cx.host("files")?;
    Ok(made.into_iter().take(requests.len()).map(|entry| Ok(Some(CitationAnswer::new(entry)))).collect())
}

/// A module's body over [`TemplateCx`]: the `files` handle kept beyond the
/// borrow of the context, then read.
fn e6_template_consumer(cx: &dyn flm_ext::api::stdcore::features::compose::TemplateCx) -> Result<Option<String>, ExtError> {
    use flm_ext::api::host::{FilesScope, HostHandle};
    let handle: HostHandle = {
        let borrowed = cx;
        borrowed.host("files")?
    };
    match handle {
        HostHandle::Files(files) => files.read_text(FilesScope::Extension, "head.html"),
        HostHandle::Service(_) => Ok(None),
    }
}

/// A binding's half over [`CitationEntry`]: the context and the entries it
/// makes lent into one call table, an entry handed back by its handle.
fn e6_binding_consumer<'c>(
    cx: &'c dyn flm_ext::api::stdcore::features::citations::ProviderCx,
    table: &mut flm_ext::adapter::handles::CallTable<'c>,
) -> Result<Vec<flm_ext::api::stdcore::features::citations::CitationResult>, ExtError> {
    use flm_ext::adapter::handles::Entry;
    use flm_ext::adapter::values::{citation_results_from_value, handle_to_value};
    let cx = table.lend(Entry::ProviderCx(cx));
    let entry = flm_ext::adapter::dispatch::call(table, cx, "parse_entry", &serde_json::json!(["Knuth", "refs.json"]))?;
    let answer = serde_json::json!([{ "entry": entry }, null]);
    let _ = handle_to_value(cx);
    citation_results_from_value(table, &answer)
}

#[test]
fn e6_the_consumers_of_the_new_facades_compile() {
    let _ = e6_provider_consumer as fn(_, _) -> _;
    let _ = e6_template_consumer as fn(_) -> _;
    let _ = e6_binding_consumer as fn(_, _) -> _;
}

// --- E4 -----------------------------------------------------------------------------------
//
// The compile-only proof of E4's contract (plan E4, section 6.5): the durable
// handles, the handler aspects, and a point read back from data. Items 1, 3
// and 4 run; items 2 and 6 compile against the real items of WP-1 and WP-3
// (the durable table, `lang()`, `TemplateLanguage`, `compile_template_with`,
// `ValueBindings`, `instantiate`, and `durable_template` on the façades that
// lend the table), which `tests/durable.rs` and `tests/expand.rs` run; item 5
// compiles against the stand-in marked `// WP-2a` and runs through it, the
// adapter's `ExtReferenceable` over the subset of points that name no node.

mod e4 {
    use std::sync::{Arc, Mutex};

    use flm_core::core::registry::{BuildError, FeatureBuilder as CoreFeatureBuilder, FeatureInfo, FlmFeature, PackageEntry};
    use flm_core::lang::rewrites::userdefs::{
        CompiledTemplate as CoreCompiledTemplate, PlaceholderBindings as CorePlaceholderBindings, PlaceholderTarget,
        PlaceholderValue, PlaceholderVocabulary, TemplateLanguage as CoreTemplateLanguage, TemplateSpec as CoreTemplateSpec,
        ValueBindings as CoreValueBindings,
    };
    use flm_core::lang::{parse_fragment, Flm, FlmOrigin, FlmSource, ParseOptions};
    use flm_core::model::addr::{NodeAddr, TreeId};
    use flm_core::model::aspects as core_aspects;
    use flm_core::model::doc::FragmentLayout;
    use flm_core::model::names::{Name, RefKind};
    use flm_core::techy::core::node::{NodeRef, NodeSlice, NodeTree};
    use flm_core::techy::latexlike::Mode;

    use flm_ext::adapter::durable::{DurableKind, DurableTable, DurableToken};
    use flm_ext::adapter::node::NodeFacade;
    use flm_ext::adapter::values::node_addr_to_value;
    use flm_ext::api::core::lowering::{DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule};
    use flm_ext::api::lang::mint::{ArgDef, ElementDef, SyntaxDef};
    use flm_ext::api::lang::rewrites::userdefs::{
        compile_template_with, instantiate, Expand, ExpandCx, Expander, Expansion, TemplateLanguage, TemplateSpec,
        ValueBindings,
    };
    use flm_ext::api::model::aspects::{
        AcceptedRefTypes, AnchorPolicy, ContentRef, Label, RefPoint, Referenceable,
    };
    use flm_ext::api::model::element::ElementSpec;
    use flm_ext::api::techy::node::Node;
    use flm_ext::ExtError;

    use super::common;

    fn assert_send_sync<T: Send + Sync>() {}

    /// The template a title is written with: a value placeholder the feature
    /// fills (`#{number}`), as `myhead` and the `headings` feature write theirs.
    fn title_vocabulary() -> PlaceholderVocabulary {
        PlaceholderVocabulary::new(0).extra("number", false)
    }

    fn title_spec(vocabulary: &PlaceholderVocabulary) -> CoreTemplateSpec<'_> {
        CoreTemplateSpec {
            text: "Exercise #{number}",
            mode: Mode::Text,
            vocabulary,
            offset: 0,
            definition: "the title of an exercise",
            label: "title of an exercise",
            origin_label: "title of an exercise",
            user_definition: false,
        }
    }

    // --- 1. A durable table shared across threads --------------------------------------

    #[test]
    fn e4_1_a_durable_table_is_shared_across_threads() {
        assert_send_sync::<CoreTemplateLanguage>();
        assert_send_sync::<CoreCompiledTemplate>();
        assert_send_sync::<DurableTable>();
        assert_send_sync::<DurableToken>();
        let registry = common::registry_with([]);
        let lang = registry.lang();
        let table = DurableTable::new("ex", "_", "titles");
        let (language_token, _) = table.language(lang).expect("the template language builds");
        assert_eq!(language_token.kind, DurableKind::TemplateLanguage);
        // Answered again, not built again.
        let (again, _) = table.language(lang).expect("kept");
        assert_eq!(again, language_token);

        let vocabulary = title_vocabulary();
        let spec = title_spec(&vocabulary);
        let tokens: Vec<DurableToken> = std::thread::scope(|scope| {
            let threads: Vec<_> = (0..2)
                .map(|_| {
                    scope.spawn(|| {
                        let (token, template) = table.template(&spec, lang).expect("the template compiles");
                        assert!(template.diagnostics().is_empty(), "{:?}", template.diagnostics());
                        token
                    })
                })
                .collect();
            threads.into_iter().map(|thread| thread.join().expect("the thread finishes")).collect()
        });
        assert_eq!(tokens[0], tokens[1], "one index for one specification");
        assert_eq!(tokens[0].kind, DurableKind::CompiledTemplate);
        assert_eq!(tokens[0].table, table.number());
        assert_eq!(table.len(), 2, "one language and one template: compiled once");
        assert!(table.compiled_template(tokens[0]).is_ok());
    }

    // --- 3. A stale durable token ------------------------------------------------------

    #[test]
    fn e4_3_a_token_of_another_table_is_stale() {
        let registry = common::registry_with([]);
        let lang = registry.lang();
        let vocabulary = title_vocabulary();
        let spec = title_spec(&vocabulary);
        let compile = |table: &DurableTable| -> DurableToken { table.template(&spec, lang).expect("the template compiles").0 };
        // The table of this registration, and the table of the same feature
        // under another registry: the same specification, the same index.
        let this = DurableTable::new("ex", "_", "titles");
        let other = DurableTable::new("ex", "_", "titles");
        let mine = compile(&this);
        let theirs = compile(&other);
        assert_eq!((mine.kind, mine.index), (theirs.kind, theirs.index));
        assert_ne!(mine.table, theirs.table);

        let stale = |token: DurableToken| {
            let error = this.compiled_template(token).err().expect("stale");
            assert!(matches!(error, ExtError::StaleDurable { .. }), "{error:?}");
            error.to_string()
        };
        let text = "the `compiled_template` object 1 must be one the feature `titles` of the package `_` of the \
                    extension `ex` made under the registry in use";
        assert_eq!(stale(theirs), text);
        // An index this table does not hold, and an entry of another kind.
        assert!(stale(DurableToken { index: 9, ..mine }).contains("object 9 must be one"));
        assert!(stale(DurableToken { kind: DurableKind::TemplateLanguage, ..mine }).starts_with("the `template_language` object 1"));
        assert!(this.compiled_template(mine).is_ok());
    }

    // --- 4. A node found by its index --------------------------------------------------

    #[test]
    fn e4_4_a_node_is_found_again_by_the_index_its_address_was_written_from() {
        let registry = common::registry_with([]);
        let source = Arc::new(FlmSource::new("A \\emph{b} and $x^2$.\n\nTwo.").with_origin(FlmOrigin::none()));
        let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
            .expect("the document parses");
        let tree: &NodeTree<Flm> = fragment.tree();
        let mut count = 0u32;
        for node in tree.iter_storage_order() {
            // The address as the mirror writes it, read back.
            let value = node_addr_to_value(&NodeAddr::new(TreeId::tree(0), node.id()));
            let index = u32::try_from(value["node"].as_u64().expect("an index")).expect("a u32");
            let slice: NodeSlice<'_, Flm> = tree.slice(index..index + 1).expect("one node");
            let found: NodeRef<'_, Flm> = slice.first().expect("the node");
            assert_eq!(found.id(), node.id());
            count += 1;
        }
        assert!(count > 5);
        assert!(tree.slice(count..count + 1).is_none(), "an index past the end answers nothing");
    }

    // --- 2. A durable template through the per-call façade -----------------------------

    /// The title's specification, as a handler writes it.
    fn title_spec_data() -> TemplateSpec {
        TemplateSpec {
            text: "Exercise #{number}".to_string(),
            mode: Mode::Text,
            vocabulary: title_vocabulary(),
            definition: "the title of an exercise".to_string(),
            label: "title of an exercise".to_string(),
            origin_label: "title of an exercise".to_string(),
            user_definition: false,
            offset: 0,
        }
    }

    /// The token a hook keeps in module scope.
    static KEPT: Mutex<Option<DurableToken>> = Mutex::new(None);

    /// A hook that compiles its title once, keeps the token, and instantiates
    /// the template the token names at every later call.
    struct KeepsItsTemplate {
        number: Arc<NodeTree<Flm>>,
    }

    impl Expand for KeepsItsTemplate {
        fn expand<'c>(
            &self,
            node: &'c dyn Node<'c>,
            cx: &'c mut dyn ExpandCx,
            xp: &'c mut dyn Expander,
        ) -> Result<Expansion, ExtError> {
            let kept = *KEPT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let template = match kept {
                // Resolved against the table of the hook's feature.
                Some(token) => xp.durable_template(token)?,
                None => {
                    // The language's façade borrows the expander; the durable
                    // template does not, so the borrow ends here.
                    let lang = xp.lang();
                    let language = TemplateLanguage::new(&*lang)?;
                    let template = compile_template_with(&title_spec_data(), &*lang, &language)?;
                    *KEPT.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(template.token());
                    template
                }
            };
            let number = NodeFacade::shared(Arc::clone(&self.number), self.number.root().id());
            let bindings = ValueBindings::new().with("number", &number)?;
            // The façade owns its `Arc`: `&dyn CompiledTemplate` borrows the
            // façade, not the expander or the table.
            let staged = xp.instantiate(cx, node, &template, &bindings)?;
            Ok(Expansion::Emit(staged))
        }
    }

    #[test]
    fn e4_2_a_durable_template_meets_the_expander_s_borrow() {
        // Compile-only: the hook is built, not run (`tests/expand.rs` and
        // `tests/durable.rs` run the same calls).
        let registry = common::registry_with([]);
        let fragment = parse_fragment(
            registry.lang(),
            Arc::new(FlmSource::new("3").with_origin(FlmOrigin::none())),
            FragmentLayout::default(),
            &ParseOptions::default(),
        )
        .expect("the number parses");
        let hook: Arc<dyn Expand> = Arc::new(KeepsItsTemplate { number: Arc::clone(fragment.tree()) });
        let _ = hook;
    }

    // --- 6. A value bindings object ----------------------------------------------------

    /// A rule's body writing the title from the durable template and a number
    /// tree it holds, spliced through the lowerer.
    struct TitleRule {
        template: DurableToken,
        number: Arc<NodeTree<Flm>>,
    }

    impl Lower for TitleRule {
        fn lower<'c>(&self, node: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
            let template = lw.durable_template(self.template)?;
            let number = NodeFacade::shared(Arc::clone(&self.number), self.number.root().id());
            let bindings = ValueBindings::new().with("number", &number)?;
            let title = instantiate(node, &template, &bindings)?;
            Ok(Lowering::Emit(lw.splice(cx, &*title)?))
        }
    }

    #[test]
    fn e4_6_value_bindings_reach_instantiate_and_the_lowerer() {
        // `flm-core`'s bindings (request 9) answer as the request sketches;
        // the rule is built, not run (`tests/durable.rs` runs it).
        let registry = common::registry_with([]);
        let number = parse_fragment(
            registry.lang(),
            Arc::new(FlmSource::new("3").with_origin(FlmOrigin::none())),
            FragmentLayout::default(),
            &ParseOptions::default(),
        )
        .expect("the number parses");
        let bindings = CoreValueBindings::new().with("number", Arc::clone(number.tree()));
        assert!(matches!(bindings.value(&PlaceholderTarget::Extra("number".to_string()), None), PlaceholderValue::Tree(_)));
        assert!(matches!(bindings.value(&PlaceholderTarget::Extra("other".to_string()), None), PlaceholderValue::NoValue));
        assert!(matches!(bindings.value(&PlaceholderTarget::Body, None), PlaceholderValue::Unresolvable));
        let rule: Arc<dyn Lower> = Arc::new(TitleRule {
            template: DurableToken { kind: DurableKind::CompiledTemplate, table: 0, index: 0 },
            number: Arc::clone(number.tree()),
        });
        let _ = rule;
    }

    // --- 5. A handler aspect in Rust ---------------------------------------------------

    /// The reference point of `\exercise{title}`: the label `ex:<title>`,
    /// computed, and the title as the content a reference shows.
    struct ExercisePoints;

    impl Referenceable for ExercisePoints {
        fn points<'c>(&self, node: &'c dyn Node<'c>) -> Result<Vec<RefPoint<'c>>, ExtError> {
            let title = node.argument_content_nodes_named("title")?.unwrap_or_default();
            let text = title.iter().filter_map(|node| node.chars()).collect::<String>();
            Ok(vec![RefPoint {
                kind: RefKind::new("exercise"),
                labels: vec![Label::new("ex", text.to_lowercase())],
                computed: 1,
                malformed: Vec::new(),
                anchor: AnchorPolicy::FromFirstLabel,
                number: None,
                content: Some(ContentRef::ArgumentNamed("title".to_string())),
                accepts: AcceptedRefTypes::these(["ex"]),
            }])
        }
    }

    /// A point that names no node, as `flm-core`'s; `None` for one that names
    /// a node.
    // WP-2a: `ref_point_to_core` resolves the nodes through the sealed
    // accessor, which code outside the crate cannot call.
    fn ref_point_to_core_stand_in(point: RefPoint<'_>) -> Option<core_aspects::RefPoint> {
        let content = |content: ContentRef<'_>| match content {
            ContentRef::Argument(i) => Some(core_aspects::ContentRef::Argument(i)),
            ContentRef::ArgumentNamed(name) => Some(core_aspects::ContentRef::ArgumentNamed(name)),
            ContentRef::Body => Some(core_aspects::ContentRef::Body),
            ContentRef::Node(_) | ContentRef::Synthesized(_) => None,
        };
        let anchor = match point.anchor {
            AnchorPolicy::Explicit(id) => core_aspects::AnchorPolicy::Explicit(id),
            AnchorPolicy::FromFirstLabel => core_aspects::AnchorPolicy::FromFirstLabel,
            AnchorPolicy::Slug { prefix, content: slug } => core_aspects::AnchorPolicy::Slug { prefix, content: content(slug)? },
            AnchorPolicy::FromValue { prefix } => core_aspects::AnchorPolicy::FromValue { prefix },
            AnchorPolicy::Auto { prefix } => core_aspects::AnchorPolicy::Auto { prefix },
        };
        let number = match point.number {
            Some(number) if number.instances.is_empty() => Some(core_aspects::NumberSource {
                counter: number.counter,
                instances: Vec::new(),
                style: number.style,
                advance: number.advance,
                number_format: number.number_format,
            }),
            Some(_) => return None,
            None => None,
        };
        let content = match point.content {
            Some(shown) => Some(content(shown)?),
            None => None,
        };
        if !point.malformed.is_empty() {
            return None;
        }
        Some(core_aspects::RefPoint {
            kind: point.kind,
            labels: point.labels,
            computed: point.computed,
            malformed: Vec::new(),
            anchor,
            number,
            content,
            accepts: point.accepts,
        })
    }

    // WP-2a: the adapter's `ExtReferenceable`; a stand-in that lends the node
    // as the adapter does (`NodeFacade::lent`) and answers no point on a
    // failure (plan section 4.3).
    struct ExtReferenceableStandIn {
        aspect: Arc<dyn Referenceable>,
    }

    impl core_aspects::Referenceable for ExtReferenceableStandIn {
        fn points(&self, node: &NodeRef<'_, Flm>) -> Vec<core_aspects::RefPoint> {
            let lent = NodeFacade::lent(*node);
            match self.aspect.points(&lent) {
                Ok(points) => points.into_iter().filter_map(ref_point_to_core_stand_in).collect(),
                Err(_) => Vec::new(),
            }
        }
    }

    // WP-2a: `Aspect::Referenceable` on the element's specification, or
    // `attach`; a `flm-core` feature attaches the stand-in to the scripted
    // element here.
    struct AttachesPoints;

    impl FlmFeature for AttachesPoints {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new("ex-points", "The reference points of `\\exercise`")
        }

        fn register(&self, b: &mut CoreFeatureBuilder<'_>) -> Result<(), BuildError> {
            b.attach::<dyn core_aspects::Referenceable>(
                "ex.exercise",
                Arc::new(ExtReferenceableStandIn { aspect: Arc::new(ExercisePoints) }),
            )
        }
    }

    /// The rule of `\exercise`: its title.
    struct ExerciseTitle;

    impl Lower for ExerciseTitle {
        fn lower<'c>(&self, node: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
            Ok(Lowering::Emit(lw.lower_arg(cx, node, 0)?))
        }
    }

    #[test]
    fn e4_5_a_rust_referenceable_on_a_scripted_element_gives_ref_its_target() {
        let exercises = common::double::Double::new("ex").feature("ex", |b| {
            let exercise = ElementDef::new(
                b.feature(),
                ["exercise"],
                SyntaxDef::Macro { args: vec![ArgDef::mandatory("title")] },
                ElementSpec::new("ex.exercise"),
            );
            b.element(exercise)?;
            b.lowering_rule(LoweringRule::new(
                "ex.exercise.lower.default",
                "ex.exercise",
                LoweringDecl::new(),
                Arc::new(ExerciseTitle),
                DefaultMark::Everywhere,
            ))
        });
        let registry = common::registry_with([
            exercises.package(),
            PackageEntry::new(Name::new("ex-points")).feature(Arc::new(AttachesPoints)),
        ]);
        let (text, diagnostics) = common::text_of(&registry, "\\exercise{Sums}\n\nSee \\ref{ex:sums}.");
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(text.contains("See Sums."), "{text}");
    }
}
