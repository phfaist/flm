//! The lowering rules of the metadata's pieces on the `latex-recomposed` backend: the
//! title, the authors and the date written back as the LaTeX text that a LaTeX
//! document passes to `\title`, `\author` and `\date`.
//!
//! - The title and the date are their content, written back as LaTeX
//!   ([`MetaTextLatexRule`]): `On \emph{flexible} markup`.
//! - The authors are each author's content, written back as LaTeX and joined by
//!   LaTeX's `\and` ([`AuthorLatexRule`]): `Ada Lovelace \and Grace Hopper`.
//!
//! The rules are the defaults on the `latex-recomposed` backend only. That backend
//! places a piece only where the render options of the `pieces` feature list it, and
//! leaves the piece out of its entry artifact (the file that stands for the output;
//! [`ArtifactSet`](crate::core::artifact::ArtifactSet)) for the program to place ([pieces in the
//! recomposed LaTeX](crate::guide::rendering#pieces-in-the-recomposed-latex)).
//!
//! The feature registers the rules with a peer declaration
//! ([`PeerName`](crate::core::registry::PeerName)) of the backend and of the macro
//! construct `flm.pres.latex.macro` the authors' rule stages, so that a registry
//! without the LaTeX features still builds, with the rules set aside (excluded from
//! rule selection).

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::model::names::ElementName;
use crate::stdcore::backends::latex::construct::MACRO;
use crate::stdcore::backends::latex::LatexMacroPayload;
use crate::stdcore::backends::latex_recomposed::BACKEND;

use super::{AUTHOR, DATE, TITLE};
use super::rules::author_names;

/// Returns the name of the lowering rule of the element `element` on the
/// `latex-recomposed` backend, `<element>.lower.latex-recomposed`.
pub fn latex_rule_name(element: &str) -> String {
    format!("{element}.lower.latex-recomposed")
}

/// Lowers the content of `node`, writing each paragraph the block-structure pass
/// placed the content in as its content alone, because the content of a piece is
/// one run of inline text.
fn inline_content(node: &NodeRef<'_, Flm>, cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer) -> Result<Vec<BuildId>, LowerError> {
    let paragraph = Arc::clone(&lw.lang().structural().paragraph);
    let children = node.children();
    let start = children.range().start;
    let mut content = Vec::new();
    for (index, child) in (0u32..).zip(children.iter()) {
        if child.ext().element.name == paragraph.name {
            content.extend(lw.lower_children(cx, &child)?);
        } else {
            let one = node
                .tree()
                .slice(start + index..start + index + 1)
                .ok_or_else(|| LowerError::structure("a child outside its parent's children"))?;
            content.extend(lw.lower_nodes(cx, one)?);
        }
    }
    Ok(content)
}

/// The lowering rule of the title and of the date on the `latex-recomposed`
/// backend: the content, written back as LaTeX.
#[derive(Debug, Default)]
pub struct MetaTextLatexRule;

impl Lower for MetaTextLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(inline_content(&node, cx, lw)?))
    }
}

/// The lowering rule of the authors on the `latex-recomposed` backend: each
/// author's content, written back as LaTeX, the authors joined by ` \and `.
#[derive(Debug, Default)]
pub struct AuthorLatexRule;

impl Lower for AuthorLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let mut names = Vec::new();
        author_names(&node, &mut names);
        let mut joined = Vec::new();
        for name in names {
            if !joined.is_empty() {
                joined.push(lw.text(cx, " ")?);
                let and = LatexMacroPayload::new("and", []);
                joined.push(lw.pres_slots_with(cx, &ElementName::from(MACRO), and, Vec::new())?);
                joined.push(lw.text(cx, " ")?);
            }
            joined.extend(inline_content(&name, cx, lw)?);
        }
        Ok(Lowering::Emit(joined))
    }
}

/// Registers the three rules, with the peer declarations of the backend and of
/// the macro construct they need.
pub(super) fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the metadata's LaTeX rules are the default",
    )?;
    b.peer_element::<LatexMacroPayload>(MACRO, "the LaTeX family's macro-call construct, which the authors' rule stages for `\\and` between two authors")?;
    for element in [TITLE, DATE] {
        b.lowering_rule(
            LoweringRule::new(
                latex_rule_name(element),
                element,
                LoweringDecl::new(),
                Arc::new(MetaTextLatexRule),
                DefaultMark::OnBackends(vec![BACKEND.into()]),
            )
            .docs(format!("on the recomposed LaTeX, `{element}` is its content, written back as LaTeX")),
        )?;
    }
    b.lowering_rule(
        LoweringRule::new(
            latex_rule_name(AUTHOR),
            AUTHOR,
            LoweringDecl::new().produces(MACRO),
            Arc::new(AuthorLatexRule),
            DefaultMark::OnBackends(vec![BACKEND.into()]),
        )
        .docs("on the recomposed LaTeX, the authors are each author's content, joined by `\\and`"),
    )
}
