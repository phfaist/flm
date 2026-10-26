//! The parse driver, [`FlmDriver`]: the object that holds the behavior and the
//! settings of a running parse, and the two conditions it reports.
//!
//! techy splits a language in two parts. The language type ([`Flm`]) states,
//! at compile time, which kinds of groups and callables exist and which data is
//! attached to states and nodes. The **driver** is a value that holds what
//! matters only while a parse runs: how far the parse recovers from a problem,
//! how a command name is resolved, which node a paragraph break becomes, how
//! many diagnostics are kept, how a general condition is replaced by a more
//! specific one, how the parsing state changes inside a mathematics group, and
//! which object reads the file an `\input` command refers to.
//!
//! Most of [`FlmDriver`]'s methods call the corresponding function of techy's
//! LaTeX-like preset, so a parse behaves as the preset specifies. FLM adds four
//! things:
//!
//! - A paragraph break becomes a callable node of the special-character-sequence
//!   form, whose element is `flm.parbreak`
//!   ([`PARBREAK`](crate::lang::structural::PARBREAK)).
//! - When `$` is not a mathematics delimiter, the condition reported for the
//!   forbidden character `$` is replaced by [`DollarMathDisabled`], which states
//!   the delimiters to use instead.
//! - When a lone `%` is forbidden, the condition reported for it is replaced by
//!   [`PercentForbidden`], which states the spellings to use instead under the
//!   parsing settings of the file where the `%` is written.
//! - The driver holds the embedding program's input resolver
//!   ([`InputResolver`]), which the parser of `\input` asks for the included
//!   file ([`crate::lang::input`]).
//!
//! The driver implements techy's `ParseDriver` trait itself rather than wrapping
//! the preset's driver type, because a wrapper that passed recovery on to the
//! preset would skip the replacement of the two conditions above.

use std::fmt;
use std::sync::Arc;

use techy::core::node::NodeKind;
use techy::core::specs::{resolve_command_in_scopes, CommandResolution};
use techy::core::token::{ForbiddenChar, GroupRule, Token, TokenReader};
use techy::core::{ParseDriver, ParsingState, ParsingStateDelta, ParsingStateStack};
use techy::error::{DiagnosticData, DiagnosticInfo, Diagnostics, ParseError, Recovery};
use techy::latexlike::{
    exit_math_context_delta, make_paragraph_break_node, math_group_interior_delta, Event,
    LatexlikeEvent, LatexlikeLang, LatexlikeParseDriver, ParagraphBreakStyle,
};
use techy::source::{Source, SourceSpan};

use crate::lang::flm::{Flm, FlmCallableType};
use crate::lang::input::{InputResolver, Placements};
use crate::lang::origin::FlmOrigin;

/// Condition `flm.lang.dollar-math-disabled`: a `$` is written in the source
/// while `$` is not a mathematics delimiter
/// ([`FlmParsing::dollar_math`](crate::lang::FlmParsing::dollar_math), off by
/// default).
///
/// Reported at error severity by the parse. Recovery: the character is kept
/// in the text as written, and the parse continues. The condition carries the
/// [suggestion](Self::suggestion), which states the delimiters to use instead.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lang.dollar-math-disabled",
    message = "‘$’ must not be used here, because the parsing settings do not make it a \
               mathematics delimiter: write {suggestion}"
)]
pub struct DollarMathDisabled {
    /// The spellings to use instead, which the diagnostic's message includes.
    pub suggestion: String,
}

impl DollarMathDisabled {
    /// The suggestion the driver puts in every one of these conditions:
    /// `\(…\)` for inline mathematics, `\[…\]` for display mathematics.
    pub const SUGGESTION: &'static str =
        "\\(…\\) for inline mathematics or \\[…\\] for display mathematics";
}

/// Condition `flm.lang.percent-forbidden`: a lone `%` is written in the source
/// while `%` is a forbidden character
/// ([`FlmParsing::percent_forbidden`](crate::lang::FlmParsing::percent_forbidden),
/// which returns `true` under the default parsing settings).
///
/// A lone `%` is refused because it is ambiguous: in `add 10% to the total` it
/// is a percent sign, and in `% a note to myself` it is a LaTeX comment. The
/// language asks for an unambiguous spelling instead of guessing.
///
/// Reported at error severity by the parse. Recovery: the character is kept
/// in the text as written, and the parse continues.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lang.percent-forbidden",
    message = "a lone ‘%’ is not allowed, because it could be a percent sign or the start of \
               a comment: write {suggestion}"
)]
pub struct PercentForbidden {
    /// The spellings to use instead, which the diagnostic's message includes.
    pub suggestion: String,
    /// What starts a comment under the parsing settings where the `%` is
    /// written, or `None` when no comment is recognized at all.
    pub comment_start: Option<String>,
}

