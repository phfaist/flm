//! The instantiation of a compiled template at one use site of a definition:
//! the **substitution pass**, then the **re-sourcing pass**.
//!
//! Instantiating a compiled template at a use site takes two passes over the
//! template's tree. Each pass builds a new tree, and both are driven by techy's
//! restage driver (a traversal that builds a new tree as it goes):
//!
//! 1. The substitution pass (`Substitute`) replaces every placeholder node by
//!    the value that the placeholder bindings ([`PlaceholderBindings`]) give it,
//!    and every conditional by the branch its test selects. A placeholder's
//!    value is one of: the use site's already parsed argument or body nodes,
//!    copied with their own positions in the document and never parsed again; a
//!    default value's compiled tree, instantiated in place; the finished content
//!    of a tree an expansion hook built, run through this pass so that no
//!    placeholder node remains in it; the placeholder's own substitution
//!    argument, copied; or nothing, for an argument without a value. Every other
//!    node is copied, and the driver rebuilds the records of the template's
//!    commands (techy's record of which children form each argument and the
//!    body) around children that were multiplied or removed.
//! 2. The re-sourcing pass (`Resource`) copies every node whose position points
//!    into the compiled template, or into a default value's compiled tree, onto
//!    a new source created for this use: a source with the template's text that
//!    records the use site's position as the place where it was created. Each
//!    copied node keeps its byte range in that text. Every expanded node can therefore
//!    be traced both to its position in the template and to the use site, and
//!    two uses of one definition are told apart. The nodes of a compiled
//!    template have held their own text since compilation, so a copy carries its
//!    text with it; the use site's nodes, inserted by the first pass, keep their
//!    positions. This pass copies each node exactly once, so the records of
//!    every command are rebuilt unchanged from what techy's driver returns.
//!
//! The [expand pass](super::expand) then runs the result through itself, one
//! level deeper: substitute, then expand. The function [`instantiate`] runs the
//! two passes, for the expand pass, for the other callers inside the library and
//! for a feature outside it, so that every caller gets the same result for the
//! same template and bindings.
//!
//! # Values an expansion hook computes
//!
//! An expansion hook whose definition declares extra placeholders gives their
//! values as data: [`ExtraValues`] holds each value under the placeholder's
//! name and the text of its substitution argument, and
//! [`over`](ExtraValues::over) lays the values over the use site's bindings
//! ([`LayeredBindings`]), with which the hook instantiates the template
//! ([`Expander::instantiate`](super::Expander::instantiate)).
//!
//! # Instantiating a template outside the expand pass
//!
//! A feature that compiles a template of its own against the built language
//! ([`compile_template_with`](super::compile_template_with)) and writes its
//! instantiation from somewhere other than an expansion hook, such as a
//! lowering rule writing a title number, calls [`instantiate`] itself. The
//! values of the template's extra placeholders are finished trees the feature
//! computed, which [`ValueBindings`] holds by name.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use techy::core::node::{
    BuildId, CallableData, ChildRegion, NodeBuildError, NodeKind, NodeRef, NodeSlice, NodeTree,
    ParsedArgument, ParsedArguments, ParsedSlot, ParsedSlots,
};
use techy::source::{Source, SourceSpan};
use techy::transform::{Restage, RestageContext, RestageVisitor};

use crate::model::element::element_value;
use crate::lang::flm::{Flm, FlmCallableType};
use crate::lang::origin::{FlmOrigin, FlmSource, FlmSpan};
use crate::lang::guard::{self, GuardPolicy};
use crate::lang::text::is_blank;
use crate::lang::rewrites::userdefs::hook::{ExpandError, PlaceholderBindings, PlaceholderValue};
use crate::lang::rewrites::userdefs::template::{
    substitution_of, substitution_text_of, CompiledTemplate, Conditional, ConditionalTest,
    PlaceholderRef, PlaceholderTarget,
};

/// The context of the two passes, through which they build nodes into the new
/// tree: no annotations in or out.
type Cx<'t> = RestageContext<'t, Flm, (), ()>;

/// How deep the blankness test (`\ifblank`) follows default values into other
/// default values before it stops and treats the content as not blank. The
/// default values of a definition cannot refer to each other in a cycle (the
/// compiler refuses one), but the placeholder bindings of an expansion hook are
/// not covered by that check.
const BLANK_TEST_DEPTH: usize = 32;

/// A visitor that copies a subtree exactly (positions, parsing states, node
/// extensions, records). The use site's parsed nodes enter an instance this way,
/// keeping the origin they were parsed with.
pub(crate) struct Verbatim;

