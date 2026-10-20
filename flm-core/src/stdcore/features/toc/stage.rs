//! The stage [`BuildToc`] (`flm.toc.build`), which writes the table of entries
//! and the piece of the [`toc`](super) feature, and the diagnostic
//! [`TocFailed`] it reports when the piece cannot be built.

use std::sync::Arc;

use techy::core::node::{NodeRef, NodeTree};
use techy::error::{Diagnostic, DiagnosticInfo};

use crate::core::data::store::StageOut;
use crate::core::data::table::TableHandle;
use crate::core::piece::Piece;
use crate::core::plan::PlanCx;
use crate::core::registry::FlmRegistry;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::copy::{copy_pieces_into_value, SynthesizedPiece, ValueContent, ValuePiece};
use crate::lang::guard::GuardPolicy;
use crate::lang::mint::FlmSynthesizedSpec;
use crate::lang::{nowhere, Flm, FlmOrigin};
use crate::model::aspects::{PointId, Sectioning};
use crate::stdcore::backends::pres;
use crate::stdcore::features::index::{walk_nodes_with, ELEMENT_INDEX};
use crate::stdcore::features::refpoints::{self, REFPOINTS};

use super::{
    TocEntries, TocEntry, TocEntryPayload, TocOptions, BUILD, ENTRIES, ENTRY, LIST, PIECE,
};

/// The stage [`BUILD`] (`flm.toc.build`): a compute stage that runs once per
/// document. It reads the option [`TocOptions`], the element index
/// (`flm.element-index`) and, where the `refpoints` feature is registered, the
/// table of reference points (`flm.refpoints`); it writes the table
/// [`ENTRIES`] and the piece table [`PIECE`].
///
/// It walks every heading of the run in document order (every node whose
/// element carries [`Sectioning`]), keeps those the aspect lists whose level
/// is at most [`TocOptions::depth`], and records for each its level, its first
/// reference point where the table of reference points holds it, and what the
/// aspect says its entry shows ([`Sectioning::toc_title`]). It writes the
/// entries, then builds the piece: one synthesized value of nested
/// [`PIECE`], [`LIST`] and [`ENTRY`] nodes, the entries nested by relative
/// level (see the [module documentation](super#the-shape-of-the-piece)),
/// each entry holding a copy of its title as content shown a second time
/// ([`ValueContent::Shown`]: a footnote becomes its mark).
///
/// A document with no listed heading writes an empty table and a piece with
/// no content, which no backend places. A piece that cannot be built is
/// reported as [`TocFailed`], and the piece holds no content; the table is
/// written all the same.
///
/// The read of the table of reference points is a required read where it is
/// made, so that the table exists in every plan that schedules this stage; it
/// is made only where the `refpoints` feature is registered, because the
/// construction of a plan fails for a stage that reads a table no feature
/// declares. Without it, no entry has a point.
#[derive(Debug, Default, Clone, Copy)]
pub struct BuildToc;

impl Stage for BuildToc {
    fn decl(&self) -> StageDecl {
        StageDecl::new(BUILD, Effect::Compute, Scope::Document)
            .writes(ENTRIES)
            .writes(TableHandle::<Piece>::new(PIECE))
            .docs(
                "Lists the document's headings that the table of contents shows, in document \
                 order, into `flm.toc.entries`, and builds the table of contents as nested lists \
                 into the piece `flm.toc`.",
            )
    }

