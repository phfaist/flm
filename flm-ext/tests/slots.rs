//! The data-valued slots (plan E4, section 5.3): `peer_slot` and
//! `contribute_to` with a value written as data (`Contributed::Data`, E6's
//! route), typed at the call by the slot's name. That a counter numbers a point, that a style's words appear
//! in a `\ref` and that a parent set resolves a `Within.parent_set` is shown
//! end to end in `tests/numbering.rs`; here, each reader's value as the
//! registry holds it, the preamble piece in the style file, the package
//! definition's line, and the refusals.

mod common;

use std::sync::Arc;

use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::stdcore::backends::latex::{packages as core_packages, preamble as core_preamble};
use flm_core::stdcore::backends::latex_recomposed;
use flm_core::stdcore::features::numbering::{
    CounterDecl, CounterStyleDecl, Delimiters, ParentSetDecl, PrefixWords, Within,
};
use flm_ext::adapter::dispatch;
use flm_ext::adapter::handles::{CallTable, Entry};
use flm_ext::api::core::lowering::{DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule};
use flm_ext::api::core::registry::{Contributed, FeatureBuilder, Finalizer};
use flm_ext::api::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_ext::api::model::element::ElementSpec;
use flm_ext::api::stdcore::backends::latex::{packages, preamble};
use flm_ext::api::stdcore::features::numbering::{COUNTERS_SLOT, PARENT_SETS_SLOT, STYLES_SLOT, WITHIN_JOINERS_SLOT};
use flm_ext::api::techy::node::Node;
use flm_ext::ExtError;
use serde_json::{json, Value};

use common::double::Double;
use common::{registry_with, run, try_registry_with};

/// The package `ex`, whose feature `slots` runs `register`.
fn slots(register: impl Fn(&mut dyn FeatureBuilder) -> Result<(), ExtError> + Send + Sync + 'static) -> Double {
    Double::new("ex").feature("slots", register)
}

#[test]
fn each_reader_reads_a_value_the_registry_holds() {
    let registry = registry_with([slots(|b| {
        b.peer_slot(
            STYLES_SLOT,
            Contributed::Data(json!({ "name": "ex.style", "prefix_words": PrefixWords::new("Ex~", "Exs~"), "delimiters": Delimiters::new("(", ")"), "ranges": false })),
        )?;
        b.peer_slot(COUNTERS_SLOT, Contributed::Data(json!({ "name": "ex.part", "default_style": "ex.style" })))?;
        b.peer_slot(
            COUNTERS_SLOT,
            Contributed::Data(json!({ "name": "ex.counter", "default_style": "ex.style", "docs": "The exercises.", "within": Within::parent("ex.part") })),
        )?;
        b.peer_slot(PARENT_SETS_SLOT, Contributed::Data(json!({ "name": "ex.levels", "counters": ["ex.part"] })))?;
        b.contribute_to(preamble::SLOT, Contributed::Data(json!({ "name": "ex.env", "content": "\\providecommand\\exEnv{}\n", "requires": [packages::AMSTHM] })))?;
        b.contribute_to(packages::SLOT, Contributed::Data(json!({ "id": "ex.foo", "package": "foo", "options": ["a", "b=c"], "docs": "Foo." })))
    })
    .package()]);
    let services = registry.services();
    let styles: Vec<&CounterStyleDecl> = services.services_named::<CounterStyleDecl>(STYLES_SLOT).filter(|s| s.name.as_str() == "ex.style").collect();
    let mut expected = CounterStyleDecl::new("ex.style");
    expected.prefix_words = PrefixWords::new("Ex~", "Exs~");
    expected.delimiters = Delimiters::new("(", ")");
    expected.ranges = false;
    assert_eq!(styles, [&expected]);
    let counters: Vec<&CounterDecl> =
        services.services_named::<CounterDecl>(COUNTERS_SLOT).filter(|c| c.name.as_str().starts_with("ex.")).collect();
    assert_eq!(
        counters,
        [&CounterDecl::new("ex.part", "ex.style"), &CounterDecl::new("ex.counter", "ex.style").docs("The exercises.").within(Within::parent("ex.part"))]
    );
    let sets: Vec<&ParentSetDecl> =
        services.services_named::<ParentSetDecl>(PARENT_SETS_SLOT).filter(|s| s.name.as_str() == "ex.levels").collect();
    assert_eq!(sets, [&ParentSetDecl::new("ex.levels", ["ex.part"])]);
    let piece = core_preamble::piece_named(services, &preamble::piece_id("ex.env")).expect("the piece");
    assert_eq!(piece.content, "\\providecommand\\exEnv{}\n");
    assert_eq!(piece.requires, [flm_core::model::names::Name::from(core_packages::AMSTHM)]);
    let package = core_packages::packages_named(services, "ex.foo").next().expect("the package definition");
    assert_eq!(package.usepackage_line(), "\\usepackage[a,b=c]{foo}");
}

