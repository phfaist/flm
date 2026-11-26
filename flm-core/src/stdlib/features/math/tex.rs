//! The TeX of a formula, with holes: what a renderer of mathematics converts,
//! written from the formula's nodes.
//!
//! A processing feature that hands its formulas to a typesetter of TeX
//! notation (MathJax, a converter to Unicode text) needs each formula's TeX as
//! a string, except where the formula holds document text: the argument of a
//! text command (`\text{…}`, `\textbf{…}`, `\mbox{…}`, the commands of
//! [`text`](super::text)) is content the feature lowers itself, so that a
//! reference in it is a link and its style is FLM's. This module writes the
//! TeX as a list of [parts](TexPart), each a piece of TeX text or a
//! [hole](TextHole) standing for one text command.
//!
//! # What it reads
//!
//! - [`group_tex`]: a math group (`\(…\)`, `\[…\]`, a node of the element
//!   [`flm.math`](crate::lang::structural::MATH)), whose TeX is its content,
//!   without the delimiters.
//! - [`line_tex`]: one equation line of a display-math environment, an
//!   [`EquationLine`] as [`equation_lines`](super::lines::equation_lines)
//!   reads it, whose TeX is its [content](EquationLine::content) written
//!   under the newline rule (below). A line's `\\`, `\tag`, `\nonumber` and
//!   labels are not part of its content and are never written: they are data
//!   of the line, which its [frame](super::frame::LineFrame) holds. The
//!   environment as a whole is not assembled here: a caller that writes it
//!   writes `\begin{name}`, then per line its content, what its frame shows,
//!   its line break and its trailing run (below), then `\end{name}`, passing
//!   each line's [`LineTex::ends_line`] to [`line_tex`] for the next line.
//!
//! # How the nodes are written
//!
//! The nodes are those of any processor's commands of math mode, each written
//! by its source:
//!
//! - a command of the processor (a macro, an environment) and a run of
//!   characters, as their source as written;
//! - a group, as its delimiters around its content;
//! - `\ensuremath{…}` ([`ensuremath`](super::ensuremath)), as its argument's
//!   content alone, the command left out, since inside a formula it stands for
//!   its argument;
//! - a text command, as a hole, at any depth (inside a group or a command's
//!   argument);
//! - a comment, as one space: the comment takes in its line end, so that
//!   dropping it would run the tokens on either side together (`\alpha%%c`
//!   then `b` on the next line is `\alpha b`, not `\alphab`).
//!
//! A command that holds a text command, a comment or `\ensuremath{…}` is
//! written as its call as written (an environment as `\begin{name}` and
//! `\end{name}` around its arguments and body), then each argument by the same
//! rules; any other node is written as its source, unchanged.
//!
//! Where the TeX written so far ends with a control word (a backslash and
//! letters, `\rangle`) and the next text begins with a letter, one space stands
//! between them, which TeX skips after a control word: the two come from
//! places the source did not write side by side (the end of a user
//! definition's expansion and the letter after its use; `\alpha` and the
//! argument of `\ensuremath{b}`), and the letter would otherwise continue the
//! control word (`\rangleA`, `\alphab`).
//!
//! # The newline rule on a line
//!
//! A line's content is written under the `math` library's newline rule
//! ([`normalize_line_newlines`]; the module documentation of
//! [`latex`](super::latex), *The newline rule*), so that a `\label`, `\tag` or
//! `\nonumber` taken off a line of its own leaves no blank line, which LaTeX
//! refuses inside mathematics. Each TeX part of the content is normalized; the
//! run of whitespace that ends the content is taken off and returned as the
//! line's [`trailing`](LineTex::trailing) run, for the caller to write after
//! the line's tag and its line break; when it is not empty, the run that
//! begins the next line's content is dropped. A hole is not a line's end, and
//! a comment, written as a space, does not end a line either.
//!
//! # The shape of the result
//!
//! The result holds strings and node handles only, so that an extension can
//! receive it across a binding as data: a part is `{tex}` or `{hole: {command,
//! node}}`, and a line adds its trailing run.
//!
//! The functions recompose the source of the nodes they write as source
//! ([`recompose_source`]), so call them on a worker thread, as every stage
//! and lowering rule of a run already executes.

