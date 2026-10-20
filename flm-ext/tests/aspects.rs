//! The handler aspects (`api::model::aspects`): a `Referenceable` and a
//! `Sectioning` answered by an extension, on its own element and attached to
//! another feature's (`FeatureBuilder::attach`), the failure of a handler,
//! and a reference point read from data with every variant of its parts;
//! the `refs` library (`api::stdcore::features::refs`) through the Rust
//! façade and the dispatcher.

mod common;

use std::sync::Arc;

use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{parse_fragment, Flm, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::aspects::{
    AcceptedRefTypes as CoreAccepted, AnchorPolicy as CoreAnchor, ContentRef as CoreContent, Label as CoreLabel,
    NumberSource as CoreNumberSource, RefPoint as CoreRefPoint,
};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::names::{Name, RefKind};
use flm_core::stdcore::features::toc;
use flm_core::techy::core::node::NodeTree;
use flm_ext::adapter::aspects::{ref_point_from_value, ref_point_to_core, ref_points_from_value};
use flm_ext::adapter::dispatch;
use flm_ext::adapter::handles::{CallTable, Entry};
use flm_ext::adapter::node::NodeFacade;
use flm_ext::adapter::values::{handle_from_value, handle_to_value};
use flm_ext::api::core::lowering::{DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule};
use flm_ext::api::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_ext::api::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, ContentRef, RefPoint, Referenceable, Sectioning,
};
use flm_ext::api::model::element::{Aspect, ElementSpec};
use flm_ext::api::stdcore::features::refs;
use flm_ext::api::techy::node::Node;
use flm_ext::ExtError;
use serde_json::{json, Value};

use common::double::Double;
use common::json_double::{module, JsonDouble};
use common::{identifiers, latex_of, registry_with, text_of, try_registry_with};

// --- the element `\exbox{content}` and its rule -----------------------------------------------

/// A rule's body that emits the lowered first argument.
struct EmitContent;

impl Lower for EmitContent {
    fn lower<'c>(&self, node: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
        Ok(Lowering::Emit(lw.lower_arg(cx, node, 0)?))
    }
}

/// The declaration of `\exbox{content}` followed by `\label` fields, the
/// element `ex.box`, carrying `aspects`.
fn exbox(feature: &str, aspects: Vec<Aspect>) -> ElementDef {
    let mut spec = ElementSpec::new("ex.box");
    for aspect in aspects {
        spec = spec.with(aspect).expect("one aspect of each kind");
    }
    let args = vec![ArgDef::mandatory("content"), refs::label_fields_arg()];
    ElementDef::new(feature, ["exbox"], SyntaxDef::Macro { args }, spec)
}

/// The rule of `ex.box`.
fn exbox_rule() -> LoweringRule {
    LoweringRule::new("ex.box.lower.default", "ex.box", LoweringDecl::new(), Arc::new(EmitContent), DefaultMark::Everywhere)
}

/// The package `ex`, whose feature `boxes` declares `\exbox` with `aspects`.
fn boxes(aspects: Vec<Aspect>) -> Double {
    Double::new("ex").feature("boxes", move |b| {
        b.element(exbox(&b.feature(), aspects.clone()))?;
        b.lowering_rule(exbox_rule())
    })
}

// --- a Referenceable answered in Rust ---------------------------------------------------------

/// What the test's `Referenceable` answers.
#[derive(Clone)]
enum Answer {
    /// A point labeled from the `\label` fields, showing the first argument.
    Good,
    /// The same, showing the root of a tree the handler holds.
    Synthesized(Arc<NodeTree<Flm>>),
    /// A point whose content is a node of another tree.
    OtherTree(Arc<NodeTree<Flm>>),
    /// A point whose synthesized content is not the root of a tree.
    NotRoot,
    /// The handler's failure.
    Fails,
}

/// A `Referenceable` answering one point per occurrence, as `Answer` says.
struct Points(Answer);