impl RestageVisitor<Flm, (), ()> for Verbatim {
    type Error = ExpandError;

    fn restage(
        &mut self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut Cx<'_>,
    ) -> Result<Restage<()>, ExpandError> {
        Ok(Restage::Descend(()))
    }
}

/// Copies the nodes of `slice` into the new tree, exactly.
fn copy_slice(cx: &mut Cx<'_>, slice: NodeSlice<'_, Flm>) -> Result<Vec<BuildId>, ExpandError> {
    let mut ids = Vec::new();
    for node in slice.iter() {
        ids.extend(cx.restage_subtree(node, &mut Verbatim)?);
    }
    Ok(ids)
}

/// Whether `node` is ignored around the single placeholder that a value test
/// reads: a blank node ([`is_blank`]: a run of whitespace, or a node that is
/// whitespace, such as `~` and `\ `), or a comment. These are left out before
/// the test argument is read, so that a template may lay out a conditional
/// readably.
pub(crate) fn is_noise(node: &NodeRef<'_, Flm>) -> bool {
    node.is_comment() || is_blank(node)
}

/// Whether `node` counts as blank for `\ifblank`: a comment, and nothing else.
/// Whitespace, `~` and `\ ` included, is content for this test (an argument
/// written as one space is not blank), which is why this differs from
/// [`is_noise`].
pub(crate) fn is_blank_node(node: &NodeRef<'_, Flm>) -> bool {
    node.is_comment()
}

/// The substitution pass (see the module documentation). The only problem in
/// the document that it finds is a placeholder without a value, which
/// [`Expander::instantiate`](super::Expander::instantiate) turns into the use
/// site's recovery; any other failure fails the whole expansion.
pub(crate) struct Substitute<'a> {
    bindings: &'a dyn PlaceholderBindings,
    /// The compiled trees the pass has entered: the template first, then every
    /// default value it instantiated. The placeholder nodes are looked up in them,
    /// and the re-sourcing pass replaces their sources.
    templates: Vec<&'a CompiledTemplate>,
}

