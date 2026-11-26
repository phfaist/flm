//! The painter of the output `tty`: [`paint()`] writes the printed text of the text output
//! with the terminal escape sequences that show its style marks, and the colors of the
//! images drawn in it.
//!
//! A style mark is a range of the printed text whose content carries a property plain text
//! cannot show, such as bold weight or the address of a link; the text backend records them
//! beside the text, in the table `flm.output.text`, whose projection
//! [`StyledText::from_projection`] reads. The painter reads the text, its style marks and the
//! colors of the drawn images' cells, and changes no character: removing its escape
//! sequences from its result gives back the printed text, every control character replaced
//! (see below).
//!
//! # The palette
//!
//! Each style sets terminal attributes and a color of the text, through the escape
//! sequences of ECMA-48 called SGR (Select Graphic Rendition), and a link to an address
//! outside the document also sets a hyperlink ([`palette`] holds the mapping):
//!
//! | style | written as |
//! |---|---|
//! | [`Heading`](Style::Heading) at depth 0 | bold (SGR 1), bright cyan |
//! | [`Heading`](Style::Heading) at depth 1 or more | bold, cyan |
//! | [`HeadingRule`](Style::HeadingRule) | cyan |
//! | [`TableBorder`](Style::TableBorder) | bright black (a gray) |
//! | [`Bold`](Style::Bold) | bold |
//! | [`Italic`](Style::Italic), [`Emphasis`](Style::Emphasis) | italic (SGR 3) |
//! | [`Underline`](Style::Underline) | underlined (SGR 4) |
//! | [`Link`](Style::Link) whose target does not begin with `#` | underlined, bright blue, and a hyperlink to the target |
//! | [`Link`](Style::Link) to a place inside the document (`#…`), or with an empty target | nothing |
//! | [`Monospace`](Style::Monospace), [`SmallCaps`](Style::SmallCaps), [`Verbatim`](Style::Verbatim) | nothing |
//!
//! The colors are among the sixteen standard colors of a terminal, whose values the
//! terminal's theme chooses. Which colors are written is the choice of [`Colors`], the
//! setting `tty.colors`: none under [`Colors::None`], which leaves the attributes and the
//! hyperlinks; the standard colors under [`Colors::Standard`], the default, where a color
//! given by its red, green and blue values is written as the standard color nearest to it
//! (by the squared distance to the values of xterm's default theme); and a color given by
//! its values as it is under [`Colors::Rich`]. No style of the palette has a color given by
//! its values, so the last two give the same result outside the drawn images.
//!
//! # Drawn images
//!
//! An image drawn with half-block characters (the module `graphics`) is a grid of
//! characters whose colors the style marks do not carry: the painter receives them as
//! [`Cell`]s, the colors of the character at a byte offset of the text, each a color given
//! by its values for the glyph, for the background, or for both. A cell's colors take the
//! place of the colors of the marks covering its character, and its attributes stay; under
//! [`Colors::Standard`] they are written as the nearest standard colors; under
//! [`Colors::None`] the cells are ignored.
//!
//! A character is written with the attributes of every style mark that covers it, with the
//! color of the innermost mark that sets one, and with the hyperlink of the innermost link
//! mark to an address outside the document that covers it. The marks nest: the painter
//! orders them by where they start, the longer first of two that start together, whatever
//! the order of the list it reads. Each link mark is a hyperlink of its own, opened where
//! the mark starts and closed where it ends, even next to another one with the same target.
//!
//! # The escape sequences
//!
//! Where the attributes or the colors change, the painter writes one SGR sequence that
//! resets everything and sets the new state, always in the same order: `0`, then `1`, `3`
//! and `4` for bold, italic and underline, then the color of the text (`30`–`37` for the
//! standard colors 0 to 7, `90`–`97` for 8 to 15, `38;2;R;G;B` for a color given by its
//! values), then the color of the background (`40`–`47`, `100`–`107`, `48;2;R;G;B`); for
//! instance `ESC[0;1;96m` for a bold, bright cyan heading and `ESC[0m` for nothing. A
//! hyperlink is opened with the sequence of the OSC 8 convention, `ESC ] 8 ; ; TARGET ESC \`,
//! and closed with `ESC ] 8 ; ; ESC \`. In the target, every byte outside the printable
//! ASCII range 0x21–0x7E is written as `%XX`, its value in uppercase hexadecimal, so that a
//! space, a non-ASCII character or an escape character in an address cannot end or break
//! the sequence. Where the hyperlink and the attributes change at one place, the
//! hyperlink's sequence comes first.
//!
//! # Lines
//!
//! Every line of the result is self-contained: before each newline, the painter closes an
//! open hyperlink and resets the attributes and colors, and after it the painter sets them again at the
//! line's first character that is not a space or is a cell. A terminal, or a pager such as
//! `less -R`, can therefore show any line on its own, and no attribute leaks past the end of
//! the text. The spaces that indent a line are never styled and never part of a hyperlink,
//! so a styled range that wraps onto an indented line leaves the indentation plain; a space
//! that is a cell of a drawn image is not indentation, and shows its background.
//!
//! # Control characters
//!
//! A control character in the printed text (verbatim content can hold one) is written as
//! U+FFFD, the replacement character: every character from U+0000 to U+001F except the
//! newline and the tab, U+007F, and every character from U+0080 to U+009F. The only control
//! characters the result holds are therefore the newlines, the tabs and the escape
//! characters of the painter's own sequences.

