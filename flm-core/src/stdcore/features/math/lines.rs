//! Equation lines: the four commands an author writes inside a display-math
//! environment's body (`\\`, `\label`, `\tag` and `\nonumber`), and the reader
//! that splits such a body into lines.
//!
//! An equation line is one line of a display-math environment as its body reads
//! between two `\\`: its mathematics, its labels, its tag and its no-number mark;
//! each line is one reference point ([equation
//! lines](crate::guide::concepts_overview#equation-lines)). An `align` body such
//! as `a &= b \label{eq:first} \\ c &= d \nonumber` holds two lines. The
//! [`math`](super) feature declares one reference point per line and numbers it
//! from what [`equation_lines`] reads here; the lowering rules of the feature
//! that processes mathematics write each line's tag or number, and, on the
//! `latex-recomposed` backend, each line's labels.
//!
//! # The four commands
//!
//! | written | field name | what the line records |
//! |---|---|---|
//! | `\\`, `\\[1ex]` | [`LINEBREAK_COMMAND`] | the end of the line, and the optional spacing as written |
//! | `\label{eq:name}` | [`LABEL_FIELD`](crate::stdcore::features::refs::LABEL_FIELD) | a label of the line |
//! | `\tag{A}`, `\tag*{A}` | [`TAG_COMMAND`] | the tag's content as inline nodes, and whether the star was written |
//! | `\nonumber` | [`NONUMBER_COMMAND`] | the line is not numbered |
//!
//! The four commands are declared in the scoped package
//! ([`ScopedPackageDef`]) [`PACKAGE`] (`flm.math.lines`): a set of element
//! declarations that can be written only inside the environment bodies and
//! arguments whose declarations refer to the package by name ([scoped
//! packages](crate::guide::concepts_overview#scoped-packages)). The bodies of the
//! six display-math environments refer to it, and the declarations are for math
//! mode only, so this module's declarations apply inside
//! `\begin{align}…\end{align}` and the other five environments, and nowhere
//! else. In inline mathematics `\(…\)` and in displayed mathematics `\[…\]`,
//! `\\` and `\tag` are commands this module does not declare (under the
//! [`math-passthrough`](crate::math_passthrough) feature,
//! unknown commands kept as source); in text mode they mean what another feature
//! declares there, if anything.
//!
//! All four are **fields**: macros that a construct reads as data rather than
//! as content ([fields](crate::guide::concepts_overview#fields-data-a-construct-reads)). Each node
//! carries the field element [`FIELD`](crate::lang::mint::FIELD) with the
//! command's name as its value, and the environment's reader, [`equation_lines`],
//! gives them their meaning. Where a command is written inside its line makes no
//! difference to what the line records. The `\label` declared here is a
//! separate declaration from the `\label` that a heading reads after its
//! arguments, but it produces the same field node, with the same field name, so
//! the references feature's reader of a label, [`label_use`](crate::stdcore::features::refs::label_use),
//! reads it unchanged.
//!
//! The optional argument of `\\` must follow the command immediately: in
//! `a \\ [b]` the `[b]` is content of the next line, not spacing. This keeps a
//! bracketed expression at the start of a line from being read as spacing.
//!
//! `\nonumber` is the only way to write the no-number mark. `\notag`, which
//! LaTeX's `amsmath` package also accepts, is not declared: inside a body it is
//! an unknown command, kept as source in the line.
//!
//! The whitespace right after a command's name belongs to the command's node
//! (see [macros](crate::guide::language_syntax#macros) in the syntax guide), so
//! the newline after `\nonumber` is not part of the line's content. The name of
//! `\\` is a single character that is not a letter, which takes no such
//! whitespace, so the newline after it begins the next line's content.
//!
//! # Reading a body as lines
//!
//! [`equation_lines`] reads the direct children of an environment's body once
//! and returns one [`EquationLine`] per line, in order:
//!
//! - A `\\` ends the line being read.
//! - A `\label`, `\tag` or `\nonumber` is taken out of the content and recorded
//!   on the line it is written in. A `\label` written on a line of its own before
//!   the `\\` belongs to the line that `\\` ends; one written right after a `\\`
//!   belongs to the next line. The whitespace around such a command stays in
//!   the content; where the command stood on a line of its own, the LaTeX
//!   writers keep a blank line from appearing through the `math` library's
//!   newline rule ([`normalize_line_newlines`](super::latex::normalize_line_newlines)).
//! - Everything else (mathematics, whitespace, comments, and nested environments
//!   as a whole) is the line's [`content`](EquationLine::content), in order. The
//!   nodes of a nested environment are not examined, so a `\\` inside a `cases`
//!   or `matrix` environment nested in an `align` does not split the outer lines.
//! - A body with no `\\` is one line.
//! - A `\\` followed only by whitespace, blank lines and comments ends the last
//!   line and begins no further one. It is not recorded (the last line's
//!   [`linebreak`](EquationLine::linebreak) is `None`), and the whitespace and
//!   comments after it are added to the last line's content as written. A
//!   backend that writes the lines back therefore never writes a final `\\`,
//!   which would make LaTeX add the space of an empty line after the last
//!   equation. Anything else after the last `\\` (mathematics, a `\label`, a
//!   `\tag` or a `\nonumber`) begins a further line.
//!
//! A line's `linebreak` is therefore `Some` exactly when another line follows it.
//!
//! # The error, and empty lines
//!
//! The body parser of the display-math environments ([`LinesBehavior`]) checks
//! the lines when a body is parsed and reports one condition as an Error:
//! [`LabelWithoutNumber`] (`flm.math.label-without-number`), a `\label` or a
//! `\tag` written on a line that cannot be numbered, a line marked `\nonumber`
//! or any line of `equation*`, `align*` or `gather*`. Each such command is
//! reported once.
//!
//! Whether a line must hold mathematics is a question for the feature that
//! processes mathematics, not for this module: a line holding only whitespace,
//! comments, a `\label`, a `\tag` or a `\nonumber`, a `\\` at the start of a
//! body, two `\\` in a row and an empty body are read here without a report.
//! A processing feature reports each such line from its check stage, as
//! [`math-passthrough`](crate::math_passthrough) does.
//!
//! The tree keeps the body as written, and [`equation_lines`] returns every
//! line, an empty line as an [`EquationLine`] with no content, so that a backend
//! can still write back what the author wrote.
//!
//! # Lowering
//!
//! None of the four commands is lowered on its own. Their nodes are only ever
//! written inside an environment's body, which the environment's lowering rule
//! reads as lines and writes from the lines' data. A field node that reaches a
//! backend on its own is an Error, reported by the only lowering rule of the
//! field element ([`FieldRule`](crate::stdcore::backends::lowering::FieldRule)).
//!
//! # The lines and the numbers
//!
//! The environment declares one reference point per line, whose index is the
//! line's index. A line that can be numbered carries the labels written on it, so
//! `\ref{eq:name}` refers to the line it labels and shows the number the document
//! gave that line; a line that cannot be numbered keeps no label, so nothing can
//! refer to it. A line's `\tag` replaces its number: the reference point states the tag's content ([`Tag::content_ref`]), and the
//! line counts in nothing. The [`math`](super) feature's documentation describes
//! the reference points in full.
//!
//! # Map of the module
//!
//! - [`PACKAGE`], [`package`] — the scoped package of the four commands.
//! - [`LINEBREAK_COMMAND`], [`TAG_COMMAND`], [`NONUMBER_COMMAND`],
//!   [`SPACING_ARG`], [`STAR_ARG`], [`TEXT_ARG`] — the names of the commands and
//!   of their arguments.
//! - [`equation_lines`], [`EquationLine`] — the reader and what it returns for one
//!   line; [`line_break`], [`LineBreak`], [`tag_of`], [`Tag`] — the readers of
//!   one `\\` and one `\tag`.
//! - [`LinesBehavior`], [`body_behavior`] — the body parser of the environments.
//! - [`LabelWithoutNumber`], [`LineCommand`] — the condition the parse reports.

