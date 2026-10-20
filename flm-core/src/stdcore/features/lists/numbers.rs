//! The numbers of list items: the counter and the counter styles that each
//! numbered list kind declares to the
//! [`numbering`](crate::stdcore::features::numbering) feature.
//!
//! A counter ([`CounterDecl`]) is a dotted name under which things are
//! counted in document order, and a counter style ([`CounterStyleDecl`])
//! says how a counter value is shown.
//!
//! # What a numbered list kind declares
//!
//! Each numbered list kind of the feature's configuration
//! ([`ListsConfig`]) declares one counter,
//! `flm.lists.<kind>` ([`counter_name`]), for example `flm.lists.enumerate`
//! for the shipped `enumerate`, and one counter style per nesting depth for
//! which it declares a template, `flm.lists.style.<kind>.<depth>`
//! ([`style_name`]), the first for the outermost lists. A list nested deeper
//! than the list kind has templates for uses the last counter style. An
//! unnumbered list kind declares nothing.
//!
//! Every list environment of a numbered list kind is one *scoped instance*
//! of its counter (see
//! [scoped instances](crate::model::aspects::NumberSource#scoped-instances)
//! and the [numbering guide](crate::guide::numbering#scoped-instances)): a
//! separate
//! sequence of values, so that each list numbers its items from one. A list
//! nested in an item of another list of the same list kind is an instance
//! numbered within the enclosing list, and the list kind's nesting joiner
//! ([`ListKind::nesting`](super::ListKind::nesting)) says what of the
//! enclosing item's number its items write before their own. The counter
//! itself is numbered within no other counter, so the outermost lists of a
//! document each start at one; a document's numbering options
//! ([`NumberingOptions`](crate::stdcore::features::numbering::NumberingOptions))
//! may number them within a section.
//!
//! A counter style writes the item's number through the depth's reference
//! template ([`ListKind::ref_labels`](super::ListKind::ref_labels)), with the
//! word `Item~` before one number and `Items~` before several, and no
//! delimiters: this is what a reference to an item shows, `Item~2`,
//! `Item~(ii)`. Several numbers are joined compactly (`Items~1,3`,
//! `Items~1–3`). The list's lowering rule writes the label beside an item
//! from the same counter value, through the depth's label template instead.
//!
//! # How the declarations reach the numbering feature
//!
//! The counters and counter styles are contributed to the numbering
//! feature's service slots ([`COUNTERS_SLOT`], [`STYLES_SLOT`]) as peer
//! contributions: contributions to a service slot another feature owns,
//! which the registry build drops with one Note when that feature is not
//! registered (see [services](crate::core::services)). The lists feature
//! therefore registers in a registry that holds no numbering feature; a
//! numbered list then shows no numbers and is reported
//! ([`NoNumbering`](super::NoNumbering)).

use std::sync::Arc;

use crate::core::registry::{BuildError, FeatureBuilder};
use crate::model::names::Name;
use crate::stdcore::features::numbering::{
    CounterDecl, CounterStyleDecl, JoinDecl, NumberTemplate, PluralWords, PrefixWords,
    COUNTERS_SLOT, STYLES_SLOT,
};
use crate::stdcore::features::refs::CAPITAL_VARIANT;

use super::config::{ListKind, ListsConfig};

/// The prefix of a numbered list kind's counter name, `flm.lists.`: the
/// items of the list kind `enumerate` are counted in `flm.lists.enumerate`.
pub const COUNTER_PREFIX: &str = "flm.lists.";

/// The prefix of a numbered list kind's counter style names,
/// `flm.lists.style.`: the outermost lists of the list kind `enumerate` are
/// shown in `flm.lists.style.enumerate.1`.
pub const STYLE_PREFIX: &str = "flm.lists.style.";

/// The words a reference writes before one item's number, `Item~`, and
/// before several, `Items~`; the `~` is FLM's non-breaking space.
pub const WORDS: (&str, &str) = ("Item~", "Items~");

/// The name of the counter in which the items of the numbered list kind
/// `kind` are counted: `flm.lists.<kind>`.
pub fn counter_name(kind: &str) -> Name {
    Name::from(format!("{COUNTER_PREFIX}{kind}").as_str())
}