    fn needs(&self, cx: &PlanCx<'_>) -> Needs {
        let needs = Needs::new().option::<TocOptions>().reads(ELEMENT_INDEX);
        if cx.has_feature(refpoints::FEATURE) { needs.reads(REFPOINTS) } else { needs }
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let depth = match cx.option::<TocOptions>() {
            Ok(options) => options.depth,
            Err(error) => return failed(out, error.to_string()),
        };
        let index = match cx.get(ELEMENT_INDEX) {
            Ok(index) => index,
            Err(error) => return failed(out, error.to_string()),
        };
        let points = if cx.registry().has_feature(refpoints::FEATURE) {
            match cx.get(REFPOINTS) {
                Ok(points) => points,
                Err(error) => return failed(out, error.to_string()),
            }
        } else {
            None
        };
        let mut kept: Vec<(NodeRef<'_, Flm>, TocEntry)> = Vec::new();
        if let Some(index) = index {
            for (addr, node, aspect) in walk_nodes_with::<dyn Sectioning>(cx, index) {
                let level = aspect.level(&node);
                if !aspect.in_toc(&node) || level > depth {
                    continue;
                }
                let first = PointId { addr, index: 0 };
                let point = points.and_then(|points| points.point(first)).map(|_| first);
                let title = aspect.toc_title(&node);
                kept.push((node, TocEntry { addr, level, point, title }));
            }
        }
        match out.table(ENTRIES) {
            Ok(table) => *table = TocEntries { entries: kept.iter().map(|(_, entry)| entry.clone()).collect() },
            Err(error) => failed(out, error.to_string()),
        }
        let content = build_piece(cx.registry(), &kept).unwrap_or_else(|detail| {
            failed(out, detail);
            None
        });
        match out.table(&TableHandle::<Piece>::new(PIECE)) {
            Ok(table) => *table = Piece { content },
            Err(error) => failed(out, error.to_string()),
        }
    }
}

/// The synthesized specifications of the three elements of the piece, which
/// this feature registers.
struct Specs {
    root: Arc<FlmSynthesizedSpec>,
    list: Arc<FlmSynthesizedSpec>,
    entry: Arc<FlmSynthesizedSpec>,
}

/// Returns the synthesized specification of the element `name`.
fn spec(registry: &FlmRegistry, name: &str) -> Result<Arc<FlmSynthesizedSpec>, String> {
    match registry.element_value(name) {
        Ok(Some(element)) => Ok(Arc::new(FlmSynthesizedSpec::new(Arc::clone(element)))),
        Ok(None) => Err(format!("no element `{name}` is registered")),
        Err(error) => Err(error.to_string()),
    }
}

/// Returns, for each entry in order, given by its level in `levels`, the
/// entry it is nested in (`None` for a top-level entry) and its depth (0 for
/// a top-level entry),
/// by the stack rule of the [module documentation](super#the-shape-of-the-piece):
/// the open entries whose level is not smaller than the new entry's are
/// closed, and the new entry goes into the list of the one left open, or at
/// the top.
fn nest(levels: &[u32]) -> (Vec<Option<usize>>, Vec<usize>) {
    let mut parents = Vec::with_capacity(levels.len());
    let mut depths = Vec::with_capacity(levels.len());
    let mut open: Vec<usize> = Vec::new();
    for (index, level) in levels.iter().enumerate() {
        while open.last().is_some_and(|&top| levels[top] >= *level) {
            open.pop();
        }
        parents.push(open.last().copied());
        depths.push(open.len());
        open.push(index);
    }
    (parents, depths)
}

/// What every depth of the piece's construction reads.
struct Build<'k, 't> {
    specs: Specs,
    kept: &'k [(NodeRef<'t, Flm>, TocEntry)],
    titles: &'k [Vec<NodeRef<'t, Flm>>],
    parents: Vec<Option<usize>>,
    depths: Vec<usize>,
    guard: GuardPolicy,
}