impl<'a> Substitute<'a> {
    /// The substitution of `template` under `bindings`.
    pub(crate) fn new(
        template: &'a CompiledTemplate,
        bindings: &'a dyn PlaceholderBindings,
    ) -> Substitute<'a> {
        Substitute { bindings, templates: vec![template] }
    }

    /// The compiled trees the pass entered: the template and the default values it
    /// instantiated.
    pub(crate) fn templates(&self) -> &[&'a CompiledTemplate] {
        &self.templates
    }

    fn remember(&mut self, template: &'a CompiledTemplate) {
        if !self.templates.iter().any(|known| std::ptr::eq(*known, template)) {
            self.templates.push(template);
        }
    }

    /// The compiled placeholder at `node`, if `node` belongs to a compiled tree the
    /// pass has entered. A node identifier carries the tag of its tree, so one lookup
    /// per tree is the whole search.
    fn placeholder_of(&self, node: &NodeRef<'_, Flm>) -> Option<&'a PlaceholderRef> {
        self.templates.iter().find_map(|template| template.placeholder(node.id()))
    }

    /// The value of the placeholder node `node`, or [`ExpandError::BadPlaceholder`]
    /// when the pass cannot give it a value.
    fn value_of(&self, node: &NodeRef<'_, Flm>) -> Result<PlaceholderValue<'a>, ExpandError> {
        let Some(reference) = self.placeholder_of(node) else {
            // A placeholder node of no compiled tree the pass entered (content a
            // hook synthesized with a placeholder left in it): nothing binds it.
            return Err(ExpandError::BadPlaceholder {
                reference: node.name().unwrap_or("").to_string(),
            });
        };
        match self.bindings.value(&reference.target, substitution_of(node)) {
            PlaceholderValue::Unresolvable => {
                Err(ExpandError::BadPlaceholder { reference: reference.written.clone() })
            }
            value => Ok(value),
        }
    }

    /// Builds the replacement of a placeholder node.
    fn substitute(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut Cx<'_>,
    ) -> Result<Vec<BuildId>, ExpandError> {
        match self.value_of(&node)? {
            PlaceholderValue::NoValue | PlaceholderValue::Unresolvable => Ok(Vec::new()),
            PlaceholderValue::Provided(content) => copy_slice(cx, content),
            PlaceholderValue::Substitution => match substitution_of(&node) {
                Some(content) => copy_slice(cx, content),
                None => Ok(Vec::new()),
            },
            PlaceholderValue::Tree(tree) => {
                // Hook-computed content flows through this pass like a default:
                // no placeholder node survives it (an unknown one is the use
                // site's recovery), and its conditionals are evaluated.
                let mut ids = Vec::new();
                for child in tree.root().children() {
                    ids.extend(cx.restage_subtree(child, self)?);
                }
                Ok(ids)
            }
            PlaceholderValue::Default(template) => {
                // The default's own placeholders substitute in turn: its tree
                // flows through this pass.
                self.remember(template);
                let mut ids = Vec::new();
                for child in template.tree().root().children() {
                    ids.extend(cx.restage_subtree(child, self)?);
                }
                Ok(ids)
            }
        }
    }

    /// The value of a test argument that is exactly one placeholder, whitespace and
    /// comments aside; `None` when the argument is anything else.
    fn single_value(
        &self,
        argument: NodeSlice<'_, Flm>,
    ) -> Result<Option<PlaceholderValue<'a>>, ExpandError> {
        let mut significant = argument.iter().filter(|node| !is_noise(node));
        let Some(first) = significant.next() else {
            return Ok(None);
        };
        let placeholder = first.callable_type() == Some(FlmCallableType::Placeholder);
        if significant.next().is_some() || !placeholder {
            return Ok(None);
        }
        self.value_of(&first).map(Some)
    }

    /// Whether `nodes`, with their placeholders substituted, are blank: nothing at
    /// all, or nothing but comments.
    fn is_blank<'n>(
        &self,
        nodes: impl Iterator<Item = NodeRef<'n, Flm>>,
        depth: usize,
    ) -> Result<bool, ExpandError> {
        if depth > BLANK_TEST_DEPTH {
            return Ok(false);
        }
        for node in nodes {
            if is_blank_node(&node) {
                continue;
            }
            if node.callable_type() != Some(FlmCallableType::Placeholder) {
                return Ok(false);
            }
            let blank = match self.value_of(&node)? {
                PlaceholderValue::NoValue | PlaceholderValue::Unresolvable => true,
                PlaceholderValue::Provided(content) => self.is_blank(content.iter(), depth + 1)?,
                PlaceholderValue::Default(template) => {
                    self.is_blank(template.tree().root().children().iter(), depth + 1)?
                }
                PlaceholderValue::Tree(tree) => {
                    self.is_blank(tree.root().children().iter(), depth + 1)?
                }
                PlaceholderValue::Substitution => match substitution_of(&node) {
                    Some(content) => self.is_blank(content.iter(), depth + 1)?,
                    None => true,
                },
            };
            if !blank {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Whether the conditional's test holds for the test argument of the
    /// conditional node `node`.
    fn test_holds(
        &self,
        node: &NodeRef<'_, Flm>,
        test: ConditionalTest,
    ) -> Result<bool, ExpandError> {
        let Some(argument) = node.argument_content_nodes(0) else {
            return Ok(false);
        };
        Ok(match test {
            ConditionalTest::NoValue => {
                matches!(self.single_value(argument)?, Some(PlaceholderValue::NoValue))
            }
            ConditionalTest::Value => {
                !matches!(self.single_value(argument)?, Some(PlaceholderValue::NoValue))
            }
            ConditionalTest::Boolean => matches!(
                self.single_value(argument)?,
                Some(
                    PlaceholderValue::Provided(_)
                        | PlaceholderValue::Tree(_)
                        | PlaceholderValue::Substitution
                )
            ),
            ConditionalTest::Blank => self.is_blank(argument.iter(), 0)?,
            ConditionalTest::NotBlank => !self.is_blank(argument.iter(), 0)?,
        })
    }

    /// Builds the replacement of a conditional node: the content of the selected
    /// branch, itself substituted.
    fn select(
        &mut self,
        node: NodeRef<'_, Flm>,
        conditional: Conditional,
        cx: &mut Cx<'_>,
    ) -> Result<Vec<BuildId>, ExpandError> {
        let holds = self.test_holds(&node, conditional.test)?;
        let branch = conditional
            .branch_index(holds)
            .and_then(|index| node.argument_content_nodes(index));
        let mut ids = Vec::new();
        if let Some(branch) = branch {
            for child in branch.iter() {
                ids.extend(cx.restage_subtree(child, self)?);
            }
        }
        Ok(ids)
    }
}

impl RestageVisitor<Flm, (), ()> for Substitute<'_> {
    type Error = ExpandError;

    fn restage(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut Cx<'_>,
    ) -> Result<Restage<()>, ExpandError> {
        if node.callable_type() == Some(FlmCallableType::Placeholder) {
            return Ok(Restage::Emit(self.substitute(node, cx)?));
        }
        if let Some(conditional) = element_value::<Conditional>(&node) {
            return Ok(Restage::Emit(self.select(node, *conditional, cx)?));
        }
        Ok(Restage::Descend(()))
    }
}

/// Appends `added` to `children` and returns the range of positions they take
/// there.
fn append(
    children: &mut Vec<BuildId>,
    added: &[BuildId],
) -> Result<std::ops::Range<u32>, ExpandError> {
    let too_many = || ExpandError::Build(NodeBuildError::TooManyNodes);
    let start = u32::try_from(children.len()).map_err(|_| too_many())?;
    children.extend_from_slice(added);
    let end = u32::try_from(children.len()).map_err(|_| too_many())?;
    Ok(start..end)
}

// techy has no span-overriding restage primitive, and declined one: a swapped
// span would silently re-resolve byte-range payloads on another source. Copying
// the nodes here is FLM's own answer.
/// The re-sourcing pass: the copy of a compiled template onto the source
/// created for one use (see the module documentation).
pub(crate) struct Resource<'a> {
    site: FlmSpan,
    templates: &'a [&'a CompiledTemplate],
    /// The sources created for this use, each with the compiled source it replaces:
    /// (compiled source, source for this use).
    minted: Vec<(Arc<FlmSource>, Arc<FlmSource>)>,
}

