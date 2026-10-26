//! The optional argument of an xnote, which holds the `\label{…}` commands
//! that label the xnote and nothing else: [`labels_of`] reads it, and
//! [`Rejected`] records what it refuses.

use techy::core::node::NodeRef;

use crate::lang::text::{is_blank, is_space};
use crate::lang::{Flm, FlmSpan, structural};
use crate::stdcore::features::refs::{self, LabelUse};

use super::LABELS_ARG;

/// Something written in the optional argument of an xnote that is neither a
/// `\label{…}`, whitespace nor a comment (text, a group, `\textbf{…}`, a
/// command that no feature defines there), as [`labels_of`] returns it: what
/// was written, and where.
///
/// Such a thing is ignored. The lowering rule of the xnote reports the first
/// one of each xnote as the Error
/// [`BadLabelArgument`](super::BadLabelArgument).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected {
    /// The node as written in the source, trimmed, and cut after forty
    /// characters with `…`.
    pub found: String,
    /// The node's span.
    pub span: FlmSpan,
}

/// The longest text a [`Rejected`] quotes, in characters.
const QUOTED_CHARS: usize = 40;

/// Reads the optional argument [`LABELS_ARG`] of the xnote node `node`: the
/// `\label{…}` commands written there, in order, and every other thing
/// written there that is not whitespace or a comment.
///
/// The labels are read by the references feature's own reader,
/// [`refs::label_uses_in`], so a `\label` here is read exactly as one written
/// after a heading; [`refs::point_labels`] splits them into what a reference
/// point declares. The labels count regardless of what else the argument
/// holds: `\footnote[\textbf{x}\label{fn:e}]{…}` is labeled `fn:e`, and its
/// `\textbf{x}` is rejected.
///
/// A `\label` counts only where the references feature declares it, in its
/// scoped package [`refs::LABEL_PACKAGE`]: in a registry without that
/// feature, a `\label` in the argument is an unknown command and is rejected
/// like any other. For a node with no such argument, or with the argument not
/// written, the function returns two empty lists.
pub fn labels_of(node: &NodeRef<'_, Flm>) -> (Vec<LabelUse>, Vec<Rejected>) {
    let Ok(Some(run)) = node.argument_content_nodes_named(LABELS_ARG) else {
        return (Vec::new(), Vec::new());
    };
    let uses = refs::label_uses_in(run.iter());
    let rejected = run
        .iter()
        .filter(|node| refs::label_use(node).is_none() && !is_layout(node))
        .map(|node| Rejected { found: quoted(node.span_content()), span: node.span().clone() })
        .collect();
    (uses, rejected)
}

/// Whether `node` is nothing but layout: whitespace ([`is_blank`]: text made
/// of whitespace alone, or a node that is whitespace, such as `~` and `\ `), a
/// comment, or the blank line that the block-structure pass reports in an
/// inline argument.
fn is_layout(node: &NodeRef<'_, Flm>) -> bool {
    node.is_comment() || is_blank(node) || node.ext().element.name.as_str() == structural::PARBREAK
}

/// `text` without the whitespace ([`is_space`]) at either end, cut after
/// [`QUOTED_CHARS`] characters with `…`.
fn quoted(text: &str) -> String {
    let text = text.trim_matches(is_space);
    match text.char_indices().nth(QUOTED_CHARS) {
        Some((at, _)) => format!("{}\u{2026}", &text[..at]),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_text_is_cut() {
        assert_eq!(quoted("  \\textbf{x}  "), "\\textbf{x}");
        let long = "a".repeat(50);
        assert_eq!(quoted(&long), format!("{}\u{2026}", "a".repeat(40)));
        assert_eq!(quoted(&"é".repeat(40)), "é".repeat(40));
    }
}
