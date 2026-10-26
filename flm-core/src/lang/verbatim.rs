//! The parsers of raw text: verbatim arguments
//! ([`FlmVerbatimArgumentParser`]), verbatim environment bodies
//! ([`FlmVerbatimBehavior`]), and the reader of a [terminated
//! argument](crate::lang::mint::ArgStdKind::Terminated) read as raw text.
//!
//! A verbatim region is a run of raw text in which no markup character means
//! anything, always inside a group of the verbatim class. FLM establishes one
//! rule about raw text while parsing, and the rest of the library relies on it:
//! **every run of verbatim content is a child of a group node of the verbatim
//! class**. Whether content is verbatim is then one question about a node,
//! anywhere in the library. See [verbatim
//! regions](crate::guide::concepts_overview#verbatim-regions).
//!
//! techy's own parser adds a verbatim environment body to the tree as a plain
//! list of characters, and nothing in the tree records that it was verbatim.
//! The two parsers here apply the rule instead:
//!
//! - [`FlmVerbatimArgumentParser`] reads a delimited verbatim argument
//!   (`\verbcode|…|`, `\verbcode{…}`);
//! - [`FlmVerbatimBehavior`] reads a verbatim environment's body, up to its
//!   `\end{name}`;
//! - a [terminated argument](crate::lang::mint::ArgStdKind::Terminated) read
//!   as raw text (such as a code span `` `…` ``) is read by techy's
//!   terminated parser, which stages a group of the verbatim class, under the
//!   raw-text state of the same policy.
//!
//! All three wrap what they read in a group of the verbatim class, and all
//! three parse the interior under the parsing state that the
//! [restricted-interior policy](crate::lang::RestrictedInterior) of the
//! current state derives
//! ([`RestrictedInterior::verbatim_delta`](crate::lang::RestrictedInterior::verbatim_delta),
//! [`RestrictedInterior::raw_text_delta`](crate::lang::RestrictedInterior::raw_text_delta)).
//! Verbatim content that the library creates is wrapped in the same kind of
//! group.
//!
//! A feature declares a verbatim argument with
//! [`ArgStdKind::Verbatim`](crate::lang::mint::ArgStdKind::Verbatim), a
//! terminated argument read as raw text with
//! [`ArgStdKind::Terminated`](crate::lang::mint::ArgStdKind::Terminated), and a
//! verbatim body with [`BodyDef::verbatim`](crate::lang::mint::BodyDef::verbatim);
//! it does not construct these parsers itself.
//!
//! Parsing the interior under that state has a second use. The nodes inside a
//! verbatim group record that state as their own parsing state, which shows,
//! independently of the group's class, that the region was read with the
//! tokenizer's rules switched off. The tree check of the block-structure pass
//! ([`crate::lang::rewrites::blocks::check`]) relies on it.
//!
//! # What the interior recognizes
//!
//! The verbatim argument and body parsers read an interior with one loop over
//! techy's token reader; a raw terminated argument's content is read by techy's
//! ordinary content loop, under the same state, and recognizes the same tokens.
//! In a document parse, the loop reads only characters and the terminator. In the
//! parse of a [user definition](crate::lang::rewrites::userdefs::Definition)'s
//! template, the policy may keep a command syntax, or the recognition of special
//! character sequences, enabled, so that a placeholder written inside raw text
//! is recognized. Such a token is handled exactly as the ordinary content loop
//! handles a command, and its node becomes a child of the verbatim group,
//! beside the runs of raw characters. A command that no feature defines is
//! resolved there by the language's own fallback for unknown macros, as
//! anywhere else: it is reported and becomes a node of `flm.unknown.macro`.
//!
//! # The two restricted argument shapes
//!
//! The language has two argument shapes whose content is not read as FLM
//! content: the **verbatim argument** read here (`\verbcode{…}`, and the body of
//! a verbatim environment), and the **characters-only group** read by
//! [`FlmCharsGroupArgumentParser`](crate::lang::chars_group::FlmCharsGroupArgumentParser)
//! (`\label{…}`, `\ref{…}`).
//!
//! What the two shapes share:
//!
//! - Commands and special character sequences are not recognized, so `\` and
//!   `~` are ordinary characters.
//! - The content is carried over as written by the [block-structure
//!   pass](crate::lang::rewrites::blocks): a verbatim argument's content
//!   stands under its verbatim group, and the reader of a characters-only
//!   group places the group under an opaque wrapper
//!   ([`FlmGroupType::OpaqueWrapper`]).
//! - Both follow the same policy of the parsing state
//!   ([`RestrictedInterior`](crate::lang::RestrictedInterior)). In the parse of
//!   a user definition's template, the placeholder syntax stays enabled inside
//!   them: `\ref{#1}` and `\verbcode{<#1>}` hold a placeholder node, and `##`
//!   is a literal `#`. In a document parse, `#` is an ordinary character.
//!
//! How they differ:
//!
//! - In a **characters-only group**, comments and nested `{…}` groups are
//!   recognized: the comment prefix (`%%` by default) starts a comment that
//!   contributes no character, and a nested group's interior is read as
//!   ordinary content.
//! - In a **verbatim argument**, neither is recognized: `%` is a character, and
//!   nested braces are counted, so that the argument ends at the brace that
//!   balances the opening one, and kept as characters.
//!
//! # Delimiters, the newline after `\begin`, and empty content
//!
//! - When the two delimiters of an argument differ, nested pairs are counted,
//!   so `\verbcode{a{b}c}` reads `a{b}c` whole and ends at the closing brace
//!   that balances the opening one. An environment body ends at
//!   the literal text `\end{name}`, matched byte for byte.
//! - A single newline directly after `\begin{name}` belongs to how the
//!   environment is written, not to its content. It becomes a text node
//!   *outside* the verbatim group, under the surrounding parsing state, so that
//!   it is never mistaken for part of the interior.
//! - Empty verbatim content (`\verbcode||`, an empty verbatim environment body)
//!   produces a verbatim group with no children. No node then records the
//!   interior state, so a check that relies on that state (such as
//!   [`crate::lang::rewrites::blocks::check`]) must accept an empty verbatim
//!   group as consistent.