impl<'a> Resource<'a> {
    /// The re-sourcing of the compiled trees `templates` for the use site at
    /// `site`.
    pub(crate) fn new(site: FlmSpan, templates: &'a [&'a CompiledTemplate]) -> Resource<'a> {
        Resource { site, templates, minted: Vec::new() }
    }

    /// The source for this use that replaces the compiled `source`, created the
    /// first time it is needed; `None` for a source that belongs to no compiled
    /// tree (that of a use site's node, or of a tree an expansion hook built).
    fn resourced(&mut self, source: &Arc<FlmSource>) -> Option<Arc<FlmSource>> {
        if let Some((_, minted)) = self.minted.iter().find(|(old, _)| Arc::ptr_eq(old, source)) {
            return Some(Arc::clone(minted));
        }
        let template =
            self.templates.iter().find(|template| Arc::ptr_eq(template.source(), source))?;
        // The label names the expansion in any diagnostic raised at an expanded
        // node; the source keys nothing (no canonical path).
        let minted = Arc::new(
            Source::synthesized(template.text(), template.label(), self.site.clone())
                .with_origin(FlmOrigin::synthetic(template.label())),
        );
        self.minted.push((Arc::clone(source), Arc::clone(&minted)));
        Some(minted)
    }

    /// The position that the copy of `node` takes: the same range on the source for
    /// this use, for a node of a compiled tree; the node's own position otherwise.
    fn span_of(&mut self, node: &NodeRef<'_, Flm>) -> FlmSpan {
        match self.resourced(node.span().source()) {
            Some(source) => SourceSpan::new(&source, node.span().range()),
            None => node.span().clone(),
        }
    }

    /// Copies a command node: its argument regions and its node slot regions (a
    /// node slot, techy's `ParsedSlot`, is a part of a node's content outside its
    /// arguments, such as an environment's body) go through this pass (each node
    /// copied once), the records are rebuilt over them, and the node is built
    /// with its new position.
    fn copy_callable(
        &mut self,
        node: NodeRef<'_, Flm>,
        data: &CallableData<Flm>,
        span: FlmSpan,
        cx: &mut Cx<'_>,
    ) -> Result<BuildId, ExpandError> {
        let mut children: Vec<BuildId> = Vec::new();
        let mut arguments = Vec::with_capacity(data.arguments.len());
        for index in 0..data.arguments.len() {
            let bundle = cx.restage_argument(node, index, self)?;
            let region = match bundle.content() {
                Some(content) if bundle.is_provided() => {
                    let range = append(&mut children, bundle.nodes())?;
                    Some(ChildRegion::new(range, content.clone()))
                }
                _ => None,
            };
            arguments.push(ParsedArgument {
                spec: Arc::clone(bundle.spec()),
                region,
                ext: bundle.ext().cloned(),
            });
        }
        let mut slots = Vec::with_capacity(data.slots.len());
        for index in 0..data.slots.len() {
            let bundle = cx.restage_slot(node, index, self)?;
            let range = append(&mut children, bundle.nodes())?;
            slots.push(ParsedSlot {
                name: bundle.name().map(Box::from),
                region: ChildRegion::new(range, bundle.content().clone()),
                role: bundle.role(),
                ext: *bundle.ext(),
            });
        }
        let kind = NodeKind::callable(CallableData {
            callable_type: data.callable_type,
            name: data.name.clone(),
            spec: Arc::clone(&data.spec),
            arguments: ParsedArguments::new(arguments),
            slots: ParsedSlots::new(slots),
            invocation_syntax: data.invocation_syntax.clone(),
        });
        let id = cx.builder().add(
            kind,
            span,
            Arc::clone(node.parsing_state()),
            children,
            node.ext().clone(),
            (),
        )?;
        Ok(id)
    }
}