/// The name of the counter style in which the items of a list of the
/// numbered list kind `kind`, at nesting depth `depth`, are recorded, where
/// `kind` declares templates for `count` depths ([`ListKind::depth_count`]):
/// `flm.lists.style.<kind>.<n>`, where `n` is `depth`, or `count` for a list
/// nested deeper than that. Depth 0 is read as depth 1.
pub fn style_name(kind: &str, depth: usize, count: usize) -> Name {
    let n = depth.clamp(1, count.max(1));
    Name::from(format!("{STYLE_PREFIX}{kind}.{n}").as_str())
}

/// The counter style of depth `depth` (from 1) of the numbered list kind
/// `kind`, whose number format is `template`, the depth's reference
/// template.
fn style_of(kind: &str, depth: usize, count: usize, template: &str) -> CounterStyleDecl {
    // The words, the variant and the join texts follow the other standard
    // styles. See decision rationale at [§flm-dd-dr:std-list-counters].
    let mut style = CounterStyleDecl::new(style_name(kind, depth, count));
    style.number_format = NumberTemplate::formats(template);
    let (one, other) = WORDS;
    style.prefix_words =
        PrefixWords::new(one, other).with_variant(CAPITAL_VARIANT, PluralWords::new(one, other));
    style.join = JoinDecl::compact();
    style
}

/// The counter of the numbered list kind `kind`: shown by default in the
/// counter style of its outermost lists, numbered within no other counter,
/// with the list kind's nesting joiner.
fn counter_of(name: &str, kind: &ListKind) -> CounterDecl {
    CounterDecl::new(counter_name(name), style_name(name, 1, kind.depth_count()))
        .docs(format!("The items of the `{name}` lists of the document, one run per list."))
        .nesting(kind.nesting.clone())
}

/// Contributes, for every numbered list kind of `config`, its counter and
/// one counter style per depth for which it declares a template (each
/// written through the depth's reference template) to the numbering
/// feature's service slots, as peer contributions. An unnumbered list kind
/// contributes nothing.
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a declaration is made in
/// a registration round that does not accept it (see
/// [`FeatureBuilder`](crate::core::registry::FeatureBuilder)).
pub(super) fn declare(b: &mut FeatureBuilder<'_>, config: &ListsConfig) -> Result<(), BuildError> {
    for (name, kind) in &config.environments {
        if !kind.numbered {
            continue;
        }
        let name = name.as_str();
        let count = kind.depth_count();
        for depth in 1..=count {
            // A numbered kind declares a reference template at least, which
            // the configuration's check made sure of when the feature
            // registered.
            let template = kind.ref_label_at(depth).unwrap_or_default();
            b.peer_slot::<CounterStyleDecl>(
                STYLES_SLOT,
                Arc::new(style_of(name, depth, count, template)),
            )?;
        }
        b.peer_slot::<CounterDecl>(COUNTERS_SLOT, Arc::new(counter_of(name, kind)))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_numbered_kind_counts_in_a_counter_of_its_own_name() {
        assert_eq!(counter_name("enumerate").as_str(), "flm.lists.enumerate");
        let kind = ListKind::numbered(["#{arabic}.", "(#{roman})"]);
        let counter = counter_of("enumerate", &kind);
        assert_eq!(counter.name.as_str(), "flm.lists.enumerate");
        assert_eq!(counter.default_style.as_str(), "flm.lists.style.enumerate.1");
        assert!(counter.within.is_none());
    }

    #[test]
    fn a_list_deeper_than_the_templates_uses_the_last_style() {
        assert_eq!(style_name("enumerate", 1, 3).as_str(), "flm.lists.style.enumerate.1");
        assert_eq!(style_name("enumerate", 3, 3).as_str(), "flm.lists.style.enumerate.3");
        assert_eq!(style_name("enumerate", 6, 3).as_str(), "flm.lists.style.enumerate.3");
        assert_eq!(style_name("enumerate", 0, 3).as_str(), "flm.lists.style.enumerate.1");
    }

    #[test]
    fn a_style_writes_the_word_item_and_no_delimiters() {
        let style = style_of("enumerate", 2, 3, "(#{roman})");
        assert_eq!(style.number_format, NumberTemplate::formats("(#{roman})"));
        assert_eq!(style.prefix_words.base.one, "Item~");
        assert_eq!(style.prefix_words.base.other, "Items~");
        assert_eq!(style.delimiters.open, "");
        assert_eq!(style.delimiters.close, "");
    }
}
