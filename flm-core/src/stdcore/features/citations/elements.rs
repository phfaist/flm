//! The two elements of the `citations` feature, `\cite` and `\R`, their
//! argument declarations, and the reader of a citation chunk.
//!
//! A citation chunk is a `\cite` together with the `\cite` calls written
//! directly after it, or the `\cite` calls written directly after an `\R`
//! (see the [module documentation](super#citation-chunks)). The further calls
//! are fields named `cite` ([`CITE_FIELD`]) of a tack-on argument (the
//! trailing argument that absorbs field macros; see
//! [fields](crate::guide::concepts_overview#fields-data-a-construct-reads)):
//! [`MORE_ARG`] on `\cite`, [`CITATION_ARG`] on `\R`. [`read_chunk`] reads a
//! citation chunk; the stages and the general lowering rules read through it,
//! and the rules of the recomposed LaTeX read the same way, one call at a
//! time, so every reader gets the same citation keys.

use std::sync::{Arc, LazyLock};

use techy::core::node::NodeRef;
use techy::extract::split_at_chars_drop_annotations;

use crate::core::lowering::{spelling_at_lowering, LowerError};
use crate::lang::mint::{is_field_named, ArgDef, ArgStdKind, TackOnField};
use crate::lang::{Flm, FlmSpan};
use crate::model::aspects::{
    ApartFromText, BlockRole, BlockRoleAspect, ContentRef, FixedBlockRole, InlineSpacing,
};
use crate::model::element::{element_value, ElementSpec};
use crate::stdcore::backends::pres::spelling_identity;

/// The name of the element of `\cite`, `flm.citations.cite`. One node of it
/// carries one citation chunk: its own call and the calls written directly
/// after it, held in its tack-on argument [`MORE_ARG`].
pub const CITE: &str = "flm.citations.cite";

/// The name of the element of `\R`, `flm.citations.refcite`. One node of it
/// carries the citation chunk written directly after the `\R`, shown as a
/// noun of its sentence with the citation counter style's word before its
/// citation mark.
pub const REFCITE: &str = "flm.citations.refcite";

/// The name of a call's optional note argument, `note`.
pub const NOTE_ARG: &str = "note";

/// The name of a call's citation keys argument, `keys`.
pub const KEYS_ARG: &str = "keys";

/// The argument role of citation keys, `flm.citations.key`: the role of the
/// keys argument ([`KEYS_ARG`]) of `\cite` and of the `cite` fields that
/// `\cite` and `\R` take.
pub const KEY_ARG_ROLE: &str = "flm.citations.key";

/// The name of the tack-on argument of `\cite` that holds the further calls of
/// its citation chunk, `more`.
pub const MORE_ARG: &str = "more";

/// The name of the field that each further call of a citation chunk is read
/// as, `cite`: `\cite{a}\cite{b}` is one `\cite` node whose argument
/// [`MORE_ARG`] holds one field named `cite`. The `\cite` written after an
/// `\R` is also a field of this name, in the argument [`CITATION_ARG`].
pub const CITE_FIELD: &str = "cite";

/// The name of `\R`'s optional flags argument, `flags`.
pub const FLAGS_ARG: &str = "flags";

/// The name of `\R`'s tack-on argument, `citation`, which holds the `\cite`
/// written directly after the `\R` as a field named [`CITE_FIELD`].
pub const CITATION_ARG: &str = "citation";

/// The per-value data of the two elements: whether a node is a `\cite` or an
/// `\R`, the two kinds of node that carry a citation chunk.
///
/// [`read_chunk`] reads a citation chunk from either: from a `\cite` node's
/// own arguments and its further calls, or from the field `cite` written
/// after an `\R`. The stages find the nodes carrying a citation chunk by this
/// type (`walk_values_of::<CitationElement>` over the element index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CitationElement {
    /// `\cite[note]{keys}`, attached to the content before it.
    Cite,
    /// `\R[flags]\cite[note]{keys}`, a noun of its sentence, its citation mark
    /// standalone.
    RefCite,
}