impl RestageVisitor<Flm, (), ()> for Resource<'_> {
    type Error = ExpandError;

    fn restage(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut Cx<'_>,
    ) -> Result<Restage<()>, ExpandError> {
        let span = self.span_of(&node);
        let id = match node.callable() {
            Some(data) => self.copy_callable(node, data, span, cx)?,
            None => {
                let children = cx.restage_children(node, self)?;
                cx.builder().add(
                    node.kind().clone(),
                    span,
                    Arc::clone(node.parsing_state()),
                    children,
                    node.ext().clone(),
                    (),
                )?
            }
        };
        Ok(Restage::Emit(vec![id]))
    }
}

/// The nodes of an instantiated template, which the expand pass inserts in place
/// of the use site: the root's children, in order.
pub(crate) fn instance_nodes(instance: &NodeTree<Flm>) -> NodeSlice<'_, Flm> {
    instance.root().children()
}

/// Instantiates `template` as a new tree: the substitution pass under
/// `bindings`, then the re-sourcing pass onto a source created for the site
/// `site` (see the module documentation). The instance's nodes are the
/// children of the returned tree's root. The two traversals run under the
/// [recursion guard](crate::lang::guard) policy `guard`.
///
/// `site` is the span the instance is attributed to: a use site's span when
/// the template is instantiated for a use, or any span that says where the
/// content comes from (a lowering rule passes the node it lowers; the
/// numbering feature passes the source of the counter style).
///
/// This is the only function that runs the two passes, so every caller gets the
/// same result for the same template and bindings. The expand pass runs it at a use site and then expands the
/// result one level deeper ([`super::pass::expand`]);
/// [`ModeVariant::instantiate_default`](super::ModeVariant::instantiate_default)
/// runs it to give a default value's content outside an expansion, which the
/// kept-definition lowering rules write into a LaTeX call; the numbering
/// feature runs it to instantiate its own templates; and a feature outside the
/// library runs it with [`ValueBindings`] (or bindings of its own) to
/// instantiate a template it compiled ([`compile_template_with`](super::compile_template_with)).
/// The instance is not expanded: a user definition used in the template stays
/// a use site in it.
///
/// The traversals descend recursively, so the call must run on a [worker
/// thread](crate::lang::guard#the-worker-thread-requirement): a lowering
/// rule and an expansion hook already run on one; any other caller wraps the
/// call in [`on_worker_thread`](crate::lang::guard::on_worker_thread).
/// [`GuardPolicy::StackBudget`] is the policy for normal use.
///
/// # Errors
///
/// [`ExpandError::BadPlaceholder`] for a placeholder that `bindings` cannot give
/// a value, at any depth of the template (inside a conditional's branch,
/// inside a default value): a caller that has a recovery for it, as the expand
/// pass does, reads the placeholder from the error and recovers the use site.
/// A refusal of the recursion guard ([`ExpandError::descent_refusal`] answers
/// its detail) when the content nests too deeply. Any other [`ExpandError`] is
/// an implementation failure, and ends the caller's work.
pub fn instantiate(
    site: &FlmSpan,
    template: &CompiledTemplate,
    bindings: &dyn PlaceholderBindings,
    guard: GuardPolicy,
) -> Result<NodeTree<Flm>, ExpandError> {
    // Substitute: the use site's parsed nodes at the placeholders, the
    // conditionals' branches selected.
    let mut substitute = Substitute::new(template, bindings);
    let substituted =
        guard::restager(&mut substitute, guard).restage(template.tree()).map_err(ExpandError::from)?;
    // Re-source: the compiled trees' nodes onto this use's synthesized sources.
    let templates = substitute.templates().to_vec();
    let mut resource = Resource::new(site.clone(), &templates);
    guard::restager(&mut resource, guard).restage(&substituted).map_err(ExpandError::from)
}

// --- values a feature computed, by name -----------------------------------------------------------

