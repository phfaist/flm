//! The sizes of delimiters: the four size commands and the star, how a use of
//! a macro asks for a size, and how a delimiter is written at a size.
//!
//! A use asks for a size through the macro's *size slots*, the arguments that
//! carry a size ([`SlotKind`]): a size after a backtick, a star, a size in
//! brackets, or, in the backtick form, a size right after the backtick. Each
//! slot is an argument of one of the library's standard kinds, declared by its
//! argument code ([`SlotKind::arg`]). [`size_of_use`] reads the size a use asks
//! for ([`Size`]), and [`sized_delimiter`] writes a delimiter at a size from the
//! patterns of [`SIDES`]. The sizes and the patterns are data
//! ([`SIZE_COMMANDS`], [`SIDES`]); no macro of the built-in tables is treated as
//! a special case.
//!
//! A size that is not a size command or the star, or a size given more than
//! once, is the Error `phfpack.phfqit.bad-size` at the use ([`BAD_SIZE`],
//! [`SizeProblem::message`]).

use flm_ext::ExtError;
use flm_ext::guest::lang::mint::{ArgDef, ArgStdKind};
use flm_ext::guest::techy::node::Node;

/// The four size commands, smallest first: `\big`, `\Big`, `\bigg` and
/// `\Bigg`. A use writes one after a backtick or in brackets to ask for
/// delimiters of that size.
pub const SIZE_COMMANDS: [&str; 4] = [r"\big", r"\Big", r"\bigg", r"\Bigg"];

/// The star, `*`: what a use writes to ask for delimiters as tall as the
/// content they enclose.
pub const FIT: &str = "*";

/// The identifier of the Error reported at a use that asks for a size that is
/// not one of the four size commands or the star, or that gives a size in more
/// than one way: `phfpack.phfqit.bad-size`. The use is replaced by its source
/// text.
pub const BAD_SIZE: &str = "phfpack.phfqit.bad-size";

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// No size: the delimiter as written.
    Natural,
    /// The size command at index `i` of [`SIZE_COMMANDS`].
    Command(usize),
    /// As tall as the content: the star.
    Fit,
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
    /// A backtick followed by a size command or a star (``\abs`\Big{x}``,
    /// ``\abs`*{x}``): the embellishment argument whose one marker is the
    /// backtick (argument code ``e{`}``), the size being the expression that
    /// follows the marker.
    Backtick,
    /// A star (`\abs*{x}`), which asks for the size [`Size::Fit`] (argument
    /// code `*`).
    Star,
    /// A size command or a star in brackets (`\abs[\Big]{x}`, `\abs[*]{x}`;
    /// argument code `[`).
    Bracket,
    /// One size command written right after the backtick of the backtick form
    /// (`` `\Big(x) ``): the optional marker argument spelled as the size
    /// command at this index of [`SIZE_COMMANDS`] (argument code `t{\Big}`).
    Marker(usize),
}

impl SlotKind {
    /// Returns the argument declaration of a size slot of this kind, declared
    /// by its standard kind: the embellishment argument whose one marker is
    /// the backtick, the star, the optional argument in brackets, or the
    /// optional marker spelled as the size command (the kinds of the argument
    /// codes ``e{`}``, `*`, `[` and `t{<size command>}`).
    pub fn arg(self) -> ArgDef {
        ArgDef::new(match self {
            SlotKind::Backtick => ArgStdKind::Embellishments { markers: vec!['`'] },
            SlotKind::Star => ArgStdKind::Star,
            SlotKind::Bracket => ArgStdKind::Optional,
            SlotKind::Marker(i) => ArgStdKind::Marker { marker: SIZE_COMMANDS[i].to_string() },
        })
    }
}

/// What is wrong with the size a use asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SizeProblem {
    /// The text written as the size is not a size command or the star; the
    /// field holds that text.
    Unknown(String),
    /// A size is given in more than one way (for example after a backtick and
    /// in brackets).
    Twice,
}

impl SizeProblem {
    /// Returns the message of the Error [`BAD_SIZE`] for this problem at a use
    /// of the macro `name`, as an author writes it (`\abs`).
    pub fn message(&self, name: &str) -> String {
        match self {
            SizeProblem::Unknown(written) => format!(
                "the size of the delimiters of `{name}` must be one of {} or `{FIT}`, not `{written}`",
                SIZE_COMMANDS.map(|command| format!("`{command}`")).join(", "),
            ),
            SizeProblem::Twice => format!(
                "the size of the delimiters of `{name}` may be given only once: after a backtick, by \
                 a star, or in brackets"
            ),
        }
    }
}

/// Returns the size that the use `site` asks for through its size slots
/// (`slots`: each slot's argument index and kind), or [`Size::Natural`] when
/// no slot is written.
///
/// A slot is written when its argument is provided. The size it gives is read
/// as follows: after a backtick, the source text of the expression that follows
/// the backtick (the children of the wrapper group the embellishment argument
/// holds for its marker); in brackets, the source text between the brackets; a
/// star, the star; a marker of the backtick form, its size command. Each text
/// is taken with the white space around it removed.
///
/// # Errors
///
/// The outer error is the host's answer to a query of the use's nodes. The
/// inner one is [`SizeProblem::Twice`] when more than one slot is written, and
/// [`SizeProblem::Unknown`] when the size written is not a size command or the
/// star.
pub fn size_of_use(site: &Node, slots: &[(usize, SlotKind)]) -> Result<Result<Size, SizeProblem>, ExtError> {
    let mut written: Option<String> = None;
    for &(index, kind) in slots {
        let Some(content) = site.argument_content_nodes(index)? else { continue };
        let text = match kind {
            SlotKind::Star => FIT.to_string(),
            SlotKind::Marker(i) => SIZE_COMMANDS[i].to_string(),
            SlotKind::Bracket => text_of(&content.to_vec()?)?,
            SlotKind::Backtick => match content.get(0)? {
                Some(wrapper) => text_of(&wrapper.children()?.to_vec()?)?,
                None => continue,
            },
        };
        if written.is_some() {
            return Ok(Err(SizeProblem::Twice));
        }
        written = Some(text);
    }
    Ok(match written {
        None => Ok(Size::Natural),
        Some(text) if text == FIT => Ok(Size::Fit),
        Some(text) => match SIZE_COMMANDS.iter().position(|command| *command == text) {
            Some(i) => Ok(Size::Command(i)),
            None => Err(SizeProblem::Unknown(text)),
        },
    })
}

/// Returns the source text of the run of nodes `nodes`, with the white space
/// around it removed; an empty text for no node.
///
/// # Errors
///
/// The host's answer to a query of a node's source text.
pub fn text_of(nodes: &[Node]) -> Result<String, ExtError> {
    let mut text = String::new();
    for node in nodes {
        text.push_str(&node.span_content()?);
    }
    Ok(text.trim().to_string())
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
}
