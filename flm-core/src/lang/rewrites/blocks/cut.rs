//! The cut step: the first of the two walks of the block-structure pass, which
//! cuts every [block-transparent](super::transparent) node into cut parts
//! before the layout step builds any paragraph.
//!
//! The step is one restage (techy's walk that builds a new tree from the old
//! one, node by node) over the tree, under the
//! [recursion guard](crate::lang::guard). At a node whose element carries the
//! [`BlockTransparent`] aspect, the step calls the aspect: the items it returns
//! replace the node among its siblings, without a wrapper, and
//! [`Cutting::Default`] continues into the node's arguments and node slots
//! (techy's `ParsedSlot`, the parts of its content outside the arguments, such
//! as an environment's body) like any other node.
//! Every other node is copied (texts, comments, groups and callables, their
//! arguments and slots walked by the step). An [opaque
//! region](crate::guide::concepts_overview#opaque-regions) is copied unchanged,
//! with the exception the layout step also makes: an argument or a body that a
//! construct of mathematics reads in text mode is document content and is
//! walked. The exception does not reach inside a verbatim group or an opaque
//! wrapper, whose content is copied unchanged whatever its parsing mode.
//!
//! The step does not look at region layouts: it cuts in a content region laid
//! out inline as in one laid out as blocks. The layout step decides, and
//! reports, what a paragraph-break marker or a block cut part means where it
//! ends up.
//!
//! The walk also implements [`BlockCutter`], the operations the aspects call.

// Why the step drives its own descent, as the layout walk does: a node cut
// into several pieces changes the number of children of the node holding it,
// and techy refuses a changed child count under a node an argument's content
// designation points into. The step therefore builds each region's new content
// itself and hands it to `restage_argument_with_content` /
// `restage_slot_with_content`, and `Descend` survives only inside opaque
// regions, where nothing is cut.

// Why a rebuild with a payload goes through the raw builder: techy's
// `restage_invocation` copies the node's extension unchanged, so a node carrying
// data of its own is assembled here from the same public parts — the argument
// and slot bundles read back through their accessors, laid out over the new
// child list as `restage_invocation` lays them out. See the design rationale
// at [§flm-dd-dr:lang-blocks-cut].

use std::sync::Arc;

use techy::core::node::{
    BodySlotExt, BuildId, CallableData, ChildRegion, NodeKind, NodeRef, NodeTree, ParsedArgument,
    ParsedArguments, ParsedSlot, ParsedSlots,
};
use techy::transform::{Restage, RestageVisitor, RestagedArgument, RestagedSlot};

use super::error::StructureError;
use super::hook::{StructureCx, SynthesizedNode};
use super::pass::{copy_leaf, copy_over, mint};
use super::predicates::{
    Opacity, argument_opacity, group_opacity, is_content, is_math_run, is_opaque_group,
    is_parbreak, run_opacity, slot_opacity,
};
use super::transparent::{BlockCutter, BlockTransparent, CutItem, CutPart, Cutting};
use crate::lang::ext::{FlmNodeExt, PresPayload};
use crate::lang::flm::Flm;
use crate::lang::guard::{self, GuardPolicy};
use crate::lang::mint::reads_text_in_math;
use crate::lang::origin::{FlmDiagnostic, FlmDiagnostics};
use crate::model::aspects::{BlockRole, block_role_of};
use crate::model::element::{ElementSpec, element_of};

/// Runs the cut step over `tree` under the guard `policy`, and returns the cut
/// tree. The diagnostics the aspects report are pushed onto `diags`.
pub(super) fn cut(
    tree: &NodeTree<Flm>,
    policy: GuardPolicy,
    diags: &mut FlmDiagnostics,
) -> Result<NodeTree<Flm>, StructureError> {
    let mut walk = Cut {
        diags,
        raw: None,
        at_root: true,
        cuts: 0,
        answered: None,
    };
    Ok(guard::restager(&mut walk, policy).restage(tree)?)
}

/// The walk: the visitor of the restage, and the [`BlockCutter`] the aspects
/// receive.
struct Cut<'a> {
    diags: &'a mut FlmDiagnostics,
    /// Whether the run being walked is an opaque region, and of which kind.
    raw: Option<Opacity>,
    /// Whether the next visited node is the tree's root (the driver's first
    /// call).
    at_root: bool,
    /// How many cut numbers have been given out.
    cuts: usize,
    /// The items the last visited node was cut into, when its aspect cut it;
    /// set at the end of every visit, so that right after one node is staged
    /// it describes that node.
    answered: Option<Vec<CutItem>>,
}

