//! The two elements of the [`defterm`](super) feature, the term definition
//! (`\begin{defterm}{term}…\end{defterm}`, [`DEFTERM`]) and the term link
//! (`\term[ref]{shown}`, [`TERM`]), with the identity of a term, which both
//! compute with the same function, [`spelling_identity`].
//!
//! # The identity of a term
//!
//! A term is matched by what its author wrote, not by what it shows. The
//! **identity** of a term is the source text of the nodes the term is written
//! as, each node written out again from what it records of its own source
//! text (its characters, the command and arguments of a call, the delimiters
//! of a group, a formula as written), with every run of whitespace made one
//! space and the ends trimmed. Source comments between those nodes are left
//! out. `\(\alpha\)-decay` and `\emph{Markov} chain` keep their markup, and
//! `Markov  chain` written across a line break is the same term as `Markov
//! chain`. Matching is exact, case included: `Entropy` and `entropy` are two
//! terms. A term is read in the tree where it occurs, after the same passes
//! over the document as every other term, so the typographic substitutions of
//! the block-structure pass (`--` read as a dash) apply alike to a term
//! definition's term and to a term link's.
//!
//! The source text of each node is read on its own, never as one slice of a
//! source: the term of a term definition written through a user definition's
//! template holds nodes of the template beside nodes of the use site, and
//! each of them reads as it was written.
//!
//! # The term definition
//!
//! A term definition is one [reference
//! point](crate::model::aspects::RefPoint) of the reference kind
//! [`DEFINITION_KIND`], declared by [`DeftermElement`]. Its content, which a
//! reference to it shows, is the term argument. It carries labels of three
//! origins, in this order:
//!
//! 1. one label computed from the term's identity, by which a term link finds
//!    the term definition;
//! 2. one more label computed from the identity of each alias, written
//!    `\aliasterm{…}`, a repeatable field of the environment: another way of
//!    writing the term (for example a plural) under which a term link finds
//!    the same term definition;
//! 3. the labels the author wrote with `\label{…}`, of the types `term` and
//!    `topic`, by which `\ref` and `\hyperref` refer to the term definition.
//!
//! The computed labels are labels of the references feature like any other,
//! so that a term link resolves through that feature's stages. They are
//! never written in a document, and the label types a term definition
//! accepts apply to its written labels alone: a written label of any other
//! type is refused. The point is anchored on its first label: the label
//! computed from the term, or, where the term is empty (and so computes no
//! label), the first alias's label or the first written label.
//!
//! # The term link
//!
//! A term link, [`TermElement`], declares one
//! [reference site](crate::model::aspects::RefSite) whose target is the
//! label computed from the term in its bracket argument, or, where it has
//! none, from the term in its braces ([`term_target`]). The references
//! feature resolves it as it resolves every site; no code of this feature
//! looks a term definition up.

use std::sync::{Arc, LazyLock};

use techy::core::node::{NodeRef, NodeSlice};

use crate::lang::mint::{ArgDef, ArgStdKind, TackOnField, field_name};
use crate::lang::{Flm, FlmCallableType};
use crate::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, ApartFromText, BlockRole, BlockRoleAspect, ContentRef,
    FixedBlockRole, Label, RefDisplayOptions, RefPoint, RefSite, Referenceable, Referencing,
};
use crate::model::element::{ElementSpec, element_value};
use crate::model::names::RefKind;
use crate::stdcore::backends::pres::spelling_identity;
use crate::stdcore::features::refs;

/// The element name of a term definition, `flm.defterm`: the environment
/// `defterm`. Its single element value carries a [`DeftermElement`]
/// ([`defterm_element`]).
pub const DEFTERM: &str = "flm.defterm";

/// The element name of a term link, `flm.term`: the command `\term`. Its
/// single element value carries a [`TermElement`] ([`term_element`]).
pub const TERM: &str = "flm.term";

/// The name of the `defterm` environment's argument that holds the term,
/// `term`.
pub const TERM_ARG: &str = "term";

/// The name of the field that declares an alias of the term being defined,
/// `aliasterm`, written `\aliasterm{…}` after the term, beside the
/// `\label{…}` fields. It is one of the fields of the environment's tack-on
/// argument (the trailing argument that absorbs field macros; see
/// [`defterm_args`]).
pub const ALIAS_FIELD: &str = "aliasterm";

/// The name of the single argument of an [`ALIAS_FIELD`] field, `term`: the
/// alias.
pub const ALIAS_ARG: &str = "term";

/// The name of a term link's optional argument, `ref`: the term it links
/// to, where that term is not the text the link shows.
pub const REF_ARG: &str = "ref";

/// The name of a term link's mandatory argument, `shown`: the text it
/// shows.
pub const SHOWN_ARG: &str = "shown";

