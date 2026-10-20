//! The synthesized elements of the table of contents and their general
//! lowering rules, the default on every backend.
//!
//! The three elements are [`PIECE`] (the root), [`LIST`] and [`ENTRY`]. They
//! are synthesized: no document can write them, and the stage
//! [`BuildToc`](super::BuildToc) builds them. Each declares the block role
//! `Block`, so the block-structure pass, which runs over the value before it
//! is lowered, wraps the title inside an entry in one paragraph, before the
//! entry's sub-list; the entry's rule writes that paragraph as its content
//! alone. The rules stage:
//!
//! - the root ([`RootRule`]): a `flm.pres.block` of role `flm.toc`, kind
//!   token `toc` and document role [`DocRole::Toc`], holding its list;
//! - a list ([`ListRule`]): an unlabeled `flm.pres.list` of kind tokens
//!   `unnumbered` and `toc`, each entry with an empty label;
//! - an entry ([`EntryRule`]): the number in a span of role
//!   [`NUMBER_ROLE`] and a space, where the heading has a number; the
//!   title; both inside one link to the heading's anchor where it has one;
//!   then the sub-list, where there is one.
//!
//! The rules read the counters and anchors tables where they are computed,
//! and are registered with peer declarations of those tables and of the
//! presentation elements they produce, so that the feature builds in a
//! registry without the `numbering` or `refs` features.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};
use techy::error::Diagnostic;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::core::run::MissingEntry;
use crate::lang::mint::{ElementDef, SyntaxDef};
use crate::lang::Flm;
use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::ElementSpec;
use crate::model::names::{CapName, ElementName, Name};
use crate::stdcore::backends::pres::{
    self, BlockPayload, DocRole, LinkPayload, ListPayload, SpanPayload, ITEM_SLOT, LABEL_SLOT,
    LIST_UNNUMBERED,
};
use crate::stdcore::features::numbering::{Counters, FormatOptions, COUNTERS};
use crate::stdcore::features::refs::{Anchors, ANCHORS};

use super::{TocEntryPayload, ENTRY, KIND_TOC, LIST, NUMBER_ROLE, PIECE, STYLE};

/// The link kind of an entry's link, `toc`.
pub const LINK_KIND: &str = "toc";

/// Returns the name of the general lowering rule of the element `element`,
/// `<element>.lower.default`.
pub fn rule_name(element: &str) -> String {
    format!("{element}.lower.default")
}

/// The general lowering rule of the root of the table of contents, the
/// element [`PIECE`], `flm.toc.lower.default`. It stages
///
/// ```text
/// flm.pres.block { role: flm.toc, kinds: [toc], doc_role: Toc }
///   └ the root's children, lowered: its list
/// ```
///
/// It requires the feature's style identifier [`STYLE`] (`flm.toc.style`),
/// which each backend resolves as it resolves requirements: the stylesheet on
/// the `html` backend, the definition pieces delivering it on the
/// `latex-rendered` backend, nothing on the `text` backend.
#[derive(Debug, Default, Clone, Copy)]
pub struct RootRule;

impl Lower for RootRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lw.require(&Name::from(STYLE));
        let content = lw.lower_children(cx, &node)?;
        let block = BlockPayload {
            role: Some(Name::from(PIECE)),
            kinds: vec![Name::from(KIND_TOC)],
            anchor: None,
            doc_role: Some(DocRole::Toc),
        };
        Ok(Lowering::Emit(vec![lw.pres_with(cx, &ElementName::from(pres::BLOCK), block, content)?]))
    }
}

/// The general lowering rule of a list of entries, the element [`LIST`],
/// `flm.toc.list.lower.default`. It stages
///
/// ```text
/// flm.pres.list { kinds: [unnumbered, toc] }
///   per entry:  label ⇒ nothing
///               item  ⇒ the entry, lowered
/// ```
///
/// The list carries no `labeled` token, no anchor and no entry anchors; every
/// entry has its two slots, the label slot empty, as the `itemize` list of the
/// `lists` feature writes an item without a label. A child that is not an
/// entry, which the stage never builds, is not written.
#[derive(Debug, Default, Clone, Copy)]
pub struct ListRule;