use std::fmt::Write;
use std::ops::Range;

use serde_json::Value;

use crate::profile_tty::graphics::{CellColors, Rgb};

// The escape sequences, in one place. SGR (Select Graphic Rendition) is the control
// sequence `CSI … m` of ECMA-48, with `CSI` = `ESC [`; parameter 0 resets every attribute,
// 1 sets bold, 3 italic, 4 underline; 30–37 and 90–97 set one of the sixteen standard colors
// of the text, 40–47 and 100–107 of the background, and 38;2;R;G;B and 48;2;R;G;B a color
// given by its red, green and blue values (the form of ITU T.416 that terminals accept).
// OSC 8 is the hyperlink convention `OSC 8 ; params ; URI ST`, with `OSC` = `ESC ]` and the
// string terminator `ST` = `ESC \`; an empty URI closes the hyperlink. A terminal that does
// not honor OSC 8 ignores it, which is harmless since the sequence shows nothing.

/// The start of an SGR sequence: `ESC [`.
const CSI: &str = "\u{1b}[";
/// The end of an SGR sequence.
const SGR_END: &str = "m";
/// The SGR parameter that sets bold, written after the reset `0`.
const SGR_BOLD: &str = ";1";
/// The SGR parameter that sets italic, written after bold.
const SGR_ITALIC: &str = ";3";
/// The SGR parameter that sets underline, written after italic.
const SGR_UNDERLINE: &str = ";4";
/// The SGR parameters that set a color of one plane, the text's or the background's.
struct Plane {
    /// The parameter of the standard color 0; the colors 1 to 7 follow it.
    dark: u16,
    /// The parameter of the standard color 8; the colors 9 to 15 follow it.
    bright: u16,
    /// The parameter that starts a color given by its values, `;2;R;G;B` following it.
    rgb: u16,
}
/// The SGR parameters of the text's color, written after the attributes.
const SGR_FOREGROUND: Plane = Plane { dark: 30, bright: 90, rgb: 38 };
/// The SGR parameters of the background's color, written after the text's color.
const SGR_BACKGROUND: Plane = Plane { dark: 40, bright: 100, rgb: 48 };
/// The start of an OSC 8 hyperlink sequence, up to its target: `ESC ] 8 ; ;`.
const OSC8: &str = "\u{1b}]8;;";
/// The string terminator ending an OSC 8 sequence: `ESC \`.
const ST: &str = "\u{1b}\\";

/// The style of a style mark: the property of its content that plain text cannot show.
///
/// The variants are the values of the key `style` of a style mark in the projection of
/// `flm.output.text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Style {
    /// A heading's title (`heading`), with the heading's depth, the key `depth`: 0 for a
    /// heading at the root level, one more for each level below it.
    Heading {
        /// The heading's depth.
        depth: u8,
    },
    /// The characters the text output draws to set a heading apart (`heading-rule`): the
    /// rule under its title, or the `#…` prefix of a deeper heading, with the heading's
    /// depth, the key `depth`.
    HeadingRule {
        /// The heading's depth.
        depth: u8,
    },
    /// The characters the text output draws for a table (`table-border`): its frame, or the
    /// rule under its header rows.
    TableBorder,
    /// Bold text (`bold`).
    Bold,
    /// Italic text (`italic`).
    Italic,
    /// Emphasized text (`emphasis`).
    Emphasis,
    /// Underlined text (`underline`).
    Underline,
    /// Text in a monospaced font (`monospace`).
    Monospace,
    /// Text in small capitals (`small-caps`).
    SmallCaps,
    /// Verbatim content (`verbatim`).
    Verbatim,
    /// A link's content (`link`), with the link's target, the key `href`.
    Link {
        /// The link's target: an address, or `#…` for a place inside the document.
        href: String,
    },
}

/// A style mark: a style over a byte range of the printed text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleMark {
    /// The style.
    pub style: Style,
    /// The byte range of the printed text that the mark covers.
    pub bytes: Range<usize>,
}

/// The printed text of the text output and its style marks, as the painter reads them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StyledText {
    /// The printed text.
    pub text: String,
    /// The style marks over the text, in any order.
    pub styles: Vec<StyleMark>,
}

impl StyledText {
    /// Reads the text and the style marks from `projection`, the projection of the table
    /// `flm.output.text`: a map holding the text as `text` and the style marks as `styles`,
    /// each a map holding `style`, `bytes` (the start and the end, two whole numbers) and,
    /// for a link, `href`. The other keys (the position marks, a style mark's lines and
    /// columns) are not read.
    ///
    /// A style mark whose `style` is none of the names [`Style`] lists paints nothing and is
    /// left out. `None` when `projection` does not have this shape: a `heading` or a
    /// `heading-rule` must hold its `depth`, a whole number from 0 to 255.
    pub fn from_projection(projection: &Value) -> Option<StyledText> {
        let text = projection.get("text")?.as_str()?.to_string();
        let mut styles = Vec::new();
        for mark in projection.get("styles")?.as_array()? {
            let bytes = mark.get("bytes")?.as_array()?;
            let [start, end] = bytes.as_slice() else { return None };
            let bytes = usize::try_from(start.as_u64()?).ok()?..usize::try_from(end.as_u64()?).ok()?;
            let style = match mark.get("style")?.as_str()? {
                "heading" => Style::Heading { depth: depth(mark)? },
                "heading-rule" => Style::HeadingRule { depth: depth(mark)? },
                "table-border" => Style::TableBorder,
                "bold" => Style::Bold,
                "italic" => Style::Italic,
                "emphasis" => Style::Emphasis,
                "underline" => Style::Underline,
                "monospace" => Style::Monospace,
                "small-caps" => Style::SmallCaps,
                "verbatim" => Style::Verbatim,
                "link" => Style::Link { href: mark.get("href")?.as_str()?.to_string() },
                _ => continue,
            };
            styles.push(StyleMark { style, bytes });
        }
        Some(StyledText { text, styles })
    }
}