impl RestageVisitor<Flm, (), ()> for Cut<'_> {
    type Error = StructureError;

    fn restage(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut StructureCx<'_>,
    ) -> Result<Restage<()>, StructureError> {
        let (restage, answered) = self.visit(node, cx)?;
        self.answered = answered;
        Ok(restage)
    }
}

impl Cut<'_> {
    /// One node: the root, a node of an opaque region, a block-transparent
    /// node, or any other node.
    fn visit(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut StructureCx<'_>,
    ) -> Result<(Restage<()>, Option<Vec<CutItem>>), StructureError> {
        if self.at_root {
            self.at_root = false;
            return Ok((Restage::Emit(self.root(cx, node)?), None));
        }
        match self.raw {
            Some(Opacity::AnyMode) => return Ok((Restage::Descend(()), None)),
            Some(Opacity::Math) => {
                if is_opaque_group(&node) {
                    return Ok((Restage::Emit(vec![self.group(cx, node)?]), None));
                }
                if reads_text_in_math(&node) {
                    return Ok((Restage::Emit(vec![self.callable(cx, node)?]), None));
                }
                return Ok((Restage::Descend(()), None));
            }
            None => {}
        }
        if let Some(aspect) = element_of(&node).aspect::<dyn BlockTransparent>() {
            if let Cutting::Emit(items) = aspect.cut(node, cx, self)? {
                let ids = items.iter().map(|item| item.id()).collect();
                return Ok((Restage::Emit(ids), Some(items)));
            }
        }
        Ok((Restage::Emit(self.default_node(cx, node)?), None))
    }

    /// The root: a list root rebuilt over its walked children; any other root
    /// walked as a one-node run, and wrapped in a list when it was cut into
    /// several nodes, so that the result still has one root.
    fn root(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: NodeRef<'_, Flm>,
    ) -> Result<Vec<BuildId>, StructureError> {
        if node.is_list() {
            let run: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
            let out = self.run(cx, &run, None)?;
            return Ok(vec![copy_over(cx, &node, out)?]);
        }
        let out = self.run(cx, &[node], None)?;
        if out.len() == 1 {
            return Ok(out);
        }
        let span = node.span().clone();
        let state = Arc::clone(node.parsing_state());
        Ok(vec![mint(cx, NodeKind::list(), span, state, out)?])
    }

    /// Walks a run of sibling nodes and returns their replacements,
    /// concatenated. `raw` says the run is an opaque region, and of which
    /// kind; a run parsed in mathematics mode is one of the kind
    /// [`Opacity::Math`] when `raw` is `None`.
    fn run(
        &mut self,
        cx: &mut StructureCx<'_>,
        run: &[NodeRef<'_, Flm>],
        raw: Option<Opacity>,
    ) -> Result<Vec<BuildId>, StructureError> {
        let saved = self.raw;
        self.raw = raw.or_else(|| is_math_run(run).then_some(Opacity::Math));
        let mut out = Vec::new();
        let mut result = Ok(());
        for node in run {
            match cx.restage_subtree(*node, self) {
                Ok(ids) => out.extend(ids),
                Err(error) => {
                    result = Err(StructureError::from(error));
                    break;
                }
            }
        }
        self.raw = saved;
        result.map(|()| out)
    }

    /// A node that is not cut, outside an opaque region: copied, its content
    /// walked.
    fn default_node(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: NodeRef<'_, Flm>,
    ) -> Result<Vec<BuildId>, StructureError> {
        match node.kind() {
            NodeKind::Chars { .. } | NodeKind::Comment(_) => Ok(vec![copy_leaf(cx, &node)?]),
            NodeKind::Group(_) => Ok(vec![self.group(cx, node)?]),
            NodeKind::List => {
                let run: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
                let children = self.run(cx, &run, None)?;
                Ok(vec![copy_over(cx, &node, children)?])
            }
            NodeKind::Callable(_) => Ok(vec![self.callable(cx, node)?]),
        }
    }

    /// A group, copied over its walked children: the content of a verbatim
    /// group or of an opaque wrapper is an opaque region of the kind
    /// [`Opacity::AnyMode`], the content of a mathematics group one of the
    /// kind [`Opacity::Math`] ([`group_opacity`]).
    fn group(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: NodeRef<'_, Flm>,
    ) -> Result<BuildId, StructureError> {
        let run: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
        let children = self.run(cx, &run, group_opacity(&node))?;
        copy_over(cx, &node, children)
    }

    /// A callable that is not cut: every part walked, the node rebuilt with
    /// its extension unchanged.
    fn callable(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: NodeRef<'_, Flm>,
    ) -> Result<BuildId, StructureError> {
        let count = node.arguments().map_or(0, |arguments| arguments.len());
        let mut arguments = Vec::with_capacity(count);
        for index in 0..count {
            arguments.push(self.argument(cx, node, index)?);
        }
        let slots = self.slots(cx, node, None)?;
        Ok(cx.restage_invocation(node, arguments, slots, ())?)
    }

    /// The argument at `index` with its content walked (an opaque argument
    /// copied unchanged); an argument that was not written stays unwritten.
    fn argument(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: NodeRef<'_, Flm>,
        index: usize,
    ) -> Result<RestagedArgument<Flm>, StructureError> {
        let Some(record) = node.arguments().and_then(|arguments| arguments.get(index)) else {
            return Err(StructureError::cut(
                element_of(&node).name.clone(),
                format!("the node has no argument at position {index}"),
            ));
        };
        if !record.is_provided() {
            return Ok(RestagedArgument::absent(Arc::clone(&record.spec)));
        }
        let content: Vec<NodeRef<'_, Flm>> = node
            .argument_content_nodes(index)
            .map(|slice| slice.iter().collect())
            .unwrap_or_default();
        let raw = argument_opacity(&node, index, &content);
        let ids = self.run(cx, &content, raw)?;
        Ok(cx.restage_argument_with_content(node, index, ids, ())?)
    }

    /// The node slots of a callable (techy's `ParsedSlot`, the parts of its
    /// content outside its arguments), each with its content walked (an opaque body
    /// copied unchanged), except the body slot when `body` supplies its new
    /// content already staged.
    fn slots(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: NodeRef<'_, Flm>,
        mut body: Option<Vec<BuildId>>,
    ) -> Result<Vec<RestagedSlot<Flm>>, StructureError> {
        let count = node.slots().map_or(0, |slots| slots.len());
        let mut out = Vec::with_capacity(count);
        for index in 0..count {
            let is_body = node
                .slots()
                .and_then(|slots| slots.get(index))
                .is_some_and(|slot| slot.ext.is_body());
            if is_body {
                if let Some(ids) = body.take() {
                    out.push(cx.restage_slot_with_content(node, index, ids, ())?);
                    continue;
                }
            }
            let content: Vec<NodeRef<'_, Flm>> = node
                .slot_content_nodes(index)
                .map(|slice| slice.iter().collect())
                .unwrap_or_default();
            let ids = self.run(cx, &content, slot_opacity(&node, index))?;
            out.push(cx.restage_slot_with_content(node, index, ids, ())?);
        }
        Ok(out)
    }
}

