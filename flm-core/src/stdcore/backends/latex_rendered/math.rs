//! The writing of mathematics on the `latex-rendered` backend: a formula's
//! delimiters around its source, written byte for byte, with the content of a
//! text command written inside it.
//!
//! This is the one place the backend writes mathematics, so that the
//! mathematics build's rename of the element touches one file. An inline
//! formula is written `\(`…`\)`; a displayed formula without an environment
//! `\[`…`\]`, on a line of its own; a displayed environment
//! `\begin{<env>*}`…`\end{<env>*}`, the environment starred so that LaTeX
//! numbers no line itself. Between the delimiters the source is written as
//! the document gave it, and a `flm.pres.math-text` node standing in it is
//! written `\<command>{<content>}` (`\text` where the node names no command),
//! `\flmxTarget{<anchor>}` where it carries an anchor, and its content alone
//! at a tag, where the source already holds `\tag*{` around it.

use techy::core::node::NodeRef;

use crate::lang::Flm;
use crate::model::element::element_of;
use crate::stdcore::backends::latex::LatexRaw;
use crate::stdcore::backends::pres::{
    self, MathDisplay, MathPassthroughPayload, MathTextPayload, payload_of,
};

use super::emit::Emitter;

impl Emitter<'_> {
    /// A `flm.pres.math-passthrough` node.
    pub(super) fn math_passthrough(&mut self, node: &NodeRef<'_, Flm>) {
        let payload = payload_of::<MathPassthroughPayload>(node);
        let display = payload.map_or(MathDisplay::Inline, |payload| payload.display);
        let env = payload.and_then(|payload| payload.env.as_deref());
        let (open, close) = match (display, env) {
            (MathDisplay::Inline, _) => (String::from("\\("), String::from("\\)")),
            (_, None) => (String::from("\\["), String::from("\\]")),
            (_, Some(env)) => {
                let starred = if env.ends_with('*') { env.to_string() } else { format!("{env}*") };
                (format!("\\begin{{{starred}}}"), format!("\\end{{{starred}}}"))
            }
        };
        self.inline();
        let displayed = display != MathDisplay::Inline;
        if displayed {
            self.line_start();
        }
        self.no_break += 1;
        self.put(&open);
        let content = node.body().map(|slice| slice.iter().collect::<Vec<_>>()).unwrap_or_default();
        self.formula_source(&content);
        self.put(&close);
        self.no_break -= 1;
        if displayed {
            self.newline();
        }
    }

    /// The source of a formula: runs of source nodes byte for byte, a
    /// math-text node where it stands.
    fn formula_source(&mut self, nodes: &[NodeRef<'_, Flm>]) {
        let mut run: Vec<NodeRef<'_, Flm>> = Vec::new();
        for node in nodes {
            if element_of(node).name == pres::MATH_TEXT {
                self.source_run(&run);
                run.clear();
                self.math_text(node);
            } else {
                run.push(*node);
            }
        }
        self.source_run(&run);
    }

    fn source_run(&mut self, run: &[NodeRef<'_, Flm>]) {
        if let Some(first) = run.first() {
            self.put_raw(first, &LatexRaw::from_source(run));
        }
    }

    /// A `flm.pres.math-text` node inside a formula.
    pub(super) fn math_text(&mut self, node: &NodeRef<'_, Flm>) {
        let payload = payload_of::<MathTextPayload>(node);
        if let Some(anchor) = payload.and_then(|payload| payload.anchor.as_deref()) {
            self.target(anchor);
        }
        let content = node.body().map(|slice| slice.iter().collect::<Vec<_>>()).unwrap_or_default();
        if content.is_empty() {
            return;
        }
        if payload.is_some_and(|payload| payload.at_tag) {
            self.inline_content(Some(node), &content);
            return;
        }
        let command = payload.and_then(|payload| payload.command.as_deref()).unwrap_or("text");
        self.put(&format!("\\{command}{{"));
        self.inline_content(Some(node), &content);
        self.put("}");
    }
}
