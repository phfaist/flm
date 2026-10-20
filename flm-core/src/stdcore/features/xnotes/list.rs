//! The list of the xnotes of one xnote kind: its declarations, which
//! [`contribute_kind`](super::contribute_kind) makes for every kind; its
//! title, compiled when the registry is built; and its general lowering rule,
//! [`ListRule`].
//!
//! The list is a piece: a document-level [synthesized
//! value](crate::guide::concepts_overview#synthesized-values) that a backend
//! places beside the document's content ([`Piece`]). The kind's builder
//! ([`ListBuilder`](super::ListBuilder)) writes it into the kind's table
//! `flm.xnotes.list.<kind>`, and each backend lowers it with its rules and
//! places it after the content. This module holds the rule that lowers the
//! list element `flm.xnotes.<kind>.list` into presentation elements.

use std::sync::{Arc, OnceLock};

use techy::core::node::{BuildId, NodeRef, NodeTree};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::piece::{Piece, PieceDecl, PiecePlace};
use crate::core::registry::{BuildError, FeatureBuilder, LateInit, LateInitError};
use crate::lang::rewrites::userdefs::template::{PlaceholderVocabulary, TemplateLanguage};
use crate::lang::{BuiltLang, Flm, FlmDiagnostics};
use crate::model::element::element_value;
use crate::model::names::{ElementName, Name, RuleName};
use crate::stdcore::backends::lowering::ContentPointer;
use crate::stdcore::backends::pieces;
use crate::stdcore::backends::pres::{
    self, BlockPayload, DocRole, HeadingPayload, ListPayload, SpanPayload, SpanStyle, ITEM_SLOT,
    LABEL_SLOT, LIST_LABELED, LIST_NUMBERED, LIST_UNNUMBERED,
};
use crate::stdcore::features::numbering::{
    compile_text, instantiate_fixed, Counters, COUNTERS,
};
use crate::stdcore::features::refs::{self, ANCHORS};

use super::config::XnoteKind;
use super::lower::{append_to_last_paragraph, stage_mark_text};
use super::pointer::lower_pointed;
use super::stages::{ListBuilder, XnoteRecord, Xnotes, XNOTES};
use super::{
    list_element_name, piece_name, XnotesListElement, CONTENT_POINTER, ENTRY, ENTRY_MARK_ROLE,
    KIND_XNOTES, STYLE,
};

/// The name of the general lowering rule of the list element of the xnote
/// kind `kind`, `flm.xnotes.<kind>.list.lower.default` ([`ListRule`]).
pub fn list_rule_name(kind: &str) -> RuleName {
    RuleName::from(format!("{}.lower.default", list_element_name(kind)))
}

/// The heading name under which every list of xnotes stages its title,
/// `xnotes`: a backend's options that map heading names (such as the HTML
/// render option
/// [`heading_tags`](crate::stdcore::backends::html::HtmlRenderOptions::heading_tags))
/// map it as they map the name of a heading kind ([`HeadingPayload::name`]).
pub const TITLE_HEADING: &str = KIND_XNOTES;

/// Makes the declarations of the list of the xnote kind `kind`, on behalf of
/// the feature that `b` registers (the piece table, the builder stage that
/// writes it, the piece's declaration, the list's title and its general
/// lowering rule), with the peer declarations the rule needs, and returns the
/// rule, which a backend's own rule of the list calls
/// ([`ListRule::lower_list`]).
pub(super) fn declare_list(
    b: &mut FeatureBuilder<'_>,
    kind: &Arc<XnoteKind>,
) -> Result<Arc<ListRule>, BuildError> {
    let name = kind.name.as_str();
    b.table_named::<Piece>(
        piece_name(name),
        format!(
            "The list of the `{name}` xnotes of the document: a synthesized value, or nothing to \
             place."
        ),
    )?;
    b.stage(Arc::new(ListBuilder::new(Arc::clone(kind))))?;
    // The pieces feature owns the slot, and may be absent: the list is then
    // placed by no backend.
    b.peer_slot(
        pieces::SLOT,
        Arc::new(PieceDecl::new(
            piece_name(name),
            PiecePlace::AfterContent,
            format!("the list of the `{name}` xnotes, after the document's content"),
        )),
    )?;
    let title = Arc::new(Title { kind: Arc::clone(kind), compiled: OnceLock::new() });
    b.late_init(Arc::new(CompileTitle { title: Arc::clone(&title) }))?;
    declare_peers(b)?;
    let rule = Arc::new(ListRule { title });
    b.lowering_rule(
        LoweringRule::new(
            list_rule_name(name),
            list_element_name(name),
            list_decl(),
            Arc::clone(&rule) as Arc<dyn Lower>,
            DefaultMark::Everywhere,
        )
        .docs(format!(
            "the list of the `{name}` xnotes lowers to a `flm.pres.block` holding the title as a \
             `flm.pres.block-heading` and a labeled `flm.pres.list`, one entry per xnote, its \
             mark as the label"
        )),
    )?;
    Ok(rule)
}

