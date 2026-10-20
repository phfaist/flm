//! The sizes of delimiters: the four size commands and the star, how a use of
//! a macro asks for a size, and how a delimiter is written at a size.
//!
//! A use asks for a size through the macro's *size slots*, the arguments that
//! carry a size ([`SlotKind`]): a size after a backtick, a star, a size in
//! brackets, or, in the backtick form, a size right after the backtick.
//! [`size_of_use`] reads the size a use asks for ([`Size`]), and
//! [`sized_delimiter`] writes a delimiter at a size from the patterns of
//! [`SIDES`]. The sizes and the patterns are data ([`SIZE_COMMANDS`],
//! [`SIDES`]); no macro of the built-in tables is treated as a special case.
//!
//! [`SizeToken`] is the crate's own argument parser, for a size written as one
//! token after a backtick, and [`BadSize`] is the condition reported for a size
//! that is not a size command or the star, or is given more than once.

use std::fmt;
use std::sync::Arc;

use flm_core::lang::mint::{arg_def_from_code, ArgCodeError, ArgDef, DefinitionShape, FlmArgumentParser};
use flm_core::lang::Flm;
use flm_core::techy::core::constructs::{
    scan_argument_noise, stage_pre_space, ArgumentParser, ConstructParserResult, ParseContext,
    ParsedArgumentNodes,
};
use flm_core::techy::core::node::{ContentNodes, NodeBuildError, NodeKind, NodeRef, NodeSlice};
use flm_core::techy::core::specs::ArgumentSpec;
use flm_core::techy::core::token::{TokenEdge, TokenKind};
use flm_core::techy::error::{DiagnosticInfo, HookFailed, ParseError};
use flm_core::techy::source::TextContent;

/// The four size commands, smallest first: `\big`, `\Big`, `\bigg` and
/// `\Bigg`. A use writes one after a backtick or in brackets to ask for
/// delimiters of that size.
pub const SIZE_COMMANDS: [&str; 4] = [r"\big", r"\Big", r"\bigg", r"\Bigg"];

/// The star, `*`: what a use writes to ask for delimiters as tall as the
/// content they enclose.
pub const FIT: &str = "*";

/// One of the three positions of a delimiter in a macro's template, each
/// written with its own placeholder: `#[X]{DelimLeft}`, `#[X]{DelimMiddle}`,
/// `#[X]{DelimRight}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    /// An opening delimiter, before the content.
    Left,
    /// A delimiter between two parts of the content (a bar, or a closing and
    /// an opening angle bracket).
    Middle,
    /// A closing delimiter, after the content.
    Right,
}

/// How a delimiter at one [`Side`] is written at a size: two patterns, in
/// which `{size}` is replaced by the size command and `{delim}` by the
/// delimiter.
#[derive(Debug, Clone, Copy)]
pub struct SidePatterns {
    /// The side these patterns are for.
    pub side: Side,
    /// The name of the placeholder a template writes for this side
    /// (`DelimLeft`, `DelimMiddle` or `DelimRight`).
    pub placeholder: &'static str,
    /// The pattern of the delimiter at a size command (`{size}l {delim}` on
    /// the left).
    pub command: &'static str,
    /// The pattern of the delimiter as tall as the content, written with
    /// `\left`, `\middle` or `\right`.
    pub fit: &'static str,
}

/// The patterns of the three sides, each with its placeholder name.
pub const SIDES: [SidePatterns; 3] = [
    SidePatterns {
        side: Side::Left,
        placeholder: "DelimLeft",
        command: "{size}l {delim}",
        fit: r"\mathopen{}\left {delim}",
    },
    SidePatterns {
        side: Side::Middle,
        placeholder: "DelimMiddle",
        command: r"\mathclose{}{size} {delim}\mathopen{}",
        fit: r"\mathclose{}\middle {delim}\mathopen{}",
    },
    SidePatterns {
        side: Side::Right,
        placeholder: "DelimRight",
        command: "{size}r {delim}",
        fit: r"\right {delim}\mathclose{}",
    },
];

/// Returns the patterns of `side`.
pub fn patterns(side: Side) -> &'static SidePatterns {
    // `SIDES` lists every side once.
    SIDES.iter().find(|patterns| patterns.side == side).unwrap_or(&SIDES[0])
}

