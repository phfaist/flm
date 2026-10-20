//! The parser of an annotator's one argument, and the function that reads
//! which form an annotation was written in.
//!
//! An annotator's command takes one argument, written in one of three forms
//! that the parser tells apart by the first character after the command and
//! any whitespace and source comments that follow it:
//!
//! - `{` — a braced group, `\phf{…}`: a **highlight** of what the braces hold;
//! - `[` — a bracketed group, `\phf[…]`: a **comment**, the brackets holding
//!   the remark;
//! - anything else — the content up to the command `\endphf` (`end` followed
//!   by the command's name), `\phf … \endphf`: a **highlight** of that
//!   content.
//!
//! In every form, the argument is built as **one group node** holding the
//! content, and the group records the delimiters as they were read: `{` and
//! `}`; `[` and `]`; or no opening delimiter and `\endphf` as the closing
//! one. [`form_of`] reads the form from that record, and the recomposed
//! LaTeX writes the delimiters back from it; both read the record with the
//! same function ([`delimiters_of`]).

use std::cell::Cell;
use std::fmt;
use std::sync::Arc;

use techy::core::constructs::{
    scan_argument_noise, ArgumentParser, ChildStateSpec, ConstructParserResult,
    GroupArgumentParser, OptionalGroupArgumentParser, ParseContext, ParsedArgumentNodes,
    StopCause, StopSpec, TokenStopKind,
};
use techy::core::node::{ContentNodes, GroupData, NodeBuildError, NodeKind, NodeRef};
use techy::core::specs::ArgumentSpec;
use techy::core::token::{GroupRule, Token, TokenKind, TokenReader};
use techy::error::{HookFailed, ParseError};
use techy::source::TextContent;

use crate::lang::mint::{DefinitionShape, FlmArgumentParser};
use crate::lang::{Flm, FlmGroupType, FlmOrigin, FlmSpan};

use super::diagnostics::Unterminated;
use super::TEXT_ARG;

/// The name of the command that ends an environment, `\end{…}`: the
/// language's environments are those of techy's built-in package, whose
/// closing command is `end`.
const ENVIRONMENT_END: &str = "end";

/// The kind of an annotation, highlight or comment, as [`form_of`] reads it
/// from the way its argument was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Form {
    /// A passage the annotator marks, which stays part of the text: written
    /// `\phf{…}` or `\phf … \endphf`.
    Highlight,
    /// A remark the annotator writes, which is not part of the text: written
    /// `\phf[…]`.
    Comment,
}

/// Returns the form of the annotation `node` (a node of an annotator's
/// command, or a cut part of one), read from the opening delimiter that its
/// argument's group recorded: `[` makes a [comment](Form::Comment), and `{` or
/// no delimiter at all (the passage closed by `\endphf`) a
/// [highlight](Form::Highlight).
///
/// Every lowering rule of the element, on every backend, reads the form with
/// this function, and so does the element's [`ApartFromText`] aspect.
/// Returns `None` when `node` has no argument named `text` holding a group,
/// which never happens for a node of the element.
///
/// [`ApartFromText`]: crate::model::aspects::ApartFromText
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
/// use flm_core::lang::{parse_fragment, FlmSource, ParseOptions};
/// use flm_core::model::doc::FragmentLayout;
/// use flm_core::stdcore::features::annotations::{form_of, AnnotationsConfig, Annotator, Form};
/// use flm_core::stdcore::{package, StdcoreConfig};
///
/// let config = AnnotationsConfig::new().annotator(Annotator::new("phf").initials("PhF"));
/// let mut builder = RegistryBuilder::new();
/// builder.use_package(package(StdcoreConfig::new().annotations(config)));
/// builder.use_package(flm_core::math_passthrough::package());
/// let registry = builder.build(&RegistryInputs::new())?;
/// let source = Arc::new(FlmSource::new(r"\phf{a} \phf[b] \phf c\endphf"));
/// let fragment = parse_fragment(registry.lang(), source, FragmentLayout::Inline, &ParseOptions::default())?;
/// let forms: Vec<_> = fragment
///     .tree()
///     .root()
///     .descendants()
///     .filter(|node| node.name() == Some("phf"))
///     .filter_map(|node| form_of(&node))
///     .collect();
/// assert_eq!(forms, [Form::Highlight, Form::Comment, Form::Highlight]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn form_of(node: &NodeRef<'_, Flm>) -> Option<Form> {
    let (open, _) = delimiters_of(node)?;
    Some(if open == "[" { Form::Comment } else { Form::Highlight })
}

