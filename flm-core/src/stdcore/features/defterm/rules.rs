//! The general lowering rules of the [`defterm`](super) feature, each the
//! default on every backend: a term definition becomes a block that opens
//! with its lead-in ([`DefinitionRule`]), and a term link becomes a link to
//! its term definition ([`TermRule`]).
//!
//! # A term definition
//!
//! [`DefinitionRule`] writes a term definition as a
//! [`flm.pres.block`](crate::stdcore::backends::pres::BLOCK) of role
//! [`DEFTERM`], around the term definition's body laid out as blocks. The
//! block carries the anchor of the term definition's reference point where
//! the references feature computed one. The lead-in is placed at the start of
//! the body's first paragraph (or in a paragraph of its own, where the body
//! opens with another block or is empty) as a run-in decoration, which each
//! backend sets as it sets a run-in heading (see [`lead_in`](super::lead_in)
//! and [decorations](crate::stdcore::backends::pres::Decoration)). The
//! term definition's fields, `\label{…}` and `\aliasterm{…}`, label it and are
//! not written.
//!
//! # A term link
//!
//! [`TermRule`] writes the text of a term link (what its author wrote in the
//! braces, lowered as inline content) in one of three ways:
//!
//! - inside the definition of its own term
//!   ([`in_own_definition`](super::in_own_definition)), as when a term
//!   definition uses its own term: a
//!   [`flm.pres.span`](crate::stdcore::backends::pres::SPAN) of role
//!   [`TERM_ROLE`], and no link, because it is the term being defined;
//! - elsewhere, where the references feature resolved it: a
//!   [`flm.pres.link`](crate::stdcore::backends::pres::LINK) of the kind
//!   [`TERM_LINK_KIND`] to the term definition's anchor;
//! - where nothing resolved it: a span styled as an unresolved reference,
//!   which the references feature has already reported as
//!   `flm.refs.unresolved`.
//!
//! Where the registry holds no references feature there is no resolution to
//! read, and a term link outside its own term's definition is its text alone.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};
use techy::error::Diagnostic;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::run::MissingEntry;
use crate::lang::Flm;
use crate::model::aspects::PointId;
use crate::model::names::{CapName, ElementName, Name};
use crate::stdcore::backends::lowering::{lower_arg_named_or_nothing, lower_blocks_with_edges};
use crate::stdcore::backends::pres::{
    self, BlockPayload, LinkPayload, SpanPayload, SpanStyle, spelling_identity,
};
use crate::stdcore::features::refs::{ANCHORS, REF_RESOLUTIONS, Resolution, SiteId};

use super::diagnostics::EmptyTerm;
use super::html::STYLE;
use super::elements::{
    DEFTERM, SHOWN_ARG, TERM, TERM_ARG, decide_term,
};
use super::lead_in::{LeadIn, stage_lead_in};

/// The name of the term definition's general lowering rule,
/// `flm.defterm.lower.default` ([`DefinitionRule`]).
pub const DEFINITION_RULE: &str = "flm.defterm.lower.default";

/// The name of the term link's general lowering rule,
/// `flm.term.lower.default` ([`TermRule`]).
pub const TERM_RULE: &str = "flm.term.lower.default";

/// The kind written on the link that a term link produces, `term`: it tells
/// a backend, and a stylesheet, that the link leads to the definition of a
/// term.
pub const TERM_LINK_KIND: &str = "term";

/// The role of the span that a term link inside the definition of its own
/// term is written as, `flm.defterm.term`.
pub const TERM_ROLE: &str = "flm.defterm.term";

/// The lowering rule [`DEFINITION_RULE`]: a term definition as a block
/// opening with its lead-in. See the [module
/// documentation](self#a-term-definition).
///
/// A term definition whose term is empty or only whitespace (`~` and `\ ` are
/// part of a term as written) is reported as [`EmptyTerm`] and still written.
///
/// The rule requires the feature's style identifier [`STYLE`]
/// (`flm.defterm.style`) at every term definition it stages, which each
/// backend resolves as it resolves requirements: the stylesheet on the `html`
/// backend, the definition pieces delivering it on the `latex-rendered`
/// backend, nothing on the `text` backend.
#[derive(Debug)]
pub struct DefinitionRule {
    lead_in: Arc<LeadIn>,
}

