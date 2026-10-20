//! The registry build of `flm_core::core::registry`: the roster validations as
//! typed errors listing what was declared, per-phase error collection, aspects
//! attached on structural and unknown-callable elements, the digest, late-init
//! hooks, packages, and feature lookup.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use techy::error::Diagnostic;
use techy::latexlike::Mode;
use techy::serialize::SerialValue;

use flm_core::core::artifact::ArtifactSet;
use flm_core::core::lowering::{DefaultMark, LoweringDecl, LoweringRule};
use flm_core::core::registry::{
    BackendDef, BuildError, BuildPhase, CapabilityDef, CollectionDef, FeatureBuilder, Finalizer,
    LateInit, LateInitError, OrderingDef, OutputDef, PackageEntry, RegistryBuilder, RegistryInputs,
    Site,
};
use flm_core::core::registry::{FlmFeature, NativeImplication, NativeImplicationKind};
use flm_core::core::services::Arity;
use flm_core::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_core::core::data::table::TableName;
use flm_core::dump::session::{self, ElementResolver};
use flm_core::lang::{BuiltLang, FlmDiagnostics};
use flm_core::lang::ext::PresPayload;
use flm_core::lang::mint::{ArgDef, ArgLayout, ArgStdKind, ElementDef, ScopedPackageDef, SyntaxDef};
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::{lowering, pres};
use flm_core::stdcore::features::blocks::Blocks;

use common::pipeline::{COUNT, Note, feature, noop, provider, registry, registry_error, span};
use common::pipeline::{lowered_cap, plan_for, register_backend};
use common::{element, macro_def, macro_with};
use flm_core::core::plan::{LoweringSelection, Plan, PlanError, PlanRequest, PlanWarning};
use flm_core::core::registry::{PeerName, FlmRegistry};

fn single(error: flm_core::core::registry::RegistryBuildError) -> BuildError {
    assert_eq!(error.errors.len(), 1, "one error expected:\n{}", error.render());
    error.errors.into_iter().next().unwrap()
}

// --- duplicates and undeclared references -----------------------------------------------------

#[test]
fn duplicate_capability_and_stage_declarations_are_collected() {
    let error = registry_error(vec![feature("t", |b| {
        b.capability(CapabilityDef::new("a"))?;
        b.capability(CapabilityDef::new("a"))?;
        b.stage(provider("s", &["a"], &[]))?;
        b.stage(provider("s", &["a"], &[]))?;
        Ok(())
    })]);
    assert_eq!(error.phase, BuildPhase::Validate);
    let kinds: Vec<(&str, String)> = error
        .errors
        .iter()
        .map(|e| match e {
            BuildError::Duplicate { kind, name, .. } => (*kind, name.to_string()),
            other => panic!("unexpected error: {other:?}"),
        })
        .collect();
    assert_eq!(kinds, [("capability", "a".to_string()), ("stage", "s".to_string())]);
}

