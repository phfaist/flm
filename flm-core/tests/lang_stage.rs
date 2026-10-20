//! Staging nodes of the never-parsed specs by hand (`flm_core::lang::mint`) —
//! what a rewrite and the lowering stage do — and techy's region-tiling law
//! that the records they write must obey.

mod common;

use std::sync::Arc;

use flm_core::model::element::element_of;
use flm_core::lang::mint::{FlmPresentationSpec, FlmSynthesizedSpec};
use flm_core::lang::{Flm, FlmCallableType, FlmOrigin};
use techy::core::node::{
    BodySlotExt, BuildId, ChildRegion, ContentNodes, NodeBuildError, NodeKind, NodeTreeBuilder,
    ParsedArguments, ParsedSlot, ParsedSlots, SlotRole,
};
use techy::core::{Lang, ParsingState};
use techy::latexlike::BodyMarker;
use techy::source::SourceSpan;

use common::{element, lang, source};

/// A staging site: a builder, the state every node is staged under, and the one
/// (empty) span every node carries.
struct Site {
    builder: NodeTreeBuilder<Flm>,
    state: Arc<ParsingState<Flm>>,
    span: SourceSpan<FlmOrigin>,
}

impl Site {
    fn new() -> Site {
        let lang = lang(Vec::new());
        let source = source("");
        Site {
            builder: NodeTreeBuilder::new(),
            state: Arc::clone(lang.initial_state()),
            span: SourceSpan::new(&source, 0..0),
        }
    }

    /// Stages a node the way a transform does: the ext minted by the language,
    /// then the builder's one staging method.
    fn stage(
        &mut self,
        kind: NodeKind<Flm>,
        children: Vec<BuildId>,
    ) -> Result<BuildId, NodeBuildError> {
        let staged = self.builder.staged_children(&children);
        let ext = Flm::make_node_ext(&kind, &self.span, &self.state, staged)?;
        self.builder.add(kind, self.span.clone(), Arc::clone(&self.state), children, ext, ())
    }
}

#[test]
fn a_synthesized_node_carries_its_element_in_the_specials_form() {
    let spec = Arc::new(FlmSynthesizedSpec::new(element("thm.qed")));
    let mut site = Site::new();
    let node = site.stage(NodeKind::callable(spec.callable_data()), Vec::new()).unwrap();
    let root = site.stage(NodeKind::list(), vec![node]).unwrap();
    let tree = site.builder.finish(root).unwrap();
    let node = tree.root().child(0).unwrap();
    assert!(Arc::ptr_eq(element_of(&node), spec.element()));
    assert_eq!(node.callable_type(), Some(FlmCallableType::Specials));
    assert_eq!(node.name(), Some("thm.qed"));
    assert!(node.arguments().unwrap().is_empty());
    assert!(node.slots().unwrap().is_empty());
}

#[test]
fn a_presentation_node_carries_its_element_and_its_body_slot() {
    let spec = Arc::new(FlmPresentationSpec::new(element("flm.pres.block")));
    let mut site = Site::new();
    let text = site.stage(NodeKind::chars("x"), Vec::new()).unwrap();
    let body = site.stage(NodeKind::list(), vec![text]).unwrap();
    let slot = ParsedSlot::new(
        ChildRegion::new(0..1, ContentNodes::InChildrenOf(body, 0..1)),
        "body",
        SlotRole::Content,
        BodyMarker::make_body(),
    );
    let data = spec.callable_data(ParsedArguments::empty(), ParsedSlots::new(vec![slot]));
    let node = site.stage(NodeKind::callable(data), vec![body]).unwrap();
    let root = site.stage(NodeKind::list(), vec![node]).unwrap();
    let tree = site.builder.finish(root).unwrap();
    let node = tree.root().child(0).unwrap();
    assert!(Arc::ptr_eq(element_of(&node), spec.element()));
    assert_eq!(node.callable_type(), Some(FlmCallableType::Presentation));
    let content = node.body().unwrap();
    assert_eq!(content.first().and_then(|child| child.chars()), Some("x"));
}

#[test]
fn slot_records_must_tile_the_children_of_a_presentation_node() {
    let spec = Arc::new(FlmPresentationSpec::new(element("flm.pres.block")));
    let mut site = Site::new();
    let first = site.stage(NodeKind::list(), Vec::new()).unwrap();
    let second = site.stage(NodeKind::list(), Vec::new()).unwrap();
    let slot = ParsedSlot::new(
        ChildRegion::new(0..1, ContentNodes::InChildrenOf(first, 0..0)),
        "body",
        SlotRole::Content,
        BodyMarker::make_body(),
    );
    let data = spec.callable_data(ParsedArguments::empty(), ParsedSlots::new(vec![slot]));
    let error = site.stage(NodeKind::callable(data), vec![first, second]).unwrap_err();
    assert!(matches!(error, NodeBuildError::ChildrenNotInRegions { .. }));
}
