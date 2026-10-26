//! The opaque wrapper that minting stages around an argument's or a body's
//! content: [`OpaqueWrapperReader`] for an argument, [`OpaqueWrapperBodyParser`]
//! for an environment's body.
//!
//! An **opaque wrapper** is a group of the class
//! [`FlmGroupType::OpaqueWrapper`], with no delimiters of its own, placed over
//! what a reader staged. The [block-structure
//! pass](crate::lang::rewrites::blocks) carries the content of such a group
//! over as written, so the element that declared the argument reads it as
//! typed. Minting places one where the declaration asks for it
//! ([`ArgDef::opaque_wrapper`](super::ArgDef::opaque_wrapper),
//! [`BodyDef::opaque_wrapper`](super::BodyDef::opaque_wrapper)), and the
//! reader of the characters-only group always does
//! ([`ArgStdKind::CharsGroup`](super::ArgStdKind::CharsGroup)). A reader that
//! decides from what it read whether to place one stages it through
//! [`wrap_argument`].
//!
//! The wrapper changes the shape of the nodes, never which nodes are the
//! content: the content designation names the same nodes as the inner
//! reader's, so every reader of an argument's or a body's content
//! (`argument_content_nodes`, `slot_content_nodes`) sees exactly what it would
//! see without the wrapper.

use std::sync::Arc;

use techy::core::constructs::{
    ArgumentParser, ConstructParser, ConstructParserResult, EnvironmentBody, ParseContext,
    ParsedArgumentNodes,
};
use techy::core::node::{BuildId, ContentNodes, GroupData, NodeBuildError, NodeKind};
use techy::core::specs::ArgumentSpec;
use techy::core::ParsingStateDelta;
use techy::error::{HookFailed, ParseError};
use techy::source::SourceSpan;

use crate::lang::flm::{Flm, FlmGroupType};
use crate::lang::origin::{FlmOrigin, FlmSpan};

/// The reader of an argument whose content is placed under an opaque wrapper:
/// it runs the inner reader, then stages one group of the class
/// [`FlmGroupType::OpaqueWrapper`], with empty delimiters, over the nodes the
/// inner reader staged.
///
/// The argument's nodes are the wrapper alone. Its content designates the
/// same nodes as the inner reader's: content inside one of the inner nodes
/// stays designated as it was, since that node is a descendant of the
/// wrapper, and content among the inner nodes themselves is designated among
/// the wrapper's children. The argument's extension data is the inner
/// reader's. An argument the inner reader reports absent stays absent, and an
/// argument the inner reader already returns as one opaque wrapper is returned
/// as it is, so that a reader is never wrapped twice.
#[derive(Debug)]
pub(crate) struct OpaqueWrapperReader {
    inner: Arc<dyn ArgumentParser<Flm>>,
}

impl OpaqueWrapperReader {
    /// Returns the reader that places what `inner` reads under an opaque
    /// wrapper.
    pub(crate) fn new(inner: Arc<dyn ArgumentParser<Flm>>) -> OpaqueWrapperReader {
        OpaqueWrapperReader { inner }
    }
}

impl ArgumentParser<Flm> for OpaqueWrapperReader {
    fn parse_argument(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
        spec: &ArgumentSpec<Flm>,
    ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
        let Some(read) = self.inner.parse_argument(cx, spec)? else {
            return Ok(None);
        };
        wrap_argument(cx, read).map(Some)
    }

    fn can_match_empty(&self) -> bool {
        self.inner.can_match_empty()
    }
}

/// The parser of an environment's body whose content is placed under an
/// opaque wrapper: it runs the inner body parser, then stages one group of
/// the class [`FlmGroupType::OpaqueWrapper`], with empty delimiters, over the
/// body node the inner parser staged, and a body node over the wrapper.
///
/// The body node a body parser stages is a list that holds the body's nodes,
/// and it has already claimed them when the inner parser returns; the wrapper
/// is therefore staged over that list, and a list over the wrapper is the
/// body's node, as every environment body is a list. The content designates
/// the same nodes as the inner parser's, in the same way as for an argument
/// ([`OpaqueWrapperReader`]). Where the body ends, how it was terminated, the
/// state it ended in and its after-effects are the inner parser's.
pub(crate) struct OpaqueWrapperBodyParser<'p> {
    inner: Box<dyn ConstructParser<Flm, Output = EnvironmentBody<Flm>> + 'p>,
}

impl<'p> OpaqueWrapperBodyParser<'p> {
    /// Returns the parser that places what `inner` reads under an opaque
    /// wrapper.
    pub(crate) fn new(
        inner: Box<dyn ConstructParser<Flm, Output = EnvironmentBody<Flm>> + 'p>,
    ) -> OpaqueWrapperBodyParser<'p> {
        OpaqueWrapperBodyParser { inner }
    }
}

