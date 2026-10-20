//! An element no backend emits natively, through the whole pipeline: a
//! test-only feature registers `x.box` beside `flm_core::stdcore::package`.
//! With its own lowering rule producing `block` the element is reachable; with
//! no rule the text plan is refused at plan build with the typed error listing
//! it; with the element accepted as a hole it lowers to
//! `flm.pres.unsupported`, prints as `[unsupported: x.box]`, and raises one
//! error at the node's span.

mod common;

use std::sync::Arc;

use flm_core::model::addr::TreeId;
use flm_core::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use flm_core::model::element::{ElementSpec, element_of};
use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_core::model::names::ElementName;
use flm_core::core::plan::{Overrides, Plan, PlanError};
use flm_core::core::registry::FlmFeature;
use flm_core::dump::diag::expect;
use flm_core::lang::Flm;
use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_core::stdcore::backends::lowering::lower_arg_or_nothing;
use flm_core::stdcore::backends::pres::{self, BlockPayload, UnsupportedPayload, payload_of};
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::refs::UnresolvedRefs;
use techy::core::node::NodeRef;
use techy::error::Severity;

use common::lowering::{lowered, text as text_of};
use common::pipeline;
use common::spine::{overrides, registry_with, render};

/// The unknown element's name.
const BOX: &str = "x.box";

/// The document: a `\box` between two paragraphs' worth of text.
const DOCUMENT: &str = "Before \\box{inside} after.";

/// `x.box` → `flm.pres.block { role: x.box }` over the lowered argument.
struct BoxRule;

impl Lower for BoxRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let body = lower_arg_or_nothing(cx, lw, &node, 0)?;
        let payload = BlockPayload { role: Some(BOX.into()), ..BlockPayload::default() };
        let block = lw.pres_with(cx, &ElementName::from(pres::BLOCK), payload, body)?;
        Ok(Lowering::Emit(vec![block]))
    }
}

/// The test-only feature `x`: the block element `\box{content}` and, when
/// asked, its rule.
fn x_feature(with_rule: bool) -> Arc<dyn FlmFeature> {
    pipeline::feature("x", move |b| {
        let element = Arc::new(
            ElementSpec::new(BOX)
                .docs("The deliberately unknown element of build step 1.")
                .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Block))),
        );
        b.element(ElementDef::new(
            "x",
            ["box"],
            SyntaxDef::Macro { args: vec![ArgDef::mandatory("content")] },
            element,
        ))?;
        if with_rule {
            b.lowering_rule(LoweringRule::new(
                "x.box.lower.default",
                BOX,
                LoweringDecl::new().produces(pres::BLOCK),
                Arc::new(BoxRule),
                DefaultMark::Everywhere,
            ))?;
        }
        Ok(())
    })
}

/// The `text` request accepting `x.box` as a hole.
fn hole() -> Overrides {
    Overrides {
        unsupported: [(text::BACKEND.into(), vec![ElementName::from(BOX)])].into_iter().collect(),
        ..overrides(None, UnresolvedRefs::Error)
    }
}

#[test]
fn an_unknown_element_with_its_own_rule_is_reachable() {
    let registry = registry_with(vec![x_feature(true)]);
    let run = render(&registry, &[DOCUMENT], &overrides(None, UnresolvedRefs::Error));
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    let block = lowered(&run, TreeId::tree(0))
        .tree
        .iter_storage_order()
        .find(|node| element_of(node).name == pres::BLOCK)
        .expect("the box lowered to a block");
    assert_eq!(payload_of::<BlockPayload>(&block).unwrap().role.as_deref(), Some(BOX));
    assert_eq!(text_of(&run), "Before\n\ninside\n\nafter.\n");
}

#[test]
fn an_unknown_element_with_no_rule_is_refused_at_plan_build_naming_it() {
    let registry = registry_with(vec![x_feature(false)]);
    let error = Plan::for_workflow(&registry, text::WORKFLOW, &Overrides::none())
        .expect_err("the text plan is incomplete");
    match error {
        PlanError::Incomplete { backend, missing, .. } => {
            assert_eq!(backend, text::BACKEND);
            assert_eq!(missing, [ElementName::from(BOX)]);
        }
        other => panic!("not the completeness error: {other}"),
    }
}

#[test]
fn an_accepted_hole_lowers_to_unsupported_and_prints_as_a_note_with_one_error() {
    let registry = registry_with(vec![x_feature(false)]);
    let run = render(&registry, &[DOCUMENT], &hole());
    let lowered = lowered(&run, TreeId::tree(0));
    let unsupported = lowered
        .tree
        .iter_storage_order()
        .find(|node| element_of(node).name == pres::UNSUPPORTED)
        .expect("an unsupported node");
    assert_eq!(payload_of::<UnsupportedPayload>(&unsupported).unwrap().element, BOX);
    assert_eq!(text_of(&run), "Before\n\n[unsupported: x.box]\n\nafter.\n");
    let diagnostics: Vec<_> = run.diagnostics().iter().collect();
    assert_eq!(diagnostics.len(), 1);
    let diagnostic = expect(diagnostics, "flm.lowering.unsupported-element")
        .at("\\box{inside}")
        .severity(Severity::Error)
        .once();
    assert_eq!(diagnostic.severity(), Severity::Error);
}