/// A size that a use asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Size {
    /// No size: the delimiter as written.
    Natural,
    /// The size command at index `i` of [`SIZE_COMMANDS`].
    Command(usize),
    /// As tall as the content: the star.
    Fit,
}

impl Size {
    /// Returns every size in a fixed order: the natural size, the four size
    /// commands smallest first, then the star. [`Size::index`] numbers the
    /// sizes in this order.
    pub fn all() -> impl Iterator<Item = Size> {
        std::iter::once(Size::Natural)
            .chain((0..SIZE_COMMANDS.len()).map(Size::Command))
            .chain(std::iter::once(Size::Fit))
    }

    /// Returns the size's position, from 0, in the order of [`Size::all`].
    pub fn index(self) -> usize {
        match self {
            Size::Natural => 0,
            Size::Command(i) => 1 + i,
            Size::Fit => 1 + SIZE_COMMANDS.len(),
        }
    }
}

/// Returns the text of the delimiter `delim` at the side `side`, drawn at the
/// size `size`: `\Bigl \langle` for the left `\langle` at `\Big`, and the
/// delimiter unchanged at the natural size.
pub fn sized_delimiter(side: Side, size: Size, delim: &str) -> String {
    let patterns = patterns(side);
    match size {
        Size::Natural => delim.to_string(),
        Size::Command(i) => {
            patterns.command.replace("{size}", SIZE_COMMANDS[i]).replace("{delim}", delim)
        }
        Size::Fit => patterns.fit.replace("{delim}", delim),
    }
}

/// The kind of a size slot: how the argument carries a size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotKind {
    /// A backtick followed by a size command or a star (``\abs`\Big{x}``),
    /// read by [`SizeToken::AFTER_BACKTICK`].
    Backtick,
    /// A star (`\abs*{x}`), which asks for the size [`Size::Fit`].
    Star,
    /// A size command or a star in brackets (`\abs[\Big]{x}`, `\abs[*]{x}`).
    Bracket,
    /// The size of the backtick form (`` `\Big(x) ``): a size command or a star
    /// written right after the backtick, read by [`SizeToken::BARE`].
    Token,
}

impl SlotKind {
    /// Returns the argument declaration of a size slot of this kind: the
    /// standard argument that the argument code `*` or `[` reads, for a star
    /// and for a size in brackets; the crate's own [`SizeToken`], for the two
    /// kinds that no standard argument reads.
    ///
    /// # Errors
    ///
    /// The [`ArgCodeError`] of reading the code `*` or `[`. These codes are the
    /// library's own, so the error does not occur.
    pub fn arg(self) -> Result<ArgDef, ArgCodeError> {
        match self {
            SlotKind::Star => arg_def_from_code("*"),
            SlotKind::Bracket => arg_def_from_code("["),
            SlotKind::Backtick => Ok(ArgDef::new(SizeToken::AFTER_BACKTICK)),
            SlotKind::Token => Ok(ArgDef::new(SizeToken::BARE)),
        }
    }
}

/// Condition `phfpack.phfqit.bad-size`: a use of a macro of the feature
/// `phfqit` ([`Phfqit`](crate::Phfqit)) asks for a delimiter size that is not
/// one of the four size commands or the star, or gives a size in more than one
/// way.
///
/// The feature reports it as an Error at the use, and the use is replaced by
/// its source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadSize {
    /// The macro, as an author writes it (`\abs`).
    pub name: String,
    /// What is wrong with the size.
    pub problem: SizeProblem,
}

/// What is wrong with the size a use asks for (the detail of [`BadSize`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SizeProblem {
    /// The text written as the size is not a size command or the star; the
    /// field holds that text.
    Unknown(String),
    /// A size is given in more than one way (for example after a backtick and
    /// in brackets).
    Twice,
}

impl fmt::Display for BadSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.problem {
            SizeProblem::Unknown(written) => write!(
                f,
                "the size of the delimiters of `{}` must be one of {} or `{FIT}`, not `{written}`",
                self.name,
                SIZE_COMMANDS.map(|command| format!("`{command}`")).join(", "),
            ),
            SizeProblem::Twice => write!(
                f,
                "the size of the delimiters of `{}` may be given only once: after a backtick, by \
                 a star, or in brackets",
                self.name
            ),
        }
    }
}

