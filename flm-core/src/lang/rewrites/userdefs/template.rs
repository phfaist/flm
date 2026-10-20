//! The template language and its compiler.
//!
//! The **template language** is FLM text with placeholders (`#1`, `#{name}`,
//! `#{body}`, `#[…]{ref}`), compiled once when the registry is built; a user
//! definition's template is written in it, and so are number templates, within
//! templates, list label templates, heading title templates, theorem heading
//! templates and the defterm lead-in. A **placeholder** is a reference in a
//! template to an argument, the body, or a name whose content an expansion hook
//! computes; at expansion it is replaced by the argument's content nodes and
//! nothing else.
//!
//! The template language is the language the registry declares, extended by one
//! character, `#`, which introduces a placeholder, and by the conditional
//! commands. A template is parsed once, when the registry is built, under a
//! parsing state that adds these two to the registry's language
//! ([`TemplateLanguage`]). The result, a
//! [`CompiledTemplate`], is a tree whose placeholder nodes are known and checked,
//! ready to be copied onto a new source at every use; the
//! [expand pass](super::expand) does that copying.
//!
//! The other features that compile templates of their own in this language
//! each declare their own placeholder names (see [the template language shared by
//! features](crate::guide::concepts_overview#the-template-language-shared-by-features)).
//! For authors, the language is described in
//! [the template language](crate::guide::language_syntax#the-template-language).
//!
//! # The placeholder syntax
//!
//! - `#1` … `#9`: the numbered arguments. One digit follows `#`, as in TeX:
//!   `#12` is `#1` followed by `2`, and an argument beyond the ninth is written
//!   in the braced form, `#{12}`. The numbering starts at the definition's
//!   declared *argument number offset*, one value for the whole template, so `#1`
//!   is the argument at index `offset`. The offset is declared beside the
//!   arguments and is never written in the template.
//! - `#{name}`: the argument declared with that name; `#{body}`: an
//!   environment's body; any other name: a placeholder whose content an
//!   expansion hook of the definition computes.
//! - `#[…]{name}`: the placeholder `name` with a *substitution argument*, data
//!   for a placeholder whose content a feature computes; the numbered, named and
//!   `body` placeholders accept none.
//! - `##`: a literal `#`.
//!
//! **How `#` is recognized.** In a template parse, `#` is a second command
//! escape character beside `\`, with no name characters: the name of a command
//! written with `#` is the single character after it, so `#1` is a command named
//! `1`, `#{` one named `{`, and `##` one named `#`. As for TeX's control symbols,
//! the space after such a command is kept in the content, so `#1 and` keeps its
//! space. Each command is looked up through the
//! providers of the parsing state, innermost first. The template's placeholder
//! provider ([`PlaceholderProvider`]) returns the placeholder specification for
//! every command written with the `#` escape and for no other, so `\{` in a
//! template is never mistaken for a placeholder: the lookup carries the escape
//! character, and while a techy package cannot tell the two escapes apart, a
//! provider can.
//!
//! Inside the two restricted argument shapes of a template, a characters-only
//! group (such as the argument of `\ref{…}` or `\label{…}`) and verbatim
//! content, the same escape stays enabled through the language layer's
//! restricted-interior policy ([`RestrictedInterior::Template`]). A definition
//! can therefore pass its argument into a label, a reference target or raw
//! text. In verbatim content, where groups are not recognized, `#{name}` is
//! still a placeholder but its name is read as raw characters, and the bracketed
//! form `#[…]{…}` is not available; in a characters-only group every form is.
//! Because techy's tokenizer tries commands before special character sequences,
//! `#` opens a placeholder wherever it is written in a template, so a special
//! character sequence of the document whose characters begin with `#` cannot be
//! written inside a template. The template language reserves `#` for itself:
//! looking for special character sequences first would make the meaning of a
//! template depend on which sequences happen to be declared.
//!
//! # The conditionals
//!
//! The eleven conditionals (`\IfNoValueT`, `\IfNoValueF`, `\IfNoValueTF`,
//! `\IfValueT/F/TF`, `\IfBooleanT/F/TF`, `\ifblank`, `\notblank`), and the
//! three synonyms of the boolean family with plainer names
//! (`\IfProvidedT/F/TF`), are ordinary macros of the template language that
//! carry a [`Conditional`] as the data of their element ([`ElementSpec::value`]);
//! the expand pass evaluates them at each use and inserts the selected branch. The
//! fourteen share one element type name ([`CONDITIONAL_ELEMENT`]) and are told
//! apart by that data: one element type with one element value per command, the way the standard features declare a family of related commands
//! (see [elements and aspects](crate::guide::concepts_overview#elements-and-aspects)).
//! [`ConditionalTest`] and [the parent module's documentation](super) describe
//! what each family tests.
//!
//! # What compilation checks
//!
//! Every placeholder is resolved against the definition's
//! [`PlaceholderVocabulary`]: a name an expansion hook computes first, then
//! `body`, then the named and the numbered arguments. A placeholder that is out
//! of range, refers to nothing or is misused is reported as
//! `flm.userdefs.bad-placeholder` when the registry is built, and the definition
//! is refused; so is any Error diagnostic of the template parse. A template's
//! errors are therefore found once, when the registry is built, rather than in
//! each document.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use techy::core::constructs::{
    parse_declared_arguments, ConstructParser, ConstructParserResult, Invocation, ParseContext,
};
use techy::core::node::{
    BuildId, CallableData, ContentNodes, NodeBuildError, NodeId, NodeKind, NodeTree,
    ParsedArgument, ParsedArguments, ParsedSlots,
};
use techy::core::specs::{
    ArgumentSpec, CallableQuery, CallableSpec, CallableSyntax, Package, ProviderError,
    SpecsProvider,
};
use techy::core::token::{CommandOverrides, CommandRule, TokenEdge, TokenKind, TokenRulesOverrides};
use techy::core::{DeriveError, FrameRole, ParsingState, ParsingStateDelta};
use techy::error::{Diagnostic, HookFailed, ParseError, Severity};
use techy::latexlike::{argument_specs_named, InvocationSyntaxData, Mode};
use techy::serialize::SerializableObject;
use techy::source::{Source, SourceSpan, TextContent};