use std::fmt;
use std::sync::Arc;

use techy::core::constructs::{
    stage_pre_space, ArgumentParser, ConstructParser, ConstructParserResult, EnvironmentBody,
    EnvironmentTerminatorSyntaxData, ExpectedVerbatimDelimiter, Invocation,
    MissingEnvironmentTerminator, MissingTerminatorFound, NameGroup, ParseContext,
    ParsedArgumentNodes, TerminatedArgumentParser, UnterminatedVerbatim,
};
use techy::core::node::{BuildId, ContentNodes, GroupData, NodeBuildError, NodeKind};
use techy::core::specs::{ArgumentSpec, CallableSpec, CommandResolution, ResolvedCallable};
use techy::core::token::{
    CommandOverrides, CommentOverrides, GroupOverrides, GroupRule, ParagraphOverrides,
    SpecialsOverrides, StreamPosition, Token, TokenEdge, TokenKind, TokenRulesOverrides,
};
use techy::core::{Frame, FrameRole, FrameTitle, ParseDriver, ParsingState, ParsingStateDelta};
use techy::error::{HookFailed, ParseError};
use techy::latexlike::{EnvironmentBehavior, EnvironmentInvocation};
use techy::source::{SourceSpan, TextContent};

use crate::lang::flm::{Flm, FlmGroupType};
use crate::lang::origin::{FlmOrigin, FlmSpan};

