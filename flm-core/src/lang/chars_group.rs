//! The parser of a characters-only group, the argument shape of `\label{…}`
//! and `\ref{…}`: [`FlmCharsGroupArgumentParser`].
//!
//! A **characters-only group** is a mandatory `{…}` argument whose content is
//! read as plain characters: a backslash or a tilde is an ordinary character
//! inside it, so `\label{sec:a~b}` keeps every character it was written with.
//! It is one of the language's two restricted argument shapes; the other is the
//! verbatim argument. The [module documentation of
//! `lang::verbatim`](crate::lang::verbatim#the-two-restricted-argument-shapes)
//! states what the two shapes share and how they differ.
//!
//! A feature declares an argument of this shape with
//! [`ArgStdKind::CharsGroup`](crate::lang::mint::ArgStdKind::CharsGroup); it does
//! not construct the parser itself.
//!
//! # What the interior recognizes
//!
//! The interior is read under the parsing state that the [restricted-interior
//! policy](crate::lang::RestrictedInterior) of the current state derives
//! ([`RestrictedInterior::chars_group_delta`](crate::lang::RestrictedInterior::chars_group_delta)):
//!
//! - Commands and special character sequences are not recognized.
//! - Comments are recognized: the comment prefix of the parsing settings (`%%`
//!   by default) starts a comment, which contributes no character.
//! - Nested `{…}` groups are recognized, and a nested group's interior is read
//!   under the state the argument itself was parsed under, with every command
//!   available again: `\cite{manual:{a \emph{b}}}` reads `\emph{b}` as a
//!   command. The delimiters of mathematics, which belong to another group
//!   class, are ordinary characters at the first level.
//! - The forbidden characters of the parsing settings are still refused and
//!   reported (by default `$`, and a lone `%`; see
//!   [`FlmParsing::token_rules_overrides`](crate::lang::FlmParsing::token_rules_overrides)).
//! - Under the [`Template`](crate::lang::RestrictedInterior::Template) policy of
//!   a template parse, the command syntax of a placeholder stays enabled, so a
//!   placeholder (`\ref{#1}`, `\label{sec:#{name}}`) is a node of the
//!   argument's content. In a document parse, `#` is an ordinary character.
//!
//! [`read_chars`](crate::lang::text::read_chars) reads the content back as
//! characters, and returns a placeholder as a segment of its own.

use std::fmt;
use std::sync::Arc;

use techy::core::constructs::{
    scan_argument_noise, stage_pre_space, ArgumentParser, ChildStateSpec, ConstructParserResult,
    GroupChildState, InvocationChildState, MissingMandatoryArgument, ParseContext,
    ParsedArgumentNodes,
};
use techy::core::node::ContentNodes;
use techy::core::specs::ArgumentSpec;
use techy::core::token::{TokenEdge, TokenKind};

use crate::lang::flm::{Flm, FlmGroupType};

/// The parser of a characters-only group: a `{…}` group of the content class,
/// as in `\label{sec:intro}`. The [module documentation](self) states what its
/// interior recognizes.
///
/// Whitespace and comments before the `{` are read under the argument's own
/// state; the restriction applies to the group's interior only. The group's
/// children are the argument's content, and its delimiters are the argument's
/// syntax.
///
/// The argument is mandatory and has no single-token form (a character without
/// braces is not accepted): where no `{` follows, techy's
/// `MissingMandatoryArgument` is reported (under tolerant recovery) and the
/// argument is absent, with nothing consumed.
///
/// Declare an argument of this shape with
/// [`ArgStdKind::CharsGroup`](crate::lang::mint::ArgStdKind::CharsGroup).
#[derive(Default)]
pub struct FlmCharsGroupArgumentParser;

impl FlmCharsGroupArgumentParser {
    /// Returns the parser. It holds no data, so this is the same as
    /// `FlmCharsGroupArgumentParser::default()`.
    pub fn new() -> FlmCharsGroupArgumentParser {
        FlmCharsGroupArgumentParser
    }
}

impl fmt::Debug for FlmCharsGroupArgumentParser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlmCharsGroupArgumentParser").finish()
    }
}

impl ArgumentParser<Flm> for FlmCharsGroupArgumentParser {
    fn parse_argument(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
        spec: &ArgumentSpec<Flm>,
    ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
        let mut noise = scan_argument_noise(cx)?;
        let open = noise.next.clone().and_then(|next| match cx.tokens.token_kind(&next) {
            TokenKind::GroupOpen { rule, .. } if rule.group_type == FlmGroupType::Content => {
                let rule = Arc::clone(rule);
                Some((next, rule))
            }
            _ => None,
        });
        let Some((open, rule)) = open else {
            let at = match noise.next.as_ref() {
                Some(token) => cx.tokens.source_span_of(token),
                None => cx.here(),
            };
            cx.recover(MissingMandatoryArgument::new(spec.name.as_deref().map(String::from)), at)?;
            noise.rewind(cx);
            return Ok(None);
        };

        // The argument's own state is kept: a nested group's interior is read
        // under it, and the policy derives the restricted interior from it.
        let outer = Arc::clone(&cx.state);
        let delta = outer.ext().restricted_interior.chars_group_delta(&outer, FlmGroupType::Content);
        let contents_state = cx.derive_state(&delta)?;

        stage_pre_space(cx, &mut noise.nodes, &open)?;
        cx.tokens.move_to(&open, TokenEdge::EndPastPostSpace);
        let child_states = ChildStateSpec {
            group: GroupChildState::Fixed(outer),
            invocation: InvocationChildState::Inherit,
        };
        let (id, _after_effect) = cx.parse_group(contents_state, &open, rule, child_states, None)?;
        // A group this parse staged always reads back; an id that does not is
        // a driver's fault, which the tree builder reports when the region is
        // staged, so the count degrades to zero here rather than panicking.
        let child_count =
            cx.staged_nodes().get(id).map_or(0, |view| view.children().len() as u32);
        noise.nodes.push(id);
        Ok(Some(ParsedArgumentNodes::new(
            noise.nodes,
            ContentNodes::InChildrenOf(id, 0..child_count),
            (),
        )))
    }

    /// Returns `false`: the argument is mandatory, so an absent one is reported
    /// instead of silently skipped.
    fn can_match_empty(&self) -> bool {
        false
    }
}
