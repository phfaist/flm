//! The registration items of the extension API beside the lowering rule
//! (`api::core::registry`): peer declarations by name, a table whose
//! capability refines another, a table of a definition's text, and style and
//! script definitions with their four part forms, through the Rust façade and
//! through the dispatcher, over the standard package and a hand-made package.

mod common;

use std::sync::Arc;

use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageEntry, PeerName};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdcore::backends::{html, latex_recomposed, pres};
use flm_core::stdcore::features::math;
use flm_ext::adapter::table::ExtTable;
use flm_ext::api::core::lowering::{DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule};
use flm_ext::api::core::registry::{DefContent, ScriptDef, StyleDef};
use flm_ext::api::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_ext::api::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_ext::api::model::element::ElementSpec;
use flm_ext::api::techy::node::Node;
use flm_ext::ExtError;
use serde_json::{json, Value};

use common::double::{stage, Double};
use common::json_double::JsonDouble;
use common::{registry_with, text_of, try_registry_with};

// --- helpers -----------------------------------------------------------------------------------

/// A body that stages a span of role `lw.box` over the lowered argument.
struct Boxing;

impl Lower for Boxing {
    fn lower<'c>(&self, node: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
        let content = lw.lower_arg(cx, node, 0)?;
        Ok(Lowering::Emit(vec![lw.pres(cx, pres::SPAN, &json!({ "role": "lw.box" }), content)?]))
    }
}

/// The declaration of `\lwbox{content}`, the element `lw.box`.
fn box_element(feature: &str) -> ElementDef {
    ElementDef::new(feature, ["lwbox"], SyntaxDef::Macro { args: vec![ArgDef::mandatory("content")] }, ElementSpec::new("lw.box"))
}

/// The rule of `lw.box` with the declaration `decl` and the mark `mark`.
fn box_rule(decl: LoweringDecl, mark: DefaultMark) -> LoweringRule {
    LoweringRule::new("lw.box.lower.default", "lw.box", decl, Arc::new(Boxing), mark)
}

/// A stylesheet `lw.style` of the parts `content`.
fn style(content: Vec<DefContent>) -> StyleDef {
    StyleDef { id: "lw.style".into(), content, docs: "the boxes' stylesheet".to_string() }
}

/// The package `lw` whose feature `boxes` declares `\lwbox`, a table of a
/// definition's text `lw.css`, a document stage `lw.write-css` writing
/// `written` into it (pushed as one item each), the stylesheet `lw.style` of
/// the text of `lw.css` and an inline part, and the rule of `lw.box` that
/// requires the stylesheet by selection and the table's capability, which
/// schedules the stage.
fn styled_boxes(written: Vec<Value>) -> PackageEntry {
    styled_boxes_in_order(written, false)
}

/// As [`styled_boxes`]; with `stage_first`, the stage is declared before the
/// table it writes.
fn styled_boxes_in_order(written: Vec<Value>, stage_first: bool) -> PackageEntry {
    Double::new("lw")
        .feature("boxes", move |b| {
            b.element(box_element(&b.feature()))?;
            if !stage_first {
                b.definition_text_table("lw.css", "the text of the stylesheet's first part")?;
            }
            let written = written.clone();
            b.stage(stage(
                StageDecl::new("lw.write-css", Effect::Compute, Scope::Document).writes("lw.css"),
                Needs::new(),
                move |_, out| {
                    for item in &written {
                        out.table("lw.css")?.push(item.clone())?;
                    }
                    Ok(())
                },
            ))?;
            if stage_first {
                b.definition_text_table("lw.css", "the text of the stylesheet's first part")?;
            }
            b.style_def(style(vec![
                DefContent::Table("lw.css".into()),
                DefContent::Inline(".lw-after{}".to_string()),
            ]))?;
            let decl = LoweringDecl::new().produces(pres::SPAN).selection_requires("lw.style").requires("lw.css");
            b.lowering_rule(box_rule(decl, DefaultMark::Everywhere))
        })
        .package()
}