use crate::model::doc::FragmentLayout;
use crate::model::element::ElementSpec;
use crate::lang::build::BuiltLang;
use crate::lang::ext::{TemplateCommandRule, TemplateInteriorRules, RestrictedInterior};
use crate::lang::flm::{Flm, FlmCallableType};
use crate::lang::mint::{mint_packages, ArgDef, ArgStdKind, ElementDef, MintError, SyntaxDef};
use crate::lang::origin::{FlmDiagnostic, FlmOrigin, FlmSource, FlmSpan};
use crate::lang::parse::{parse_fragment_with_state, ParseOptions};
use crate::lang::rewrites::userdefs::diagnostics::{
    BadDefinition, BadPlaceholder, DefinitionProblem, PlaceholderProblem,
};

/// The escape character that introduces a placeholder in a template: `#`.
pub const PLACEHOLDER_ESCAPE: char = '#';

/// The placeholder name that refers to an environment's body: `#{body}`.
pub const BODY_REFERENCE: &str = "body";

/// The name of the template language's techy package (the conditionals), which
/// is also the owner name of that techy package's element declarations.
pub const TEMPLATE_PACKAGE_NAME: &str = "_userdefs.template";

/// The name of the placeholder provider ([`PlaceholderProvider`]) among the
/// providers of a parsing state.
pub const PLACEHOLDER_PROVIDER_NAME: &str = "_userdefs.placeholder";

/// The element type name of the conditional macros, `flm.userdefs.conditional`;
/// each conditional is one element value carrying its [`Conditional`].
pub const CONDITIONAL_ELEMENT: &str = "flm.userdefs.conditional";

// --- the conditionals ---------------------------------------------------------------

/// What a conditional tests about its first argument. There are three different
/// questions, easy to confuse: *provided* asks whether the use site wrote
/// something here; *value*, whether there is something to substitute, a
/// declared default value included; *blank*, whether what would be substituted
/// is empty or holds only comments.
///
/// A declared default value is bound before a conditional is evaluated, which is
/// why *provided* and *value* are two tests and not one: they differ exactly on
/// an argument that the use site left out and a default value fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConditionalTest {
    /// `\IfNoValue…`: the argument is a single placeholder that has nothing to
    /// substitute: not written at the use site *and* without a declared default
    /// value.
    NoValue,
    /// `\IfValue…`: the negation of [`NoValue`](ConditionalTest::NoValue): the
    /// placeholder has something to substitute, a declared default value included.
    Value,
    /// `\IfBoolean…` and `\IfProvided…`: the argument is a single placeholder whose
    /// value was written at the use site (typically a `*` marker), or computed by an
    /// expansion hook. A declared default value does not make this test hold.
    Boolean,
    /// `\ifblank`: the argument, with its placeholders substituted, is empty or
    /// holds nothing but comments. Whitespace is content, and so are groups and
    /// commands.
    Blank,
    /// `\notblank`: the negation of [`Blank`](ConditionalTest::Blank).
    NotBlank,
}

/// Which branches a conditional macro takes after its test argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Branches {
    /// One branch, used when the test holds (`…T`).
    True,
    /// One branch, used when the test fails (`…F`).
    False,
    /// Two branches, the first when the test holds, the second when it fails
    /// (`…TF`, `\ifblank`, `\notblank`).
    Both,
}

/// The data of a conditional macro's element: its test and its branches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Conditional {
    /// The test.
    pub test: ConditionalTest,
    /// The branches that follow the test argument.
    pub branches: Branches,
}

impl Conditional {
    /// The fourteen conditionals, each with its macro name.
    pub const ALL: [(&'static str, Conditional); 14] = [
        ("IfNoValueT", Conditional { test: ConditionalTest::NoValue, branches: Branches::True }),
        ("IfNoValueF", Conditional { test: ConditionalTest::NoValue, branches: Branches::False }),
        ("IfNoValueTF", Conditional { test: ConditionalTest::NoValue, branches: Branches::Both }),
        ("IfValueT", Conditional { test: ConditionalTest::Value, branches: Branches::True }),
        ("IfValueF", Conditional { test: ConditionalTest::Value, branches: Branches::False }),
        ("IfValueTF", Conditional { test: ConditionalTest::Value, branches: Branches::Both }),
        ("IfBooleanT", Conditional { test: ConditionalTest::Boolean, branches: Branches::True }),
        ("IfBooleanF", Conditional { test: ConditionalTest::Boolean, branches: Branches::False }),
        ("IfBooleanTF", Conditional { test: ConditionalTest::Boolean, branches: Branches::Both }),
        ("IfProvidedT", Conditional { test: ConditionalTest::Boolean, branches: Branches::True }),
        ("IfProvidedF", Conditional { test: ConditionalTest::Boolean, branches: Branches::False }),
        ("IfProvidedTF", Conditional { test: ConditionalTest::Boolean, branches: Branches::Both }),
        ("ifblank", Conditional { test: ConditionalTest::Blank, branches: Branches::Both }),
        ("notblank", Conditional { test: ConditionalTest::NotBlank, branches: Branches::Both }),
    ];

    /// How many mandatory arguments the macro takes: the test argument plus one
    /// per branch.
    pub fn argument_count(&self) -> usize {
        match self.branches {
            Branches::True | Branches::False => 2,
            Branches::Both => 3,
        }
    }

    /// The index, in the order the arguments are written, of the branch argument
    /// selected when the test's outcome is `holds`; `None` when the macro has no
    /// branch for that outcome.
    pub fn branch_index(&self, holds: bool) -> Option<usize> {
        match (self.branches, holds) {
            (Branches::True, true) | (Branches::False, false) | (Branches::Both, true) => Some(1),
            (Branches::Both, false) => Some(2),
            (Branches::True, false) | (Branches::False, true) => None,
        }
    }
}

// --- placeholder references ------------------------------------------------------------

/// What a placeholder refers to, decided when the template is compiled.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PlaceholderTarget {
    /// The argument at this index (in the order the arguments are written, counting
    /// from zero), for numbered and named placeholders alike, the argument number
    /// offset applied.
    Argument(usize),
    /// The environment's body.
    Body,
    /// A name whose content the definition's expansion hook computes.
    Extra(String),
}

