//! This module defines [`FlmDriver`], the parse driver of the FLM language,
//! along with the error conditions that the driver reports.
//!
//! techy describes a language in two parts. The *language type*, [`Flm`] for
//! FLM, defines at compile time the kinds of groups and callables of the
//! language and the data attached to parsing states and to nodes. The *parse
//! driver* is a value that provides the settings and the callbacks that the
//! parser uses during a parse: the error recovery policy, the cap on the number
//! of stored diagnostics, the resolution of command names, the node created for
//! a paragraph break, and the change of parsing state inside a math group.
//!
//! Most methods of [`FlmDriver`] call the corresponding function of techy's
//! LaTeX-like preset ([`techy::latexlike`]). FLM's driver differs from the
//! preset in the following points:
//!
//! - A paragraph break (a blank line) is parsed as a node with the element
//!   `flm.parbreak` ([`PARBREAK`](crate::lang::structural::PARBREAK)).
//! - When `$` is not a math delimiter, each `$` in the source is reported with
//!   the condition [`DollarMathDisabled`]. The message of this condition gives
//!   the math delimiters to use instead.
//! - When a lone `%` is a forbidden character, each lone `%` in the source is
//!   reported with the condition [`PercentForbidden`]. The message of this
//!   condition gives the syntax for a percent sign and for a comment.
//! - The driver stores the input resolver ([`InputResolver`]) that the
//!   embedding program provides. The parser of the `\input` command uses the
//!   input resolver to read the included file; see [`crate::lang::input`].

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

/// The error that is reported when a `$` appears in the source while `$` is not
/// a math delimiter (id `flm.lang.dollar-math-disabled`).
///
/// `$` is a math delimiter only when the parsing setting
/// [`FlmParsing::dollar_math`](crate::lang::FlmParsing::dollar_math) is on.
/// When this setting is off, which is the default, inline math is written
/// `\(…\)`, as in `\(x^2\)`, and display math is written `\[…\]`. `\$` writes a
/// dollar sign in either setting.
///
/// Reported by the parser when it reads the `$`. The `$` is kept in the text as
/// an ordinary character, and the parse continues.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lang.dollar-math-disabled",
    message = "‘$’ may not be used here; use {suggestion}"
)]
pub struct DollarMathDisabled {
    /// The text that the message gives after "use": the math delimiters to
    /// write instead of `$`. The driver always sets this field to
    /// [`DollarMathDisabled::SUGGESTION`].
    pub suggestion: String,
}

impl DollarMathDisabled {
    /// The value of the `suggestion` field in every condition that the driver
    /// reports. It gives the delimiters `\(…\)` for inline math and `\[…\]` for
    /// display math.
    pub const SUGGESTION: &'static str =
        "\\(…\\) for inline mathematics or \\[…\\] for display mathematics";
}

/// The error that is reported when a lone `%` appears in the source under
/// parsing settings that forbid it (id `flm.lang.percent-forbidden`).
///
/// A lone `%` is forbidden unless it starts a comment, that is, unless comments
/// are on and the comment prefix is `%`
/// ([`FlmParsing::percent_forbidden`](crate::lang::FlmParsing::percent_forbidden)).
/// Under the default parsing settings, the comment prefix is `%%`, and a lone
/// `%` is therefore forbidden. A lone `%` is ambiguous: it can be meant as a
/// percent sign or as the start of a comment. The author writes `\%` for a
/// percent sign, as in `add 10\% to the total`, and the comment prefix, e.g.
/// `%%`, to start a comment.
///
/// Reported by the parser when it reads the `%`. The `%` is kept in the text as
/// an ordinary character, and the parse continues.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.lang.percent-forbidden",
    message = "A lone ‘%’ is not allowed; use {suggestion}"
)]
pub struct PercentForbidden {
    /// The text that the message gives after "use": the syntax for a percent
    /// sign and for a comment, as returned by [`PercentForbidden::suggestion`].
    pub suggestion: String,
    /// The comment prefix under the parsing settings of the file in which the
    /// `%` appears, or `None` if no comment is recognized in this file. An
    /// included file can have parsing settings of its own.
    pub comment_start: Option<String>,
}

