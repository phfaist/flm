//! The element of the [`theorems`](super) feature, [`THEOREM`]
//! (`flm.theorems.theorem`), with one element value per configured theorem
//! kind: its arguments and fields, the reference point every theorem declares,
//! and the back-reference a proof's title declares ([`title_reading`]).
//!
//! # What an author writes
//!
//! ```text
//! \begin{lemma}[Key lemma]\label{lem:key}
//!   Every bounded monotone sequence converges.
//! \end{lemma}
//! \begin{proof}[*lem:key]
//!   …
//! \end{proof}
//! ```
//!
//! Every theorem kind is an environment of its own name. It takes an optional
//! **title**, ordinary inline content, then two fields in any order: the
//! references feature's `\label{…}`, repeatable, and the feature's own
//! `\noproofref`, which takes no argument and says that no proof of the
//! statement is coming. A field is a macro that a construct reads as data
//! rather than as content (see [fields: data a construct
//! reads](crate::guide::concepts_overview#fields-data-a-construct-reads)).
//! The environment's body is laid out as blocks.
//!
//! # The reference point
//!
//! Every theorem is one [reference point](crate::model::aspects::RefPoint) of
//! the reference kind [`POINT_KIND`], `theorem`, including a proof.
//!
//! - A theorem of a **numbered** kind carries the labels of its `\label`
//!   fields and accepts the configuration's
//!   [`label_types`](super::TheoremsConfig::label_types). It counts in its
//!   class's counter and is shown in its kind's counter style
//!   ([`numbers`](super::numbers)). Its anchor is the environment's name and
//!   its number, `lemma-4` (`lemma-2-3` in a document that numbers theorems
//!   within sections); where the document did not number it, its anchor is
//!   formed from its first label ([`AnchorPolicy::FromValue`]).
//! - A theorem of an **unnumbered** kind, such as a proof, is numbered by
//!   nothing and accepts no label. A `\label` on a proof is reported by the
//!   references feature as `flm.refs.label-type-not-accepted` (with the
//!   condition's field `accepted` empty and its field `kind` equal to
//!   `theorem`), and the proof has no anchor.
//!
//! # The back-reference
//!
//! On a kind whose class reads back-references
//! ([`relation_ref`](super::TheoremClass::relation_ref)), a title that is one
//! run of plain characters beginning with a star is a **back-reference** to
//! the statement the theorem proves: `[*lem:key]` refers to the statement
//! labeled `lem:key` and shows its word and number in the heading, and
//! `[**lem:key]` refers to the same statement and shows nothing in the
//! heading. The theorem then
//! declares one [reference site](crate::model::aspects::RefSite), which the
//! references feature resolves as it resolves `\ref`. Every other title is a
//! title, whether it begins with a star or not ([`title_reading`]).
//!
//! A title on such a kind that begins with a star but is not one run of
//! characters (`[*\emph{x}]`), or whose text after the stars is not of the
//! form `type:label`, is malformed ([`TitleReading::Malformed`]): it declares
//! no reference site, the feature's check reports it
//! ([`check`](super::check)), and the theorem's rules show the title as
//! written.

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::lang::mint::{ArgDef, ArgStdKind, TackOnField, field_name};
use crate::lang::text::{CharsSegment, read_chars};
use crate::lang::Flm;
use crate::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, ApartFromText, BlockRole, BlockRoleAspect, ContentRef,
    FixedBlockRole, Label, NumberSource, PointId, RefDisplayOptions, RefPoint, RefSite,
    Referenceable, Referencing,
};
use crate::model::addr::NodeAddr;
use crate::model::element::{ElementSpec, element_of, element_value};
use crate::model::names::{Name, RefKind};
use crate::stdcore::features::refs;

use super::config::{TheoremClass, TheoremKind, TheoremsConfig};
use super::numbers::{counter_name, style_name};

/// The element name of every theorem, `flm.theorems.theorem`. The element
/// has one element value per configured theorem kind, each carrying a
/// [`TheoremElement`] as its data.
pub const THEOREM: &str = "flm.theorems.theorem";

