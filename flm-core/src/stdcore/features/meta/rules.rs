//! The synthesized elements of the metadata's pieces, and their default lowering
//! rules. Every backend uses these rules, except that the `latex-recomposed` backend
//! has rules of its own for the title, the authors and the date
//! ([`latex`](super::latex)).
//!
//! The four elements are [`TITLE`], [`AUTHOR`], [`AUTHOR_NAME`] and [`DATE`]. They are
//! synthesized: no document can write them, and the stage
//! [`BuildMeta`](super::BuildMeta) builds them from the metadata. The rules stage a
//! [`flm.pres.block`](crate::stdcore::backends::pres::BLOCK) per piece, whose role is
//! the element's name and whose kind token is [`KIND_META`], so that a stylesheet or a
//! program's lowering rule can select on them:
//!
//! - the title and the date ([`MetaTextRule`]): the block holds the text as one
//!   paragraph;
//! - the authors ([`AuthorRule`]): the block holds one paragraph in which each author
//!   is a [`flm.pres.span`](crate::stdcore::backends::pres::SPAN) of role
//!   `flm.meta.author.name`, the authors separated by [`AUTHOR_SEPARATOR`].
//!
//! Both rules require the feature's style identifier [`STYLE`] at every
//! piece they stage: the `html` backend resolves it to the feature's stylesheet, the
//! `latex-rendered` backend to the feature's definition piece
//! ([`latex_rendered`](super::latex_rendered)).
//!
//! An [`AUTHOR_NAME`] node is lowered by the authors' rule; its own rule, which runs
//! only for an author outside the authors piece, fails with a structure error.

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::mint::{ElementDef, SyntaxDef};
use crate::lang::Flm;
use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::ElementSpec;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::lowering::lower_blocks_with_edges;
use crate::stdcore::backends::pres::{self, BlockPayload, SpanPayload};

use super::{AUTHOR, AUTHOR_NAME, DATE, KIND_META, STYLE, TITLE};

/// The text written between two authors by [`AuthorRule`], `, `.
pub const AUTHOR_SEPARATOR: &str = ", ";

/// Returns the name of the default lowering rule of the element `element`,
/// `<element>.lower.default`.
pub fn rule_name(element: &str) -> String {
    format!("{element}.lower.default")
}

/// Returns the payload of the block the rules of the three pieces stage: the
/// element's name as its role, and the kind token [`KIND_META`].
fn block(role: &str) -> BlockPayload {
    BlockPayload { role: Some(Name::from(role)), kinds: vec![Name::from(KIND_META)], anchor: None, doc_role: None }
}

/// The default lowering rule of the title and of the date, the elements
/// [`TITLE`] and [`DATE`], on every backend except `latex-recomposed`. It stages
///
/// ```text
/// flm.pres.block { role: <the element's name>, kinds: [meta] }
///   └ flm.pres.par
///       └ the text, lowered through the rules
/// ```
#[derive(Debug)]
pub struct MetaTextRule {
    role: &'static str,
}

impl Lower for MetaTextRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lw.require(&Name::from(STYLE));
        let content: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
        let body = lower_blocks_with_edges(cx, lw, &content, Vec::new(), Vec::new())?;
        Ok(Lowering::Emit(vec![lw.pres_with(cx, &ElementName::from(pres::BLOCK), block(self.role), body)?]))
    }
}

/// The default lowering rule of the authors, the element [`AUTHOR`], on every
/// backend except `latex-recomposed`. It stages
///
/// ```text
/// flm.pres.block { role: flm.meta.author, kinds: [meta] }
///   └ flm.pres.par
///       ├ flm.pres.span { role: flm.meta.author.name }  the first author
///       ├ ", "
///       └ flm.pres.span { role: flm.meta.author.name }  the next author, …
/// ```
///
/// Each author is in a span of its own, so that a backend or a stylesheet can
/// tell the authors apart; they are separated by [`AUTHOR_SEPARATOR`]. Any other
/// content below the node (which the stage [`BuildMeta`](super::BuildMeta)
/// never builds) is not written.
#[derive(Debug, Default)]
pub struct AuthorRule;

