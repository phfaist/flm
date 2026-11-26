//! The synthesized elements of the metadata's pieces, and their default lowering
//! rules. Every backend uses these rules, except that the `latex-recomposed` backend
//! has rules of its own for each of them ([`latex`](super::latex)).
//!
//! The elements are [`TITLE`], [`AUTHOR`], [`AUTHOR_ENTRY`], [`AUTHOR_NAME`],
//! [`AUTHOR_AFFILIATION`], [`AUTHOR_EMAIL`], [`AUTHOR_NOTE`] and [`DATE`]. They are
//! synthesized: no document can write them, and the stage
//! [`BuildMetadata`](super::BuildMetadata) builds them from the metadata. The rules
//! of the three pieces stage a [`flm.pres.block`](crate::stdlib::backends::pres::BLOCK)
//! per piece, whose role is the element's name and whose kind token is
//! [`KIND_METADATA`], so that a stylesheet or a program's lowering rule can select
//! on them:
//!
//! - the title and the date ([`MetaTextRule`]): the block holds the text as one
//!   paragraph;
//! - the authors ([`AuthorRule`]): the block holds each author, one after the other,
//!   each a unit of lines ([`EntryRule`]): the name, each affiliation, the email
//!   address and each note on a line of its own, each in a
//!   [`flm.pres.span`](crate::stdlib::backends::pres::SPAN) whose role is the
//!   element's name ([`PartRule`], [`EmailRule`]). The email address is a link to
//!   `mailto:` followed by the address.
//!
//! ```text
//! flm.pres.block { role: flm.metadata.author, kinds: [metadata] }
//!   ├ flm.pres.lines                                      the first author
//!   │   ├ flm.pres.line
//!   │   │   └ flm.pres.span { role: flm.metadata.author.name }  the name
//!   │   ├ flm.pres.line
//!   │   │   └ flm.pres.span { role: flm.metadata.author.affiliation }  an affiliation
//!   │   ├ flm.pres.line
//!   │   │   └ flm.pres.span { role: flm.metadata.author.email }
//!   │   │       └ flm.pres.link { href: mailto:<address>, kind: url }  the address
//!   │   └ flm.pres.line
//!   │       └ flm.pres.span { role: flm.metadata.author.note }  a note
//!   └ flm.pres.lines                                      the next author, …
//! ```
//!
//! The rules write no affiliation's number ([`AffiliationPayload`](super::AffiliationPayload)).
//! How the authors are laid out beyond this (the size of each part, the authors
//! side by side) is the business of the feature's stylesheet ([`stylesheet`](super::stylesheet))
//! and LaTeX definitions ([`latex_rendered`](super::latex_rendered)), or of a
//! compose template's.
//!
//! The rules of the three pieces require the feature's style identifier [`STYLE`]
//! at every piece they stage: the `html` backend resolves it to the feature's
//! stylesheet, the `latex-rendered` backend to the feature's definition piece
//! ([`latex_rendered`](super::latex_rendered)).

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef, NodeSlice};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::mint::{ElementDef, SyntaxDef};
use crate::lang::Flm;
use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::ElementSpec;
use crate::model::names::{ElementName, Name};
use crate::stdlib::backends::lowering::lower_blocks_with_edges;
use crate::stdlib::backends::pres::{self, BlockPayload, LinePayload, LinesPayload, LinkPayload, SpanPayload};
use crate::stdlib::features::href::LINK_KIND;

use super::{
    EmailPayload, AUTHOR, AUTHOR_AFFILIATION, AUTHOR_EMAIL, AUTHOR_ENTRY, AUTHOR_NAME, AUTHOR_NOTE, DATE,
    KIND_METADATA, STYLE, TITLE,
};

/// The scheme written before an author's email address in the address of its
/// link, `mailto:`.
const MAILTO: &str = "mailto:";

/// Returns the name of the default lowering rule of the element `element`,
/// `<element>.lower.default`.
pub fn rule_name(element: &str) -> String {
    format!("{element}.lower.default")
}

/// Returns the payload of the block the rules of the three pieces stage: the
/// element's name as its role, and the kind token [`KIND_METADATA`].
fn block(role: &str) -> BlockPayload {
    BlockPayload { role: Some(Name::from(role)), kinds: vec![Name::from(KIND_METADATA)], anchor: None, doc_role: None }
}