/// What every rule of a list element that writes the list reads and
/// produces: the table of xnotes, the counter and anchors tables, all
/// optionally, and the presentation elements of [`ListRule`]'s shape. A
/// backend's own rule adds what it stages beyond that shape.
pub(super) fn list_decl() -> LoweringDecl {
    // The table of xnotes is read optionally: required wherever a stage
    // writes it, absent where a feature contributed the kind to a registry
    // without the `xnotes` feature.
    // See decision rationale at [§flm-dd-dr:std-xnote-list-rule].
    LoweringDecl::new()
        .reads_opt(XNOTES)
        .reads_opt(COUNTERS)
        .reads_opt(ANCHORS)
        .produces(pres::BLOCK)
        .produces(pres::BLOCK_HEADING)
        .produces(pres::LIST)
        .produces(pres::SPAN)
        .produces(pres::PAR)
}

/// Makes, for the feature that `b` registers, the peer declarations
/// ([`PeerName`](crate::core::registry::PeerName)) of the names that the
/// rules of the `xnotes` feature and the list rules of every xnote kind use:
/// the table of xnotes (the `xnotes` feature's, absent where a feature
/// contributes a kind to a registry without it), the counter and anchors
/// tables, and the presentation elements the rules produce. The feature
/// therefore builds in a registry without the feature that owns each name:
/// a rule that reads an absent table optionally finds it absent, and a rule
/// that produces an element nobody declares is set aside (excluded from rule
/// selection).
///
/// It is called once per xnote kind the feature contributes, and by the
/// `xnotes` feature for its own rules. The registry merges a repeated
/// identical peer declaration into the first, so each call declares every
/// name.
// See decision rationale at [§flm-dd-dr:std-xnote-pres-peers].
pub(super) fn declare_peers(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_table::<Xnotes>(XNOTES.name(), "the xnotes feature's table of xnotes, by kind")?;
    b.peer_table::<Counters>(
        COUNTERS.name(),
        "the numbering feature's counter table: the number of each xnote of a numbered kind",
    )?;
    b.peer_table::<refs::Anchors>(
        ANCHORS.name(),
        "the references feature's anchors table: the anchors of each xnote and of its mark",
    )?;
    b.peer_element::<SpanPayload>(pres::SPAN, "a mark, a list entry's label, an in-place xnote")?;
    b.peer_element::<pres::LinkPayload>(pres::LINK, "the link from a mark to its xnote")?;
    b.peer_element::<BlockPayload>(
        pres::BLOCK,
        "a list of xnotes; an in-place xnote holding paragraphs",
    )?;
    b.peer_element::<pres::ParPayload>(pres::PAR, "a paragraph of an xnote")?;
    b.peer_element::<HeadingPayload>(pres::BLOCK_HEADING, "the title of a list of xnotes")?;
    b.peer_element::<ListPayload>(pres::LIST, "the entries of a list of xnotes")
}

/// The title of the list of one xnote kind, compiled once when the registry
/// is built: FLM text, in the language of a counter style's number template,
/// with no placeholder available.
struct Title {
    kind: Arc<XnoteKind>,
    compiled: OnceLock<Option<Arc<NodeTree<Flm>>>>,
}

impl std::fmt::Debug for Title {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Title").field("kind", &self.kind.name).finish_non_exhaustive()
    }
}

/// The late-initialization hook (a build hook that runs once the language is
/// built) that compiles an xnote kind's title: a title that is not valid FLM,
/// or that writes a `#` placeholder, fails the build, as a kind's mark
/// does.
// See decision rationale at [§flm-dd-dr:std-xnote-list-title].
struct CompileTitle {
    title: Arc<Title>,
}

impl LateInit for CompileTitle {
    fn late_init(&self, lang: &BuiltLang, _diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        let kind = &self.title.kind;
        let compiled = match &kind.title {
            None => None,
            Some(text) => {
                let failed = |error: &dyn std::fmt::Display| {
                    LateInitError::new(format!("the title `{text}` of the xnote kind `{}`: {error}", kind.name))
                };
                let language = TemplateLanguage::new(lang).map_err(|errors| {
                    failed(&errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))
                })?;
                let what = format!("the title of the xnote kind `{}`", kind.name);
                let template = compile_text(text, &what, &PlaceholderVocabulary::new(0), lang, &language)
                    .map_err(|error| failed(&error))?;
                Some(instantiate_fixed(&template).map_err(|error| failed(&error))?)
            }
        };
        // One hook per registration of the kind, run once.
        let _ = self.title.compiled.set(compiled);
        Ok(())
    }
}

