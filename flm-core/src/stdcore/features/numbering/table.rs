//! The counter table, `flm.counters` ([`COUNTERS`], a [`Counters`]): what the
//! numbering stage recorded for every reference point it read, and the
//! methods that write those records as the numbers a reader sees.
//!
//! A record ([`CounterRecord`]) holds a counter value and no text. A lowering
//! rule writes the number from the record where it shows the number, with
//! [`Counters::format`], [`Counters::format_in_style`] or
//! [`Counters::format_many`], so that a number is counted once and stored in
//! one form only, and the same record serves every backend and every
//! reference to the point.

use std::collections::BTreeMap;
use std::sync::Arc;

use techy::core::node::NodeTree;
use techy::serialize::SerialValue;

use crate::core::data::table::{PieceClass, ProjectCx, SingletonTable, Table, TableName};
use crate::model::addr::NodeAddr;
use crate::model::aspects::PointId;
use crate::model::names::Name;
use crate::lang::Flm;

use super::many::format_many;
use super::joiner::{written_with_keys, CompiledWithinJoiner, JoinCx};
use super::style::{format, on_worker, Ancestor, CounterStyle, FormatError, FormatOptions, Parent};
use super::template::AncestorKeys;
use super::writer::NumberedValue;
use super::value::CounterValue;

/// The typed name of the counter table, `flm.counters`, whose value is a
/// [`Counters`].
///
/// The numbering stage writes the table. The lowering rules that show numbers
/// declare an optional read of it, so that they still run, without numbers,
/// in a registry without the numbering feature.
pub const COUNTERS: TableName<Counters> = TableName::new("flm.counters");

/// What the numbering stage recorded for one numbered reference point: the
/// counter the point counts in, that counter's value at the point, the
/// counters it is numbered within with their own values, and the counter
/// style in which its number is shown.
///
/// The record holds no text. Everything a reader sees is built from it when it
/// is shown, so the same record serves every backend and every reference to
/// the point.
///
/// The ancestors are the whole chain, outermost first, in the order
/// [`Parent`] requires: the ancestors of `ancestors[i]` are `ancestors[..i]`.
/// For a counter with several possible parents, the chain is the one that
/// applied at the point: the parent advanced most recently, preceded by its
/// own ancestors.
///
/// A point counted in a **scoped instance** of its counter (an independent
/// sequence of the counter's values attached to an enclosing node, such as a
/// list environment) records that instance
/// ([`instance`](CounterRecord::instance)). Where the instance is nested in
/// another instance of the same counter, the record's last ancestor is the
/// enclosing instance's value at the point, which records its own instance,
/// and the number is written with the counter's nesting joiner
/// ([`CounterDecl::nesting`](super::CounterDecl::nesting)) instead of the
/// counter's within-joiner; the ancestors before it are the enclosing
/// instance's own, down to the counters within which the outermost instance
/// is numbered. A record is *nested* exactly when its last ancestor is of its
/// own counter, since no counter is otherwise numbered within itself.
#[derive(Clone, Debug)]
pub struct CounterRecord {
    /// The counter the point counts in.
    pub counter: Name,
    /// The counter's value at the point.
    pub value: CounterValue,
    /// The counters this one is numbered within, outermost first, each with
    /// its value at the point, the counter style it is written in and its own
    /// within-joiner.
    pub ancestors: Vec<Ancestor>,
    /// The counter style in which the point's own number is shown: the
    /// counter style its number source gives, or, for a point that carries a
    /// number template of its own, that counter style with its number format
    /// replaced by the template
    /// ([`CounterStyle::with_number_format`](super::CounterStyle::with_number_format)).
    pub style: Arc<CounterStyle>,
    /// The scoped instance the point was counted in, as the address of the
    /// node that instance is attached to (the last of the point's
    /// [instances](crate::model::aspects::NumberSource::instances)), or `None`
    /// for a point counted in the counter's single sequence through the
    /// document.
    pub instance: Option<NodeAddr>,
}

impl CounterRecord {
    /// Returns whether the point was counted in a scoped instance nested in
    /// another instance of the same counter: its last ancestor is then of its
    /// own counter, and its number is written with the counter's nesting
    /// joiner.
    pub fn is_nested(&self) -> bool {
        is_nested(&self.counter, &self.ancestors)
    }
}