/// The depth of the style mark `mark`, its key `depth`.
fn depth(mark: &Value) -> Option<u8> {
    u8::try_from(mark.get("depth")?.as_u64()?).ok()
}

/// Which colors the painter writes: the values of the setting `tty.colors`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Colors {
    /// No color: the attributes and the hyperlinks alone (`none`).
    None,
    /// The sixteen standard colors of the terminal, whose values the terminal's theme
    /// chooses; a color given by its red, green and blue values is written as the standard
    /// color nearest to it (`standard`, the default).
    #[default]
    Standard,
    /// The standard colors, and a color given by its red, green and blue values written as
    /// it is (`rich`), for a terminal that shows 24-bit colors.
    Rich,
}

impl Colors {
    /// The value of the setting `tty.colors` named `name`, `None` for any other name.
    pub fn named(name: &str) -> Option<Colors> {
        match name {
            "none" => Some(Colors::None),
            "standard" => Some(Colors::Standard),
            "rich" => Some(Colors::Rich),
            _ => None,
        }
    }
}

/// A color of the text or of its background.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Color {
    /// One of the sixteen standard colors of a terminal, 0 to 15 (the table of
    /// [`STANDARD_RGB`] names them), whose values the terminal's theme chooses.
    Standard(u8),
    /// A color given by its red, green and blue values.
    Rgb(u8, u8, u8),
}

impl From<Rgb> for Color {
    fn from(Rgb(r, g, b): Rgb) -> Color {
        Color::Rgb(r, g, b)
    }
}

/// A character of the printed text that is a cell of a drawn image, with its colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// The byte offset of the character in the printed text.
    pub at: usize,
    /// The colors of the cell.
    pub colors: CellColors,
}

/// The red, green and blue values of the sixteen standard colors in xterm's default theme,
/// against which a color given by its values is quantized: black, red, green, yellow, blue,
/// magenta, cyan and white (0 to 7), then their bright forms (8 to 15).
const STANDARD_RGB: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (205, 0, 0),
    (0, 205, 0),
    (205, 205, 0),
    (0, 0, 238),
    (205, 0, 205),
    (0, 205, 205),
    (229, 229, 229),
    (127, 127, 127),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (92, 92, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
];

/// The standard color cyan.
const CYAN: u8 = 6;
/// The standard color bright black, a gray.
const BRIGHT_BLACK: u8 = 8;
/// The standard color bright blue.
const BRIGHT_BLUE: u8 = 12;
/// The standard color bright cyan.
const BRIGHT_CYAN: u8 = 14;

/// The standard color nearest to the color of the values `r`, `g` and `b`: the one of
/// [`STANDARD_RGB`] at the smallest squared distance, the lower number of two at the same
/// distance.
fn nearest_standard(r: u8, g: u8, b: u8) -> u8 {
    let distance = |(sr, sg, sb): (u8, u8, u8)| {
        let d = |x: u8, y: u8| (i32::from(x) - i32::from(y)).pow(2);
        d(r, sr) + d(g, sg) + d(b, sb)
    };
    let mut nearest = 0;
    for (n, values) in (0u8..).zip(STANDARD_RGB) {
        if distance(values) < distance(STANDARD_RGB[usize::from(nearest)]) {
            nearest = n;
        }
    }
    nearest
}

/// The terminal attributes and colors a character is written with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Attrs {
    bold: bool,
    italic: bool,
    underline: bool,
    fg: Option<Color>,
    bg: Option<Color>,
}

impl Attrs {
    /// The attributes of both, and the colors of `inner` where it has some, else those of
    /// `self`: `inner` is a mark inside the marks `self` gathers.
    fn union(self, inner: Attrs) -> Attrs {
        Attrs {
            bold: self.bold || inner.bold,
            italic: self.italic || inner.italic,
            underline: self.underline || inner.underline,
            fg: inner.fg.or(self.fg),
            bg: inner.bg.or(self.bg),
        }
    }

    /// The attributes as the painter writes them under `colors`: without their colors under
    /// [`Colors::None`], with a color given by its values written as the nearest standard
    /// color under [`Colors::Standard`], unchanged under [`Colors::Rich`].
    fn under(self, colors: Colors) -> Attrs {
        let color = |color: Option<Color>| match (colors, color) {
            (Colors::None, _) => None,
            (Colors::Standard, Some(Color::Rgb(r, g, b))) => Some(Color::Standard(nearest_standard(r, g, b))),
            (_, color) => color,
        };
        Attrs { fg: color(self.fg), bg: color(self.bg), ..self }
    }
}