/// The general lowering rule of the list element of an xnote kind,
/// `flm.xnotes.<kind>.list.lower.default` ([`list_rule_name`]), registered
/// for every xnote kind by [`contribute_kind`](super::contribute_kind) and
/// marked default on every backend. The plain-text backend uses it; the
/// `html` backend uses [`ListHtmlRule`](super::html::ListHtmlRule), which
/// adds the links back to the marks, and the `latex-recomposed` backend
/// [`ListLatexRule`](super::latex::ListLatexRule), which writes nothing.
///
/// It stages
///
/// ```text
/// flm.pres.block { role: flm.xnotes.<kind>.list, kinds: [xnotes, <kind>], doc_role: Endnotes }
///   ├ flm.pres.block-heading { name: xnotes, level: <the kind's title level>, in_toc: false }
///   │   └ the kind's title
///   └ flm.pres.list { kinds: [numbered | unnumbered, xnotes, <kind>, labeled],
///                     entry_anchors: [<each xnote's anchor>] }
///       per entry:  label ⇒ flm.pres.span { style: Superscript, role: flm.xnotes.entry.mark }
///                             holding the xnote's mark text
///                   item  ⇒ the entry's content
/// ```
///
/// - The heading is staged for a kind with a [`title`](XnoteKind::title), at
///   its [`title_level`](XnoteKind::title_level), under the heading name
///   [`TITLE_HEADING`] and out of any table of contents; no heading is staged
///   for a kind with no title.
/// - The list carries the kind token `numbered` for a numbered kind,
///   `unnumbered` otherwise, and `labeled`: every entry carries its label, so
///   an entry whose mark is empty shows none.
/// - Each entry node of the list ([`ENTRY`]) holds a content pointer
///   ([`CONTENT_POINTER`](super::CONTENT_POINTER)) that refers to the node
///   carrying its xnote and to the region of that node holding the content.
///   The rule finds the entry's xnote among the kind's records in the table
///   of xnotes ([`Xnotes::of_kind`]) as the record of that node whose content
///   is that region ([`XnoteRecord::content`]). A node carrying xnotes of two
///   kinds is thus found in each kind's list, and each of several xnotes of
///   one kind on a node is found in its own entry. The xnote's point gives
///   the entry's label, which is the mark text ([`stage_mark_text`]: the
///   number in the mark style, or the fixed mark), and the entry's anchor,
///   from the references feature's anchors table. An entry whose pointer
///   refers to no recorded xnote of the kind, or that holds no pointer, has
///   neither.
/// - An entry's content is the content its pointer refers to, lowered here,
///   where the entry is placed, as content of the tree that holds the xnote,
///   through the ordinary rules: inline content as one paragraph, paragraphs
///   and blocks as they are ([`append_to_last_paragraph`]). It is lowered
///   once, here: the rule lowers the pointer itself, never through the
///   pointer's own rule, and the xnote's rule writes the mark alone. A pointer
///   that refers to no xnote's content is refused as the pointer's rule
///   refuses it ([`PointerRule`](super::PointerRule)).
/// - A child of the list that is not an entry, which no builder of this
///   feature writes, is not written.
///
/// The rule reads the table of xnotes, and the counter and anchors tables
/// where the numbering and references features are registered; without the
/// table of xnotes, no entry has a label or an anchor.
#[derive(Debug)]
pub struct ListRule {
    title: Arc<Title>,
}

/// What is appended at the end of the last paragraph of each entry: the
/// function that stages the link back from the entry to its mark, on the
/// outputs that write one, or nothing.
pub(super) type Appended<'f> =
    &'f mut dyn FnMut(&mut LowerCx<'_>, &mut dyn Lowerer, Option<&XnoteRecord>) -> Result<Vec<BuildId>, LowerError>;

