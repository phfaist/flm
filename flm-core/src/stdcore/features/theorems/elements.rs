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
//! A title on such a kind that begins with a star, comments and whitespace
//! aside, but is not one run of characters (`[*\emph{x}]`, `[~*lem:key]`), or
//! whose text after the stars is not of the form `type:label`, is malformed ([`TitleReading::Malformed`]): it declares
//! no reference site, the feature's check reports it
//! ([`check`](super::check)), and the theorem's rules show the title as
//! written.
//!
//! # A starred title is kept as typed
//!
//! The references feature compares a back-reference's label with the labels
//! of `\label` as typed. On a kind whose class reads back-references, the
//! title's reader ([`theorem_args`]) therefore places a title that begins with
//! a star, comments and whitespace aside, under an opaque wrapper (see [opaque
//! regions](crate::guide::concepts_overview#opaque-regions)): the
//! block-structure pass carries it over as written, so `[*lem:a--b]` refers to
//! the statement labeled `lem:a--b`, and a malformed back-reference is shown as
//! typed. Every other title is content, laid out and shown as any optional
//! argument is, with the typographic substitutions applied.
//!
//! The reader sees the star only where the theorem's own source writes it. A
//! star written in a user definition's template (`[*#1]`) is seen, and the
//! title is kept as typed whatever the use site writes after it. A star that
//! arrives through a placeholder (a template `[#1]` used as `\pf{*lem:a--b}`)
//! is not seen: that title is laid out as content, and a label holding `--`,
//! quotes, an apostrophe or `...` does not resolve through it.

use std::sync::Arc;

use techy::core::constructs::{ArgumentParser, ConstructParserResult, ParseContext, ParsedArgumentNodes};
use techy::core::node::{BuildId, ContentNodes, NodeKind, NodeRef, StagedNodeView, StagedNodes};
use techy::core::specs::ArgumentSpec;