/// Returns the delimiter pairs a verbatim argument recognizes automatically: an
/// opening character listed here is closed by its partner, and any other
/// character is closed by itself.
fn default_auto_delimiters() -> Vec<(char, char)> {
    vec![('{', '}'), ('[', ']'), ('<', '>'), ('(', ')')]
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

/// What one run of the interior loop read: the children of the verbatim group,
/// in order; where the content ended; the span of the terminator that was
/// consumed (`None` when the region ended without one); and the position just
/// after it.
struct VerbatimRegion {
    children: Vec<BuildId>,
    content_end: StreamPosition<Flm>,
    terminator: Option<FlmSpan>,
    end: StreamPosition<Flm>,
}

/// Adds the characters from `start` to `end` to the tree as a text node under
/// `state`; does nothing when that range is empty.
fn flush_chars(
    cx: &mut ParseContext<'_, '_, Flm>,
    children: &mut Vec<BuildId>,
    start: &StreamPosition<Flm>,
    end: &StreamPosition<Flm>,
    state: &Arc<ParsingState<Flm>>,
) -> ConstructParserResult<Flm, ()> {
    let span = cx.source_span_within(start, end)?;
    if span.is_empty() {
        return Ok(());
    }
    let id = cx
        .stage_node(NodeKind::chars(span.span()), span.clone(), Arc::clone(state), Vec::new())
        .map_err(|error| staging_error(cx, error, span))?;
    children.push(id);
    Ok(())
}

/// Handles one command or special character sequence found inside a verbatim
/// region under the template policy, exactly as the ordinary content loop
/// would: resolves it, consumes its token, runs its specification's parser
/// under the interior state, and adds the resulting node as a child of the
/// verbatim group.
///
/// Any change of parsing state the command would leave behind is dropped, so
/// that no change made inside a verbatim region applies after it.
fn dispatch_invocation(
    cx: &mut ParseContext<'_, '_, Flm>,
    token: &Token<Flm>,
    state: &Arc<ParsingState<Flm>>,
    children: &mut Vec<BuildId>,
) -> ConstructParserResult<Flm, ()> {
    let (callable_type, name, spec): (_, &str, Arc<dyn CallableSpec<Flm>>) =
        match cx.tokens.token_kind(token) {
            TokenKind::Command { name, escape_char } => {
                let resolution = cx.driver.resolve_command(state, token, &*cx.tokens)?;
                match resolution {
                    CommandResolution::Resolved(ResolvedCallable { callable_type, spec }) => {
                        (callable_type, name, spec)
                    }
                    CommandResolution::Failed { detail } => {
                        // An operational resolver failure: a provider errored
                        // while answering. flm's providers never do — every one
                        // of them is minted here and answers from a table — so
                        // this is a caller-contract violation, not document
                        // input.
                        let span = cx.tokens.source_span_of(token);
                        return Err(cx.implementation_error(
                            format!(
                                "looking up the command ‘{escape_char}{name}’ inside a \
                                 verbatim region failed ({})",
                                detail.unwrap_or_else(|| String::from("no detail"))
                            ),
                            span,
                        ));
                    }
                    CommandResolution::Unresolved { .. } => {
                        // Unreachable from document input: the language layer
                        // installs an unknown-macro fallback at the bottom of
                        // the scope stack for every parsing mode, so every
                        // command name resolves. A miss here means a scope
                        // stack this language did not build. See the design
                        // rationale at [§flm-dd-dr:err-unknown-callables].
                        let span = cx.tokens.source_span_of(token);
                        return Err(cx.implementation_error(
                            format!(
                                "the command ‘{escape_char}{name}’ inside a verbatim region \
                                 resolved to no specification, although the language's \
                                 fallback provider resolves every command name"
                            ),
                            span,
                        ));
                    }
                    other => {
                        // `CommandResolution` is non-exhaustive: a variant techy
                        // adds later is an implementation error here until this
                        // dispatch is taught what it means.
                        let span = cx.tokens.source_span_of(token);
                        return Err(cx.implementation_error(
                            format!("unexpected command resolution {other:?} in a verbatim region"),
                            span,
                        ));
                    }
                }
            }
            TokenKind::Specials { callable_type, name, spec } => {
                (callable_type, name, Arc::clone(spec))
            }
            other => {
                let span = cx.tokens.source_span_of(token);
                return Err(cx.implementation_error(
                    format!("unexpected {} token inside a verbatim region", other.as_str()),
                    span,
                ));
            }
        };
    let invocation = Invocation { callable_type, name, spec: &spec, token };
    let frame = Frame {
        title: FrameTitle::Callable {
            spec: Arc::clone(&spec),
            role: FrameRole::Invocation,
            name: cx.tokens.source_span_between(token, TokenEdge::Start, TokenEdge::End),
        },
        span: cx.tokens.source_span_of(token),
    };
    cx.tokens.move_to(token, TokenEdge::EndPastPostSpace);
    let driver = cx.driver;
    let mut parser =
        driver.make_invocation_parser(invocation).map_err(|error| cx.attach_hook_frames(error))?;
    let (id, _after_effect) =
        cx.parse_construct(&mut *parser, Some(Arc::clone(state)), Some(frame))?;
    drop(parser);
    children.push(id);
    Ok(())
}

/// Reads a verbatim region under `state`, the interior state the policy
/// derived, up to its terminator (the closing delimiter it expects), to the end
/// of the input, or to a token it could not read and tolerated.
///
/// `pairing` is the delimiter pair of the delimited form, when the two
/// characters differ: a nested opening delimiter increases a counter and a
/// matching closing delimiter decreases it, so the region ends at the closing
/// delimiter that balances the opening one. Runs of characters become text
/// nodes under `state`; a command or special character sequence, which only the
/// template policy lets through, is handled by [`dispatch_invocation`].
fn read_verbatim_region(
    cx: &mut ParseContext<'_, '_, Flm>,
    state: &Arc<ParsingState<Flm>>,
    pairing: Option<(char, char)>,
) -> ConstructParserResult<Flm, VerbatimRegion> {
    let mut children = Vec::new();
    let mut run_start = cx.tokens.position_here();
    let mut depth = 1usize;
    let (content_end, terminator, end) = loop {
        let Some(token) = cx.probe_token(state)? else {
            // A tolerated unreadable token: the region ends here; the enclosing
            // content loop re-reads and recovers it.
            let here = cx.tokens.position_here();
            break (here, None, here);
        };
        match cx.tokens.token_kind(&token) {
            TokenKind::Char(c) => {
                if let Some((open, _)) = pairing {
                    if c == open {
                        depth += 1;
                    }
                }
                cx.tokens.move_to(&token, TokenEdge::EndPastPostSpace);
            }
            TokenKind::GroupClose { .. } => {
                if pairing.is_some() {
                    depth -= 1;
                    if depth > 0 {
                        // A closer balancing a nested opener: ordinary content.
                        cx.tokens.move_to(&token, TokenEdge::EndPastPostSpace);
                        continue;
                    }
                }
                let content_end = cx.tokens.position_at(&token, TokenEdge::Start);
                let end = cx.tokens.position_at(&token, TokenEdge::EndPastPostSpace);
                let span = cx.tokens.source_span_of(&token);
                cx.tokens.move_to(&token, TokenEdge::EndPastPostSpace);
                break (content_end, Some(span), end);
            }
            TokenKind::EndOfStream => {
                let at = cx.tokens.position_at(&token, TokenEdge::Start);
                break (at, None, at);
            }
            TokenKind::Command { .. } | TokenKind::Specials { .. } => {
                let token_start = cx.tokens.position_at(&token, TokenEdge::StartBeforePreSpace);
                flush_chars(cx, &mut children, &run_start, &token_start, state)?;
                dispatch_invocation(cx, &token, state, &mut children)?;
                run_start = cx.tokens.position_here();
            }
            other => {
                let span = cx.tokens.source_span_of(&token);
                return Err(cx.implementation_error(
                    format!("unexpected {} token inside a verbatim region", other.as_str()),
                    span,
                ));
            }
        }
    };
    flush_chars(cx, &mut children, &run_start, &content_end, state)?;
    Ok(VerbatimRegion { children, content_end, terminator, end })
}

/// Returns the state under which a verbatim region ending at `terminator` is
/// read, as the restricted-interior policy of the current state derives it.
fn interior_state(
    cx: &mut ParseContext<'_, '_, Flm>,
    terminator: Arc<GroupRule<Flm>>,
) -> ConstructParserResult<Flm, Arc<ParsingState<Flm>>> {
    let delta = cx.state.ext().restricted_interior.verbatim_delta(terminator);
    cx.derive_state(&delta)
}

// --- the delimited argument -----------------------------------------------------------

/// The parser of a verbatim argument delimited by a pair of characters
/// (`\verbcode|…|`, `\verbcode{…}`).
///
/// The opening delimiter is the first character after any whitespace. By
/// default, the closing delimiter is its partner in the table of pairs (`{}`,
/// `[]`, `<>`, `()`) or, for any other character, the same character again.
/// [`with_delimiters`](FlmVerbatimArgumentParser::with_delimiters) sets a fixed
/// pair instead; the argument is then present only when that opening character
/// comes next.
///
/// The content is read under the interior state and becomes one group node of
/// the verbatim class, whose children are the argument's content. The group
/// node is added under the surrounding state, its text children under the
/// interior state.
///
/// If the input ends before the closing delimiter, techy's
/// `UnterminatedVerbatim` is reported and the group records an empty closing
/// delimiter. The argument is mandatory: when no opening delimiter follows, the
/// parser reports `ExpectedVerbatimDelimiter` and consumes nothing.
///
/// Declare an argument of this shape with
/// [`ArgStdKind::Verbatim`](crate::lang::mint::ArgStdKind::Verbatim) rather than
/// constructing the parser directly.
pub struct FlmVerbatimArgumentParser {
    delimiters: Option<(char, char)>,
    auto_delimiters: Vec<(char, char)>,
}

impl FlmVerbatimArgumentParser {
    /// Returns a parser that accepts any opening character and closes the
    /// argument with that character's partner (or the character itself).
    pub fn new() -> FlmVerbatimArgumentParser {
        FlmVerbatimArgumentParser { delimiters: None, auto_delimiters: default_auto_delimiters() }
    }

    /// Builder: sets a fixed delimiter pair. The argument is then present only
    /// when `open` is the next character, and it ends at `close`.
    pub fn with_delimiters(mut self, open: char, close: char) -> FlmVerbatimArgumentParser {
        self.delimiters = Some((open, close));
        self
    }

    /// Returns the change of state under which the parser looks for the opening
    /// delimiter: one character after optional whitespace. Every tokenizer rule
    /// except whitespace handling is off, and an inherited expected closing
    /// delimiter is cleared, so that a `}` is read as a delimiter character even
    /// inside a braced group.
    fn probe_delta(&self) -> ParsingStateDelta<Flm> {
        ParsingStateDelta::new().rules(TokenRulesOverrides {
            paragraphs: ParagraphOverrides::disable(),
            groups: GroupOverrides { expecting_close: Some(None), ..GroupOverrides::disable() },
            commands: CommandOverrides::disable(),
            comments: CommentOverrides::disable(),
            specials: SpecialsOverrides::disable(),
            ..TokenRulesOverrides::default()
        })
    }
}

impl Default for FlmVerbatimArgumentParser {
    fn default() -> FlmVerbatimArgumentParser {
        FlmVerbatimArgumentParser::new()
    }
}

impl fmt::Debug for FlmVerbatimArgumentParser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlmVerbatimArgumentParser")
            .field("delimiters", &self.delimiters)
            .field("auto_delimiters", &self.auto_delimiters)
            .finish()
    }
}