/// The name of a theorem's optional title argument, `title`.
pub const TITLE_ARG: &str = "title";

/// The name of the field that marks a statement as having no proof,
/// `noproofref`, written `\noproofref` after the title. It is one of the
/// fields of the theorem's tack-on argument (the trailing argument that
/// absorbs field macros; see [`theorem_args`]).
pub const NOPROOFREF_FIELD: &str = "noproofref";

/// The reference kind of every theorem's reference point, `theorem`, for
/// every theorem kind alike.
pub const POINT_KIND: &str = "theorem";

/// The data of one element value of [`THEOREM`]: the theorem kind an
/// environment is, its class, and what its reference point needs. The
/// feature creates one per configured kind ([`TheoremElement::of`]).
///
/// The element value carries this data and the aspects it implements: one
/// reference point ([`Referenceable`]), one reference site for a
/// back-reference ([`Referencing`]), and its fields kept apart from its text
/// ([`ApartFromText`]). A rule or a stage reads it from a node with
/// [`element_value`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheoremElement {
    /// The kind's name, which is also the environment's name.
    pub name: Name,
    /// The kind as configured.
    pub kind: TheoremKind,
    /// The name of the kind's class.
    pub class_name: Name,
    /// The kind's class as configured.
    pub class: TheoremClass,
    /// The counter a theorem of this kind counts in
    /// ([`counter_name`]), or `None` for an
    /// unnumbered kind.
    pub counter: Option<Name>,
    /// The label types a theorem of this kind accepts: the configuration's
    /// [`label_types`](TheoremsConfig::label_types) for a numbered kind, none
    /// for an unnumbered kind.
    pub label_types: Vec<String>,
}

impl TheoremElement {
    /// The data of the kind `name` of `config`; `None` where the kind or its
    /// class is not declared.
    pub fn of(config: &TheoremsConfig, name: &Name) -> Option<TheoremElement> {
        let (kind, class) = config.kind_and_class(name.as_str())?;
        let counter = counter_name(config, name.as_str());
        let label_types = if counter.is_some() { config.label_types.clone() } else { Vec::new() };
        Some(TheoremElement {
            name: name.clone(),
            kind: kind.clone(),
            class_name: kind.class.clone(),
            class: class.clone(),
            counter,
            label_types,
        })
    }
}

/// What a theorem's title argument is, as [`title_reading`] reads it: no
/// title, a title, a back-reference, a malformed back-reference, or not yet
/// known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TitleReading {
    /// No title argument is written.
    Absent,
    /// A title, shown as the theorem's own content.
    Title,
    /// A back-reference to the statement labeled `label`: shown in the
    /// heading where `shown` (one star), recorded only where not (two stars).
    Relation {
        /// The text after the stars, trimmed.
        text: String,
        /// The label that `text` is the written form of.
        label: Label,
        /// Whether the heading shows the reference: one star.
        shown: bool,
    },
    /// A title beginning with a star, on a kind that reads back-references,
    /// that is not one run of characters or whose text after the stars is
    /// not of the form `type:label`. It declares no reference site; the
    /// feature's check reports it ([`BadRelationRef`](super::BadRelationRef)),
    /// and the theorem's rules show the title as written.
    Malformed,
    /// A title in the template of a kept definition (a user definition whose
    /// use sites a plan leaves in the tree; see [kept
    /// definitions](crate::stdcore::features::userdefs::kept)), with a
    /// placeholder where a back-reference's first characters would be:
    /// whether it is a back-reference is not known until the
    /// template is called. It declares no reference site, and the theorem's
    /// rules return
    /// [`LowerError::PlaceholderCharacters`](crate::core::lowering::LowerError::PlaceholderCharacters),
    /// so that each use site of the definition is expanded instead.
    Unknown,
}

