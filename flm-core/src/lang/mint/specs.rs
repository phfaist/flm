//! The FLM specification types, of which every specification a parse can
//! resolve is one, and the element resolution they make possible.
//!
//! A specification tells techy's parser how to parse one command (see the
//! [module documentation](super)). techy resolves a command to a
//! `CallableSpec` trait object, and Rust can recover a concrete type from a
//! trait object only by testing for that type. So a specification can carry
//! an element only if it is one of a **closed list** of concrete types, each
//! holding its element. FLM builds every techy package and specification
//! provider itself ([`super::packages`]), so the list is exactly:
//!
//! - [`FlmCallable`], for a macro or a special character sequence. An inner
//!   specification does the parsing (techy's, built from the declared
//!   arguments, or the language's own for an unknown macro), and the element
//!   is held beside it.
//! - [`FlmEnvironment`], for an environment. Here the element is held by the
//!   *behavior* inside a techy `EnvironmentSpec`, because techy's handling of
//!   `\begin` reaches an environment's body behavior only through that
//!   concrete type; a wrapper around it would lose the body behavior without
//!   any error.
//! - [`FlmSynthesizedSpec`] and [`FlmPresentationSpec`], which are never
//!   parsed: the specifications of the nodes that rewrites and lowering
//!   stages create.
//! - techy's own paragraph-break specification, which the parse driver puts
//!   on the node a blank line leaves behind.
//!
//! A macro that no declaration defines resolves to an [`FlmCallable`] around
//! [`FlmUnknownMacroSpec`], and an environment that no declaration defines to
//! an [`FlmEnvironment`] around [`FlmUnknownEnvironmentBehavior`]. These are
//! the language's own two fallbacks, each reporting the name it could not
//! attribute to any feature; because of them, no specification of techy's own
//! ever reaches a node through a name lookup.
//!
//! [`resolve_element`] tests a node's specification against exactly this list.
//! It takes one test for every type except the environment, whose element is
//! held by the behavior inside techy's `EnvironmentSpec` and so takes two.
//!
//! The module also holds the readers of what a parsed node's specification
//! records: [`arg_layout_of`] and [`body_def_of`] for the block-structure
//! pass, and [`reads_text_in_math`] and [`is_math_text_command`] for the
//! writers of mathematics.

use std::any::Any;
use std::fmt;
use std::sync::Arc;

use techy::core::constructs::{
    ConstructParser, ConstructParserResult, EnvironmentBody, EnvironmentBodyParser, Invocation,
    ParseContext, StdInvocationParser, UnresolvableCommand,
};
use techy::core::node::{
    BuildId, CallableData, NodeKind, NodeRef, NodeSlice, ParsedArguments, ParsedSlots,
};
use techy::core::specs::{ArgumentSpec, CallableSpec, IntoCallableSpec, SpecProvenance};
use techy::core::token::{Token, TokenKind};
use techy::core::{FrameRole, ParsingStateDelta};
use techy::error::ParseError;
use techy::latexlike::{
    EnvironmentBehavior, EnvironmentInvocation, EnvironmentSpec, InvocationSyntaxData, MacroSpec,
    Mode, ParagraphBreakSpec, UnknownEnvironment,
};

use crate::model::element::ElementSpec;
use crate::lang::ext::FlmStateExt;
use crate::lang::flm::{Flm, FlmCallableType, FlmGroupType};
use crate::lang::mint::decl::{ArgLayout, BodyDef};
use crate::lang::mint::parsers::FlmBodyParser;
use crate::lang::origin::FlmOrigin;

/// Returns the title of a traceback frame covering an FLM command, worded as
/// techy's own frames are.
fn frame_title(kind: &str, role: FrameRole, name: &str) -> String {
    match role {
        FrameRole::Argument { index } => format!("argument #{} of ‘{}’", index + 1, name),
        _ => format!("{kind} ‘{name}’"),
    }
}

// --- macros and specials --------------------------------------------------------------

