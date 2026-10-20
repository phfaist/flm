//! The list of cited works: [`build_list`], the only function that builds
//! one, the two synthesized elements a list is made of with their general
//! lowering rules, and the bibliography, the piece that places the list of
//! every cited work after the document's content.
//!
//! A piece is a document-level synthesized value that a backend places
//! before or after the document's content ([`Piece`]). A **list entry** is an
//! entry of a list of cited works: one node per work, labeled with the work's
//! number and holding its citation entry. The bibliography's
//! builder stage ([`BUILD_BIBLIOGRAPHY`]) writes the piece table
//! [`BIBLIOGRAPHY`]; the feature contributes the piece's declaration to the
//! `pieces` feature's service slot ([`pieces::SLOT`]), and each backend that
//! places pieces lowers the list with the rule of its element:
//! [`BibliographyRule`] by default, and on the recomposed LaTeX a rule that
//! writes nothing ([`ListNotWritten`](super::latex::ListNotWritten)).

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef, NodeTree};
use techy::serialize::SerialValue;

use crate::core::data::store::StageOut;
use crate::core::data::table::TableHandle;
use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::piece::{Piece, PieceDecl, PiecePlace};
use crate::core::plan::PlanCx;
use crate::core::registry::{BuildError, FeatureBuilder, FlmRegistry};
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::copy::{copy_pieces_into_value, SynthesizedPiece, ValueContent, ValuePiece};
use crate::lang::mint::{ElementDef, FlmSynthesizedSpec, SyntaxDef};
use crate::lang::{Flm, PresPayload};
use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::ElementSpec;
use crate::model::names::{ElementName, FeatureName, Name};
use crate::stdcore::backends::lowering::lower_blocks_with_edges;
use crate::stdcore::backends::pieces;
use crate::stdcore::backends::pres::{
    self, BlockPayload, DocRole, DroppedPayload, HeadingPayload, ListPayload, SpanPayload, ITEM_SLOT,
    LABEL_SLOT, LIST_LABELED, LIST_NUMBERED,
};
use crate::stdcore::features::numbering::{format_many, CounterValue, FormatOptions, NumberedValue};

use super::config::{CitationPlacement, MarkForm};
use super::entries::{CitationEntries, ENTRIES};
use super::mark::MarkSettings;
use super::numbers::{stage_failed, CitedWorks, CITATIONS};
use super::providers::CitationEntry;

/// The name `flm.citations.bibliography`, used for four things: the
/// synthesized element of a list of cited works, the role of the block that
/// list lowers to, the piece that places the list of every cited work after
/// the content, and that piece's table ([`bibliography_table`]).
pub const BIBLIOGRAPHY: &str = "flm.citations.bibliography";

/// The name of the synthesized element of one entry of a list of cited works,
/// `flm.citations.bibliography.entry`: a node of it holds the citation
/// entry's content and carries a [`ListedEntry`] as its payload.
pub const BIBLIOGRAPHY_ENTRY: &str = "flm.citations.bibliography.entry";

/// The name of the bibliography's builder stage,
/// `flm.citations.build-bibliography`, which writes the piece table
/// [`BIBLIOGRAPHY`].
pub const BUILD_BIBLIOGRAPHY: &str = "flm.citations.build-bibliography";

/// The role of the span that holds an entry's label in a list of cited works
/// (its number written through the citation counter style),
/// `flm.citations.entry.number`.
pub const ENTRY_NUMBER_ROLE: &str = "flm.citations.entry.number";

/// The kind token that a list of cited works carries on its block and on its
/// list, `citations`; also the name given to the list's heading
/// ([`HeadingPayload::name`]).
pub const KIND_CITATIONS: &str = "citations";

/// The name of the general lowering rule of the list element,
/// `flm.citations.bibliography.lower.default` ([`BibliographyRule`]), the
/// default on every backend.
pub const BIBLIOGRAPHY_RULE: &str = "flm.citations.bibliography.lower.default";

/// The name of the general lowering rule of the entry element,
/// `flm.citations.bibliography.entry.lower.default`. A list entry is lowered
/// by its list's rule; this rule fails with a structure error when a list
/// entry is lowered on its own.
pub const BIBLIOGRAPHY_ENTRY_RULE: &str = "flm.citations.bibliography.entry.lower.default";