impl PercentForbidden {
    /// Returns the suggestion for parsing settings whose comments start with
    /// `comment_start`: `\%` for a percent sign, and the comment prefix for a
    /// comment. `None` means that no comment is recognized at all (comment
    /// recognition is switched off, or the comment prefix is empty); only the
    /// percent sign is then suggested.
    ///
    /// The driver puts this suggestion in every one of these conditions. The
    /// wording is not a stable interface; the fields are.
    pub fn suggestion(comment_start: Option<&str>) -> String {
        match comment_start {
            Some(start) => {
                format!("‘\\%’ for a percent sign, or ‘{start}’ to start a comment")
            }
            None => {
                String::from("‘\\%’ for a percent sign (no comment is recognized here)")
            }
        }
    }
}

/// FLM's parse driver: the behavior and the settings of a running parse (the
/// recovery policy, the cap on kept diagnostics, the input resolver).
///
/// [`build_language`] constructs the driver from the cap on diagnostics and the
/// input resolver of its [`LangInputs`](crate::lang::LangInputs), and
/// [`BuiltLang`] holds it ([`BuiltLang::driver`](crate::lang::BuiltLang::driver)).
/// A caller rarely builds one with [`FlmDriver::new`]; a parse with settings of
/// its own goes through [`ParseOptions`](crate::lang::ParseOptions) instead.
///
/// [`build_language`]: crate::lang::build_language
/// [`BuiltLang`]: crate::lang::BuiltLang
pub struct FlmDriver {
    recovery: Recovery,
    diagnostics_limit: Option<usize>,
    input_resolver: Option<Arc<dyn InputResolver>>,
    placements: Option<Arc<Placements>>,
}

impl FlmDriver {
    /// Returns a driver with the given settings and no input resolver.
    ///
    /// `recovery` says what happens when the parser reaches a problem.
    /// [`build_language`](crate::lang::build_language) always uses `Recovery::Tolerant`, so that one mistake
    /// in a document does not lose the rest of the tree; the strict policy is
    /// for tools that want a parse to fail at the first problem.
    ///
    /// `diagnostics_limit` caps how many diagnostics a parse keeps; `None` keeps
    /// techy's own default. Nothing is lost silently: the count of the
    /// diagnostics dropped past the cap is reported beside the kept ones.
    ///
    /// The driver holds no parsing setting: which characters are forbidden, and
    /// what starts a comment, are read from the parsing state at the character
    /// ([`FlmStateExt::parsing`](crate::lang::FlmStateExt::parsing)).
    pub fn new(recovery: Recovery, diagnostics_limit: Option<usize>) -> FlmDriver {
        FlmDriver { recovery, diagnostics_limit, input_resolver: None, placements: None }
    }

    /// Builder: sets the cap on how many diagnostics a parse keeps; `None`
    /// keeps techy's own default. A parse with its own cap
    /// ([`ParseOptions::max_diagnostics`](crate::lang::ParseOptions::max_diagnostics))
    /// runs with a copy of the driver built this way.
    pub fn with_diagnostics_limit(mut self, limit: Option<usize>) -> FlmDriver {
        self.diagnostics_limit = limit;
        self
    }

    /// Builder: sets the input resolver, the object that finds and reads the
    /// file an `\input` command refers to ([`crate::lang::input`]).
    ///
    /// The library performs no input or output of its own, so it reads no file
    /// unless the embedding program supplies a resolver; without one, every
    /// `\input` is reported ([`InputUnavailable`](crate::lang::input::InputUnavailable))
    /// and includes nothing. The `flm` command-line program supplies one that
    /// reads files from disk.
    pub fn with_input_resolver(mut self, resolver: Arc<dyn InputResolver>) -> FlmDriver {
        self.input_resolver = Some(resolver);
        self
    }

    /// Returns the input resolver, when one is set
    /// ([`FlmDriver::with_input_resolver`]).
    pub fn input_resolver(&self) -> Option<&dyn InputResolver> {
        self.input_resolver.as_deref()
    }

    /// Builder: sets the record of the heading inputs (included files that open
    /// with a heading) found while one document is assembled; the assembly
    /// reads it back ([`crate::model::assemble`]).
    pub(crate) fn with_placements(mut self, placements: Arc<Placements>) -> FlmDriver {
        self.placements = Some(placements);
        self
    }

    /// Returns the record of heading inputs, when this driver parses for the
    /// assembly of a document.
    pub(crate) fn placements(&self) -> Option<&Placements> {
        self.placements.as_deref()
    }

    /// Returns the cap on kept diagnostics this driver was given; `None` means
    /// that techy's own default applies.
    pub fn diagnostics_limit_setting(&self) -> Option<usize> {
        self.diagnostics_limit
    }
}

impl Clone for FlmDriver {
    fn clone(&self) -> FlmDriver {
        FlmDriver {
            recovery: self.recovery,
            diagnostics_limit: self.diagnostics_limit,
            input_resolver: self.input_resolver.clone(),
            placements: self.placements.clone(),
        }
    }
}

impl fmt::Debug for FlmDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlmDriver")
            .field("recovery", &self.recovery)
            .field("diagnostics_limit", &self.diagnostics_limit)
            .field("input_resolver", &self.input_resolver.as_ref().map(|_| "…"))
            .field("placements", &self.placements.is_some())
            .finish()
    }
}