/// Returns the piece's content for the entries `kept`, or `None` where there
/// is no entry; an error text where the value cannot be built.
fn build_piece(
    registry: &FlmRegistry,
    kept: &[(NodeRef<'_, Flm>, TocEntry)],
) -> Result<Option<Arc<NodeTree<Flm>>>, String> {
    if kept.is_empty() {
        return Ok(None);
    }
    let specs = Specs { root: spec(registry, PIECE)?, list: spec(registry, LIST)?, entry: spec(registry, ENTRY)? };
    // The nodes each entry shows: those the content reference designates on
    // its heading, the children of the root of a tree the aspect built.
    let titles: Vec<Vec<NodeRef<'_, Flm>>> = kept
        .iter()
        .map(|(node, entry)| entry.title.as_ref().and_then(|title| title.nodes(node)).unwrap_or_default())
        .collect();
    let levels: Vec<u32> = kept.iter().map(|(_, entry)| entry.level).collect();
    let (parents, depths) = nest(&levels);
    let deepest = depths.iter().copied().max().unwrap_or(0);
    let build = Build { specs, kept, titles: &titles, parents, depths, guard: registry.lang().guard() };
    at_depth(&build, deepest, &[]).map(Some)
}

/// Builds the entries at depth `depth` and the lists that hold them, each
/// entry holding its own list from `below` (the lists of depth `depth + 1`,
/// each with the entry it belongs in); then the depth above, until the root,
/// which is copied into the value. Each depth's pieces live in this call's
/// frame while the calls for the depths above it borrow them.
fn at_depth<'t>(
    build: &Build<'_, 't>,
    depth: usize,
    below: &[(usize, SynthesizedPiece<'_, 't>)],
) -> Result<Arc<NodeTree<Flm>>, String> {
    let entries: Vec<(usize, SynthesizedPiece<'_, 't>)> = (0..build.kept.len())
        .filter(|&index| build.depths[index] == depth)
        .map(|index| {
            let (node, entry) = &build.kept[index];
            let mut body: Vec<ValuePiece<'_, 't>> = build.titles[index].iter().copied().map(ValuePiece::Node).collect();
            if let Some((_, list)) = below.iter().find(|(parent, _)| *parent == index) {
                body.push(ValuePiece::Synthesized(list));
            }
            let piece = SynthesizedPiece {
                spec: Arc::clone(&build.specs.entry),
                slot: pres::BODY_SLOT,
                span: node.span().clone(),
                state: Arc::clone(node.parsing_state()),
                body,
                payload: Some(Arc::new(TocEntryPayload { point: entry.point, level: entry.level })),
            };
            (index, piece)
        })
        .collect();
    // The entries of one list are consecutive at their depth: a list per run
    // of entries nested in the same entry.
    let mut runs: Vec<(Option<usize>, Vec<usize>)> = Vec::new();
    for (at, (index, _)) in entries.iter().enumerate() {
        let parent = build.parents[*index];
        match runs.last_mut() {
            Some((last, members)) if *last == parent => members.push(at),
            _ => runs.push((parent, vec![at])),
        }
    }
    let lists: Vec<(Option<usize>, SynthesizedPiece<'_, 't>)> = runs
        .iter()
        .filter_map(|(parent, members)| {
            let first = &entries[*members.first()?].1;
            let list = SynthesizedPiece {
                spec: Arc::clone(&build.specs.list),
                slot: pres::BODY_SLOT,
                span: first.span.clone(),
                state: Arc::clone(&first.state),
                body: members.iter().map(|&at| ValuePiece::Synthesized(&entries[at].1)).collect(),
                payload: None,
            };
            Some((*parent, list))
        })
        .collect();
    if depth > 0 {
        let lists: Vec<(usize, SynthesizedPiece<'_, 't>)> =
            lists.into_iter().filter_map(|(parent, list)| Some((parent?, list))).collect();
        return at_depth(build, depth - 1, &lists);
    }
    let Some((_, first)) = lists.first() else { return Err("no top-level entry".to_string()) };
    let root = SynthesizedPiece {
        spec: Arc::clone(&build.specs.root),
        slot: pres::BODY_SLOT,
        span: first.span.clone(),
        state: Arc::clone(&first.state),
        body: lists.iter().map(|(_, list)| ValuePiece::Synthesized(list)).collect(),
        payload: None,
    };
    let anchor = &build.kept[0].0;
    copy_pieces_into_value(&[ValuePiece::Synthesized(&root)], anchor, ValueContent::Shown, build.guard)
        .map_err(|error| error.to_string())
}

/// Reports [`TocFailed`] with `detail`.
fn failed(out: &mut StageOut<'_>, detail: String) {
    out.diags.push(Diagnostic::error(TocFailed { detail }, nowhere(FlmOrigin::none())));
}

/// Condition `flm.toc.failed`, an Error: the table of contents could not be
/// built, because a title nests too deeply or because of a fault in the
/// program. Recovery: the piece holds no content.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.toc.failed", message = "the table of contents could not be built: {detail}")]
pub struct TocFailed {
    /// Why it could not be built: the copy's or the build's own error text.
    pub detail: String,
}

#[cfg(test)]
mod tests {
    use super::nest;

    #[test]
    fn a_level_gap_nests_one_list_deeper_and_a_first_entry_is_top_level() {
        // section, subsubsection, subsection, section
        let (parents, depths) = nest(&[1, 3, 2, 1]);
        assert_eq!(parents, [None, Some(0), Some(0), None]);
        assert_eq!(depths, [0, 1, 1, 0]);
        // A subsection first, then a section: both at the top.
        let (parents, _) = nest(&[2, 1, 2]);
        assert_eq!(parents, [None, None, Some(1)]);
    }
}