/// The specification FLM registers for every macro and every special character
/// sequence: an inner specification that does the parsing, plus the element the
/// nodes carry.
///
/// Every method of techy's `CallableSpec` is passed to the inner specification
/// (the arguments, whether content is required, the invocation parser, the
/// frame titles), so a specification wrapped here parses exactly as it would
/// unwrapped. Only what the node records changes: the node's specification is
/// this value, which [`resolve_element`] recognizes.
///
/// Besides the element, it records the declared [layout](ArgLayout) of each
/// argument, which the [block-structure pass](crate::lang::rewrites::blocks)
/// reads back through [`arg_layout_of`]. Minting creates these values
/// ([`mint_packages`](super::mint_packages)), and so does the reading of a
/// serialized tree ([`crate::lang::serialize`]). A caller that builds techy
/// packages by hand, outside minting, wraps its specifications with
/// [`FlmCallable::new`] so that their nodes carry an element.
pub struct FlmCallable {
    syntax: Arc<dyn CallableSpec<Flm>>,
    element: Arc<ElementSpec>,
    arg_layouts: Vec<ArgLayout>,
    provenance: Option<SpecProvenance<Flm>>,
}

impl FlmCallable {
    /// Wraps `syntax` so that the nodes it produces carry `element`.
    ///
    /// `M` is techy's marker type for converting a specification given by
    /// value or already shared; a caller never writes it.
    ///
    /// Every argument is laid out inline until
    /// [`with_arg_layouts`](FlmCallable::with_arg_layouts) sets the layouts.
    pub fn new<M>(syntax: impl IntoCallableSpec<Flm, M>, element: Arc<ElementSpec>) -> FlmCallable {
        FlmCallable {
            syntax: syntax.into_callable_spec(),
            element,
            arg_layouts: Vec::new(),
            provenance: None,
        }
    }

    /// Builder: sets the declared layout of each argument, in argument order.
    /// An argument past the end of the list is inline.
    pub fn with_arg_layouts(mut self, layouts: Vec<ArgLayout>) -> FlmCallable {
        self.arg_layouts = layouts;
        self
    }

    /// Builder: sets the provenance stamp ([`SpecProvenance`]) that the techy
    /// package this specification is inserted into issued for it. The stamp
    /// records that package and the key the specification is held under, so a
    /// serialized tree ([`crate::lang::serialize`]) refers to the
    /// specification by that key and finds the same value again when it is
    /// read back.
    pub fn with_provenance(mut self, provenance: SpecProvenance<Flm>) -> FlmCallable {
        self.provenance = Some(provenance);
        self
    }

    /// Returns the inner specification that does the parsing.
    pub fn syntax(&self) -> &Arc<dyn CallableSpec<Flm>> {
        &self.syntax
    }

    /// Returns the element every node of this specification carries.
    pub fn element(&self) -> &Arc<ElementSpec> {
        &self.element
    }

    /// Returns the declared layout of each argument, in argument order; an
    /// argument past the end is inline. [`arg_layout_of`] reads it off a
    /// parsed node.
    pub fn arg_layouts(&self) -> &[ArgLayout] {
        &self.arg_layouts
    }
}

impl fmt::Debug for FlmCallable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlmCallable")
            .field("element", &self.element.name)
            .field("syntax", &self.syntax)
            .finish()
    }
}

impl CallableSpec<Flm> for FlmCallable {
    fn arguments(&self) -> &[Arc<ArgumentSpec<Flm>>] {
        self.syntax.arguments()
    }

    fn requires_content(&self) -> bool {
        self.syntax.requires_content()
    }

    fn make_invocation_parser<'a>(
        &'a self,
        invocation: Invocation<'a, Flm>,
    ) -> Result<Box<dyn ConstructParser<Flm, Output = BuildId> + 'a>, ParseError<FlmOrigin>> {
        self.syntax.make_invocation_parser(invocation)
    }

    fn stack_frame_title(&self, role: FrameRole, name: &str) -> String {
        self.syntax.stack_frame_title(role, name)
    }

    fn provenance(&self) -> Option<&SpecProvenance<Flm>> {
        self.provenance.as_ref()
    }
}

// --- environments ---------------------------------------------------------------------