/// One placeholder of a compiled template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceholderRef {
    /// The placeholder as written between the `#` and the end of the placeholder
    /// (`1`, `body`, `title`).
    pub written: String,
    /// What it refers to.
    pub target: PlaceholderTarget,
    /// Whether a substitution argument (`#[…]{…}`) was written; its nodes are the
    /// placeholder node's first argument.
    pub substitution: bool,
}

/// A placeholder name whose content a definition's expansion hook computes.
/// Such names are tried first when a placeholder is resolved; see
/// [`PlaceholderBindings`](super::PlaceholderBindings).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraPlaceholder {
    /// The name, as written between the braces of `#{name}`.
    pub name: String,
    /// Whether the name accepts a substitution argument (`#[…]{name}`).
    pub substitution: bool,
}

/// What a template may refer to: the declared arguments (by number and, where a
/// name is declared, by name), the body, and the names whose content an
/// expansion hook computes. [`Definition::vocabulary`](super::Definition::vocabulary)
/// builds a user definition's vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlaceholderVocabulary {
    /// The number of declared arguments.
    pub arity: usize,
    /// The declared name of each argument, by index (`None` for an argument
    /// declared without a name).
    pub names: Vec<Option<String>>,
    /// Whether `#{body}` is available (an environment).
    pub body: bool,
    /// The names whose content an expansion hook computes.
    pub extras: Vec<ExtraPlaceholder>,
}

impl PlaceholderVocabulary {
    /// A vocabulary of `arity` arguments without names, with no body and no
    /// computed names.
    pub fn new(arity: usize) -> PlaceholderVocabulary {
        PlaceholderVocabulary { arity, names: vec![None; arity], body: false, extras: Vec::new() }
    }

    /// Builder: sets the argument names, by index (`None` for an argument without a
    /// name).
    pub fn names(mut self, names: Vec<Option<String>>) -> PlaceholderVocabulary {
        self.names = names;
        self
    }

    /// Builder: `#{body}` is available.
    pub fn with_body(mut self) -> PlaceholderVocabulary {
        self.body = true;
        self
    }

    /// Builder: adds a name whose content an expansion hook computes; it accepts a
    /// substitution argument when `substitution` is `true`.
    pub fn extra(mut self, name: impl Into<String>, substitution: bool) -> PlaceholderVocabulary {
        self.extras.push(ExtraPlaceholder { name: name.into(), substitution });
        self
    }

    /// The index of the argument declared with the name `name`, if any.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|declared| declared.as_deref() == Some(name))
    }

    /// The target of the placeholder written as `written`, with or without a
    /// substitution argument, under the definition's argument number offset
    /// `offset`.
    ///
    /// The placeholder is resolved in a fixed order: a computed name, then `body`,
    /// then a declared argument name, then a number. A numbered placeholder `#n`
    /// refers to the argument at index `n - 1 + offset`; a named placeholder is not
    /// affected by the offset, which applies only to what is written as a number.
    ///
    /// # Errors
    ///
    /// The [`PlaceholderProblem`] that says what is wrong with the placeholder: it
    /// is out of range, refers to nothing, refers to a body the definition does not
    /// have, or carries a substitution argument where none is accepted.
    pub fn resolve(
        &self,
        written: &str,
        substitution: bool,
        offset: usize,
    ) -> Result<PlaceholderTarget, PlaceholderProblem> {
        if let Some(extra) = self.extras.iter().find(|extra| extra.name == written) {
            if substitution && !extra.substitution {
                return Err(PlaceholderProblem::SubstitutionNotAccepted);
            }
            return Ok(PlaceholderTarget::Extra(written.to_string()));
        }
        if written == BODY_REFERENCE {
            if !self.body {
                return Err(PlaceholderProblem::NoBody);
            }
            if substitution {
                return Err(PlaceholderProblem::SubstitutionNotAccepted);
            }
            return Ok(PlaceholderTarget::Body);
        }
        if let Some(index) = self.index_of(written) {
            if substitution {
                return Err(PlaceholderProblem::SubstitutionNotAccepted);
            }
            return Ok(PlaceholderTarget::Argument(index));
        }
        if !written.is_empty() && written.bytes().all(|byte| byte.is_ascii_digit()) {
            // The arity a numbered reference is measured against is what the
            // offset leaves referable, which is what the diagnostic reports.
            let referable = self.arity.saturating_sub(offset);
            let out_of_range = || PlaceholderProblem::OutOfRange { arity: referable };
            let number: usize = written.parse().map_err(|_| out_of_range())?;
            if number < 1 || number > referable {
                return Err(out_of_range());
            }
            if substitution {
                return Err(PlaceholderProblem::SubstitutionNotAccepted);
            }
            return Ok(PlaceholderTarget::Argument(number - 1 + offset));
        }
        Err(PlaceholderProblem::UnknownName)
    }
}

// --- the placeholder spec and its parser -----------------------------------------------

/// Turns a failure to build a node into the error that aborts the parse,
/// separating a hook failure from an implementation error as techy does (the
/// language layer's verbatim parsers do the same).
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

/// The specification of the `#` placeholder command: two declared arguments,
/// the optional substitution argument `[…]` and the mandatory placeholder name
/// `{…}` (a single character is read as a braced group, as for any mandatory
/// argument, which is how `#1` is read). The placeholder's own invocation parser
/// ([`PlaceholderParser`]) parses them, and also handles the `##` escape and the
/// raw-name form inside verbatim content.
pub struct PlaceholderSpec {
    arguments: Vec<Arc<ArgumentSpec<Flm>>>,
}

impl PlaceholderSpec {
    /// The specification.
    ///
    /// # Panics
    ///
    /// If the argument codes, which are constants of this module, fail to parse.
    /// No document input can reach this panic.
    pub fn new() -> PlaceholderSpec {
        let codes = [("o", "substitution"), ("m", "reference")];
        let specs = argument_specs_named::<Flm, _, _, _>(codes)
            .expect("the placeholder argument codes are constants of this module");
        PlaceholderSpec { arguments: specs }
    }
}

impl Default for PlaceholderSpec {
    fn default() -> PlaceholderSpec {
        PlaceholderSpec::new()
    }
}

impl fmt::Debug for PlaceholderSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlaceholderSpec").finish()
    }
}

// The template language is never serialized in build step 1 (a compiled template
// is a build-time value); the serialization work package may give these types a
// codec.
impl SerializableObject<Flm> for PlaceholderSpec {}