/// Returns whether a value of `counter` whose ancestors are `ancestors` was
/// counted in a nested scoped instance: its last ancestor is of the same
/// counter. The numbering stage, which chooses each ancestor's joiner, and the
/// table, which chooses a record's joiner, both use this test.
pub(super) fn is_nested(counter: &Name, ancestors: &[Ancestor]) -> bool {
    ancestors.last().is_some_and(|last| &last.counter == counter)
}

/// One stretch of a number as the counter table writes it: a small tree of
/// content, and the reference point it links to.
///
/// It is a [`Segment`](super::Segment) whose link mark has been replaced by
/// the reference point at that position. A lowering rule splices the content
/// with [`Lowerer::splice`](crate::core::lowering::Lowerer::splice) and builds
/// the link from the point, for instance through the anchor the point was
/// given ([`ANCHORS`](crate::stdcore::features::refs::ANCHORS)). A segment
/// that links to no point is a text between two numbers, or a word or
/// delimiter that the counter style keeps out of the link.
#[derive(Clone, Debug)]
pub struct NumberSegment {
    content: Arc<NodeTree<Flm>>,
    link: Option<PointId>,
}

impl NumberSegment {
    /// A segment of the content `content` (a synthesized value whose root is a
    /// list node over the content's nodes), linking to the reference point
    /// `link`, or to none.
    pub fn new(content: Arc<NodeTree<Flm>>, link: Option<PointId>) -> NumberSegment {
        NumberSegment { content, link }
    }

    /// Returns the segment's content, a synthesized value whose root is a
    /// list node over the content's nodes ([synthesized
    /// values](crate::guide::concepts_overview#synthesized-values)).
    pub fn content(&self) -> &Arc<NodeTree<Flm>> {
        &self.content
    }

    /// Returns the reference point this segment links to, or `None` when the
    /// segment is not part of a link.
    pub fn link(&self) -> Option<PointId> {
        self.link
    }
}

/// The numbers of several reference points, written in one group per
/// counter style ([`Counters::format_many`]), and the points that have no
/// number.
///
/// A group is what the [number writer](super::NumberWriter) of one counter
/// style wrote for the points shown in that counter style, such as
/// `Eqs.~(1 and 2)`, and the groups are in the order in which their counter
/// styles first occur among the points. The table does not join the groups:
/// the caller writes them as the items of its own list, with texts of its own
/// between them. The points without a number keep the caller's order; only
/// the caller knows what an unnumbered point shows (its content, or the label
/// it was given), so the table returns the points instead of writing them.
#[derive(Clone, Debug, Default)]
pub struct ManyNumbers {
    /// Per counter style, in the order of first occurrence, the segments of
    /// its numbers, each linking to its own point or to nothing.
    pub groups: Vec<Vec<NumberSegment>>,
    /// The points the table holds no number for, in the caller's order.
    pub unnumbered: Vec<PointId>,
}

/// The counter table, `flm.counters`: every reference point the numbering
/// stage read, each with its record ([`CounterRecord`]) or with nothing for a
/// point that is not numbered.
///
/// A point that the table does not hold at all is a point the stage never
/// read, such as a point inside a synthesized value, which is not part of the
/// document; this differs from a point the stage read and did not number.
/// [`Counters::lookup`] tells the two apart, as
/// [`Anchors::lookup`](crate::stdcore::features::refs::Anchors::lookup) does
/// for anchors.
///
/// The table also carries the compiled counter styles and within-joiners with
/// which the numbers are written (the counter styles in the records, the
/// within-joiners per counter), so that a lowering rule writes a number from
/// the table alone, without the registry. It also carries every counter style
/// the registry declares, compiled, with the host's replacements applied,
/// whether or not a record of this document uses it: [`Counters::style`]
/// returns one by name, so that a rule may show a point's number in a declared
/// counter style other than its record's, through
/// [`Counters::format_in_style`].
///
/// The numbering stage builds the table with [`Counters::insert`],
/// [`Counters::number_within`] and [`Counters::number_nested`]; a caller that
/// builds a table of its own, for instance in a test, uses the same methods.
#[derive(Debug, Default)]
pub struct Counters {
    /// From a point to its record; `None` for a point the stage read and did
    /// not number.
    records: BTreeMap<PointId, Option<CounterRecord>>,
    /// Per counter, the within-joiner that writes its ancestors' numbers
    /// before its own, as the hierarchy of this run made it. A counter
    /// numbered within nothing has no entry.
    hierarchy: BTreeMap<Name, Arc<dyn CompiledWithinJoiner>>,
    /// Per counter, the nesting joiner, which writes the enclosing instance's
    /// number before the number of a point of a nested scoped instance.
    nesting: BTreeMap<Name, Arc<dyn CompiledWithinJoiner>>,
    /// Every declared counter style, compiled, by name, with the host's
    /// replacements applied: a single map per registry, shared by the tables
    /// of every run instead of being copied into each.
    styles: Arc<BTreeMap<Name, Arc<CounterStyle>>>,
}