/// Returns the handle of the bibliography's piece table, [`BIBLIOGRAPHY`]: the
/// builder stage writes it, and a backend that places pieces reads it.
pub fn bibliography_table() -> TableHandle<Piece> {
    TableHandle::new(BIBLIOGRAPHY)
}

/// One cited work of a list, as [`build_list`] takes it: its citation key,
/// number and identifier, and its citation entry, or `None` for a work with
/// no citation entry.
#[derive(Debug, Clone, Copy)]
pub struct ListedWork<'w> {
    /// The citation key.
    pub key: &'w str,
    /// The work's number, which the label of its entry in the list shows.
    pub number: usize,
    /// The work's identifier, which its entry in the list carries.
    pub identifier: &'w str,
    /// The work's citation entry; `None` for a citation key with no citation
    /// entry.
    pub entry: Option<&'w CitationEntry>,
}

/// The payload of a node of the entry element ([`BIBLIOGRAPHY_ENTRY`]): the
/// work's citation key, number and identifier, and whether its citation entry
/// is missing (the node then holds nothing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedEntry {
    /// The citation key.
    pub key: String,
    /// The number that the label of the list entry shows.
    pub number: usize,
    /// The identifier that the list entry carries.
    pub identifier: String,
    /// Whether the citation key has no citation entry.
    pub missing: bool,
}

impl PresPayload for ListedEntry {
    fn project(&self) -> SerialValue {
        let mut entries = vec![
            ("key".to_string(), SerialValue::Str(self.key.clone())),
            ("number".to_string(), SerialValue::Int(self.number as i64)),
            ("id".to_string(), SerialValue::Str(self.identifier.clone())),
        ];
        if self.missing {
            entries.push(("missing".to_string(), SerialValue::Bool(true)));
        }
        SerialValue::Map(entries)
    }
}

impl ListedEntry {
    /// Returns the payload of the entry node `node`, or `None` for a node that
    /// carries none.
    pub fn of<'n>(node: &'n NodeRef<'_, Flm>) -> Option<&'n ListedEntry> {
        node.ext().payload.as_deref()?.downcast_ref::<ListedEntry>()
    }
}