// Why the structure lands before numbering, why the `\label` written here is a
// field like the other three, and why the four commands are scoped to the
// environments' bodies rather than to every mathematics region: see design
// rationale at [§flm-dd-dr:std-math-equation-lines]. The reference point of
// each line: [§flm-dd-dr:std-math-line-points].

use std::fmt;
use std::sync::Arc;

use techy::core::constructs::{
    ArgumentParser, ConstructParser, ConstructParserResult, EnvironmentBody, EnvironmentBodyParser,
    OptionalGroupArgumentParser, ParseContext, ParsedArgumentNodes,
};
use techy::core::node::{BodySlotExt, BuildId, NodeKind, NodeRef, NodeSlice, StagedNodes};
use techy::core::specs::ArgumentSpec;
use techy::core::token::GroupRule;
use techy::core::ParsingStateDelta;
use techy::error::{DiagnosticInfo, DiagnosticValue, ToDiagnosticValue};
use techy::latexlike::{EnvironmentBehavior, EnvironmentInvocation, Mode};
use techy::source::{Source, SourceSpan};

use crate::lang::{Flm, FlmSpan};
use crate::lang::flm::FlmGroupType;
use crate::lang::origin::FlmOrigin;
use crate::lang::structural;
use crate::lang::mint::{
    ArgDef, ArgStdKind, DefinitionShape, ElementDef, Field, FlmArgumentParser, FlmBodyParser,
    ScopedPackageDef, SyntaxDef, field_element,
    is_field_named,
};
use crate::lang::text::is_blank_text;
use crate::model::aspects::ContentRef;
use crate::model::element::ElementSpec;
use crate::model::names::{FeatureName, Name};
use crate::stdcore::features::refs::{self, LabelUse};

/// The name of the scoped package that holds the four commands,
/// `flm.math.lines`: the body declaration of each display-math environment
/// refers to it, which makes the commands available inside the body.
pub const PACKAGE: &str = "flm.math.lines";

