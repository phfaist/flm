//! Counter values ([`CounterValue`]): the value a counter has at a numbered
//! reference point, and the text that identifies the point by that value
//! ([`identifier_text`]).

use super::style::Ancestor;

/// A counter value: the value a counter has at one numbered reference point,
/// a number with optional **sub-numbers** under it.
///
/// A sub-number is a further level of counting under the number: the value
/// `(3, 2)` is the second sub-numbered item under the number three, shown as
/// `3b` by a counter style whose first sub-level is written in letters
/// ([`SubLevelDecl`](super::SubLevelDecl)). A value always has a number, and
/// it has a sub-number for every deeper level at which the counter was
/// advanced.
///
/// Values are ordered by their components: the number first, then each
/// sub-number in turn, a value without a given sub-number coming before the
/// same value with one. The default number writer sorts several values in
/// this order before it writes them, and collapses values that
/// [immediately succeed](CounterValue::immediately_succeeds) one another into
/// a range.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::numbering::CounterValue;
///
/// let third = CounterValue::new(3);
/// assert_eq!(third.number(), 3);
/// assert!(third.sub_numbers().is_empty());
///
/// let second_under_third = CounterValue::with_sub_numbers(3, [2]);
/// assert_eq!(second_under_third.components(), [3, 2]);
/// assert!(second_under_third.immediately_succeeds(&CounterValue::with_sub_numbers(3, [1])));
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CounterValue {
    /// The number followed by the sub-numbers. Never empty: the first entry is
    /// the number.
    components: Vec<i64>,
}

impl CounterValue {
    /// Returns the value `number`, with no sub-number.
    pub fn new(number: i64) -> CounterValue {
        CounterValue { components: vec![number] }
    }

    /// Returns the value `number` with the given sub-numbers, outermost first.
    pub fn with_sub_numbers(number: i64, sub_numbers: impl IntoIterator<Item = i64>) -> CounterValue {
        let mut components = vec![number];
        components.extend(sub_numbers);
        CounterValue { components }
    }

    /// Returns the number: the value's count at the counter's own level.
    pub fn number(&self) -> i64 {
        self.components[0]
    }

    /// Returns the sub-numbers, outermost first; empty for a value that has
    /// none.
    pub fn sub_numbers(&self) -> &[i64] {
        &self.components[1..]
    }

    /// Returns the value's components: the number followed by the
    /// sub-numbers, in the order they are written. Never empty.
    pub fn components(&self) -> &[i64] {
        &self.components
    }

    /// Returns whether this value comes immediately after `other`: the two
    /// have the same number of components, all components but the last are
    /// equal, and the last is one more.
    ///
    /// The default number writer builds runs of consecutive values with this
    /// test, so `(3, 4)` immediately succeeds `(3, 3)`, while `4` does not
    /// immediately succeed `(3, 9)`: values of different depths are never
    /// consecutive.
    pub fn immediately_succeeds(&self, other: &CounterValue) -> bool {
        if self.components.len() != other.components.len() {
            return false;
        }
        let split = self.components.len() - 1;
        self.components[..split] == other.components[..split]
            && other.components[split]
                .checked_add(1)
                .is_some_and(|next| self.components[split] == next)
    }

    /// Returns this value advanced at `level`: level 0 is the number, level 1
    /// the first sub-number, and so on. The advanced level gains one, every
    /// deeper level returns to zero, and a level the value does not have yet
    /// is created, together with the levels between, which are created at
    /// zero.
    ///
    /// Advancing is the only operation that moves a counter on, and creating a
    /// deeper level is part of it: a reference point that starts a
    /// sub-numbered group advances the number and then the first sub-number,
    /// which takes `3` to `(3, 1)`.
    ///
    /// A level that already has the largest number a value can hold
    /// (`i64::MAX`) keeps it instead of wrapping round to the smallest. No
    /// document reaches it, since every level counts points one by one.
    ///
    /// The caller is responsible for bounding `level`: a value holds every
    /// level up to the one advanced, so advancing at a level far deeper than
    /// the value's own depth takes memory in proportion to that level. A
    /// counter style declares how many sub-numbers it can write, and the
    /// numbering stage refuses a point that would advance a level deeper than
    /// that before it advances anything; this bounds the work in the library.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::features::numbering::CounterValue;
    ///
    /// let value = CounterValue::with_sub_numbers(3, [2, 1]);
    /// assert_eq!(value.advanced(1).components(), [3, 3, 0]);
    /// assert_eq!(value.advanced(0).components(), [4, 0, 0]);
    /// assert_eq!(CounterValue::new(3).advanced(1).components(), [3, 1]);
    /// ```
    pub fn advanced(&self, level: usize) -> CounterValue {
        let mut components = self.components.clone();
        while components.len() <= level {
            components.push(0);
        }
        components[level] = components[level].saturating_add(1);
        for deeper in components.iter_mut().skip(level + 1) {
            *deeper = 0;
        }
        CounterValue { components }
    }
}

/// Returns the text that identifies a numbered reference point by its
/// counter value: the values of the ancestors first, outermost first as a
/// [`Parent`](super::Parent) carries them, then the point's own value, every
/// component written in decimal and joined by hyphens.
///
/// A theorem numbered 3 within section 2 gives `2-3`, and a value with
/// sub-numbers contributes each of them: `(1, 2, 3)` with no ancestor gives
/// `1-2-3`. The text holds no counter name and no word; a caller that builds
/// an identifier from it adds its own prefix. The refs feature builds the
/// anchor of a point whose anchor is derived from its number in this way
/// ([`ANCHORS`](crate::stdcore::features::refs::ANCHORS)).
///
/// The text is not the number a reader sees: it is written in decimal
/// regardless of the counter style, so that the identifier of a point does
/// not change when its counter style does.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::numbering::{identifier_text, CounterValue};
///
/// assert_eq!(identifier_text(&[], &CounterValue::with_sub_numbers(1, [2, 3])), "1-2-3");
/// ```
pub fn identifier_text(ancestors: &[Ancestor], value: &CounterValue) -> String {
    let mut out = String::new();
    let components = ancestors
        .iter()
        .flat_map(|ancestor| ancestor.value.components())
        .chain(value.components());
    for component in components {
        if !out.is_empty() {
            out.push('-');
        }
        out.push_str(&component.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_orders_by_its_components() {
        let mut values = [
            CounterValue::with_sub_numbers(1, [2]),
            CounterValue::new(1),
            CounterValue::new(0),
            CounterValue::with_sub_numbers(1, [1]),
        ];
        values.sort();
        let ordered: Vec<&[i64]> = values.iter().map(CounterValue::components).collect();
        assert_eq!(ordered, [&[0][..], &[1][..], &[1, 1][..], &[1, 2][..]]);
    }

    #[test]
    fn advancing_the_deepest_level_of_the_largest_number_does_not_overflow() {
        let value = CounterValue::new(i64::MAX);
        assert_eq!(value.advanced(0).number(), i64::MAX);
    }

    #[test]
    fn a_value_never_immediately_succeeds_one_of_another_depth() {
        assert!(!CounterValue::new(2).immediately_succeeds(&CounterValue::with_sub_numbers(1, [9])));
        assert!(!CounterValue::with_sub_numbers(2, [1]).immediately_succeeds(&CounterValue::new(1)));
    }
}