/// Builds the list of `works`, in the order given, and returns it as a
/// synthesized value's tree. The tree holds one node of the list element
/// [`BIBLIOGRAPHY`], whose children are one node of the entry element
/// [`BIBLIOGRAPHY_ENTRY`] per work. Each entry node carries its
/// [`ListedEntry`] and holds a copy of its citation entry's content as the
/// synthesized value's own content ([`ValueContent::Own`]), or nothing for a
/// work with no citation entry. The new nodes take `anchor`'s span and
/// parsing state.
///
/// This is the only function that builds a list of cited works. The
/// bibliography's builder stage calls it over every cited work of the
/// document; a caller that lists the works of one part of a document calls it
/// over that part's works, numbered with
/// [`number_works`](super::number_works). Wherever the list is placed, the
/// rule of its element lowers it and writes each number through the citation
/// counter style ([`BibliographyRule`]).
///
/// Call it on a worker thread (a thread with a large enough stack; see
/// [`on_worker_thread`](crate::lang::guard::on_worker_thread)), as
/// [`copy_pieces_into_value`](crate::lang::copy::copy_pieces_into_value)
/// requires.
///
/// # Errors
///
/// A text that says what went wrong: an element the registry does not hold
/// (the `citations` feature is not registered), content nesting too deeply,
/// or the copy failing.
pub fn build_list(
    works: &[ListedWork<'_>],
    registry: &FlmRegistry,
    anchor: &NodeRef<'_, Flm>,
) -> Result<Arc<NodeTree<Flm>>, String> {
    let spec = |name: &str| match registry.element_value(name) {
        Ok(Some(element)) => Ok(Arc::new(FlmSynthesizedSpec::new(Arc::clone(element)))),
        Ok(None) => Err(format!("no element `{name}` is registered")),
        Err(error) => Err(error.to_string()),
    };
    let (list_spec, entry_spec) = (spec(BIBLIOGRAPHY)?, spec(BIBLIOGRAPHY_ENTRY)?);
    let contents: Vec<Vec<NodeRef<'_, Flm>>> = works
        .iter()
        .map(|work| work.entry.map_or_else(Vec::new, |entry| entry.content.root().children().iter().collect()))
        .collect();
    let entries: Vec<SynthesizedPiece<'_, '_>> = works
        .iter()
        .zip(&contents)
        .map(|(work, content)| SynthesizedPiece {
            spec: Arc::clone(&entry_spec),
            slot: pres::BODY_SLOT,
            span: anchor.span().clone(),
            state: Arc::clone(anchor.parsing_state()),
            body: content.iter().copied().map(ValuePiece::Node).collect(),
            payload: Some(Arc::new(ListedEntry {
                key: work.key.to_string(),
                number: work.number,
                identifier: work.identifier.to_string(),
                missing: work.entry.is_none(),
            })),
        })
        .collect();
    let list = SynthesizedPiece {
        spec: list_spec,
        slot: pres::BODY_SLOT,
        span: anchor.span().clone(),
        state: Arc::clone(anchor.parsing_state()),
        body: entries.iter().map(ValuePiece::Synthesized).collect(),
        payload: None,
    };
    copy_pieces_into_value(&[ValuePiece::Synthesized(&list)], anchor, ValueContent::Own, registry.lang().guard())
        .map_err(|error| error.to_string())
}

/// The bibliography's builder stage, [`BUILD_BIBLIOGRAPHY`]: a compute stage
/// of document scope. It reads the cited works ([`CITATIONS`]) and their
/// citation entries ([`ENTRIES`]) and writes the piece table
/// [`BIBLIOGRAPHY`]: the list of every cited work in the order of the numbers
/// ([`build_list`]), whose nodes take the span and parsing state of the root
/// of the run's first tree. Where the document cites nothing, or under the
/// in-place placement, it writes a piece with no content, which no backend
/// places.
#[derive(Debug)]
pub(super) struct BuildBibliography {
    pub(super) placement: CitationPlacement,
}

impl BuildBibliography {
    /// Returns the list's synthesized value, or `None` for nothing to place;
    /// a failure is reported into `out`.
    fn build(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) -> Option<Arc<NodeTree<Flm>>> {
        if self.placement == CitationPlacement::InPlace {
            return None;
        }
        let (citations, entries): (&CitedWorks, Option<&CitationEntries>) =
            match (cx.get(CITATIONS), cx.get(ENTRIES)) {
                (Ok(Some(citations)), Ok(entries)) => (citations, entries),
                (Err(error), _) | (_, Err(error)) => {
                    stage_failed(out, BUILD_BIBLIOGRAPHY, error);
                    return None;
                }
                (Ok(None), _) => return None,
            };
        if citations.is_empty() {
            return None;
        }
        let works: Vec<ListedWork<'_>> = citations
            .works()
            .map(|(key, work)| ListedWork {
                key,
                number: work.number,
                identifier: &work.identifier,
                entry: entries.and_then(|entries| entries.entry(key)),
            })
            .collect();
        // See decision rationale at [§flm-dd-dr:std-citations-bibliography].
        let (_, tree) = cx.trees().next()?;
        match build_list(&works, cx.registry(), &tree.root()) {
            Ok(value) => Some(value),
            Err(detail) => {
                stage_failed(out, BUILD_BIBLIOGRAPHY, format!("the list of cited works could not be assembled: {detail}"));
                None
            }
        }
    }
}

impl Stage for BuildBibliography {
    fn decl(&self) -> StageDecl {
        StageDecl::new(BUILD_BIBLIOGRAPHY, Effect::Compute, Scope::Document)
            .writes(bibliography_table())
            .docs(
                "Builds the list of the cited works, in the order of their numbers, each with its \
                 citation entry, into the piece `flm.citations.bibliography`.",
            )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(CITATIONS).reads(ENTRIES)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let content = self.build(cx, out);
        match out.table(&bibliography_table()) {
            Ok(table) => *table = Piece { content },
            Err(error) => stage_failed(out, BUILD_BIBLIOGRAPHY, error),
        }
    }
}

/// The body of the general rule of the list element [`BIBLIOGRAPHY`],
/// [`BIBLIOGRAPHY_RULE`], the default on every backend except the recomposed
/// LaTeX. It stages (builds into the lowered tree; see
/// [`Lowerer`])
///
/// ```text
/// flm.pres.block { role: flm.citations.bibliography, kinds: [citations],
///                  doc_role: Bibliography }
///   ├ flm.pres.block-heading { name: citations, level: <heading_level>, in_toc: false }
///   │   └ the configured title (`References`), at the configured level (2)
///   └ flm.pres.list { kinds: [numbered, citations, labeled],
///                     entry_anchors: [<each work's identifier>] }
///       per list entry:  label ⇒ flm.pres.span { role: flm.citations.entry.number }
///                                  holding the number written through the citation counter style
///                        item  ⇒ the citation entry's content, as one paragraph
/// ```
///
/// The heading is staged where the configuration gives the list a title
/// ([`heading_title`](super::CitationsConfig::heading_title)); where the
/// configuration sets no title, the block holds the list alone.
///
/// The block carries the document role [`DocRole::Bibliography`] (written
/// `role="doc-bibliography"` on the HTML page); the list entries carry none,
/// because the role `doc-biblioentry` is deprecated in the Digital Publishing
/// ARIA module.
///
/// A list entry's label is the work's number written through the citation
/// counter style: with the citation counter style's delimiters under the
/// bracketed form (`[1]`), and bare under the superscript form (`1`). The item of a work with no citation
/// entry is a dropped mark (`flm.pres.dropped`, condition
/// `flm.citations.no-entry`; see
/// [recovery](crate::stdcore::backends::lowering::error)) in a paragraph of
/// its own; the check stage has reported the citation key. A child of the
/// list that is not a list entry is not written.
#[derive(Debug)]
pub struct BibliographyRule {
    settings: Arc<MarkSettings>,
}

impl BibliographyRule {
    /// Stages the label of the work numbered `number`.
    fn label(&self, cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer, number: usize) -> Result<BuildId, LowerError> {
        let style = self.settings.style()?;
        let value = CounterValue::new(number as i64);
        let numbered = [NumberedValue::new(&value)];
        let options = match self.settings.config().mark {
            MarkForm::Bracketed => FormatOptions::new().without_word(),
            MarkForm::Superscript => FormatOptions::new().without_word().without_delimiters(),
        };
        let segments = format_many(style, &numbered, &options)
            .map_err(|error| LowerError::structure(error.to_string()))?;
        let mut body = Vec::new();
        for segment in &segments {
            body.extend(lw.splice(cx, segment.content())?);
        }
        let payload = SpanPayload { role: Some(Name::from(ENTRY_NUMBER_ROLE)), ..SpanPayload::default() };
        lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)
    }
}