/// What the title of the theorem `node` is (see the [module
/// documentation](self#the-back-reference)); `None` for a node that is not a
/// theorem. On a kind whose class does not read back-references, every
/// written title is a [`Title`](TitleReading::Title).
///
/// This is the only function that reads a title: the theorem's reference
/// site, its rules, the feature's check and every later reader of the
/// relation call it, so they all agree.
pub fn title_reading(node: &NodeRef<'_, Flm>) -> Option<TitleReading> {
    let value = element_value::<TheoremElement>(node)?;
    let Ok(Some(title)) = node.argument_content_nodes_named(TITLE_ARG) else {
        return Some(TitleReading::Absent);
    };
    if !value.class.relation_ref {
        return Some(TitleReading::Title);
    }
    let Ok(read) = read_chars(title.iter()) else {
        // Not one run of characters: a back-reference only where the title's
        // first node, comments and blank characters aside, is characters
        // beginning with a star; a title opening with a command, a group or
        // a formula is a title.
        let starred = title
            .iter()
            .filter(|node| !node.is_comment())
            .find(|node| node.chars().is_none_or(|text| !text.trim().is_empty()))
            .and_then(|node| node.chars())
            .is_some_and(|text| text.trim_start().starts_with('*'));
        return Some(if starred { TitleReading::Malformed } else { TitleReading::Title });
    };
    // The characters before the first placeholder, if any stands.
    let leading: String = read
        .segments()
        .iter()
        .map_while(|segment| segment.as_text())
        .collect();
    let leading = leading.trim_start();
    let placeholder = read.segments().iter().any(|segment| matches!(segment, CharsSegment::Placeholder(_)));
    if placeholder && (leading.is_empty() || leading.starts_with('*')) {
        return Some(TitleReading::Unknown);
    }
    let Some(starred) = leading.strip_prefix('*') else {
        return Some(TitleReading::Title);
    };
    let (rest, shown) = match starred.strip_prefix('*') {
        Some(rest) => (rest, false),
        None => (starred, true),
    };
    let text = rest.trim().to_string();
    Some(match Label::parse(&text) {
        Some(label) => TitleReading::Relation { text, label, shown },
        None => TitleReading::Malformed,
    })
}

/// The back-reference site the theorem `node` declares, read through its
/// element's [`Referencing`] aspect: the site the references feature
/// resolves, whose [`shown`](RefSite::shown) and
/// [`display`](RefSite::display) say how the heading shows it. `None` where
/// the title declares no site.
pub(super) fn relation_site(node: &NodeRef<'_, Flm>) -> Option<RefSite> {
    element_of(node).aspect::<dyn Referencing>()?.sites(node).into_iter().next()
}

/// The reference point the theorem at `addr` declares: its first and only
/// point ([`Referenceable`] below declares one).
pub(super) fn theorem_point(addr: NodeAddr) -> PointId {
    PointId { addr, index: 0 }
}

/// The back-reference site the theorem at `addr` declares, where its title
/// declares one: its first and only site ([`Referencing`] below declares at
/// most one).
pub(super) fn relation_site_id(addr: NodeAddr) -> refs::SiteId {
    refs::SiteId { addr, index: 0 }
}

/// Whether the theorem `node` carries the field `\noproofref`.
pub fn has_noproofref(node: &NodeRef<'_, Flm>) -> bool {
    let Ok(Some(fields)) = node.argument_content_nodes_named(refs::FIELDS_ARG) else {
        return false;
    };
    fields.iter().any(|field| field_name(&field).is_some_and(|name| name.as_str() == NOPROOFREF_FIELD))
}

/// One reference point per theorem, carrying the theorem's labels. For a
/// numbered kind, the point is numbered in its class's counter, shown in the
/// kind's counter style and anchored on its number. For an unnumbered kind,
/// the point is numbered by nothing and accepts no label.
impl Referenceable for TheoremElement {
    fn points(&self, node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        let (labels, malformed) = refs::point_labels(&refs::label_uses(node));
        // See decision rationale at [§flm-dd-dr:std-theorem-points].
        let (anchor, number) = match &self.counter {
            Some(counter) => (
                AnchorPolicy::FromValue { prefix: self.name.to_string() },
                Some(NumberSource {
                    counter: counter.clone(),
                    instances: Vec::new(),
                    style: style_name(self.name.as_str()),
                    advance: vec![0],
                    number_format: None,
                }),
            ),
            None => (AnchorPolicy::FromFirstLabel, None),
        };
        vec![RefPoint {
            kind: RefKind::from(POINT_KIND),
            labels,
            computed: 0,
            malformed,
            anchor,
            number,
            content: None,
            accepts: AcceptedRefTypes::these(self.label_types.iter().map(String::as_str)),
        }]
    }
}