/// The argument role of a term that a definition defines,
/// `flm.defterm.defined-term`: the role of the term argument ([`TERM_ARG`])
/// of the `defterm` environment and of the argument of its `\aliasterm`
/// field ([`ALIAS_ARG`]).
pub const DEFINED_TERM_ARG_ROLE: &str = "flm.defterm.defined-term";

/// The argument role of the term that a term link points to,
/// `flm.defterm.target`: the role of the bracket argument ([`REF_ARG`]) of
/// `\term`.
pub const TARGET_ARG_ROLE: &str = "flm.defterm.target";

/// The reference kind of a term definition's reference point, `defterm`.
pub const DEFINITION_KIND: &str = "defterm";

/// The label types an author may write on a term definition with
/// `\label{…}`, `term` and `topic`: `term` is to a term definition what `sec`
/// is to a heading, and `topic` is accepted by a heading too.
pub const LABEL_TYPES: [&str; 2] = ["term", "topic"];

/// The type of the labels computed from a term's identity, `defterm`: kept
/// apart from every type an author writes, so that a computed label can never
/// equal a label an author writes.
// See decision rationale at [§flm-dd-dr:std-defterm-internal-label].
pub(crate) const COMPUTED_TYPE: &str = "defterm";

/// The label computed from the identity `identity`, or `None` for the empty
/// identity, which identifies no term.
fn computed_label(identity: String) -> Option<Label> {
    (!identity.is_empty()).then(|| Label::new(COMPUTED_TYPE, identity))
}

/// Whether a placeholder of a user definition's template is among `nodes`
/// or their descendants: then `nodes` are the content of a template kept
/// rather than expanded, in which no term can be read.
pub(super) fn holds_placeholder<'t>(nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>) -> bool {
    nodes.into_iter().any(|node| {
        std::iter::once(node)
            .chain(node.descendants())
            .any(|inner| inner.callable_type() == Some(FlmCallableType::Placeholder))
    })
}

// --- the definition --------------------------------------------------------------------------

/// The data of the element value of [`DEFTERM`], and the aspects it
/// implements: a block, with one reference point ([`Referenceable`]) and its
/// fields kept apart from its text ([`ApartFromText`]). See the [module
/// documentation](self#the-term-definition).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeftermElement;

/// The labels a term definition computes, in order: its term's, then each
/// alias's, each once; empty for a node that is not a term definition. A term
/// link finds the term definition by one of these ([`term_target`]).
pub(super) fn computed_labels(node: &NodeRef<'_, Flm>) -> Vec<Label> {
    if element_value::<DeftermElement>(node).is_none() {
        return Vec::new();
    }
    let mut labels = Vec::new();
    let term = node.argument_content_nodes_named(TERM_ARG).ok().flatten();
    if let Some(label) = term.and_then(|content| computed_label(declared_identity(content.iter()))) {
        labels.push(label);
    }
    let fields = node.argument_content_nodes_named(refs::FIELDS_ARG).ok().flatten();
    for field in fields.iter().flat_map(|fields| fields.iter()) {
        if field_name(&field).is_none_or(|name| name != ALIAS_FIELD) {
            continue;
        }
        let alias = field.argument_content_nodes_named(ALIAS_ARG).ok().flatten();
        if let Some(label) = alias.and_then(|content| computed_label(declared_identity(content.iter())))
        {
            if !labels.contains(&label) {
                labels.push(label);
            }
        }
    }
    labels
}