/// The behavior FLM registers for every environment, wrapped in a techy
/// `EnvironmentSpec` by [`FlmEnvironment::into_spec`].
///
/// It holds the declared arguments; the change of parsing state the body is
/// read under, when the declaration asks for a mode or scoped packages; the
/// element the nodes carry; the declared layout of each argument; and the body
/// declaration, whose [parser](BodyDef::parser), when it declares one, reads
/// the body (a verbatim body is read this way). The [block-structure
/// pass](crate::lang::rewrites::blocks) reads the layouts and the body declaration
/// back through [`arg_layout_of`] and [`body_def_of`].
///
/// The element is held here, by the behavior, rather than by a specification
/// of its own: see the [module documentation](self) for the reason.
pub struct FlmEnvironment {
    arguments: Vec<Arc<ArgumentSpec<Flm>>>,
    arg_layouts: Vec<ArgLayout>,
    body_delta: Option<ParsingStateDelta<Flm>>,
    element: Arc<ElementSpec>,
    body: BodyDef,
}

impl FlmEnvironment {
    /// Creates an environment behavior with the given arguments, element and
    /// body declaration, no change of parsing state for the body, and every
    /// argument laid out inline until
    /// [`with_arg_layouts`](FlmEnvironment::with_arg_layouts) sets the
    /// layouts. The body is read by the body declaration's parser, or the
    /// ordinary way when it declares none.
    pub fn new(
        arguments: Vec<Arc<ArgumentSpec<Flm>>>,
        element: Arc<ElementSpec>,
        body: BodyDef,
    ) -> FlmEnvironment {
        FlmEnvironment {
            arguments,
            arg_layouts: Vec::new(),
            body_delta: None,
            element,
            body,
        }
    }

    /// Builder: sets the declared layout of each argument, in argument order.
    /// An argument past the end of the list is inline.
    pub fn with_arg_layouts(mut self, layouts: Vec<ArgLayout>) -> FlmEnvironment {
        self.arg_layouts = layouts;
        self
    }

    /// Builder: sets the change of parsing state the body is read under.
    pub fn with_body_delta(mut self, delta: ParsingStateDelta<Flm>) -> FlmEnvironment {
        self.body_delta = Some(delta);
        self
    }

    /// Wraps this behavior in a techy `EnvironmentSpec`: the value that is
    /// registered in a techy package, and the only type through which techy's
    /// handling of `\begin` reaches a body behavior.
    pub fn into_spec(self) -> EnvironmentSpec<Flm> {
        EnvironmentSpec::from_behavior(Arc::new(self))
    }

    /// Returns the element every node of this environment carries.
    ///
    /// [`element_of_spec`] finds it from the registered specification.
    pub fn element(&self) -> &Arc<ElementSpec> {
        &self.element
    }

    /// Returns the body declaration.
    pub fn body(&self) -> &BodyDef {
        &self.body
    }

    /// Returns the declared layout of each argument, in argument order; an
    /// argument past the end is inline. [`arg_layout_of`] reads it off a
    /// parsed node.
    pub fn arg_layouts(&self) -> &[ArgLayout] {
        &self.arg_layouts
    }
}

impl fmt::Debug for FlmEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlmEnvironment")
            .field("element", &self.element.name)
            .field("arguments", &self.arguments.len())
            .field("body", &self.body)
            .finish_non_exhaustive()
    }
}

impl EnvironmentBehavior<Flm> for FlmEnvironment {
    /// The declared arguments.
    fn arguments(&self) -> &[Arc<ArgumentSpec<Flm>>] {
        &self.arguments
    }

    /// Returns the change of parsing state the body declaration asks for, if
    /// any, and never the body parser's own: the declaration's mode and
    /// scoped packages apply around any parser that reads the body.
    fn body_state_delta(
        &self,
        _invocation: EnvironmentInvocation<'_, Flm>,
    ) -> Result<Option<ParsingStateDelta<Flm>>, ParseError<FlmOrigin>> {
        Ok(self.body_delta.clone())
    }

