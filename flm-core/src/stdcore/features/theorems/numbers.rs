//! The numbers of theorems: the counters and the counter styles that the
//! [`theorems`](super) feature declares. A
//! [counter](crate::stdcore::features::numbering::CounterDecl) is a dotted name
//! under which things are counted in document order, and a [counter
//! style](crate::stdcore::features::numbering::CounterStyleDecl) says how a
//! counter value is shown.
//!
//! # What the feature declares
//!
//! - The **shared counter** `flm.theorems.theorem` ([`SHARED_COUNTER`]), in
//!   which every kind of a numbered class with a shared counter counts, so
//!   that a theorem, a lemma and a definition in a row are numbered `1`, `2`,
//!   `3`. It is numbered within nothing: a document that numbers theorems
//!   within sections says so in its numbering options, and the numbers then
//!   read `2.3` on every output except the recomposed LaTeX, where LaTeX
//!   numbers the theorems (see [the
//!   limitation](super::latex#limitation-theorems-numbered-within-sections)).
//! - A **counter of its own**, `flm.theorems.counter.<kind>`
//!   ([`counter_name`]), for every kind of a numbered class whose kinds do
//!   not share the counter, also numbered within nothing. The kind `theorem`
//!   may count on its own beside kinds that share the counter: its own
//!   counter, `flm.theorems.counter.theorem`, is not the shared one.
//! - One **counter style** per numbered kind, `flm.theorems.style.<kind>`
//!   ([`style_name`]): the kind's words, each followed by a non-breaking
//!   space (`Lemma~`, `Lemmas~`, and every variant the kind declares), then
//!   the number in the configuration's number format, with no delimiters. A
//!   kind's reference point declares this style as its own, so `\ref{lem:x}`
//!   reads `Lemma~4` and the heading `Lemma 4`.
//! - The counters' **default style** `flm.theorems.number`
//!   ([`DEFAULT_STYLE`]), which writes the number alone, with no word. Every
//!   reference point of the feature declares its kind's style, so this style
//!   only completes the counters' declarations.
//!
//! # Where the declarations go
//!
//! The feature contributes these declarations to the numbering feature's
//! service slots `flm.numbering.counters` ([`COUNTERS_SLOT`], which takes
//! [`CounterDecl`] values) and `flm.numbering.styles` ([`STYLES_SLOT`], which
//! takes [`CounterStyleDecl`] values) as peer contributions: contributions
//! to a slot that another feature owns, which the registry build drops with
//! one Note when that feature is not registered (see
//! [`services`](crate::core::services)). The theorems feature therefore
//! registers in a registry without the numbering feature; there, no theorem
//! is numbered and every heading shows the kind's word alone.

use std::sync::Arc;

use crate::core::registry::{BuildError, FeatureBuilder};
use crate::model::names::Name;
use crate::stdcore::features::numbering::{
    COUNTERS_SLOT, CounterDecl, CounterStyleDecl, PluralWords, PrefixWords, STYLES_SLOT,
};

use super::config::{TheoremKind, TheoremsConfig};

/// The text that begins the name of a kind's own counter,
/// `flm.theorems.counter.`: the kind `exercise` of a class that does not
/// share the counter counts in `flm.theorems.counter.exercise`. See
/// [`counter_name`] for why a kind's own counter is not called
/// `flm.theorems.<kind>`.
pub const COUNTER_PREFIX: &str = "flm.theorems.counter.";

/// The shared counter, `flm.theorems.theorem`: the counter every kind of a
/// numbered class with a shared counter counts in.
pub const SHARED_COUNTER: &str = "flm.theorems.theorem";

/// The text that begins the name of a numbered kind's counter style,
/// `flm.theorems.style.`: a theorem of the kind `lemma` is shown in the
/// counter style `flm.theorems.style.lemma`.
pub const STYLE_PREFIX: &str = "flm.theorems.style.";

/// The default style of the feature's counters, `flm.theorems.number`: the
/// number alone, in the configuration's number format, with no word.
///
/// Its name does not begin with [`STYLE_PREFIX`], because every name that
/// begins with it is the style of the kind of the same last word, a host's
/// kind included: no kind's style name can clash with this one.
pub const DEFAULT_STYLE: &str = "flm.theorems.number";

/// The text a counter style writes between a kind's word and its number,
/// `~`, FLM's non-breaking space: `Lemma~4`.
const TIE: &str = "~";

/// The counter the kind `name` counts in, where `config` declares the kind's
/// class numbered: [`SHARED_COUNTER`] for a class with a shared counter, and
/// `flm.theorems.counter.<kind>` ([`COUNTER_PREFIX`]) for a class without
/// one. `None` for a kind that is not numbered, or whose kind or class is not
/// declared.
///
/// A kind's own counter is called `flm.theorems.counter.<kind>` rather than
/// `flm.theorems.<kind>`, the pattern of the xnotes feature's counters
/// (`flm.xnotes.footnote`), because under that pattern the kind `theorem`
/// counting on its own would count in the shared counter
/// `flm.theorems.theorem`. With the separate prefix, no kind's own counter
/// can be the shared counter, and a configuration in which theorems count on
/// their own while lemmas and propositions share the counter works like any
/// other.
pub fn counter_name(config: &TheoremsConfig, name: &str) -> Option<Name> {
    let (_, class) = config.kind_and_class(name)?;
    if !class.numbered {
        return None;
    }
    Some(if class.shared_counter {
        Name::from(SHARED_COUNTER)
    } else {
        Name::from(format!("{COUNTER_PREFIX}{name}").as_str())
    })
}