/// The name of the line-break field, a single backslash: the author writes
/// it after the escape character, so the command reads `\\`.
pub const LINEBREAK_COMMAND: &str = "\\";

/// The name of the tag field, `tag`.
pub const TAG_COMMAND: &str = "tag";

/// The name of the no-number field, `nonumber`.
pub const NONUMBER_COMMAND: &str = "nonumber";

/// The name of the optional argument of `\\`, `spacing`: the vertical space
/// added after the line, as in `\\[1ex]`.
pub const SPACING_ARG: &str = "spacing";

/// The name of the star argument of `\tag`, `star`: when it is written
/// (`\tag*`), the tag is shown without parentheses.
pub const STAR_ARG: &str = "star";

/// The name of the mandatory argument of `\tag`, `text`: the tag's content,
/// read in text mode.
pub const TEXT_ARG: &str = "text";

/// The syntax of `\\`: one optional `[…]` argument named [`SPACING_ARG`],
/// which must follow the command immediately and is read by [`SpacingParser`];
/// its content is kept as written.
fn linebreak_syntax() -> SyntaxDef {
    SyntaxDef::Macro { args: vec![ArgDef::named(SPACING_ARG, SpacingParser::new())] }
}

/// The parser of the spacing argument of `\\` ([`SPACING_ARG`]): an optional
/// `[…]` group that must follow the command immediately, as in `\\[1ex]`.
///
/// An optional argument of a standard kind ([`ArgStdKind::Optional`]) may be
/// written after whitespace, so the argument uses this parser instead: techy's
/// parser of an optional group, required to be adjacent, so that only a `[`
/// written directly after the command opens the argument. A single group
/// inside the brackets is not unwrapped, so `\\[{1ex}]` records its spacing
/// as `{1ex}`, exactly as written.
///
/// Otherwise the parser behaves as the parser of an optional group: the
/// argument is read in the mode of the body it is written in (math mode),
/// declares no field, and is written as a value when a user definition is
/// recomposed. Its identity is its own, distinct from the optional group's.
#[derive(Debug)]
struct SpacingParser(OptionalGroupArgumentParser<Flm>);

impl SpacingParser {
    /// The parser of `[…]` directly after the command.
    fn new() -> SpacingParser {
        let brackets = Arc::new(GroupRule {
            group_type: FlmGroupType::Content,
            open: String::from("["),
            close: String::from("]"),
        });
        SpacingParser(OptionalGroupArgumentParser::<Flm>::new(brackets).require_adjacent())
    }
}

impl ArgumentParser<Flm> for SpacingParser {
    fn parse_argument(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
        spec: &ArgumentSpec<Flm>,
    ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
        self.0.parse_argument(cx, spec)
    }

    fn can_match_empty(&self) -> bool {
        self.0.can_match_empty()
    }
}

impl FlmArgumentParser for SpacingParser {
    fn identity(&self) -> Vec<u8> {
        b"flm.math.lines.spacing".to_vec()
    }

    fn definition_shape(&self) -> DefinitionShape {
        DefinitionShape::Value
    }

    fn description(&self) -> String {
        String::from("an optional `[…]` group written directly after the command")
    }
}

/// The declaration of `\\`.
fn linebreak_def(owner: &FeatureName) -> ElementDef {
    ElementDef::new(
        owner.clone(),
        [LINEBREAK_COMMAND],
        linebreak_syntax(),
        field_element(
            LINEBREAK_COMMAND,
            "A `\\\\` inside a display-math environment: the end of an equation line, \
             with the optional vertical spacing written after it.",
        ),
    )
    .modes([Mode::Math])
    .docs("`\\\\[spacing]`: ends an equation line; the optional argument is vertical spacing.")
}

/// The declaration of `\tag` and `\tag*`.
fn tag_def(owner: &FeatureName) -> ElementDef {
    ElementDef::new(
        owner.clone(),
        [TAG_COMMAND],
        SyntaxDef::Macro {
            args: vec![
                ArgDef::named(STAR_ARG, ArgStdKind::Star)
                    .docs("A star: the text is shown as written, without parentheses."),
                ArgDef::named(TEXT_ARG, ArgStdKind::Mandatory)
                    .mode(Mode::Text)
                    .docs("The tag's text, as ordinary document content."),
            ],
        },
        field_element(
            TAG_COMMAND,
            "A `\\tag{…}` or `\\tag*{…}` inside a display-math environment: a custom tag \
             for the equation line, replacing its number.",
        ),
    )
    .modes([Mode::Math])
    .docs(
        "`\\tag{text}`, `\\tag*{text}`: a custom tag for the line, shown in parentheses \
         unless starred. The text is read in text mode, so it is ordinary document \
         content: a command inside it is read as a command, and a character an output \
         format must escape is escaped.",
    )
}

/// The declaration of `\nonumber`.
fn nonumber_def(owner: &FeatureName) -> ElementDef {
    ElementDef::new(
        owner.clone(),
        [NONUMBER_COMMAND],
        SyntaxDef::Macro { args: Vec::new() },
        field_element(
            NONUMBER_COMMAND,
            "A `\\nonumber` inside a display-math environment: the equation line is not \
             to be numbered.",
        ),
    )
    .modes([Mode::Math])
    .docs("`\\nonumber`: the line is not numbered.")
}

