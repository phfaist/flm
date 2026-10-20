//! The numbers of headings: the counter and the counter style each heading kind
//! declares, which counter each is numbered within, and the word a reference writes
//! before a heading's number.
//!
//! A *counter* is a dotted name under which things are counted in document order,
//! declared by a feature together with the counter it is numbered within
//! ([`CounterDecl`]), and a *counter style* says how a counter value is shown: the
//! number template, the sub-levels, the prefix words, the delimiters and the join texts
//! ([`CounterStyleDecl`]; see [counters and counter
//! styles](crate::guide::concepts_overview#counters-and-counter-styles)).
//!
//! # What a heading kind declares
//!
//! Each heading kind ([`HeadingDecl`]) that has a counter declares that counter,
//! `flm.headings.<name>` by default ([`COUNTER_PREFIX`](super::COUNTER_PREFIX)), and
//! its counter style, `flm.headings.style.<name>` by default
//! ([`STYLE_PREFIX`](super::STYLE_PREFIX)). The feature makes both from the kinds of
//! its configuration; a kind without a counter declares nothing.
//!
//! The five standard kinds ([`HeadingsConfig::new`]) count in the counters
//! `flm.headings.section`, `flm.headings.subsection`, `flm.headings.subsubsection`,
//! `flm.headings.paragraph` and `flm.headings.subparagraph`. The first three are
//! written in decimal digits and the last two in letters, so a numbered `\paragraph`
//! reads `a.`, `b.`, `c.`. Each kind except the first and the fourth is **numbered
//! within** the kind above it, which means two things: the counter restarts whenever
//! the counter above it advances, and the number above is written before its own
//! (`1.2` for the second subsection of the first section). A `\paragraph` is numbered
//! within whichever of `\section`, `\subsection` and `\subsubsection` came last: it
//! restarts at each of them, so a paragraph directly under a subsection, with no
//! subsubsection before it, is still numbered; and it shows no part of their numbers.
//! This is the only standard kind for which the two meanings of "numbered within"
//! differ.
//!
//! Where a number is shown on its own, in a reference to a heading, the word [`WORD`]
//! is written before it, so `\ref{sec:intro}` reads `§ 1.2`, with a narrow no-break
//! space between the sign and the number.
//!
//! # Declared whether numbered or not
//!
//! A kind declares its counter and its counter style regardless of the
//! configuration's numbering depth ([`HeadingsConfig::numbering`]). A kind the depth
//! leaves unnumbered states no number source, so its counter never advances; the
//! declaration still lets a document's numbering options number another counter
//! within it. The [feature's documentation](super#numbering) says how the depth is
//! set.
//!
//! # The parent set of the levels
//!
//! The feature also declares the parent set [`LEVELS_PARENT_SET`],
//! `flm.headings.levels`: the counters of the kinds that have one, the outermost level
//! first, and the kinds of one level in the order they are declared.
//!
//! # Where these declarations go
//!
//! The counters, the counter styles and the parent set are declared to the three
//! service slots of the [`numbering`](crate::stdcore::features::numbering) feature
//! they belong in ([`COUNTERS_SLOT`], [`STYLES_SLOT`], [`PARENT_SETS_SLOT`]), as peer
//! contributions: values for a service slot another feature declares, which the
//! registry build drops with one Note when no feature declares the slot (see
//! [services and service
//! slots](crate::guide::concepts_overview#services-and-service-slots)). The headings
//! feature therefore registers in a registry that holds no `numbering` feature, and its
//! headings then have no numbers.

use std::sync::Arc;

use crate::core::registry::{BuildError, FeatureBuilder};
use crate::stdcore::features::numbering::{
    COUNTERS_SLOT, CounterDecl, CounterStyleDecl, JoinDecl, PARENT_SETS_SLOT, ParentSetDecl,
    PrefixWords, STYLES_SLOT,
};

use super::{HeadingDecl, HeadingsConfig, LEVELS_PARENT_SET};

/// The role of the span that holds a numbered heading's number,
/// `flm.heading.number`.
///
/// A role is a feature-owned dotted name on a presentation payload that says what
/// a container is ([`pres::payload`](crate::stdcore::backends::pres::payload); see
/// [decorations, roles and
/// kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)). A backend
/// or a stylesheet that selects on this role can set the number apart from the
/// title; one that does not writes the number as ordinary content. The HTML
/// backend writes the role as the second CSS class of the span,
/// `flm-heading-number`.
pub const NUMBER_ROLE: &str = "flm.heading.number";

/// The word written before a heading's number where the number is shown on its
/// own: the section sign `§` followed by a narrow no-break space (U+202F).
///
/// The space is as narrow as a thin space and no line is ever broken at it, so
/// the sign is always on the same line as the number it introduces. The
/// plain-text backend writes it as it is, as it writes every no-break space.
///
/// A heading shows no word before its own number; the word belongs to what a
/// *reference* to the heading shows, `§ 1.2`. A reference written
/// `\ref[noprefix]{sec:intro}` drops it and shows `1.2`. The word is the same
/// for one number and for several, so `\ref{sec:a,sec:b}` reads `§ 1,2`.
pub const WORD: &str = "§\u{202F}";