impl Lower for DefinitionRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let term = node.argument_content_nodes_named(TERM_ARG).ok().flatten();
        if term.is_none_or(|content| spelling_identity(content.iter()).is_empty()) {
            lw.error(EmptyTerm { what: String::from("term definition") }, node.span().clone());
        }
        // The anchors table is absent in a registry holding no references
        // feature: the definition is written with no anchor.
        let point = PointId { addr: lw.addr(), index: 0 };
        let anchor = lw.get(ANCHORS)?.and_then(|anchors| anchors.id(point)).map(String::from);
        lw.require(&Name::from(STYLE));
        let lead_in = stage_lead_in(&self.lead_in, &node, cx, lw)?;
        let body: Vec<NodeRef<'_, Flm>> =
            node.body().map(|body| body.iter().collect()).unwrap_or_default();
        let children = lower_blocks_with_edges(cx, lw, &body, lead_in, Vec::new())?;
        let payload = BlockPayload {
            role: Some(Name::from(DEFTERM)),
            kinds: Vec::new(),
            anchor,
            doc_role: None,
        };
        let block = lw.pres_with(cx, &ElementName::from(pres::BLOCK), payload, children)?;
        Ok(Lowering::Emit(vec![block]))
    }
}

/// The term definition's general lowering rule, [`DEFINITION_RULE`], the
/// default on every backend, writing the lead-in that `lead_in` configures.
/// The rule declares that it produces the decoration and the span around the
/// lead-in only where a lead-in is configured, so that a registry in which
/// none can occur does not require its backend to render them.
pub(super) fn definition_rule(lead_in: Arc<LeadIn>) -> LoweringRule {
    let mut decl =
        LoweringDecl::new().reads_opt(ANCHORS).produces(pres::BLOCK).produces(pres::PAR);
    if lead_in.is_set() {
        decl = decl.produces(pres::SPAN).produces(pres::DECO_RUNIN);
    }
    LoweringRule::new(
        DEFINITION_RULE,
        DEFTERM,
        decl,
        Arc::new(DefinitionRule { lead_in }),
        DefaultMark::Everywhere,
    )
    .docs(
        "a term definition lowers to `flm.pres.block` of role `flm.defterm` carrying its \
         anchor, around its body's blocks; the lead-in is a `flm.pres.deco.runin` decoration \
         at the start of the first paragraph",
    )
}

/// Stages `body` inside a span carrying `payload`.
fn span(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    payload: SpanPayload,
    body: Vec<BuildId>,
) -> Result<Vec<BuildId>, LowerError> {
    Ok(vec![lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)?])
}

/// The lowering rule [`TERM_RULE`]: a term link as a link to its term
/// definition. See the [module documentation](self#a-term-link).
///
/// A term link whose term is empty or only whitespace (`~` and `\ ` are part
/// of a term as written) is reported as [`EmptyTerm`] and shows its text
/// marked as unresolved. A term link whose term holds a placeholder in the
/// template of a kept definition ([kept
/// definitions](crate::stdcore::features::userdefs::kept)) shows its text and reports
/// nothing. A resolution that the resolutions table does not hold (content
/// that nobody resolved, or a synthesized value that its consumer did not
/// bring up to date through [value
/// catch-up](crate::guide::concepts_overview#value-catch-up)) is reported at
/// the node as `flm.run.missing-entry`, and the rule then fails.
///
/// Where the term link is inside the definition of its own term, the rule
/// requires the feature's style identifier [`STYLE`]
/// (`flm.defterm.style`), which each backend resolves as it resolves
/// requirements: the stylesheet on the `html` backend, the definition pieces
/// delivering it on the `latex-rendered` backend, nothing on the `text`
/// backend.
#[derive(Debug, Default, Clone, Copy)]
pub struct TermRule;