use techy::core::node::NodeRef;
use techy::latexlike::InvocationSyntaxData;

use crate::lang::Flm;
use crate::lang::mint::is_math_text_command;
use crate::model::element::element_of;
use crate::stdlib::backends::pres::read::ends_with_control_word;
use crate::stdlib::backends::pres::recompose_source;

use super::ensuremath::ENSUREMATH;
use super::latex::normalize_line_newlines;
use super::lines::EquationLine;

/// One part of a formula's TeX, in order: TeX text, or a hole for a text
/// command (see the module documentation).
///
/// Two TeX parts never stand side by side: the text between two holes is one
/// part.
#[derive(Debug, Clone)]
pub enum TexPart<'t> {
    /// TeX text, never empty.
    Tex(String),
    /// The place of a text command, which the caller writes.
    Hole(TextHole<'t>),
}

/// A text command inside a formula (`\text{…}`, `\textbf{…}`, `\mbox{…}`; see
/// [`is_math_text_command`]), which the TeX leaves as a hole: the caller
/// lowers its argument itself and decides how the hole is written.
#[derive(Debug, Clone)]
pub struct TextHole<'t> {
    /// The command's name, without the escape character (`textbf`).
    pub command: String,
    /// The text command's node.
    pub node: NodeRef<'t, Flm>,
}

/// The TeX of one equation line's content (see the module documentation,
/// *The newline rule on a line*): what [`line_tex`] returns.
#[derive(Debug, Clone)]
pub struct LineTex<'t> {
    /// The content's TeX, in order, each TeX part under the newline rule.
    pub parts: Vec<TexPart<'t>>,
    /// The run of whitespace that ended the line's content, reduced by the
    /// newline rule, for a caller that assembles the environment to write
    /// after the line's tag and its line break; empty when the content did
    /// not end with a run holding a newline.
    pub trailing: String,
}

impl LineTex<'_> {
    /// Whether this line, once its trailing run is written, ends a line of
    /// the output: the run is not empty. It is the `after_line_end` argument
    /// of [`line_tex`] for the next line of the environment.
    pub fn ends_line(&self) -> bool {
        !self.trailing.is_empty()
    }
}

/// The TeX of the math group `node` (a node of
/// [`flm.math`](crate::lang::structural::MATH)): its content, without its
/// delimiters, written by the rules of the module documentation. A formula
/// that holds no text command gives one TeX part, an empty formula none.
pub fn group_tex<'t>(node: &NodeRef<'t, Flm>) -> Vec<TexPart<'t>> {
    let mut writer = Writer::default();
    writer.nodes(&node.children().iter().collect::<Vec<_>>());
    writer.parts
}

/// The TeX of the equation line `line`'s content, written by the rules of the
/// module documentation, each TeX part under the newline rule, and the run of
/// whitespace that ended the content.
///
/// `after_line_end` says whether the output before this line's content ends a
/// line: `false` for the first line of an environment, and for a later line
/// the previous line's [`LineTex::ends_line`]. The run that begins the content
/// is then dropped.
pub fn line_tex<'t>(line: &EquationLine<'t>, after_line_end: bool) -> LineTex<'t> {
    let mut writer = Writer::default();
    writer.nodes(&line.content);
    let count = writer.parts.len();
    let mut after_line_end = after_line_end;
    let mut parts = Vec::with_capacity(count);
    let mut trailing = String::new();
    for (index, part) in writer.parts.into_iter().enumerate() {
        match part {
            TexPart::Tex(text) => {
                let line = normalize_line_newlines(&text, after_line_end, index + 1 < count);
                if !line.text.is_empty() {
                    parts.push(TexPart::Tex(line.text));
                }
                trailing = line.trailing;
            }
            TexPart::Hole(hole) => {
                parts.push(TexPart::Hole(hole));
                after_line_end = false;
            }
        }
    }
    LineTex { parts, trailing }
}

/// Whether `node` is written by the walk ([`Writer::node`]) rather than as its
/// source: it is, or holds, a text command, a comment or `\ensuremath{…}`.
fn needs_walk(node: &NodeRef<'_, Flm>) -> bool {
    let walked = |node: &NodeRef<'_, Flm>| {
        node.is_comment() || is_math_text_command(node) || element_of(node).name == ENSUREMATH
    };
    walked(node) || node.descendants().any(|descendant| walked(&descendant))
}