impl CallableSpec<Flm> for PlaceholderSpec {
    fn arguments(&self) -> &[Arc<ArgumentSpec<Flm>>] {
        &self.arguments
    }

    fn requires_content(&self) -> bool {
        true
    }

    fn make_invocation_parser<'a>(
        &'a self,
        invocation: Invocation<'a, Flm>,
    ) -> Result<Box<dyn ConstructParser<Flm, Output = BuildId> + 'a>, ParseError<FlmOrigin>> {
        Ok(Box::new(PlaceholderParser { invocation }))
    }

    fn stack_frame_title(&self, role: FrameRole, name: &str) -> String {
        match role {
            FrameRole::Argument { index } => {
                format!("argument #{} of placeholder beginning ‘{}’", index + 1, name)
            }
            _ => format!("placeholder beginning ‘{name}’"),
        }
    }
}

/// The change of parsing state under which `#` is an ordinary character: the
/// command rules without the placeholder escape. The parser reads the escape
/// again as a character under this state, to place itself right after the `#`
/// before it parses the bracketed forms.
fn without_placeholder_escape(state: &ParsingState<Flm>) -> ParsingStateDelta<Flm> {
    let commands: Vec<Arc<CommandRule>> = state
        .rules()
        .command_rules()
        .iter()
        .filter(|rule| rule.escape_char != PLACEHOLDER_ESCAPE)
        .cloned()
        .collect();
    ParsingStateDelta::new().rules(TokenRulesOverrides {
        commands: CommandOverrides { enabled: Some(true), rules: Some(commands) },
        ..TokenRulesOverrides::default()
    })
}

/// The placeholder name of a parsed name argument, read from the nodes already
/// built: the characters of its content when the content is text, else the
/// source text the content nodes cover; surrounding whitespace removed.
fn staged_reference_text(
    cx: &ParseContext<'_, '_, Flm>,
    children: &[BuildId],
    argument: &ParsedArgument<Flm>,
) -> String {
    let Some(region) = &argument.region else {
        return String::new();
    };
    let Some((range, content)) = region.staged() else {
        return String::new();
    };
    let staged = cx.staged_nodes();
    let ids: Vec<BuildId> = match content {
        ContentNodes::InRegion(inner) => children
            .get(range.start as usize..range.end as usize)
            .and_then(|region| region.get(inner.start as usize..inner.end as usize))
            .map(<[BuildId]>::to_vec)
            .unwrap_or_default(),
        ContentNodes::InChildrenOf(parent, inner) => staged
            .get(*parent)
            .and_then(|view| view.children().get(inner.start as usize..inner.end as usize))
            .map(<[BuildId]>::to_vec)
            .unwrap_or_default(),
    };
    let mut text = String::new();
    let mut all_text = true;
    for id in &ids {
        match staged.get(*id) {
            Some(view) => match view.kind() {
                NodeKind::Chars { content } => text.push_str(content.resolve(view.span().source())),
                _ => {
                    all_text = false;
                    break;
                }
            },
            None => {
                all_text = false;
                break;
            }
        }
    }
    if !all_text {
        text.clear();
        if let (Some(first), Some(last)) = (ids.first(), ids.last()) {
            if let (Some(first), Some(last)) = (staged.get(*first), staged.get(*last)) {
                let source = first.span().source();
                if Arc::ptr_eq(source, last.span().source()) {
                    let range = first.span().start()..last.span().end();
                    if let Some(covered) = source.content().get(range) {
                        text.push_str(covered);
                    }
                }
            }
        }
    }
    text.trim().to_string()
}

/// Builds a placeholder node: a command of the placeholder type, named after
/// the placeholder name, with the given argument records and children (the
/// regions of the bracketed forms; none for the bare forms), under the current
/// parsing state.
fn stage_placeholder(
    cx: &mut ParseContext<'_, '_, Flm>,
    invocation: &Invocation<'_, Flm>,
    reference: &str,
    arguments: ParsedArguments<Flm>,
    children: Vec<BuildId>,
    span: FlmSpan,
) -> ConstructParserResult<Flm, BuildId> {
    let data = CallableData {
        callable_type: FlmCallableType::Placeholder,
        name: reference.into(),
        spec: Arc::clone(invocation.spec),
        arguments,
        slots: ParsedSlots::empty(),
        // Recorded as specials syntax although the token was a `#`-escaped
        // command: a placeholder never survives into a document tree, so its
        // spelling is never re-emitted (a recomposition of a *template* tree
        // would print the reference without its `#`; nothing recomposes one).
        invocation_syntax: InvocationSyntaxData::Specials,
    };
    let state = Arc::clone(&cx.state);
    cx.stage_node(NodeKind::callable(data), span.clone(), state, children)
        .map_err(|error| staging_error(cx, error, span))
}

/// The invocation parser of [`PlaceholderSpec`]. The parser's main loop hands it
/// a command token written with the `#` escape, whose name is the one character
/// after the escape: a digit is the placeholder name itself; `#` is the escape
/// for a literal `#`; `{` and `[` start the bracketed forms, read again from the
/// `#` with the specification's declared arguments (or, inside verbatim content,
/// as a raw name); any other character is a placeholder name that the compiler
/// will refuse.
pub struct PlaceholderParser<'a> {
    invocation: Invocation<'a, Flm>,
}