/// The declaration of `\label{…}`: a macro whose only argument, a group of
/// characters, has the name that the `refs` feature gives it
/// ([`TARGET_ARG`](crate::stdcore::features::refs::TARGET_ARG)), so that that
/// feature's reader of a label node reads it, and whose element is the field
/// of the same name as the `\label` a heading reads.
fn label_def(owner: &FeatureName) -> ElementDef {
    ElementDef::new(
        owner.clone(),
        [refs::LABEL_FIELD],
        SyntaxDef::Macro {
            args: vec![
                ArgDef::named(refs::TARGET_ARG, ArgStdKind::CharsGroup)
                    .docs("The label, `type:label` (`eq:main`).")
                    .role(refs::DEFINED_LABEL_ARG_ROLE),
            ],
        },
        field_element(
            refs::LABEL_FIELD,
            "A `\\label{type:label}` inside a display-math environment: a label on the \
             equation line it is written on.",
        ),
    )
    .modes([Mode::Math])
    .docs("`\\label{type:label}`: attaches a label to the equation line it is written on.")
}

/// The scoped package [`PACKAGE`] with the declarations of the four commands,
/// owned by `owner`: what the `math` feature registers, and what the body of
/// each display-math environment refers to.
///
/// The registry's feature builder attributes the package and its declarations
/// to the feature that registers it, regardless of `owner`; the parameter
/// matters only for a package built outside a registry.
pub fn package(owner: impl Into<FeatureName>) -> ScopedPackageDef {
    let owner = owner.into();
    ScopedPackageDef {
        owner: owner.clone(),
        name: Name::from(PACKAGE),
        elements: vec![
            linebreak_def(&owner),
            tag_def(&owner),
            nonumber_def(&owner),
            label_def(&owner),
        ],
        docs: String::from(
            "The commands of a display-math environment's body: `\\\\`, `\\label`, `\\tag` \
             and `\\nonumber`.",
        ),
    }
}

// --- the environment behavior and its error -----------------------------------------------

/// The command that a [`LabelWithoutNumber`] condition reports: `\label` or
/// `\tag`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCommand {
    /// A `\label`.
    Label,
    /// A `\tag`.
    Tag,
}

impl LineCommand {
    /// The command's name without the escape character: `label` or `tag`.
    /// The diagnostic's data holds the command under this name.
    pub fn name(self) -> &'static str {
        match self {
            LineCommand::Label => "label",
            LineCommand::Tag => "tag",
        }
    }
}

impl fmt::Display for LineCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl ToDiagnosticValue for LineCommand {
    fn to_diagnostic_value(&self) -> DiagnosticValue {
        DiagnosticValue::Str(String::from(self.name()))
    }
}

/// Condition `flm.math.label-without-number` (an Error): a `\label` or a
/// `\tag` is written on an equation line that cannot be numbered, a line
/// marked `\nonumber` or any line of a starred environment (`equation*`,
/// `align*`, `gather*`).
///
/// [`LinesBehavior`] reports it when the body is parsed, at the command, once
/// per such command. The recovery is to keep the body as written:
/// [`equation_lines`] records the label or the tag on its line, and the line's
/// reference point keeps no label (see [`MathEnvironment`](super::MathEnvironment)).
/// See the module documentation, *The error, and empty lines*.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.math.label-without-number",
    message = "`\\{command}` must not be written on a line marked `\\nonumber` or in a \
               starred environment; line {line} (counting from 0) of the `{environment}` \
               environment is such a line"
)]
pub struct LabelWithoutNumber {
    /// The environment's name as written (`align*`).
    pub environment: String,
    /// The index of the line among the environment's lines, counting from zero.
    pub line: usize,
    /// The command written on the line: `label` or `tag`.
    pub command: LineCommand,
}

/// The body parser ([`FlmBodyParser`]) of the six display-math environments:
/// techy's ordinary parse of an environment body, up to and including
/// `\end{name}`, followed by the check of the equation lines, which reports
/// [`LabelWithoutNumber`] among the parse's diagnostics. See the module
/// documentation, *The error, and empty lines*.
///
/// The parser records whether the environment numbers its lines (`align` does,
/// `align*` does not). The body's parsing mode and its scoped package come
/// from the body declaration that carries the parser
/// ([`BodyDef`](crate::lang::mint::BodyDef)). Build one with
/// [`body_behavior`].
#[derive(Debug)]
pub struct LinesBehavior {
    numbered: bool,
}

/// The [`LinesBehavior`] of a display-math environment, `numbered` saying
/// whether the environment numbers its lines: the body parser that the
/// environment's body declaration carries
/// ([`BodyDef::parser`](crate::lang::mint::BodyDef::parser)).
pub fn body_behavior(numbered: bool) -> LinesBehavior {
    LinesBehavior { numbered }
}