impl Referenceable for Points {
    fn points<'c>(&self, node: &'c dyn Node<'c>) -> Result<Vec<RefPoint<'c>>, ExtError> {
        let uses = refs::label_uses(node);
        let (labels, malformed) = refs::point_labels(&uses);
        let content = match &self.0 {
            Answer::Good => ContentRef::Argument(0),
            Answer::Synthesized(tree) => ContentRef::Synthesized(Box::new(NodeFacade::shared(Arc::clone(tree), tree.root().id()))),
            Answer::OtherTree(tree) => ContentRef::Node(Box::new(NodeFacade::shared(Arc::clone(tree), tree.root().id()))),
            Answer::NotRoot => ContentRef::Synthesized(node.child(0).unwrap_or_else(|| node.parent().expect("a parent"))),
            Answer::Fails => return Err(ExtError::Handler { detail: "no points today".to_string(), data: Value::Null }),
        };
        Ok(vec![RefPoint {
            kind: RefKind::from("exbox"),
            labels,
            computed: 0,
            malformed,
            anchor: AnchorPolicy::FromFirstLabel,
            number: None,
            content: Some(content),
            accepts: AcceptedRefTypes::Any,
        }])
    }
}

/// A tree of its own, parsed from `text` with `registry`'s language.
fn parsed(registry: &Arc<FlmRegistry>, text: &str) -> Arc<NodeTree<Flm>> {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the text parses");
    Arc::clone(fragment.tree())
}

const REFERENCED: &str = "\\exbox{Alpha}\\label{ex:one} See \\ref{ex:one}.";

#[test]
fn a_referenceable_answered_in_rust_gives_ref_its_target() {
    let registry = registry_with([boxes(vec![Aspect::Referenceable(Arc::new(Points(Answer::Good)))]).package()]);
    let (text, diagnostics) = text_of(&registry, REFERENCED);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Alpha See Alpha.\n");
    let (latex, diagnostics) = latex_of(&registry, REFERENCED);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(latex.contains("Alpha See \\flmRef{ref1}."), "{latex}");
}

#[test]
fn a_synthesized_root_is_the_content_a_reference_shows() {
    let plain = registry_with([]);
    let tree = parsed(&plain, "Beta");
    let registry = registry_with([boxes(vec![Aspect::Referenceable(Arc::new(Points(Answer::Synthesized(tree))))]).package()]);
    let (text, diagnostics) = text_of(&registry, REFERENCED);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Alpha See Beta.\n");
}

#[test]
fn a_failing_or_malformed_answer_gives_the_occurrence_no_point() {
    let plain = registry_with([]);
    let other = parsed(&plain, "Gamma");
    for answer in [Answer::OtherTree(other), Answer::NotRoot, Answer::Fails] {
        let registry = registry_with([boxes(vec![Aspect::Referenceable(Arc::new(Points(answer)))]).package()]);
        let (_, diagnostics) = text_of(&registry, REFERENCED);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics, ["flm.refs.unresolved: the reference points at the label `ex:one`, which is not defined"]);
    }
}

// --- a Sectioning answered in Rust ------------------------------------------------------------

/// A `Sectioning` of level `level`, listed when `listed`, titled by its
/// first argument; `level` is the handler's failure when `None`.
struct Section {
    level: Option<u32>,
    listed: bool,
}

impl Sectioning for Section {
    fn level<'c>(&self, _: &'c dyn Node<'c>) -> Result<u32, ExtError> {
        self.level.ok_or_else(|| ExtError::Handler { detail: "no level".to_string(), data: Value::Null })
    }

    fn in_toc<'c>(&self, _: &'c dyn Node<'c>) -> Result<bool, ExtError> {
        Ok(self.listed)
    }

    fn toc_title<'c>(&self, _: &'c dyn Node<'c>) -> Result<Option<ContentRef<'c>>, ExtError> {
        Ok(Some(ContentRef::Argument(0)))
    }
}