/// How a character is shown: its attributes, and its hyperlink (the index of the link mark
/// that sets it, and that mark's target), so that two link marks are two hyperlinks even
/// when their targets are equal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Look<'a> {
    attrs: Attrs,
    link: Option<(usize, &'a str)>,
}

/// The palette: what a style mark paints, its attributes and its colors, and the target of
/// its hyperlink for a link to an address outside the document. `None` for a style the
/// painter does not show.
fn palette(style: &Style) -> Option<(Attrs, Option<&str>)> {
    let attrs = Attrs::default();
    let fg = |n: u8| Some(Color::Standard(n));
    match style {
        Style::Heading { depth: 0 } => Some((Attrs { bold: true, fg: fg(BRIGHT_CYAN), ..attrs }, None)),
        Style::Heading { .. } => Some((Attrs { bold: true, fg: fg(CYAN), ..attrs }, None)),
        Style::HeadingRule { .. } => Some((Attrs { fg: fg(CYAN), ..attrs }, None)),
        Style::TableBorder => Some((Attrs { fg: fg(BRIGHT_BLACK), ..attrs }, None)),
        Style::Bold => Some((Attrs { bold: true, ..attrs }, None)),
        Style::Italic | Style::Emphasis => Some((Attrs { italic: true, ..attrs }, None)),
        Style::Underline => Some((Attrs { underline: true, ..attrs }, None)),
        Style::Link { href } if !href.is_empty() && !href.starts_with('#') => {
            Some((Attrs { underline: true, fg: fg(BRIGHT_BLUE), ..attrs }, Some(href.as_str())))
        }
        Style::Link { .. } | Style::Monospace | Style::SmallCaps | Style::Verbatim => None,
    }
}

/// Whether the painter writes `c` as U+FFFD: a control character other than the newline and
/// the tab.
fn is_replaced(c: char) -> bool {
    matches!(c, '\u{0}'..='\u{8}' | '\u{b}'..='\u{1f}' | '\u{7f}'..='\u{9f}')
}

/// Returns the printed text of `output` with the escape sequences that show its style
/// marks: bold, italic, underline and the colors that `colors` chooses as SGR sequences,
/// and a link to an address outside the document as an OSC 8 hyperlink (the [module
/// documentation](self) gives the palette and the rules); and with the colors of `cells`,
/// the characters of the drawn images, in any order, each in place of the colors of the
/// marks covering it. A cell whose offset is not that of a character of the text is left
/// out.
///
/// Every line of the result is self-contained (its attributes, colors and hyperlink closed
/// before its newline and set again at the next line's first character that is not a space
/// or is a cell, so a line's indentation is never styled), and every control character of the text other
/// than the newline and the tab is written as U+FFFD. Removing the escape sequences from the
/// result gives the text with that replacement and nothing else changed; an empty text gives
/// an empty result. A style mark whose range lies outside the text, or does not start or end
/// on a character boundary, never makes `paint` panic: it covers the characters whose first
/// byte lies in its range.
pub fn paint(output: &StyledText, cells: &[Cell], colors: Colors) -> String {
    let text = output.text.as_str();
    // The marks that paint something, ordered by where they start, the longer first of two
    // that start together, so that an outer mark comes before the marks it holds.
    let mut marks: Vec<(Range<usize>, Attrs, Option<&str>)> = output
        .styles
        .iter()
        .filter(|mark| mark.bytes.start < mark.bytes.end)
        .filter_map(|mark| palette(&mark.style).map(|(attrs, link)| (mark.bytes.clone(), attrs, link)))
        .collect();
    marks.sort_by_key(|(range, _, _)| (range.start, std::cmp::Reverse(range.end)));
    let mut cells = cells.to_vec();
    cells.sort_by_key(|cell| cell.at);
    let mut next_cell = 0;
    let mut out = String::with_capacity(text.len() + text.len() / 4);
    // The marks covering the character at hand, in the order they started: the innermost
    // hyperlink is the last one's that has one.
    let mut active: Vec<usize> = Vec::new();
    let mut next = 0;
    let mut wanted = Look::default();
    let mut shown = Look::default();
    // Whether the character at hand is in a line's indentation: at the start of a line,
    // before its first character that is not a space.
    let mut indentation = true;
    for (at, c) in text.char_indices() {
        let before = active.len();
        active.retain(|index| marks[*index].0.end > at);
        let mut changed = active.len() != before;
        while next < marks.len() && marks[next].0.start <= at {
            if marks[next].0.end > at {
                active.push(next);
                changed = true;
            }
            next += 1;
        }
        if changed {
            wanted = Look {
                attrs: active
                    .iter()
                    .fold(Attrs::default(), |attrs, index| attrs.union(marks[*index].1))
                    .under(colors),
                link: active.iter().rev().find_map(|index| marks[*index].2.map(|target| (*index, target))),
            };
        }
        while next_cell < cells.len() && cells[next_cell].at < at {
            next_cell += 1;
        }
        // Under `none`, a cell is a character like any other.
        let cell = cells.get(next_cell).filter(|cell| cell.at == at && colors != Colors::None).map(|cell| cell.colors);
        if c == '\n' {
            switch(&mut out, &mut shown, Look::default());
            out.push('\n');
            indentation = true;
            continue;
        }
        // The indentation is written plain: the styles are set again at the line's first
        // character that is not a space, or is a cell.
        if indentation && c == ' ' && cell.is_none() {
            out.push(' ');
            continue;
        }
        indentation = false;
        let look = match cell {
            None => wanted,
            Some(CellColors { fg, bg }) => {
                let attrs = Attrs { fg: fg.map(Color::from), bg: bg.map(Color::from), ..wanted.attrs };
                Look { attrs: attrs.under(colors), ..wanted }
            }
        };
        switch(&mut out, &mut shown, look);
        out.push(if is_replaced(c) { '\u{fffd}' } else { c });
    }
    switch(&mut out, &mut shown, Look::default());
    out
}

