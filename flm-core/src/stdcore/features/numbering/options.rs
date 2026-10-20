//! The numbering options of a document ([`NumberingOptions`]): per counter,
//! the counter within which it is numbered, and the value it has before it is
//! first advanced.
//!
//! The options are an option type of the numbering feature, bound on the
//! document; a render request may set them again
//! ([`Bind::Either`](crate::model::options::Bind::Either)). The numbering
//! stage applies them at the start of every run, before it reads any
//! reference point.

use std::collections::BTreeMap;

use crate::model::names::Name;

use super::decl::Within;

/// The place in the hierarchy of counters that a document's numbering options
/// give a counter, replacing the place the declaring feature gave it.
///
/// The place decides two things: what restarts the counter, and what is
/// written before its number.
#[derive(Clone, PartialEq, Eq, Hash, Debug, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WithinOption {
    /// Numbered within nothing: nothing restarts the counter, and no other
    /// counter's number is written before its number.
    ///
    /// A document uses it to take a counter out of a hierarchy it does not
    /// want. For example, a document whose subsections come before any
    /// section, or whose sections are never advanced, numbers its subsections
    /// within nothing, or gives the sections an
    /// [`initial`](CounterOptions::initial) value.
    Nothing,
    /// Numbered within the counters this [`Within`] gives, with the numbers
    /// written by its within-joiner.
    Counter(Within),
}

/// What a document's numbering options say about one counter: the counter
/// within which it is numbered, and the value it has before it is first
/// advanced. A field left `None` keeps what the counter was declared with.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CounterOptions {
    /// The counter's place in the hierarchy of counters. `None` keeps the
    /// place its declaration gives.
    pub within: Option<WithinOption>,
    /// The value the counter has before it is first advanced, or `None` for a
    /// counter that has no value until then.
    ///
    /// A counter with no value cannot be written, so a reference point
    /// numbered within it takes no number and is reported as the Error
    /// `flm.numbering.parent-uninitialized`. `Some(0)` gives the counter the
    /// value `0`, so that a subsection written before any section is numbered
    /// `0.1`. The counter is then advanced from that value: `Some(3)` makes
    /// the first section `4`. The value applies under every combination of
    /// the values of the counter's ancestors: an initial `0` for
    /// `flm.headings.subsection` makes a subsubsection written in a section
    /// that has no subsection yet read as though the subsection had the value
    /// 0, `1.0.1`. When the parent that applies is chosen among several, the
    /// counters with initial values count as advanced before the document
    /// begins, the outermost counter first.
    pub initial: Option<u32>,
}

impl CounterOptions {
    /// Returns options that change nothing.
    pub fn new() -> CounterOptions {
        CounterOptions::default()
    }

    /// Builder: numbers the counter within the counters `within` gives, in
    /// place of what it was declared with.
    ///
    /// The counters that `within` gives must be declared by some feature; a
    /// counter in `within` that no feature declares is reported as a Warning
    /// and the option changes nothing. The reports and their effects are
    /// listed under [`NumberingOptions`].
    pub fn within(mut self, within: Within) -> CounterOptions {
        self.within = Some(WithinOption::Counter(within));
        self
    }

    /// Builder: numbers the counter within nothing, so that it is never
    /// restarted and no other counter's number is written before its own.
    pub fn within_nothing(mut self) -> CounterOptions {
        self.within = Some(WithinOption::Nothing);
        self
    }

    /// Builder: gives the counter the value `value` before it is first
    /// advanced (see [`initial`](CounterOptions::initial)).
    pub fn initial(mut self, value: u32) -> CounterOptions {
        self.initial = Some(value);
        self
    }
}