use crate::lang::mint::opaque::wrap_argument;
use crate::lang::mint::{
    ArgDef, ArgStdKind, DefinitionShape, FlmArgumentParser, IntoFlmArgumentParser, TackOnField,
    field_name,
};
use crate::lang::text::{CharsSegment, is_blank, is_blank_text, is_space, read_chars};
use crate::lang::{Flm, FlmCallableType, FlmGroupType};
use crate::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, ApartFromText, BlockRole, BlockRoleAspect, ContentRef,
    FixedBlockRole, Label, NumberSource, PointId, RefDisplayOptions, RefPoint, RefSite,
    Referenceable, Referencing, Whitespace,
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
/// This is the only function that says what a title is: the theorem's
/// reference site, its rules, the feature's check and every later reader of
/// the relation call it, so they all agree. The title's reader that
/// [`theorem_args`] gives a class reading back-references decides, while it
/// parses the title, whether to keep the title as typed; the tree does not
/// exist yet, so that reader holds a copy of this function's test for a star,
/// run over the nodes it stages. A change to the test here is made there
/// too, so that a title is kept as typed exactly where this function reads it
/// as starred.
pub fn title_reading(node: &NodeRef<'_, Flm>) -> Option<TitleReading> {
    let value = element_value::<TheoremElement>(node)?;
    let Ok(Some(title)) = node.argument_content_nodes_named(TITLE_ARG) else {
        return Some(TitleReading::Absent);
    };
    if !value.class.relation_ref {
        return Some(TitleReading::Title);
    }
    // The title's reader makes the same test for a star over the nodes it
    // stages (`begins_with_star`); the two tests change together.
    let Ok(read) = read_chars(title.iter()) else {
        // Not one run of characters: a back-reference only where the title's
        // first node, comments, blank characters and whitespace nodes (`~`)
        // aside, is characters beginning with a star; a title opening with a
        // command, a group or a formula is a title.
        let starred = title
            .iter()
            .find(|node| !(node.is_comment() || is_blank(node)))
            .and_then(|node| node.chars())
            .is_some_and(|text| text.trim_start_matches(is_space).starts_with('*'));
        return Some(if starred { TitleReading::Malformed } else { TitleReading::Title });
    };
    // The characters before the first placeholder, if any stands.
    let leading: String = read
        .segments()
        .iter()
        .map_while(|segment| segment.as_text())
        .collect();
    let leading = leading.trim_start_matches(is_space);
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
    let text = rest.trim_matches(is_space).to_string();
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

/// The arguments of the theorem environments of a class, in order: the
/// optional title ([`TITLE_ARG`]), then the tack-on argument (the trailing
/// argument that absorbs field macros; [`ArgStdKind::TackOn`]) under the
/// references feature's argument name ([`FIELDS_ARG`](refs::FIELDS_ARG)),
/// holding the fields `\label{…}` (the references feature's declaration,
/// [`label_field`](refs::label_field)), repeatable, and `\noproofref`
/// ([`NOPROOFREF_FIELD`]).
///
/// `relation_ref` is the class's
/// [`relation_ref`](super::TheoremClass::relation_ref). Where it is false, the
/// title is an optional argument of the standard kind
/// ([`ArgStdKind::Optional`]). Where it is true, the title is read by the
/// feature's own reader, which reads the same optional `[…]` group and places
/// a title that begins with a star under an opaque wrapper, so that a
/// back-reference is read as typed (see the [module
/// documentation](self#a-starred-title-is-kept-as-typed)).
pub fn theorem_args(relation_ref: bool) -> Vec<ArgDef> {
    let noproofref = TackOnField::new(NOPROOFREF_FIELD, Vec::new())
        .docs("Says that no proof of the statement is coming.");
    let fields = ArgStdKind::TackOn { fields: vec![refs::label_field(), noproofref] };
    let title = if relation_ref {
        ArgDef::named(TITLE_ARG, RelationTitleParser::new())
    } else {
        ArgDef::named(TITLE_ARG, ArgStdKind::Optional)
    };
    vec![
        title
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

/// The identity of the title reader of a class that reads back-references
/// ([`RelationTitleParser`]) in the registry digest,
/// `flm.theorems.relation-title`.
const RELATION_TITLE_IDENTITY: &str = "flm.theorems.relation-title";

/// The reader of the title of a theorem whose class reads back-references
/// ([`relation_ref`](super::TheoremClass::relation_ref)): an optional `[…]`
/// group, read by the reader of an optional argument of the standard kind
/// ([`ArgStdKind::Optional`]), and placed under an opaque wrapper when it
/// begins with a star ([`begins_with_star`]). See the [module
/// documentation](self#a-starred-title-is-kept-as-typed).
///
/// The content designation names the same nodes with or without the
/// wrapper, so [`title_reading`] reads the title the same way in both cases.
/// An argument that is not written is absent.
///
/// What it states through [`FlmArgumentParser`]: its identity is
/// [`RELATION_TITLE_IDENTITY`]; it is written as a value in a recomposed user
/// definition, as an optional argument is; its description is "an optional
/// `[…]` group, kept as written when it begins with `*`".
#[derive(Debug)]
struct RelationTitleParser {
    /// The parser of an optional argument of the standard kind.
    optional: Arc<dyn FlmArgumentParser>,
}

impl RelationTitleParser {
    /// Creates the reader.
    fn new() -> RelationTitleParser {
        RelationTitleParser { optional: ArgStdKind::Optional.into_flm_argument_parser() }
    }
}

impl ArgumentParser<Flm> for RelationTitleParser {
    fn parse_argument(
        &self,
        cx: &mut ParseContext<'_, '_, Flm>,
        spec: &ArgumentSpec<Flm>,
    ) -> ConstructParserResult<Flm, Option<ParsedArgumentNodes<Flm>>> {
        let Some(read) = self.optional.parse_argument(cx, spec)? else {
            return Ok(None);
        };
        let staged = cx.staged_nodes();
        let content = match &read.content {
            ContentNodes::InRegion(range) => read.nodes.get(range.start as usize..range.end as usize),
            ContentNodes::InChildrenOf(parent, range) => staged
                .get(*parent)
                .and_then(|view| view.children().get(range.start as usize..range.end as usize)),
        };
        if !begins_with_star(staged, content.unwrap_or_default()) {
            return Ok(Some(read));
        }
        wrap_argument(cx, read).map(Some)
    }

    fn can_match_empty(&self) -> bool {
        self.optional.can_match_empty()
    }
}

impl FlmArgumentParser for RelationTitleParser {
    fn identity(&self) -> Vec<u8> {
        RELATION_TITLE_IDENTITY.as_bytes().to_vec()
    }

    fn definition_shape(&self) -> DefinitionShape {
        DefinitionShape::Value
    }

    fn description(&self) -> String {
        String::from("an optional `[…]` group, kept as written when it begins with `*`")
    }
}

/// Whether the staged nodes `content`, the content of a title just read,
/// begin with a star as [`title_reading`] reads a title: where they are one
/// run of characters (characters, comments, placeholders and groups that are
/// not formulas), whether their characters before the first placeholder,
/// whitespace aside, begin with a star; otherwise, whether their first node,
/// comments, blank characters and whitespace nodes (`~`) aside, is
/// characters beginning with a star.
/// So the wrapper stands over exactly the titles that [`title_reading`] reads
/// as a back-reference, a malformed one, or a placeholder after a star.
///
/// This is a copy of [`title_reading`]'s test for a star, and
/// [`leading_characters`] walks the nodes as [`read_chars`] does: the reader
/// runs before the tree exists, so it has no [`NodeRef`] to give
/// [`title_reading`]. A change to either test is made to both.
fn begins_with_star(staged: StagedNodes<'_, Flm>, content: &[BuildId]) -> bool {
    match leading_characters(staged, content) {
        Some(leading) => leading.trim_start_matches(is_space).starts_with('*'),
        None => content
            .iter()
            .filter_map(|id| staged.get(*id))
            .filter(|view| {
                !matches!(view.kind(), NodeKind::Comment(_))
                    && !view.ext().element.has_aspect::<dyn Whitespace>()
            })
            .find(|view| staged_chars(view).is_none_or(|text| !is_blank_text(text)))
            .and_then(|view| staged_chars(&view))
            .is_some_and(|text| text.trim_start_matches(is_space).starts_with('*')),
    }
}

/// The characters of the staged nodes `content` before their first
/// placeholder, in document order, where the nodes are one run of
/// characters as [`read_chars`] reads one; `None` where they hold a command
/// other than a placeholder, or a formula.
fn leading_characters(staged: StagedNodes<'_, Flm>, content: &[BuildId]) -> Option<String> {
    let mut leading = String::new();
    let mut at_placeholder = false;
    // Document order without recursion, as in `read_chars`.
    let mut pending: Vec<BuildId> = content.iter().rev().copied().collect();
    while let Some(id) = pending.pop() {
        let view = staged.get(id)?;
        match view.kind() {
            NodeKind::Chars { .. } => {
                if !at_placeholder {
                    leading.push_str(staged_chars(&view).unwrap_or_default());
                }
            }
            NodeKind::Comment(_) => {}
            NodeKind::Group(group) if matches!(group.group_type, Some(FlmGroupType::Math(_))) => {
                return None;
            }
            NodeKind::Group(_) | NodeKind::List => pending.extend(view.children().iter().rev()),
            NodeKind::Callable(callable) => {
                if callable.callable_type != FlmCallableType::Placeholder {
                    return None;
                }
                at_placeholder = true;
            }
        }
    }
    Some(leading)
}

/// The characters of the staged node `view`, where it is a node of
/// characters.
fn staged_chars<'b>(view: &StagedNodeView<'b, Flm>) -> Option<&'b str> {
    match view.kind() {
        NodeKind::Chars { content } => Some(content.resolve(view.span().source())),
        _ => None,
    }
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
