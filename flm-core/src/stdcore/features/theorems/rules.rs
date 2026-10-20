//! The general lowering rule of a theorem, [`TheoremRule`] ([`THEOREM_RULE`]),
//! the default on every backend that has no rule of its own for theorems: a
//! block that opens with the theorem's heading and, for a class with a
//! closing mark, closes with the mark.
//!
//! [`TheoremRule`] writes a theorem as a
//! [`flm.pres.block`](crate::stdcore::backends::pres::BLOCK) of role
//! [`THEOREM`], whose kind tokens are the class's name then the kind's
//! (`theoremlike lemma`), around the theorem's body laid out as blocks. The
//! block carries the anchor of the theorem's reference point where the
//! references feature computed one. The [role and the kind
//! tokens](crate::stdcore::backends::pres::payload) are what a stylesheet or a
//! host's lowering rule selects on.
//!
//! The **heading** (the class's template instantiated at the theorem; see
//! [`heading`](super::heading)) is placed at the start of the body's first
//! paragraph as a
//! [`flm.pres.deco.runin`](crate::stdcore::backends::pres::DECO_RUNIN)
//! decoration, which each backend sets as it sets a run-in heading. The
//! class's **closing mark** is placed at the end of the body's last paragraph
//! as a [`flm.pres.deco.final`](crate::stdcore::backends::pres::DECO_FINAL)
//! decoration. Where the body opens or closes with another block (a list, a
//! display formula), the heading or the mark gets a paragraph of its own; an
//! empty body gets one paragraph holding both.
//!
//! The kind's word and number in the heading are the theorem's number as the
//! numbering feature wrote it, with the kind's word (`Lemma~4`), read from the
//! counter table ([`COUNTERS`]) when the theorem is lowered. Where the
//! document did not number the theorem (an unnumbered kind, or a registry
//! without the numbering feature), the heading shows the kind's word alone
//! (`Proof`). The heading's title is the theorem's title, or, where a proof
//! shows its back-reference, the statement it refers to. The theorem's
//! fields, `\label{…}` and `\noproofref`, are not written.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::lang::Flm;
use crate::model::aspects::PointId;
use crate::model::element::element_value;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::lowering::lower_blocks_with_edges;
use crate::stdcore::backends::pres::{self, BlockPayload, DecorationPayload};
use crate::stdcore::features::numbering::{COUNTERS, FormatOptions, NumberSegment};
use crate::stdcore::features::refs::ANCHORS;

use super::elements::{
    THEOREM, TheoremElement, TitleReading, relation_site, theorem_point, title_reading,
};
use super::heading::{Headings, TitleShown, heading_instance};
use super::html::STYLE;

/// The name of the theorem's general lowering rule,
/// `flm.theorems.theorem.lower.default` ([`TheoremRule`]).
pub const THEOREM_RULE: &str = "flm.theorems.theorem.lower.default";

/// The lowering rule [`THEOREM_RULE`]: a theorem as a block with its heading
/// and closing mark. See the [module documentation](self).
///
/// A title that begins with a star, on a kind whose class reads
/// back-references, but that is not a well-formed back-reference is shown as
/// written; the feature's check reports it as
/// [`BadRelationRef`](super::BadRelationRef), under every workflow. A title
/// in the template of a kept definition ([kept
/// definitions](crate::stdcore::features::userdefs::kept)) whose first
/// characters a call supplies
/// ([`TitleReading::Unknown`]) makes the rule return
/// [`LowerError::PlaceholderCharacters`], so that each use site is expanded
/// instead.
///
/// At every theorem the rule requires the feature's style identifier
/// [`STYLE`] (`flm.theorems.style`), which each backend
/// resolves as it resolves requirements: the stylesheet on the `html`
/// backend, the definition pieces delivering it on the `latex-rendered`
/// backend, nothing on the `text` backend.
#[derive(Debug)]
pub struct TheoremRule {
    headings: Arc<Headings>,
}

impl TheoremRule {
    /// The rule that writes the headings and closing marks that `headings`
    /// compiled.
    pub(super) fn new(headings: Arc<Headings>) -> TheoremRule {
        TheoremRule { headings }
    }

    /// The number of the theorem at `point`, as the numbering feature wrote
    /// it with the kind's word; `None` where this document did not number it,
    /// or the registry holds no numbering feature.
    fn number(lw: &dyn Lowerer, point: PointId) -> Result<Option<Vec<NumberSegment>>, LowerError> {
        let Some(counters) = lw.get(COUNTERS)? else { return Ok(None) };
        match counters.format(point, &FormatOptions::new()) {
            None => Ok(None),
            Some(written) => written.map(Some).map_err(|error| LowerError::structure(error.to_string())),
        }
    }