// --- the façade ------------------------------------------------------------------------

impl BlockCutter for Cut<'_> {
    fn restage_and_classify(
        &mut self,
        cx: &mut StructureCx<'_>,
        run: &[NodeRef<'_, Flm>],
    ) -> Result<Vec<CutItem>, StructureError> {
        let raw = run_opacity(run);
        let mut items = Vec::with_capacity(run.len());
        for node in run {
            if raw.is_none() && is_parbreak(node) {
                items.push(CutItem::Marker(copy_leaf(cx, node)?));
                continue;
            }
            let ids = self.run(cx, std::slice::from_ref(node), raw)?;
            if let Some(passed) = self.answered.take() {
                items.extend(passed);
                continue;
            }
            let class: fn(BuildId) -> CutItem = if raw.is_none() && block_role_of(node) == BlockRole::Block {
                CutItem::Block
            } else if is_content(node) {
                CutItem::Inline
            } else {
                CutItem::Blank
            };
            items.extend(ids.into_iter().map(class));
        }
        Ok(items)
    }

    fn rebuild(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: &NodeRef<'_, Flm>,
        replacements: Vec<(CutPart, Vec<BuildId>)>,
        payload: Option<Arc<dyn PresPayload>>,
    ) -> Result<BuildId, StructureError> {
        let fail = |detail: String| StructureError::cut(element_of(node).name.clone(), detail);
        let Some(data) = node.callable() else {
            return Err(fail("the node to rebuild is not a callable".to_string()));
        };
        let records: Vec<&ParsedArgument<Flm>> = data.arguments.iter().collect();
        let mut contents: Vec<Option<Vec<BuildId>>> = vec![None; records.len()];
        let mut body: Option<Vec<BuildId>> = None;
        for (part, content) in replacements {
            let index = match &part {
                CutPart::Body => {
                    let has_body = data.slots.iter().any(|slot| slot.ext.is_body());
                    if !has_body {
                        return Err(fail("the node has no body slot to rebuild".to_string()));
                    }
                    if body.replace(content).is_some() {
                        return Err(fail("the body is replaced twice".to_string()));
                    }
                    continue;
                }
                CutPart::Argument(index) if *index < records.len() => *index,
                CutPart::Argument(index) => {
                    return Err(fail(format!("the node has no argument at position {index}")));
                }
                CutPart::ArgumentNamed(name) => records
                    .iter()
                    .position(|record| record.name() == Some(name.as_str()))
                    .ok_or_else(|| fail(format!("the node has no argument named ‘{name}’")))?,
            };
            if !records[index].is_provided() {
                return Err(fail(format!(
                    "the argument at position {index} was not written, so it has no \
                     delimiters to hold new content"
                )));
            }
            if contents[index].replace(content).is_some() {
                return Err(fail(format!("the argument at position {index} is replaced twice")));
            }
        }
        let mut arguments = Vec::with_capacity(records.len());
        for (index, content) in contents.into_iter().enumerate() {
            arguments.push(match content {
                Some(ids) => cx.restage_argument_with_content(*node, index, ids, ())?,
                None => self.argument(cx, *node, index)?,
            });
        }
        let slots = self.slots(cx, *node, body)?;
        let ext = match payload {
            Some(payload) => node.ext().clone().with_payload(payload),
            None => node.ext().clone(),
        };
        invoke(cx, node, data, arguments, slots, ext)
    }

    fn carry(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: &NodeRef<'_, Flm>,
    ) -> Result<BuildId, StructureError> {
        let mut ids = cx.restage_subtree(*node, &mut Unchanged)?;
        match (ids.pop(), ids.is_empty()) {
            (Some(id), true) => Ok(id),
            _ => Err(StructureError::cut(
                element_of(node).name.clone(),
                "an unchanged copy did not stage exactly one node",
            )),
        }
    }

    fn synthesize_callable(
        &mut self,
        node: &NodeRef<'_, Flm>,
        element: &Arc<ElementSpec>,
    ) -> Result<SynthesizedNode, StructureError> {
        SynthesizedNode::mint(node, element)
    }

    fn diagnose(&mut self, diagnostic: FlmDiagnostic) {
        self.diags.push(diagnostic);
    }

    fn new_cut_number(&mut self) -> usize {
        let number = self.cuts;
        self.cuts += 1;
        number
    }
}