/// Writes into `out` the sequences that change what the terminal shows from `shown` to
/// `wanted`: the hyperlink's first, then the attributes'; nothing where they are equal.
fn switch<'a>(out: &mut String, shown: &mut Look<'a>, wanted: Look<'a>) {
    if shown.link != wanted.link {
        if shown.link.is_some() {
            out.push_str(OSC8);
            out.push_str(ST);
        }
        if let Some((_, target)) = wanted.link {
            out.push_str(OSC8);
            push_target(out, target);
            out.push_str(ST);
        }
    }
    if shown.attrs != wanted.attrs {
        push_sgr(out, wanted.attrs);
    }
    *shown = wanted;
}

/// Writes into `out` the one SGR sequence that sets `attrs`: the reset `0`, then bold,
/// italic and underline where set, then the foreground and the background colors where set
/// (`30`–`37` and `90`–`97` for a standard color, `38;2;R;G;B` for a color given by its
/// values; `40`–`47`, `100`–`107` and `48;2;R;G;B` for the background).
fn push_sgr(out: &mut String, attrs: Attrs) {
    out.push_str(CSI);
    out.push('0');
    if attrs.bold {
        out.push_str(SGR_BOLD);
    }
    if attrs.italic {
        out.push_str(SGR_ITALIC);
    }
    if attrs.underline {
        out.push_str(SGR_UNDERLINE);
    }
    for (color, plane) in [(attrs.fg, SGR_FOREGROUND), (attrs.bg, SGR_BACKGROUND)] {
        match color {
            None => {}
            Some(Color::Standard(n)) if n < 8 => {
                let _ = write!(out, ";{}", plane.dark + u16::from(n));
            }
            Some(Color::Standard(n)) => {
                let _ = write!(out, ";{}", plane.bright + u16::from(n.min(15) - 8));
            }
            Some(Color::Rgb(r, g, b)) => {
                let _ = write!(out, ";{};2;{r};{g};{b}", plane.rgb);
            }
        }
    }
    out.push_str(SGR_END);
}