impl DiagnosticInfo for BadSize {
    const IDENTIFIER: &'static str = "phfpack.phfqit.bad-size";
}

/// Returns the size that the use `site` asks for through its size slots
/// (`slots`: each slot's argument index and kind), or [`Size::Natural`] when
/// no slot is written.
///
/// # Errors
///
/// [`SizeProblem::Twice`] when more than one slot is written, and
/// [`SizeProblem::Unknown`] when the size written is not a size command or the
/// star.
pub fn size_of_use(
    site: &NodeRef<'_, Flm>,
    slots: &[(usize, SlotKind)],
) -> Result<Size, SizeProblem> {
    let mut written: Option<String> = None;
    for &(index, kind) in slots {
        let Some(content) = site.argument_content_nodes(index) else { continue };
        let text = match kind {
            SlotKind::Star => FIT.to_string(),
            SlotKind::Backtick | SlotKind::Bracket | SlotKind::Token => text_of(&content),
        };
        if written.is_some() {
            return Err(SizeProblem::Twice);
        }
        written = Some(text);
    }
    match written {
        None => Ok(Size::Natural),
        Some(text) if text == FIT => Ok(Size::Fit),
        Some(text) => match SIZE_COMMANDS.iter().position(|command| *command == text) {
            Some(i) => Ok(Size::Command(i)),
            None => Err(SizeProblem::Unknown(text)),
        },
    }
}

/// Returns the source text of the run of nodes `nodes`, with the white space
/// around it removed; an empty text when the nodes have no source text.
pub fn text_of(nodes: &NodeSlice<'_, Flm>) -> String {
    nodes.source_text().map(|text| text.trim().to_string()).unwrap_or_default()
}

/// The crate's argument parser for a size written as one token, a size
/// command or a star, in one of two places: right after the backtick of the
/// backtick form ([`SizeToken::BARE`], `` `\Big(x) ``), or after a backtick of
/// its own, the first of the three ways a delimited macro takes a size
/// ([`SizeToken::AFTER_BACKTICK`], ``\abs`\Big{x}``; see the crate
/// documentation, section "What an author may write").
///
/// The argument is optional: when the tokens at that place are not what the
/// parser expects, it consumes nothing and the argument is absent. The parser
/// accepts any command as the size token; whether the command is one of the
/// size commands is checked when the use is expanded, which reports a command
/// that is not a size command ([`BadSize`]).
///
/// No standard argument kind
/// ([`ArgStdKind`](flm_core::lang::mint::ArgStdKind)) reads these tokens.
/// Right after the backtick of the backtick form, a mandatory argument
/// ([`ArgStdKind::Mandatory`](flm_core::lang::mint::ArgStdKind::Mandatory), a
/// group or a single token) would always be read, and would take the group's
/// opening character when no size is written. An embellishment argument
/// ([`ArgStdKind::Embellishments`](flm_core::lang::mint::ArgStdKind::Embellishments),
/// code ``e{`}``) begins with a marker character, never with a command; and in
/// a formula the backtick begins the backtick form, and a character that begins
/// another construct is never accepted as an embellishment marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizeToken {
    after_backtick: bool,
}

impl SizeToken {
    /// The size token alone, as the backtick form reads it right after its
    /// backtick.
    pub const BARE: SizeToken = SizeToken { after_backtick: false };
    /// A backtick, then the size token: the first of the three size arguments
    /// of a delimited macro (the size after a backtick).
    pub const AFTER_BACKTICK: SizeToken = SizeToken { after_backtick: true };
}

/// The backtick that introduces a size.
const BACKTICK: &str = "`";

/// Returns whether `kind` is a size token: a command, or the star.
fn is_size_token(kind: &TokenKind<'_, Flm>) -> bool {
    match kind {
        TokenKind::Command { .. } => true,
        TokenKind::Char(c) => c.to_string() == FIT,
        _ => false,
    }
}

/// Returns whether `kind` is the backtick: a plain character, or the trigger
/// of the backtick form where that form is defined (in formulas).
fn is_backtick(kind: &TokenKind<'_, Flm>) -> bool {
    match kind {
        TokenKind::Char(c) => c.to_string() == BACKTICK,
        TokenKind::Specials { name, .. } => *name == BACKTICK,
        _ => false,
    }
}