/// Returns the counter style of one heading kind, as the `numbering` feature
/// takes it: the kind's number format, the word [`WORD`] before one number and
/// before several, no delimiters, and the compact join texts, which put a comma
/// without a space between two numbers written together. `None` for a kind with
/// no counter style.
fn style_of(kind: &HeadingDecl) -> Option<CounterStyleDecl> {
    let mut style = CounterStyleDecl::new(kind.style.clone()?);
    style.number_format = kind.number_format.clone();
    style.prefix_words = PrefixWords::new(WORD, WORD);
    style.join = JoinDecl::compact();
    Some(style)
}

/// Returns the counter of one heading kind, as the `numbering` feature takes it:
/// shown in the kind's own counter style, and numbered within the counter the
/// kind's declaration gives. `None` for a kind with no counter, or with a counter
/// and no counter style, which the configuration's check refuses.
fn counter_of(kind: &HeadingDecl) -> Option<CounterDecl> {
    let decl = CounterDecl::new(kind.counter.clone()?, kind.style.clone()?)
        .docs(format!("The `{}` headings of the document.", kind.name));
    Some(match &kind.within {
        Some(within) => decl.within(within.clone()),
        None => decl,
    })
}

/// Returns the parent set of the levels of `config`: the counters of the kinds
/// that have one, the outermost level first, and the kinds of one level in the
/// order they are declared.
fn levels_of(config: &HeadingsConfig) -> ParentSetDecl {
    let mut kinds: Vec<&HeadingDecl> =
        config.headings.iter().filter(|kind| kind.counter.is_some()).collect();
    kinds.sort_by_key(|kind| kind.level);
    ParentSetDecl::new(LEVELS_PARENT_SET, kinds.into_iter().filter_map(|kind| kind.counter.clone()))
}

/// Declares, for every kind of `config` that has a counter, its counter and its
/// counter style to the service slots of the `numbering` feature, and the parent
/// set of the levels, as peer contributions. A kind with no counter declares
/// nothing.
///
/// # Errors
///
/// [`BuildError::ContributionAfterAssembly`] when called from a
/// [`Finalizer`](crate::core::registry::Finalizer), the second registration round, in
/// which a feature may no longer contribute to a service slot
/// ([`FeatureBuilder`]).
pub(super) fn declare(b: &mut FeatureBuilder<'_>, config: &HeadingsConfig) -> Result<(), BuildError> {
    for kind in &config.headings {
        let (Some(style), Some(counter)) = (style_of(kind), counter_of(kind)) else { continue };
        b.peer_slot::<CounterStyleDecl>(STYLES_SLOT, Arc::new(style))?;
        b.peer_slot::<CounterDecl>(COUNTERS_SLOT, Arc::new(counter))?;
    }
    b.peer_slot::<ParentSetDecl>(PARENT_SETS_SLOT, Arc::new(levels_of(config)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_kind_has_a_counter_named_after_it() {
        let config = HeadingsConfig::new();
        for kind in &config.headings {
            let counter = counter_of(kind).expect("a counter");
            assert_eq!(counter.name.as_str(), format!("flm.headings.{}", kind.name));
            assert_eq!(style_of(kind).expect("a style").name.as_str(), format!("flm.headings.style.{}", kind.name));
        }
    }

    #[test]
    fn each_shipped_kind_but_the_first_is_numbered_within_the_one_above_it_and_the_fourth_within_any_of_the_three_above_it() {
        use crate::model::names::Name;
        use crate::stdcore::features::numbering::{Parents, WithinJoinerSpec};
        let config = HeadingsConfig::new();
        assert!(config.headings[0].within.is_none());
        for (index, kind) in config.headings.iter().enumerate().skip(1) {
            let within = kind.within.as_ref().expect("a kind below the first");
            if index == 3 {
                continue;
            }
            assert_eq!(within.parents, Parents::One(config.headings[index - 1].counter.clone().expect("a counter")));
        }
        // The fourth kind restarts under whichever of the three above it came
        // last, and writes nothing of them.
        let fourth = config.headings[3].within.as_ref().expect("the fourth kind");
        assert_eq!(
            fourth.parents,
            Parents::Any(vec![
                Name::from("flm.headings.section"),
                Name::from("flm.headings.subsection"),
                Name::from("flm.headings.subsubsection"),
            ])
        );
        assert_eq!(fourth.joiner, WithinJoinerSpec::Bare);
    }

    #[test]
    fn a_counter_is_declared_in_its_own_style() {
        for kind in &HeadingsConfig::new().headings {
            let decl = counter_of(kind).expect("a counter");
            assert_eq!(Some(&decl.default_style), kind.style.as_ref());
        }
    }

    #[test]
    fn the_levels_run_outermost_first_over_the_kinds_with_a_counter() {
        let mut kinds = HeadingsConfig::new().headings;
        kinds.insert(0, HeadingDecl::new("aside", 2).no_counter());
        kinds.push(HeadingDecl::new("chapter", 1));
        let levels = levels_of(&HeadingsConfig::new().headings(kinds));
        let names: Vec<&str> = levels.counters.iter().map(|name| name.as_str()).collect();
        assert_eq!(
            names,
            [
                "flm.headings.section",
                "flm.headings.chapter",
                "flm.headings.subsection",
                "flm.headings.subsubsection",
                "flm.headings.paragraph",
                "flm.headings.subparagraph",
            ]
        );
        assert_eq!(levels.name.as_str(), LEVELS_PARENT_SET);
    }

    #[test]
    fn a_kind_with_no_counter_declares_nothing() {
        let kind = HeadingDecl::new("aside", 3).no_counter();
        assert!(counter_of(&kind).is_none());
        assert!(style_of(&kind).is_none());
    }
}