/// The run of a plan demanding `demand` over the document `text`.
fn run_demanding(registry: &Arc<FlmRegistry>, demand: &str, text: &str) -> Run {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Arc::new(Plan::build(registry, &PlanRequest::new().demand(demand)).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

/// The HTML page of `source`, and the diagnostics.
fn page_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let run = common::run(registry, html::WORKFLOW, source);
    let page = run
        .table(html::DOCUMENT)
        .expect("declared")
        .and_then(|set| set.entry().and_then(|entry| entry.as_bytes().map(|b| String::from_utf8_lossy(b).into_owned())))
        .unwrap_or_default();
    (page, common::diagnostics(&run))
}

/// The rule of `registry` named `name`.
fn rule<'r>(registry: &'r FlmRegistry, name: &str) -> &'r flm_core::core::registry::RuleEntry {
    registry.rules().iter().find(|entry| entry.rule.name.as_str() == name).expect("the rule is registered")
}

/// The build diagnostics of `registry`, rendered.
fn build_notes(registry: &FlmRegistry) -> Vec<String> {
    registry.build_diagnostics().iter().map(|diagnostic| diagnostic.message().to_string()).collect()
}

// --- style and script definitions --------------------------------------------------------------

#[test]
fn a_style_definition_required_by_selection_reaches_the_page_with_its_table_part() {
    let registry = registry_with([styled_boxes(vec![json!(".lw-box{color:inherit}")])]);
    let (page, diagnostics) = page_of(&registry, "A \\lwbox{x}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let part = page.find(".lw-box{color:inherit}").expect("the table's text");
    assert!(part < page.find(".lw-after{}").expect("the inline part"), "{page}");
    assert!(page.contains("data-flm-def=\"lw.style\""), "{page}");
}

#[test]
fn a_table_of_a_definition_s_text_may_be_declared_after_the_stage_that_writes_it() {
    let registry = registry_with([styled_boxes_in_order(vec![json!(".lw-box{color:inherit}")], true)]);
    let (page, diagnostics) = page_of(&registry, "A \\lwbox{x}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(page.contains(".lw-box{color:inherit}"), "{page}");
}

#[test]
fn a_later_non_empty_text_replaces_an_earlier_one_and_a_text_must_be_a_string() {
    let registry = registry_with([styled_boxes(vec![json!(".first{}"), json!(""), json!(".second{}")])]);
    let (page, diagnostics) = page_of(&registry, "\\lwbox{x}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(page.contains(".second{}") && !page.contains(".first{}"), "{page}");

    let registry = registry_with([styled_boxes(vec![json!(["not", "a", "string"])])]);
    let (_, diagnostics) = page_of(&registry, "\\lwbox{x}");
    assert!(
        diagnostics.contains(
            &"flm.ext.handler-failed: the stage `lw.write-css` failed: the text of a definition's part must be a \
              string, not `[\"not\",\"a\",\"string\"]`"
                .to_string()
        ),
        "{diagnostics:?}"
    );
}

#[test]
fn a_part_naming_an_undeclared_table_fails_the_registry_build() {
    let package = Double::new("lw")
        .feature("boxes", |b| {
            b.table("lw.items", "a table of the extension's own type")?;
            b.style_def(style(vec![DefContent::Table("lw.nothing".into()), DefContent::Table("lw.items".into())]))?;
            b.script_def(ScriptDef {
                id: "lw.script".into(),
                content: vec![DefContent::Url("https://example.org/lw.js".to_string())],
                docs: String::new(),
            })
        })
        .package();
    let Err(error) = try_registry_with([package]) else { panic!("the registry builds") };
    let text = error.render();
    assert!(
        text.contains(
            "the table `lw.nothing` that the style definition `lw.style` takes a part from must be declared by a \
             feature of the registry"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "the table `lw.items` that the style definition `lw.style` takes a part from must hold \
             `DefinitionText`, not `ExtTable`"
        ),
        "{text}"
    );
}

#[test]
fn a_definition_s_identifier_must_begin_with_the_stem() {
    let package = Double::new("lw")
        .feature("boxes", |b| {
            let refused = b.style_def(StyleDef { id: "other.style".into(), content: Vec::new(), docs: String::new() });
            assert_eq!(
                refused.map_err(|error| error.to_string()),
                Err("the style `other.style` declared by the package `_` of the extension `lw` must begin with `lw.`"
                    .to_string())
            );
            Ok(())
        })
        .package();
    registry_with([package]);
}

// --- peers -------------------------------------------------------------------------------------

#[test]
fn a_peer_table_absent_sets_the_rule_aside_and_a_rust_owner_s_table_resolves_it() {
    // `lw.absent` is declared by no feature: the rule that reads it is set aside.
    let package = Double::new("lw")
        .feature("boxes", |b| {
            b.element(box_element(&b.feature()))?;
            b.peer_table("lw.absent", "a table no feature declares")?;
            let decl = LoweringDecl::new().produces(pres::SPAN).reads("lw.absent");
            b.lowering_rule(box_rule(decl, DefaultMark::Never))
        })
        .package();
    let registry = registry_with([package]);
    assert_eq!(rule(&registry, "lw.box.lower.default").set_aside, [PeerName::Table("lw.absent".into())]);
    assert!(build_notes(&registry).iter().any(|note| note.contains("lw.absent")), "{:?}", build_notes(&registry));

    // `flm.anchors` is the references feature's (a Rust table of its own
    // type): the peer takes that type, so it resolves, and the rule reads it.
    let package = Double::new("lw")
        .feature("boxes", |b| {
            b.peer_table("flm.anchors", "the references feature's anchors")?;
            b.element(box_element(&b.feature()))?;
            let decl = LoweringDecl::new().produces(pres::SPAN).reads_opt("flm.anchors");
            b.lowering_rule(box_rule(decl, DefaultMark::Everywhere))
        })
        .package();
    let registry = registry_with([package]);
    assert!(rule(&registry, "lw.box.lower.default").set_aside.is_empty());
    let (text, diagnostics) = text_of(&registry, "\\lwbox{x}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "x\n");
}

#[test]
fn a_peer_element_and_a_peer_backend_resolve_or_set_the_rule_aside() {
    let package = Double::new("lw")
        .feature("boxes", |b| {
            b.element(box_element(&b.feature()))?;
            b.peer_backend(latex_recomposed::BACKEND, "the recomposed LaTeX")?;
            b.peer_element("flm.pres.latex.raw", "the LaTeX family's raw construct")?;
            let decl = LoweringDecl::new().produces("flm.pres.latex.raw");
            b.lowering_rule(LoweringRule::new(
                "lw.box.lower.latex",
                "lw.box",
                decl,
                Arc::new(Boxing),
                DefaultMark::OnBackends(vec![latex_recomposed::BACKEND.into()]),
            ))?;
            b.peer_backend("lw.nowhere", "a backend no feature registers")?;
            b.peer_element("lw.pres.nothing", "an element no feature registers")?;
            let decl = LoweringDecl::new().produces("lw.pres.nothing");
            b.lowering_rule(LoweringRule::new(
                "lw.box.lower.nowhere",
                "lw.box",
                decl,
                Arc::new(Boxing),
                DefaultMark::OnBackends(vec!["lw.nowhere".into()]),
            ))
        })
        .package();
    let registry = registry_with([package]);
    // The raw construct is registered with its own payload type, which the
    // peer takes: no mismatch, nothing set aside.
    assert!(rule(&registry, "lw.box.lower.latex").set_aside.is_empty());
    assert_eq!(
        rule(&registry, "lw.box.lower.nowhere").set_aside,
        [PeerName::Backend("lw.nowhere".into()), PeerName::Element("lw.pres.nothing".into())]
    );
}

// --- a table that refines a capability ---------------------------------------------------------

#[test]
fn a_table_s_capability_refines_another() {
    let package = Double::new("lw")
        .feature("render", |b| {
            b.table_refining("lw.rendered", math::RENDERED, "the formulas, rendered")?;
            b.stage(stage(
                StageDecl::new("lw.render", Effect::Compute, Scope::Document).writes("lw.rendered"),
                Needs::new(),
                |_, out| out.table("lw.rendered")?.push(json!("rendered")),
            ))
        })
        .package();
    let registry = registry_with([package]);
    let rendered = registry.capability_named(math::RENDERED).expect("the parent");
    let refining = registry.capability_named("lw.rendered").expect("the table's capability");
    assert_eq!(refining.refines, Some(rendered.id));
    // A plan demanding the parent schedules the stage writing the refinement.
    let run = run_demanding(&registry, math::RENDERED, "Text.");
    let items = run.table(flm_core::core::data::table::TableHandle::<ExtTable>::new("lw.rendered")).expect("declared");
    assert_eq!(items.map(|table| table.items().to_vec()), Some(vec![json!("rendered")]));
}

// --- through the dispatcher --------------------------------------------------------------------

#[test]
fn the_dispatcher_offers_every_registration_item_by_name() {
    let package = JsonDouble::new("lw")
        .feature("boxes", |js, b| {
            js.call(b, "definition_text_table", json!(["lw.css", "the text"]))?;
            js.call(b, "table_refining", json!(["lw.rendered", math::RENDERED, "rendered"]))?;
            js.call(b, "peer_table", json!(["flm.anchors", "anchors"]))?;
            js.call(b, "peer_backend", json!(["html", "the html backend"]))?;
            js.call(b, "peer_element", json!(["flm.pres.latex.raw", "raw"]))?;
            js.call(
                b,
                "style_def",
                json!([{ "id": "lw.style", "content": [{ "table": "lw.css" }, { "inline": ".a{}" }, { "url": "u.css" }], "docs": "d" }]),
            )?;
            js.call(b, "script_def", json!([{ "id": "lw.script", "content": [{ "file": "x.js" }] }]))?;
            let refused = js.call(b, "style_def", json!([{ "id": "lw.bad", "content": [{ "inline": 1 }] }]));
            assert_eq!(
                refused.map_err(|error| error.to_string()),
                Err("`style_def`: a part of a definition must be `{\"inline\": text}`, `{\"file\": path}`, \
                     `{\"url\": address}` or `{\"table\": name}`, not `{\"inline\":1}`"
                    .to_string())
            );
            Ok(())
        })
        .package();
    let registry = registry_with([package]);
    let style = registry.styles().iter().find(|(_, def)| def.id.as_str() == "lw.style").expect("the style");
    assert_eq!(
        style.1.content,
        [DefContent::Table("lw.css".into()), DefContent::Inline(".a{}".to_string()), DefContent::Url("u.css".to_string())]
    );
    assert_eq!(style.1.docs, "d");
    let script = registry.scripts().iter().find(|(_, def)| def.id.as_str() == "lw.script").expect("the script");
    assert_eq!(script.1.content, [DefContent::File("x.js".to_string())]);
    assert!(registry.capability_named("lw.rendered").expect("declared").refines.is_some());
}

// --- argument roles ------------------------------------------------------------------------

#[test]
fn an_argument_role_is_declared_peer_declared_and_set_through_the_facade() {
    let package = Double::new("lw")
        .feature("keys", |b| {
            b.argument_role("lw.key", "The key of a box.")?;
            b.peer_argument_role("flm.refs.target")?;
            let args = vec![ArgDef::mandatory("key").role("lw.key"), ArgDef::mandatory("target").role("flm.refs.target")];
            b.element(ElementDef::new(b.feature(), ["lwkey"], SyntaxDef::Macro { args }, ElementSpec::new("lw.key")))
        })
        .package();
    let registry = registry_with([package]);
    let role = registry.argument_role_named("lw.key").expect("declared by the extension's feature");
    assert_eq!((role.owner.as_str(), role.docs.as_str()), ("keys", "The key of a box."));
    assert_eq!(registry.declarations().role_uses("lw.key").len(), 1);
    let uses = registry.declarations().role_uses("flm.refs.target");
    assert!(uses.iter().any(|place| place.declaration.names[0] == "lwkey"));
    assert!(registry.peer_argument_roles().iter().any(|peer| peer.owner == "keys" && peer.resolved));

    let outside = Double::new("lw").feature("keys", |b| b.argument_role("other.key", "A key.")).package();
    let error = try_registry_with([outside]).expect_err("a role outside the stem is refused").render();
    assert!(error.contains("the argument role `other.key` declared by the package"), "{error}");
    assert!(error.contains("must begin with `lw.`"), "{error}");
}

#[test]
fn the_dispatcher_declares_roles_and_reads_an_argument_s_role_from_data() {
    let package = JsonDouble::new("lw")
        .feature("keys", |js, b| {
            js.call(b, "argument_role", json!(["lw.key", "The key of a box."]))?;
            js.call(b, "peer_argument_role", json!(["lw.absent"]))?;
            let args = json!([{ "code": "{", "name": "key", "role": "lw.key" }, { "code": "{", "name": "other", "role": "lw.absent" }]);
            let element = json!({ "names": ["lwkey"], "syntax": { "macro": { "args": args } }, "element": { "name": "lw.key" } });
            js.call(b, "element", json!([element]))?;
            Ok(())
        })
        .package();
    let registry = registry_with([package]);
    assert_eq!(registry.argument_role_named("lw.key").map(|role| role.owner.as_str()), Some("keys"));
    let roles: Vec<Option<&str>> = registry
        .declarations()
        .elements_of("keys")
        .flat_map(|def| def.syntax.args())
        .map(|arg| arg.role.as_ref().map(|name| name.as_str()))
        .collect();
    assert_eq!(roles, [Some("lw.key"), Some("lw.absent")]);
    assert!(
        build_notes(&registry).iter().any(|note| note.contains("the argument role `lw.absent`")),
        "{:?}",
        build_notes(&registry)
    );
}

// --- E4: an argument's mode and scoped packages, extra placeholders, read back with codes -----

#[test]
fn a_definition_reads_back_with_its_codes_modes_scoped_packages_and_extra_placeholders() {
    use flm_ext::adapter::values::{definition_from_value, definition_to_value, no_handles};
    use flm_ext::api::lang::mint::Mode;
    let data = json!({
        "kind": "macro", "name": "stamped",
        "args": ["[", { "code": "{", "name": "formula", "layout": "inline", "opaque_wrapper": false, "docs": "", "mode": "math", "scoped": ["ex.hints"], "role": null }],
        "content": "#{stamp} #{formula}", "modes": "text", "defaults": {}, "argument_number_offset": 0,
        "block_role": { "inline": { "absorb_before": false, "absorb_after": false } }, "expand": "per-plan",
        "overrides": false, "docs": "", "extra_placeholders": [{ "name": "stamp", "substitution": false }],
    });
    let definition = definition_from_value(&data, &mut no_handles).expect("a definition");
    let formula = &definition.args[1];
    assert_eq!((formula.code(), formula.mode, formula.scoped.len()), (Some("{"), Some(Mode::Math), 1));
    assert_eq!(formula.scoped[0].as_str(), "ex.hints");
    assert_eq!(definition.args[0].code(), Some("["));
    assert_eq!(definition.extra_placeholders.len(), 1);
    assert_eq!((definition.extra_placeholders[0].name.as_str(), definition.extra_placeholders[0].substitution), ("stamp", false));
    let written = definition_to_value(&definition);
    assert_eq!(written["args"][0], json!({ "code": "[", "name": null, "layout": "inline", "opaque_wrapper": false, "docs": "", "mode": null, "scoped": [], "role": null }));
    assert_eq!(written["args"][1], data["args"][1]);
    assert_eq!(written["extra_placeholders"], data["extra_placeholders"]);
    let again = definition_from_value(&written, &mut no_handles).expect("it reads back");
    assert_eq!(definition_to_value(&again), written);
    let error = definition_from_value(&json!({ "kind": "macro", "name": "x", "extra_placeholders": [{ "name": "s", "substitution": 1 }] }), &mut no_handles)
        .err()
        .map(|error| error.to_string());
    assert_eq!(error.as_deref(), Some("the field `substitution` of an extra placeholder must be a boolean, not `1`"));
}

#[test]
fn a_definition_writes_each_argument_back_in_the_form_it_was_read_from() {
    use flm_ext::adapter::values::{definition_from_value, definition_to_value, no_handles};
    let data = json!({
        "kind": "macro", "name": "sized", "content": "#{size}#{other}",
        "args": [{ "syntax": "delimited-group", "delimiters": ["(", ")"], "name": "size" }, { "code": "r()", "name": "other" }],
    });
    let definition = definition_from_value(&data, &mut no_handles).expect("a definition");
    let written = definition_to_value(&definition);
    let (structured, coded) = (&written["args"][0], &written["args"][1]);
    assert_eq!((&structured["syntax"], &structured["delimiters"], structured.get("code")), (&json!("delimited-group"), &json!(["(", ")"]), None));
    assert_eq!((&coded["code"], coded.get("syntax")), (&json!("r()"), None));
    let again = definition_from_value(&written, &mut no_handles).expect("it reads back");
    assert_eq!(again.args, definition.args);
}

#[test]
fn the_dispatcher_reads_a_definition_and_writes_it_back_with_its_codes() {
    use flm_ext::adapter::dispatch;
    use flm_ext::adapter::handles::CallTable;
    use flm_ext::adapter::values::handle_from_value;
    let mut table = CallTable::new(8100);
    let userdefs = common::json_double::module("lang/rewrites/userdefs");
    let data = json!({
        "kind": "macro", "name": "stamped",
        "args": [{ "code": "{", "name": "formula", "layout": "inline", "opaque_wrapper": false, "docs": "", "mode": "math", "scoped": [], "role": null }],
        "content": "#{stamp} #1", "extra_placeholders": [{ "name": "stamp", "substitution": true }],
    });
    let ud = dispatch::call(&mut table, handle_from_value(&userdefs).expect("a handle"), "UserDefinition::new", &json!([data]))
        .expect("a user definition");
    let written = dispatch::call(&mut table, handle_from_value(&ud).expect("a handle"), "definition", &json!([])).expect("its data");
    assert_eq!(written["args"], data["args"]);
    assert_eq!(written["extra_placeholders"], data["extra_placeholders"]);
}

#[test]
fn an_argument_s_mode_and_a_body_s_scoped_package_are_read_from_data() {
    use flm_ext::adapter::values::{body_def_from_value, no_handles, syntax_def_from_value};
    use flm_ext::api::lang::mint::Mode;
    let body = body_def_from_value(&json!({ "layout": "blocks", "scoped": ["ex.hints", "ex.more"] })).expect("a body");
    assert_eq!(body.scoped.iter().map(|name| name.as_str()).collect::<Vec<_>>(), ["ex.hints", "ex.more"]);
    let error = body_def_from_value(&json!({ "scoped": "ex.hints" })).err().map(|error| error.to_string());
    assert_eq!(error.as_deref(), Some("the field `scoped` of a body must be a list of strings, not `\"ex.hints\"`"));
    let syntax = syntax_def_from_value(&json!({ "macro": { "args": [{ "code": "{", "mode": "math" }] } }), &mut no_handles)
        .expect("a syntax");
    let SyntaxDef::Macro { args } = syntax else { panic!("a macro") };
    assert_eq!(args[0].mode, Some(Mode::Math));
}