impl ArgumentParser<Flm> for SizeToken {
    fn parse_argument(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
        _spec: &ArgumentSpec<Flm>,
    ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
        let mut noise = scan_argument_noise(cx)?;
        let Some(first) = noise.next.clone() else {
            noise.rewind(cx);
            return Ok(None);
        };
        let state = Arc::clone(&cx.state);
        let mut tokens = vec![first.clone()];
        if self.after_backtick {
            if !is_backtick(&cx.tokens.token_kind(&first)) {
                noise.rewind(cx);
                return Ok(None);
            }
            cx.tokens.move_to(&first, TokenEdge::EndPastPostSpace);
            match cx.probe_token(&state)? {
                Some(size) => tokens.push(size),
                None => {
                    noise.rewind(cx);
                    return Ok(None);
                }
            }
        }
        let Some(size) = tokens.last().cloned() else {
            noise.rewind(cx);
            return Ok(None);
        };
        if !is_size_token(&cx.tokens.token_kind(&size)) {
            noise.rewind(cx);
            return Ok(None);
        }
        // Each token read becomes a node of the argument, its own pre-space
        // before it; the last, the size, is the content.
        for token in &tokens {
            stage_pre_space(cx, &mut noise.nodes, token)?;
            let start = cx.tokens.position_at(token, TokenEdge::Start);
            let end = cx.tokens.position_at(token, TokenEdge::EndPastPostSpace);
            cx.tokens.move_to(token, TokenEdge::EndPastPostSpace);
            let span = cx.source_span_within(&start, &end)?;
            let node = cx
                .stage_node(
                    NodeKind::chars(TextContent::Spanned(span.span())),
                    span.clone(),
                    Arc::clone(&state),
                    Vec::new(),
                )
                .map_err(|error| match error {
                    // techy's split of a staging failure: the extension mint's
                    // own report is a hook failure, anything else a broken
                    // contract.
                    NodeBuildError::ExtMintFailed { detail } => {
                        ParseError::new(HookFailed::new(detail, None), span.clone())
                    }
                    other => cx.implementation_error(other, span.clone()),
                })?;
            noise.nodes.push(node);
        }
        let last = noise.nodes.len() as u32 - 1;
        Ok(Some(ParsedArgumentNodes::new(
            noise.nodes,
            ContentNodes::InRegion(last..last + 1),
            (),
        )))
    }
}

impl FlmArgumentParser for SizeToken {
    fn holds_content(&self) -> bool {
        false
    }

    fn identity(&self) -> Vec<u8> {
        match self.after_backtick {
            false => b"phfqit.size-token".to_vec(),
            true => b"phfqit.size-token.after-backtick".to_vec(),
        }
    }

    fn definition_shape(&self) -> DefinitionShape {
        DefinitionShape::NoShape { token: "phfqit-size-token" }
    }

    fn description(&self) -> String {
        match self.after_backtick {
            false => format!("a size command or `{FIT}`, or nothing"),
            true => format!("a backtick followed by a size command or `{FIT}`, or nothing"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_delimiter_is_written_at_each_size_as_the_patterns_say() {
        assert_eq!(sized_delimiter(Side::Left, Size::Natural, r"\langle"), r"\langle");
        assert_eq!(sized_delimiter(Side::Left, Size::Command(1), r"\langle"), r"\Bigl \langle");
        assert_eq!(
            sized_delimiter(Side::Middle, Size::Command(0), r"\vert"),
            r"\mathclose{}\big \vert\mathopen{}"
        );
        assert_eq!(sized_delimiter(Side::Right, Size::Command(3), "]"), r"\Biggr ]");
        assert_eq!(sized_delimiter(Side::Left, Size::Fit, "["), r"\mathopen{}\left [");
        assert_eq!(
            sized_delimiter(Side::Middle, Size::Fit, "|"),
            r"\mathclose{}\middle |\mathopen{}"
        );
        assert_eq!(sized_delimiter(Side::Right, Size::Fit, "]"), r"\right ]\mathclose{}");
    }

    #[test]
    fn the_sizes_are_numbered_in_one_order() {
        let indices: Vec<usize> = Size::all().map(Size::index).collect();
        assert_eq!(indices, (0..SIZE_COMMANDS.len() + 2).collect::<Vec<_>>());
    }
}