/// The table of contents' entries of `text`, as `(level, title)`.
fn entries(registry: &Arc<FlmRegistry>, text: &str) -> Vec<(u32, Option<CoreContent>)> {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Plan::build(registry, &PlanRequest::new().demand(toc::ENTRIES.name())).expect("a plan");
    let mut run = Run::new(Arc::new(Doc::new(content)), Arc::new(plan), Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run.table(toc::ENTRIES)
        .expect("declared")
        .map(|table| table.entries.iter().map(|entry| (entry.level, entry.title.clone())).collect())
        .unwrap_or_default()
}

/// Whether `title` is the first argument.
fn first_argument(title: &Option<CoreContent>) -> bool {
    matches!(title, Some(CoreContent::Argument(0)))
}

#[test]
fn a_sectioning_answered_in_rust_lists_the_element_with_its_level_and_title() {
    let registry = registry_with([boxes(vec![Aspect::Sectioning(Arc::new(Section { level: Some(2), listed: true }))]).package()]);
    let found = entries(&registry, "\\exbox{Intro} text \\exbox{More}");
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|(level, title)| *level == 2 && first_argument(title)), "{found:?}");
}

#[test]
fn a_section_not_in_the_toc_and_a_failing_level_are_left_out() {
    let hidden = registry_with([boxes(vec![Aspect::Sectioning(Arc::new(Section { level: Some(1), listed: false }))]).package()]);
    assert!(entries(&hidden, "\\exbox{Intro}").is_empty());
    // A failing `level` answers 0; the entry is still listed, at level 0.
    let failing = registry_with([boxes(vec![Aspect::Sectioning(Arc::new(Section { level: None, listed: true }))]).package()]);
    let found = entries(&failing, "\\exbox{Intro}");
    assert_eq!(found.iter().map(|(level, _)| *level).collect::<Vec<_>>(), [0]);
}

// --- attach ------------------------------------------------------------------------------------

#[test]
fn attach_from_a_second_feature_adds_sectioning_to_the_first_s_element() {
    let package = boxes(Vec::new())
        .feature("outline", |b| b.attach("ex.box", Aspect::Sectioning(Arc::new(Section { level: Some(3), listed: true }))))
        .package();
    let registry = registry_with([package]);
    let found = entries(&registry, "\\exbox{Intro}");
    assert_eq!(found.iter().map(|(level, _)| *level).collect::<Vec<_>>(), [3]);
}

#[test]
fn attach_from_another_extension_is_not_checked_under_its_stem() {
    let other = Double::new("other")
        .feature("pointer", |b| b.attach("ex.box", Aspect::Referenceable(Arc::new(Points(Answer::Good)))))
        .package();
    let registry = registry_with([boxes(Vec::new()).package(), other]);
    let (text, diagnostics) = text_of(&registry, REFERENCED);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Alpha See Alpha.\n");
}

#[test]
fn an_attachment_to_an_element_no_feature_declares_fails_the_build() {
    let package = Double::new("ex")
        .feature("outline", |b| b.attach("ex.nothing", Aspect::Sectioning(Arc::new(Section { level: Some(1), listed: true }))))
        .package();
    let error = try_registry_with([package]).expect_err("an undeclared element").to_string();
    assert!(error.contains("ex.nothing"), "{error}");
}

#[test]
fn a_second_aspect_of_one_kind_on_an_element_fails_the_build() {
    let package = boxes(vec![Aspect::Sectioning(Arc::new(Section { level: Some(1), listed: true }))])
        .feature("outline", |b| b.attach("ex.box", Aspect::Sectioning(Arc::new(Section { level: Some(2), listed: true }))))
        .package();
    let error = try_registry_with([package]).expect_err("a collision").to_string();
    assert!(error.contains("ex.box"), "{error}");
}

// --- the refs library --------------------------------------------------------------------------

/// A rule's body that writes the labels of an occurrence after its
/// content, through `stage_label` and `stage_label_group`.
struct WithLabels;

impl Lower for WithLabels {
    fn lower<'c>(&self, node: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
        let mut out = lw.lower_arg(cx, node, 0)?;
        let mut labels = Vec::new();
        for use_ in refs::label_uses(node) {
            labels.push(refs::stage_label(cx, lw, &use_.text, use_.label.clone())?);
        }
        out.extend(refs::stage_label_group(cx, lw, labels, false)?);
        Ok(Lowering::Emit(out))
    }
}