    /// Returns the body parser of the body declaration's
    /// [parser](BodyDef::parser), or else the ordinary body parser.
    fn make_body_parser<'p>(
        &'p self,
        invocation: EnvironmentInvocation<'p, Flm>,
    ) -> Box<dyn ConstructParser<Flm, Output = EnvironmentBody<Flm>> + 'p> {
        match &self.body.parser {
            Some(parser) => parser.make_body_parser(invocation),
            // The preset's default body handling, as its documentation constructs
            // it: content up to and including the rigid `\end{name}` terminator.
            None => Box::new(
                EnvironmentBodyParser::new(
                    invocation.trigger_span.clone(),
                    invocation.name,
                    invocation.end_command_name,
                    FlmGroupType::Content,
                )
                .with_invocation_name_span(invocation.name_span),
            ),
        }
    }
}

/// Returns whether `node` is a construct of mathematics that reads a content
/// region of its own (an argument's content, or the content of a node slot:
/// techy's `ParsedSlot`, a part of a node's content outside its arguments, such
/// as an environment's body) in **text** mode: the form of a text command such
/// as `\text{…}`, whose argument is document content although it is written
/// inside a formula.
///
/// The node itself must be in math mode, so a text-mode node inside a
/// verbatim region is never counted.
///
/// The result is read off the region's own nodes, so a region with no nodes
/// at all (`\text{}` written inside a formula) returns `false` and is carried
/// over as written; an empty region holds nothing to lay out.
///
/// The [block-structure pass](crate::lang::rewrites::blocks) uses it to find
/// the parts of a formula that hold document content, which it lays out like
/// any other content. A declaration asks for text mode with
/// [`ArgDef::mode`](super::ArgDef::mode) or
/// [`BodyDef::mode`](super::BodyDef::mode); this function reads the result
/// back off a parsed node. [`is_math_text_command`] adds the exclusion of
/// fields; the writers of mathematics use that function.
pub fn reads_text_in_math(node: &NodeRef<'_, Flm>) -> bool {
    /// Returns whether a region's content was parsed in text mode. A run's
    /// nodes share their region's mode, so the first node decides; an empty
    /// run returns `false`.
    fn is_text(content: Option<NodeSlice<'_, Flm>>) -> bool {
        content
            .and_then(|slice| slice.iter().next())
            .is_some_and(|node| node.parsing_state().mode() == Mode::Text)
    }
    if node.parsing_state().mode() != Mode::Math {
        return false;
    }
    let Some(callable) = node.callable() else {
        return false;
    };
    (0..callable.arguments.len()).any(|i| is_text(node.argument_content_nodes(i)))
        || (0..callable.slots.len()).any(|i| is_text(node.slot_content_nodes(i)))
}

/// Returns whether `node` is a **text command of mathematics**: a construct
/// that reads a region of its own in text mode ([`reads_text_in_math`]) and is
/// content where it is written, such as `\text{…}` and the other text commands
/// of math mode.
///
/// A [field](super::FIELD) is not one, although it may read a region of its own
/// in text mode: a field is data of the construct that holds it, not content
/// at the place where it is written. `\tag{…}` reads its text in text mode,
/// and the equation line it belongs to reads the field and renders that text;
/// a writer of mathematics that reaches a field writes it back as it was
/// typed.
///
/// A writer of mathematics tests each node it reaches with this function: the
/// HTML rules for mathematics and the recomposition of mathematics both use it
/// to find what, inside a formula, is document content.
pub fn is_math_text_command(node: &NodeRef<'_, Flm>) -> bool {
    reads_text_in_math(node) && super::field_name(node).is_none()
}

/// Returns the body declaration of `node`'s environment, or `None` when `node`
/// is not a node of an FLM environment.
///
/// The [block-structure pass](crate::lang::rewrites::blocks) reads it to
/// decide how to lay out an environment's body. The declaration is recorded by
/// the node's specification, so the result is the same on a tree read back from
/// serialized form.
pub fn body_def_of<'t>(node: &NodeRef<'t, Flm>) -> Option<&'t BodyDef> {
    let spec = node.spec()?;
    let environment = (&**spec as &dyn Any).downcast_ref::<EnvironmentSpec<Flm>>()?;
    let behavior = (environment.behavior() as &dyn Any).downcast_ref::<FlmEnvironment>()?;
    Some(&behavior.body)
}