/// The identity a term definition declares for the term or the alias written
/// as `nodes`: [`spelling_identity`], except for a term that holds a
/// placeholder of a user definition's template. No document holds such a
/// term, only a template kept rather than expanded does, and it is read as
/// the template's text as written, `#1` included. Such a term definition
/// still declares its labels, so that the user-definitions feature refuses
/// the template that holds it, as it refuses every template that declares a
/// reference point.
fn declared_identity<'t>(nodes: impl IntoIterator<Item = NodeRef<'t, Flm>> + Clone) -> String {
    if !holds_placeholder(nodes.clone()) {
        return spelling_identity(nodes);
    }
    // The span, not `spelling_identity`: the source recomposer re-emits nothing
    // for a placeholder, so `spelling_identity` reads `#1` as the empty text,
    // which would declare no label and let the kept template through.
    let written: String = nodes
        .into_iter()
        .filter(|node| !node.is_comment())
        .map(|node| node.span_content())
        .collect();
    written.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether the term definition `node` defines the term that `target`
/// identifies, under its term or under one of its aliases.
fn defines(node: &NodeRef<'_, Flm>, target: &Label) -> bool {
    computed_labels(node).iter().any(|label| label == target)
}

/// One reference point per term definition: the term's computed label, each
/// alias's, then the labels the author wrote. The computed labels are counted
/// as such ([`RefPoint::computed`]), so that the types [`LABEL_TYPES`] the
/// point accepts apply to the written labels alone. The point's content is
/// the term, and it is anchored on its first label: the label computed from
/// the term, or, where the term is empty (and so computes no label), the
/// first alias's label or the first written label.
// See decision rationale at [§flm-dd-dr:std-defterm-internal-label].
impl Referenceable for DeftermElement {
    fn points(&self, node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        let mut labels = computed_labels(node);
        let computed = labels.len();
        let (written, mut malformed) = refs::point_labels(&refs::label_uses(node));
        labels.extend(written);
        // A malformed text's position counts every label of the point, the
        // computed ones standing first.
        for text in &mut malformed {
            text.position += computed;
        }
        vec![RefPoint {
            kind: RefKind::from(DEFINITION_KIND),
            labels,
            computed,
            malformed,
            anchor: AnchorPolicy::FromFirstLabel,
            number: None,
            content: Some(ContentRef::ArgumentNamed(TERM_ARG.to_string())),
            accepts: AcceptedRefTypes::these(LABEL_TYPES),
        }]
    }
}

/// A term definition's fields, its `\label{…}` and `\aliasterm{…}`, are not
/// part of its text: they label it.
impl ApartFromText for DeftermElement {
    fn apart_from_text(&self, _node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        vec![ContentRef::ArgumentNamed(refs::FIELDS_ARG.to_string())]
    }
}

static DEFTERM_ELEMENT: LazyLock<Arc<ElementSpec>> = LazyLock::new(|| {
    Arc::new(
        ElementSpec::new(DEFTERM)
            .docs(
                "The definition of a term (the `defterm` environment), the target of the term \
                 links to that term.",
            )
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Block)))
            .with::<dyn Referenceable>(Arc::new(DeftermElement))
            .with::<dyn ApartFromText>(Arc::new(DeftermElement))
            .with_value(Arc::new(DeftermElement)),
    )
});

/// The element value of every term definition node: the same value for
/// every node, carrying [`DeftermElement`].
pub fn defterm_element() -> Arc<ElementSpec> {
    Arc::clone(&DEFTERM_ELEMENT)
}

/// The arguments of the `defterm` environment, in order: the term
/// ([`TERM_ARG`]), then the tack-on argument (the trailing argument that
/// absorbs field macros; [`ArgStdKind::TackOn`]) under the references
/// feature's argument name ([`FIELDS_ARG`](refs::FIELDS_ARG)), holding the
/// fields `\label{…}` (the references feature's declaration,
/// [`label_field`](refs::label_field)) and `\aliasterm{…}`
/// ([`ALIAS_FIELD`]), each repeatable.
pub fn defterm_args() -> Vec<ArgDef> {
    let alias = TackOnField::new(
        ALIAS_FIELD,
        vec![ArgDef::named(ALIAS_ARG, ArgStdKind::Mandatory).docs("The alias.").role(DEFINED_TERM_ARG_ROLE)],
    )
    .repeatable()
    .docs("Declares another way of writing the term, by which term links find it too.");
    let fields = ArgStdKind::TackOn { fields: vec![refs::label_field(), alias] };
    vec![
        ArgDef::named(TERM_ARG, ArgStdKind::Mandatory)
            .docs("The term being defined.")
            .role(DEFINED_TERM_ARG_ROLE),
        ArgDef::named(refs::FIELDS_ARG, fields)
            .docs(
                "The `\\label{…}` and `\\aliasterm{…}` fields written after the term: they \
                 label the term definition.",
            ),
    ]
}

// --- the term link ---------------------------------------------------------------------------

/// The data of the element value of [`TERM`], and the aspects it
/// implements: inline content, with one reference site ([`Referencing`]) and
/// its bracket argument kept apart from its text ([`ApartFromText`]). See the
/// [module documentation](self#the-term-link).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermElement;

/// The label under which the term link `node` finds its term definition: the
/// label computed from the term in its bracket argument, or, where the
/// bracket is absent or blank, from the term in its braces.
///
/// `None` for a node that is not a term link; for a term that is empty or
/// only whitespace, which identifies no term; and for a term that holds a
/// placeholder of a user definition's template that was not expanded (only a
/// template kept rather than expanded holds one), whose term cannot be read.
/// No reference site is declared then, and this function reports nothing.
pub fn term_target(node: &NodeRef<'_, Flm>) -> Option<Label> {
    let written = written_term(node)?;
    if holds_placeholder(written.iter()) {
        return None;
    }
    computed_label(spelling_identity(written.iter()))
}