/// A rule's body that places its labels twice, in two groups (S12): the
/// second group is refused by the lowerer's placement check.
struct LabelsTwice(Arc<std::sync::Mutex<Vec<String>>>);

impl Lower for LabelsTwice {
    fn lower<'c>(&self, node: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
        let mut out = lw.lower_arg(cx, node, 0)?;
        let mut labels = Vec::new();
        for use_ in refs::label_uses(node) {
            labels.push(refs::stage_label(cx, lw, &use_.text, use_.label.clone())?);
        }
        out.extend(refs::stage_label_group(cx, lw, labels.clone(), false)?);
        let refused = refs::stage_label_group(cx, lw, labels, false).err().map(|error| error.to_string());
        self.0.lock().expect("the lock").extend(refused);
        Ok(Lowering::Emit(out))
    }
}

#[test]
fn a_label_placed_in_two_groups_is_refused_by_the_placement_check() {
    let refused = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = Arc::clone(&refused);
    let package = Double::new("ex")
        .feature("boxes", move |b| {
            b.element(exbox(&b.feature(), vec![Aspect::Referenceable(Arc::new(Points(Answer::Good)))]))?;
            let decl = LoweringDecl::new()
                .reads_opt(refs::SAFE_LABELS)
                .option("LatexRecomposedRenderOptions")
                .produces("flm.pres.latex.macro")
                .produces("flm.pres.latex.raw");
            let rule = LoweringRule::new("ex.box.lower.latex", "ex.box", decl, Arc::new(LabelsTwice(Arc::clone(&seen))), DefaultMark::Everywhere);
            b.lowering_rule(rule)
        })
        .package();
    let registry = registry_with([package]);
    let (_, diagnostics) = latex_of(&registry, REFERENCED);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(*refused.lock().expect("the lock"), ["`stage_label_group`: a node may be placed only once"]);
}

#[test]
fn a_rule_writes_the_labels_of_an_occurrence_in_the_recomposed_latex() {
    let package = Double::new("ex")
        .feature("boxes", |b| {
            b.element(exbox(&b.feature(), vec![Aspect::Referenceable(Arc::new(Points(Answer::Good)))]))?;
            let decl = LoweringDecl::new()
                .reads_opt(refs::SAFE_LABELS)
                .option("LatexRecomposedRenderOptions")
                .produces("flm.pres.latex.macro")
                .produces("flm.pres.latex.raw");
            let rule = LoweringRule::new("ex.box.lower.latex", "ex.box", decl, Arc::new(WithLabels), DefaultMark::Everywhere);
            b.lowering_rule(rule)
        })
        .package();
    let registry = registry_with([package]);
    let (latex, diagnostics) = latex_of(&registry, REFERENCED);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(latex.contains("Alpha\\flmLabels{\\label{ref1}} See \\flmRef{ref1}."), "{latex}");
}

#[test]
fn label_uses_and_point_labels_read_the_fields_with_their_spans() {
    let registry = registry_with([boxes(Vec::new()).package()]);
    let tree = parsed(&registry, "\\exbox{A}\\label{ex:one}\\label{bad}");
    let root = tree.root();
    let node = root.child(0).expect("the occurrence");
    let facade = NodeFacade::lent(node);
    let uses = refs::label_uses(&facade);
    assert_eq!(uses.iter().map(|u| u.text.as_str()).collect::<Vec<_>>(), ["ex:one", "bad"]);
    let (labels, malformed) = refs::point_labels(&uses);
    assert_eq!(labels, [CoreLabel::new("ex", "one")]);
    assert_eq!(malformed.len(), 1);
    assert_eq!((malformed[0].text.as_str(), malformed[0].position), ("bad", 1));
    let source = "\\exbox{A}\\label{ex:one}\\label{bad}";
    assert_eq!(&source[malformed[0].span.start..malformed[0].span.end], "\\label{bad}");
}