/// Returns the declared [layout](ArgLayout) of `node`'s argument at position
/// `index` (counting from zero, in the order the arguments are written), or
/// `None` when `node` is not an FLM macro, special character sequence or
/// environment, or has no argument at that position.
///
/// An argument for which the declaration gave no layout is
/// [`ArgLayout::Inline`]. The node's specification carries the layout, so the
/// result is the same on a tree read back from serialized form.
///
/// This is the counterpart of [`body_def_of`] for arguments: the
/// [block-structure pass](crate::lang::rewrites::blocks) reads it to decide how to
/// lay out each argument's content.
pub fn arg_layout_of(node: &NodeRef<'_, Flm>, index: usize) -> Option<ArgLayout> {
    let spec = node.spec()?;
    let any = &**spec as &dyn Any;
    let layouts = if let Some(callable) = any.downcast_ref::<FlmCallable>() {
        &callable.arg_layouts
    } else {
        let environment = any.downcast_ref::<EnvironmentSpec<Flm>>()?;
        let behavior = (environment.behavior() as &dyn Any).downcast_ref::<FlmEnvironment>()?;
        &behavior.arg_layouts
    };
    if index >= node.arguments()?.len() {
        return None;
    }
    Some(layouts.get(index).copied().unwrap_or_default())
}

// --- unknown-callable recovery -----------------------------------------------------------

/// The specification of the language's own macro fallback: what `\name`
/// resolves to when no declaration defines `name`.
///
/// Minting installs it for every parsing mode, consulted after every fallback
/// a feature declares, wrapped in an [`FlmCallable`] carrying the element
/// [`UNKNOWN_MACRO`](crate::lang::structural::UNKNOWN_MACRO). Because it is
/// always installed, every command name resolves to a specification of FLM's
/// own types, and every node has an element.
///
/// Parsing one of these does two things. First it reports techy's
/// `UnresolvableCommand` as an Error diagnostic at the command token, in every
/// mode where no feature's fallback applies. Then it runs the ordinary parse
/// of a macro taking **no argument**: the node records the escape character
/// and the name as written, and a backend writes those back. Whitespace
/// after the name is handled as for any other macro with no argument.
#[derive(Debug, Default)]
pub struct FlmUnknownMacroSpec {
    syntax: MacroSpec<Flm>,
}

impl FlmUnknownMacroSpec {
    /// Creates the specification. The same as
    /// `FlmUnknownMacroSpec::default()`.
    pub fn new() -> FlmUnknownMacroSpec {
        FlmUnknownMacroSpec { syntax: MacroSpec::new(Vec::new()) }
    }
}

impl CallableSpec<Flm> for FlmUnknownMacroSpec {
    fn arguments(&self) -> &[Arc<ArgumentSpec<Flm>>] {
        self.syntax.arguments()
    }

    fn make_invocation_parser<'a>(
        &'a self,
        invocation: Invocation<'a, Flm>,
    ) -> Result<Box<dyn ConstructParser<Flm, Output = BuildId> + 'a>, ParseError<FlmOrigin>> {
        Ok(Box::new(UnknownMacroInvocationParser {
            token: invocation.token,
            name: invocation.name,
            inner: StdInvocationParser::new(invocation),
        }))
    }

    fn stack_frame_title(&self, role: FrameRole, name: &str) -> String {
        frame_title("unknown macro", role, name)
    }
}

/// The invocation parser of [`FlmUnknownMacroSpec`]: it reports the
/// unresolvable command, then runs the ordinary parse of a macro with no
/// argument.
struct UnknownMacroInvocationParser<'p> {
    token: &'p Token<Flm>,
    name: &'p str,
    inner: StdInvocationParser<'p, Flm>,
}