/// The nodes in which the term link `node` writes the term it links to: its
/// bracket argument, or its braces where the bracket is absent or blank.
/// `None` for a node that is not a term link, or one that a parse recovery
/// left without them. [`term_target`] reads these nodes, and the recomposed
/// LaTeX looks in them for a template's placeholder, which leaves the term
/// unknown until a call.
pub(super) fn written_term<'t>(node: &NodeRef<'t, Flm>) -> Option<NodeSlice<'t, Flm>> {
    element_value::<TermElement>(node)?;
    let named = |name: &str| node.argument_content_nodes_named(name).ok().flatten();
    // A placeholder reads as no text through `spelling_identity` (the source
    // recomposer re-emits nothing for it), so a bracket holding one is not
    // blank: it names the term the template leaves open.
    named(REF_ARG)
        .filter(|content| holds_placeholder(content.iter()) || !spelling_identity(content.iter()).is_empty())
        .or_else(|| named(SHOWN_ARG))
}

/// Whether the term link `node`, whose target is `target`, is inside the
/// definition of that same term: whether a term definition enclosing it
/// defines the term that `target` identifies, under its term or under an
/// alias.
///
/// The answer is read from the structure of the tree: the nodes enclosing
/// `node` in its own tree. A term link in content copied out of the document
/// (the term that a reference to the term definition shows) has no enclosing
/// term definition there. Every rule that writes a term link calls this
/// function, so that every output agrees on the answer.
pub fn in_own_definition(node: &NodeRef<'_, Flm>, target: &Label) -> bool {
    let mut current = node.parent();
    while let Some(ancestor) = current {
        if defines(&ancestor, target) {
            return true;
        }
        current = ancestor.parent();
    }
    false
}

/// The target of a term link, and whether the link is inside the definition
/// of that term: the decision every rule that writes a term link takes,
/// computed once per link by [`decide_term`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TermDecision {
    /// The label the link targets ([`term_target`]).
    pub(super) target: Label,
    /// Whether the link is inside the definition of that term
    /// ([`in_own_definition`]).
    pub(super) in_own_definition: bool,
}

/// The decision for the term link `node`: its target and whether it is
/// inside its own term's definition; `None` where the link has no target
/// ([`term_target`]).
pub(super) fn decide_term(node: &NodeRef<'_, Flm>) -> Option<TermDecision> {
    let target = term_target(node)?;
    let in_own_definition = in_own_definition(node, &target);
    Some(TermDecision { target, in_own_definition })
}

/// One reference site, whose target is [`term_target`], shown without any
/// prefix word: what it shows is the text the author wrote. No site where
/// the link has no target.
impl Referencing for TermElement {
    fn sites(&self, node: &NodeRef<'_, Flm>) -> Vec<RefSite> {
        term_target(node)
            .map(|target| RefSite {
                target,
                display: RefDisplayOptions { variant: None, with_prefix: false },
                shown: true,
            })
            .into_iter()
            .collect()
    }
}

/// The term a link gives in its bracket argument, [`REF_ARG`], is not part
/// of the text around the link; the text it shows is, and stays part of it.
impl ApartFromText for TermElement {
    fn apart_from_text(&self, _node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        vec![ContentRef::ArgumentNamed(REF_ARG.to_string())]
    }
}

static TERM_ELEMENT: LazyLock<Arc<ElementSpec>> = LazyLock::new(|| {
    Arc::new(
        ElementSpec::new(TERM)
            .docs("A link to the definition of a term, showing the text written for it.")
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT)))
            .with::<dyn Referencing>(Arc::new(TermElement))
            .with::<dyn ApartFromText>(Arc::new(TermElement))
            .with_value(Arc::new(TermElement)),
    )
});

/// The element value of every term link node: the same value for every
/// node, carrying [`TermElement`].
pub fn term_element() -> Arc<ElementSpec> {
    Arc::clone(&TERM_ELEMENT)
}

/// The arguments of `\term`, in order: the optional term linked to
/// ([`REF_ARG`]), then the text shown ([`SHOWN_ARG`]).
pub fn term_args() -> Vec<ArgDef> {
    vec![
        ArgDef::named(REF_ARG, ArgStdKind::Optional)
            .docs("The term linked to, where it is not the text shown.")
            .role(TARGET_ARG_ROLE),
        ArgDef::named(SHOWN_ARG, ArgStdKind::Mandatory).docs("The text shown."),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_elements_carry_their_aspects() {
        assert!(defterm_element().has_aspect::<dyn Referenceable>());
        assert!(defterm_element().has_aspect::<dyn ApartFromText>());
        assert!(term_element().has_aspect::<dyn Referencing>());
        assert!(term_element().has_aspect::<dyn ApartFromText>());
        assert!(!ElementSpec::same_value(&defterm_element(), &term_element()));
    }

    #[test]
    fn the_empty_identity_names_no_term() {
        assert_eq!(computed_label(String::new()), None);
        assert_eq!(
            computed_label("entropy".to_string()),
            Some(Label::new(COMPUTED_TYPE, "entropy"))
        );
    }
}