impl ParseDriver<Flm> for FlmDriver {
    fn recovery(&self) -> Recovery {
        self.recovery
    }

    fn diagnostics_limit(&self) -> Option<usize> {
        self.diagnostics_limit
    }

    /// Resolves a command name against the state's scope stack, as a macro:
    /// every provider is consulted from the innermost outwards.
    ///
    /// `\begin` and `\end` are resolved like any other command; they resolve
    /// to techy's built-in entries, which then look the environment up as an
    /// environment. This resolution never fails the parse: an unresolved name is
    /// one of its possible results, and the fallback provider at the bottom of
    /// the stack ([`FlmFallbackProvider`](crate::lang::mint::FlmFallbackProvider))
    /// resolves every name anyway.
    ///
    /// # Errors
    ///
    /// None in practice: the return type allows a parse to be aborted, and this
    /// resolution does not abort one.
    fn resolve_command(
        &self,
        state: &ParsingState<Flm>,
        token: &Token<Flm>,
        tokens: &dyn TokenReader<'_, Flm>,
    ) -> Result<CommandResolution<Flm>, ParseError<FlmOrigin>> {
        Ok(resolve_command_in_scopes(state, token, tokens, FlmCallableType::Macro))
    }

    /// Returns the node a paragraph break becomes: a callable of the
    /// special-character-sequence form, with techy's standard paragraph-break
    /// specification. Such a node receives the element
    /// [`flm.parbreak`](crate::lang::structural::PARBREAK), by which the
    /// block-structure rewrite pass recognizes it.
    fn make_paragraph_break_node(
        &self,
        state: &ParsingState<Flm>,
        break_span: &SourceSpan<FlmOrigin>,
    ) -> NodeKind<Flm> {
        make_paragraph_break_node(ParagraphBreakStyle::Specials, state, break_span)
    }

    /// Replaces the condition reported for a forbidden character by an FLM
    /// condition that says what to write instead: [`DollarMathDisabled`] for
    /// `$`, and [`PercentForbidden`] for `%`, which suggests the comment prefix
    /// of the parsing settings in `state` (those of the file where the
    /// character is written). Every other condition is returned unchanged.
    fn refine_diagnostic(
        &self,
        data: Box<dyn DiagnosticData>,
        state: &ParsingState<Flm>,
    ) -> Box<dyn DiagnosticData> {
        if let Some(forbidden) = data.downcast_ref::<ForbiddenChar>() {
            // The forbidden characters come from the parsing settings alone:
            // `$` is forbidden exactly where it delimits no mathematics, and `%`
            // exactly where a lone `%` starts no comment — under the document's
            // settings or under an included file's own — so the character alone
            // says which condition this is.
            if forbidden.ch == '$' {
                return Box::new(DollarMathDisabled::new(DollarMathDisabled::SUGGESTION));
            }
            if forbidden.ch == '%' {
                let comment_start = state.ext().parsing.comment_start();
                return Box::new(PercentForbidden::new(
                    PercentForbidden::suggestion(comment_start),
                    comment_start.map(String::from),
                ));
            }
        }
        data
    }

    /// Runs the language's checks at the start of a parse,
    /// [`Flm::check_parse_start`], which FLM leaves at the family's default:
    /// no check.
    fn observe_parse_start(
        &self,
        source: &Arc<Source<FlmOrigin>>,
        initial_state: &Arc<ParsingState<Flm>>,
        diagnostics: &mut Diagnostics<FlmOrigin>,
    ) {
        Flm::check_parse_start(source, initial_state, diagnostics);
    }

    /// Interprets an event that needs the stack of enclosing parsing states:
    /// the request to leave a mathematics context becomes the change that
    /// restores the innermost enclosing context that is not mathematics. Every
    /// other event returns `None`.
    ///
    /// # Errors
    ///
    /// None in practice: the return type allows a parse to be aborted, and this
    /// interpretation does not abort one.
    fn resolve_state_event(
        &self,
        event: &Event,
        stack: &ParsingStateStack<Flm>,
    ) -> Result<Option<ParsingStateDelta<Flm>>, ParseError<FlmOrigin>> {
        Ok(event.is_exit_math_context().then(|| exit_math_context_delta(stack)))
    }

    /// Returns the change of parsing state the parser applies when it enters a
    /// group: for a group of the mathematics class, mathematics mode with the
    /// opening delimiters of mathematics removed; for every other group class,
    /// nothing beyond techy's standard change.
    fn group_interior_delta(
        &self,
        base: &ParsingState<Flm>,
        rule: &Arc<GroupRule<Flm>>,
    ) -> Option<ParsingStateDelta<Flm>> {
        math_group_interior_delta(base, rule)
    }
}

/// The preset's additional driver behavior, all at its defaults. In particular,
/// no environment changes the parsing state after its end.
impl LatexlikeParseDriver<Flm> for FlmDriver {}