impl Lower for TermRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let decision = decide_term(&node);
        // The author's text is what this shows in every case, so it is
        // lowered once, before anything is looked up. A text a parse recovery
        // left absent lowers to nothing and is not reported a second time.
        let shown = lower_arg_named_or_nothing(cx, lw, &node, SHOWN_ARG)?;
        let unresolved = || SpanPayload {
            style: Some(SpanStyle::UnresolvedReference),
            ..SpanPayload::default()
        };
        let Some(decision) = decision else {
            // A template kept rather than expanded may hold a placeholder
            // where the term stands: that names nothing and is not
            // reported. Anything else naming nothing is an empty term.
            let written = |name: &str| node.argument_content_nodes_named(name).ok().flatten();
            let blank = [super::elements::REF_ARG, SHOWN_ARG].iter().all(|name| {
                written(name).is_none_or(|content| spelling_identity(content.iter()).is_empty())
            });
            if !blank {
                return Ok(Lowering::Emit(shown));
            }
            lw.error(EmptyTerm { what: String::from("term link") }, node.span().clone());
            return Ok(Lowering::Emit(span(cx, lw, unresolved(), shown)?));
        };
        if decision.in_own_definition {
            lw.require(&Name::from(STYLE));
            let payload = SpanPayload { role: Some(Name::from(TERM_ROLE)), ..SpanPayload::default() };
            return Ok(Lowering::Emit(span(cx, lw, payload, shown)?));
        }
        let site = SiteId { addr: lw.addr(), index: 0 };
        let resolution = match lw.get(REF_RESOLUTIONS)? {
            // No references feature: nothing resolves, and nothing is a
            // fault. The link is its text.
            None => return Ok(Lowering::Emit(shown)),
            Some(table) => table.site(site).cloned(),
        };
        let staged = match resolution {
            Some(Resolution::Resolved { href, .. }) => {
                let payload = LinkPayload {
                    href,
                    kind: TERM_LINK_KIND.into(),
                    doc_role: None,
                    label: None,
                    shows_target: false,
                };
                vec![lw.pres_with(cx, &ElementName::from(pres::LINK), payload, shown)?]
            }
            Some(Resolution::Unresolved { .. }) => span(cx, lw, unresolved(), shown)?,
            None => {
                let table = CapName::from(REF_RESOLUTIONS.name());
                let producer = lw
                    .producer_of(&table)
                    .map(|stage| stage.to_string())
                    .unwrap_or_else(|| "nobody".to_string());
                lw.diagnose(Diagnostic::error(
                    MissingEntry { table: table.to_string(), producer, element: TERM.to_string() },
                    node.span().clone(),
                ));
                return Err(LowerError::structure("the term link has no resolution entry"));
            }
        };
        Ok(Lowering::Emit(staged))
    }
}

/// The declaration of every rule that writes a term link as [`TermRule`]
/// does: it reads the references feature's resolutions table optionally and
/// produces a link or a span.
pub(super) fn term_decl() -> LoweringDecl {
    LoweringDecl::new().reads_opt(REF_RESOLUTIONS).produces(pres::LINK).produces(pres::SPAN)
}

/// The term link's general lowering rule, [`TERM_RULE`], the default on
/// every backend. It reads the references feature's resolutions table
/// optionally, so that a registry without that feature still renders a term
/// link, as its text.
pub(super) fn term_rule() -> LoweringRule {
    LoweringRule::new(
        TERM_RULE,
        TERM,
        term_decl(),
        Arc::new(TermRule),
        DefaultMark::Everywhere,
    )
    .docs(
        "a resolved term link lowers to `flm.pres.link` of kind `term` around the text written \
         for it, an unresolved one to an unresolved-styled `flm.pres.span`, one inside the \
         definition of its own term to `flm.pres.span` of role `flm.defterm.term`",
    )
}