impl ArgumentParser<Flm> for FlmVerbatimArgumentParser {
    fn parse_argument(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
        _spec: &ArgumentSpec<Flm>,
    ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
        // The opening delimiter. Every absent exit consumes nothing.
        let probe_state = cx.derive_state(&self.probe_delta())?;
        let Some(token) = cx.probe_token(&probe_state)? else {
            return Ok(None);
        };
        let open = match cx.tokens.token_kind(&token) {
            TokenKind::Char(c) => c,
            _ => {
                let span = cx.tokens.source_span_of(&token);
                cx.recover(
                    ExpectedVerbatimDelimiter::new(self.delimiters.map(|(open, _)| open)),
                    span,
                )?;
                return Ok(None);
            }
        };
        let close = match self.delimiters {
            Some((expected, close)) => {
                if open != expected {
                    let span = cx.tokens.source_span_of(&token);
                    cx.recover(ExpectedVerbatimDelimiter::new(expected), span)?;
                    return Ok(None);
                }
                close
            }
            None => self
                .auto_delimiters
                .iter()
                .find(|(auto_open, _)| *auto_open == open)
                .map(|(_, auto_close)| *auto_close)
                .unwrap_or(open),
        };

        // Committed: the whitespace becomes region noise, the delimiter is consumed.
        let mut nodes = Vec::new();
        stage_pre_space(cx, &mut nodes, &token)?;
        cx.tokens.move_to(&token, TokenEdge::EndPastPostSpace);
        let open_span = cx.tokens.source_span_of(&token);
        let open_start = cx.tokens.position_at(&token, TokenEdge::Start);

        let terminator = Arc::new(GroupRule {
            group_type: FlmGroupType::Verbatim,
            open: String::from(open),
            close: String::from(close),
        });
        let state = interior_state(cx, terminator)?;
        let pairing = (open != close).then_some((open, close));
        let region = read_verbatim_region(cx, &state, pairing)?;
        if region.terminator.is_none() {
            cx.recover(UnterminatedVerbatim::new(String::from(close)), open_span.clone())?;
        }

        // The wrap: the group under the surrounding state.
        let child_count = region.children.len() as u32;
        let group_span = cx.source_span_within(&open_start, &region.end)?;
        let data = GroupData {
            group_type: Some(FlmGroupType::Verbatim),
            open: TextContent::Spanned(open_span.span()),
            close: region
                .terminator
                .as_ref()
                .map(|span| TextContent::Spanned(span.span()))
                .unwrap_or_else(TextContent::empty),
        };
        let state = Arc::clone(&cx.state);
        let group = cx
            .stage_node(NodeKind::group(data), group_span.clone(), state, region.children)
            .map_err(|error| staging_error(cx, error, group_span))?;
        nodes.push(group);
        Ok(Some(ParsedArgumentNodes::new(
            nodes,
            ContentNodes::InChildrenOf(group, 0..child_count),
            (),
        )))
    }