impl ConstructParser<Flm> for UnknownMacroInvocationParser<'_> {
    type Output = BuildId;

    fn parse(
        &mut self,
        cx: &mut ParseContext<'_, '_, Flm>,
    ) -> ConstructParserResult<Flm, (BuildId, Option<Box<ParsingStateDelta<Flm>>>)> {
        let span = cx.tokens.source_span_of(self.token);
        // Only a command token reaches a macro fallback: the gate answers
        // `FlmCallableType::Macro` queries, and only the command dispatch raises
        // those (a specials trigger carries its own spec on the token).
        let TokenKind::Command { escape_char, .. } = cx.tokens.token_kind(self.token) else {
            return Err(cx.implementation_error(
                "the fallback for unknown macros was invoked for a token that is not a command",
                span,
            ));
        };
        cx.recover(UnresolvableCommand::new(self.name, escape_char, None), span.clone())?;
        self.inner.parse(cx)
    }
}

/// The body behavior of the language's own environment fallback: what
/// `\begin{name}…\end{name}` resolves to when no declaration defines `name`.
///
/// Minting installs it for every parsing mode, consulted after every fallback
/// a feature declares, as the [body parser](BodyDef::parser) of an [`FlmEnvironment`]
/// carrying the element
/// [`UNKNOWN_ENVIRONMENT`](crate::lang::structural::UNKNOWN_ENVIRONMENT). It is
/// the environment counterpart of [`FlmUnknownMacroSpec`].
///
/// Parsing one reports techy's `UnknownEnvironment` as an Error diagnostic at
/// the name, in every mode where no feature's fallback applies, and then
/// parses the body as usual up to `\end{name}`, so the content is kept under
/// the node. The body is declared as a region of blocks
/// ([`BodyLayout::Blocks`](super::BodyLayout::Blocks)), so a blank line inside
/// it becomes a paragraph break rather than a second problem.
#[derive(Debug, Default)]
pub struct FlmUnknownEnvironmentBehavior;

impl FlmUnknownEnvironmentBehavior {
    /// Creates the behavior. It holds no data, so this is the same as
    /// `FlmUnknownEnvironmentBehavior::default()`.
    pub fn new() -> FlmUnknownEnvironmentBehavior {
        FlmUnknownEnvironmentBehavior
    }
}

impl FlmBodyParser for FlmUnknownEnvironmentBehavior {
    /// The body is parsed as usual: it is content.
    fn holds_content(&self) -> bool {
        true
    }

    fn identity(&self) -> Vec<u8> {
        b"flm.unknown.environment".to_vec()
    }

    fn description(&self) -> String {
        String::from("content up to `\\end{name}` of an environment that no feature defines")
    }
}

impl EnvironmentBehavior<Flm> for FlmUnknownEnvironmentBehavior {
    fn make_body_parser<'p>(
        &'p self,
        invocation: EnvironmentInvocation<'p, Flm>,
    ) -> Box<dyn ConstructParser<Flm, Output = EnvironmentBody<Flm>> + 'p> {
        Box::new(UnknownEnvironmentBodyParser { invocation })
    }
}

/// The body parser of [`FlmUnknownEnvironmentBehavior`]: it reports the
/// unknown environment, then runs techy's ordinary body parse.
struct UnknownEnvironmentBodyParser<'p> {
    invocation: EnvironmentInvocation<'p, Flm>,
}

impl ConstructParser<Flm> for UnknownEnvironmentBodyParser<'_> {
    type Output = EnvironmentBody<Flm>;

    fn parse(
        &mut self,
        cx: &mut ParseContext<'_, '_, Flm>,
    ) -> ConstructParserResult<Flm, (EnvironmentBody<Flm>, Option<Box<ParsingStateDelta<Flm>>>)> {
        let invocation = &self.invocation;
        cx.recover(UnknownEnvironment::new(invocation.name), invocation.name_span.clone())?;
        let mut body = EnvironmentBodyParser::new(
            invocation.trigger_span.clone(),
            invocation.name,
            invocation.end_command_name,
            FlmGroupType::Content,
        )
        .with_invocation_name_span(invocation.name_span.clone());
        body.parse(cx)
    }
}

// --- never-parsed specs ----------------------------------------------------------------