/// Placeholder bindings that give each extra placeholder of a template
/// (`#{name}`, a name a feature computes the content of) a finished tree held
/// by name: the bindings with which a feature instantiates a template it
/// compiled, through [`instantiate`].
///
/// For an extra placeholder whose name it holds, the bindings answer the tree
/// as finished content ([`PlaceholderValue::Tree`]: the children of the tree's
/// root are substituted, with their own positions in their sources); a
/// substitution argument written on the placeholder (`#[…]{name}`) is not
/// read. For an extra placeholder whose name it does not hold, they answer
/// [`PlaceholderValue::NoValue`]: nothing is substituted and
/// `\IfNoValueTF{#{name}}` takes its first branch. For an argument (`#1`, a
/// declared argument name) or the body (`#{body}`), which a value template
/// does not have, they answer [`PlaceholderValue::Unresolvable`], and
/// [`instantiate`] fails with [`ExpandError::BadPlaceholder`].
///
/// ```
/// use flm_core::lang::rewrites::userdefs::{
///     PlaceholderBindings, PlaceholderTarget, PlaceholderValue, ValueBindings,
/// };
///
/// let bindings = ValueBindings::new();
/// let number = PlaceholderTarget::Extra("number".to_string());
/// assert!(matches!(bindings.value(&number, None), PlaceholderValue::NoValue));
/// assert!(matches!(bindings.value(&PlaceholderTarget::Argument(0), None), PlaceholderValue::Unresolvable));
/// ```
#[derive(Debug, Clone, Default)]
pub struct ValueBindings {
    /// The tree of each name, by name.
    values: BTreeMap<String, Arc<NodeTree<Flm>>>,
}

impl ValueBindings {
    /// Creates bindings that hold no value: every extra placeholder answers
    /// [`PlaceholderValue::NoValue`].
    pub fn new() -> ValueBindings {
        ValueBindings::default()
    }

    /// Builder: gives the extra placeholder `name` the finished tree `tree`,
    /// whose root's children are substituted. A name given twice keeps the
    /// later tree.
    pub fn with(mut self, name: impl Into<String>, tree: Arc<NodeTree<Flm>>) -> ValueBindings {
        self.values.insert(name.into(), tree);
        self
    }
}

impl PlaceholderBindings for ValueBindings {
    fn value<'a>(
        &'a self,
        target: &PlaceholderTarget,
        _substitution: Option<NodeSlice<'_, Flm>>,
    ) -> PlaceholderValue<'a> {
        match target {
            PlaceholderTarget::Extra(name) => match self.values.get(name) {
                Some(tree) => PlaceholderValue::Tree(tree),
                None => PlaceholderValue::NoValue,
            },
            PlaceholderTarget::Argument(_) | PlaceholderTarget::Body => PlaceholderValue::Unresolvable,
        }
    }
}

// --- values a hook computed, by name and substitution argument -----------------------------------

/// The value [`ExtraValues`] holds for one extra placeholder.
#[derive(Debug, Clone)]
enum ExtraValue {
    /// A compiled template, instantiated in place.
    Template(Arc<CompiledTemplate>),
    /// Finished content: the children of the tree's root.
    Tree(Arc<NodeTree<Flm>>),
    /// Nothing to substitute.
    NoValue,
}