#[test]
fn a_duplicate_names_both_features() {
    let error = registry_error(vec![
        feature("t.first", |b| b.capability(CapabilityDef::new("a"))),
        feature("t.second", |b| b.capability(CapabilityDef::new("a"))),
    ]);
    match single(error) {
        BuildError::Duplicate { first, second, .. } => {
            assert_eq!(first, "t.first");
            assert_eq!(second, "t.second");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_undeclared_provides_lists_the_roster() {
    let error = registry_error(vec![feature("t", |b| {
        b.capability(CapabilityDef::new("a"))?;
        b.capability(CapabilityDef::new("b"))?;
        b.stage(provider("s", &["nope"], &[]))
    })]);
    match single(error) {
        BuildError::Undeclared { feature, kind, site, name, roster } => {
            assert_eq!(feature, "t");
            assert_eq!(kind, "capability");
            assert_eq!(site, Site::StageProvides { stage: "s".into() });
            assert_eq!(name, "nope");
            assert_eq!(roster, ["a", "b"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_refines_cycle_is_reported_once_from_its_name_least_member() {
    let error = registry_error(vec![feature("t", |b| {
        b.capability(CapabilityDef::new("beta").refines("gamma"))?;
        b.capability(CapabilityDef::new("gamma").refines("alpha"))?;
        b.capability(CapabilityDef::new("alpha").refines("beta"))
    })]);
    match single(error) {
        BuildError::RefinesCycle { cycle } => assert_eq!(cycle, ["alpha", "beta", "gamma"]),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_undeclared_parent_lists_the_roster() {
    let error = registry_error(vec![feature("t", |b| {
        b.capability(CapabilityDef::new("leaf").refines("missing"))
    })]);
    match single(error) {
        BuildError::Undeclared { kind, site, name, roster, .. } => {
            assert_eq!(kind, "capability");
            assert_eq!(site, Site::CapabilityRefines { capability: "leaf".into() });
            assert_eq!(name, "missing");
            assert_eq!(roster, ["leaf"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn two_defaults_under_one_parent_is_an_error() {
    let error = registry_error(vec![feature("t", |b| {
        b.capability(CapabilityDef::new("t.x"))?;
        b.capability(CapabilityDef::new("t.x.a").refines("t.x").default())?;
        b.capability(CapabilityDef::new("t.x.b").refines("t.x").default())
    })]);
    match single(error) {
        BuildError::TwoDefaults { parent, first, second } => {
            assert_eq!(parent, "t.x");
            assert_eq!(first, "t.x.a");
            assert_eq!(second, "t.x.b");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_default_without_a_parent_is_an_error() {
    let error =
        registry_error(vec![feature("t", |b| b.capability(CapabilityDef::new("t.x").default()))]);
    match single(error) {
        BuildError::DefaultWithoutParent { name } => assert_eq!(name, "t.x"),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_table_name_clashing_with_a_capability_is_an_error() {
    let error = registry_error(vec![
        feature("t.caps", |b| b.capability(CapabilityDef::new("t.count"))),
        feature("t.tables", |b| b.table(COUNT, "a count").map(|_| ())),
    ]);
    match single(error) {
        BuildError::TableCapabilityClash { name, table_owner, capability_owner } => {
            assert_eq!(name, "t.count");
            assert_eq!(table_owner, "t.tables");
            assert_eq!(capability_owner, "t.caps");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_check_stage_declaring_a_write_is_an_error() {
    let error = registry_error(vec![feature("t", |b| {
        b.table(COUNT, "a count")?;
        b.stage(noop(
            StageDecl::new("t.chk", Effect::Check, Scope::Document).writes(COUNT),
            Needs::new(),
        ))
    })]);
    match single(error) {
        BuildError::CheckWrites { stage, table } => {
            assert_eq!(stage, "t.chk");
            assert_eq!(table, "t.count");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_stage_writing_an_external_table_is_an_error() {
    let error = registry_error(vec![feature("t", |b| {
        b.external_table(COUNT, "installed by the host")?;
        b.stage(noop(
            StageDecl::new("t.w", Effect::Compute, Scope::Document).writes(COUNT),
            Needs::new(),
        ))
    })]);
    match single(error) {
        BuildError::ExternalTableWritten { stage, table } => {
            assert_eq!(stage, "t.w");
            assert_eq!(table, "t.count");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_collection_in_provides_is_an_error() {
    let error = registry_error(vec![feature("t", |b| {
        b.collection(CollectionDef::new("t.all"))?;
        b.stage(provider("t.s", &["t.all"], &[]))
    })]);
    match single(error) {
        BuildError::CollectionAsCapability { stage, site, collection } => {
            assert_eq!(stage, "t.s");
            assert_eq!(site, Site::StageProvides { stage: "t.s".into() });
            assert_eq!(collection, "t.all");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

/// A member of a collection no feature declares is dropped silently: the
/// build succeeds, the collection is absent, and the build diagnostics say
/// nothing about it.
#[test]
fn a_member_of_an_undeclared_collection_is_dropped_silently() {
    let registry = registry(vec![feature("t", |b| {
        b.capability(CapabilityDef::new("t.a"))?;
        b.collection(CollectionDef::new("t.all"))?;
        b.collection_member("t.nope", "t.a")
    })]);
    let identifiers: Vec<&str> =
        registry.build_diagnostics().iter().map(|d| d.identifier()).collect();
    assert_eq!(identifiers, Vec::<&str>::new());
    assert!(registry.collection("t.nope").is_none());
    assert!(registry.collection("t.all").expect("declared").members.is_empty());
}

#[test]
fn an_ordering_edge_on_an_unknown_stage_lists_the_roster() {
    let error = registry_error(vec![feature("t", |b| {
        b.capability(CapabilityDef::new("t.a"))?;
        b.stage(provider("t.s", &["t.a"], &[]))?;
        b.ordering(OrderingDef::new("t.nope").after("t.a"))
    })]);
    match single(error) {
        BuildError::Undeclared { kind, name, roster, .. } => {
            assert_eq!(kind, "stage");
            assert_eq!(name, "t.nope");
            assert_eq!(roster, ["t.s"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// --- rules, backends, outputs --------------------------------------------------------------------

fn rule(name: &str, element: &str, default: DefaultMark) -> LoweringRule {
    common::pipeline::rule(name, element, LoweringDecl::new(), default)
}

fn backend(name: &str, native: &[&str]) -> BackendDef {
    let stage = |suffix: &str| {
        noop(
            StageDecl::new(format!("t.{name}.{suffix}"), Effect::Compute, Scope::Document),
            Needs::new(),
        )
    };
    BackendDef::new(name, native.iter().copied(), stage("lower"), stage("emit"), stage("final"))
}

#[test]
fn two_general_default_rules_for_one_element_is_an_error() {
    let error = registry_error(vec![feature("t", |b| {
        b.element(macro_def("t", "foo", 0))?;
        b.lowering_rule(rule("t.foo.lower.a", "t.foo", DefaultMark::Everywhere))?;
        b.lowering_rule(rule("t.foo.lower.b", "t.foo", DefaultMark::Everywhere))
    })]);
    match single(error) {
        BuildError::TwoDefaultRules { element, backend, first, second } => {
            assert_eq!(element, "t.foo");
            assert_eq!(backend, None);
            assert_eq!(first, "t.foo.lower.a");
            assert_eq!(second, "t.foo.lower.b");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn two_default_rules_for_one_element_on_one_backend_is_an_error() {
    let error = registry_error(vec![feature("t", |b| {
        b.element(macro_def("t", "foo", 0))?;
        b.backend(backend("html", &["flm.text"]))?;
        let on_html = || DefaultMark::OnBackends(vec!["html".into()]);
        b.lowering_rule(rule("t.foo.lower.a", "t.foo", on_html()))?;
        b.lowering_rule(rule("t.foo.lower.b", "t.foo", on_html()))
    })]);
    match single(error) {
        BuildError::TwoDefaultRules { backend, first, second, .. } => {
            assert_eq!(backend.map(|b| b.to_string()), Some("html".to_string()));
            assert_eq!(first, "t.foo.lower.a");
            assert_eq!(second, "t.foo.lower.b");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_rule_for_an_undeclared_element_is_an_error() {
    let error = registry_error(vec![feature("t", |b| {
        b.lowering_rule(rule("t.foo.lower.a", "t.foo", DefaultMark::Never))
    })]);
    match single(error) {
        BuildError::Undeclared { kind, name, .. } => {
            assert_eq!(kind, "element");
            assert_eq!(name, "t.foo");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_backend_native_element_must_be_declared() {
    let error = registry_error(vec![feature("t", |b| b.backend(backend("html", &["t.nope"])))]);
    match single(error) {
        BuildError::Undeclared { kind, name, .. } => {
            assert_eq!(kind, "element");
            assert_eq!(name, "t.nope");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_output_whose_table_is_not_an_artifact_set_is_an_error() {
    let error = registry_error(vec![feature("t", |b| {
        b.table(COUNT, "a count")?;
        b.output(OutputDef::new("count", "t.count", "t.count", "text/plain", "txt"))
    })]);
    match single(error) {
        BuildError::OutputTableNotArtifactSet { output, table, ty } => {
            assert_eq!(output, "count");
            assert_eq!(table, "t.count");
            assert_eq!(ty, "Count");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_output_capability_is_its_table_or_a_refinement_of_it() {
    const ART: TableName<ArtifactSet> = TableName::new("t.art");
    let error = registry_error(vec![feature("t", |b| {
        b.table(ART, "artifacts")?;
        b.capability(CapabilityDef::new("t.other"))?;
        b.output(OutputDef::new("html", "t.other", "t.art", "text/html", "html"))
    })]);
    match single(error) {
        BuildError::OutputCapabilityMismatch { output, capability, table } => {
            assert_eq!(output, "html");
            assert_eq!(capability, "t.other");
            assert_eq!(table, "t.art");
        }
        other => panic!("unexpected error: {other:?}"),
    }
    // A refinement of the table's capability is accepted.
    registry(vec![feature("t", |b| {
        b.table(ART, "artifacts")?;
        b.capability(CapabilityDef::new("t.art.html").refines("t.art"))?;
        b.output(OutputDef::new("html", "t.art.html", "t.art", "text/html", "html"))
    })]);
}

#[test]
fn an_output_is_made_from_declared_outputs_only() {
    const ART: TableName<ArtifactSet> = TableName::new("t.art");
    const PDF: TableName<ArtifactSet> = TableName::new("t.pdf");
    // Declared in either order, the outputs listed are stored as given.
    let built = registry(vec![feature("t", |b| {
        b.table(ART, "artifacts")?;
        b.table(PDF, "the PDF")?;
        b.output(OutputDef::new("pdf", "t.pdf", "t.pdf", "application/pdf", "pdf").made_from("tex").made_from("html"))?;
        b.output(OutputDef::new("tex", "t.art", "t.art", "application/x-latex", "tex"))?;
        b.output(OutputDef::new("html", "t.art", "t.art", "text/html", "html"))
    })]);
    let made_from: Vec<&str> = built.output("pdf").expect("declared").def.made_from.iter().map(|name| name.as_str()).collect();
    assert_eq!(made_from, ["tex", "html"]);
    let error = registry_error(vec![feature("t", |b| {
        b.table(ART, "artifacts")?;
        b.table(PDF, "the PDF")?;
        b.output(OutputDef::new("pdf", "t.pdf", "t.pdf", "application/pdf", "pdf").made_from("txe"))?;
        b.output(OutputDef::new("tex", "t.art", "t.art", "application/x-latex", "tex"))
    })]);
    let single = single(error);
    match &single {
        BuildError::Undeclared { feature, kind, site: Site::OutputMadeFrom { output }, name, roster } => {
            assert_eq!((feature.as_str(), *kind, output.as_str(), name.as_str()), ("t", "output", "pdf", "txe"));
            assert_eq!(roster.iter().map(|name| name.as_str()).collect::<Vec<_>>(), ["pdf", "tex"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(
        single.to_string(),
        "feature `t`: output `pdf` is made from the output `txe`, but no feature declares this output; declared \
         output names: [pdf, tex]"
    );
}

// --- aspects and elements ------------------------------------------------------------------------

trait Tag: Send + Sync {
    fn tag(&self) -> &'static str;
}

struct Tagged(&'static str);

impl Tag for Tagged {
    fn tag(&self) -> &'static str {
        self.0
    }
}

#[test]
fn an_aspect_attaches_to_a_structural_element() {
    let registry =
        registry(vec![feature("t", |b| b.attach::<dyn Tag>("flm.text", Arc::new(Tagged("text"))))]);
    let element = registry.element_named("flm.text").expect("the structural element is registered");
    assert_eq!(element.aspect::<dyn Tag>().map(|a| a.tag()), Some("text"));
    assert_eq!(registry.aspect_bitset::<dyn Tag>().len(), 1);
}

#[test]
fn an_aspect_attaches_to_the_unknown_environment_element() {
    let registry = registry(vec![feature("t", |b| {
        b.attach::<dyn Tag>("flm.unknown.environment", Arc::new(Tagged("unknown")))
    })]);
    let element = registry.element_named("flm.unknown.environment").expect("registered");
    assert_eq!(element.aspect::<dyn Tag>().map(|a| a.tag()), Some("unknown"));
}

#[test]
fn the_registry_language_stages_the_attached_element_value() {
    // The element value carrying the attachment is the one the parser stages.
    let registry = registry(vec![feature("t", |b| {
        b.element(macro_def("t", "foo", 0))?;
        b.attach::<dyn Tag>("t.foo", Arc::new(Tagged("foo")))
    })]);
    let fragment = common::parse(registry.lang(), r"\foo");
    let node = common::callables(&fragment).into_iter().next().expect("the macro parsed");
    let element = flm_core::model::element::element_of(&node);
    assert_eq!(element.aspect::<dyn Tag>().map(|a| a.tag()), Some("foo"));
    assert!(Arc::ptr_eq(element, registry.element_named("t.foo").unwrap()));
}

#[test]
fn attaching_one_aspect_twice_is_a_collision() {
    let error = registry_error(vec![feature("t", |b| {
        b.attach::<dyn Tag>("flm.text", Arc::new(Tagged("one")))?;
        b.attach::<dyn Tag>("flm.text", Arc::new(Tagged("two")))
    })]);
    match single(error) {
        BuildError::AspectCollision { feature, element, .. } => {
            assert_eq!(feature, "t");
            assert_eq!(element, "flm.text");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn attaching_to_an_undeclared_element_lists_the_roster() {
    let error = registry_error(vec![feature("t", |b| {
        b.attach::<dyn Tag>("t.nope", Arc::new(Tagged("x")))
    })]);
    match single(error) {
        BuildError::Undeclared { kind, name, roster, .. } => {
            assert_eq!(kind, "element");
            assert_eq!(name, "t.nope");
            assert!(roster.iter().any(|n| n == "flm.text"), "{roster:?}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[derive(Debug)]
struct Payload;

impl PresPayload for Payload {
    fn project(&self) -> SerialValue {
        SerialValue::Null
    }
}

#[test]
fn source_and_structural_elements_are_occurrable_but_not_presentation_ones() {
    let registry = registry(vec![feature("t", |b| {
        b.element(macro_def("t", "foo", 0))?;
        b.presentation_element::<Payload>("t.pres", "a presentation element")?;
        Ok(())
    })]);
    let occurrable = registry.occurrable_elements();
    assert!(occurrable.contains("t.foo"));
    assert!(occurrable.contains("flm.text"));
    assert!(occurrable.contains("flm.unknown.macro"));
    assert!(occurrable.contains("flm.unknown.environment"));
    assert!(occurrable.contains("flm.parbreak"));
    assert!(!occurrable.contains("flm.placeholder"));
    assert!(!occurrable.contains("t.pres"));
    assert!(registry.presentation_element("t.pres").is_some());
}

#[test]
fn a_presentation_element_reusing_a_source_name_names_both_owners() {
    let error = registry_error(vec![
        feature("t.source", |b| b.element(macro_def("t", "foo", 0))),
        feature("t.pres", |b| b.presentation_element::<Payload>("t.foo", "reused").map(|_| ())),
    ]);
    match single(error) {
        BuildError::Duplicate { kind, name, first, second } => {
            assert_eq!(kind, "element");
            assert_eq!(name, "t.foo");
            assert_eq!(first, "t.source");
            assert_eq!(second, "t.pres");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// --- the element resolver seam -------------------------------------------------------------------

/// Two declarations sharing one element type name (two `flm.heading` values).
fn two_headings(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    let heading = |name: &str| {
        ElementDef::new("t", [name], SyntaxDef::Macro { args: Vec::new() }, element("t.heading"))
    };
    b.element(heading("section"))?;
    b.element(heading("subsection"))
}

#[test]
fn a_name_with_one_value_resolves_to_it() {
    let registry = registry(vec![feature("t", |b| b.element(macro_def("t", "foo", 0)))]);
    let value = registry.element_value("t.foo").expect("one value").expect("registered");
    assert!(Arc::ptr_eq(value, registry.element_named("t.foo").unwrap()));
}

#[test]
fn an_unregistered_name_resolves_to_none() {
    let registry = registry(Vec::new());
    assert!(registry.element_value("t.nope").expect("no ambiguity").is_none());
}

#[test]
fn the_registry_is_the_read_sides_element_resolver() {
    // The registry behind the `ElementResolver` door answers one value, none,
    // or the typed ambiguity naming each value's owner.
    let registry = registry(vec![feature("t", |b| {
        b.element(macro_def("t", "foo", 0))?;
        two_headings(b)
    })]);
    let resolver: Arc<dyn ElementResolver> = Arc::clone(&registry) as Arc<dyn ElementResolver>;
    let foo = resolver.resolve_element("t.foo").expect("one value").expect("registered");
    assert!(Arc::ptr_eq(&foo, registry.element_named("t.foo").unwrap()));
    assert!(resolver.resolve_element("t.nope").expect("no ambiguity").is_none());
    let error = resolver.resolve_element("t.heading").expect_err("two values");
    assert_eq!(error.owners, ["t", "t"]);
}

#[test]
fn a_written_fragment_reads_back_through_a_reader_over_the_registry() {
    let registry = registry(vec![feature("t", |b| b.element(macro_def("t", "foo", 0)))]);
    let fragment = Arc::new(common::parse(registry.lang(), r"\foo"));
    let (_, segment) = common::dump::write(registry.lang(), &fragment);
    let resolver: Arc<dyn ElementResolver> = Arc::clone(&registry) as Arc<dyn ElementResolver>;
    let mut reader = session::reader(registry.lang(), resolver).expect("the reader prepares");
    let pushed = reader.push_segment(segment).expect("a sound segment");
    let (table, index) = pushed.expect("a main entry");
    let fragments = session::fragments_table(&reader).expect("the fragments table");
    assert_eq!(table, fragments.id());
    let read = session::read_fragment(&mut reader, fragments.position(index)).expect("reads back");
    let node = common::callables(&read).into_iter().next().expect("the macro parsed");
    let element = flm_core::model::element::element_of(&node);
    assert!(Arc::ptr_eq(element, registry.element_named("t.foo").unwrap()));
}

#[test]
fn a_name_with_several_values_is_a_typed_ambiguity() {
    let registry = registry(vec![feature("t", two_headings)]);
    assert_eq!(registry.elements_named("t.heading").len(), 2);
    let error = registry.element_value("t.heading").expect_err("two values");
    assert_eq!(error.name, "t.heading");
    assert_eq!(error.owners, ["t", "t"]);
}

// --- per-phase error collection ------------------------------------------------------------------

#[test]
fn a_feature_refusing_to_register_is_attributed_in_the_collect_phase() {
    let error = registry_error(vec![feature("t", |b| {
        Err(BuildError::Feature { feature: b.feature().clone(), detail: "no".to_string() })
    })]);
    assert_eq!(error.phase, BuildPhase::Collect);
    match single(error) {
        BuildError::Feature { feature, detail } => {
            assert_eq!(feature, "t");
            assert_eq!(detail, "no");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn independent_collect_errors_are_reported_together() {
    let error = registry_error(vec![
        feature("t.a", |b| {
            Err(BuildError::Feature { feature: b.feature().clone(), detail: "no".into() })
        }),
        feature("t.b", |b| b.contribute::<String>(Arc::new("orphan".to_string()))),
    ]);
    assert_eq!(error.phase, BuildPhase::Collect);
    assert_eq!(error.errors.len(), 2, "{}", error.render());
    assert!(matches!(error.errors[0], BuildError::Feature { .. }));
    assert!(matches!(error.errors[1], BuildError::Service(_)));
}

struct Flag(Arc<AtomicBool>);

impl Finalizer for Flag {
    fn finalize(&self, _b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        self.0.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn collect_errors_stop_the_build_before_round_two() {
    let ran = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&ran);
    let error = registry_error(vec![feature("t", move |b| {
        b.finalizer(Arc::new(Flag(Arc::clone(&flag))))?;
        Err(BuildError::Feature { feature: b.feature().clone(), detail: "no".into() })
    })]);
    assert_eq!(error.phase, BuildPhase::Collect);
    assert!(!ran.load(Ordering::SeqCst), "the finalizer must not run");
}

#[test]
fn independent_validate_errors_are_reported_together() {
    let error = registry_error(vec![feature("t", |b| {
        b.capability(CapabilityDef::new("a"))?;
        b.capability(CapabilityDef::new("a"))?;
        b.stage(provider("s", &["nope"], &[]))
    })]);
    assert_eq!(error.phase, BuildPhase::Validate);
    assert_eq!(error.errors.len(), 2, "{}", error.render());
    assert!(matches!(error.errors[0], BuildError::Duplicate { .. }));
    assert!(matches!(error.errors[1], BuildError::Undeclared { .. }));
}

#[test]
fn duplicate_feature_names_are_an_error() {
    let error = registry_error(vec![feature("t", |_| Ok(())), feature("t", |_| Ok(()))]);
    assert!(
        error.errors.iter().any(|e| matches!(e, BuildError::DuplicateFeature { .. })),
        "{}",
        error.render()
    );
}

#[test]
fn a_callable_clash_between_features_is_a_language_phase_error() {
    // Two features define `\foo` and neither declares an override: the
    // language build reports the clash (naming both owners, the language
    // layer's contract) and the registry build stops in the language phase.
    let error = registry_error(vec![
        feature("t.first", |b| b.element(macro_def("t.first", "foo", 0))),
        feature("t.second", |b| b.element(macro_def("t.second", "foo", 0))),
    ]);
    assert_eq!(error.phase, BuildPhase::Language);
    match single(error) {
        BuildError::Lang(error) => {
            let rendered = error.to_string();
            assert!(rendered.contains("t.first") && rendered.contains("t.second"), "{rendered}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// --- late init, digest, packages, lookup ---------------------------------------------------------

struct SeesLanguage(Arc<AtomicBool>);

impl LateInit for SeesLanguage {
    fn late_init(
        &self,
        lang: &BuiltLang,
        _diags: &mut FlmDiagnostics,
    ) -> Result<(), LateInitError> {
        self.0.store(lang.is_sound(), Ordering::SeqCst);
        Ok(())
    }
}

struct Refuses;

impl LateInit for Refuses {
    fn late_init(
        &self,
        _lang: &BuiltLang,
        _diags: &mut FlmDiagnostics,
    ) -> Result<(), LateInitError> {
        Err(LateInitError::new("template did not compile"))
    }
}

/// A hook reporting one Error-severity diagnostic about the configuration it
/// initializes, without failing.
struct Reports;

impl LateInit for Reports {
    fn late_init(
        &self,
        _lang: &BuiltLang,
        diags: &mut FlmDiagnostics,
    ) -> Result<(), LateInitError> {
        diags.push(Diagnostic::error(Note { detail: "a template problem".into() }, span()));
        Ok(())
    }
}

#[test]
fn a_late_init_hooks_diagnostics_are_the_registrys_build_diagnostics() {
    // The build succeeds; what the hook reported is readable on the registry,
    // Error severity included.
    let registry = registry(vec![feature("t", |b| b.late_init(Arc::new(Reports)))]);
    let identifiers: Vec<&str> =
        registry.build_diagnostics().iter().map(|d| d.identifier()).collect();
    assert_eq!(identifiers, ["t.note"]);
    assert!(registry.build_diagnostics().has_errors());
}

#[test]
fn a_late_init_hook_runs_over_the_built_language() {
    let seen = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&seen);
    registry(vec![feature("t", move |b| b.late_init(Arc::new(SeesLanguage(Arc::clone(&flag)))))]);
    assert!(seen.load(Ordering::SeqCst));
}

#[test]
fn a_failing_late_init_hook_is_a_typed_error_in_its_phase() {
    let error = registry_error(vec![feature("t", |b| b.late_init(Arc::new(Refuses)))]);
    assert_eq!(error.phase, BuildPhase::LateInit);
    match single(error) {
        BuildError::LateInit { feature, detail } => {
            assert_eq!(feature, "t");
            assert_eq!(detail, "template did not compile");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

fn caps_feature(
    name: &'static str,
    caps: &'static [&'static str],
) -> Arc<dyn flm_core::core::registry::FlmFeature> {
    feature(name, move |b| {
        for cap in caps {
            b.capability(CapabilityDef::new(*cap))?;
        }
        Ok(())
    })
}

#[test]
fn the_digest_is_stable_across_builds() {
    let first = registry(vec![caps_feature("t", &["t.a", "t.b"])]);
    let second = registry(vec![caps_feature("t", &["t.a", "t.b"])]);
    assert_eq!(first.digest(), second.digest());
}

#[test]
fn the_digest_changes_with_an_element_declaration() {
    // Element declarations are semantic build inputs; the compute roster —
    // capabilities, stages — deliberately stays out of the digest.
    // See design rationale at [§flm-dd-dr:feat-digest].
    let first = registry(vec![feature("t", |b| b.element(macro_def("t", "foo", 0)))]);
    let second = registry(vec![feature("t", |b| {
        b.element(macro_def("t", "foo", 0))?;
        b.element(macro_def("t", "bar", 0))
    })]);
    assert_ne!(first.digest(), second.digest());
}

#[test]
fn the_digest_tells_argument_shapes_apart() {
    let with = |kind: ArgStdKind| {
        registry(vec![feature("t", move |b| {
            b.element(macro_with("t", "foo", vec![ArgDef::new(kind.clone())]))
        })])
    };
    assert_ne!(with(ArgStdKind::Mandatory).digest(), with(ArgStdKind::Optional).digest());
    assert_eq!(with(ArgStdKind::Mandatory).digest(), with(ArgStdKind::Mandatory).digest());
}

#[test]
fn the_digest_tells_an_arguments_mode_and_layout_apart() {
    // Two declarations differing only in how an argument is read or laid out
    // produce different trees, so they must not share cached data.
    let with = |arg: fn() -> ArgDef| {
        registry(vec![feature("t", move |b| b.element(macro_with("t", "foo", vec![arg()])))])
            .digest()
    };
    let plain = with(|| ArgDef::mandatory("x"));
    assert_ne!(plain, with(|| ArgDef::mandatory("x").mode(Mode::Math)));
    assert_ne!(plain, with(|| ArgDef::mandatory("x").layout(ArgLayout::Blocks)));
    assert_ne!(
        with(|| ArgDef::mandatory("x").layout(ArgLayout::Blocks)),
        with(|| ArgDef::mandatory("x").layout(ArgLayout::InlineOrBlocks))
    );
    assert_eq!(plain, with(|| ArgDef::mandatory("x")));
}

#[test]
fn the_digest_ignores_the_compute_roster() {
    let first = registry(vec![caps_feature("t", &["t.a"])]);
    let second = registry(vec![caps_feature("t", &["t.a", "t.b"])]);
    assert_eq!(first.digest(), second.digest());
}

#[test]
fn the_build_identity_enters_the_digest() {
    let build = |identity: &str| {
        let mut inputs = RegistryInputs::new();
        inputs.build_identity = Some(identity.to_string());
        let mut builder = RegistryBuilder::new();
        builder.feature(caps_feature("t", &["t.a"]));
        builder.build(&inputs).expect("builds")
    };
    assert_ne!(build("v1").digest(), build("v2").digest());
}

#[test]
fn a_package_used_twice_registers_its_features_once() {
    let package = |name: &str| PackageEntry::new(name).feature(caps_feature("t.a", &["t.a"]));
    let mut builder = RegistryBuilder::new();
    builder.use_package(package("p")).use_package(package("p"));
    let registry = builder.build(&RegistryInputs::new()).expect("deduplicated by package name");
    assert_eq!(registry.features().len(), 1);
}

#[test]
fn a_package_brings_the_packages_it_uses() {
    let inner = PackageEntry::new("q").feature(caps_feature("t.b", &["t.b"]));
    let outer = PackageEntry::new("p").feature(caps_feature("t.a", &["t.a"])).uses(inner);
    let mut builder = RegistryBuilder::new();
    builder.use_package(outer);
    let registry = builder.build(&RegistryInputs::new()).expect("both register");
    assert!(registry.has_feature("t.a"));
    assert!(registry.has_feature("t.b"));
}

#[test]
fn features_are_looked_up_by_name() {
    let registry = registry(vec![caps_feature("t.a", &["t.a"])]);
    assert!(registry.has_feature("t.a"));
    assert!(!registry.has_feature("t.b"));
    assert_eq!(registry.feature("t.a").map(|info| info.title.as_str()), Some("t.a"));
}

#[test]
fn the_shared_values_are_send_and_sync() {
    // The registry crosses threads behind an `Arc` (the runner's pool, the
    // serialization reader), a plan is reused across runs,
    // a store version and the services are read by every stage, a stamp
    // travels with an install.
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<flm_core::core::registry::FlmRegistry>();
    assert_send_sync::<flm_core::core::plan::Plan>();
    assert_send_sync::<flm_core::core::data::store::Computed>();
    assert_send_sync::<flm_core::core::services::Services>();
    assert_send_sync::<flm_core::core::run::Stamp>();
}

#[test]
fn a_registry_without_features_builds() {
    let registry = registry(Vec::new());
    assert!(registry.features().is_empty());
    assert!(registry.capabilities().is_empty());
    assert!(registry.lang().is_sound());
}

// --- native-set implications ---------------------------------------------------------------------

/// The `lowering` feature — the presentation vocabulary and its two native-set
/// implications — and the `blocks` feature its default rules require, beside a
/// toy feature registering one backend `html` that claims `native`.
fn with_backend(native: &'static [&'static str]) -> Vec<Arc<dyn FlmFeature>> {
    let mut features = shared();
    features.push(feature("t", move |b| b.backend(backend("html", native))));
    features
}

/// The `lowering` and `blocks` features alone.
fn shared() -> Vec<Arc<dyn FlmFeature>> {
    vec![lowering::feature(), Arc::new(Blocks::default())]
}

/// The one `NativeSetIncomplete` error of a failed build, as its three parts.
fn incomplete(
    error: flm_core::core::registry::RegistryBuildError,
) -> (String, Option<String>, String) {
    assert_eq!(error.phase, BuildPhase::Validate);
    match single(error) {
        BuildError::NativeSetIncomplete { backend, because, must_claim } => (
            backend.to_string(),
            because.map(|element| element.to_string()),
            must_claim.to_string(),
        ),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_backend_claiming_a_decoration_without_the_marker_is_refused() {
    let error = registry_error(with_backend(&[pres::DECO_RUNIN, pres::UNSUPPORTED]));
    let (backend, because, must_claim) = incomplete(error);
    assert_eq!(backend, "html");
    assert_eq!(because.as_deref(), Some(pres::DECO_RUNIN));
    assert_eq!(must_claim, pres::DECO_MARK);
}

#[test]
fn a_backend_claiming_the_marker_beside_its_decoration_builds() {
    let registry =
        registry(with_backend(&[pres::DECO_RUNIN, pres::DECO_MARK, pres::UNSUPPORTED]));
    assert!(registry.backend("html").is_some());
}

#[test]
fn a_backend_not_claiming_the_unsupported_stand_in_is_refused() {
    let error = registry_error(with_backend(&["flm.text"]));
    let (backend, because, must_claim) = incomplete(error);
    assert_eq!(backend, "html");
    assert_eq!(because, None);
    assert_eq!(must_claim, pres::UNSUPPORTED);
}

#[test]
fn one_missing_marker_is_reported_once_naming_the_first_decoration_claimed() {
    let error = registry_error(with_backend(&[
        pres::DECO_FINAL,
        pres::DECO_MARGIN,
        pres::DECO_RUNIN,
        pres::UNSUPPORTED,
    ]));
    let (_, because, must_claim) = incomplete(error);
    assert_eq!(because.as_deref(), Some(pres::DECO_FINAL));
    assert_eq!(must_claim, pres::DECO_MARK);
}

#[test]
fn every_backend_is_held_against_every_implication_and_the_violations_are_reported_together() {
    let mut features = shared();
    features.push(feature("t.one", |b| b.backend(backend("a", &[pres::DECO_RUNIN]))));
    features.push(feature("t.two", |b| b.backend(backend("b", &["flm.text"]))));
    let error = registry_error(features);
    assert_eq!(error.phase, BuildPhase::Validate);
    let found: Vec<(String, Option<String>, String)> = error
        .errors
        .into_iter()
        .map(|e| match e {
            BuildError::NativeSetIncomplete { backend, because, must_claim } => (
                backend.to_string(),
                because.map(|element| element.to_string()),
                must_claim.to_string(),
            ),
            other => panic!("unexpected error: {other:?}"),
        })
        .collect();
    let s = |text: &str| text.to_string();
    assert_eq!(
        found,
        [
            (s("a"), Some(s(pres::DECO_RUNIN)), s(pres::DECO_MARK)),
            (s("a"), None, s(pres::UNSUPPORTED)),
            (s("b"), None, s(pres::UNSUPPORTED)),
        ]
    );
}

#[test]
fn an_implication_naming_an_undeclared_element_lists_the_roster() {
    let error = registry_error(vec![feature("t", |b| {
        b.element(macro_def("t", "foo", 0))?;
        b.native_implication(NativeImplication::always("t.nope"))
    })]);
    match single(error) {
        BuildError::Undeclared { feature, kind, site, name, roster } => {
            assert_eq!(feature, "t");
            assert_eq!(kind, "element");
            assert_eq!(site, Site::NativeImplication);
            assert_eq!(name, "t.nope");
            assert!(roster.iter().any(|declared| declared == "t.foo"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_implication_naming_an_undeclared_element_is_reported_once_even_with_a_backend() {
    // The backend cannot claim `t.nope`, since it does not exist; the one
    // `Undeclared` error is the whole report, not one `NativeSetIncomplete`
    // per backend on top of it.
    let error = registry_error(vec![feature("t", |b| {
        b.element(macro_def("t", "foo", 0))?;
        b.native_implication(NativeImplication::always("t.nope"))?;
        b.backend(backend("html", &["t.foo"]))
    })]);
    match single(error) {
        BuildError::Undeclared { site, name, .. } => {
            assert_eq!(site, Site::NativeImplication);
            assert_eq!(name, "t.nope");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

/// A toy feature declaring `t.foo` carrying the `Tag` aspect, `t.bar` and
/// `t.mark` without it, and the implication that claiming a tagged element
/// requires claiming `t.mark`, beside a backend claiming `native`.
fn tagged_implication(native: &'static [&'static str]) -> Arc<dyn FlmFeature> {
    feature("t", move |b| {
        b.element(macro_def("t", "foo", 0))?;
        b.element(macro_def("t", "bar", 0))?;
        b.element(macro_def("t", "mark", 0))?;
        b.attach::<dyn Tag>("t.foo", Arc::new(Tagged("foo")))?;
        b.native_implication(NativeImplication::aspect::<dyn Tag>("t.mark"))?;
        b.backend(backend("html", native))
    })
}

#[test]
fn an_aspect_implication_is_brought_into_force_by_the_aspect_not_by_a_name() {
    let error = registry_error(vec![tagged_implication(&["t.foo"])]);
    let (backend, because, must_claim) = incomplete(error);
    assert_eq!(backend, "html");
    assert_eq!(because.as_deref(), Some("t.foo"));
    assert_eq!(must_claim, "t.mark");
}

#[test]
fn an_aspect_implication_leaves_a_backend_claiming_no_carrier_alone() {
    let registry = registry(vec![tagged_implication(&["t.bar"])]);
    assert!(registry.backend("html").is_some());
}

#[test]
fn an_aspect_another_feature_attaches_brings_the_implication_into_force() {
    // The check runs after attachments are merged, so the aspect `t.two`
    // attaches to `t.one`'s element counts against `t.one`'s backend.
    let error = registry_error(vec![
        feature("t.one", |b| {
            b.element(macro_def("t", "foo", 0))?;
            b.element(macro_def("t", "mark", 0))?;
            b.backend(backend("html", &["t.foo"]))
        }),
        feature("t.two", |b| {
            b.attach::<dyn Tag>("t.foo", Arc::new(Tagged("foo")))?;
            b.native_implication(NativeImplication::aspect::<dyn Tag>("t.mark"))
        }),
    ]);
    let (_, because, must_claim) = incomplete(error);
    assert_eq!(because.as_deref(), Some("t.foo"));
    assert_eq!(must_claim, "t.mark");
}

#[test]
fn the_standard_package_declares_the_two_implications_and_builds() {
    let registry = common::stdcore::registry(StdcoreConfig::new());
    let listed: Vec<(&str, &str, bool)> = registry
        .native_implications()
        .iter()
        .map(|(owner, implication)| {
            (
                owner.as_str(),
                implication.must_claim.as_str(),
                implication.kind == NativeImplicationKind::Always,
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            (lowering::FEATURE_NAME, pres::DECO_MARK, false),
            (lowering::FEATURE_NAME, pres::UNSUPPORTED, true),
        ]
    );
}

// --- peer declarations ------------------------------------------------------------------------

/// A second payload type, for the signature mismatch.
#[derive(Debug)]
struct OtherPayload;

impl PresPayload for OtherPayload {
    fn project(&self) -> SerialValue {
        SerialValue::Null
    }
}

/// The name of the toy rule that is the default on the backend `html` and
/// produces `t.pres.box`, which its feature does not register.
const BOX_RULE: &str = "t.foo.lower.html";

/// A feature `t.rule` owning the element `t.foo` and a rule for it that is
/// the default on the backend `html` and produces the presentation element
/// `t.pres.box` — both declared as peer declarations, since the feature
/// registers neither.
fn rule_feature() -> Arc<dyn FlmFeature> {
    feature("t.rule", |b| {
        b.element(macro_def("t", "foo", 0))?;
        b.peer_backend("html", "the backend the rule is the default on")?;
        b.peer_element::<Payload>("t.pres.box", "the box the rule stages")?;
        b.lowering_rule(common::pipeline::rule(
            BOX_RULE,
            "t.foo",
            LoweringDecl::new().produces("t.pres.box"),
            DefaultMark::OnBackends(vec!["html".into()]),
        ))
    })
}

/// A feature `t.backend` registering the presentation element `t.pres.box`
/// with payload type `P` and the backend `html` claiming it.
fn backend_feature<P: PresPayload>() -> Arc<dyn FlmFeature> {
    feature("t.backend", |b| {
        b.presentation_element::<P>("t.pres.box", "a box")?;
        register_backend(b, "html", &["t.pres.box"])
    })
}

/// The identifiers of a registry's build diagnostics.
fn build_diagnostic_ids(registry: &FlmRegistry) -> Vec<&str> {
    registry.build_diagnostics().iter().map(|d| d.identifier()).collect()
}

#[test]
fn a_peer_backend_and_element_resolve_when_a_feature_declares_them() {
    let registry = registry(vec![rule_feature(), backend_feature::<Payload>()]);
    assert!(registry.build_diagnostics().is_empty());
    assert_eq!(registry.set_aside_rules().count(), 0);
    let backends: Vec<(&str, &str, bool)> = registry
        .peer_backends()
        .iter()
        .map(|entry| (entry.owner.as_str(), entry.name.as_str(), entry.resolved))
        .collect();
    assert_eq!(backends, [("t.rule", "html", true)]);
    let elements: Vec<(&str, &str, bool)> = registry
        .peer_elements()
        .iter()
        .map(|entry| (entry.owner.as_str(), entry.name.as_str(), entry.resolved))
        .collect();
    assert_eq!(elements, [("t.rule", "t.pres.box", true)]);
    assert!(registry.peer_elements()[0].payload.is::<Payload>());
    // The rule takes part: the plan for `html` selects it for `t.foo`.
    let plan = plan_for(&registry, &PlanRequest::new().demand(lowered_cap("html")));
    let selected = plan.selection("html").expect("html is active");
    let chosen = selected.rule_for("t.foo").expect("the rule is selected");
    assert_eq!(registry.rule(chosen).rule.name, BOX_RULE);
    assert_eq!(registry.rule(chosen).owner, "t.rule");
}

#[test]
fn an_unresolved_peer_declaration_is_a_note_and_sets_the_rule_aside() {
    let registry = registry(vec![rule_feature()]);
    assert_eq!(
        build_diagnostic_ids(&registry),
        ["flm.registry.unresolved-peer", "flm.registry.unresolved-peer"]
    );
    assert!(!registry.build_diagnostics().has_errors());
    let messages: Vec<String> =
        registry.build_diagnostics().iter().map(|d| d.message()).collect();
    assert_eq!(
        messages[0],
        "feature `t.rule` refers to the backend `html` and expects another feature to declare it, but no feature of this registry declares it; the rules depending on it are set aside: [t.foo.lower.html]"
    );
    assert!(messages[1].contains("refers to the element `t.pres.box`"), "{}", messages[1]);
    let set_aside: Vec<(&str, Vec<PeerName>)> = registry
        .set_aside_rules()
        .map(|entry| (entry.rule.name.as_str(), entry.set_aside.clone()))
        .collect();
    assert_eq!(
        set_aside,
        [(
            BOX_RULE,
            vec![PeerName::Backend("html".into()), PeerName::Element("t.pres.box".into())]
        )]
    );
    assert!(registry.rule(registry.rule_idx(BOX_RULE).unwrap()).is_set_aside());
    assert!(!registry.peer_backends()[0].resolved);
    assert!(!registry.peer_elements()[0].resolved);
}

#[test]
fn an_unresolved_name_is_noted_once_however_many_rules_refer_to_it() {
    let registry = registry(vec![feature("t.rule", |b| {
        b.element(macro_def("t", "foo", 0))?;
        b.element(macro_def("t", "bar", 0))?;
        b.peer_backend("html", "declared once, referred to twice")?;
        b.lowering_rule(rule(
            "t.foo.lower.html",
            "t.foo",
            DefaultMark::OnBackends(vec!["html".into()]),
        ))?;
        b.lowering_rule(rule("t.bar.lower.html", "t.bar", DefaultMark::OnBackends(vec!["html".into()])))
    })]);
    assert_eq!(build_diagnostic_ids(&registry), ["flm.registry.unresolved-peer"]);
    let names: Vec<&str> = registry.set_aside_rules().map(|e| e.rule.name.as_str()).collect();
    assert_eq!(names, ["t.foo.lower.html", "t.bar.lower.html"]);
    assert_eq!(registry.peer_backends().len(), 1);
}

#[test]
fn a_peer_element_value_is_kept_where_a_feature_declares_its_element_type_and_dropped_elsewhere() {
    // `t.value` brings two commands whose element type, `t.owner.kind`, the
    // feature `t.owner` declares, as a type with no command of its own.
    let value = || {
        feature("t.value", |b| {
            for name in ["extra", "more"] {
                b.peer_element_value(ElementDef::new(
                    "t.value",
                    [name],
                    SyntaxDef::Macro { args: Vec::new() },
                    element("t.owner.kind"),
                ))?;
            }
            Ok(())
        })
    };
    let owner = feature("t.owner", |b| {
        b.element(ElementDef::new("t.owner", Vec::<&str>::new(), SyntaxDef::Synthesized, element("t.owner.kind")))
    });
    let values_of = |registry: &FlmRegistry| -> Vec<(String, bool)> {
        registry
            .peer_element_values()
            .iter()
            .map(|entry| (entry.names.iter().map(ToString::to_string).collect(), entry.resolved))
            .collect()
    };
    let kinds = |registry: &FlmRegistry| {
        registry.elements().iter().filter(|entry| entry.element.name.as_str() == "t.owner.kind").count()
    };
    // Alone: one Note for the element type, both declarations dropped.
    let alone = registry(vec![value()]);
    assert_eq!(build_diagnostic_ids(&alone), ["flm.registry.unresolved-peer"]);
    let message = alone.build_diagnostics().iter().next().expect("one").to_string();
    assert!(message.contains("refers to the element type `t.owner.kind`"), "{message}");
    assert_eq!(values_of(&alone), [("extra".to_string(), false), ("more".to_string(), false)]);
    assert_eq!(kinds(&alone), 0);
    // Beside the owner: kept, attributed to the declaring feature.
    let both = registry(vec![value(), owner]);
    assert_eq!(build_diagnostic_ids(&both), Vec::<&str>::new());
    assert_eq!(values_of(&both), [("extra".to_string(), true), ("more".to_string(), true)]);
    assert_eq!(kinds(&both), 3);
    assert!(both.elements().iter().filter(|entry| entry.owner.as_str() == "t.value").count() == 2);
    // An owner declaring the type only inside a scoped package declares it
    // too: the values are kept.
    let scoped_owner = feature("t.owner", |b| {
        b.scoped_package(ScopedPackageDef {
            owner: "t.owner".into(),
            name: "t.owner.inner".into(),
            elements: vec![ElementDef::new(
                "t.owner",
                ["inner"],
                SyntaxDef::Macro { args: Vec::new() },
                element("t.owner.kind"),
            )],
            docs: String::new(),
        })
    });
    let scoped = registry(vec![value(), scoped_owner]);
    assert_eq!(build_diagnostic_ids(&scoped), Vec::<&str>::new());
    assert_eq!(values_of(&scoped), [("extra".to_string(), true), ("more".to_string(), true)]);
}

#[test]
fn an_identical_repeated_peer_declaration_is_merged_into_the_first() {
    // A peer declaration registers nothing, so one feature declaring the same
    // name again with the same signature changes nothing: one listed
    // declaration each, the first docs kept, one Note each.
    let registry = registry(vec![feature("t.rule", |b| {
        b.peer_backend("html", "declared once")?;
        b.peer_element::<Payload>("t.pres.box", "declared once")?;
        b.peer_table::<common::pipeline::Count>(COUNT.name(), "declared once")?;
        b.peer_scoped_package("t.label", "declared once")?;
        b.peer_backend("html", "declared twice")?;
        b.peer_element::<Payload>("t.pres.box", "declared twice")?;
        b.peer_table::<common::pipeline::Count>(COUNT.name(), "declared twice")?;
        b.peer_scoped_package("t.label", "declared twice")
    })]);
    assert_eq!(build_diagnostic_ids(&registry), ["flm.registry.unresolved-peer"; 4]);
    let docs: Vec<&str> = registry
        .peer_backends()
        .iter()
        .map(|e| e.docs.as_str())
        .chain(registry.peer_elements().iter().map(|e| e.docs.as_str()))
        .chain(registry.peer_tables().iter().map(|e| e.docs.as_str()))
        .chain(registry.peer_scoped_packages().iter().map(|e| e.docs.as_str()))
        .collect();
    let once = if cfg!(feature = "docs") { "declared once" } else { "" };
    assert_eq!(docs, [once; 4]);
}

#[test]
fn a_repeated_peer_declaration_under_another_signature_is_a_duplicate() {
    // The same element or table declared again by one feature with another
    // payload or value type is refused, as a repeated declaration in any other
    // namespace; the errors name the feature on both sides.
    let error = registry_error(vec![feature("t.rule", |b| {
        b.peer_element::<Payload>("t.pres.box", "declared once")?;
        b.peer_element::<OtherPayload>("t.pres.box", "declared again, another payload")?;
        b.peer_table::<common::pipeline::Count>(COUNT.name(), "declared once")?;
        b.peer_table::<common::pipeline::Log>(COUNT.name(), "declared again, another value")
    })]);
    assert_eq!(error.phase, BuildPhase::Validate);
    let found: Vec<(&str, String, String, String)> = error
        .errors
        .iter()
        .map(|e| match e {
            BuildError::Duplicate { kind, name, first, second } => {
                (*kind, name.to_string(), first.to_string(), second.to_string())
            }
            other => panic!("unexpected error: {other:?}"),
        })
        .collect();
    let feature = "t.rule".to_string();
    assert_eq!(
        found,
        [
            ("peer element", "t.pres.box".to_string(), feature.clone(), feature.clone()),
            ("peer table", "t.count".to_string(), feature.clone(), feature),
        ]
    );
}

#[test]
fn two_features_may_each_declare_the_same_peer_name() {
    // A declaration counts for its own feature alone, so two features each
    // declaring `html` is no duplicate: each gets its own Note and its own
    // listed declaration.
    let registry = registry(vec![
        rule_feature(),
        feature("t.other", |b| {
            b.element(macro_def("t", "bar", 0))?;
            b.peer_backend("html", "declared by the other feature too")?;
            b.lowering_rule(rule(
                "t.bar.lower.html",
                "t.bar",
                DefaultMark::OnBackends(vec!["html".into()]),
            ))
        }),
    ]);
    assert_eq!(build_diagnostic_ids(&registry).len(), 3);
    let backends: Vec<(&str, &str)> = registry
        .peer_backends()
        .iter()
        .map(|entry| (entry.owner.as_str(), entry.name.as_str()))
        .collect();
    assert_eq!(backends, [("t.rule", "html"), ("t.other", "html")]);
    let names: Vec<&str> = registry.set_aside_rules().map(|e| e.rule.name.as_str()).collect();
    assert_eq!(names, [BOX_RULE, "t.bar.lower.html"]);
}

#[test]
fn a_peer_element_with_the_wrong_payload_type_is_refused() {
    let error = registry_error(vec![rule_feature(), backend_feature::<OtherPayload>()]);
    assert_eq!(error.phase, BuildPhase::Validate);
    match single(error) {
        BuildError::PeerElementMismatch { feature, element, declared, expected } => {
            assert_eq!(feature, "t.rule");
            assert_eq!(element, "t.pres.box");
            assert_eq!(declared, Some("OtherPayload"));
            assert_eq!(expected, "Payload");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_peer_element_naming_a_source_element_is_refused() {
    let error = registry_error(vec![feature("t", |b| {
        b.element(macro_def("t", "foo", 0))?;
        b.peer_element::<Payload>("t.foo", "not a presentation element")
    })]);
    match single(error) {
        BuildError::PeerElementMismatch { element, declared, expected, .. } => {
            assert_eq!(element, "t.foo");
            assert_eq!(declared, None);
            assert_eq!(expected, "Payload");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_peer_declaration_counts_for_its_own_feature_only() {
    // `t.other`'s rule names the backend `t.rule` declared as a peer; that
    // declaration is not `t.other`'s, so the reference is undeclared as before.
    let error = registry_error(vec![
        rule_feature(),
        feature("t.other", |b| {
            b.element(macro_def("t", "bar", 0))?;
            b.lowering_rule(rule(
                "t.bar.lower.html",
                "t.bar",
                DefaultMark::OnBackends(vec!["html".into()]),
            ))
        }),
    ]);
    match single(error) {
        BuildError::Undeclared { feature, kind, site, name, .. } => {
            assert_eq!(feature, "t.other");
            assert_eq!(kind, "backend");
            assert_eq!(site, Site::RuleDefaultMark { rule: "t.bar.lower.html".into() });
            assert_eq!(name, "html");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// --- peer tables ---------------------------------------------------------------------------

/// The name of the toy rule that reads the table `t.count` as a hard
/// dependency, which no feature of its own registry declares.
const READS_RULE: &str = "t.foo.lower.reads";

/// The name of the toy rule that reads the same table optionally.
const READS_OPT_RULE: &str = "t.foo.lower.reads-opt";

/// A feature `t.reader` owning `t.bar` and two rules for it, both marked
/// `Never` so that neither is a default: one reads the refers to the table `t.count` as
/// a hard dependency, the other reads it optionally. The feature declares the
/// table as a peer table; `t.count` holds a [`Count`](common::pipeline::Count).
fn reader_feature() -> Arc<dyn FlmFeature> {
    feature("t.reader", |b| {
        b.element(macro_def("t", "bar", 0))?;
        b.peer_table::<common::pipeline::Count>(COUNT.name(), "the count another feature writes")?;
        b.lowering_rule(common::pipeline::rule(
            READS_RULE,
            "t.bar",
            LoweringDecl::new().reads(COUNT),
            DefaultMark::Never,
        ))?;
        b.lowering_rule(common::pipeline::rule(
            READS_OPT_RULE,
            "t.bar",
            LoweringDecl::new().reads_opt(COUNT),
            DefaultMark::Never,
        ))
    })
}

#[test]
fn an_unresolved_peer_table_sets_aside_the_rule_that_reads_it_and_not_the_one_that_may() {
    // Nobody declares `t.count`: one Note, the hard read set aside, the
    // optional read left to run and see the table as absent.
    let alone = registry(vec![reader_feature()]);
    assert_eq!(build_diagnostic_ids(&alone), ["flm.registry.unresolved-peer"]);
    assert!(!alone.build_diagnostics().has_errors());
    assert_eq!(
        alone.build_diagnostics().iter().next().expect("the note").message(),
        "feature `t.reader` refers to the table `t.count` and expects another feature to declare it, but no feature of this registry declares it; the rules depending on it are set aside: [t.foo.lower.reads]"
    );
    let set_aside: Vec<(&str, Vec<PeerName>)> = alone
        .set_aside_rules()
        .map(|entry| (entry.rule.name.as_str(), entry.set_aside.clone()))
        .collect();
    assert_eq!(set_aside, [(READS_RULE, vec![PeerName::Table("t.count".into())])]);
    let tables: Vec<(&str, &str, bool)> = alone
        .peer_tables()
        .iter()
        .map(|entry| (entry.owner.as_str(), entry.name.as_str(), entry.resolved))
        .collect();
    assert_eq!(tables, [("t.reader", "t.count", false)]);
    // The rule that may read the table is the sole candidate and is chosen.
    let with_backend =
        registry(vec![reader_feature(), feature("t.text", |b| register_backend(b, "text", &[]))]);
    let plan = plan_for(&with_backend, &PlanRequest::new().demand(lowered_cap("text")));
    let selected = plan.selection("text").expect("text is active");
    let chosen = selected.rule_for("t.bar").expect("the optional read is selected");
    assert_eq!(with_backend.rule(chosen).rule.name, READS_OPT_RULE);
}

#[test]
fn a_peer_table_resolves_against_the_table_another_feature_declares() {
    let registry = registry(vec![
        reader_feature(),
        feature("t.writer", |b| {
            b.table(COUNT, "the count")?;
            b.stage(noop(
                StageDecl::new("t.count.write", Effect::Compute, Scope::Document).writes(COUNT),
                Needs::new(),
            ))
        }),
    ]);
    assert!(registry.build_diagnostics().is_empty());
    assert_eq!(registry.set_aside_rules().count(), 0);
    let tables: Vec<(&str, &str, bool)> = registry
        .peer_tables()
        .iter()
        .map(|entry| (entry.owner.as_str(), entry.name.as_str(), entry.resolved))
        .collect();
    assert_eq!(tables, [("t.reader", "t.count", true)]);
}

#[test]
fn a_stage_writing_an_unresolved_peer_table_is_set_aside_with_its_orderings() {
    // Nobody declares `t.count`; the feature's stage writing it has nowhere to
    // write and is not registered, and the ordering placed on it goes with it
    // rather than failing as an ordering on an undeclared stage.
    let registry = registry(vec![feature("t.writer", |b| {
        b.capability(CapabilityDef::new("t.a"))?;
        b.stage(provider("t.s", &["t.a"], &[]))?;
        b.peer_table::<common::pipeline::Count>(COUNT.name(), "the count another feature declares")?;
        b.stage(noop(
            StageDecl::new("t.count.write", Effect::Compute, Scope::Document).writes(COUNT),
            Needs::new(),
        ))?;
        b.ordering(OrderingDef::new("t.count.write").after("t.a"))
    })]);
    assert!(!registry.build_diagnostics().has_errors());
    let stages: Vec<&str> = registry.stages().iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(stages, ["t.s"]);
    assert!(registry.orderings().is_empty());
    assert_eq!(build_diagnostic_ids(&registry), ["flm.registry.unresolved-peer"]);
    assert_eq!(
        registry.build_diagnostics().iter().next().expect("the note").message(),
        "feature `t.writer` refers to the table `t.count` and expects another feature to declare it, but no feature of this registry declares it; the stages writing it are set aside: [t.count.write]"
    );
}

#[test]
fn a_peer_table_declared_with_the_wrong_value_type_is_refused() {
    let error = registry_error(vec![
        reader_feature(),
        feature("t.writer", |b| {
            b.table(TableName::<common::pipeline::Log>::new("t.count"), "another type")?;
            b.stage(noop(
                StageDecl::new("t.count.write", Effect::Compute, Scope::Document)
                    .writes(TableName::<common::pipeline::Log>::new("t.count")),
                Needs::new(),
            ))
        }),
    ]);
    assert_eq!(error.phase, BuildPhase::Validate);
    let mismatch = error
        .errors
        .iter()
        .find(|e| matches!(e, BuildError::PeerTableMismatch { .. }))
        .expect("the mismatch");
    match mismatch {
        BuildError::PeerTableMismatch { feature, table, declared, expected } => {
            assert_eq!(feature, "t.reader");
            assert_eq!(table, "t.count");
            assert_eq!(*declared, "Log");
            assert_eq!(*expected, "Count");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_table_a_rule_reads_without_a_peer_declaration_is_undeclared_even_when_the_rule_is_set_aside() {
    // The rule is set aside for want of the backend `html`, and the table it
    // reads is neither declared nor a peer declaration of its own feature:
    // the misspelled name is reported all the same.
    let error = registry_error(vec![feature("t.rule", |b| {
        b.element(macro_def("t", "foo", 0))?;
        b.peer_backend("html", "the backend the rule is the default on")?;
        b.lowering_rule(common::pipeline::rule(
            BOX_RULE,
            "t.foo",
            LoweringDecl::new().reads(COUNT),
            DefaultMark::OnBackends(vec!["html".into()]),
        ))
    })]);
    match single(error) {
        BuildError::Undeclared { feature, kind, site, name, .. } => {
            assert_eq!(feature, "t.rule");
            assert_eq!(kind, "table");
            assert_eq!(site, Site::RuleReads { rule: BOX_RULE.into() });
            assert_eq!(name, "t.count");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// --- peer slots ----------------------------------------------------------------------------

/// A feature `t.reader` extended with a value for a service slot `t.styles`
/// another feature is expected to declare: a peer contribution beside a peer
/// table declaration of the same feature.
fn peer_slot_feature() -> Arc<dyn FlmFeature> {
    feature("t.contributor", |b| {
        b.peer_table::<common::pipeline::Count>(COUNT.name(), "the count another feature writes")?;
        b.peer_slot("t.styles", Arc::new("one style".to_string()))
    })
}

#[test]
fn an_unresolved_peer_slot_is_noted_beside_the_other_peer_declarations() {
    // Nobody declares the table, nobody declares the slot: two Notes, the
    // declarations first and the contribution last, and both listings say what
    // went unresolved.
    let registry = registry(vec![peer_slot_feature()]);
    assert_eq!(
        build_diagnostic_ids(&registry),
        ["flm.registry.unresolved-peer", "flm.registry.unresolved-peer"]
    );
    let messages: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message()).collect();
    assert!(messages[0].contains("refers to the table `t.count`"), "{}", messages[0]);
    assert_eq!(
        messages[1],
        "feature `t.contributor` refers to the slot `t.styles` and expects another feature to declare it, but no feature of this registry declares it; the value contributed to it is dropped"
    );
    let slots: Vec<(&str, &str, bool)> = registry
        .peer_slots()
        .iter()
        .map(|entry| (entry.owner.as_str(), entry.slot.as_str(), entry.resolved))
        .collect();
    assert_eq!(slots, [("t.contributor", "t.styles", false)]);
    assert!(!registry.peer_tables()[0].resolved);
    // Nothing is set aside: no rule can depend on a slot.
    assert_eq!(registry.set_aside_rules().count(), 0);
}

#[test]
fn a_peer_slot_resolves_against_the_slot_another_feature_declares() {
    let registry = registry(vec![
        peer_slot_feature(),
        feature("t.owner", |b| {
            b.table(COUNT, "the count")?;
            b.stage(noop(
                StageDecl::new("t.count.write", Effect::Compute, Scope::Document).writes(COUNT),
                Needs::new(),
            ))?;
            b.service_slot::<String>("t.styles", Arity::OrderedMany, "styles")
        }),
    ]);
    assert!(registry.build_diagnostics().is_empty());
    let styles: Vec<&str> =
        registry.services().services_named::<String>("t.styles").map(String::as_str).collect();
    assert_eq!(styles, ["one style"]);
    assert!(registry.peer_slots()[0].resolved);
    assert!(registry.peer_slots()[0].ty.is::<String>());
}

/// The rule feature beside a backend `text` that does not claim `t.pres.box`
/// and no `html`: the rule is set aside, and `t.foo` has no other rule.
fn set_aside_registry() -> Arc<FlmRegistry> {
    registry(vec![
        rule_feature(),
        feature("t.text", |b| register_backend(b, "text", &[])),
    ])
}

#[test]
fn a_set_aside_rule_is_never_a_candidate_not_even_the_sole_one() {
    let registry = set_aside_registry();
    let request = PlanRequest::new().demand(lowered_cap("text"));
    // Without the rule, `t.foo` is neither native nor lowered: refused.
    match Plan::build(&registry, &request).expect_err("incomplete") {
        PlanError::Incomplete { backend, missing, .. } => {
            assert_eq!(backend, "text");
            assert_eq!(missing.iter().map(|e| e.to_string()).collect::<Vec<_>>(), ["t.foo"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    // Accepted as a hole, the element lowers to the stand-in, never through
    // the rule — even when the request prefers the rule by name, which the plan
    // reports as a Warning naming the peer declarations that did not resolve.
    let request = request
        .unsupported("text", "t.foo")
        .lowering("text", LoweringSelection::new().prefer(BOX_RULE));
    let plan = plan_for(&registry, &request);
    let selected = plan.selection("text").expect("text is active");
    assert!(selected.rule_for("t.foo").is_none());
    assert_eq!(
        plan.warnings().to_vec(),
        vec![PlanWarning::PreferredRuleSetAside {
            backend: "text".into(),
            rule: BOX_RULE.into(),
            unresolved: vec![
                PeerName::Backend("html".into()),
                PeerName::Element("t.pres.box".into())
            ],
            workflow: None,
        }]
    );
    assert_eq!(
        plan.warnings()[0].to_string(),
        "the plan request prefers the lowering rule `t.foo.lower.html` on backend `text`, but that rule cannot be used: it \
         depends on backend `html`, element `t.pres.box`, which no registered feature \
         declares"
    );
}

#[test]
fn a_set_aside_rule_is_not_selected_as_the_marked_default_on_its_backend() {
    // A third feature registers the backend `html` the rule is the default
    // on, but nobody registers `t.pres.box`: the backend declaration resolves,
    // the element declaration does not, and the rule — the marked default on
    // the very backend being planned — is set aside all the same.
    let registry = registry(vec![
        rule_feature(),
        feature("t.html", |b| register_backend(b, "html", &[])),
    ]);
    assert_eq!(build_diagnostic_ids(&registry), ["flm.registry.unresolved-peer"]);
    assert!(registry.peer_backends()[0].resolved);
    assert!(!registry.peer_elements()[0].resolved);
    assert_eq!(
        registry.rule(registry.rule_idx(BOX_RULE).unwrap()).set_aside,
        [PeerName::Element("t.pres.box".into())]
    );
    let request = PlanRequest::new().demand(lowered_cap("html"));
    match Plan::build(&registry, &request).expect_err("incomplete") {
        PlanError::Incomplete { backend, missing, .. } => {
            assert_eq!(backend, "html");
            assert_eq!(missing.iter().map(|e| e.to_string()).collect::<Vec<_>>(), ["t.foo"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    let plan = plan_for(&registry, &request.unsupported("html", "t.foo"));
    let selected = plan.selection("html").expect("html is active");
    assert!(selected.rule_for("t.foo").is_none());
}

#[test]
fn the_standard_package_resolves_every_peer_declaration() {
    let registry = common::stdcore::registry(StdcoreConfig::new());
    assert!(registry.build_diagnostics().is_empty());
    assert_eq!(registry.set_aside_rules().count(), 0);
    // Every peer backend, in declaration order, which is the feature order (the
    // package's, then the feature of the package `math-passthrough`) and,
    // within a feature, the order its `register` declares them. Four
    // backends are named this way: `latex-recomposed`, by the recomposition
    // rule every content feature holds and by the LaTeX family's placeholder
    // rule; `html`, by the page's rules of the `xnotes` feature and by the image
    // rule of the `graphics` feature; `text`, by the image rule of the `graphics`
    // feature; and `latex-rendered`, by the image rule of the `graphics` feature.
    let backends: Vec<(&str, &str, bool)> = registry
        .peer_backends()
        .iter()
        .map(|entry| (entry.owner.as_str(), entry.name.as_str(), entry.resolved))
        .collect();
    assert_eq!(
        backends,
        [
            ("textstyles", "latex-recomposed", true),
            ("literals", "latex-recomposed", true),
            ("math", "latex-recomposed", true),
            ("refs", "latex-recomposed", true),
            ("headings", "latex-recomposed", true),
            ("verbatim", "latex-recomposed", true),
            ("lists", "latex-recomposed", true),
            ("xnotes", "html", true),
            ("xnotes", "latex-recomposed", true),
            ("xnotes", "latex-rendered", true),
            ("href", "latex-recomposed", true),
            ("defterm", "latex-recomposed", true),
            ("theorems", "latex-recomposed", true),
            ("quote", "latex-recomposed", true),
            ("graphics", "text", true),
            ("graphics", "html", true),
            ("graphics", "latex-recomposed", true),
            ("graphics", "latex-rendered", true),
            ("floats", "latex-recomposed", true),
            ("cells", "latex-recomposed", true),
            ("citations", "latex-recomposed", true),
            ("userdefs", "latex-recomposed", true),
            ("meta", "latex-recomposed", true),
            ("toc", "latex-recomposed", true),
            ("latex", "latex-recomposed", true),
            ("math-passthrough", "latex-recomposed", true)
        ]
    );
    assert!(!registry.peer_elements().is_empty());
    assert!(registry.peer_elements().iter().all(|entry| entry.resolved));
    // The LaTeX family's constructs, which the recomposition rules stage, and
    // the shared vocabulary's elements, which the `xnotes` and `toc` features
    // declare so that they build without the lowering feature.
    assert!(registry.peer_elements().iter().all(|entry| {
        entry.name.as_str().starts_with("flm.pres.latex.")
            || (["xnotes", "toc"].contains(&entry.owner.as_str()) && pres::ALL.contains(&entry.name.as_str()))
    }));
    // The thirty-five peer tables: the anchors table the `math-passthrough`
    // feature's rule for a display-math environment, the heading rule, the list rule, the
    // xnotes rules, the definition's rule of the `defterm` feature, the
    // theorem's rule and the float's rules read, the safe-labels table the recomposition rules that
    // write labels read, the resolutions table the `\eqref` rule, the term
    // link's rule and a proof's back-reference read, the value names the recomposition rules of `\eqref`,
    // of a term link and of a theorem read — twenty-two of the references feature's, read by
    // other features — the counter table of
    // the `numbering` feature, which the `\ref` rule, that same display-math
    // rule, the heading rule, the list rule, the xnotes rules, the theorem's
    // rule and the float's rules read to show
    // the number a point was given, and the table of xnotes, which the rules of
    // every kind's list read, declared by `contribute_kind` on behalf of
    // whichever feature contributes the kind; and the artifact tables of the
    // HTML output and the two LaTeX outputs, which the `graphics` feature's
    // publish stages write the copies of images into. The `toc` feature's rules read the counter and anchors tables too. All thirty-five resolve here. The list is in declaration order, which is the feature
    // order: the package's, then the feature of the package `math-passthrough`.
    let tables: Vec<(&str, &str, bool)> = registry
        .peer_tables()
        .iter()
        .map(|entry| (entry.owner.as_str(), entry.name.as_str(), entry.resolved))
        .collect();
    assert_eq!(
        tables,
        [
            ("math", "flm.counters", true),
            ("math", "flm.labels.safe", true),
            ("math", "flm.refs.resolutions", true),
            ("math", "flm.labels.safe.value", true),
            ("refs", "flm.counters", true),
            ("headings", "flm.anchors", true),
            ("headings", "flm.counters", true),
            ("headings", "flm.labels.safe", true),
            ("lists", "flm.counters", true),
            ("lists", "flm.anchors", true),
            ("lists", "flm.labels.safe", true),
            ("xnotes", "flm.xnotes", true),
            ("xnotes", "flm.counters", true),
            ("xnotes", "flm.anchors", true),
            ("xnotes", "flm.labels.safe", true),
            ("defterm", "flm.anchors", true),
            ("defterm", "flm.refs.resolutions", true),
            ("defterm", "flm.labels.safe", true),
            ("defterm", "flm.labels.safe.value", true),
            ("theorems", "flm.counters", true),
            ("theorems", "flm.anchors", true),
            ("theorems", "flm.refs.resolutions", true),
            ("theorems", "flm.labels.safe", true),
            ("theorems", "flm.labels.safe.value", true),
            ("graphics", "flm.output.html.artifacts", true),
            ("graphics", "flm.output.latex-recomposed.artifacts", true),
            ("graphics", "flm.output.latex-rendered.artifacts", true),
            ("floats", "flm.counters", true),
            ("floats", "flm.anchors", true),
            ("floats", "flm.labels.safe", true),
            ("toc", "flm.counters", true),
            ("toc", "flm.anchors", true),
            ("math-passthrough", "flm.anchors", true),
            ("math-passthrough", "flm.counters", true),
            ("math-passthrough", "flm.labels.safe", true)
        ]
    );
    // Every value contributed to a slot another feature owns finds its slot.
    assert!(registry.peer_slots().iter().all(|entry| entry.resolved));
    // Every element value declared as a peer finds its element type: the
    // float types, declared through the one function any feature calls.
    let values: Vec<(&str, &str, bool)> = registry
        .peer_element_values()
        .iter()
        .map(|entry| (entry.owner.as_str(), entry.names[0].as_str(), entry.resolved))
        .collect();
    assert_eq!(values, [("floats", "figure", true), ("floats", "table", true)]);
    // Every recomposition rule is registered by the feature that owns its
    // element; the backend's feature registers its two structural rules only.
    let owner_of = |rule: &str| registry.rule(registry.rule_idx(rule).unwrap()).owner.to_string();
    assert_eq!(owner_of("flm.textstyle.lower.latex-recomposed"), "textstyles");
    assert_eq!(owner_of("flm.literal.lower.latex-recomposed"), "literals");
    assert_eq!(owner_of("flm.heading.lower.latex-recomposed"), "headings");
    assert_eq!(owner_of("flm.ref.lower.latex-recomposed"), "refs");
    assert_eq!(owner_of("flm.hyperref.lower.latex-recomposed"), "refs");
    // The rules of the formulas are the feature's that processes mathematics:
    // the `math-passthrough` feature registers them, and the backend names no
    // content feature. The rules of the `math` feature's own commands (the
    // text commands and `\eqref`) are that feature's.
    assert_eq!(owner_of("flm.math.environment.lower.passthrough-latex-recomposed"), "math-passthrough");
    assert_eq!(owner_of("flm.math.lower.passthrough-latex-recomposed"), "math-passthrough");
    assert_eq!(owner_of("flm.math.lower.passthrough"), "math-passthrough");
    assert_eq!(owner_of("flm.math.environment.lower.passthrough"), "math-passthrough");
    assert_eq!(owner_of("flm.math-passthrough.opaque.lower.default"), "math-passthrough");
    assert_eq!(owner_of("flm.math.text.lower.latex-recomposed"), "math");
    assert_eq!(owner_of("flm.math.text.lower.default"), "math");
    assert_eq!(owner_of("flm.math.eqref.lower.latex-recomposed"), "math");
    assert_eq!(owner_of("flm.verbatim.macro.lower.latex-recomposed"), "verbatim");
    assert_eq!(owner_of("flm.href.link.lower.latex-recomposed"), "href");
    // The rule that writes a stamped point of a template copy is the LaTeX
    // family's, whose feature owns the stamped point.
    assert_eq!(owner_of("flm.placeholder.lower.latex-recomposed"), "latex");
    assert_eq!(owner_of("flm.lists.list.lower.latex-recomposed"), "lists");
    let by_backend: Vec<&str> = registry
        .rules()
        .iter()
        .filter(|entry| entry.owner == "latex-recomposed")
        .map(|entry| entry.rule.name.as_str())
        .collect();
    assert_eq!(
        by_backend,
        ["flm.parbreak.lower.latex-recomposed", "flm.paragraph.lower.latex-recomposed"]
    );
}