// --- through the dispatcher --------------------------------------------------------------------

/// A `Referenceable` whose `points` acts through the dispatcher alone, as a
/// script's object does: given the node's handle, it answers the points as
/// data.
struct JsonPoints(Box<PointsFn>);

/// What a script's `points` does: given the call's table and the node's
/// handle, the points as data.
type PointsFn = dyn for<'c> Fn(&mut CallTable<'c>, &Value) -> Result<Value, ExtError> + Send + Sync;

impl Referenceable for JsonPoints {
    fn points<'c>(&self, node: &'c dyn Node<'c>) -> Result<Vec<RefPoint<'c>>, ExtError> {
        let mut table = CallTable::new(9000);
        let handle = handle_to_value(table.lend(Entry::Node(Box::new(node))));
        let answer = (self.0)(&mut table, &handle)?;
        ref_points_from_value(&mut table, &answer)
    }
}

/// `dispatch::call` on `target` (a handle in its JSON shape).
fn ask(table: &mut CallTable<'_>, target: &Value, method: &str, args: Value) -> Result<Value, ExtError> {
    dispatch::call(table, handle_from_value(target)?, method, &args)
}

/// The points of a script: the labels read through the module `refs`.
fn scripted_points(table: &mut CallTable<'_>, node: &Value) -> Result<Value, ExtError> {
    let refs_module = module("stdcore/features/refs");
    let uses = ask(table, &refs_module, "label_uses", json!([node]))?;
    let pair = ask(table, &refs_module, "point_labels", json!([uses]))?;
    Ok(json!([{
        "kind": "exbox",
        "labels": pair[0],
        "malformed": pair[1],
        "anchor": "from_first_label",
        "content": { "argument": 0 },
    }]))
}

#[test]
fn a_scripted_feature_declares_and_attaches_through_the_dispatcher() {
    let elements = JsonDouble::new("ex").feature("boxes", |js, b| {
        let refs_module = module("stdcore/features/refs");
        let fields = js.call(&refs_module, "label_fields_arg", json!([]))?;
        let element = json!({
            "names": ["exbox"],
            "syntax": { "macro": { "args": ["{", fields] } },
            "element": { "name": "ex.box" },
        });
        js.call(b, "element", json!([element]))?;
        let body = js.lend_lower(|js, node, cx, lw| {
            let content = js.call(lw, "lower_arg", json!([cx, node, 0]))?;
            Ok(json!({ "emit": content }))
        });
        let rule = js.call(
            &module("core/lowering"),
            "LoweringRule::new",
            json!(["ex.box.lower.default", "ex.box", {}, body, "everywhere"]),
        )?;
        js.call(b, "lowering_rule", json!([rule]))?;
        let points = js.lend(Entry::Referenceable(Arc::new(JsonPoints(Box::new(scripted_points)))));
        js.call(b, "attach", json!(["ex.box", { "Referenceable": points }]))?;
        Ok(())
    });
    let registry = registry_with([elements.package()]);
    let (text, diagnostics) = text_of(&registry, REFERENCED);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Alpha See Alpha.\n");
}

#[test]
fn a_scripted_answer_of_a_bad_shape_gives_no_point() {
    let bad = |_: &mut CallTable<'_>, _: &Value| -> Result<Value, ExtError> { Ok(json!([{ "kind": "exbox", "computed": -1, "anchor": "from_first_label" }])) };
    let registry = registry_with([boxes(vec![Aspect::Referenceable(Arc::new(JsonPoints(Box::new(bad))))]).package()]);
    let (_, diagnostics) = text_of(&registry, REFERENCED);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let ids = identifiers(&common::run(&registry, flm_core::stdcore::backends::text::WORKFLOW, REFERENCED));
    assert_eq!(ids, ["flm.refs.unresolved"]);
}