/// The values that an expansion hook computed for the extra placeholders of a
/// template at one use site, each held under the placeholder's name and the
/// text of its substitution argument. Laid over the use site's other bindings
/// with [`over`](ExtraValues::over), they give the bindings with which the hook
/// instantiates the template ([`Expander::instantiate`](super::Expander::instantiate)),
/// with no implementation of [`PlaceholderBindings`] of its own.
///
/// An *extra placeholder* is a name whose content the hook computes at each
/// use ([`Definition::extra_placeholder`](super::Definition::extra_placeholder)).
/// A template writes it as `#{name}` or, when the name accepts one, with a
/// bracketed *substitution argument* that the hook reads as data
/// (`#[\lvert]{DelimLeft}`: the delimiter to draw). One name may be written
/// several times with different substitution arguments, and each pair of a
/// name and a substitution argument has a value of its own here.
///
/// # The key of a value
///
/// A value is held under the placeholder's name and the text of its
/// substitution argument: the source text between the brackets, with the
/// whitespace around it removed (`Some("\\lvert")` for `#[ \lvert ]{DelimLeft}`,
/// `Some("")` for `#[]{DelimLeft}`), or `None` for a placeholder written
/// without one (`#{DelimLeft}`). [`CompiledTemplate::substitution_text`] gives
/// this text for each placeholder of a compiled template. A hook therefore
/// lists the placeholders of the template it instantiates and of the default
/// values of the arguments ([`CompiledTemplate::placeholders`],
/// [`ModeVariant::defaults`](super::ModeVariant::defaults)), computes a value
/// for each name and text it finds, and adds it here.
///
/// # The values
///
/// - [`template`](ExtraValues::template): a compiled template, instantiated in
///   place of the placeholder ([`PlaceholderValue::Default`]): one the hook
///   compiled ([`compile_template_with`](super::compile_template_with)), or a
///   definition's template or default value
///   ([`ModeVariant::template`](super::ModeVariant::template),
///   [`ModeVariant::default_of`](super::ModeVariant::default_of)). Its own
///   placeholders are given values by the same bindings, and its nodes are
///   copied onto the source created for the use, like the nodes of the template
///   being instantiated.
/// - [`tree`](ExtraValues::tree): finished content ([`PlaceholderValue::Tree`]):
///   the children of the tree's root are substituted, with their own positions
///   in their sources.
/// - [`no_value`](ExtraValues::no_value): nothing is substituted, and
///   `\IfNoValueTF{#{name}}` takes its first branch
///   ([`PlaceholderValue::NoValue`]).
///
/// A pair of a name and a substitution argument given twice keeps the later
/// value.
///
/// # The bindings
///
/// [`over`](ExtraValues::over) lays the values over base bindings, usually the
/// use site's [`DefinitionBindings`](super::DefinitionBindings). The resulting
/// [`LayeredBindings`] answer an extra placeholder from the values, and a pair
/// that the values do not hold as
/// [`Unresolvable`](PlaceholderValue::Unresolvable): the instantiation fails
/// with an error that gives the placeholder
/// ([`ExpandError::bad_placeholder`]), which
/// [`Expander::instantiate`](super::Expander::instantiate) handles by replacing
/// the use site by its literal source text and reporting
/// `flm.userdefs.bad-placeholder`.
/// An argument (`#1`, a declared argument name) and the body (`#{body}`) are
/// answered by the base bindings.
///
/// [`ValueBindings`] are the simpler form for a template that has no use site:
/// they hold one finished tree per name, read no substitution argument, have no
/// base bindings, and give a name they do not hold no value.
///
/// # Examples
///
/// A template whose extra placeholder `side` takes a substitution argument,
/// instantiated with a compiled template for `#[ left ]{side}` and no value for
/// `#{side}`:
///
/// ```
/// use std::sync::Arc;
///
/// use flm_core::lang::guard::{on_worker_thread, GuardPolicy};
/// use flm_core::lang::rewrites::userdefs::{
///     compile_template_with, instantiate, CompiledTemplate, ExtraValues, PlaceholderVocabulary,
///     TemplateLanguage, TemplateSpec, ValueBindings,
/// };
/// use flm_core::lang::{build_language, LangInputs};
/// use flm_core::techy::latexlike::Mode;
///
/// let lang = build_language(LangInputs::new());
/// let language = TemplateLanguage::new(&lang).expect("the template language mints");
/// let compile = |text: &str, vocabulary: &PlaceholderVocabulary| -> CompiledTemplate {
///     let spec = TemplateSpec {
///         text,
///         mode: Mode::Text,
///         vocabulary,
///         offset: 0,
///         definition: "example",
///         label: "a template of the example",
///         origin_label: "a template of the example",
///         user_definition: false,
///     };
///     compile_template_with(&spec, &lang, &language).expect("the template compiles")
/// };
/// let vocabulary = PlaceholderVocabulary::new(0).extra("side", true);
/// let template = compile("(#[ left ]{side})(#{side})", &vocabulary);
///
/// // The keys of the template's placeholders.
/// let mut keys: Vec<Option<String>> =
///     template.placeholders().map(|(node, _)| template.substitution_text(node)).collect();
/// keys.sort();
/// assert_eq!(keys, [None, Some("left".to_string())]);
///
/// let values = ExtraValues::new()
///     .template("side", Some("left"), Arc::new(compile("L", &PlaceholderVocabulary::new(0))))
///     .no_value("side", None);
/// // The base bindings of a template without arguments; an expansion hook lays
/// // the values over the use site's `DefinitionBindings`.
/// let base = ValueBindings::new();
/// let instantiated = |template: &CompiledTemplate| {
///     let site = template.tree().root().span().clone();
///     on_worker_thread(|| instantiate(&site, template, &values.over(&base), GuardPolicy::StackBudget))
///         .expect("the worker thread starts")
/// };
///
/// let instance = instantiated(&template).expect("every placeholder has a value");
/// let text: String = instance.root().descendants().filter_map(|node| node.chars()).collect();
/// assert_eq!(text, "(L)()");
///
/// // A substitution argument the values do not hold: no value can be given.
/// let other = compile("#[right]{side}", &vocabulary);
/// let error = instantiated(&other).expect_err("`#[right]{side}` has no value");
/// assert_eq!(error.bad_placeholder(), Some("side"));
/// ```
#[derive(Debug, Clone, Default)]
pub struct ExtraValues {
    /// The value of each pair, by name, then by the text of the substitution
    /// argument.
    values: BTreeMap<String, BTreeMap<Option<String>, ExtraValue>>,
}