/// One reference site for a back-reference ([`TitleReading::Relation`]),
/// whose target is the back-reference's label, shown with the reference's
/// default display (the target's word and number), and shown in the heading
/// or not as the stars say; no site for any other title.
impl Referencing for TheoremElement {
    fn sites(&self, node: &NodeRef<'_, Flm>) -> Vec<RefSite> {
        match title_reading(node) {
            Some(TitleReading::Relation { label, shown, .. }) => vec![RefSite {
                target: label,
                display: RefDisplayOptions { variant: None, with_prefix: true },
                shown,
            }],
            _ => Vec::new(),
        }
    }
}

/// A theorem's fields, its `\label{…}` and `\noproofref`, are not part of its
/// text: they label the theorem and say something about it.
impl ApartFromText for TheoremElement {
    fn apart_from_text(&self, _node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        vec![ContentRef::ArgumentNamed(refs::FIELDS_ARG.to_string())]
    }
}

/// The element value of the kind `value`: a block with one reference point,
/// the back-reference's site, and its fields kept apart from its text.
pub(super) fn theorem_element(value: TheoremElement) -> Arc<ElementSpec> {
    let docs = format!("`{}`: a theorem-like environment of the theorem class `{}`.", value.name, value.class_name);
    let value = Arc::new(value);
    Arc::new(
        ElementSpec::new(THEOREM)
            .docs(docs)
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Block)))
            .with::<dyn Referenceable>(Arc::clone(&value) as Arc<dyn Referenceable>)
            .with::<dyn Referencing>(Arc::clone(&value) as Arc<dyn Referencing>)
            .with::<dyn ApartFromText>(Arc::clone(&value) as Arc<dyn ApartFromText>)
            .with_value(value),
    )
}

/// The arguments of every theorem environment, in order: the optional title
/// ([`TITLE_ARG`]), then the tack-on argument (the trailing argument that
/// absorbs field macros; [`ArgStdKind::TackOn`]) under the references
/// feature's argument name ([`FIELDS_ARG`](refs::FIELDS_ARG)), holding the
/// fields `\label{…}` (the references feature's declaration,
/// [`label_field`](refs::label_field)), repeatable, and `\noproofref`
/// ([`NOPROOFREF_FIELD`]).
pub fn theorem_args() -> Vec<ArgDef> {
    let noproofref = TackOnField::new(NOPROOFREF_FIELD, Vec::new())
        .docs("Says that no proof of the statement is coming.");
    let fields = ArgStdKind::TackOn { fields: vec![refs::label_field(), noproofref] };
    vec![
        ArgDef::named(TITLE_ARG, ArgStdKind::Optional)
            .docs(
                "The theorem's title, or, on a proof, a back-reference `*type:label` to the \
                 statement it proves.",
            ),
        ArgDef::named(refs::FIELDS_ARG, fields)
            .docs(
                "The `\\label{…}` and `\\noproofref` fields written after the title: they label \
                 the theorem and mark it as having no proof.",
            ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kind_of_a_numbered_class_counts_and_accepts_the_label_types() {
        let config = TheoremsConfig::new();
        let lemma = TheoremElement::of(&config, &Name::from("lemma")).expect("declared");
        assert_eq!(lemma.counter.as_deref(), Some("flm.theorems.theorem"));
        assert_eq!(lemma.label_types, config.label_types);
        assert_eq!(lemma.class_name.as_str(), "theoremlike");
        let proof = TheoremElement::of(&config, &Name::from("proof")).expect("declared");
        assert_eq!(proof.counter, None);
        assert!(proof.label_types.is_empty());
        assert!(TheoremElement::of(&config, &Name::from("nosuch")).is_none());
    }
}