    /// Stages `content` inside a decoration of the element `element`.
    fn decoration(
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
        element: &str,
        content: Vec<BuildId>,
    ) -> Result<Vec<BuildId>, LowerError> {
        Ok(vec![lw.pres_with(cx, &ElementName::from(element), DecorationPayload, content)?])
    }
}

impl Lower for TheoremRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lw.require(&Name::from(STYLE));
        let value = element_value::<TheoremElement>(&node)
            .ok_or_else(|| LowerError::structure("a theorem without its element's data"))?;
        // The title is read once; a malformed back-reference is shown as
        // written, the feature's check reporting it. See decision rationale
        // at [§flm-dd-dr:std-theorem-relation-reading].
        let title = match title_reading(&node).unwrap_or(TitleReading::Absent) {
            TitleReading::Absent => TitleShown::Nothing,
            TitleReading::Title => TitleShown::Title,
            // Shown as the declared site says.
            TitleReading::Relation { .. } => match relation_site(&node) {
                Some(site) if site.shown => TitleShown::Relation,
                _ => TitleShown::Nothing,
            },
            TitleReading::Malformed => TitleShown::Title,
            // A kept template's title whose characters a call supplies: the
            // use sites are expanded, and each is lowered with its own.
            TitleReading::Unknown => return Err(LowerError::PlaceholderCharacters),
        };
        let point = theorem_point(lw.addr());
        // The anchors table is absent in a registry holding no references
        // feature: the theorem is written with no anchor.
        let anchor = lw.get(ANCHORS)?.and_then(|anchors| anchors.id(point)).map(String::from);
        let number = TheoremRule::number(lw, point)?;
        let instance = heading_instance(&self.headings, &node, lw, value, number.as_deref(), title)?;
        let heading = lw.splice(cx, &instance)?;
        let leading = TheoremRule::decoration(cx, lw, pres::DECO_RUNIN, heading)?;
        let trailing = match self.headings.mark(&value.class_name)? {
            Some(mark) => {
                let mark = lw.splice(cx, &mark)?;
                TheoremRule::decoration(cx, lw, pres::DECO_FINAL, mark)?
            }
            None => Vec::new(),
        };
        let body: Vec<NodeRef<'_, Flm>> =
            node.body().map(|body| body.iter().collect()).unwrap_or_default();
        let children = lower_blocks_with_edges(cx, lw, &body, leading, trailing)?;
        let payload = BlockPayload {
            role: Some(Name::from(THEOREM)),
            kinds: vec![value.class_name.clone(), value.name.clone()],
            anchor,
            doc_role: None,
        };
        let block = lw.pres_with(cx, &ElementName::from(pres::BLOCK), payload, children)?;
        Ok(Lowering::Emit(vec![block]))
    }
}

/// The theorem's general lowering rule, [`THEOREM_RULE`], the default on
/// every backend, writing the headings and closing marks that `headings`
/// compiled. It reads the anchors table and the counter table optionally, so
/// that a registry without the references or the numbering feature still
/// renders a theorem. The rule declares that it produces the final decoration
/// only where a class has a closing mark, so that a registry in which none can
/// occur does not require its backend to render one.
pub(super) fn theorem_rule(headings: Arc<Headings>) -> LoweringRule {
    LoweringRule::new(
        THEOREM_RULE,
        THEOREM,
        theorem_decl(&headings),
        Arc::new(TheoremRule::new(headings)),
        DefaultMark::Everywhere,
    )
    .docs(
        "a theorem lowers to `flm.pres.block` of role `flm.theorems.theorem`, its kind tokens \
         the class and the kind, carrying its anchor, around its body's blocks; the heading is a \
         `flm.pres.deco.runin` decoration at the start of the first paragraph and the closing \
         mark a `flm.pres.deco.final` decoration at the end of the last",
    )
}

/// What the theorem's general rule declares: it reads the anchors table and
/// the counter table optionally, and produces the block, its paragraphs and
/// the run-in decoration, and the final decoration where a class of
/// `headings` has a closing mark.
fn theorem_decl(headings: &Headings) -> LoweringDecl {
    let decl = LoweringDecl::new()
        .reads_opt(ANCHORS)
        .reads_opt(COUNTERS)
        .produces(pres::BLOCK)
        .produces(pres::PAR)
        .produces(pres::DECO_RUNIN);
    if headings.any_mark() { decl.produces(pres::DECO_FINAL) } else { decl }
}