impl ListRule {
    /// Lowers the list `node` of the xnote kind the rule was made for, as the
    /// rule's documentation says, with `appended(record)` at the end of the
    /// last paragraph of the entry of the xnote `record`. The general rule and
    /// the HTML rule of the list element call it, so that what they write
    /// differs in that alone.
    ///
    /// It first requires the feature's style identifier
    /// [`STYLE`](super::STYLE) (`flm.xnotes.style`), which each backend
    /// resolves as it resolves requirements.
    pub(super) fn lower_list(
        &self,
        node: &NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
        appended: Appended<'_>,
    ) -> Result<Lowering, LowerError> {
        lw.require(&Name::from(STYLE));
        let value = element_value::<XnotesListElement>(node)
            .ok_or_else(|| LowerError::structure("a list of xnotes without its element's data"))?;
        let kind = Arc::clone(&value.kind);
        let entries: Vec<NodeRef<'_, Flm>> =
            node.children().iter().filter(|child| child.ext().element.name.as_str() == ENTRY).collect();
        // Each entry's pointer, and the record of the xnote it names: by
        // address and content region, among the kind's records.
        // See decision rationale at [§flm-dd-dr:std-xnote-several].
        let pointers: Vec<Option<NodeRef<'_, Flm>>> = entries
            .iter()
            .map(|entry| {
                let is_pointer = |child: &NodeRef<'_, Flm>| {
                    child.ext().element.name.as_str() == CONTENT_POINTER
                };
                entry.children().iter().find(is_pointer)
            })
            .collect();
        let records: Vec<Option<XnoteRecord>> = {
            let of_kind = lw.get(XNOTES)?.map_or(&[][..], |xnotes| xnotes.of_kind(kind.name.as_str()));
            pointers
                .iter()
                .map(|pointer| {
                    let named = ContentPointer::of(pointer.as_ref()?)?;
                    of_kind
                        .iter()
                        .find(|record| record.point.addr == named.target && record.content == Some(named.region))
                        .cloned()
                })
                .collect()
        };
        let entry_anchors: Vec<Option<String>> = {
            let anchors = lw.get(ANCHORS)?;
            records
                .iter()
                .map(|record| anchors?.lookup(record.as_ref()?.point).flatten().map(String::from))
                .collect()
        };
        let mut slots: Vec<(String, Vec<BuildId>)> = Vec::with_capacity(2 * entries.len());
        for (pointer, record) in pointers.iter().zip(&records) {
            let record = record.as_ref();
            let text = match record {
                Some(record) => stage_mark_text(cx, lw, &kind, record.point)?,
                None => Vec::new(),
            };
            let label = SpanPayload {
                style: Some(SpanStyle::Superscript),
                role: Some(Name::from(ENTRY_MARK_ROLE)),
                ..SpanPayload::default()
            };
            let label = lw.pres_with(cx, &ElementName::from(pres::SPAN), label, text)?;
            let mut tail = Some(appended(cx, lw, record)?);
            let item = match pointer {
                Some(pointer) => {
                    let mut last = |cx: &mut LowerCx<'_>,
                                    lw: &mut dyn Lowerer,
                                    _: &NodeRef<'_, Flm>,
                                    content: &[NodeRef<'_, Flm>]| {
                        append_to_last_paragraph(cx, lw, content, tail.take().unwrap_or_default())
                    };
                    lower_pointed(cx, lw, pointer, &mut last)?
                }
                None => Vec::new(),
            };
            // A pointer refused, or no pointer: the tail stands alone.
            let item = match tail.take() {
                Some(tail) if !tail.is_empty() => {
                    [item, append_to_last_paragraph(cx, lw, &[], tail)?].concat()
                }
                _ => item,
            };
            slots.push((LABEL_SLOT.to_string(), vec![label]));
            slots.push((ITEM_SLOT.to_string(), item));
        }
        let structure = if kind.numbered { LIST_NUMBERED } else { LIST_UNNUMBERED };
        let payload = ListPayload {
            kinds: vec![
                Name::from(structure),
                Name::from(KIND_XNOTES),
                kind.name.clone(),
                Name::from(LIST_LABELED),
            ],
            anchor: None,
            entry_anchors,
        };
        let list = lw.pres_slots_with(cx, &ElementName::from(pres::LIST), payload, slots)?;
        let mut children = Vec::with_capacity(2);
        if let Some(title) = self.title.compiled.get().cloned().flatten() {
            let body = lw.splice(cx, &title)?;
            let heading = HeadingPayload {
                name: Name::from(TITLE_HEADING),
                level: kind.title_level,
                in_toc: false,
                anchor: None,
                number: None,
            };
            children.push(lw.pres_with(cx, &ElementName::from(pres::BLOCK_HEADING), heading, body)?);
        }
        children.push(list);
        let block = BlockPayload {
            role: Some(Name::from(list_element_name(kind.name.as_str()).as_str())),
            kinds: vec![Name::from(KIND_XNOTES), kind.name.clone()],
            anchor: None,
            doc_role: Some(DocRole::Endnotes),
        };
        Ok(Lowering::Emit(vec![lw.pres_with(cx, &ElementName::from(pres::BLOCK), block, children)?]))
    }
}

impl Lower for ListRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        self.lower_list(&node, cx, lw, &mut |_, _, _| Ok(Vec::new()))
    }
}