impl PlaceholderParser<'_> {
    /// `##`: a text node holding one `#`.
    fn stage_escape(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
        span: FlmSpan,
    ) -> ConstructParserResult<Flm, BuildId> {
        let kind = NodeKind::chars(TextContent::from(String::from(PLACEHOLDER_ESCAPE)));
        let state = Arc::clone(&cx.state);
        cx.stage_node(kind, span.clone(), state, Vec::new())
            .map_err(|error| staging_error(cx, error, span))
    }

    /// `#{name}` and `#[…]{name}` where groups are recognized: moves to right after
    /// the `#` and parses the declared arguments.
    fn parse_bracketed(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
    ) -> ConstructParserResult<Flm, BuildId> {
        let token = self.invocation.token;
        cx.tokens.move_to(token, TokenEdge::Start);
        let plain = cx.derive_state(&without_placeholder_escape(&cx.state))?;
        let Some(escape) = cx.probe_token(&plain)? else {
            let here = cx.here();
            return Err(cx.implementation_error(
                "the placeholder escape could not be re-read as a character",
                here,
            ));
        };
        match cx.tokens.token_kind(&escape) {
            TokenKind::Char(c) if c == PLACEHOLDER_ESCAPE => {}
            other => {
                let span = cx.tokens.source_span_of(&escape);
                return Err(cx.implementation_error(
                    format!("expected the placeholder escape, found a {} token", other.as_str()),
                    span,
                ));
            }
        }
        let start = cx.tokens.position_at(&escape, TokenEdge::Start);
        let name = cx.tokens.source_span_of(&escape);
        cx.tokens.move_to(&escape, TokenEdge::EndPastPostSpace);
        let (children, arguments) = parse_declared_arguments(cx, self.invocation.spec, &name)?;
        let reference = arguments
            .get(1)
            .map(|argument| staged_reference_text(cx, &children, argument))
            .unwrap_or_default();
        let end = cx.tokens.position_here();
        let span = cx.source_span_within(&start, &end)?;
        stage_placeholder(
            cx,
            &self.invocation,
            &reference,
            ParsedArguments::from(arguments),
            children,
            span,
        )
    }

    /// `#{name}` where groups are not recognized (verbatim content): the name is
    /// read as raw characters up to the closing brace.
    fn parse_raw_name(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
    ) -> ConstructParserResult<Flm, BuildId> {
        let token = self.invocation.token;
        let state = Arc::clone(&cx.state);
        let start = cx.tokens.position_at(token, TokenEdge::Start);
        let mut name = String::new();
        let end = loop {
            let Some(next) = cx.probe_token(&state)? else {
                break cx.tokens.position_here();
            };
            let closes = match cx.tokens.token_kind(&next) {
                TokenKind::Char('}') => true,
                TokenKind::GroupClose { delim } => delim == "}",
                TokenKind::Char(c) => {
                    name.push(c);
                    cx.tokens.move_to(&next, TokenEdge::EndPastPostSpace);
                    continue;
                }
                _ => break cx.tokens.position_here(),
            };
            if closes {
                let end = cx.tokens.position_at(&next, TokenEdge::EndPastPostSpace);
                cx.tokens.move_to(&next, TokenEdge::EndPastPostSpace);
                break end;
            }
            break cx.tokens.position_here();
        };
        let span = cx.source_span_within(&start, &end)?;
        stage_placeholder(
            cx,
            &self.invocation,
            name.trim(),
            ParsedArguments::empty(),
            Vec::new(),
            span,
        )
    }
}

impl fmt::Debug for PlaceholderParser<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlaceholderParser").field("invocation", &self.invocation).finish()
    }
}

impl ConstructParser<Flm> for PlaceholderParser<'_> {
    type Output = BuildId;

    fn parse(
        &mut self,
        cx: &mut ParseContext<'_, '_, Flm>,
    ) -> ConstructParserResult<Flm, (BuildId, Option<Box<ParsingStateDelta<Flm>>>)> {
        let token = self.invocation.token;
        let name: String = match cx.tokens.token_kind(token) {
            TokenKind::Command { name, .. } => name.to_string(),
            other => {
                let span = cx.tokens.source_span_of(token);
                return Err(cx.implementation_error(
                    format!("a placeholder was dispatched from a {} token", other.as_str()),
                    span,
                ));
            }
        };
        let span = cx.tokens.source_span_of(token);
        let id = if name.chars().all(|c| c == PLACEHOLDER_ESCAPE) && !name.is_empty() {
            self.stage_escape(cx, span)?
        } else if (name == "{" || name == "[") && cx.state.rules().groups_enabled() {
            self.parse_bracketed(cx)?
        } else if name == "{" {
            self.parse_raw_name(cx)?
        } else {
            stage_placeholder(
                cx,
                &self.invocation,
                &name,
                ParsedArguments::empty(),
                Vec::new(),
                span,
            )?
        };
        Ok((id, None))
    }
}

// --- the placeholder provider ------------------------------------------------------------

// The placeholder is spelled as a command escape rather than as a specials
// because the restricted-interior policy gates the specials scan as a whole: a
// specials-shaped placeholder would make every feature's specials visible inside
// a template's verbatim content, whereas the policy lists command escapes
// individually. See the design rationale at [§flm-dd-dr:lang-userdef-escape].

/// The provider, among those of a parsing state, of the placeholder
/// specification. It returns the specification for every command written with
/// the `#` escape (as a macro) and for nothing else, so that the template's own
/// escape reaches the placeholder parser while the commands written with `\` are
/// looked up, unchanged, in the packages below it.
pub struct PlaceholderProvider {
    spec: Arc<dyn CallableSpec<Flm>>,
}

impl PlaceholderProvider {
    /// The provider, over a new [`PlaceholderSpec`].
    ///
    /// # Panics
    ///
    /// For the same reason as [`PlaceholderSpec::new`]: if the argument codes, which
    /// are constants of this module, fail to parse. No document input can reach
    /// this panic.
    pub fn new() -> PlaceholderProvider {
        PlaceholderProvider { spec: Arc::new(PlaceholderSpec::new()) }
    }

    /// The specification that every placeholder node records.
    pub fn spec(&self) -> &Arc<dyn CallableSpec<Flm>> {
        &self.spec
    }
}

impl Default for PlaceholderProvider {
    fn default() -> PlaceholderProvider {
        PlaceholderProvider::new()
    }
}

impl fmt::Debug for PlaceholderProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlaceholderProvider").finish()
    }
}

impl SerializableObject<Flm> for PlaceholderProvider {}

impl SpecsProvider<Flm> for PlaceholderProvider {
    fn name(&self) -> &str {
        PLACEHOLDER_PROVIDER_NAME
    }

    fn retrieve_spec(
        &self,
        query: &CallableQuery<'_, Flm>,
        _state: &ParsingState<Flm>,
    ) -> Result<Option<Arc<dyn CallableSpec<Flm>>>, ProviderError> {
        let placeholder_escape = matches!(
            query.syntax,
            CallableSyntax::Command { escape_char } if escape_char == PLACEHOLDER_ESCAPE
        );
        if query.callable_type == FlmCallableType::Macro && placeholder_escape {
            Ok(Some(Arc::clone(&self.spec)))
        } else {
            Ok(None)
        }
    }
}