impl FlmBodyParser for LinesBehavior {
    fn identity(&self) -> Vec<u8> {
        let mut out = b"flm.math.lines".to_vec();
        out.push(u8::from(self.numbered));
        out
    }

    fn description(&self) -> String {
        String::from("equation lines separated by `\\\\`, checked as they are read")
    }
}

impl EnvironmentBehavior<Flm> for LinesBehavior {
    fn make_body_parser<'p>(
        &'p self,
        invocation: EnvironmentInvocation<'p, Flm>,
    ) -> Box<dyn ConstructParser<Flm, Output = EnvironmentBody<Flm>> + 'p> {
        Box::new(LinesBodyParser { invocation, numbered: self.numbered })
    }
}

/// The body parser that [`LinesBehavior`] creates for one environment: the
/// ordinary body parse, then the check of the lines over the parsed body.
struct LinesBodyParser<'p> {
    invocation: EnvironmentInvocation<'p, Flm>,
    numbered: bool,
}

impl ConstructParser<Flm> for LinesBodyParser<'_> {
    type Output = EnvironmentBody<Flm>;

    fn parse(
        &mut self,
        cx: &mut ParseContext<'_, '_, Flm>,
    ) -> ConstructParserResult<Flm, (EnvironmentBody<Flm>, Option<Box<ParsingStateDelta<Flm>>>)> {
        let invocation = &self.invocation;
        let mut inner = EnvironmentBodyParser::new(
            invocation.trigger_span.clone(),
            invocation.name,
            invocation.end_command_name,
            FlmGroupType::Content,
        )
        .with_invocation_name_span(invocation.name_span.clone());
        let (body, delta) = inner.parse(cx)?;
        // The staged nodes are read first and reported after: reading borrows the
        // context, reporting needs it mutably.
        let faults = match cx.staged_nodes().get(body.body) {
            Some(list) => {
                let staged = cx.staged_nodes();
                line_faults(staged, list.children(), invocation.name, self.numbered)
            }
            None => {
                return Err(cx.implementation_error(
                    "the body parser of a display-math environment returned an unstaged body",
                    invocation.trigger_span.clone(),
                ));
            }
        };
        for (condition, span) in faults {
            cx.recover(condition, span)?;
        }
        Ok((body, delta))
    }
}

/// The labels and tags the line check finds on lines that cannot be numbered,
/// each with the span it is reported at: `children` are the body's direct
/// children, `environment` the environment's name and `numbered` whether it
/// numbers its lines. The body is split into lines as [`equation_lines`]
/// splits it.
fn line_faults(
    staged: StagedNodes<'_, Flm>,
    children: &[BuildId],
    environment: &str,
    numbered: bool,
) -> Vec<(LabelWithoutNumber, FlmSpan)> {
    let pieces = children.iter().filter_map(|id| {
        let node = staged.get(*id)?;
        let piece = piece_of(node.kind(), &node.ext().element, node.span().source());
        Some((piece, node.span().clone()))
    });
    let mut faults = Vec::new();
    for (line, segment) in split(pieces).into_iter().enumerate() {
        let unnumbered = !numbered || segment.items.iter().any(|(piece, _)| *piece == Piece::NoNumber);
        if unnumbered {
            for (piece, span) in &segment.items {
                let command = match piece {
                    Piece::Label => LineCommand::Label,
                    Piece::Tag => LineCommand::Tag,
                    _ => continue,
                };
                let condition =
                    LabelWithoutNumber { environment: String::from(environment), line, command };
                faults.push((condition, span.clone()));
            }
        }
    }
    faults
}

/// The equation lines of a display-math environment that hold no mathematics,
/// each as its index among the lines (counting from zero) and the span a
/// report of it points at: the line's `\label`, `\tag` and `\nonumber`
/// commands (from the first to the last), or the `\\` that ends the line when
/// it holds none, or the environment's body when the body is entirely empty.
///
/// Whitespace (a blank line included), comments and the four commands of the
/// body are not mathematics. The body is split into lines as
/// [`equation_lines`] splits it, so the indices are those of the list it
/// returns. A node with no body has no lines.
///
/// A feature that processes mathematics reports these lines, for example
/// [`math-passthrough`](crate::math_passthrough).
pub(crate) fn empty_lines(node: &NodeRef<'_, Flm>) -> Vec<(usize, FlmSpan)> {
    let Some(body) = node.body() else {
        return Vec::new();
    };
    let pieces = body.iter().map(|child| {
        let piece = piece_of(child.kind(), &child.ext().element, child.span().source());
        (piece, child.span().clone())
    });
    let mut empty = Vec::new();
    for (line, segment) in split(pieces).into_iter().enumerate() {
        if segment.items.iter().any(|(piece, _)| *piece == Piece::Content) {
            continue;
        }
        let commands: Vec<&FlmSpan> = segment
            .items
            .iter()
            .filter(|(piece, _)| *piece != Piece::Blank)
            .map(|(_, span)| span)
            .collect();
        let span = match (cover(&commands), &segment.linebreak) {
            (Some(span), _) => span,
            (None, Some(brk)) => brk.clone(),
            (None, None) => body_span(node),
        };
        empty.push((line, span));
    }
    empty
}