impl Lower for ListRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let mut slots: Vec<(String, Vec<BuildId>)> = Vec::new();
        for one in each_child(&node)? {
            let is_entry = one.first().is_some_and(|child| child.ext().element.name.as_str() == ENTRY);
            if !is_entry {
                continue;
            }
            slots.push((LABEL_SLOT.to_string(), Vec::new()));
            slots.push((ITEM_SLOT.to_string(), lw.lower_nodes(cx, one)?));
        }
        let payload = ListPayload {
            kinds: vec![Name::from(LIST_UNNUMBERED), Name::from(KIND_TOC)],
            anchor: None,
            entry_anchors: Vec::new(),
        };
        Ok(Lowering::Emit(vec![lw.pres_slots_with(cx, &ElementName::from(pres::LIST), payload, slots)?]))
    }
}

/// Returns each child of `node` as a slice of one node, in order, as
/// [`Lowerer::lower_nodes`] takes them.
fn each_child<'t>(node: &NodeRef<'t, Flm>) -> Result<Vec<techy::core::node::NodeSlice<'t, Flm>>, LowerError> {
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

/// The general lowering rule of one entry, the element [`ENTRY`],
/// `flm.toc.entry.lower.default`. It reads the entry's payload
/// ([`TocEntryPayload`]) and stages
///
/// ```text
/// flm.pres.link { href: "#<anchor>", kind: toc, shows_target: true }
///   ├ flm.pres.span { role: flm.toc.entry.number }  the number
///   ├ " "
///   └ the title
/// the sub-list, lowered, where the entry holds one
/// ```
///
/// - The **number** is the point's number as the counters table writes it,
///   without the word and the delimiters a reference writes
///   ([`FormatOptions::without_word`], [`FormatOptions::without_delimiters`]),
///   and without the heading kind's title template. Where the counters table
///   holds no number for the point (an unnumbered heading, or no `numbering`
///   feature), there is no number span and no space.
/// - The **title** is the entry's content other than its sub-list, each
///   paragraph the block-structure pass made written as its content alone.
/// - The **link** stands around the number and the title where the anchors
///   table gives the point an anchor; otherwise they stand bare. The link has
///   no document role and no accessible name, and says that its content is
///   the target's own display. A point the anchors table never saw is the
///   Error `flm.run.missing-entry` ([`MissingEntry`]), and the entry is
///   written without a link.
///
/// An entry with no point has no number and no link.
#[derive(Debug, Default, Clone, Copy)]
pub struct EntryRule;

impl Lower for EntryRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let point = TocEntryPayload::of(&node).and_then(|payload| payload.point);
        // The number's segments and the anchor are read before anything is
        // staged, since reading a table borrows the façade the staging needs.
        let segments = match (point, lw.get(COUNTERS)?) {
            (Some(point), Some(counters)) => number(counters, point)?,
            _ => Vec::new(),
        };
        let anchor = match point {
            Some(point) => anchor(&node, point, lw)?,
            None => None,
        };
        let mut shown = Vec::new();
        if !segments.is_empty() {
            let mut number = Vec::new();
            for segment in &segments {
                number.extend(lw.splice(cx, segment.content())?);
            }
            let span = SpanPayload { role: Some(Name::from(NUMBER_ROLE)), ..SpanPayload::default() };
            shown.push(lw.pres_with(cx, &ElementName::from(pres::SPAN), span, number)?);
            shown.push(lw.text(cx, " ")?);
        }
        let paragraph = Arc::clone(&lw.lang().structural().paragraph);
        let mut lists = Vec::new();
        for (one, child) in each_child(&node)?.into_iter().zip(node.children().iter()) {
            let name = &child.ext().element.name;
            if name.as_str() == LIST {
                lists.push(one);
            } else if *name == paragraph.name {
                shown.extend(lw.lower_children(cx, &child)?);
            } else {
                shown.extend(lw.lower_nodes(cx, one)?);
            }
        }
        let mut staged = match anchor {
            Some(anchor) => {
                let link = LinkPayload {
                    href: format!("#{anchor}"),
                    kind: Name::from(LINK_KIND),
                    doc_role: None,
                    label: None,
                    shows_target: true,
                };
                vec![lw.pres_with(cx, &ElementName::from(pres::LINK), link, shown)?]
            }
            None => shown,
        };
        for one in lists {
            staged.extend(lw.lower_nodes(cx, one)?);
        }
        Ok(Lowering::Emit(staged))
    }
}

/// The segments of the number of `point`, bare; empty where the counters
/// table holds no number for it.
fn number(counters: &Counters, point: crate::model::aspects::PointId) -> Result<Vec<crate::stdcore::features::numbering::NumberSegment>, LowerError> {
    let options = FormatOptions::new().without_word().without_delimiters();
    match counters.format(point, &options) {
        None => Ok(Vec::new()),
        Some(result) => result.map_err(|error| LowerError::structure(error.to_string())),
    }
}