/// Stages the callable `node`, whose callable data is `data`, from the
/// restaged arguments `arguments` and the restaged node slots `slots` (a node
/// slot, techy's `ParsedSlot`, is a part of a node's content outside its
/// arguments, such as an environment's body), placed in that order as techy's
/// `restage_invocation` places them, with the extension `ext`, through
/// techy's lower-level tree builder.
fn invoke(
    cx: &mut StructureCx<'_>,
    node: &NodeRef<'_, Flm>,
    data: &CallableData<Flm>,
    arguments: Vec<RestagedArgument<Flm>>,
    slots: Vec<RestagedSlot<Flm>>,
    ext: FlmNodeExt,
) -> Result<BuildId, StructureError> {
    let mut children: Vec<BuildId> = Vec::new();
    let mut place = |nodes: &[BuildId]| -> Result<std::ops::Range<u32>, StructureError> {
        let start = children.len();
        children.extend_from_slice(nodes);
        let range = u32::try_from(start).and_then(|start| {
            u32::try_from(children.len()).map(|end| start..end)
        });
        range.map_err(|_| {
            StructureError::from(techy::core::node::NodeBuildError::TooManyNodes)
        })
    };
    let mut records = Vec::with_capacity(arguments.len());
    for bundle in &arguments {
        let region = match bundle.content() {
            Some(content) => Some(ChildRegion::new(place(bundle.nodes())?, content.clone())),
            None => None,
        };
        records.push(ParsedArgument {
            spec: Arc::clone(bundle.spec()),
            region,
            ext: bundle.ext().cloned(),
        });
    }
    let mut slot_records = Vec::with_capacity(slots.len());
    for bundle in &slots {
        slot_records.push(ParsedSlot {
            name: bundle.name().map(Box::from),
            region: ChildRegion::new(place(bundle.nodes())?, bundle.content().clone()),
            role: bundle.role(),
            ext: *bundle.ext(),
        });
    }
    let kind = NodeKind::callable(CallableData {
        callable_type: data.callable_type,
        name: data.name.clone(),
        spec: Arc::clone(&data.spec),
        arguments: ParsedArguments::new(records),
        slots: ParsedSlots::new(slot_records),
        invocation_syntax: data.invocation_syntax.clone(),
    });
    let span = node.span().clone();
    let state = Arc::clone(node.parsing_state());
    Ok(cx.builder().add(kind, span, state, children, ext, ())?)
}

/// The visitor of an unchanged copy: it descends into every node, so the
/// whole subtree is copied as it is, under the walk's guard.
struct Unchanged;

impl RestageVisitor<Flm, (), ()> for Unchanged {
    type Error = StructureError;

    fn restage(
        &mut self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut StructureCx<'_>,
    ) -> Result<Restage<()>, StructureError> {
        Ok(Restage::Descend(()))
    }
}