    /// Returns `false`: the argument is mandatory, so an absent one is reported
    /// instead of silently skipped.
    fn can_match_empty(&self) -> bool {
        false
    }
}

// --- the terminated argument read as raw text ----------------------------------------

/// The parser of a [terminated argument](crate::lang::mint::ArgStdKind::Terminated)
/// read as raw text: techy's [`TerminatedArgumentParser`], with a group of
/// the verbatim class, run under the parsing state that the
/// restricted-interior policy of the current state derives for raw text
/// ([`RestrictedInterior::raw_text_delta`](crate::lang::RestrictedInterior::raw_text_delta)).
///
/// techy's parser reads the content under the argument's own state, and adds
/// to that state only the terminator, which it composes from the run of
/// characters that opens the argument at the use site. Running it under the
/// raw-text state reads the content with the tokenizer's rules off (in the
/// parse of a template, with the rules the policy keeps), and the group node
/// it stages, of the verbatim class, holds the characters: the content is a
/// verbatim run like any other.
///
/// The change of state cannot be part of the argument's specification, which
/// is fixed when the language is built: it depends on the policy of the state
/// in which the command is written (a document or a template). It is derived
/// here, at each use, as the two other parsers of this module derive theirs.
#[derive(Debug)]
pub(crate) struct FlmRawTerminatedArgumentParser {
    inner: TerminatedArgumentParser<Flm>,
}

