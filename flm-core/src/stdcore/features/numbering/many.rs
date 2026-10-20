//! Writing several counter values at once ([`format_many`]), such as
//! `Eqs.~(1–3, 7, and 9)`, and the default number writer's algorithm for
//! several values.
//!
//! The default [number writer](super::NumberWriter) groups the values by the
//! counters they are numbered within, sorts each group and collapses its
//! consecutive values into ranges (unless the counter style's
//! [`ranges`](super::CounterStyleDecl::ranges) is off), and joins everything
//! with the counter style's join texts. A counter style written by another
//! writer goes through that writer instead, which may write the same values
//! in any other way.

use super::style::{
    list_shape, number_pieces, on_worker, segments, CounterStyle, FormatError, FormatOptions,
    Item, ListPiece, Mark, Piece, Segment,
};
use super::value::CounterValue;
use super::writer::{JoinText, NumberOut, NumberedValue, WriteCx};

/// One run: a sequence of consecutive values of one group, given by its first
/// and its last value. A run of one value has the same value at both ends.
#[derive(Clone, Copy)]
struct Run<'a> {
    first: NumberedValue<'a>,
    last: NumberedValue<'a>,
}

/// Writes several counter values in `style` as one text and returns the
/// segments of that text: the word, the delimiters, and the values as the
/// counter style's [number writer](super::NumberWriter) writes them.
///
/// Each value carries the counters it is numbered within ([`NumberedValue`]),
/// and the values come in the caller's order. Under the default writer:
///
/// - values numbered within the same counters, with the same values, and
///   counted in the same scoped instance (or in none) form one group, and the
///   groups are written in the order of their first value in the caller's
///   list;
/// - within a group, the values are sorted, and runs of consecutive values
///   are collapsed into ranges; a range never spans two groups. A counter
///   style whose [`ranges`](super::CounterStyleDecl::ranges) is off collapses
///   nothing: the values are sorted and each is written separately, `1,2,3`;
/// - a single run of exactly two consecutive values is written as two
///   values, `1 and 2`, not as a range;
/// - no value is removed: a value given twice is written twice.
///
/// The link marks of the segments are positions in the caller's list: the
/// first value is `0`, regardless of where the sorting and the grouping put
/// it. Each number links to its own value, and the texts that join numbers
/// link to nothing. The word before the numbers links to the first number
/// written when the counter style's
/// [`name_in_link`](super::CounterStyleDecl::name_in_link) is on (the
/// default), and to nothing otherwise; with a single value and `name_in_link`
/// on, the whole text is one link.
///
/// With no value at all, the result is the counter style's `empty` join text,
/// written without the word and without delimiters.
///
/// # Errors
///
/// The same as [`format`](super::format): a sub-number at a level the counter
/// style declares no format for, a within-joiner that refers to a counter
/// that is not an ancestor, and a failure to build the content.
///
/// # Examples
///
/// ```
/// use flm_core::lang::{build_language, LangInputs};
/// use flm_core::stdcore::features::numbering::{
///     format_many, CounterStyleDecl, CounterValue, FormatOptions, JoinDecl, NumberedValue,
/// };
///
/// let lang = build_language(LangInputs::new());
/// let mut decl = CounterStyleDecl::new("example.style.item");
/// decl.join = JoinDecl::compact();
/// let style = decl.compile(&lang).expect("the style compiles");
///
/// let values = [CounterValue::new(3), CounterValue::new(1), CounterValue::new(2)];
/// let written: Vec<NumberedValue<'_>> = values.iter().map(NumberedValue::new).collect();
/// let segments = format_many(&style, &written, &FormatOptions::new())
///     .expect("the numbers are written");
/// // Three consecutive values are one range: `1`, `–`, `3`.
/// assert_eq!(segments.len(), 3);
/// assert_eq!(segments[0].link(), Some(1));
/// assert_eq!(segments[1].link(), None);
/// assert_eq!(segments[2].link(), Some(0));
/// ```
pub fn format_many(
    style: &CounterStyle,
    values: &[NumberedValue<'_>],
    options: &FormatOptions,
) -> Result<Vec<Segment>, FormatError> {
    // The whole call runs on a worker thread, for the reason
    // [`on_worker`](super::style::on_worker) gives.
    on_worker(|| {
        // A value's place among the caller's values is what its link mark
        // carries, and it is the place in this list, whatever the writer then
        // does with the values.
        let numbered: Vec<NumberedValue<'_>> =
            values.iter().enumerate().map(|(index, value)| value.at(index)).collect();
        let cx = WriteCx::new(style);
        let mut out = NumberOut::new();
        style.writer().write_many(&numbered, options, &cx, &mut out)?;
        segments(style, out.into_items())
    })
}

/// Writes several values of one counter style as the default writer does.
///
/// [`WriteCx::default_many`] calls it, so a registered writer that hands a
/// whole list to the default writer gets this result.
///
/// # Errors
///
/// The same as [`format_many`].
pub(super) fn default_many<'a>(
    values: &[NumberedValue<'a>],
    options: &FormatOptions,
    cx: &WriteCx<'a>,
    out: &mut NumberOut<'a>,
) -> Result<(), FormatError> {
    let style = cx.style();
    let count = values.len();
    if count == 0 {
        out.item(Item::text(cx.join_text(JoinText::Empty), Mark::Never));
        return Ok(());
    }
    let groups = groups_of(values);
    let runs = runs_of(&groups, style.ranges());
    let mut items = join(cx, &runs)?;
    let only_one = count == 1;

    // The word takes the link of the first value there is one for; with one
    // value the delimiters take it too, so that the whole text is one link.
    let first = if style.name_in_link() {
        items
            .iter()
            .find_map(|item| match item.mark {
                Mark::Value(position) => Some(Mark::Value(position)),
                _ => None,
            })
            .unwrap_or(Mark::Either)
    } else {
        Mark::Never
    };
    let around = if style.name_in_link() && only_one { Mark::Either } else { Mark::Never };

    if options.with_word {
        if let Some(word) = cx.word(count, options.variant.as_deref()) {
            out.item(Item::text(word, first));
        }
    }
    if options.with_delimiters {
        out.item(Item::text(cx.open(), around));
    }
    out.items(items.drain(..));
    if options.with_delimiters {
        out.item(Item::text(cx.close(), around));
    }
    Ok(())
}

/// Returns the values grouped by the counters they are numbered within: two
/// values are in one group when they were counted in the same scoped instance,
/// or in none, and the same within-joiner writes before them the same
/// ancestors, with the same values and of the same instances. A group keeps
/// the place of its first value.
///
/// The groups decide which values may collapse into a range with each other:
/// a range may only collapse numbers whose ancestors are written the same way,
/// so `A.I` and `A.III` may, and `A.III` and `B.I` may not, even when the two
/// counter values are consecutive.
fn groups_of<'a>(values: &[NumberedValue<'a>]) -> Vec<Vec<NumberedValue<'a>>> {
    let mut groups: Vec<Vec<NumberedValue<'a>>> = Vec::new();
    for value in values {
        match groups.iter().position(|group| same_parent(&group[0], value)) {
            Some(index) => groups[index].push(*value),
            None => groups.push(vec![*value]),
        }
    }
    groups
}