impl Counters {
    /// Records `record` for `point`; `None` records that the point was read
    /// and not numbered. A record already held for `point` is replaced.
    pub fn insert(&mut self, point: PointId, record: Option<CounterRecord>) {
        self.records.insert(point, record);
    }

    /// Records `joiner` as the within-joiner that writes `counter`'s ancestors'
    /// numbers before its number.
    pub fn number_within(
        &mut self,
        counter: impl Into<Name>,
        joiner: Arc<dyn CompiledWithinJoiner>,
    ) {
        self.hierarchy.insert(counter.into(), joiner);
    }

    /// Records `joiner` as the nesting joiner of `counter`, which writes the
    /// enclosing instance's number before the number of a point of a nested
    /// scoped instance of `counter`.
    pub fn number_nested(
        &mut self,
        counter: impl Into<Name>,
        joiner: Arc<dyn CompiledWithinJoiner>,
    ) {
        self.nesting.insert(counter.into(), joiner);
    }

    /// Makes the table carry `styles`, every declared counter style compiled,
    /// which [`Counters::style`] reads.
    pub(super) fn carry_styles(&mut self, styles: Arc<BTreeMap<Name, Arc<CounterStyle>>>) {
        self.styles = styles;
    }

    /// Returns the compiled counter style declared under `name`, or `None`
    /// when no feature declares a counter style of that name.
    ///
    /// Every counter style a feature declares is returned, whether or not a
    /// record of this document uses it, and where the host replaced the
    /// declaration through [`NumberingConfig`](super::NumberingConfig), the
    /// result is the replacement. A counter style derived for a point's own
    /// number template is not declared and is not returned here; the point's
    /// record holds it ([`CounterRecord::style`]).
    ///
    /// A lowering rule shows a point's number in a declared counter style
    /// other than its record's (for instance a mark written in a counter style
    /// of its own, beside the counter style that references use) by passing
    /// the result to [`Counters::format_in_style`], without the registry. The
    /// counter style must declare at least as many sub-levels as the point's
    /// value carries, or the writing fails with
    /// [`FormatError::UndeclaredSubLevel`](super::FormatError::UndeclaredSubLevel).
    ///
    /// A table that the numbering stage did not write (a default value)
    /// carries no counter style and returns `None` for every name.
    pub fn style(&self, name: &Name) -> Option<&Arc<CounterStyle>> {
        self.styles.get(name)
    }

    /// Returns the record of `point`, or `None` both for a point the stage
    /// never read and for one it did not number. A caller that must tell the
    /// two apart calls [`Counters::lookup`].
    pub fn record(&self, point: PointId) -> Option<&CounterRecord> {
        self.records.get(&point).and_then(Option::as_ref)
    }

    /// Returns what the table holds for `point`: `None` when the stage never
    /// read it, `Some(None)` when it read it and gave it no number, and
    /// `Some(Some(record))` when it numbered it.
    pub fn lookup(&self, point: PointId) -> Option<Option<&CounterRecord>> {
        self.records.get(&point).map(Option::as_ref)
    }