impl FlmRawTerminatedArgumentParser {
    /// Returns the parser that runs `inner` under the raw-text state; `inner`
    /// stages a group of the verbatim class.
    pub(crate) fn new(inner: TerminatedArgumentParser<Flm>) -> FlmRawTerminatedArgumentParser {
        FlmRawTerminatedArgumentParser { inner }
    }
}

impl ArgumentParser<Flm> for FlmRawTerminatedArgumentParser {
    fn parse_argument(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
        spec: &ArgumentSpec<Flm>,
    ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
        let delta = cx.state.ext().restricted_interior.raw_text_delta();
        cx.with_derived_state(&delta, |cx| self.inner.parse_argument(cx, spec))?
    }

    fn can_match_empty(&self) -> bool {
        self.inner.can_match_empty()
    }
}

// --- the environment body -------------------------------------------------------------

/// The behavior of a verbatim environment: it reads the body as raw text up to
/// the literal `\end{name}`, composed from the text of the `\begin` command
/// itself (its escape character and name delimiters) and matched byte for byte.
///
/// The body becomes the standard body list, holding one group node of the
/// verbatim class whose text child is the raw content, under the interior
/// state. A newline directly after `\begin{name}` is placed in the body list
/// *before* that group and outside the content, as the [module
/// documentation](self) describes.
///
/// The behavior holds no data, so one instance serves every environment name.
/// Declare a verbatim body with
/// [`BodyDef::verbatim`](crate::lang::mint::BodyDef::verbatim) rather than
/// installing this directly.
#[derive(Debug, Default)]
pub struct FlmVerbatimBehavior;

