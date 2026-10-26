//! Reading the characters of content: the plain text of a run of nodes or of a
//! synthesized value ([`plain_text_of`], [`plain_text`]), the characters of an
//! argument that holds only characters, read by segments ([`read_chars`],
//! [`chars_text`]), and the whitespace of FLM text.
//!
//! # Plain text
//!
//! The **plain text** of content is the characters of its text nodes, in
//! document order. Everything else (comments, delimiters, the text of a macro or
//! environment command) is dropped, and nothing is put in its place. Content
//! that an element declares as not part of the text around it
//! ([`ApartFromText`](crate::model::aspects::ApartFromText)), such as a
//! footnote's words inside a heading's title, is dropped too. Use the plain
//! text where the words of some content are needed rather than a rendering of
//! it: the text of a reference point's content when a table is printed for
//! inspection (the [projection](crate::dump::project) of the table), or a
//! label compared with the text it refers to. [`plain_text`] reads the plain
//! text of a [synthesized
//! value](crate::guide::concepts_overview#synthesized-values), a small tree
//! held outside the document, which [`copy`](super::copy) builds.
//!
//! # Reading an argument as characters
//!
//! A label, a reference target and an address are written in arguments whose
//! content is characters: the characters-only group of `\label{…}` and
//! `\ref{…}`, verbatim content, or an ordinary argument that a command reads as
//! characters. [`read_chars`] is the function that reads such content. Where
//! [`plain_text_of`] skips everything that is not text, [`read_chars`] refuses it:
//! content holding a command (`\ref{\emph{x}}`, after a user definition put
//! `\emph{x}` into the target) or mathematics has no characters to read, and
//! [`read_chars`] returns [`NotCharacters`].
//!
//! One kind of node is neither characters nor refused: the **placeholder** of a
//! [user definition](crate::lang::rewrites::userdefs::Definition)'s template
//! (`#1`, `#{name}`, `#[…]{ref}`). A template that is lowered unexpanded (the
//! template of a [kept definition](crate::stdcore::features::userdefs::kept),
//! which the [recomposed LaTeX
//! backend](crate::stdcore::backends::latex_recomposed) writes back as a LaTeX
//! definition) still holds its placeholders when a label or a target inside it
//! is read. [`read_chars`] therefore returns the content as
//! [segments](CharsSegment), in order: runs of characters and placeholder
//! nodes. Each caller decides what a placeholder means:
//!
//! - A placeholder stands for characters that are not known yet (each use of
//!   the definition supplies them). A reader of facts about the document (a
//!   label, the target of a reference) therefore reads no label and no target
//!   where one is written; [`CharsContent::text`] and [`chars_text`] return
//!   `None` then.
//! - A lowering rule, which needs the characters to write them, reads them
//!   through [`chars_at_lowering`](crate::core::lowering::chars_at_lowering).
//!   For a placeholder, that function returns an error
//!   ([`LowerError::PlaceholderCharacters`](crate::core::lowering::LowerError::PlaceholderCharacters))
//!   that the rule passes on to the rule that inserted the template: the
//!   user-definitions feature then expands the use sites of the definition
//!   whose template it was lowering, and the lists feature reports a number
//!   format in a list's own label template.
//!
//! Only a template lowered as it stands holds placeholders (on the recomposed
//! LaTeX backend, the template of a kept definition or a list's own label
//! template); everywhere else no placeholder remains, and the content is all
//! characters.
//!
//! # Whitespace
//!
//! Whitespace in FLM text is the four ASCII whitespace characters: the space,
//! the tab, the carriage return and the line feed. Every reader of parsed FLM
//! content (a document, a template) counts whitespace this way: the
//! block-structure pass, the lowering helpers' blank test
//! ([`is_blank`](crate::stdcore::backends::lowering::is_blank)), and each
//! feature's reader where it trims what it reads, splits it at whitespace or
//! tests whether content is blank. Content is blank when it is text made of
//! whitespace alone, or a node whose element carries the
//! [`Whitespace`](crate::model::aspects::Whitespace) aspect, such as `~` and
//! `\ `. Every other character is a character of the text, the no-break space
//! U+00A0 typed directly in the text among them.
//!
//! The one exception is the key of a `key=value` entry. techy's key-value
//! reader ([`parse_keyval`](techy::extract::parse_keyval)), through which the
//! features read such entries, trims each key with [`str::trim`], which removes
//! every Unicode whitespace character. A no-break space U+00A0 typed at either
//! end of a key is therefore removed, and the key is read without it; the
//! entry's value is trimmed by the rule above
//! ([`key_value_pieces`](crate::lang::copy::key_value_pieces)).

use std::collections::HashSet;
use std::fmt;

use techy::core::node::{NodeId, NodeKind, NodeRef, NodeTree};

use crate::lang::{Flm, FlmCallableType, FlmGroupType};
use crate::model::aspects::{apart_from_text_of, is_whitespace_of, ContentRef};