/// Returns whether two values are numbered within the same counters, written
/// by the same within-joiner: the same counters with the same values, in the
/// same order, each counted in the same scoped instance, and the two values
/// themselves counted in the same instance, since two instances are two
/// separate sequences of the counter regardless of their ancestors.
fn same_parent(left: &NumberedValue<'_>, right: &NumberedValue<'_>) -> bool {
    if left.instance() != right.instance() {
        return false;
    }
    let (left_parent, right_parent) = (left.parent(), right.parent());
    match (left_parent, right_parent) {
        (None, None) => true,
        (Some(left_parent), Some(right_parent)) => {
            // The joiners are compared by the address of their data alone,
            // without the table of methods a trait object also carries, so two
            // joiners that hold no data of their own compare equal. The values
            // compared here come from the records of one counter table, where a
            // counter holds its joiner beside the ancestors it is numbered
            // within, so values with equal ancestors carry the one joiner
            // anyway.
            std::ptr::eq(
                std::ptr::from_ref(left_parent.joiner).cast::<()>(),
                std::ptr::from_ref(right_parent.joiner).cast::<()>(),
            ) && left.ancestors().len() == right.ancestors().len()
                && left.ancestors().iter().zip(right.ancestors()).all(|(one, other)| {
                    one.counter == other.counter
                        && one.value == other.value
                        && one.instance == other.instance
                })
        }
        _ => false,
    }
}