#[test]
fn a_reference_point_from_data_reads_back_with_every_variant() {
    let registry = registry_with([boxes(Vec::new()).package()]);
    let tree = parsed(&registry, "\\exbox{A}\\label{ex:one}");
    let synthesized = parsed(&registry, "Delta");
    let root = tree.root();
    let node = root.child(0).expect("the occurrence");
    let mut table = CallTable::new(9001);
    let site = handle_to_value(table.lend(Entry::Node(Box::new(NodeFacade::lent(node)))));
    let inner = handle_to_value(table.lend(Entry::Node(Box::new(NodeFacade::lent(root)))));
    let shared = handle_to_value(
        table.lend(Entry::Node(Box::new(NodeFacade::shared(Arc::clone(&synthesized), synthesized.root().id())))),
    );
    let anchors = [
        (json!("from_first_label"), CoreAnchor::FromFirstLabel),
        (json!({ "explicit": "here" }), CoreAnchor::Explicit("here".to_string())),
        (json!({ "slug": { "prefix": "ex", "content": "body" } }), CoreAnchor::Slug { prefix: "ex".to_string(), content: CoreContent::Body }),
        (json!({ "from_value": { "prefix": "ex" } }), CoreAnchor::FromValue { prefix: "ex".to_string() }),
        (json!({ "auto": { "prefix": "ex" } }), CoreAnchor::Auto { prefix: "ex".to_string() }),
    ];
    for (data, expected) in anchors {
        let point = ref_point_from_value(&mut table, &json!({ "kind": "exbox", "anchor": data })).expect("a point");
        let core = ref_point_to_core(point, &node).expect("it converts");
        assert_eq!(core.anchor, expected);
        assert_eq!(core.accepts, CoreAccepted::Any);
    }
    let contents = [
        (json!({ "argument": 1 }), "argument 1"),
        (json!({ "argument_named": "content" }), "argument content"),
        (json!("body"), "body"),
        (json!({ "node": inner }), "node"),
        (json!({ "synthesized": shared }), "synthesized"),
    ];
    for (data, what) in contents {
        let point = ref_point_from_value(&mut table, &json!({ "kind": "exbox", "anchor": "from_first_label", "content": data }))
            .expect(what);
        let core = ref_point_to_core(point, &node).expect(what);
        let found = match core.content {
            Some(CoreContent::Argument(1)) => "argument 1",
            Some(CoreContent::ArgumentNamed(name)) if name == "content" => "argument content",
            Some(CoreContent::Body) => "body",
            Some(CoreContent::Node(id)) if id == root.id() => "node",
            Some(CoreContent::Synthesized(tree)) if Arc::ptr_eq(&tree, &synthesized) => "synthesized",
            _ => "something else",
        };
        assert_eq!(found, what);
    }
    let full = json!({
        "kind": "exbox",
        "labels": [{ "ref_type": "ex", "ref_label": "one" }],
        "computed": 0,
        "malformed": [{ "text": "bad", "span": { "start": 0, "end": 3 }, "position": 1 }],
        "anchor": "from_first_label",
        "number": { "counter": "ex.counter", "instances": [inner], "style": "ex.style", "advance": [0], "number_format": null },
        "accepts": { "these": ["ex"] },
    });
    let point = ref_point_from_value(&mut table, &full).expect("a point");
    let core: CoreRefPoint = ref_point_to_core(point, &node).expect("it converts");
    assert_eq!(core.kind, RefKind::from("exbox"));
    assert_eq!(core.labels, [CoreLabel::new("ex", "one")]);
    assert_eq!(core.accepts, CoreAccepted::These(vec!["ex".to_string()]));
    assert_eq!((core.malformed[0].text.as_str(), core.malformed[0].span.range(), core.malformed[0].position), ("bad", 0..3, 1));
    assert_eq!(
        core.number,
        Some(CoreNumberSource {
            counter: Name::from("ex.counter"),
            instances: vec![root.id()],
            style: Name::from("ex.style"),
            advance: vec![0],
            number_format: None,
        })
    );
    // The shape's failures, each in the readers' form.
    let error = ref_point_from_value(&mut table, &json!({ "kind": "exbox", "anchor": "from_first_label", "computed": -1 }))
        .err()
        .map(|error| error.to_string());
    assert_eq!(error.as_deref(), Some("the field `computed` of a reference point must be a number from 0 up, not `-1`"));
    let missing = ref_point_from_value(&mut table, &json!({ "kind": "exbox" })).err().map(|error| error.to_string());
    assert_eq!(missing.as_deref(), Some("a reference point must have the field `anchor`"));
    // A node of another tree, a synthesized node that is not a root, a span
    // past the source: each the handler's failure.
    let other = handle_to_value(table.lend(Entry::Node(Box::new(NodeFacade::shared(Arc::clone(&synthesized), synthesized.root().id())))));
    let point = ref_point_from_value(&mut table, &json!({ "kind": "exbox", "anchor": "from_first_label", "content": { "node": other } }))
        .expect("a point");
    let error = ref_point_to_core(point, &node).err().map(|error| error.to_string());
    assert_eq!(
        error.as_deref(),
        Some("`Referenceable::points`: the content of a reference point must be a node of the tree the point's node is in")
    );
    let point = ref_point_from_value(&mut table, &json!({ "kind": "exbox", "anchor": "from_first_label", "content": { "synthesized": site } }))
        .expect("a point");
    let error = ref_point_to_core(point, &node).err().map(|error| error.to_string());
    assert_eq!(
        error.as_deref(),
        Some("`Referenceable::points`: the synthesized content of a reference point must be the root node of a tree")
    );
    let far = json!({ "kind": "exbox", "anchor": "from_first_label", "malformed": [{ "text": "x", "span": { "start": 0, "end": 999 }, "position": 0 }] });
    let point = ref_point_from_value(&mut table, &far).expect("a point");
    assert!(ref_point_to_core(point, &node).is_err());
}