/// Returns the payload of the span that holds a part of an author: the part's
/// element name as its role.
fn span(role: &str) -> SpanPayload {
    SpanPayload { role: Some(Name::from(role)), ..SpanPayload::default() }
}

/// Returns each child of `node` as a slice of one node, in order, as
/// [`Lowerer::lower_nodes`] takes them.
pub(super) fn each_child<'t>(node: &NodeRef<'t, Flm>) -> Result<Vec<NodeSlice<'t, Flm>>, LowerError> {
    let children = node.children();
    let start = children.range().start;
    (0u32..)
        .zip(children.iter())
        .map(|(index, _)| {
            node.tree()
                .slice(start + index..start + index + 1)
                .ok_or_else(|| LowerError::structure("a child outside its parent's children"))
        })
        .collect()
}

/// Lowers the content of `node`, one run of inline text, writing each paragraph
/// the block-structure pass placed the content in as its content alone.
pub(super) fn inline_content(
    node: &NodeRef<'_, Flm>,
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
) -> Result<Vec<BuildId>, LowerError> {
    let paragraph = Arc::clone(&lw.lang().structural().paragraph);
    let mut content = Vec::new();
    for (one, child) in each_child(node)?.into_iter().zip(node.children().iter()) {
        if child.ext().element.name == paragraph.name {
            content.extend(lw.lower_children(cx, &child)?);
        } else {
            content.extend(lw.lower_nodes(cx, one)?);
        }
    }
    Ok(content)
}

/// The default lowering rule of the title and of the date, the elements
/// [`TITLE`] and [`DATE`], on every backend except `latex-recomposed`. It stages
///
/// ```text
/// flm.pres.block { role: <the element's name>, kinds: [metadata] }
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
/// backend except `latex-recomposed`. It stages a `flm.pres.block` of role
/// `flm.metadata.author` and kind token `metadata` holding each author, lowered
/// through its rule ([`EntryRule`]), in order.
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
        let body = lw.lower_children(cx, &node)?;
        Ok(Lowering::Emit(vec![lw.pres_with(cx, &ElementName::from(pres::BLOCK), block(AUTHOR), body)?]))
    }
}

/// The default lowering rule of one author, the element [`AUTHOR_ENTRY`], on every
/// backend except `latex-recomposed`. It stages one `flm.pres.lines` holding one
/// `flm.pres.line` per part of the author (the name, each affiliation, the email
/// address, each note, in this order), each line holding its part lowered through
/// the part's rule ([`PartRule`], [`EmailRule`]). The lines carry no indentation
/// and the default alignment.
#[derive(Debug, Default)]
pub struct EntryRule;

impl Lower for EntryRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let mut lines = Vec::new();
        for one in each_child(&node)? {
            let part = lw.lower_nodes(cx, one)?;
            lines.push(lw.pres_with(cx, &ElementName::from(pres::LINE), LinePayload::default(), part)?);
        }
        Ok(Lowering::Emit(vec![lw.pres_with(cx, &ElementName::from(pres::LINES), LinesPayload, lines)?]))
    }
}

/// The default lowering rule of an author's name, of an affiliation and of a note,
/// the elements [`AUTHOR_NAME`], [`AUTHOR_AFFILIATION`] and [`AUTHOR_NOTE`], on every
/// backend except `latex-recomposed`. It stages a `flm.pres.span` whose role is the
/// element's name, holding the text lowered through the rules.
#[derive(Debug)]
pub struct PartRule {
    role: &'static str,
}

impl Lower for PartRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let content = inline_content(&node, cx, lw)?;
        Ok(Lowering::Emit(vec![lw.pres_with(cx, &ElementName::from(pres::SPAN), span(self.role), content)?]))
    }
}

/// The default lowering rule of an author's email address, the element
/// [`AUTHOR_EMAIL`], on every backend except `latex-recomposed`. It stages
///
/// ```text
/// flm.pres.span { role: flm.metadata.author.email }
///   └ flm.pres.link { href: mailto:<address>, kind: url, shows_target }
///       └ the address
/// ```
///
/// The link is of kind `url`, the kind of a link to an address outside the
/// document, and shows its target: its content is the address.
#[derive(Debug, Default)]
pub struct EmailRule;