impl Lower for BibliographyRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let entries: Vec<NodeRef<'_, Flm>> = node
            .children()
            .iter()
            .filter(|child| child.ext().element.name.as_str() == BIBLIOGRAPHY_ENTRY)
            .collect();
        let mut slots: Vec<(String, Vec<BuildId>)> = Vec::with_capacity(2 * entries.len());
        let mut entry_anchors = Vec::with_capacity(entries.len());
        for entry in &entries {
            let Some(listed) = ListedEntry::of(entry) else { continue };
            let label = self.label(cx, lw, listed.number)?;
            let item = if listed.missing {
                let dropped = DroppedPayload {
                    condition: Name::from("flm.citations.no-entry"),
                    detail: "bibliography entry".to_string(),
                };
                let mark = lw.pres_with(cx, &ElementName::from(pres::DROPPED), dropped, Vec::new())?;
                lower_blocks_with_edges(cx, lw, &[], Vec::new(), vec![mark])?
            } else {
                let content: Vec<NodeRef<'_, Flm>> = entry.children().iter().collect();
                lower_blocks_with_edges(cx, lw, &content, Vec::new(), Vec::new())?
            };
            slots.push((LABEL_SLOT.to_string(), vec![label]));
            slots.push((ITEM_SLOT.to_string(), item));
            entry_anchors.push(Some(listed.identifier.clone()));
        }
        let list = ListPayload {
            kinds: vec![Name::from(LIST_NUMBERED), Name::from(KIND_CITATIONS), Name::from(LIST_LABELED)],
            anchor: None,
            entry_anchors,
        };
        let list = lw.pres_slots_with(cx, &ElementName::from(pres::LIST), list, slots)?;
        let mut body = Vec::with_capacity(2);
        if let Some(title) = self.settings.heading_title()? {
            let title = lw.splice(cx, title)?;
            let level = self.settings.config().heading_level;
            let heading =
                HeadingPayload { name: Name::from(KIND_CITATIONS), level, in_toc: false, anchor: None, number: None };
            body.push(lw.pres_with(cx, &ElementName::from(pres::BLOCK_HEADING), heading, title)?);
        }
        body.push(list);
        let block = BlockPayload {
            role: Some(Name::from(BIBLIOGRAPHY)),
            kinds: vec![Name::from(KIND_CITATIONS)],
            anchor: None,
            doc_role: Some(DocRole::Bibliography),
        };
        Ok(Lowering::Emit(vec![lw.pres_with(cx, &ElementName::from(pres::BLOCK), block, body)?]))
    }
}