// --- the template language ------------------------------------------------------------

/// The element declarations of the template language's package: the
/// conditionals, each a macro with mandatory arguments carrying its element,
/// available in both parsing modes, since a template is read in the mode of
/// the definition it belongs to.
fn template_declarations() -> Vec<ElementDef> {
    let mut defs = Vec::with_capacity(Conditional::ALL.len());
    for (name, conditional) in Conditional::ALL {
        let element = Arc::new(
            ElementSpec::new(CONDITIONAL_ELEMENT)
                .docs(format!("the conditional `\\{name}` of the template language"))
                .with_value(Arc::new(conditional)),
        );
        let args = vec![ArgDef::new(ArgStdKind::Mandatory); conditional.argument_count()];
        defs.push(
            ElementDef::new(TEMPLATE_PACKAGE_NAME, [name], SyntaxDef::Macro { args }, element)
                .modes([Mode::Text, Mode::Math]),
        );
    }
    defs
}

/// The template language over a built language: the techy package of the
/// conditionals, built from their element declarations like every other techy
/// package a parse uses ([minting](crate::lang::mint)), and the placeholder
/// provider.
///
/// One value serves every template compiled against the same built language: the
/// `userdefs` feature builds one for all its definitions, and the other features
/// that compile templates of their own use one in the same way.
pub struct TemplateLanguage {
    package: Arc<Package<Flm>>,
    provider: Arc<PlaceholderProvider>,
}

impl TemplateLanguage {
    /// The template language over `lang`: the techy package of the conditionals
    /// and the placeholder provider.
    ///
    /// # Errors
    ///
    /// The minting errors of the template language's own declarations. Those
    /// declarations are constants of this module, so no error is expected; they are
    /// returned rather than causing a panic, and the compilers report them as
    /// `flm.userdefs.bad-definition` diagnostics ([`minting_diagnostics`]).
    pub fn new(lang: &BuiltLang) -> Result<TemplateLanguage, Vec<MintError>> {
        let defs = template_declarations();
        let minted = mint_packages(&defs, &[], &[], &[], &BTreeSet::new(), &[], lang.structural());
        if !minted.errors.is_empty() {
            return Err(minted.errors);
        }
        let Some(package) = minted.packages.into_iter().next() else {
            return Err(Vec::new());
        };
        Ok(TemplateLanguage { package, provider: Arc::new(PlaceholderProvider::new()) })
    }

    /// The techy package of the conditionals.
    pub fn package(&self) -> &Arc<Package<Flm>> {
        &self.package
    }

    /// The placeholder provider.
    pub fn provider(&self) -> &Arc<PlaceholderProvider> {
        &self.provider
    }

    /// The parsing state in which a template of the parsing mode `mode` is parsed:
    /// the built language's initial parsing state, with the `#` escape added to the
    /// command rules, the `Template` restricted-interior policy
    /// ([`RestrictedInterior::Template`]) keeping that escape enabled inside
    /// characters-only groups and verbatim content, the two providers (the techy
    /// package of the conditionals and the placeholder provider) added innermost, and the
    /// mode set.
    ///
    /// # Errors
    ///
    /// techy's [`DeriveError`] when the state cannot be derived, for example when a
    /// provider cannot be added.
    #[allow(clippy::result_large_err)]
    pub fn state(
        &self,
        lang: &BuiltLang,
        mode: Mode,
    ) -> Result<Arc<ParsingState<Flm>>, DeriveError<Flm>> {
        let base = lang.initial_state();
        let mut commands: Vec<Arc<CommandRule>> = base.rules().command_rules().to_vec();
        commands.push(Arc::new(CommandRule {
            escape_char: PLACEHOLDER_ESCAPE,
            name_chars: String::new(),
        }));
        let rules = TokenRulesOverrides {
            commands: CommandOverrides { enabled: Some(true), rules: Some(commands) },
            ..TokenRulesOverrides::default()
        };
        let policy = RestrictedInterior::Template(TemplateInteriorRules {
            commands: vec![TemplateCommandRule {
                escape_char: PLACEHOLDER_ESCAPE,
                name_chars: String::new(),
            }],
            specials: false,
        });
        let ext = lang.initial_state().ext().clone().with_restricted_interior(policy);
        let package: Arc<dyn SpecsProvider<Flm>> = self.package.clone();
        let provider: Arc<dyn SpecsProvider<Flm>> = self.provider.clone();
        let delta = ParsingStateDelta::new()
            .rules(rules)
            .ext(ext)
            .push_provider(package)
            .push_provider(provider)
            .mode(mode);
        lang.derive_state(&delta)
    }
}

impl fmt::Debug for TemplateLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TemplateLanguage").field("package", &self.package.name()).finish()
    }
}

// --- compilation ------------------------------------------------------------------------

/// What to compile: one template text for one parsing mode, against a
/// vocabulary, on behalf of a definition. [`compile_template`] and
/// [`compile_template_with`] take it.
#[derive(Debug, Clone)]
pub struct TemplateSpec<'a> {
    /// The template text.
    pub text: &'a str,
    /// The parsing mode the template is compiled for (its nodes are parsed in that
    /// mode).
    pub mode: Mode,
    /// What the template may refer to.
    pub vocabulary: &'a PlaceholderVocabulary,
    /// The definition's argument number offset: `#1` is the argument at this
    /// index (`0` for a definition that declares none).
    pub offset: usize,
    /// The definition's name as an author writes it (`\foo`), for diagnostics.
    pub definition: &'a str,
    /// The description of the sources created for each use (`expansion of \foo`):
    /// the label of those sources, through which every expanded node can be traced
    /// to the expansion.
    pub label: &'a str,
    /// What the compiled source is called, without the mode (`template of \foo`,
    /// `default value of #1 of \foo`). The compiled source's label is this text with
    /// the mode appended (`template of \foo (text mode)`), so that every diagnostic
    /// raised while the template is parsed identifies the definition.
    pub origin_label: &'a str,
    /// Whether the text is a user definition's own: its template, or the default
    /// value of one of its arguments. The compiled source's origin is then
    /// [`FlmOrigin::definition`] rather than [`FlmOrigin::synthetic`], which marks it
    /// as text that the caller configured for the document.
    pub user_definition: bool,
}