/// The span of a node's body: the span of the node that wraps the body's
/// content, where the parse made one, and otherwise the span the content
/// covers; the node's own span when neither is known.
fn body_span(node: &NodeRef<'_, Flm>) -> FlmSpan {
    let wrapper = node.callable().and_then(|callable| {
        let index = callable.slots.iter().position(|slot| slot.ext.is_body())?;
        node.slot_content_parent(index)
    });
    if let Some(wrapper) = wrapper {
        return wrapper.span().clone();
    }
    node.body().and_then(|body| body.span()).unwrap_or_else(|| node.span().clone())
}

/// The span from the start of the first of `spans` to the end of the last, when
/// they lie in one source; the first span alone otherwise; `None` for no span.
fn cover(spans: &[&FlmSpan]) -> Option<FlmSpan> {
    let first = spans.first()?;
    let last = spans.last()?;
    if first.same_source(last) {
        Some(SourceSpan::new(first.source(), first.start()..last.end()))
    } else {
        Some((*first).clone())
    }
}

// --- reading -------------------------------------------------------------------------------

/// The text a run of nodes was written as: the source they cover, taken as
/// one slice when the nodes cover one source without gaps, and otherwise each
/// node's own source text, joined.
fn written(nodes: &NodeSlice<'_, Flm>) -> String {
    match nodes.source_text() {
        Some(text) => text.to_string(),
        None => nodes.iter().map(|node| node.span_content()).collect(),
    }
}

/// A `\\` node, as [`line_break`] reads it: the node and its spacing.
#[derive(Debug, Clone)]
pub struct LineBreak<'t> {
    /// The `\\` node.
    pub node: NodeRef<'t, Flm>,
    /// The optional argument's content exactly as written, `1ex` for
    /// `\\[1ex]`; `None` when no argument was written.
    pub spacing: Option<String>,
}

/// A `\tag` node, as [`tag_of`] reads it: the node, its content and whether
/// the star was written.
#[derive(Debug, Clone)]
pub struct Tag<'t> {
    /// The `\tag` node.
    pub node: NodeRef<'t, Flm>,
    /// The nodes of the argument's content, in order: the tag's text as
    /// document content, lowered like any other inline content, so that a
    /// command inside it is lowered by its own rule and a character that the
    /// output format must escape is escaped.
    pub content: Vec<NodeRef<'t, Flm>>,
    /// Whether the star was given (`\tag*{…}`).
    pub star: bool,
}

impl Tag<'_> {
    /// Whether the tag is shown in parentheses: `\tag{A}` shows `(A)`, and
    /// `\tag*{A}` shows `A` alone.
    ///
    /// This is how FLM reads the two forms: the unstarred tag adds the
    /// parentheses a number has, and the starred one shows the content alone.
    /// The rules of the [`math`](super) feature write `\tag*{…}` around the
    /// content and add the parentheses themselves where this returns `true`,
    /// so that LaTeX adds nothing further.
    pub fn parenthesized(&self) -> bool {
        !self.star
    }

    /// The tag's content exactly as it was written, for a consumer that wants
    /// the source text rather than the nodes.
    pub fn written(&self) -> String {
        self.content.iter().map(|node| node.span_content()).collect()
    }

    /// The node that the tag's text argument was written as (the brace group
    /// of `\tag{a note}`, the single token of `\tag 1`), as a content
    /// reference ([`ContentRef`]): the way an aspect refers to content inside
    /// the node it declares something about.
    ///
    /// The environment's reference points state it as the content of a tagged
    /// line, so that a `\ref` to that line shows what the tag shows. It refers
    /// to the argument as written, delimiters included, rather than to the
    /// [`content`](Tag::content) nodes inside it, because a content reference
    /// refers to one node, and a brace group shows exactly what it holds on
    /// every backend.
    ///
    /// `None` when the argument is absent (after a parse recovery), or when it
    /// was written as more than one node.
    pub fn content_ref(&self) -> Option<ContentRef> {
        let region = self.node.argument_nodes_named(TEXT_ARG).ok()??;
        let mut nodes = region.iter();
        let first = nodes.next()?;
        nodes.next().is_none().then(|| ContentRef::Node(first.id()))
    }
}