/// The anchor of `point`, the first reference point of the entry `node`'s
/// heading: `None` where the plan computes no anchors, or where the anchors
/// stage gave the point none. A point the anchors stage never saw is
/// reported as [`MissingEntry`], and has no anchor.
fn anchor(
    node: &NodeRef<'_, Flm>,
    point: crate::model::aspects::PointId,
    lw: &mut dyn Lowerer,
) -> Result<Option<String>, LowerError> {
    let found: Option<Option<String>> = match lw.get(ANCHORS)? {
        None => return Ok(None),
        Some(anchors) => anchors.lookup(point).map(|entry| entry.map(String::from)),
    };
    match found {
        Some(anchor) => Ok(anchor),
        None => {
            let table = CapName::from(ANCHORS.name());
            let producer =
                lw.producer_of(&table).map(|stage| stage.to_string()).unwrap_or_else(|| "nobody".to_string());
            lw.diagnose(Diagnostic::error(
                MissingEntry { table: table.to_string(), producer, element: ENTRY.to_string() },
                node.span().clone(),
            ));
            Ok(None)
        }
    }
}

/// What the rules of the three elements read and produce: the counters and
/// anchors tables, both optionally, and the presentation elements they
/// stage.
pub(super) fn decl() -> LoweringDecl {
    LoweringDecl::new()
        .reads_opt(COUNTERS)
        .reads_opt(ANCHORS)
        .produces(pres::BLOCK)
        .produces(pres::LIST)
        .produces(pres::SPAN)
        .produces(pres::LINK)
        .produces(pres::PAR)
}

/// Makes, for the feature that `b` registers, the peer declarations
/// ([`PeerName`](crate::core::registry::PeerName)) of the tables and the
/// presentation elements the rules use, as the lists of xnotes declare them,
/// so that the feature builds in a registry without the features that own
/// them.
fn declare_peers(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_table::<Counters>(COUNTERS.name(), "the numbering feature's counter table: each entry's number")?;
    b.peer_table::<Anchors>(ANCHORS.name(), "the references feature's anchors table: each entry's link target")?;
    b.peer_element::<BlockPayload>(pres::BLOCK, "the table of contents")?;
    b.peer_element::<ListPayload>(pres::LIST, "a list of entries of the table of contents")?;
    b.peer_element::<SpanPayload>(pres::SPAN, "the number of an entry of the table of contents")?;
    b.peer_element::<LinkPayload>(pres::LINK, "the link from an entry of the table of contents to its heading")?;
    b.peer_element::<pres::ParPayload>(pres::PAR, "a paragraph inside an entry's title")
}

/// Declares, for the feature `b` registers, the three synthesized elements,
/// their general lowering rules, and the peer declarations the rules need.
pub(super) fn declare(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    let owner = b.feature().clone();
    let elements = [
        (PIECE, "The table of contents: one list of entries."),
        (LIST, "A list of entries of the table of contents."),
        (ENTRY, "One entry of the table of contents: the title, then the list of its sub-entries."),
    ];
    for (name, docs) in elements {
        let role = Arc::new(FixedBlockRole(BlockRole::Block)) as Arc<dyn BlockRoleAspect>;
        b.element(
            ElementDef::new(
                owner.clone(),
                Vec::<&str>::new(),
                SyntaxDef::Synthesized,
                Arc::new(ElementSpec::new(name).docs(docs).with(role)),
            )
            .docs(format!("{docs} No document can write it: a stage builds it from the headings.")),
        )?;
    }
    declare_peers(b)?;
    let rules: [(&str, Arc<dyn Lower>, &str); 3] = [
        (
            PIECE,
            Arc::new(RootRule),
            "the table of contents lowers to a `flm.pres.block` of role `flm.toc` and document role \
             `toc`, holding its list",
        ),
        (
            LIST,
            Arc::new(ListRule),
            "a list of entries lowers to an unlabeled `flm.pres.list` of kinds `unnumbered` and `toc`",
        ),
        (
            ENTRY,
            Arc::new(EntryRule),
            "an entry lowers to its number and its title inside a link to its heading, then its \
             sub-list",
        ),
    ];
    for (element, rule, docs) in rules {
        b.lowering_rule(
            LoweringRule::new(rule_name(element), element, decl(), rule, DefaultMark::Everywhere).docs(docs),
        )?;
    }
    Ok(())
}