/// A rule's body that writes `\exEnv` and nothing of the node.
struct WritesTheMacro;

impl Lower for WritesTheMacro {
    fn lower<'c>(&self, _: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
        Ok(Lowering::Emit(vec![lw.pres(cx, flm_ext::api::stdcore::backends::latex::RAW, &json!({ "text": "\\exEnv{}" }), vec![])?]))
    }
}

#[test]
fn a_preamble_piece_reaches_the_style_file_when_a_rule_requires_it() {
    let registry = registry_with([slots(|b| {
        b.peer_slot(preamble::SLOT, Contributed::Data(json!({ "name": "ex.env", "content": "\\providecommand\\exEnv{}\n", "docs": "The macro.", "requires": [packages::AMSTHM] })))?;
        b.element(ElementDef::new(b.feature(), ["exmark"], SyntaxDef::Macro { args: vec![ArgDef::mandatory("x")] }, ElementSpec::new("ex.mark")))?;
        let decl = LoweringDecl::new().selection_requires(preamble::piece_id("ex.env")).produces(flm_ext::api::stdcore::backends::latex::RAW);
        b.lowering_rule(LoweringRule::new(
            "ex.mark.lower.latex",
            "ex.mark",
            decl,
            Arc::new(WritesTheMacro),
            DefaultMark::OnBackends(vec![latex_recomposed::BACKEND.into()]),
        ))
    })
    .package()]);
    let run = run(&registry, latex_recomposed::WORKFLOW, "A \\exmark{x}.");
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let style = run
        .table(latex_recomposed::DOCUMENT)
        .expect("declared")
        .and_then(latex_recomposed::style_artifact)
        .and_then(|style| style.as_bytes().map(|b| String::from_utf8_lossy(b).into_owned()))
        .expect("a style file");
    assert!(style.contains("\\providecommand\\exEnv{}"), "{style}");
    assert!(style.contains("{amsthm}"), "{style}");
}

#[test]
fn a_value_named_outside_the_stem_is_refused() {
    for (slot, value, what) in [
        (COUNTERS_SLOT, json!({ "name": "other.c", "default_style": "ex.s" }), "counter"),
        (STYLES_SLOT, json!({ "name": "other.s" }), "counter style"),
        (PARENT_SETS_SLOT, json!({ "name": "other.p", "counters": [] }), "parent set"),
        (preamble::SLOT, json!({ "name": "other.env", "content": "" }), "preamble piece"),
        (packages::SLOT, json!({ "id": "flm.latex.pkg.foo", "package": "foo", "options": [], "docs": "" }), "package definition"),
    ] {
        let error = try_registry_with([slots(move |b| b.peer_slot(slot, Contributed::Data(value.clone()))).package()]).expect_err("outside the stem").to_string();
        assert!(error.contains("flm.ext.name-outside-extension"), "{error}");
        assert!(error.contains(&format!("the {what} `")), "{what}: {error}");
        assert!(error.contains("must begin with `ex.`"), "{error}");
    }
}

/// A slot outside the table of standard data-valued slots takes data as it
/// is (E6's route, the merge's join): a standard slot whose values are not
/// data then fails the build with `flm-core`'s type check.
#[test]
fn data_for_a_standard_slot_outside_the_table_fails_the_build_with_the_two_types() {
    let error = try_registry_with([slots(|b| b.contribute_to(WITHIN_JOINERS_SLOT, Contributed::Data(json!({ "name": "ex.j" })))).package()])
        .expect_err("a slot outside the table")
        .to_string();
    assert!(
        error.contains("a value contributed to service slot `flm.numbering.within-joiners` must have the slot's type"),
        "{error}"
    );
    assert!(error.contains("but feature `slots` contributes a `ExtService`"), "{error}");
}

#[test]
fn a_value_that_does_not_read_is_refused_with_the_field() {
    let error = try_registry_with([slots(|b| b.contribute_to(COUNTERS_SLOT, Contributed::Data(json!({ "name": "ex.c", "default_style": 3 })))).package()])
        .expect_err("a bad field")
        .to_string();
    assert!(error.contains("the field `default_style` of a counter declaration must be a string, not `3`"), "{error}");
}

/// A registry of the package `ex` alone, without the standard package, so
/// that no feature declares the numbering slots.
fn alone(register: impl Fn(&mut dyn FeatureBuilder) -> Result<(), ExtError> + Send + Sync + 'static) -> Result<Arc<flm_core::core::registry::FlmRegistry>, String> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(slots(register).package());
    builder.build(&RegistryInputs::new()).map_err(|error| error.to_string())
}