/// The citation keys of a citation chunk are not words of the text around
/// it, and neither are the flags of an `\R`; the notes are. A heading without
/// a label gets an identifier encoded from the text of its title
/// ([`ApartFromText`]); when the title holds `\cite[Thm 4]{a}`, that text is
/// the title's own words and `Thm 4`, without the citation key `a`.
impl ApartFromText for CitationElement {
    fn apart_from_text(&self, node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        let mut apart: Vec<ContentRef> = calls(node)
            .iter()
            .filter_map(|call| call.argument_content_nodes_named(KEYS_ARG).ok().flatten())
            .flat_map(|keys| keys.iter().map(|key| ContentRef::Node(key.id())).collect::<Vec<_>>())
            .collect();
        if *self == CitationElement::RefCite {
            apart.push(ContentRef::ArgumentNamed(FLAGS_ARG.to_string()));
        }
        apart
    }
}

/// The note and the keys of one call, the two arguments `\cite` and its field
/// share.
fn call_args() -> Vec<ArgDef> {
    vec![
        ArgDef::named(NOTE_ARG, ArgStdKind::Optional)
            .docs("A note shown after the numbers, `[1; Thm 4]`; it belongs to every citation key of the call."),
        ArgDef::named(KEYS_ARG, ArgStdKind::Mandatory)
            .docs(
                "The citation keys of the cited works, separated by commas; a citation key is FLM code, kept as typed, and a comma inside braces belongs to it.",
            )
            .role(KEY_ARG_ROLE)
            .with_opaque_wrapper(),
    ]
}

/// The field `cite` that holds one further call of a citation chunk, and may
/// be repeated: the note and the citation keys of that call.
fn more_field() -> TackOnField {
    TackOnField::new(CITE_FIELD, call_args())
        .repeatable()
        .docs("A further call of the citation chunk, `\\cite[note]{keys}`; may be repeated.")
}

/// The tack-on argument [`MORE_ARG`], which holds the further calls of a
/// citation chunk.
fn more_arg() -> ArgDef {
    ArgDef::named(MORE_ARG, ArgStdKind::TackOn { fields: vec![more_field()] })
        .docs("Further `\\cite` calls written directly after this one, read with it as one citation chunk.")
}

/// The argument declarations of `\cite`, in order: the note ([`NOTE_ARG`]),
/// the citation keys ([`KEYS_ARG`]), and the further calls of the citation
/// chunk ([`MORE_ARG`]).
pub fn cite_args() -> Vec<ArgDef> {
    let mut args = call_args();
    args.push(more_arg());
    args
}

/// The argument declarations of `\R`, in order: the flags ([`FLAGS_ARG`]),
/// then the tack-on argument [`CITATION_ARG`], which takes the `\cite`
/// written directly after the `\R` as a field named `cite` with `\cite`'s
/// three arguments. `\R[S]\cite{a,b}\cite{c}` therefore reads as one citation
/// chunk, and `\R[S]{\cite{a}}` holds no citation, because the `\cite` is
/// inside a group.
pub fn refcite_args() -> Vec<ArgDef> {
    let mut field_args = call_args();
    field_args.push(more_arg());
    let field = TackOnField::new(CITE_FIELD, field_args)
        .docs("The citation that `\\R` writes as a noun: `\\cite[note]{keys}`, with its further calls.");
    vec![
        ArgDef::named(FLAGS_ARG, ArgStdKind::Optional)
            .docs("Flags: `S` writes the capitalized word before the citation mark (`Refs.~`), `I` writes no word.")
            .with_opaque_wrapper(),
        ArgDef::named(CITATION_ARG, ArgStdKind::TackOn { fields: vec![field] })
            .docs("The citation written directly after `\\R`."),
    ]
}

static CITE_ELEMENT: LazyLock<Arc<ElementSpec>> = LazyLock::new(|| {
    // A mark is attached to the content before it: the whitespace the author
    // wrote before `\cite` is removed, and the rule writes the space the form
    // calls for.
    let role = BlockRole::Inline(InlineSpacing { absorb_before: true, absorb_after: false });
    Arc::new(
        ElementSpec::new(CITE)
            .docs("A citation chunk, `\\cite[note]{keys}`, attached to the content before it.")
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(role)))
            .with::<dyn ApartFromText>(Arc::new(CitationElement::Cite))
            .with_value(Arc::new(CitationElement::Cite)),
    )
});