impl ExtraValues {
    /// Creates values that hold nothing: laid over base bindings, they answer
    /// every extra placeholder as [`PlaceholderValue::Unresolvable`].
    pub fn new() -> ExtraValues {
        ExtraValues::default()
    }

    /// Builder: gives the extra placeholder `name`, written with the
    /// substitution argument whose text is `substitution` (`None` for a
    /// placeholder written without one; see [the key of a
    /// value](ExtraValues#the-key-of-a-value)), the compiled template
    /// `template`, instantiated in place ([`PlaceholderValue::Default`]).
    pub fn template(
        self,
        name: impl Into<String>,
        substitution: Option<&str>,
        template: Arc<CompiledTemplate>,
    ) -> ExtraValues {
        self.insert(name.into(), substitution, ExtraValue::Template(template))
    }

    /// Builder: gives the extra placeholder `name`, written with the
    /// substitution argument whose text is `substitution` (`None` for a
    /// placeholder written without one; see [the key of a
    /// value](ExtraValues#the-key-of-a-value)), the finished tree `tree`, whose
    /// root's children are substituted ([`PlaceholderValue::Tree`]).
    pub fn tree(
        self,
        name: impl Into<String>,
        substitution: Option<&str>,
        tree: Arc<NodeTree<Flm>>,
    ) -> ExtraValues {
        self.insert(name.into(), substitution, ExtraValue::Tree(tree))
    }

    /// Builder: gives the extra placeholder `name`, written with the
    /// substitution argument whose text is `substitution` (`None` for a
    /// placeholder written without one; see [the key of a
    /// value](ExtraValues#the-key-of-a-value)), no value
    /// ([`PlaceholderValue::NoValue`]).
    pub fn no_value(self, name: impl Into<String>, substitution: Option<&str>) -> ExtraValues {
        self.insert(name.into(), substitution, ExtraValue::NoValue)
    }

    /// The bindings that answer an extra placeholder from these values and
    /// every other placeholder from `base` (see [the
    /// bindings](ExtraValues#the-bindings)).
    pub fn over<'v>(&'v self, base: &'v dyn PlaceholderBindings) -> LayeredBindings<'v> {
        LayeredBindings { values: self, base }
    }

    /// Holds `value` under `name` and `substitution`, replacing any value held
    /// there.
    fn insert(mut self, name: String, substitution: Option<&str>, value: ExtraValue) -> ExtraValues {
        self.values.entry(name).or_default().insert(substitution.map(str::to_string), value);
        self
    }
}

/// The placeholder bindings that [`ExtraValues::over`] gives: the values that
/// an expansion hook computed for the extra placeholders of a template, laid
/// over base bindings. An extra placeholder is answered from the
/// [`ExtraValues`] by its name and the text of its substitution argument, and
/// as [`Unresolvable`](PlaceholderValue::Unresolvable) when they hold no value
/// for that pair; an argument and the body are answered by the base bindings.
#[derive(Clone, Copy)]
pub struct LayeredBindings<'v> {
    /// The values of the extra placeholders.
    values: &'v ExtraValues,
    /// The bindings of the arguments and the body.
    base: &'v dyn PlaceholderBindings,
}

impl fmt::Debug for LayeredBindings<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LayeredBindings").field("values", self.values).finish_non_exhaustive()
    }
}

impl PlaceholderBindings for LayeredBindings<'_> {
    fn value<'a>(
        &'a self,
        target: &PlaceholderTarget,
        substitution: Option<NodeSlice<'_, Flm>>,
    ) -> PlaceholderValue<'a> {
        let PlaceholderTarget::Extra(name) = target else {
            return self.base.value(target, substitution);
        };
        let key = substitution.map(|nodes| substitution_text_of(&nodes));
        match self.values.values.get(name).and_then(|values| values.get(&key)) {
            Some(ExtraValue::Template(template)) => PlaceholderValue::Default(template),
            Some(ExtraValue::Tree(tree)) => PlaceholderValue::Tree(tree),
            Some(ExtraValue::NoValue) => PlaceholderValue::NoValue,
            None => PlaceholderValue::Unresolvable,
        }
    }
}