impl PercentForbidden {
    /// Returns the text of the `suggestion` field for the comment prefix
    /// `comment_start`.
    ///
    /// The text gives `\%` for a percent sign. If `comment_start` is `Some`,
    /// the text also gives the comment prefix for a comment. `comment_start` is
    /// `None` when comments are off or the comment prefix is empty; see
    /// [`FlmParsing::comment_start`](crate::lang::FlmParsing::comment_start).
    ///
    /// The wording of this text may change between versions. A program that
    /// inspects the condition should read the `comment_start` field.
    pub fn suggestion(comment_start: Option<&str>) -> String {
        match comment_start {
            Some(start) => {
                format!("‘\\%’ for a percent sign, or ‘{start}’ to start a comment")
            }
            None => {
                String::from("‘\\%’ for a percent sign")
            }
        }
    }
}

/// An `FlmDriver` is the parse driver of the FLM language. It stores the
/// settings of a parse: the error recovery policy, the cap on the number of
/// stored diagnostics, and the input resolver that reads the files included
/// with `\input`. The [module documentation](crate::lang::driver) explains what
/// a parse driver is.
///
/// You do not usually create a driver yourself. [`build_language`] creates the
/// driver of a [`BuiltLang`] from the settings in its
/// [`LangInputs`](crate::lang::LangInputs), and
/// [`BuiltLang::driver`](crate::lang::BuiltLang::driver) returns this driver.
/// To parse with a different cap on diagnostics or a different input resolver,
/// pass [`ParseOptions`](crate::lang::ParseOptions) to
/// [`parse_fragment`](crate::lang::parse_fragment).
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
    /// Returns a driver with the error recovery policy `recovery`, the cap
    /// `diagnostics_limit` on the number of stored diagnostics, and no input
    /// resolver.
    ///
    /// With [`Recovery::Tolerant`], the parser reports a problem in the source
    /// as a diagnostic and continues the parse. With [`Recovery::Strict`], the
    /// parse fails at the first problem. [`build_language`](crate::lang::build_language)
    /// always uses `Recovery::Tolerant`.
    ///
    /// `diagnostics_limit` is the maximum number of diagnostics that a parse
    /// stores. With `None`, techy's default cap applies
    /// ([`Diagnostics::DEFAULT_LIMIT`]). Diagnostics beyond the cap are not
    /// stored, but they are counted ([`Diagnostics::suppressed`]).
    pub fn new(recovery: Recovery, diagnostics_limit: Option<usize>) -> FlmDriver {
        FlmDriver { recovery, diagnostics_limit, input_resolver: None, placements: None }
    }

    /// Sets the cap on the number of diagnostics that a parse stores, and
    /// returns the driver. With `None`, techy's default cap applies
    /// ([`Diagnostics::DEFAULT_LIMIT`]).
    pub fn with_diagnostics_limit(mut self, limit: Option<usize>) -> FlmDriver {
        self.diagnostics_limit = limit;
        self
    }

    /// Sets the input resolver, and returns the driver. The input resolver finds
    /// and reads the file that an `\input` command includes; see
    /// [`crate::lang::input`].
    ///
    /// The library reads no files itself. Without an input resolver, each
    /// `\input` command is reported with the condition
    /// [`InputUnavailable`](crate::lang::input::InputUnavailable) and includes
    /// nothing. The `flm` command-line program sets an input resolver that reads
    /// files from the disk.
    pub fn with_input_resolver(mut self, resolver: Arc<dyn InputResolver>) -> FlmDriver {
        self.input_resolver = Some(resolver);
        self
    }

    /// Returns the input resolver set with [`FlmDriver::with_input_resolver`],
    /// or `None` if no input resolver is set.
    pub fn input_resolver(&self) -> Option<&dyn InputResolver> {
        self.input_resolver.as_deref()
    }

    /// Sets the list in which the parser of `\input` records the heading
    /// inputs, and returns the driver. A heading input is an included file that
    /// begins with a heading. [`crate::model::assemble`] sets this list when it
    /// assembles a document from several files, and reads it after each parse.
    pub(crate) fn with_placements(mut self, placements: Arc<Placements>) -> FlmDriver {
        self.placements = Some(placements);
        self
    }

    /// Returns the list of heading inputs set with
    /// [`FlmDriver::with_placements`], or `None` if this driver does not parse
    /// for the assembly of a document.
    pub(crate) fn placements(&self) -> Option<&Placements> {
        self.placements.as_deref()
    }

    /// Returns the cap on the number of stored diagnostics that this driver was
    /// given. `None` means that techy's default cap applies.
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

    /// Resolves the command name of `token` as a macro, in the scopes of the
    /// parsing state `state`, from the innermost scope outwards.
    ///
    /// `\begin` and `\end` are resolved like any other command name. They
    /// resolve to techy's built-in specifications of `\begin` and `\end`, which
    /// then resolve the environment name. A name that is not defined in any
    /// package is resolved by the fallback provider
    /// ([`FlmFallbackProvider`](crate::lang::mint::FlmFallbackProvider)) at the
    /// bottom of the scope stack.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn resolve_command(
        &self,
        state: &ParsingState<Flm>,
        token: &Token<Flm>,
        tokens: &dyn TokenReader<'_, Flm>,
    ) -> Result<CommandResolution<Flm>, ParseError<FlmOrigin>> {
        Ok(resolve_command_in_scopes(state, token, tokens, FlmCallableType::Macro))
    }

    /// Returns the node created for a paragraph break: a callable node with
    /// techy's standard paragraph-break specification
    /// ([`ParagraphBreakStyle::Specials`]). Nodes of this form have the element
    /// [`flm.parbreak`](crate::lang::structural::PARBREAK), and the
    /// block-structure rewrite pass splits paragraphs at these nodes.
    fn make_paragraph_break_node(
        &self,
        state: &ParsingState<Flm>,
        break_span: &SourceSpan<FlmOrigin>,
    ) -> NodeKind<Flm> {
        make_paragraph_break_node(ParagraphBreakStyle::Specials, state, break_span)
    }

    /// Replaces the condition reported for a forbidden `$` or `%` with an FLM
    /// condition whose message contains the syntax to write instead.
    ///
    /// - For `$`, the replacement is [`DollarMathDisabled`].
    /// - For `%`, the replacement is [`PercentForbidden`]. This condition gives
    ///   the comment prefix of the parsing settings in `state`, which are the
    ///   parsing settings of the file in which the `%` appears.
    ///
    /// Every other condition is returned unchanged.
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

    /// Calls [`Flm::check_parse_start`] at the start of a parse. FLM uses the
    /// default implementation of this function, which performs no check.
    fn observe_parse_start(
        &self,
        source: &Arc<Source<FlmOrigin>>,
        initial_state: &Arc<ParsingState<Flm>>,
        diagnostics: &mut Diagnostics<FlmOrigin>,
    ) {
        Flm::check_parse_start(source, initial_state, diagnostics);
    }

    /// Returns the change of parsing state for an event that depends on the
    /// enclosing parsing states in `stack`.
    ///
    /// For the event that leaves a math context, for instance at the argument
    /// of `\text`, the change restores the innermost enclosing parsing state
    /// that is not in math mode. For every other event, returns `None`.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn resolve_state_event(
        &self,
        event: &Event,
        stack: &ParsingStateStack<Flm>,
    ) -> Result<Option<ParsingStateDelta<Flm>>, ParseError<FlmOrigin>> {
        Ok(event.is_exit_math_context().then(|| exit_math_context_delta(stack)))
    }

    /// Returns the change of parsing state that the parser applies inside a
    /// group.
    ///
    /// For a math group, the parser applies this change so that the parsing
    /// state inside the group is in math mode and has no group rules that open
    /// math. For any other group, returns `None`, and the parser applies only
    /// techy's standard change.
    fn group_interior_delta(
        &self,
        base: &ParsingState<Flm>,
        rule: &Arc<GroupRule<Flm>>,
    ) -> Option<ParsingStateDelta<Flm>> {
        math_group_interior_delta(base, rule)
    }
}

/// Uses the default implementation of every function of techy's
/// [`LatexlikeParseDriver`]. In particular, the changes of parsing state made
/// inside the body of an environment do not apply after the end of the
/// environment.
impl LatexlikeParseDriver<Flm> for FlmDriver {}