/// The beginning of a macro call as written: the escape character, the name,
/// and the whitespace after the name, which belongs to the call and not to
/// the argument that follows it.
fn macro_call(node: &NodeRef<'_, Flm>) -> String {
    let name = node.name().unwrap_or_default();
    match node.invocation_syntax() {
        Some(InvocationSyntaxData::Macro { escape_char, post_space }) => {
            format!("{escape_char}{name}{}", post_space.resolve(node.span().source()))
        }
        _ => format!("\\{name}"),
    }
}

/// Collects the parts of a formula's TeX, joining adjacent TeX text.
#[derive(Debug, Default)]
struct Writer<'t> {
    parts: Vec<TexPart<'t>>,
}

impl<'t> Writer<'t> {
    /// Appends TeX text, one space between a control word that ends the TeX
    /// written so far and a letter that begins `text`.
    fn text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(TexPart::Tex(previous)) = self.parts.last_mut() {
            if text.starts_with(|c: char| c.is_ascii_alphabetic()) && ends_with_control_word(previous) {
                previous.push(' ');
            }
            previous.push_str(text);
        } else {
            self.parts.push(TexPart::Tex(String::from(text)));
        }
    }

    /// Writes the nodes `nodes`: each run of nodes that need no walk as its
    /// source, every other node by the walk.
    fn nodes(&mut self, nodes: &[NodeRef<'t, Flm>]) {
        let mut run: Vec<NodeRef<'t, Flm>> = Vec::new();
        for node in nodes {
            if needs_walk(node) {
                self.source(&mut run);
                self.node(*node);
            } else {
                run.push(*node);
            }
        }
        self.source(&mut run);
    }

    /// Writes the run `run` as its source and empties it.
    fn source(&mut self, run: &mut Vec<NodeRef<'t, Flm>>) {
        if !run.is_empty() {
            self.text(&recompose_source(run));
            run.clear();
        }
    }

    /// Writes one node by the walk (see the module documentation).
    // This walk carries no depth limit of its own, as the source walk of the
    // `math-passthrough` feature carries none: the parse that made the tree
    // refuses a formula nested far less deeply than this walk could go.
    fn node(&mut self, node: NodeRef<'t, Flm>) {
        if node.is_comment() {
            self.text(" ");
            return;
        }
        if is_math_text_command(&node) {
            let command = String::from(node.name().unwrap_or_default());
            self.parts.push(TexPart::Hole(TextHole { command, node }));
            return;
        }
        if element_of(&node).name == ENSUREMATH {
            let content: Vec<NodeRef<'t, Flm>> =
                node.argument_content_nodes(0).map(|slice| slice.iter().collect()).unwrap_or_default();
            self.nodes(&content);
            return;
        }
        if let Some((open, close)) = node.group_delimiters() {
            self.text(open);
            self.nodes(&node.children().iter().collect::<Vec<_>>());
            self.text(close);
            return;
        }
        if node.is_list() {
            self.nodes(&node.children().iter().collect::<Vec<_>>());
            return;
        }
        if let Some(body) = node.body() {
            let name = node.name().unwrap_or_default();
            self.text(&format!("\\begin{{{name}}}"));
            self.arguments(&node);
            self.nodes(&body.iter().collect::<Vec<_>>());
            self.text(&format!("\\end{{{name}}}"));
            return;
        }
        if node.callable().is_some_and(|callable| !callable.arguments.is_empty()) {
            self.text(&macro_call(&node));
            self.arguments(&node);
            return;
        }
        self.text(&recompose_source(&[node]));
    }

    /// Writes the arguments of `node` in the order they were written. The
    /// delimiters of an argument are nodes of the argument, so they are
    /// written with it.
    fn arguments(&mut self, node: &NodeRef<'t, Flm>) {
        let count = node.callable().map_or(0, |callable| callable.arguments.len());
        for index in 0..count {
            if let Some(region) = node.argument_nodes(index) {
                self.nodes(&region.iter().collect::<Vec<_>>());
            }
        }
    }
}