/// Returns the plain text of a run of nodes: the characters of every text node
/// among the nodes and their descendants, in document order.
///
/// Comments, delimiters and the text of commands contribute nothing, and
/// neither does the content an element declares as apart from the text around
/// it ([`apart_from_text_of`]), with its descendants. The nodes may belong to
/// different trees. See the [module documentation](self#plain-text).
pub fn plain_text_of<'t>(nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>) -> String {
    let mut out = String::new();
    // Document order without recursion: the nodes still to read, the next one
    // last; the ones an element set apart are passed over whole.
    let mut pending: Vec<NodeRef<'t, Flm>> = nodes.into_iter().collect();
    pending.reverse();
    // Keyed by tree as well as node, since a run may join nodes of two trees.
    let mut apart: HashSet<(*const NodeTree<Flm>, NodeId)> = HashSet::new();
    while let Some(node) = pending.pop() {
        if apart.contains(&key(&node)) {
            continue;
        }
        if let Some(text) = node.chars() {
            out.push_str(text);
        }
        for content in apart_from_text_of(&node) {
            // A synthesized tree is no part of this one: nothing to pass over.
            if matches!(content, ContentRef::Synthesized(_)) {
                continue;
            }
            apart.extend(content.nodes(&node).into_iter().flatten().map(|inner| key(&inner)));
        }
        let first = pending.len();
        pending.extend(node.children().iter());
        pending[first..].reverse();
    }
    out
}

/// Returns the key by which [`plain_text_of`] identifies a node to skip: its
/// tree and its place there.
fn key(node: &NodeRef<'_, Flm>) -> (*const NodeTree<Flm>, NodeId) {
    (std::ptr::from_ref(node.tree()), node.id())
}

/// Returns the plain text of a synthesized value's tree: [`plain_text_of`] over
/// its root.
pub fn plain_text(tree: &NodeTree<Flm>) -> String {
    plain_text_of(std::iter::once(tree.root()))
}

// --- whitespace -----------------------------------------------------------------------

/// Whether `ch` is whitespace in FLM text: the space, the tab, the carriage
/// return or the line feed (see the [module documentation](self#whitespace)).
/// Every other character is content, the no-break space U+00A0 among them.
pub(crate) fn is_space(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\r' | '\n')
}

/// Whether the byte `byte` of UTF-8 text is whitespace ([`is_space`]); a byte
/// of a character beyond ASCII never is.
pub(crate) fn is_space_byte(byte: u8) -> bool {
    is_space(char::from(byte))
}

/// Whether `text` is blank: empty, or whitespace ([`is_space`]) alone.
pub(crate) fn is_blank_text(text: &str) -> bool {
    text.bytes().all(is_space_byte)
}

/// Whether `node` is blank: text that is blank ([`is_blank_text`]), or a node
/// that is whitespace, such as `~` and `\ ` (its element carries the
/// [`Whitespace`](crate::model::aspects::Whitespace) aspect,
/// [`is_whitespace_of`]). A comment is not blank: a reader that looks past
/// comments tests for them itself.
pub(crate) fn is_blank(node: &NodeRef<'_, Flm>) -> bool {
    is_whitespace_of(node) || node.chars().is_some_and(is_blank_text)
}

/// Returns `text` with every run of whitespace ([`is_space`]) made one space
/// and the whitespace at both ends removed: the rule by which two written
/// texts are the same however their whitespace is laid out.
/// [`spelling_identity`](crate::stdcore::backends::pres::spelling_identity)
/// reads a citation key and a term's identity with it, and
/// [`Label::parse`](crate::model::aspects::Label::parse) each part of a label.
pub(crate) fn collapse_whitespace(text: &str) -> String {
    text.split(is_space).filter(|word| !word.is_empty()).collect::<Vec<_>>().join(" ")
}

// --- reading an argument as characters ------------------------------------------------

/// One segment of content read as characters by [`read_chars`]: a run of
/// characters, or a placeholder node of a user definition's template.
#[derive(Debug, Clone)]
pub enum CharsSegment<'t> {
    /// A run of characters: the text of one or more consecutive text nodes,
    /// joined, without the comments and group delimiters between them.
    Text(String),
    /// A placeholder node (`#1`, `#{name}`, `#[…]{ref}`) among the characters,
    /// in a template that was not expanded: characters that each use of the
    /// definition supplies, not known yet. The segment holds the node, so that a
    /// caller can read which argument it refers to.
    Placeholder(NodeRef<'t, Flm>),
}

impl CharsSegment<'_> {
    /// Returns the characters of a [`Text`](Self::Text) segment; `None` for a
    /// placeholder.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            CharsSegment::Text(text) => Some(text),
            CharsSegment::Placeholder(_) => None,
        }
    }
}

/// Content read as characters by [`read_chars`]: its [segments](CharsSegment),
/// in order. Adjacent characters form one segment, so two text segments never
/// follow each other; empty content has no segment at all.
///
/// Read the characters with [`text`](Self::text) when a placeholder makes the
/// content unusable, or go through the [segments](Self::segments) to handle
/// placeholders.
#[derive(Debug, Clone, Default)]
pub struct CharsContent<'t> {
    segments: Vec<CharsSegment<'t>>,
}

