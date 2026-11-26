//! This module defines the stage [`BuildToc`], which builds the table of
//! contents of the [`toc`](super) feature, along with the error [`TocFailed`]
//! that the stage reports.

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
use crate::stdlib::backends::pres;
use crate::stdlib::features::index::{walk_nodes_with, ELEMENT_INDEX};
use crate::stdlib::features::refpoints::{self, REFPOINTS};

use super::{
    TocEntries, TocEntry, TocEntryPayload, TocOptions, BUILD, ENTRIES, ENTRY, LIST, PIECE,
};

/// The stage `flm.toc.build` ([`BUILD`]) lists the headings of the document
/// and builds the table of contents of the [`toc`](super) feature. It runs
/// once for the whole document.
///
/// A *heading* is a node whose element has the [`Sectioning`] aspect, such as
/// a `\section`. The stage lists every heading for which
/// [`Sectioning::in_toc`] returns `true` and whose level
/// ([`Sectioning::level`]) is at most [`TocOptions::depth`], in document order.
/// It writes two tables:
///
/// - [`ENTRIES`] (`flm.toc.entries`) contains one [`TocEntry`] for each listed
///   heading.
/// - [`PIECE`] (`flm.toc`) contains the table of contents as a
///   [piece](crate::guide::concepts_overview#pieces): nested lists of entries,
///   in which each entry contains the title that [`Sectioning::toc_title`]
///   returns for its heading. The [module
///   documentation](super#the-shape-of-the-piece) describes how the entries
///   are nested.
///
/// If no heading is listed, the table [`ENTRIES`] is empty and the piece has
/// no content, so the table of contents does not appear in the output.
///
/// The stage reads the option [`TocOptions`] and the element index
/// (`flm.element-index`). It also reads the table of reference points
/// (`flm.refpoints`), but only when the `refpoints` feature is registered.
/// Without that feature, no entry has a reference point, and the entries of
/// the table of contents have no number and no link.
///
/// If the stage cannot build the table of contents, it reports the error
/// [`TocFailed`].
#[derive(Debug, Default, Clone, Copy)]
pub struct BuildToc;

impl Stage for BuildToc {
    fn decl(&self) -> StageDecl {
        StageDecl::new(BUILD, Effect::Compute, Scope::Document)
            .writes(ENTRIES)
            .writes(TableHandle::<Piece>::new(PIECE))
            .docs(
                "Writes the headings to be listed in the table of contents to the table \
                 `flm.toc.entries`, in document order. Builds the table of contents as nested \
                 lists in the piece `flm.toc`.",
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

/// The specifications of the three synthesized elements that make up the
/// content of the piece: [`PIECE`] (the root), [`LIST`] and [`ENTRY`].
struct Specs {
    root: Arc<FlmSynthesizedSpec>,
    list: Arc<FlmSynthesizedSpec>,
    entry: Arc<FlmSynthesizedSpec>,
}

/// Returns the specification of the synthesized element `name`, from which the
/// nodes of this element are created. Returns the description of the error if
/// `name` is not registered, or is registered more than once.
fn spec(registry: &FlmRegistry, name: &str) -> Result<Arc<FlmSynthesizedSpec>, String> {
    match registry.element_value(name) {
        Ok(Some(element)) => Ok(Arc::new(FlmSynthesizedSpec::new(Arc::clone(element)))),
        Ok(None) => Err(format!("missing element ‘{name}’ in the registry")),
        Err(error) => Err(error.to_string()),
    }
}

/// Computes how the entries of the table of contents are nested, from the
/// level of each entry (`levels`, in document order).
///
/// An entry is placed in the sub-list of the closest preceding entry whose
/// level is smaller. An entry without such a preceding entry is a top-level
/// entry.
///
/// Returns two vectors with one element for each entry. The first vector
/// contains the index of the entry in whose sub-list the entry is placed, or
/// `None` for a top-level entry. The second vector contains the depth of the
/// entry: 0 for a top-level entry, 1 for an entry in the sub-list of a
/// top-level entry, and so on.
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

/// The data that [`at_depth`] reads at every depth while it builds the
/// content of the piece.
struct Build<'k, 't> {
    specs: Specs,
    kept: &'k [(NodeRef<'t, Flm>, TocEntry)],
    titles: &'k [Vec<NodeRef<'t, Flm>>],
    parents: Vec<Option<usize>>,
    depths: Vec<usize>,
    guard: GuardPolicy,
}

/// Builds the content of the piece [`PIECE`] from the listed headings, each
/// given with its entry in `kept`.
///
/// Returns `Ok(None)` if `kept` is empty. Returns the description of the
/// error if the content cannot be built.
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

/// Builds the content of the piece from the deepest entries up to the root,
/// one depth per call, and returns the content as a synthesized value.
/// Returns the description of the error if the content cannot be built.
///
/// Each new node is described by a [`SynthesizedPiece`], which
/// [`copy_pieces_into_value`] turns into a node at the end. A call creates the
/// entry nodes ([`ENTRY`]) of depth `depth`, and groups consecutive entries
/// that belong to the same sub-list into a list node ([`LIST`]). Each entry
/// node contains the title of its heading, followed by the sub-list of the
/// entry if there is one in `below`. `below` contains the list nodes of depth
/// `depth + 1`, each with the index of the entry that it belongs to.
///
/// The call then calls this function for the depth above, with its own list
/// nodes. At depth 0, the function places the list of top-level entries in a
/// root node ([`PIECE`]) and copies the root node into a synthesized value.
///
/// The nodes of each depth are stored in the frame of the call that creates
/// them, because the nodes of the depth above borrow them.
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
    let Some((_, first)) = lists.first() else { return Err("missing top-level entry".to_string()) };
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

/// The error that is reported when the stage [`BuildToc`] cannot build the
/// table of contents (id `flm.toc.failed`).
///
/// A document causes this error only when the title of a heading is nested
/// too deeply to be copied into the table of contents (see [the recursion
/// guard](crate::lang::guard)). Any other cause is a fault of the program.
///
/// Reported by [`BuildToc`] at no position in the document. If the error
/// occurs while the stage builds the table of contents, the piece [`PIECE`]
/// has no content, so the table of contents does not appear in the output.
/// The table [`ENTRIES`] is written as usual in this case. If the error
/// occurs while the stage reads its input, the stage writes neither table.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.toc.failed", message = "Failed to build the table of contents: {detail}")]
pub struct TocFailed {
    /// The description of the failure.
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