/// A template compiled once, to be copied onto a new source at each use: the
/// text, the source it was parsed from, the tree, and the checked placeholders
/// by node. Every text node of the tree holds its own text, so a copy of a node
/// onto another source carries its text with it.
pub struct CompiledTemplate {
    text: String,
    source: Arc<FlmSource>,
    tree: Arc<NodeTree<Flm>>,
    mode: Mode,
    definition: String,
    label: String,
    placeholders: BTreeMap<NodeId, PlaceholderRef>,
    diagnostics: Vec<FlmDiagnostic>,
}

impl CompiledTemplate {
    /// The template text, which is also the content of every source created for a
    /// use.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The source the template was parsed from: every node of
    /// [`tree`](CompiledTemplate::tree) has its position in it.
    pub fn source(&self) -> &Arc<FlmSource> {
        &self.source
    }

    /// The compiled tree.
    pub fn tree(&self) -> &Arc<NodeTree<Flm>> {
        &self.tree
    }

    /// The parsing mode the template was compiled for.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The definition's name as an author writes it (`\foo`).
    pub fn definition(&self) -> &str {
        &self.definition
    }

    /// The description of the sources created for each use.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The placeholder at `node`, if that node is one.
    pub fn placeholder(&self, node: NodeId) -> Option<&PlaceholderRef> {
        self.placeholders.get(&node)
    }

    /// Every placeholder, with its node, in the tree's storage order.
    pub fn placeholders(&self) -> impl Iterator<Item = (NodeId, &PlaceholderRef)> + '_ {
        self.placeholders.iter().map(|(id, reference)| (*id, reference))
    }

    /// The diagnostics of the template parse that are not errors.
    pub fn diagnostics(&self) -> &[FlmDiagnostic] {
        &self.diagnostics
    }
}

impl fmt::Debug for CompiledTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompiledTemplate")
            .field("text", &self.text)
            .field("mode", &self.mode)
            .field("definition", &self.definition)
            .field("placeholders", &self.placeholders)
            .finish()
    }
}

/// The empty position at the start of `source`: where a problem with the
/// definition as a whole is reported.
fn source_start(source: &Arc<FlmSource>) -> FlmSpan {
    SourceSpan::new(source, 0..0)
}

/// The diagnostics that report a template language that could not be built
/// ([`TemplateLanguage::new`]): one `flm.userdefs.bad-definition` per minting
/// error, or one when `errors` is empty, each at the start of an empty source
/// labeled `declaration of <definition>`, where `definition` is the definition's
/// name as an author writes it. A feature that compiles templates of its own
/// reports a failure to build the template language with it.
pub fn minting_diagnostics(definition: &str, errors: &[MintError]) -> Vec<FlmDiagnostic> {
    let origin = FlmOrigin::synthetic(format!("declaration of {definition}"));
    let source: Arc<FlmSource> = Arc::new(Source::new(String::new()).with_origin(origin));
    let errors = if errors.is_empty() {
        vec![String::from("its declarations produced no package and reported no error")]
    } else {
        errors.iter().map(ToString::to_string).collect()
    };
    errors
        .into_iter()
        .map(|detail| {
            Diagnostic::error(
                BadDefinition::new(definition, DefinitionProblem::Minting { detail }),
                source_start(&source),
            )
        })
        .collect()
}

/// Compiles one template against `lang`, over a template language built for this
/// call alone; [`compile_template_with`] shares one template language across
/// many templates.
///
/// On success, the compiled template keeps the diagnostics of the compilation
/// that are not errors ([`CompiledTemplate::diagnostics`]).
///
/// # Errors
///
/// Every diagnostic of the compilation, when any of them is an Error: the
/// template parse's Error diagnostics, the placeholder problems
/// (`flm.userdefs.bad-placeholder`), and `flm.userdefs.bad-definition` with the
/// problem [`ParseFailed`](super::DefinitionProblem::ParseFailed) when the parse
/// returns no tree. The template cannot be used then. When the template language
/// cannot be built, the result holds that failure ([`minting_diagnostics`]).
pub fn compile_template(
    spec: &TemplateSpec<'_>,
    lang: &BuiltLang,
) -> Result<CompiledTemplate, Vec<FlmDiagnostic>> {
    let language = TemplateLanguage::new(lang)
        .map_err(|errors| minting_diagnostics(spec.definition, &errors))?;
    compile_template_with(spec, lang, &language)
}

/// The lowercase name of a parsing mode (`text`, `math`), for the label of a
/// template's compiled source.
fn mode_name(mode: Mode) -> String {
    format!("{mode:?}").to_ascii_lowercase()
}

/// [`compile_template`] over a template language that is already built. The
/// compiled source is labeled `<origin_label> (<mode> mode)` and has no
/// canonical path, so every diagnostic raised in it identifies the definition;
/// its origin marks it as a user definition's text where
/// [`TemplateSpec::user_definition`] asks for it.
///
/// # Errors
///
/// The same as [`compile_template`], except the failure to build the template
/// language.
pub fn compile_template_with(
    spec: &TemplateSpec<'_>,
    lang: &BuiltLang,
    language: &TemplateLanguage,
) -> Result<CompiledTemplate, Vec<FlmDiagnostic>> {
    let label = format!("{} ({} mode)", spec.origin_label, mode_name(spec.mode));
    let origin =
        if spec.user_definition { FlmOrigin::definition(label) } else { FlmOrigin::synthetic(label) };
    let source: Arc<FlmSource> = Arc::new(Source::new(spec.text.to_string()).with_origin(origin));
    let failed = |detail: String| {
        vec![Diagnostic::error(
            BadDefinition::new(spec.definition, DefinitionProblem::ParseFailed { detail }),
            source_start(&source),
        )]
    };
    let state = language.state(lang, spec.mode).map_err(|error| failed(error.to_string()))?;
    // The layout is immaterial: nothing block-related exists at parse (session
    // 03 Q2), and only the fragment's tree and diagnostics are kept.
    let fragment = parse_fragment_with_state(
        lang,
        Arc::clone(&source),
        FragmentLayout::default(),
        &ParseOptions::default(),
        state,
    )
    .map_err(|error| failed(error.to_string()))?;

    let mut diagnostics: Vec<FlmDiagnostic> = fragment.diags().iter().cloned().collect();
    // Every text payload owned: a copy onto another source carries its text.
    let tree = fragment.tree().materialize();

    let mut placeholders = BTreeMap::new();
    for node in tree.root().descendants() {
        if node.callable_type() != Some(FlmCallableType::Placeholder) {
            continue;
        }
        let written = node.name().unwrap_or("").to_string();
        let substitution = node
            .arguments()
            .and_then(|arguments| arguments.get(0))
            .is_some_and(ParsedArgument::is_provided);
        match spec.vocabulary.resolve(&written, substitution, spec.offset) {
            Ok(target) => {
                placeholders.insert(node.id(), PlaceholderRef { written, target, substitution });
            }
            Err(problem) => diagnostics.push(Diagnostic::error(
                BadPlaceholder::new(spec.definition, written, problem),
                node.span().clone(),
            )),
        }
    }

    if diagnostics.iter().any(|diagnostic| diagnostic.severity() == Severity::Error) {
        return Err(diagnostics);
    }
    Ok(CompiledTemplate {
        text: spec.text.to_string(),
        source,
        tree: Arc::new(tree),
        mode: spec.mode,
        definition: spec.definition.to_string(),
        label: spec.label.to_string(),
        placeholders,
        diagnostics,
    })
}