#[test]
fn stage_label_and_stage_label_group_through_the_dispatcher() {
    // S13: the arms `stage_label` and `stage_label_group`, driven through the
    // JSON double, write what the Rust rule above writes.
    let package = JsonDouble::new("ex").feature("boxes", |js, b| {
        let refs_module = module("stdcore/features/refs");
        let fields = js.call(&refs_module, "label_fields_arg", json!([]))?;
        let element = json!({ "names": ["exbox"], "syntax": { "macro": { "args": ["{", fields] } }, "element": { "name": "ex.box" } });
        js.call(b, "element", json!([element]))?;
        let points = js.lend(Entry::Referenceable(Arc::new(Points(Answer::Good))));
        js.call(b, "attach", json!(["ex.box", { "Referenceable": points }]))?;
        let body = js.lend_lower(|js, node, cx, lw| {
            let refs_module = module("stdcore/features/refs");
            let mut emit: Vec<Value> = js.call(lw, "lower_arg", json!([cx, node, 0]))?.as_array().cloned().unwrap_or_default();
            let uses = js.call(&refs_module, "label_uses", json!([node]))?;
            let mut staged = Vec::new();
            for use_ in uses.as_array().cloned().unwrap_or_default() {
                staged.push(js.call(&refs_module, "stage_label", json!([cx, lw, use_["text"], use_["label"]]))?);
            }
            let group = js.call(&refs_module, "stage_label_group", json!([cx, lw, staged, false]))?;
            emit.extend(group.as_array().cloned().unwrap_or_default());
            Ok(json!({ "emit": emit }))
        });
        let decl = json!({
            "reads_opt": [refs::SAFE_LABELS],
            "options": ["LatexRecomposedRenderOptions"],
            "produces": ["flm.pres.latex.macro", "flm.pres.latex.raw"],
        });
        let rule = js.call(&module("core/lowering"), "LoweringRule::new", json!(["ex.box.lower.latex", "ex.box", decl, body, "everywhere"]))?;
        js.call(b, "lowering_rule", json!([rule]))?;
        Ok(())
    });
    let registry = registry_with([package.package()]);
    let (latex, diagnostics) = latex_of(&registry, REFERENCED);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(latex.contains("Alpha\\flmLabels{\\label{ref1}} See \\flmRef{ref1}."), "{latex}");
}