impl Lower for AuthorRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lw.require(&Name::from(STYLE));
        let mut names = Vec::new();
        author_names(&node, &mut names);
        let mut joined = Vec::new();
        for name in names {
            if !joined.is_empty() {
                joined.push(lw.text(cx, AUTHOR_SEPARATOR)?);
            }
            let content = lw.lower_children(cx, &name)?;
            let payload = SpanPayload { role: Some(Name::from(AUTHOR_NAME)), ..SpanPayload::default() };
            joined.push(lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, content)?);
        }
        let body = lower_blocks_with_edges(cx, lw, &[], joined, Vec::new())?;
        Ok(Lowering::Emit(vec![lw.pres_with(cx, &ElementName::from(pres::BLOCK), block(AUTHOR), body)?]))
    }
}

/// Collects every [`AUTHOR_NAME`] node below `node` into `names`, in document
/// order. The block-structure pass places the authors inside a paragraph of the
/// authors' node, so they are looked for below its children too; the content of
/// an author is not searched.
pub(super) fn author_names<'t>(node: &NodeRef<'t, Flm>, names: &mut Vec<NodeRef<'t, Flm>>) {
    for child in node.children().iter() {
        if child.ext().element.name.as_str() == AUTHOR_NAME {
            names.push(child);
        } else {
            author_names(&child, names);
        }
    }
}

/// The rule of an author node outside the authors' node, which the stage never
/// builds: a structure error, because the authors' rule lowers each author
/// itself.
#[derive(Debug, Default)]
struct AuthorOutsideList;

impl Lower for AuthorOutsideList {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut LowerCx<'_>,
        _lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Err(LowerError::structure("an author must be inside the list of the document's authors"))
    }
}

/// Declares, for the feature `b` registers, the four synthesized elements and
/// their default lowering rules.
pub(super) fn declare(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    let owner = b.feature().clone();
    let elements = [
        (TITLE, BlockRole::Block, "The document's title."),
        (AUTHOR, BlockRole::Block, "The document's authors, one author per child."),
        (AUTHOR_NAME, BlockRole::DEFAULT, "One author of the document."),
        (DATE, BlockRole::Block, "The document's date."),
    ];
    for (name, role, docs) in elements {
        let role = Arc::new(FixedBlockRole(role)) as Arc<dyn BlockRoleAspect>;
        b.element(
            ElementDef::new(
                owner.clone(),
                Vec::<&str>::new(),
                SyntaxDef::Synthesized,
                Arc::new(ElementSpec::new(name).docs(docs).with(role)),
            )
            .docs(format!("{docs} No document can write it: a stage builds it from the metadata.")),
        )?;
    }
    let block_rule = |decl: LoweringDecl| decl.produces(pres::BLOCK).produces(pres::PAR);
    for element in [TITLE, DATE] {
        b.lowering_rule(
            LoweringRule::new(
                rule_name(element),
                element,
                block_rule(LoweringDecl::new()),
                Arc::new(MetaTextRule { role: element }),
                DefaultMark::Everywhere,
            )
            .docs(format!("`{element}` lowers to a `flm.pres.block` holding the text as one paragraph")),
        )?;
    }
    b.lowering_rule(
        LoweringRule::new(
            rule_name(AUTHOR),
            AUTHOR,
            block_rule(LoweringDecl::new()).produces(pres::SPAN),
            Arc::new(AuthorRule),
            DefaultMark::Everywhere,
        )
        .docs(
            "the authors lower to a `flm.pres.block` holding one paragraph, each author a \
             `flm.pres.span`, joined by `, `",
        ),
    )?;
    b.lowering_rule(
        LoweringRule::new(
            rule_name(AUTHOR_NAME),
            AUTHOR_NAME,
            LoweringDecl::new(),
            Arc::new(AuthorOutsideList),
            DefaultMark::Everywhere,
        )
        .docs("an author is lowered by the rule of the authors, never on its own"),
    )?;
    Ok(())
}