/// The body of the general rule of the entry element, which runs only when a
/// list entry is lowered outside its list (no builder of this feature produces
/// such a tree): it fails with a structure error, because the list's rule
/// lowers the content of its entries itself.
#[derive(Debug, Default)]
struct EntryOutsideList;

impl Lower for EntryOutsideList {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut LowerCx<'_>,
        _lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Err(LowerError::structure("an entry of a list of cited works must be inside its list"))
    }
}

/// Declares the list of cited works and the bibliography for the feature that
/// `b` registers: the two synthesized elements and their general rules, the
/// piece table, its builder stage, and the piece's declaration, contributed
/// as a peer contribution to the `pieces` feature's service slot
/// ([`pieces::SLOT`]). Without the `pieces` feature, the list is built and no
/// backend places it.
pub(super) fn declare(
    b: &mut FeatureBuilder<'_>,
    owner: &FeatureName,
    settings: &Arc<MarkSettings>,
) -> Result<(), BuildError> {
    let block = || Arc::new(FixedBlockRole(BlockRole::Block)) as Arc<dyn BlockRoleAspect>;
    b.element(
        ElementDef::new(
            owner.clone(),
            Vec::<&str>::new(),
            SyntaxDef::Synthesized,
            Arc::new(ElementSpec::new(BIBLIOGRAPHY).docs("A list of cited works.").with(block())),
        )
        .docs("A list of cited works. An author never writes it: a stage builds it."),
    )?;
    b.element(
        ElementDef::new(
            owner.clone(),
            Vec::<&str>::new(),
            SyntaxDef::Synthesized,
            Arc::new(ElementSpec::new(BIBLIOGRAPHY_ENTRY).docs("One entry of a list of cited works.").with(block())),
        )
        .docs("One entry of a list of cited works. An author never writes it: a stage builds it."),
    )?;
    b.lowering_rule(
        LoweringRule::new(
            BIBLIOGRAPHY_RULE,
            BIBLIOGRAPHY,
            LoweringDecl::new()
                .produces(pres::BLOCK)
                .produces(pres::BLOCK_HEADING)
                .produces(pres::LIST)
                .produces(pres::SPAN)
                .produces(pres::PAR)
                .produces(pres::DROPPED),
            Arc::new(BibliographyRule { settings: Arc::clone(settings) }),
            DefaultMark::Everywhere,
        )
        .docs(
            "a list of cited works lowers to a `flm.pres.block` holding the title as a \
             `flm.pres.block-heading` and a labeled `flm.pres.list` with one entry per work, its \
             number as the label",
        ),
    )?;
    b.lowering_rule(
        LoweringRule::new(
            BIBLIOGRAPHY_ENTRY_RULE,
            BIBLIOGRAPHY_ENTRY,
            LoweringDecl::new(),
            Arc::new(EntryOutsideList),
            DefaultMark::Everywhere,
        )
        .docs("an entry of a list of cited works is lowered by its list's rule; lowered on its own, it fails"),
    )?;
    b.table_named::<Piece>(
        BIBLIOGRAPHY,
        "The list of the cited works of the document, as a synthesized value, or nothing to place.",
    )?;
    b.stage(Arc::new(BuildBibliography { placement: settings.config().placement }))?;
    // The pieces feature owns the slot, and may be absent: the list is then
    // placed by no backend.
    b.peer_slot(
        pieces::SLOT,
        Arc::new(PieceDecl::new(
            BIBLIOGRAPHY,
            PiecePlace::AfterContent,
            "the list of the cited works, placed after the document's content",
        )),
    )?;
    Ok(())
}