/// One equation line of a display-math environment, as [`equation_lines`]
/// reads it: its content, labels, tag, no-number mark and line break. The
/// module documentation, *Reading a body as lines*, says how a body is split
/// into lines.
#[derive(Debug, Clone, Default)]
pub struct EquationLine<'t> {
    /// The line's content, in order: every direct child of the body in this
    /// line that is not a `\\`, a `\label`, a `\tag` or a `\nonumber`
    /// (mathematics, whitespace, comments, and nested environments as a
    /// whole). On the last line this includes, as written, the whitespace and
    /// comments after a final `\\` that is not recorded (see the module
    /// documentation, *Reading a body as lines*).
    pub content: Vec<NodeRef<'t, Flm>>,
    /// The labels written on the line, in order, each as written and, when its
    /// text has the form `type:label`, parsed. The environment declares them
    /// on the line's reference point, and its recomposition rule for the
    /// `latex-recomposed` backend writes them at the line's end.
    pub labels: Vec<LabelUse>,
    /// The tag, when one was written; when a line has several, the last
    /// one.
    pub tag: Option<Tag<'t>>,
    /// Whether `\nonumber` was written on the line.
    pub nonumber: bool,
    /// The `\\` that ends the line: `Some` exactly when another line follows
    /// this one. `None` for the last line, whether or not a `\\` was written
    /// after it: a final `\\` followed only by whitespace and comments is not
    /// recorded, and that whitespace and those comments are part of the
    /// line's [`content`](EquationLine::content) (see the module
    /// documentation, *Reading a body as lines*).
    pub linebreak: Option<LineBreak<'t>>,
}

/// Reads a `\\` node: `Some` with its spacing when `node` is the line-break
/// field of this module, `None` for any other node.
pub fn line_break<'t>(node: &NodeRef<'t, Flm>) -> Option<LineBreak<'t>> {
    if !is_field_named(node, LINEBREAK_COMMAND) {
        return None;
    }
    let provided = node
        .arguments()
        .and_then(|args| args.get_named(SPACING_ARG))
        .is_some_and(|arg| arg.is_provided());
    let spacing = provided.then(|| {
        node.argument_content_nodes_named(SPACING_ARG)
            .ok()
            .flatten()
            .map(|content| written(&content))
            .unwrap_or_default()
    });
    Some(LineBreak { node: *node, spacing })
}

/// Reads a `\tag` node: `Some` with its content and its star when `node` is
/// the tag field of this module, `None` for any other node. A tag whose text
/// argument is absent (after a parse recovery) has no content.
pub fn tag_of<'t>(node: &NodeRef<'t, Flm>) -> Option<Tag<'t>> {
    if !is_field_named(node, TAG_COMMAND) {
        return None;
    }
    let star = node
        .arguments()
        .and_then(|args| args.get_named(STAR_ARG))
        .is_some_and(|arg| arg.is_provided());
    let content = node
        .argument_content_nodes_named(TEXT_ARG)
        .ok()
        .flatten()
        .map(|content| content.iter().collect())
        .unwrap_or_default();
    Some(Tag { node: *node, content, star })
}

/// The class of a direct child of a body for the split into lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Piece {
    /// A `\\`: the end of a line.
    Break,
    /// A `\label`.
    Label,
    /// A `\tag`.
    Tag,
    /// A `\nonumber`.
    NoNumber,
    /// A comment, a paragraph break, or a run of characters holding only
    /// whitespace: content of the line, but not mathematics.
    Blank,
    /// Anything else: mathematics, a group, a nested environment.
    Content,
}

/// Classifies one child of a body for the split, from its node kind, its
/// element and the source its text is read from.
///
/// The four commands are fields, and this is where the environment, the
/// construct that holds them, gives each of them its meaning. A field of any
/// other name is ordinary content of the line.
fn piece_of(kind: &NodeKind<Flm>, element: &ElementSpec, source: &Source<FlmOrigin>) -> Piece {
    let field = element.value_as::<Field>().map(|field| field.name.as_str());
    if let Some(name) = field {
        if name == LINEBREAK_COMMAND {
            return Piece::Break;
        } else if name == refs::LABEL_FIELD {
            return Piece::Label;
        } else if name == TAG_COMMAND {
            return Piece::Tag;
        } else if name == NONUMBER_COMMAND {
            return Piece::NoNumber;
        }
    }
    if element.name.as_str() == structural::PARBREAK {
        // A blank line inside the body is kept as a paragraph-break node; to the
        // lines it is whitespace.
        Piece::Blank
    } else {
        match kind {
            NodeKind::Comment(_) => Piece::Blank,
            NodeKind::Chars { content } if is_blank_text(content.resolve(source)) => Piece::Blank,
            _ => Piece::Content,
        }
    }
}

/// One line as the split produces it: its children other than the `\\`, each
/// with its class, and the `\\` that ends it (`None` for the last line).
struct Segment<T> {
    items: Vec<(Piece, T)>,
    linebreak: Option<T>,
}

impl<T> Default for Segment<T> {
    fn default() -> Segment<T> {
        Segment { items: Vec::new(), linebreak: None }
    }
}