/// The element value of every `\cite` node (one value, shared by all of
/// them), as the feature registers it.
pub fn cite_element() -> Arc<ElementSpec> {
    Arc::clone(&CITE_ELEMENT)
}

static REFCITE_ELEMENT: LazyLock<Arc<ElementSpec>> = LazyLock::new(|| {
    Arc::new(
        ElementSpec::new(REFCITE)
            .docs("A citation chunk written as a noun, `\\R\\cite{keys}`, after a word, `ref.~` or `refs.~` by default.")
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT)))
            .with::<dyn ApartFromText>(Arc::new(CitationElement::RefCite))
            .with_value(Arc::new(CitationElement::RefCite)),
    )
});

/// The element value of every `\R` node (one value, shared by all of them),
/// as the feature registers it.
pub fn refcite_element() -> Arc<ElementSpec> {
    Arc::clone(&REFCITE_ELEMENT)
}

// --- the chunk reader ------------------------------------------------------------------------

/// The fields named [`CITE_FIELD`] among the content of the argument `arg`
/// of `node`, in order.
fn fields_in<'t>(node: &NodeRef<'t, Flm>, arg: &str) -> Vec<NodeRef<'t, Flm>> {
    match node.argument_content_nodes_named(arg) {
        Ok(Some(content)) => content.iter().filter(|field| is_field_named(field, CITE_FIELD)).collect(),
        _ => Vec::new(),
    }
}

/// Returns the first call of the citation chunk that `node` carries: the
/// `\cite` node itself, or the field `cite` written after an `\R`. Returns
/// `None` for an `\R` with no `\cite` directly after it, and for a node of
/// neither element.
pub fn leading_call<'t>(node: &NodeRef<'t, Flm>) -> Option<NodeRef<'t, Flm>> {
    match element_value::<CitationElement>(node)? {
        CitationElement::Cite => Some(*node),
        CitationElement::RefCite => fields_in(node, CITATION_ARG).into_iter().next(),
    }
}

/// Returns every call of the citation chunk that `node` carries, in the order
/// written: the first call ([`leading_call`]), then the further calls held in
/// its argument [`MORE_ARG`]. Each call holds the arguments [`NOTE_ARG`] and
/// [`KEYS_ARG`]. Returns an empty list where [`leading_call`] returns
/// `None`.
pub fn calls<'t>(node: &NodeRef<'t, Flm>) -> Vec<NodeRef<'t, Flm>> {
    let Some(leader) = leading_call(node) else { return Vec::new() };
    let mut out = vec![leader];
    out.extend(fields_in(&leader, MORE_ARG));
    out
}

/// One item of a citation chunk: a citation key, and the note of the call it
/// was written in.
#[derive(Debug, Clone)]
pub struct CitationItem<'t> {
    /// The citation key: the FLM code written for it, recomposed from the
    /// parsed nodes, with every run of whitespace made one space and the ends
    /// trimmed ([`spelling_identity`]).
    pub key: String,
    /// The content of the call's note, when the call has one: the same nodes
    /// for every citation key of that call.
    pub note: Option<Vec<NodeRef<'t, Flm>>>,
}

/// A mistake in the writing of one call of a citation chunk, found by
/// [`read_chunk`]; the call contributes no item. [`report_mistakes`](super::report_mistakes)
/// reports each one as a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkMistake {
    /// The call holds no citation key: `\cite{}`, `\cite{ , }`. Reported as
    /// [`NoKey`](super::NoKey).
    NoKey {
        /// The call's span.
        span: FlmSpan,
    },
}

/// A citation chunk as [`read_chunk`] returns it: its items in the order
/// written, and the mistakes of its calls.
#[derive(Debug, Clone, Default)]
pub struct Chunk<'t> {
    /// The items, one per key, in the order written: `\cite[Thm 4]{a,b}`
    /// gives `(a, Thm 4)` and `(b, Thm 4)`.
    pub items: Vec<CitationItem<'t>>,
    /// The calls that contribute no item, and why.
    pub mistakes: Vec<ChunkMistake>,
}