#[test]
fn contribute_to_an_undeclared_slot_fails_the_build_and_peer_slot_leaves_a_note() {
    let counter = json!({ "name": "ex.c", "default_style": "ex.s" });
    let error = alone({
        let counter = counter.clone();
        move |b| b.contribute_to(COUNTERS_SLOT, Contributed::Data(counter.clone()))
    })
    .expect_err("an undeclared slot");
    assert!(error.contains("flm.numbering.counters"), "{error}");
    let registry = alone(move |b| b.peer_slot(COUNTERS_SLOT, Contributed::Data(counter.clone()))).expect("a peer declaration");
    let notes: Vec<String> = registry.build_diagnostics().iter().map(|diagnostic| diagnostic.to_string()).collect();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("flm.numbering.counters"), "{notes:?}");
}

/// A finalizer that runs `f` through the dispatcher, as a script's does.
struct ThroughTheDispatcher(fn(&mut CallTable<'_>, &Value) -> Result<Value, ExtError>);

impl Finalizer for ThroughTheDispatcher {
    fn finalize(&self, b: &mut dyn FeatureBuilder) -> Result<(), ExtError> {
        let mut table = CallTable::new(9100);
        let builder = flm_ext::adapter::values::handle_to_value(table.lend(Entry::Builder(b)));
        (self.0)(&mut table, &builder).map(|_| ())
    }
}

/// `dispatch::call` on `target`.
fn ask(table: &mut CallTable<'_>, target: &Value, method: &str, args: Value) -> Result<Value, ExtError> {
    dispatch::call(table, flm_ext::adapter::values::handle_from_value(target)?, method, &args)
}

#[test]
fn peer_slot_from_a_finalizer_is_contribution_after_assembly() {
    let package = Double::new("ex")
        .feature("late", |b| {
            b.finalizer(Arc::new(ThroughTheDispatcher(|table, builder| {
                ask(table, builder, "peer_slot", json!([COUNTERS_SLOT, { "name": "ex.c", "default_style": "ex.s" }]))
            })))
        })
        .package();
    let error = try_registry_with([package]).expect_err("a contribution of the second round").to_string();
    assert!(error.contains("flm.ext.handler-failed: "), "{error}");
    assert!(error.contains("a finalizer of feature `late` may not contribute"), "{error}");
}

#[test]
fn a_finalizer_and_a_collection_member_through_the_dispatcher() {
    // S13: the arms `finalizer` and `collection_member`, driven through the
    // JSON double; the finalizer declares a table in the second round.
    let package = common::json_double::JsonDouble::new("ex").feature("late", |js, b| {
        js.call(b, "capability", json!(["ex.counted", "A capability of the extension.", null]))?;
        js.call(b, "collection_member", json!(["flm.validated", "ex.counted"]))?;
        let hook = js.lend(Entry::Finalizer(Arc::new(ThroughTheDispatcher(|table, builder| {
            ask(table, builder, "table", json!(["ex.late", "A table of the second round."]))
        }))));
        js.call(b, "finalizer", json!([hook]))?;
        Ok(())
    });
    let registry = registry_with([package.package()]);
    assert!(registry.table_named("ex.late").is_some());
    let outside = common::json_double::JsonDouble::new("ex").feature("late", |js, b| {
        js.call(b, "collection_member", json!(["flm.validated", "other.counted"]))?;
        Ok(())
    });
    let error = try_registry_with([outside.package()]).expect_err("outside the stem").to_string();
    assert!(error.contains("other.counted"), "{error}");
}

#[test]
fn contribute_to_piece_id_and_package_id_through_the_dispatcher() {
    // S18: the generated arms `contribute_to`, `piece_id` and `package_id`,
    // driven through the JSON double.
    use common::json_double::{module, JsonDouble};
    let ids = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = Arc::clone(&ids);
    let double = JsonDouble::new("ex").feature("slots", move |js, b| {
        js.call(b, "contribute_to", json!([preamble::SLOT, { "name": "ex.env", "content": "\\providecommand\\exEnv{}\n" }]))?;
        let piece = js.call(&module("stdcore/backends/latex/preamble"), "piece_id", json!(["ex.env"]))?;
        let package = js.call(&module("stdcore/backends/latex/packages"), "package_id", json!(["amsthm"]))?;
        seen.lock().expect("the lock").push((piece, package));
        Ok(())
    });
    let registry = registry_with([double.package()]);
    let piece = core_preamble::piece_named(registry.services(), &preamble::piece_id("ex.env")).expect("the piece");
    assert_eq!(piece.content, "\\providecommand\\exEnv{}\n");
    let ids = ids.lock().expect("the lock").clone();
    assert_eq!(ids[0], (json!(preamble::piece_id("ex.env")), json!(packages::package_id("amsthm"))));
}