/// Returns the delimiters the argument of the annotation `node` was written
/// with, as its group records them: `("{", "}")`, `("[", "]")`, or
/// `("", "\endphf")` followed by the whitespace the closing command read
/// (`("", "")` where the passage was not closed). Every cut part of a cut
/// annotation carries its annotation's two delimiters.
///
/// [`form_of`] decides the form from these delimiters, and the recomposed
/// LaTeX writes the invocation back with them. Returns `None` where
/// [`form_of`] returns `None`.
pub(super) fn delimiters_of<'t>(node: &NodeRef<'t, Flm>) -> Option<(&'t str, &'t str)> {
    let region = node.arguments()?.get_named(TEXT_ARG)?.region.as_ref()?;
    let group = node.tree().get(region.content_parent())?;
    group.group_delimiters()
}

/// The parser of an annotator's one argument: a `{…}` group, a `[…]` group,
/// or the passage up to the annotator's closing command, chosen by the first
/// character after the command and any whitespace and source comments that
/// follow it. The feature declares the argument [`TEXT_ARG`]
/// of every annotator's command with this parser.
///
/// **Before the group.** As for a mandatory `{…}` argument of any command,
/// whitespace and source comments may be written between the command and the
/// opening `{` or `[`; they become the leading nodes of the argument. (A
/// space right after the command's name belongs to the command, as it does
/// after every command whose name is made of letters.) When neither `{` nor
/// `[` follows them, they are not skipped: they are the start of the passage.
///
/// **The passage.** The third form reads document content, paragraph breaks
/// and blocks included, up to the command `\end<command>` written at the
/// passage's own level, which is consumed and closes the group. An
/// `\end<command>` inside a group or an environment that the passage opened
/// belongs to that group or environment, where it is not declared. When the
/// input, or the group or environment enclosing the annotation, ends first
/// (at the input's end, at a closing brace, or at an `\end` written at the
/// passage's own level), the passage is what was read up to there, and the
/// Error [`flm.annotations.unterminated`](super::Unterminated) is reported
/// over it; the closing brace or the `\end` is left for the group or
/// environment it closes, which stays intact. (An `\end` at the passage's own
/// level can only close an environment the passage did not open: the body of
/// an environment the passage opened is read by that environment, where the
/// passage does not stop.)
///
/// **The groups.** A `[…]` group is read as an optional argument's group is,
/// and an unclosed `{` or `[` is reported by the language as any unclosed
/// group is. A `{…}` group alone inside the brackets is kept as a group of
/// the remark, not unwrapped.
///
/// In every form, the argument is one group node holding the content, and
/// the group records what was read as its delimiters ([`form_of`] reads
/// them). The argument's parsing mode and scoped packages come from its
/// declaration and apply to the whole content.
pub struct AnnotationParser {
    command: String,
    end_command: String,
    braces: GroupArgumentParser<Flm>,
    brackets: OptionalGroupArgumentParser<Flm>,
}

impl AnnotationParser {
    /// Returns the parser of the argument of the command `command` (without
    /// the backslash), whose passage form closes at `\end<command>`.
    pub fn new(command: &str) -> AnnotationParser {
        let bracket = Arc::new(GroupRule {
            group_type: FlmGroupType::Content,
            open: String::from("["),
            close: String::from("]"),
        });
        AnnotationParser {
            command: command.to_string(),
            end_command: format!("end{command}"),
            braces: GroupArgumentParser::new(FlmGroupType::Content).with_expression_fallback(false),
            brackets: OptionalGroupArgumentParser::new(bracket),
        }
    }