    /// Returns how many points the table holds, numbered or not.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns whether the table holds no point at all.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns every point the table holds, in the order of point
    /// identifiers, with its record or `None`.
    pub fn points(&self) -> impl Iterator<Item = (PointId, Option<&CounterRecord>)> + '_ {
        self.records.iter().map(|(point, record)| (*point, record.as_ref()))
    }

    /// Returns the part of `record`'s number that comes from the counters it
    /// is numbered within, or `None` for a counter numbered within nothing.
    // The joiner belongs to the counter and to the run's options, not to the
    // point, so it is held once per counter here rather than in every record.
    // See decision rationale at [§flm-dd-dr:std-numbering-table-hierarchy].
    fn parent_of<'a>(&'a self, record: &'a CounterRecord) -> Option<Parent<'a>> {
        let joiner = self.joiner_of(record)?;
        Some(Parent::new(joiner.as_ref(), &record.ancestors))
    }

    /// Returns the joiner that writes what comes before `record`'s own number:
    /// the counter's nesting joiner for a point of a nested scoped instance,
    /// else the counter's within-joiner, or `None` for a counter numbered
    /// within nothing.
    fn joiner_of(&self, record: &CounterRecord) -> Option<&Arc<dyn CompiledWithinJoiner>> {
        if record.is_nested() {
            self.nesting.get(&record.counter)
        } else {
            self.hierarchy.get(&record.counter)
        }
    }

    /// Returns the number of `point`, written in the counter style its record
    /// holds: `None` when the table holds no number for the point, and
    /// otherwise the segments of the number, each linking to `point` or to
    /// nothing.
    ///
    /// `options` says which variant of the prefix words to use, and whether
    /// the word before the number and the delimiters around it are written.
    ///
    /// # Errors
    ///
    /// [`FormatError`] when the record and its counter style do not fit each
    /// other, or when the content could not be built. The numbering stage
    /// refuses a record that its counter style cannot write, so such an error
    /// reveals a fault in the program, not in a document.
    pub fn format(
        &self,
        point: PointId,
        options: &FormatOptions,
    ) -> Option<Result<Vec<NumberSegment>, FormatError>> {
        let record = self.record(point)?;
        let parent = self.parent_of(record);
        Some(
            format(&record.value, &record.style, parent, options)
                .map(|segments| link_all(segments, &[point])),
        )
    }

    /// Returns the number of `point` written in `style` instead of the
    /// counter style its record holds: the recorded value, with `style`'s
    /// number format, words and delimiters, preceded by the parts of the
    /// counters it is numbered within, each written as the record holds it, in
    /// its own counter style. Returns `None` when the table holds no number
    /// for the point.
    ///
    /// A rule uses it to show a point's number in another form than the one a
    /// reference to the point shows, without a second record. `style` is one
    /// of two kinds:
    ///
    /// - a counter style derived from the record's own
    ///   ([`CounterStyle::with_number_format`](super::CounterStyle::with_number_format)),
    ///   which shares the sub-levels, words and number writer the record was
    ///   checked against. The lists feature writes the label beside a numbered
    ///   item in this way, through the list's label template, while the
    ///   record's counter style (from the list's reference template, its
    ///   `ref=`, see [`lists`](crate::stdcore::features::lists)) serves every
    ///   reference to the item;
    /// - a declared counter style, looked up by its name with
    ///   [`Counters::style`], for instance for a mark written in a counter
    ///   style of its own.
    ///
    /// # Errors
    ///
    /// The same as [`Counters::format`]; a `style` that is not derived from
    /// the record's own may also declare fewer sub-levels than the value
    /// carries ([`FormatError::UndeclaredSubLevel`](super::FormatError::UndeclaredSubLevel)).
    pub fn format_in_style(
        &self,
        point: PointId,
        style: &CounterStyle,
        options: &FormatOptions,
    ) -> Option<Result<Vec<NumberSegment>, FormatError>> {
        let record = self.record(point)?;
        let parent = self.parent_of(record);
        Some(format(&record.value, style, parent, options).map(|segments| link_all(segments, &[point])))
    }

    /// Returns the number of `point` as a placeholder carrying `keys` writes
    /// it: with no key, what [`format`](Counters::format) writes with
    /// `options`; with keys, the point's own number in the format the keys
    /// give, preceded by the parts of the counters it is numbered within
    /// unless the keys say `bare`, as a within template's `#[…]{counter}`
    /// writes an ancestor (the word, the delimiters and the counter style's
    /// number writer are then not used). Returns `None` when the table holds no
    /// number for the point.
    ///
    /// # Errors
    ///
    /// The same as [`Counters::format`].
    pub(crate) fn format_with_keys(
        &self,
        point: PointId,
        keys: AncestorKeys,
        options: &FormatOptions,
    ) -> Option<Result<Vec<NumberSegment>, FormatError>> {
        if keys == AncestorKeys::default() {
            return self.format(point, options);
        }
        let record = self.record(point)?;
        // The point is written as the last counter of its own chain, with its
        // own joiner, so that the keys are read by the one function that
        // reads them for a within template.
        let mut own = Ancestor::new(
            record.counter.clone(),
            record.value.clone(),
            Arc::clone(&record.style),
        );
        own.joiner = self.joiner_of(record).cloned();
        own.instance = record.instance;
        let mut chain = record.ancestors.clone();
        chain.push(own);
        let written = on_worker(|| {
            let cx = JoinCx::new(&record.style);
            written_with_keys(&chain, chain.len() - 1, keys, &cx)
        });
        Some(written.map(|part| vec![NumberSegment { content: part.content, link: Some(point) }]))
    }

    /// Returns the numbers of several points, written in one group per
    /// counter style: `Eqs.~(1–3, 7, and 9)` for the points shown in one
    /// counter style, `Thm. 1.1` for those of another.
    ///
    /// The points are grouped by the counter style they are shown in, each
    /// group keeping the place of its first point, and each group is written
    /// in one call of [`format_many`](super::format_many), with every point's
    /// own ancestors. The counter style's [number writer](super::NumberWriter)
    /// then decides which numbers of a group are written together and which
    /// collapse into a range; the default writer groups the values by the
    /// counters they are numbered within and never writes a range across two
    /// such groups. The groups are returned separately, in
    /// [`ManyNumbers::groups`], and the caller joins them. A point for which
    /// the table holds no number is not written and is listed in
    /// [`ManyNumbers::unnumbered`].
    ///
    /// Two points are in one group when their records hold the same compiled
    /// counter style, compared by identity, not merely counter styles of the
    /// same name. In a table
    /// that the numbering stage wrote, a record holds either the compiled
    /// counter style that the registry build made for its counter style's
    /// name, or, for a point with a number template of its own
    /// ([`NumberSource::number_format`](crate::model::aspects::NumberSource::number_format)),
    /// the counter style derived from it with that template, which the stage
    /// makes once per template text, counter style and run: the points of one
    /// template are grouped together, and apart from the points written in the
    /// declared counter style.
    ///
    /// With no numbered point at all, the result holds no group, and not the
    /// counter style's text for an empty list: with no counter style there is
    /// no such text to write.
    ///
    /// # Errors
    ///
    /// The same as [`Counters::format`].
    pub fn format_many(
        &self,
        points: &[PointId],
        options: &FormatOptions,
    ) -> Result<ManyNumbers, FormatError> {
        let mut unnumbered = Vec::new();
        // The grouping keeps the place of the first point that fell into it,
        // which is why it is a list searched through rather than a map: a
        // style is told from another by identity.
        let mut groups: Vec<StyleGroup<'_>> = Vec::new();
        for point in points {
            let Some(record) = self.record(*point) else {
                unnumbered.push(*point);
                continue;
            };
            let style = match groups
                .iter()
                .position(|group| Arc::ptr_eq(group.style, &record.style))
            {
                Some(index) => index,
                None => {
                    groups.push(StyleGroup {
                        style: &record.style,
                        values: Vec::new(),
                        points: Vec::new(),
                    });
                    groups.len() - 1
                }
            };
            let mut value = NumberedValue::new(&record.value);
            if let Some(parent) = self.parent_of(record) {
                value = value.within(parent);
            }
            if let Some(instance) = record.instance {
                value = value.in_instance(instance);
            }
            groups[style].values.push(value);
            groups[style].points.push(*point);
        }

        let mut written = Vec::with_capacity(groups.len());
        for group in &groups {
            let segments = format_many(group.style, &group.values, options)?;
            written.push(link_all(segments, &group.points));
        }
        Ok(ManyNumbers { groups: written, unnumbered })
    }
}