/// The name of the counter style of the numbered kind `name`,
/// `flm.theorems.style.<kind>` ([`STYLE_PREFIX`]).
pub fn style_name(name: &str) -> Name {
    Name::from(format!("{STYLE_PREFIX}{name}").as_str())
}

/// `words` with the tie after each word.
fn tied(words: &PrefixWords) -> PrefixWords {
    let tie = |plural: &PluralWords| {
        PluralWords::new(format!("{}{TIE}", plural.one), format!("{}{TIE}", plural.other))
    };
    PrefixWords {
        base: tie(&words.base),
        variants: words.variants.iter().map(|(name, plural)| (name.clone(), tie(plural))).collect(),
    }
}

/// The counter style of the numbered kind `name`: its words tied to the
/// number, the configuration's number format, no delimiters.
fn style_of(config: &TheoremsConfig, name: &str, kind: &TheoremKind) -> CounterStyleDecl {
    // See decision rationale at [§flm-dd-dr:std-theorem-counters].
    let mut style = CounterStyleDecl::new(style_name(name));
    style.number_format = config.number_format.clone();
    style.prefix_words = tied(&kind.words);
    style
}

/// Contributes the feature's counters and counter styles to the numbering
/// feature's service slots as peer contributions: the shared counter and the
/// default style always, a counter of its own for every numbered kind of a
/// class that does not share the counter, and one counter style per numbered
/// kind.
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a contribution is made
/// outside the registration round that accepts it.
pub(super) fn declare(b: &mut FeatureBuilder<'_>, config: &TheoremsConfig) -> Result<(), BuildError> {
    let mut default = CounterStyleDecl::new(DEFAULT_STYLE);
    default.number_format = config.number_format.clone();
    b.peer_slot::<CounterStyleDecl>(STYLES_SLOT, Arc::new(default))?;
    b.peer_slot::<CounterDecl>(
        COUNTERS_SLOT,
        Arc::new(
            CounterDecl::new(SHARED_COUNTER, DEFAULT_STYLE)
                .docs("The theorems of every kind that counts in the shared sequence of numbers."),
        ),
    )?;
    for (name, kind) in &config.kinds {
        let Some(counter) = counter_name(config, name.as_str()) else { continue };
        b.peer_slot::<CounterStyleDecl>(STYLES_SLOT, Arc::new(style_of(config, name.as_str(), kind)))?;
        let shared = config.classes.get(&kind.class).is_some_and(|class| class.shared_counter);
        if !shared {
            b.peer_slot::<CounterDecl>(
                COUNTERS_SLOT,
                Arc::new(
                    CounterDecl::new(counter, DEFAULT_STYLE)
                        .docs(format!("The `{name}` environments of the document.")),
                ),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::features::theorems::{TheoremClass, TheoremKind};

    #[test]
    fn a_kind_counts_in_the_counter_its_class_says() {
        let config = TheoremsConfig::new()
            .class("exerciselike", TheoremClass::numbered().shared_counter(false))
            .kind("exercise", TheoremKind::from_word("exerciselike", "exercise"));
        assert_eq!(counter_name(&config, "lemma").as_deref(), Some(SHARED_COUNTER));
        assert_eq!(
            counter_name(&config, "exercise").map(|name| name.to_string()).as_deref(),
            Some("flm.theorems.counter.exercise")
        );
        // The kind `theorem` on its own counts apart from the shared counter.
        let own = config
            .class("own", TheoremClass::numbered().shared_counter(false))
            .kind("theorem", TheoremKind::from_word("own", "theorem"));
        assert_eq!(
            counter_name(&own, "theorem").map(|name| name.to_string()).as_deref(),
            Some("flm.theorems.counter.theorem")
        );
        assert_eq!(counter_name(&own, "lemma").as_deref(), Some(SHARED_COUNTER));
        assert_eq!(counter_name(&own, "proof"), None);
        assert_eq!(counter_name(&own, "nosuchkind"), None);
    }

    #[test]
    fn a_style_ties_every_word_to_the_number() {
        let config = TheoremsConfig::new();
        let (kind, _) = config.kind_and_class("lemma").expect("shipped");
        let style = style_of(&config, "lemma", kind);
        assert_eq!(style.name.as_str(), "flm.theorems.style.lemma");
        assert_eq!(style.prefix_words.base, PluralWords::new("Lemma~", "Lemmas~"));
        assert_eq!(style.prefix_words.variants["abbreviated"], PluralWords::new("Lem.~", "Lems.~"));
        assert_eq!(style.delimiters.open, "");
        assert_eq!(style.delimiters.close, "");
    }
}