/// The specification of a node of an element declared with no syntax
/// ([`SyntaxDef::Synthesized`](super::SyntaxDef::Synthesized)): an element that
/// nothing in the source produces and only a rewrite creates.
///
/// It belongs to no techy package, so no parse resolves it; a rewrite stages
/// (builds into the tree it produces) such a node with
/// [`FlmSynthesizedSpec::callable_data`].
///
/// Such a node is recorded as a special character sequence, with an empty
/// record of how it was written, and named after its element, as the
/// paragraph-break node is: neither was written in the source, so there is
/// nothing to record. Recognize such a node by its element, never by its
/// recorded form.
pub struct FlmSynthesizedSpec {
    element: Arc<ElementSpec>,
}

impl FlmSynthesizedSpec {
    /// Creates the specification of nodes carrying `element`.
    pub fn new(element: Arc<ElementSpec>) -> FlmSynthesizedSpec {
        FlmSynthesizedSpec { element }
    }

    /// Returns the element every node of this specification carries.
    pub fn element(&self) -> &Arc<ElementSpec> {
        &self.element
    }

    /// Returns the node data for staging a node of this specification, with
    /// no arguments and no node slots (techy's `ParsedSlot`, a part of a node's
    /// content outside its arguments, such as an environment's body). A rewrite
    /// that puts content under such a node builds the slot records itself.
    pub fn callable_data(self: &Arc<Self>) -> CallableData<Flm> {
        let spec: Arc<dyn CallableSpec<Flm>> = self.clone();
        CallableData {
            callable_type: FlmCallableType::Specials,
            name: self.element.name.as_str().into(),
            spec,
            arguments: ParsedArguments::empty(),
            slots: ParsedSlots::empty(),
            invocation_syntax: InvocationSyntaxData::Specials,
        }
    }
}

impl fmt::Debug for FlmSynthesizedSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlmSynthesizedSpec").field("element", &self.element.name).finish()
    }
}

impl CallableSpec<Flm> for FlmSynthesizedSpec {
    fn stack_frame_title(&self, role: FrameRole, name: &str) -> String {
        frame_title("synthesized element", role, name)
    }
}

/// The specification of a node of a presentation tree, the tree that lowering
/// builds for one backend from a frozen document tree.
///
/// It is never parsed. A lowering stage stages such a node with
/// [`FlmPresentationSpec::callable_data`] and puts the node's typed payload in
/// its extension ([`PresPayload`](crate::lang::PresPayload)).
pub struct FlmPresentationSpec {
    element: Arc<ElementSpec>,
}

impl FlmPresentationSpec {
    /// Creates the specification of nodes carrying the presentation element
    /// `element`.
    pub fn new(element: Arc<ElementSpec>) -> FlmPresentationSpec {
        FlmPresentationSpec { element }
    }

    /// Returns the presentation element every node of this specification
    /// carries.
    pub fn element(&self) -> &Arc<ElementSpec> {
        &self.element
    }

    /// Returns the node data for staging a presentation node with the given
    /// argument records and node slot records (a node slot, techy's
    /// `ParsedSlot`, is a part of a node's content outside its arguments, such
    /// as an environment's body).
    ///
    /// Those records must cover the node's children exactly, in order, with no
    /// gap and no overlap; techy requires this of every command node.
    pub fn callable_data(
        self: &Arc<Self>,
        arguments: ParsedArguments<Flm>,
        slots: ParsedSlots<Flm>,
    ) -> CallableData<Flm> {
        let spec: Arc<dyn CallableSpec<Flm>> = self.clone();
        CallableData {
            callable_type: FlmCallableType::Presentation,
            name: self.element.name.as_str().into(),
            spec,
            arguments,
            slots,
            invocation_syntax: InvocationSyntaxData::Specials,
        }
    }
}

impl fmt::Debug for FlmPresentationSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlmPresentationSpec").field("element", &self.element.name).finish()
    }
}

impl CallableSpec<Flm> for FlmPresentationSpec {
    fn stack_frame_title(&self, role: FrameRole, name: &str) -> String {
        frame_title("presentation element", role, name)
    }
}

// --- resolution ---------------------------------------------------------------------