impl Lower for EmailRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let address = email_of(&node)?;
        let payload = LinkPayload {
            href: format!("{MAILTO}{address}"),
            kind: Name::from(LINK_KIND),
            doc_role: None,
            label: None,
            shows_target: true,
        };
        let text = lw.text(cx, address)?;
        let link = lw.pres_with(cx, &ElementName::from(pres::LINK), payload, vec![text])?;
        Ok(Lowering::Emit(vec![lw.pres_with(cx, &ElementName::from(pres::SPAN), span(AUTHOR_EMAIL), vec![link])?]))
    }
}

/// Returns the address that the email node `node` carries.
///
/// # Errors
///
/// [`LowerError::Structure`] for a node that carries no [`EmailPayload`].
pub(super) fn email_of<'n>(node: &'n NodeRef<'_, Flm>) -> Result<&'n str, LowerError> {
    EmailPayload::of(node)
        .map(|payload| payload.address.as_str())
        .ok_or_else(|| LowerError::structure("missing ‘EmailPayload’ on an author’s email node"))
}

/// Declares, for the feature `b` registers, the synthesized elements and their
/// default lowering rules.
pub(super) fn declare(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    let owner = b.feature().clone();
    let elements = [
        (TITLE, "The document's title.", "The title of the document, built from its metadata."),
        (
            AUTHOR,
            "The document's authors, one author per child.",
            "The list of the document's authors, built from its metadata.",
        ),
        (
            AUTHOR_ENTRY,
            "One author of the document: the name, then each affiliation, the email address and each note.",
            "One author of the document, in the list of its authors.",
        ),
        (AUTHOR_NAME, "An author's name.", "The name of one author of the document."),
        (
            AUTHOR_AFFILIATION,
            "One affiliation of an author, with its number among the document's affiliations.",
            "One affiliation of one author of the document.",
        ),
        (
            AUTHOR_EMAIL,
            "An author's email address, a plain text.",
            "The email address of one author of the document.",
        ),
        (AUTHOR_NOTE, "One note about an author.", "One note about one author of the document."),
        (DATE, "The document's date.", "The date of the document, built from its metadata."),
    ];
    for (name, docs, type_docs) in elements {
        b.element_type_text(name, type_docs)?;
        let role = Arc::new(FixedBlockRole(BlockRole::Block)) as Arc<dyn BlockRoleAspect>;
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
    for element in [TITLE, DATE] {
        b.lowering_rule(
            LoweringRule::new(
                rule_name(element),
                element,
                LoweringDecl::new().produces(pres::BLOCK).produces(pres::PAR),
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
            LoweringDecl::new().produces(pres::BLOCK),
            Arc::new(AuthorRule),
            DefaultMark::Everywhere,
        )
        .docs("the authors lower to a `flm.pres.block` holding each author in order"),
    )?;
    b.lowering_rule(
        LoweringRule::new(
            rule_name(AUTHOR_ENTRY),
            AUTHOR_ENTRY,
            LoweringDecl::new().produces(pres::LINES).produces(pres::LINE),
            Arc::new(EntryRule),
            DefaultMark::Everywhere,
        )
        .docs("an author lowers to a `flm.pres.lines`, each part of the author on a `flm.pres.line` of its own"),
    )?;
    for element in [AUTHOR_NAME, AUTHOR_AFFILIATION, AUTHOR_NOTE] {
        b.lowering_rule(
            LoweringRule::new(
                rule_name(element),
                element,
                LoweringDecl::new().produces(pres::SPAN),
                Arc::new(PartRule { role: element }),
                DefaultMark::Everywhere,
            )
            .docs(format!("`{element}` lowers to a `flm.pres.span` of role `{element}` holding the text")),
        )?;
    }
    b.lowering_rule(
        LoweringRule::new(
            rule_name(AUTHOR_EMAIL),
            AUTHOR_EMAIL,
            LoweringDecl::new().produces(pres::SPAN).produces(pres::LINK),
            Arc::new(EmailRule),
            DefaultMark::Everywhere,
        )
        .docs(
            "an email address lowers to a `flm.pres.span` of role `flm.metadata.author.email` holding a \
             `flm.pres.link` of kind `url` to `mailto:` followed by the address, showing the address",
        ),
    )?;
    Ok(())
}