impl<'t> CharsContent<'t> {
    /// Returns the segments, in document order.
    pub fn segments(&self) -> &[CharsSegment<'t>] {
        &self.segments
    }

    /// Returns the segments, consuming the content.
    pub fn into_segments(self) -> Vec<CharsSegment<'t>> {
        self.segments
    }

    /// Returns whether a placeholder is among the segments.
    pub fn has_placeholder(&self) -> bool {
        self.segments.iter().any(|segment| matches!(segment, CharsSegment::Placeholder(_)))
    }

    /// Returns the characters when no placeholder is among them (the empty
    /// string for empty content), and `None` when one is.
    ///
    /// Use it when a placeholder makes the content unusable: a label or a
    /// target holding a placeholder is not a label and refers to nothing.
    pub fn text(&self) -> Option<String> {
        let mut out = String::new();
        for segment in &self.segments {
            out.push_str(segment.as_text()?);
        }
        Some(out)
    }
}

/// The error [`read_chars`] returns for content it cannot read as characters:
/// the content holds a callable node that is not a placeholder (a command, an
/// environment, a special character sequence) or a mathematics group, none of
/// which has characters to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotCharacters {
    /// The first such node, in the tree the content belongs to.
    pub node: NodeId,
}

impl fmt::Display for NotCharacters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the content must be plain characters, without a command, an environment, a \
             special character sequence or mathematics"
        )
    }
}

impl std::error::Error for NotCharacters {}

/// Reads a run of nodes as characters and returns them by segments: text nodes
/// give their characters, comments nothing, groups other than mathematics their
/// children's characters without the delimiters, and a placeholder node of a
/// user definition's template becomes a segment of its own (see the [module
/// documentation](self#reading-an-argument-as-characters)).
///
/// Whitespace is kept as written; the caller trims what it needs to. The input
/// is any run of sibling nodes, typically an argument's content
/// ([`NodeRef::argument_content_nodes`]). A caller that has no use for
/// placeholders can call [`chars_text`] instead.
///
/// # Errors
///
/// [`NotCharacters`] when the content holds, at any depth, a callable node that
/// is not a placeholder, or a mathematics group.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
/// use flm_core::lang::text::read_chars;
/// use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
/// use flm_core::model::doc::FragmentLayout;
/// use flm_core::stdcore::{self, StdcoreConfig};
///
/// let mut builder = RegistryBuilder::new();
/// builder.use_package(stdcore::package(StdcoreConfig::new()));
/// builder.use_package(flm_core::math_passthrough::package());
/// let registry = builder.build(&RegistryInputs::new())?;
/// let source = FlmSource::new(r"See \ref{sec:{intro}}.");
/// let fragment = parse_fragment(
///     registry.lang(),
///     Arc::new(source.with_origin(FlmOrigin::none())),
///     FragmentLayout::default(),
///     &ParseOptions::default(),
/// )?;
/// let reference = fragment.tree().root().descendants()
///     .find(|node| node.name() == Some("ref"))
///     .expect("the reference");
/// let target = reference.argument_content_nodes_named("target")?.expect("written");
/// let content = read_chars(target.iter())?;
/// assert_eq!(content.text().as_deref(), Some("sec:intro"));
/// assert!(!content.has_placeholder());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn read_chars<'t>(
    nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>,
) -> Result<CharsContent<'t>, NotCharacters> {
    let mut segments: Vec<CharsSegment<'t>> = Vec::new();
    // Document order without recursion, as in `plain_text_of`.
    let mut pending: Vec<NodeRef<'t, Flm>> = nodes.into_iter().collect();
    pending.reverse();
    while let Some(node) = pending.pop() {
        match node.kind() {
            NodeKind::Chars { .. } => {
                let text = node.chars().unwrap_or_default();
                match segments.last_mut() {
                    Some(CharsSegment::Text(run)) => run.push_str(text),
                    _ => segments.push(CharsSegment::Text(text.to_string())),
                }
            }
            NodeKind::Comment(_) => {}
            NodeKind::Group(_) if matches!(node.group_type(), Some(FlmGroupType::Math(_))) => {
                return Err(NotCharacters { node: node.id() });
            }
            NodeKind::Group(_) | NodeKind::List => {
                let first = pending.len();
                pending.extend(node.children().iter());
                pending[first..].reverse();
            }
            NodeKind::Callable(_) => {
                if node.callable_type() != Some(FlmCallableType::Placeholder) {
                    return Err(NotCharacters { node: node.id() });
                }
                segments.push(CharsSegment::Placeholder(node));
            }
        }
    }
    Ok(CharsContent { segments })
}

/// Returns the characters of a run of nodes when it holds only characters, and
/// `None` when it holds a command, mathematics or a placeholder. This is
/// [`read_chars`] followed by [`CharsContent::text`].
///
/// Use it when a placeholder makes the content unusable: a label, a target or
/// an address holding a placeholder is treated as absent.
pub fn chars_text<'t>(nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>) -> Option<String> {
    read_chars(nodes).ok()?.text()
}