impl FlmVerbatimBehavior {
    /// Returns the behavior. It holds no data, so this is the same as
    /// `FlmVerbatimBehavior::default()`.
    pub fn new() -> FlmVerbatimBehavior {
        FlmVerbatimBehavior
    }
}

impl EnvironmentBehavior<Flm> for FlmVerbatimBehavior {
    fn make_body_parser<'p>(
        &'p self,
        invocation: EnvironmentInvocation<'p, Flm>,
    ) -> Box<dyn ConstructParser<Flm, Output = EnvironmentBody<Flm>> + 'p> {
        Box::new(FlmVerbatimBodyParser { invocation })
    }
}

/// The parser to which [`FlmVerbatimBehavior`] passes one environment
/// invocation.
struct FlmVerbatimBodyParser<'p> {
    invocation: EnvironmentInvocation<'p, Flm>,
}

impl FlmVerbatimBodyParser<'_> {
    /// Returns the exact text up to which the body is read: `\end{name}`,
    /// composed from the invocation's own escape character, end-command word,
    /// name delimiters and name.
    fn terminator_text(&self) -> String {
        let invocation = &self.invocation;
        let mut text = String::new();
        text.push(invocation.escape_char);
        text.push_str(invocation.end_command_name);
        text.push_str(invocation.name_group_open);
        text.push_str(invocation.name);
        text.push_str(invocation.name_group_close);
        text
    }

    /// Returns how the consumed terminator was written, in techy's standard
    /// scanned form: the parts the terminator text was composed from,
    /// positioned from the start of the span that matched.
    fn terminator_syntax(
        &self,
        span: FlmSpan,
        end: StreamPosition<Flm>,
    ) -> EnvironmentTerminatorSyntaxData<Flm> {
        let invocation = &self.invocation;
        let piece = |start: usize, len: usize| SourceSpan::new(span.source(), start..start + len);
        let command_word_start = span.start() + invocation.escape_char.len_utf8();
        let command_word_end = command_word_start + invocation.end_command_name.len();
        let name_start = command_word_end + invocation.name_group_open.len();
        EnvironmentTerminatorSyntaxData::Scanned {
            escape_char: invocation.escape_char,
            command_word: piece(command_word_start, invocation.end_command_name.len()),
            post_space: piece(command_word_end, 0),
            name_group: NameGroup::new(
                piece(name_start, invocation.name.len()),
                end,
                Arc::new(GroupRule {
                    group_type: FlmGroupType::Content,
                    open: invocation.name_group_open.into(),
                    close: invocation.name_group_close.into(),
                }),
            ),
        }
    }

    #[allow(clippy::type_complexity)]
    fn parse_body(
        &mut self,
        cx: &mut ParseContext<'_, '_, Flm>,
    ) -> ConstructParserResult<Flm, (EnvironmentBody<Flm>, Option<Box<ParsingStateDelta<Flm>>>)> {
        let body_start = cx.tokens.position_here();
        let close_rule = Arc::new(GroupRule {
            group_type: FlmGroupType::Verbatim,
            open: String::new(),
            close: self.terminator_text(),
        });
        let state = interior_state(cx, close_rule)?;

        // The gobble rule: a newline as the body's very first character is staged
        // but designated out of the content. It is read under the interior state
        // (the token rules that make the body raw) and staged under the
        // *surrounding* state, because it stands outside the verbatim group and
        // is not verbatim content. See the design rationale at
        // [§flm-dd-dr:lang-verbatim-newline].
        let mut children = Vec::new();
        let mut content_start_index = 0u32;
        if let Some(token) = cx.probe_token(&state)? {
            if matches!(cx.tokens.token_kind(&token), TokenKind::Char('\n')) {
                cx.tokens.move_to(&token, TokenEdge::EndPastPostSpace);
                let span = cx.tokens.source_span_of(&token);
                let kind = NodeKind::chars(span.span());
                let outer = Arc::clone(&cx.state);
                let id = cx
                    .stage_node(kind, span.clone(), outer, Vec::new())
                    .map_err(|error| staging_error(cx, error, span))?;
                children.push(id);
                content_start_index = 1;
            }
        }

        let content_start = cx.tokens.position_here();
        let region = read_verbatim_region(cx, &state, None)?;
        if region.terminator.is_none() {
            cx.recover(
                MissingEnvironmentTerminator::new(
                    self.invocation.name,
                    MissingTerminatorFound::EndOfInput,
                ),
                self.invocation.trigger_span.clone(),
            )?;
        }

        // The wrap: the verbatim group under the surrounding state, with empty
        // owned delimiters (the environment's begin/end syntax is recorded on the
        // callable node, not on the group).
        let group_span = cx.source_span_within(&content_start, &region.content_end)?;
        let data = GroupData {
            group_type: Some(FlmGroupType::Verbatim),
            open: TextContent::empty(),
            close: TextContent::empty(),
        };
        let outer = Arc::clone(&cx.state);
        let group = cx
            .stage_node(NodeKind::group(data), group_span.clone(), outer, region.children)
            .map_err(|error| staging_error(cx, error, group_span))?;
        children.push(group);

        let child_count = children.len() as u32;
        let body_span = cx.source_span_within(&body_start, &region.content_end)?;
        let body = cx
            .stage_node(NodeKind::list(), body_span.clone(), Arc::clone(&cx.state), children)
            .map_err(|error| staging_error(cx, error, body_span))?;
        let end = region.end;
        let terminator =
            region.terminator.map(|span| self.terminator_syntax(span, end));
        Ok((
            EnvironmentBody {
                body,
                end,
                content: ContentNodes::InChildrenOf(body, content_start_index..child_count),
                terminator,
                exit_state: Arc::clone(&cx.state),
                after_effects: None,
            },
            None,
        ))
    }
}

impl ConstructParser<Flm> for FlmVerbatimBodyParser<'_> {
    type Output = EnvironmentBody<Flm>;

    fn parse(
        &mut self,
        cx: &mut ParseContext<'_, '_, Flm>,
    ) -> ConstructParserResult<Flm, (EnvironmentBody<Flm>, Option<Box<ParsingStateDelta<Flm>>>)> {
        let frame = Frame {
            title: FrameTitle::Quoted {
                label: "environment",
                name: self.invocation.name_span.clone(),
            },
            span: self.invocation.trigger_span.clone(),
        };
        cx.with_frame(frame, |cx| self.parse_body(cx))
    }
}