impl ConstructParser<Flm> for OpaqueWrapperBodyParser<'_> {
    type Output = EnvironmentBody<Flm>;

    fn parse(
        &mut self,
        cx: &mut ParseContext<'_, '_, Flm>,
    ) -> ConstructParserResult<Flm, (EnvironmentBody<Flm>, Option<Box<ParsingStateDelta<Flm>>>)> {
        let (mut read, after_effect) = cx.parse_construct(&mut *self.inner, None, None)?;
        let wrapper = stage_wrapper(cx, vec![read.body])?;
        let span = covering_span(cx, &[wrapper]);
        read.body = cx
            .stage_node(NodeKind::list(), span.clone(), Arc::clone(&cx.state), vec![wrapper])
            .map_err(|error| staging_error(cx, error, span))?;
        if let ContentNodes::InRegion(range) = &read.content {
            read.content = ContentNodes::InChildrenOf(wrapper, range.clone());
        }
        Ok((read, after_effect))
    }
}

/// Places the argument a reader has just read, `read`, under an opaque
/// wrapper, as [`OpaqueWrapperReader`] does: stages one group of the class
/// [`FlmGroupType::OpaqueWrapper`], with empty delimiters, over the
/// argument's nodes, and returns the argument whose nodes are the wrapper
/// alone, whose content designates the same nodes as `read`'s, and whose
/// extension data is `read`'s. An argument that is already one opaque wrapper
/// is returned as it is.
///
/// A reader that decides from what it read whether the content is kept as
/// written, such as the title reader of the
/// [`theorems`](crate::stdcore::features::theorems) feature, calls it itself.
pub(crate) fn wrap_argument(
    cx: &mut ParseContext<'_, '_, Flm>,
    mut read: ParsedArgumentNodes<Flm>,
) -> ConstructParserResult<Flm, ParsedArgumentNodes<Flm>> {
    if let [only] = read.nodes.as_slice() {
        if is_opaque_wrapper(cx, *only) {
            return Ok(read);
        }
    }
    let wrapper = stage_wrapper(cx, std::mem::take(&mut read.nodes))?;
    if let ContentNodes::InRegion(range) = &read.content {
        read.content = ContentNodes::InChildrenOf(wrapper, range.clone());
    }
    read.nodes = vec![wrapper];
    Ok(read)
}

/// Stages a group of the class [`FlmGroupType::OpaqueWrapper`], with empty
/// delimiters, over `children`, under the current parsing state, and returns
/// it.
fn stage_wrapper(
    cx: &mut ParseContext<'_, '_, Flm>,
    children: Vec<BuildId>,
) -> ConstructParserResult<Flm, BuildId> {
    let span = covering_span(cx, &children);
    let kind = NodeKind::group(GroupData::new(FlmGroupType::OpaqueWrapper, "", ""));
    cx.stage_node(kind, span.clone(), Arc::clone(&cx.state), children)
        .map_err(|error| staging_error(cx, error, span))
}

/// Returns whether the staged node `id` is a group of the class
/// [`FlmGroupType::OpaqueWrapper`].
fn is_opaque_wrapper(cx: &ParseContext<'_, '_, Flm>, id: BuildId) -> bool {
    cx.staged_nodes().get(id).is_some_and(|view| {
        matches!(view.kind(), NodeKind::Group(group) if group.group_type == Some(FlmGroupType::OpaqueWrapper))
    })
}

/// Returns the span from the start of the first of the staged `nodes` to the
/// end of the last, counting only the nodes read from the same source as the
/// first; the current position when there is none.
fn covering_span(cx: &ParseContext<'_, '_, Flm>, nodes: &[BuildId]) -> FlmSpan {
    let staged = cx.staged_nodes();
    let mut spans = nodes.iter().filter_map(|id| staged.get(*id)).map(|view| view.span());
    let Some(anchor) = spans.next() else {
        return cx.here();
    };
    let (mut start, mut end) = (anchor.start(), anchor.end());
    for span in spans.filter(|span| span.same_source(anchor)) {
        start = start.min(span.start());
        end = end.max(span.end());
    }
    SourceSpan::new(anchor.source(), start..end)
}

/// Turns a failure to add a node to the tree into the abort error to report: a
/// failure to build the node extension becomes a `HookFailed` condition, and
/// every other error of the tree builder an implementation error.
fn staging_error(
    cx: &ParseContext<'_, '_, Flm>,
    error: NodeBuildError,
    span: FlmSpan,
) -> ParseError<FlmOrigin> {
    match error {
        NodeBuildError::ExtMintFailed { detail } => {
            ParseError::new(HookFailed::new(detail, None), span)
        }
        other => cx.implementation_error(other, span),
    }
}