/// Returns the element `spec` carries, found by testing it against the closed
/// list of FLM's specification types: one test for each of them, and two for
/// an environment, whose element is held by the behavior inside techy's
/// `EnvironmentSpec`.
///
/// Returns `None` when `spec` is none of these types, which means that FLM's
/// minting did not build it.
pub fn element_of_spec(spec: &dyn CallableSpec<Flm>) -> Option<&Arc<ElementSpec>> {
    let any = spec as &dyn Any;
    if let Some(callable) = any.downcast_ref::<FlmCallable>() {
        return Some(&callable.element);
    }
    if let Some(environment) = any.downcast_ref::<EnvironmentSpec<Flm>>() {
        return (environment.behavior() as &dyn Any)
            .downcast_ref::<FlmEnvironment>()
            .map(|behavior| &behavior.element);
    }
    if let Some(synthesized) = any.downcast_ref::<FlmSynthesizedSpec>() {
        return Some(&synthesized.element);
    }
    if let Some(presentation) = any.downcast_ref::<FlmPresentationSpec>() {
        return Some(&presentation.element);
    }
    None
}

/// The only error [`resolve_element`] returns: a command node was staged with
/// a specification that is none of FLM's types and none of the two of techy's
/// that the language accepts (the paragraph break, and a placeholder).
///
/// This is a fault of the program, never a consequence of document input.
/// Minting builds every specification provider on a parse's scope stack, the
/// language's own two fallbacks included, so a parse resolves nothing else;
/// only a caller that builds a tree by hand with a specification of its own
/// can cause it.
///
/// When the FLM language ([`Flm`]) builds a node's extension data
/// ([`FlmNodeExt`](crate::lang::FlmNodeExt)), it turns this error into techy's
/// `NodeBuildError::ExtMintFailed`, the error techy provides for this case,
/// instead of panicking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmintedSpec {
    /// The name of the callable whose specification could not be recognized.
    pub name: String,
}

impl fmt::Display for UnmintedSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the specification of the command ‘{}’ must be one of the FLM \
             specification types, which carry an element",
            self.name
        )
    }
}

/// Returns the element a node of kind `kind` receives. This is the only place
/// where a node is given its element; [`Flm`] calls it for every node when it
/// builds the node's extension data ([`FlmNodeExt`](crate::lang::FlmNodeExt)).
///
/// It covers every node shape a parse produces:
///
/// - text, comments, containers and groups of each group class receive the
///   matching [structural
///   element](crate::lang::structural::StructuralElements) from `ext`, the
///   state extension of the node's parsing state;
/// - a command receives the element its specification carries
///   ([`element_of_spec`]);
/// - a placeholder receives the placeholder element, and the paragraph-break
///   node the parse driver creates receives `flm.parbreak`;
/// - a macro or an environment that no declaration defines receives
///   `flm.unknown.macro` or `flm.unknown.environment`, through the language's
///   own fallback specifications ([`FlmUnknownMacroSpec`],
///   [`FlmUnknownEnvironmentBehavior`]).
///
/// # Errors
///
/// [`UnmintedSpec`] for a command node whose specification is none of FLM's
/// types, which no document input can produce; see that type.
pub fn resolve_element(
    kind: &NodeKind<Flm>,
    ext: &FlmStateExt,
) -> Result<Arc<ElementSpec>, UnmintedSpec> {
    let structural = &ext.structural;
    let element = match kind {
        NodeKind::Chars { .. } => &structural.text,
        NodeKind::Comment(_) => &structural.comment,
        NodeKind::List => &structural.container,
        NodeKind::Group(group) => match group.group_type {
            Some(FlmGroupType::Math(_)) => &structural.math,
            Some(FlmGroupType::Verbatim) => &structural.verbatim,
            Some(FlmGroupType::Paragraph) => &structural.paragraph,
            Some(FlmGroupType::Content) | None => &structural.group,
        },
        NodeKind::Callable(data) => {
            if data.callable_type == FlmCallableType::Placeholder {
                &structural.placeholder
            } else if let Some(element) = element_of_spec(&*data.spec) {
                element
            } else if (&*data.spec as &dyn Any).downcast_ref::<ParagraphBreakSpec>().is_some() {
                &structural.parbreak
            } else {
                return Err(UnmintedSpec { name: data.name.to_string() });
            }
        }
    };
    Ok(Arc::clone(element))
}