/// Where a citation chunk is read: in a lowering rule or in a stage. The two
/// differ in how they treat citation keys that hold a placeholder of a user
/// definition's template (`\cite{arxiv:#1}` inside a template). Such citation
/// keys are not known until the template is expanded, because a placeholder
/// has no spelling of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    /// In a lowering rule: the citation keys are checked with
    /// [`spelling_at_lowering`], and a placeholder among them makes
    /// [`read_chunk`] return [`LowerError::PlaceholderCharacters`]. The rule
    /// returns that error unchanged, and the user definition's use sites are
    /// then expanded rather than written as a definition.
    AtLowering,
    /// In a stage: a call whose citation keys hold a placeholder gives no item
    /// and no mistake.
    InStage,
}

/// Reads the citation chunk that `node` carries (a `\cite` node, or an `\R`
/// node through the field written after it) as a list of items, each a
/// citation key and its call's note, in the order written. It serves both
/// elements, in a lowering rule and in a stage (`reading`).
///
/// Each call's citation keys are its keys argument, FLM content, split at the
/// commas that are outside every group, command argument and formula
/// (`\cite{a,{b,c},d}` gives `a`, `{b,c}`, `d`). Each part is recomposed to
/// the FLM code it was written as, with every run of whitespace made one space
/// and the ends trimmed ([`spelling_identity`]), so a citation key holding
/// braces or a command keeps them. An empty part is skipped (`\cite{a,,b}`
/// gives `a`, `b`). The reader reads no meaning into a citation key: the
/// citation providers decide what it refers to. A call with no citation key
/// gives no item and a [`ChunkMistake`]. An `\R` with no `\cite` directly
/// after it gives an empty citation chunk.
///
/// # Errors
///
/// [`LowerError::PlaceholderCharacters`] under [`Reading::AtLowering`], for
/// citation keys that hold a placeholder of a user definition's template that
/// is being lowered without expansion.
pub fn read_chunk<'t>(node: &NodeRef<'t, Flm>, reading: Reading) -> Result<Chunk<'t>, LowerError> {
    let mut chunk = Chunk::default();
    for call in calls(node) {
        read_call(&call, reading, &mut chunk)?;
    }
    Ok(chunk)
}

/// Reads one call of a citation chunk (a call that [`calls`] returns) into
/// `chunk`: its items are appended in the order written, or its
/// [`ChunkMistake`] is recorded, exactly as [`read_chunk`] reads each call.
/// The recomposed LaTeX's rules read a citation chunk call by call, because
/// they write each call as the author wrote it.
///
/// # Errors
///
/// As [`read_chunk`].
pub(super) fn read_call<'t>(call: &NodeRef<'t, Flm>, reading: Reading, chunk: &mut Chunk<'t>) -> Result<(), LowerError> {
    let keys = match call.argument_content_nodes_named(KEYS_ARG) {
        Ok(Some(content)) => Some(content),
        _ => None,
    };
    if let Some(keys) = keys {
        // The source recomposer re-emits nothing for a placeholder, so keys
        // holding one are not known until the template is expanded.
        let known = spelling_at_lowering(keys.iter());
        match reading {
            Reading::AtLowering => known?,
            Reading::InStage => {
                if known.is_err() {
                    return Ok(());
                }
            }
        }
    }
    let note: Option<Vec<NodeRef<'t, Flm>>> = match call.argument_content_nodes_named(NOTE_ARG) {
        Ok(Some(content)) => Some(content.iter().collect()),
        _ => None,
    };
    let before = chunk.items.len();
    // Splitting fails only on a broken tree (techy cannot build the
    // segments' tree); such keys give no item.
    if let Some(Ok(segments)) = keys.map(|keys| split_at_chars_drop_annotations(keys, ",")) {
        for segment in segments.segments() {
            let key = spelling_identity(segment.iter());
            if !key.is_empty() {
                chunk.items.push(CitationItem { key, note: note.clone() });
            }
        }
    }
    if chunk.items.len() == before {
        chunk.mistakes.push(ChunkMistake::NoKey { span: call.span().clone() });
    }
    Ok(())
}