/// The points of one counter style, in the order of their first occurrence:
/// the values that one call of the library's `format_many` writes, with the
/// point of each value.
struct StyleGroup<'a> {
    style: &'a Arc<CounterStyle>,
    values: Vec<NumberedValue<'a>>,
    points: Vec<PointId>,
}

/// Returns the segments with their link marks replaced by points: a mark is a
/// position among the values that were written, and `points` holds the point
/// of each of those values, in the same order.
fn link_all(segments: Vec<super::Segment>, points: &[PointId]) -> Vec<NumberSegment> {
    segments
        .into_iter()
        .map(|segment| {
            let link = segment.link().and_then(|position| points.get(position).copied());
            NumberSegment { content: Arc::clone(segment.content()), link }
        })
        .collect()
}

impl Table for Counters {
    fn absorb(&mut self, piece: Counters) {
        self.records.extend(piece.records);
        self.hierarchy.extend(piece.hierarchy);
        self.nesting.extend(piece.nesting);
        // The styles are the registry's, the same map for every piece the
        // stage writes, so a piece carrying them is taken as it is. No merge
        // is needed: the stage writes the table whole, as one piece.
        if !piece.styles.is_empty() {
            self.styles = piece.styles;
        }
    }

    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::Map(
            self.records
                .iter()
                .map(|(point, record)| {
                    let value = match record {
                        Some(record) => {
                            let mut entries = vec![
                                (
                                    "counter".to_string(),
                                    SerialValue::Str(record.counter.to_string()),
                                ),
                                ("value".to_string(), components(&record.value)),
                                (
                                    "style".to_string(),
                                    SerialValue::Str(record.style.name().to_string()),
                                ),
                            ];
                            push_instance(cx, &mut entries, record.instance);
                            let within = record
                                .ancestors
                                .iter()
                                .map(|ancestor| {
                                    let mut entries = vec![
                                        (
                                            "counter".to_string(),
                                            SerialValue::Str(ancestor.counter.to_string()),
                                        ),
                                        ("value".to_string(), components(&ancestor.value)),
                                    ];
                                    push_instance(cx, &mut entries, ancestor.instance);
                                    SerialValue::Map(entries)
                                })
                                .collect();
                            entries.push(("within".to_string(), SerialValue::List(within)));
                            SerialValue::Map(entries)
                        }
                        None => SerialValue::Null,
                    };
                    (point_key(cx, *point), value)
                })
                .collect(),
        )
    }

    fn piece_class(&self) -> PieceClass {
        // The styles are the registry's, not a fact about the document, and
        // they are left out of the projection for the same reason: a table
        // holding nothing else holds nothing of the document.
        if self.records.is_empty() && self.hierarchy.is_empty() && self.nesting.is_empty() {
            PieceClass::Empty
        } else {
            PieceClass::DocumentFact
        }
    }
}

impl SingletonTable for Counters {
    const NAME: &'static str = "flm.counters";
}

/// Adds the key `instance`, the node the scoped instance is attached to, for a
/// value counted in a scoped instance; a value of a counter's single sequence
/// through the document has no such key in the projection.
fn push_instance(
    cx: &dyn ProjectCx,
    entries: &mut Vec<(String, SerialValue)>,
    instance: Option<NodeAddr>,
) {
    if let Some(instance) = instance {
        entries.push(("instance".to_string(), cx.node_ref(instance)));
    }
}

/// Returns a value's components as a list of integers, for a projection.
fn components(value: &CounterValue) -> SerialValue {
    SerialValue::List(value.components().iter().map(|part| SerialValue::Int(*part)).collect())
}

/// Returns a point written as `<node>#<index>`, the form in which every table
/// about reference points writes a point in its projection.
fn point_key(cx: &dyn ProjectCx, point: PointId) -> String {
    format!("{}#{}", node_key(cx, point.addr), point.index)
}

/// Returns a node address as the projection context writes it.
fn node_key(cx: &dyn ProjectCx, addr: NodeAddr) -> String {
    match cx.node_ref(addr) {
        SerialValue::Str(text) => text,
        other => format!("{other:?}"),
    }
}