/// Compiles one text in each of several parsing modes and keeps it in the modes
/// where it compiles: the rule that every text read in more than one mode
/// follows.
///
/// A text is parsed in the mode where it is substituted, so the same text gives
/// one compilation per mode, and it may parse in one mode and not in another:
/// `\bigl (` parses in a formula and not in running text. The rule: the text is
/// kept in each mode where it compiles; a mode where it does not compile is no
/// problem of the text, and a use there that needs it is reported at the use;
/// only when the text compiles in **none** of `modes` are the failures reported.
/// Over a single mode this is simple: the text compiles there, or its failures
/// are reported.
///
/// `compile` performs the compilation in one mode: it returns what the text
/// compiled to, or `None` when it did not compile, and pushes the compilation's
/// diagnostics onto the vector it is handed (its errors when it fails, its
/// warnings when it succeeds).
///
/// Returns the compilations kept, each with its mode, in the order of `modes`,
/// and the diagnostics to report: those of the modes that compiled, or, when
/// none did, those of every mode. A user definition's text declared for both
/// modes follows this rule ([`compile_with`](super::compile_with)); an extension
/// that compiles texts of its own for the modes its templates are read in calls
/// this with [`compile_template_with`] as `compile`.
pub fn compile_in_modes<T>(
    modes: impl IntoIterator<Item = Mode>,
    mut compile: impl FnMut(Mode, &mut Vec<FlmDiagnostic>) -> Option<T>,
) -> (Vec<(Mode, T)>, Vec<FlmDiagnostic>) {
    let mut kept = Vec::new();
    let mut kept_diagnostics = Vec::new();
    let mut failed_diagnostics = Vec::new();
    for mode in modes {
        let mut found = Vec::new();
        match compile(mode, &mut found) {
            Some(compiled) => {
                kept.push((mode, compiled));
                kept_diagnostics.extend(found);
            }
            None => failed_diagnostics.extend(found),
        }
    }
    if kept.is_empty() {
        (kept, failed_diagnostics)
    } else {
        (kept, kept_diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_conditional_selects_the_branch_its_test_answers() {
        let tf = Conditional { test: ConditionalTest::Value, branches: Branches::Both };
        assert_eq!(tf.branch_index(true), Some(1));
        assert_eq!(tf.branch_index(false), Some(2));
        let t = Conditional { test: ConditionalTest::Value, branches: Branches::True };
        assert_eq!(t.branch_index(false), None);
        assert_eq!(t.argument_count(), 2);
    }

    #[test]
    fn the_vocabulary_resolves_a_number_under_the_offset() {
        let vocabulary = PlaceholderVocabulary::new(3);
        assert_eq!(vocabulary.resolve("1", false, 1), Ok(PlaceholderTarget::Argument(1)));
        assert_eq!(vocabulary.resolve("2", false, 1), Ok(PlaceholderTarget::Argument(2)));
        // The offset shortens the range a number may take, and the diagnostic
        // reports what is left referable, not the declared count.
        assert_eq!(
            vocabulary.resolve("3", false, 1),
            Err(PlaceholderProblem::OutOfRange { arity: 2 })
        );
        assert_eq!(
            vocabulary.resolve("0", false, 0),
            Err(PlaceholderProblem::OutOfRange { arity: 3 })
        );
    }

    #[test]
    fn a_named_reference_ignores_the_offset() {
        let vocabulary =
            PlaceholderVocabulary::new(2).names(vec![Some(String::from("a")), Some("b".into())]);
        assert_eq!(vocabulary.resolve("a", false, 1), Ok(PlaceholderTarget::Argument(0)));
        assert_eq!(vocabulary.resolve("1", false, 1), Ok(PlaceholderTarget::Argument(1)));
    }

    #[test]
    fn the_vocabulary_answers_a_hook_name_before_an_argument_name() {
        let vocabulary = PlaceholderVocabulary::new(1)
            .names(vec![Some(String::from("x"))])
            .extra("x", true);
        assert_eq!(
            vocabulary.resolve("x", true, 0),
            Ok(PlaceholderTarget::Extra(String::from("x")))
        );
    }

    #[test]
    fn the_vocabulary_refuses_a_substitution_on_a_base_reference() {
        let vocabulary = PlaceholderVocabulary::new(1).with_body();
        assert_eq!(
            vocabulary.resolve("1", true, 0),
            Err(PlaceholderProblem::SubstitutionNotAccepted)
        );
        assert_eq!(
            vocabulary.resolve(BODY_REFERENCE, true, 0),
            Err(PlaceholderProblem::SubstitutionNotAccepted)
        );
        assert_eq!(vocabulary.resolve(BODY_REFERENCE, false, 0), Ok(PlaceholderTarget::Body));
    }

    #[test]
    fn the_vocabulary_reports_an_unknown_name_and_a_missing_body() {
        let vocabulary = PlaceholderVocabulary::new(0);
        assert_eq!(vocabulary.resolve("nope", false, 0), Err(PlaceholderProblem::UnknownName));
        assert_eq!(vocabulary.resolve(BODY_REFERENCE, false, 0), Err(PlaceholderProblem::NoBody));
    }
}