/// Returns the runs that the groups' values form: each group sorted,
/// consecutive values collapsed into one run when `ranges` is on (each value a
/// run of its own when it is off), the groups kept in order.
///
/// A result that is one run of exactly two values is split into two runs of
/// one, so that a lone consecutive pair is written `1 and 2` instead of as a
/// range.
fn runs_of<'a>(groups: &[Vec<NumberedValue<'a>>], ranges: bool) -> Vec<Run<'a>> {
    let mut runs: Vec<Run<'a>> = Vec::new();
    for group in groups {
        let mut entries: Vec<NumberedValue<'a>> = group.clone();
        entries.sort_by(|left, right| left.value().cmp(right.value()));
        let mut current: Option<Run<'a>> = None;
        for entry in entries {
            match current {
                Some(run) if ranges && entry.value().immediately_succeeds(run.last.value()) => {
                    current = Some(Run { first: run.first, last: entry });
                }
                Some(run) => {
                    runs.push(run);
                    current = Some(Run { first: entry, last: entry });
                }
                None => current = Some(Run { first: entry, last: entry }),
            }
        }
        if let Some(run) = current {
            runs.push(run);
        }
    }
    if runs.len() == 1 && runs[0].last.value().immediately_succeeds(runs[0].first.value()) {
        let run = runs[0];
        runs = vec![
            Run { first: run.first, last: run.first },
            Run { first: run.last, last: run.last },
        ];
    }
    runs
}

/// Returns the items that the runs become, joined by the counter style's join
/// texts: one run takes the `one_*` texts, two runs take the `pair_*` texts,
/// more take the `list_*` texts, in the places [`list_shape`] gives.
///
/// The `one_pre` and `one_post` texts around a single run join the link of
/// the run's value, if it has one; every other join text links to nothing.
fn join<'a>(cx: &WriteCx<'a>, runs: &[Run<'a>]) -> Result<Vec<Item<'a>>, FormatError> {
    // No run at all writes nothing: a call with no value answers the style's
    // `empty` text before it gets here.
    let mut items = Vec::new();
    for piece in list_shape(runs.len()) {
        match piece {
            ListPiece::Text(which) => {
                let mark = match which {
                    JoinText::OnePre | JoinText::OnePost => Mark::Either,
                    _ => Mark::Never,
                };
                items.push(Item::text(cx.join_text(which), mark));
            }
            ListPiece::Item(index) => items.append(&mut render(cx, &runs[index])?),
        }
    }
    Ok(items)
}

/// Returns the items that one run becomes: its single value, or its two ends
/// with the range texts around and between them.
///
/// The end of a range is written without the parts of its ancestors and
/// without the leading components it shares with the start (`A.II.b–e`),
/// unless the run is a pair of consecutive values or the counter style
/// repeats the ancestors' parts.
fn render<'a>(cx: &WriteCx<'a>, run: &Run<'a>) -> Result<Vec<Item<'a>>, FormatError> {
    let style = cx.style();
    if run.first.value() == run.last.value() {
        let pieces = value_pieces(cx, &run.first, None)?;
        return Ok(vec![Item { pieces, mark: Mark::Value(run.first.index()) }]);
    }
    let consecutive = run.last.value().immediately_succeeds(run.first.value());
    let from = (!consecutive && !style.repeat_parents_in_range()).then(|| run.first.value());
    let mid = if consecutive { JoinText::RangePairMid } else { JoinText::RangeMid };
    Ok(vec![
        Item::text(cx.join_text(JoinText::RangePre), Mark::Never),
        Item {
            pieces: value_pieces(cx, &run.first, None)?,
            mark: Mark::Value(run.first.index()),
        },
        Item::text(cx.join_text(mid), Mark::Never),
        Item {
            pieces: value_pieces(cx, &run.last, from)?,
            mark: Mark::Value(run.last.index()),
        },
        Item::text(cx.join_text(JoinText::RangePost), Mark::Never),
    ])
}

/// Returns the pieces that one value becomes.
///
/// `from` is the start of the range that this value ends, when the value is
/// written as the end of a range: the ancestors' parts are then left out, and
/// so is every leading component the two values share.
fn value_pieces<'a>(
    cx: &WriteCx<'a>,
    value: &NumberedValue<'a>,
    from: Option<&CounterValue>,
) -> Result<Vec<Piece<'a>>, FormatError> {
    let own = value.value();
    let mut number = Some(own.number());
    let mut sub_numbers: Vec<Option<i64>> =
        own.sub_numbers().iter().map(|sub| Some(*sub)).collect();
    let mut within = Some((value, cx));
    if let Some(from) = from {
        within = None;
        if from.number() == own.number() {
            number = None;
        }
        for (level, sub_number) in sub_numbers.iter_mut().enumerate() {
            if from.sub_numbers().get(level) == sub_number.as_ref() {
                *sub_number = None;
            }
        }
    }
    let mut pieces = Vec::new();
    number_pieces(cx.style(), number, &sub_numbers, within, &mut pieces)?;
    Ok(pieces)
}