/// Writes the hyperlink target `target` into `out`, each byte outside 0x21–0x7E as `%XX` in
/// uppercase hexadecimal.
fn push_target(out: &mut String, target: &str) {
    for byte in target.bytes() {
        if (0x21..=0x7e).contains(&byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A style mark over `bytes`.
    fn mark(bytes: Range<usize>, style: Style) -> StyleMark {
        StyleMark { style, bytes }
    }

    /// The range of the first occurrence of `needle` in `text`.
    fn find(text: &str, needle: &str) -> Range<usize> {
        let start = text.find(needle).unwrap_or_else(|| panic!("`{needle}` in `{text}`"));
        start..start + needle.len()
    }

    /// `text` and `styles`, painted without colors.
    fn painted(text: &str, styles: Vec<StyleMark>) -> String {
        painted_in(Colors::None, text, styles)
    }

    /// `text` and `styles`, painted with `colors`.
    fn painted_in(colors: Colors, text: &str, styles: Vec<StyleMark>) -> String {
        paint(&StyledText { text: String::from(text), styles }, &[], colors)
    }

    fn link(href: &str) -> Style {
        Style::Link { href: String::from(href) }
    }

    #[test]
    fn bold_nested_in_italic_sets_both_then_italic_again() {
        let text = "an italic and bold run\n";
        let styles =
            vec![mark(find(text, "italic and bold run"), Style::Italic), mark(find(text, "bold"), Style::Bold)];
        assert_eq!(painted(text, styles), "an \u{1b}[0;3mitalic and \u{1b}[0;1;3mbold\u{1b}[0;3m run\u{1b}[0m\n");
    }

    #[test]
    fn a_style_over_two_lines_is_reset_before_the_newline_and_set_again_after_it() {
        let text = "one two\nthree four\n";
        let styles = vec![mark(find(text, "two\nthree"), Style::Heading { depth: 0 })];
        assert_eq!(painted(text, styles), "one \u{1b}[0;1mtwo\u{1b}[0m\n\u{1b}[0;1mthree\u{1b}[0m four\n");
    }

    #[test]
    fn an_external_link_across_a_line_break_is_a_hyperlink_on_each_line_after_the_indentation() {
        let text = "see the\n  site here\n";
        let styles = vec![mark(find(text, "the\n  site"), link("https://example.com"))];
        let open = "\u{1b}]8;;https://example.com\u{1b}\\";
        let close = "\u{1b}]8;;\u{1b}\\";
        assert_eq!(
            painted(text, styles),
            format!("see {open}\u{1b}[0;4mthe{close}\u{1b}[0m\n  {open}\u{1b}[0;4msite{close}\u{1b}[0m here\n")
        );
    }

    #[test]
    fn a_lines_indentation_is_never_styled() {
        // The first line's indentation, a line of spaces only, and the indentation of a
        // continued line, all inside one bold range.
        let text = "  one\n   \n  two\n";
        let styles = vec![mark(0..text.len(), Style::Bold)];
        assert_eq!(painted(text, styles), "  \u{1b}[0;1mone\u{1b}[0m\n   \n  \u{1b}[0;1mtwo\u{1b}[0m\n");
    }

    #[test]
    fn a_link_inside_the_document_is_left_unstyled() {
        let text = "see Section 2 here\n";
        let styles = vec![mark(find(text, "Section 2"), link("#sec-2"))];
        assert_eq!(painted(text, styles), text);
    }

    #[test]
    fn a_target_holding_a_space_and_a_non_ascii_character_is_percent_encoded() {
        let text = "a link\n";
        let styles = vec![mark(find(text, "link"), link("https://example.com/a b/é?q=1%20"))];
        let painted = painted(text, styles);
        assert!(painted.contains("\u{1b}]8;;https://example.com/a%20b/%C3%A9?q=1%20\u{1b}\\"), "{painted:?}");
    }

    #[test]
    fn an_escape_character_in_a_target_cannot_end_the_sequence() {
        let text = "a link\n";
        let styles = vec![mark(find(text, "link"), link("https://x/\u{1b}\\\u{7}"))];
        let painted = painted(text, styles);
        assert!(painted.contains("\u{1b}]8;;https://x/%1B\\%07\u{1b}\\"), "{painted:?}");
    }

    #[test]
    fn control_characters_are_replaced_and_the_marks_keep_their_place() {
        let text = "a\u{1b}[31m b\u{7}old\u{85} c\td\r\n";
        let styles = vec![mark(find(text, "b\u{7}old\u{85}"), Style::Bold)];
        assert_eq!(painted(text, styles), "a\u{fffd}[31m \u{1b}[0;1mb\u{fffd}old\u{fffd}\u{1b}[0m c\td\u{fffd}\n");
    }

    #[test]
    fn an_empty_output_paints_an_empty_text() {
        assert_eq!(paint(&StyledText::default(), &[], Colors::Standard), "");
    }

    #[test]
    fn styles_painting_nothing_leave_the_text_unchanged() {
        let text = "code and words\n";
        let styles = vec![
            mark(find(text, "code"), Style::Monospace),
            mark(find(text, "and"), Style::Verbatim),
            mark(find(text, "words"), Style::SmallCaps),
        ];
        assert_eq!(painted(text, styles), text);
    }

    #[test]
    fn an_inner_link_holds_its_own_target_while_it_lasts() {
        let text = "outer inner outer\n";
        let (a, b, close) = ("\u{1b}]8;;https://a\u{1b}\\", "\u{1b}]8;;https://b\u{1b}\\", "\u{1b}]8;;\u{1b}\\");
        let expected = format!("{a}\u{1b}[0;4mouter {close}{b}inner{close}{a} outer{close}\u{1b}[0m\n");
        let styles = vec![mark(0..17, link("https://a")), mark(find(text, "inner"), link("https://b"))];
        assert_eq!(painted(text, styles), expected);
    }

    #[test]
    fn marks_nest_whatever_the_order_of_the_list() {
        // The inner link listed first, and an inner link starting where the outer one starts.
        let text = "outer inner\n";
        let (a, b, close) = ("\u{1b}]8;;https://a\u{1b}\\", "\u{1b}]8;;https://b\u{1b}\\", "\u{1b}]8;;\u{1b}\\");
        let styles = vec![mark(find(text, "inner"), link("https://b")), mark(0..11, link("https://a"))];
        assert_eq!(painted(text, styles), format!("{a}\u{1b}[0;4mouter {close}{b}inner{close}\u{1b}[0m\n"));
        let styles = vec![mark(0..5, link("https://b")), mark(0..11, link("https://a"))];
        assert_eq!(painted(text, styles), format!("{b}\u{1b}[0;4mouter{close}{a} inner{close}\u{1b}[0m\n"));
    }

    #[test]
    fn two_adjacent_links_to_one_target_are_two_hyperlinks() {
        let text = "onetwo\n";
        let styles = vec![mark(0..3, link("https://a")), mark(3..6, link("https://a"))];
        let (a, close) = ("\u{1b}]8;;https://a\u{1b}\\", "\u{1b}]8;;\u{1b}\\");
        assert_eq!(painted(text, styles), format!("{a}\u{1b}[0;4mone{close}{a}two{close}\u{1b}[0m\n"));
    }

    #[test]
    fn a_link_with_an_empty_target_is_left_unstyled() {
        let text = "a link\n";
        let styles = vec![mark(find(text, "link"), link(""))];
        assert_eq!(painted(text, styles), text);
    }

    #[test]
    fn a_text_without_a_final_newline_ends_with_everything_closed() {
        let styles = vec![mark(0..4, Style::Bold)];
        assert_eq!(painted("bold", styles), "\u{1b}[0;1mbold\u{1b}[0m");
    }

    #[test]
    fn a_mark_out_of_range_or_off_a_character_boundary_does_not_panic() {
        let styles = vec![mark(0..2, Style::Bold), mark(1..40, Style::Italic)];
        assert_eq!(painted("é\n", styles), "\u{1b}[0;1mé\u{1b}[0m\n");
    }

    #[test]
    fn the_projection_gives_the_text_and_the_style_marks() {
        let projection = json!({
            "text": "A bold link.\n",
            "marks": [{ "addr": "x", "bytes": [0, 1], "start": "0:0", "end": "0:1" }],
            "styles": [
                { "style": "bold", "bytes": [2, 6], "start": "0:2", "end": "0:6" },
                { "style": "link", "href": "https://a", "bytes": [7, 11], "start": "0:7", "end": "0:11" },
                { "style": "small-caps", "bytes": [0, 1], "start": "0:0", "end": "0:1" },
                { "style": "a-style-to-come", "bytes": [0, 1], "start": "0:0", "end": "0:1" },
            ],
        });
        assert_eq!(
            StyledText::from_projection(&projection),
            Some(StyledText {
                text: String::from("A bold link.\n"),
                styles: vec![
                    mark(2..6, Style::Bold),
                    mark(7..11, link("https://a")),
                    mark(0..1, Style::SmallCaps),
                ],
            })
        );
    }

    #[test]
    fn a_projection_of_another_shape_is_refused() {
        for wrong in [
            json!("text"),
            json!({ "text": "a\n" }),
            json!({ "text": 3, "styles": [] }),
            json!({ "text": "a\n", "styles": [{ "style": "bold", "bytes": [0] }] }),
            json!({ "text": "a\n", "styles": [{ "style": "bold", "bytes": [-1, 1] }] }),
            json!({ "text": "a\n", "styles": [{ "style": "link", "bytes": [0, 1] }] }),
            json!({ "text": "a\n", "styles": [{ "bytes": [0, 1] }] }),
        ] {
            assert_eq!(StyledText::from_projection(&wrong), None, "{wrong}");
        }
    }

    /// The SGR sequence that sets `attrs`.
    fn sgr(attrs: Attrs) -> String {
        let mut out = String::new();
        push_sgr(&mut out, attrs);
        out
    }

    #[test]
    fn the_sequence_writer_writes_each_form_in_one_fixed_order() {
        let none = Attrs::default();
        assert_eq!(sgr(none), "\u{1b}[0m");
        let every = Attrs {
            bold: true,
            italic: true,
            underline: true,
            fg: Some(Color::Standard(6)),
            bg: Some(Color::Standard(1)),
        };
        assert_eq!(sgr(every), "\u{1b}[0;1;3;4;36;41m");
        let standard = |fg: u8, bg: u8| sgr(Attrs { fg: Some(Color::Standard(fg)), bg: Some(Color::Standard(bg)), ..none });
        assert_eq!(standard(0, 7), "\u{1b}[0;30;47m");
        assert_eq!(standard(8, 15), "\u{1b}[0;90;107m");
        assert_eq!(standard(14, 9), "\u{1b}[0;96;101m");
        let rgb = Attrs { bold: true, fg: Some(Color::Rgb(1, 2, 3)), bg: Some(Color::Rgb(250, 128, 0)), ..none };
        assert_eq!(sgr(rgb), "\u{1b}[0;1;38;2;1;2;3;48;2;250;128;0m");
        assert_eq!(sgr(Attrs { underline: true, bg: Some(Color::Standard(12)), ..none }), "\u{1b}[0;4;104m");
    }

    #[test]
    fn the_three_values_of_colors_write_no_color_the_nearest_standard_one_or_the_values() {
        let attrs = Attrs { bold: true, fg: Some(Color::Rgb(250, 10, 10)), bg: Some(Color::Standard(4)), ..Attrs::default() };
        assert_eq!(sgr(attrs.under(Colors::None)), "\u{1b}[0;1m");
        assert_eq!(sgr(attrs.under(Colors::Standard)), "\u{1b}[0;1;91;44m");
        assert_eq!(sgr(attrs.under(Colors::Rich)), "\u{1b}[0;1;38;2;250;10;10;44m");
        assert_eq!(Colors::default(), Colors::Standard);
        assert_eq!(["none", "standard", "rich", "blue", ""].map(Colors::named), [
            Some(Colors::None),
            Some(Colors::Standard),
            Some(Colors::Rich),
            None,
            None
        ]);
    }

    #[test]
    fn a_color_given_by_its_values_is_quantized_to_the_nearest_standard_color() {
        // Each standard color's own values give that color.
        for (n, (r, g, b)) in (0u8..).zip(STANDARD_RGB) {
            assert_eq!(nearest_standard(r, g, b), n, "{n}");
        }
        assert_eq!(nearest_standard(10, 10, 10), 0);
        assert_eq!(nearest_standard(140, 140, 140), 8);
        assert_eq!(nearest_standard(200, 200, 210), 7);
        assert_eq!(nearest_standard(240, 40, 20), 9);
        assert_eq!(nearest_standard(255, 128, 0), 3);
        assert_eq!(nearest_standard(30, 30, 200), 4);
        assert_eq!(nearest_standard(100, 100, 250), 12);
        assert_eq!(nearest_standard(0, 180, 170), 6);
        // A dark blue at the same distance from black and from blue gives the lower number.
        assert_eq!(nearest_standard(0, 0, 119), 0);
    }

    #[test]
    fn the_palette_paints_every_style_name_the_projection_writes() {
        // Every name the text output's projection writes, with what the palette makes of it
        // under the standard colors, its SGR parameters; a link follows, with its hyperlink.
        let names: [(&str, Value, Option<&str>); 13] = [
            ("heading", json!(0), Some("0;1;96")),
            ("heading", json!(1), Some("0;1;36")),
            ("heading", json!(3), Some("0;1;36")),
            ("heading-rule", json!(0), Some("0;36")),
            ("heading-rule", json!(2), Some("0;36")),
            ("table-border", Value::Null, Some("0;90")),
            ("bold", Value::Null, Some("0;1")),
            ("italic", Value::Null, Some("0;3")),
            ("emphasis", Value::Null, Some("0;3")),
            ("underline", Value::Null, Some("0;4")),
            ("monospace", Value::Null, None),
            ("small-caps", Value::Null, None),
            ("verbatim", Value::Null, None),
        ];
        for (name, depth, expected) in names {
            let projection = json!({
                "text": "x\n",
                "styles": [{ "style": name, "depth": depth, "bytes": [0, 1] }],
            });
            let styled = StyledText::from_projection(&projection).unwrap_or_else(|| panic!("{name}"));
            let [mark] = styled.styles.as_slice() else { panic!("`{name}` is read: {styled:?}") };
            let painted = palette(&mark.style).map(|(attrs, link)| (sgr(attrs.under(Colors::Standard)), link));
            let expected = expected.map(|parameters| (format!("\u{1b}[{parameters}m"), None));
            assert_eq!(painted, expected, "{name} {depth}");
        }
        for (href, expected) in [("https://a", Some(("\u{1b}[0;4;94m", Some("https://a")))), ("#sec", None), ("", None)] {
            let projection = json!({ "text": "x\n", "styles": [{ "style": "link", "href": href, "bytes": [0, 1] }] });
            let styled = StyledText::from_projection(&projection).unwrap();
            let painted = palette(&styled.styles[0].style).map(|(attrs, link)| (sgr(attrs.under(Colors::Standard)), link));
            assert_eq!(painted, expected.map(|(parameters, link)| (String::from(parameters), link)), "{href}");
        }
    }

    #[test]
    fn a_heading_or_heading_rule_without_its_depth_is_refused() {
        for style in ["heading", "heading-rule"] {
            for depth in [Value::Null, json!(-1), json!(256), json!("0")] {
                let projection = json!({ "text": "x\n", "styles": [{ "style": style, "depth": depth, "bytes": [0, 1] }] });
                assert_eq!(StyledText::from_projection(&projection), None, "{style} {depth}");
            }
        }
    }

    #[test]
    fn colors_join_the_state_and_an_inner_mark_s_color_wins_while_it_lasts() {
        // A link inside a heading: bold from the heading, the link's color and underline.
        let text = "# Title link\n";
        let styles = vec![
            mark(0..2, Style::HeadingRule { depth: 2 }),
            mark(0..12, Style::Heading { depth: 2 }),
            mark(find(text, "link"), link("https://a")),
        ];
        let (a, close) = ("\u{1b}]8;;https://a\u{1b}\\", "\u{1b}]8;;\u{1b}\\");
        assert_eq!(
            painted_in(Colors::Standard, text, styles.clone()),
            format!("\u{1b}[0;1;36m# Title {a}\u{1b}[0;1;4;94mlink{close}\u{1b}[0m\n")
        );
        // Under `none`, the same marks write no color, and nothing where only a color
        // changes.
        assert_eq!(
            painted_in(Colors::None, text, styles),
            format!("\u{1b}[0;1m# Title {a}\u{1b}[0;1;4mlink{close}\u{1b}[0m\n")
        );
    }

    #[test]
    fn a_cell_s_colors_replace_the_marks_colors_and_a_leading_cell_is_not_indentation() {
        // A line indented by two spaces, whose grid starts with a space cell, inside a bold
        // mark that sets no color; the second cell has both colors.
        let text = "   \u{2580}x\n";
        let space = Cell { at: 2, colors: CellColors { fg: None, bg: Some(Rgb(1, 2, 3)) } };
        let half = Cell { at: 3, colors: CellColors { fg: Some(Rgb(250, 10, 10)), bg: Some(Rgb(0, 0, 0)) } };
        let styled = StyledText { text: String::from(text), styles: vec![mark(0..7, Style::Bold)] };
        assert_eq!(
            paint(&styled, &[half, space], Colors::Rich),
            "  \u{1b}[0;1;48;2;1;2;3m \u{1b}[0;1;38;2;250;10;10;48;2;0;0;0m\u{2580}\u{1b}[0;1mx\u{1b}[0m\n"
        );
        assert_eq!(
            paint(&styled, &[space, half], Colors::Standard),
            "  \u{1b}[0;1;40m \u{1b}[0;1;91;40m\u{2580}\u{1b}[0;1mx\u{1b}[0m\n"
        );
        // Under `none`, the cells write no color, and the leading space stays plain.
        assert_eq!(paint(&styled, &[space, half], Colors::None), painted(text, vec![mark(0..7, Style::Bold)]));
        // A cell off a character's offset is left out.
        let off = Cell { at: 4, colors: half.colors };
        assert_eq!(paint(&styled, &[off], Colors::Rich), paint(&styled, &[], Colors::Rich));
    }
}