/// The numbering options of a document: per counter, the counter within which
/// it is numbered, and the value it has before it is first advanced.
///
/// These are the per-document option values of the numbering feature: an
/// option type bound on the document, which a render request may set again,
/// its value then replacing the document's for that render
/// ([`Bind::Either`](crate::model::options::Bind::Either)). In data, they are
/// the settings key `counters` of the numbering feature, a map from a
/// counter's name to its [`CounterOptions`].
///
/// Whether a reference point is numbered at all is not an option of this
/// feature: a point whose number source gives a counter is numbered, and the
/// feature that owns the point decides whether it gives one (for example the
/// numbering depth of the headings feature, fixed when the registry is built).
///
/// A counter that the options do not mention keeps what its declaring feature
/// said. The numbering stage reports, once per run, every entry it cannot
/// apply, and applies the others:
///
/// - an entry for a counter no feature declares is a Warning
///   (`flm.numbering.unknown-counter`) and changes nothing; an entry that
///   numbers a counter within a counter no feature declares is the same
///   Warning, and its `within` changes nothing (its `initial` still applies);
/// - an entry that numbers a counter within a parent set no feature registers
///   (`flm.numbering.unknown-parent-set`), within no counter at all (an empty
///   list of parents, or a parent set that holds no counter;
///   `flm.numbering.bad-within-joiner`), or through a within-joiner that does
///   not compile (`flm.numbering.bad-within-joiner`) or that no feature
///   registers (`flm.numbering.unknown-within-joiner`), is an Error, and its
///   `within` changes nothing (its `initial` still applies);
/// - an entry whose within-joiner writes a counter that, under the options,
///   is not always one of the counters the counter is numbered within is an
///   Error (`flm.numbering.not-an-ancestor`), and the counter keeps the place
///   it was declared with;
/// - an entry that moves a counter's ancestors so that the declared
///   within-joiner or nesting joiner of a counter writes a counter it is no
///   longer always numbered within is an Error
///   (`flm.numbering.not-an-ancestor`), and no point counting in that counter
///   is numbered in that run;
/// - entries that make counters numbered within each other are an Error
///   (`flm.numbering.hierarchy-cycle`), and no point counting in those
///   counters is numbered in that run.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::numbering::{NumberingOptions, Within};
///
/// let options = NumberingOptions::new()
///     .initial("flm.headings.section", 0)
///     .within("flm.math.equation", Within::parent("flm.headings.section"));
/// assert_eq!(options.of("flm.headings.section").and_then(|entry| entry.initial), Some(0));
/// assert!(options.of("example.figure").is_none());
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct NumberingOptions {
    /// What the document says about each counter it mentions, in name order.
    pub counters: BTreeMap<Name, CounterOptions>,
}

impl NumberingOptions {
    /// Returns options that say nothing about any counter.
    pub fn new() -> NumberingOptions {
        NumberingOptions::default()
    }

    /// Returns what these options say about `counter`, or `None` when they say
    /// nothing about it.
    pub fn of(&self, counter: &str) -> Option<&CounterOptions> {
        self.counters.get(&Name::new(counter))
    }

    /// Builder: replaces what the options say about `counter` by `entry`.
    pub fn counter(mut self, counter: impl Into<Name>, entry: CounterOptions) -> NumberingOptions {
        self.counters.insert(counter.into(), entry);
        self
    }

    /// Builder: numbers `counter` within the counters `within` gives, keeping
    /// the rest of what the options say about `counter`.
    ///
    /// The counters that `within` gives must be declared by some feature; a
    /// counter in `within` that no feature declares is reported as a Warning
    /// and the option changes nothing. The reports and their effects are
    /// listed under [`NumberingOptions`].
    pub fn within(mut self, counter: impl Into<Name>, within: Within) -> NumberingOptions {
        self.counters.entry(counter.into()).or_default().within =
            Some(WithinOption::Counter(within));
        self
    }

    /// Builder: numbers `counter` within nothing, keeping the rest of what the
    /// options say about `counter`.
    pub fn within_nothing(mut self, counter: impl Into<Name>) -> NumberingOptions {
        self.counters.entry(counter.into()).or_default().within = Some(WithinOption::Nothing);
        self
    }

    /// Builder: gives `counter` the value `value` before it is first advanced,
    /// keeping the rest of what the options say about `counter` (see
    /// [`CounterOptions::initial`]).
    pub fn initial(mut self, counter: impl Into<Name>, value: u32) -> NumberingOptions {
        self.counters.entry(counter.into()).or_default().initial = Some(value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_builders_write_into_one_entry_per_counter() {
        let options = NumberingOptions::new()
            .initial("section", 0)
            .within("section", Within::parent("chapter"));
        assert_eq!(options.counters.len(), 1);
        let entry = options.of("section").expect("set");
        assert_eq!(entry.initial, Some(0));
        assert!(matches!(entry.within, Some(WithinOption::Counter(_))));
    }

    #[test]
    fn a_counter_the_document_says_nothing_about_has_no_entry() {
        assert!(NumberingOptions::new().of("equation").is_none());
    }
}