/// Splits a body into lines; [`equation_lines`], [`empty_lines`] and the
/// parse-time check use it. A `\\` ends a line. A `\\` followed only by blank children ends the
/// body: it is removed from the line it ended, and the blank children after it
/// are appended to that line in order. Anything else after the last `\\`
/// begins a further line, which also holds the blank children between the
/// `\\` and it. See the module documentation, *Reading a body as lines*.
fn split<T>(pieces: impl IntoIterator<Item = (Piece, T)>) -> Vec<Segment<T>> {
    let mut lines = Vec::new();
    let mut line = Segment::default();
    // Whether only blank pieces have followed the last `\\`: then the body ends
    // with that line break.
    let mut ends_with_break = false;
    for (piece, value) in pieces {
        match piece {
            Piece::Break => {
                line.linebreak = Some(value);
                lines.push(std::mem::take(&mut line));
                ends_with_break = true;
            }
            Piece::Blank => line.items.push((piece, value)),
            _ => {
                ends_with_break = false;
                line.items.push((piece, value));
            }
        }
    }
    if ends_with_break {
        // Only blank pieces followed the last `\\`: drop it, and give those
        // pieces to the line it ended, in order.
        if let Some(last) = lines.last_mut() {
            last.linebreak = None;
            last.items.append(&mut line.items);
        }
    } else {
        lines.push(line);
    }
    lines
}

/// The equation lines of a display-math environment's body: one
/// [`EquationLine`] per line, in order. The module documentation, *Reading a
/// body as lines*, says exactly how the body is split.
///
/// `node` is normally a node of the element [`ENVIRONMENT`](super::ENVIRONMENT);
/// any node with a body is read the same way. A node with no body returns no
/// lines. An environment with an empty body has a body and returns one empty
/// line. A final `\\` followed
/// only by whitespace and comments is not recorded, and that whitespace and
/// those comments are part of the last line's
/// [`content`](EquationLine::content), as written.
pub fn equation_lines<'t>(node: &NodeRef<'t, Flm>) -> Vec<EquationLine<'t>> {
    let Some(body) = node.body() else {
        return Vec::new();
    };
    let pieces = body.iter().map(|child| {
        let piece = piece_of(child.kind(), &child.ext().element, child.span().source());
        (piece, child)
    });
    split(pieces)
        .into_iter()
        .map(|segment| {
            let mut line = EquationLine::default();
            for (piece, child) in segment.items {
                match piece {
                    Piece::Label => {
                        if let Some(label) = refs::label_use(&child) {
                            line.labels.push(label);
                        }
                    }
                    Piece::Tag => line.tag = tag_of(&child),
                    Piece::NoNumber => line.nonumber = true,
                    Piece::Blank | Piece::Content => line.content.push(child),
                    Piece::Break => {}
                }
            }
            line.linebreak = segment.linebreak.as_ref().and_then(line_break);
            line
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::mint::FIELD;

    #[test]
    fn the_package_declares_the_four_commands_in_math_mode_only() {
        let package = package("math");
        assert_eq!(package.name, PACKAGE);
        let names: Vec<Vec<String>> = package
            .elements
            .iter()
            .map(|def| def.names.iter().map(|n| n.to_string()).collect())
            .collect();
        assert_eq!(names, [vec!["\\"], vec!["tag"], vec!["nonumber"], vec!["label"]]);
        assert!(package.elements.iter().all(|def| def.modes == vec![Mode::Math]));
        assert!(package.elements.iter().all(|def| def.owner == "math"));
    }

    #[test]
    fn every_declaration_is_a_field_named_for_the_command() {
        let package = package("math");
        let fields: Vec<&str> = package
            .elements
            .iter()
            .map(|def| {
                assert_eq!(def.element.name, FIELD, "every line command is a field");
                def.element.value_as::<Field>().expect("valued").name.as_str()
            })
            .collect();
        assert_eq!(fields, [LINEBREAK_COMMAND, TAG_COMMAND, NONUMBER_COMMAND, refs::LABEL_FIELD]);
    }

    #[test]
    fn the_split_drops_a_final_break_and_opens_a_line_for_a_command_after_it() {
        use Piece::*;
        let pieces = |seq: &[Piece]| seq.iter().enumerate().map(|(i, p)| (*p, i)).collect::<Vec<_>>();
        // `a \\ %% c` : one line, its break dropped, the blank after it on that line.
        let lines = split(pieces(&[Content, Break, Blank]));
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].items.iter().map(|(p, v)| (*p, *v)).collect::<Vec<_>>(), [(Content, 0), (Blank, 2)]);
        assert!(lines[0].linebreak.is_none());
        // `a \\ \label{x}` : two lines, the first keeps its break.
        let lines = split(pieces(&[Content, Break, Blank, Label]));
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].linebreak, Some(1));
        assert_eq!(lines[1].items.iter().map(|(p, _)| *p).collect::<Vec<_>>(), [Blank, Label]);
        assert!(lines[1].linebreak.is_none());
        // `\\ \\ %% c` : two empty lines; the second break is the final one,
        // and the blank after it is on the second line.
        let lines = split(pieces(&[Break, Blank, Break, Blank]));
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].linebreak, Some(0));
        assert_eq!(lines[1].items.iter().map(|(p, v)| (*p, *v)).collect::<Vec<_>>(), [(Blank, 1), (Blank, 3)]);
        assert!(lines[1].linebreak.is_none());
        // nothing at all: one empty line.
        assert_eq!(split(pieces(&[])).len(), 1);
    }

}