    /// Reads the passage form: content up to `\end<command>`, built as one
    /// group with no opening delimiter and the closing command as its closing
    /// delimiter, or with no closing delimiter where the passage was not
    /// closed, which is reported.
    fn passage(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
    ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
        let start = cx.tokens.position_here();
        let state = Arc::clone(&cx.state);
        // The run stops at the closing command, and at an environment's
        // `\end` at this level, which closes an environment around the
        // annotation (the body of one the passage opens is parsed by that
        // environment, where this condition does not apply). Which of the two
        // stopped the run is noted here for the match below.
        let closed = Cell::new(false);
        let stops_at = |token: &Token<Flm>, tokens: &dyn TokenReader<'_, Flm>| {
            Ok(match tokens.token_kind(token) {
                TokenKind::Command { name, .. } if name == self.end_command => {
                    closed.set(true);
                    true
                }
                TokenKind::Command { name, .. } => name == ENVIRONMENT_END,
                _ => false,
            })
        };
        let stop = StopSpec::at_token(TokenStopKind::Predicate(&stops_at), false);
        // The passage is a region of its own: what its content changes in
        // the parsing state ends with it, as a group's does.
        let (outcome, _) = cx.parse_nodes(Arc::clone(&state), stop, ChildStateSpec::inherit())?;
        let (close, end) = match outcome.stop {
            StopCause::TokenCondition { after, .. } if closed.get() => {
                // The closing command stands unconsumed here; the group's
                // closing delimiter runs to where the command ends, the space
                // after it included, as the command token reads it.
                let close_start = cx.tokens.position_here();
                cx.tokens.move_to_position(&after);
                let span = cx.source_span_within(&close_start, &after)?;
                (TextContent::Spanned(span.span()), after)
            }
            // The input's end, or the end of the group or environment around
            // the annotation, the closing brace or `\end` left unconsumed for it.
            StopCause::TokenCondition { .. }
            | StopCause::EndOfInput
            | StopCause::UnexpectedGroupClose { .. }
            | StopCause::NodeCondition => {
                let end = cx.tokens.position_here();
                let span = cx.source_span_within(&start, &end)?;
                cx.recover(
                    Unterminated { command: self.command.clone(), end_command: self.end_command.clone() },
                    span,
                )?;
                (TextContent::empty(), end)
            }
        };
        let group_span = cx.source_span_within(&start, &end)?;
        let count = outcome.nodes.len() as u32;
        let data = GroupData { group_type: Some(FlmGroupType::Content), open: TextContent::empty(), close };
        let group = cx
            .stage_node(NodeKind::group(data), group_span.clone(), state, outcome.nodes)
            .map_err(|error| staging_error(cx, error, group_span))?;
        Ok(Some(ParsedArgumentNodes::new(vec![group], ContentNodes::InChildrenOf(group, 0..count), ())))
    }
}

/// Turns a failure to build the passage's group into the error that aborts
/// the parse: a failure building the node's extension is a `HookFailed`
/// condition, any other failure an implementation error.
fn staging_error(
    cx: &ParseContext<'_, '_, Flm>,
    error: NodeBuildError,
    span: FlmSpan,
) -> ParseError<FlmOrigin> {
    match error {
        NodeBuildError::ExtMintFailed { detail } => ParseError::new(HookFailed::new(detail, None), span),
        other => cx.implementation_error(other, span),
    }
}

impl fmt::Debug for AnnotationParser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnnotationParser").field("command", &self.command).finish()
    }
}

impl ArgumentParser<Flm> for AnnotationParser {
    fn parse_argument(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
        spec: &ArgumentSpec<Flm>,
    ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
        // Look past the whitespace and comments a group may follow, then
        // leave them for whichever form reads them.
        let noise = scan_argument_noise(cx)?;
        let next = noise.next.clone();
        noise.rewind(cx);
        let braced = next.is_some_and(|token| {
            matches!(cx.tokens.token_kind(&token), TokenKind::GroupOpen { rule, .. }
                if rule.group_type == FlmGroupType::Content)
        });
        if braced {
            return self.braces.parse_argument(cx, spec);
        }
        if let Some(comment) = self.brackets.parse_argument(cx, spec)? {
            return Ok(Some(comment));
        }
        self.passage(cx)
    }

    /// Returns `false`: the argument is always present, since the third form
    /// reads any content that follows the command.
    fn can_match_empty(&self) -> bool {
        false
    }
}

impl FlmArgumentParser for AnnotationParser {
    /// The argument is document content, so it takes a layout.
    fn holds_content(&self) -> bool {
        true
    }

    /// `flm.annotations.text:` followed by the command: the parsers of two
    /// commands differ, since each stops at its own closing command.
    fn identity(&self) -> Vec<u8> {
        format!("flm.annotations.text:{}", self.command).into_bytes()
    }

    /// No user definition can declare an argument read by this parser.
    fn definition_shape(&self) -> DefinitionShape {
        DefinitionShape::NoShape { token: "annotation" }
    }

    fn description(&self) -> String {
        format!(
            "a `{{…}}` highlight, a `[…]` comment, or a highlighted passage up to `\\{}`",
            self.end_command
        )
    }
}
