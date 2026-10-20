//! The number writer, which turns counter values into the content a reader
//! sees, and the context it writes with.
//!
//! A counter style says what a number is made of: the word before it, the
//! delimiters, the number format, the texts that join several numbers. The
//! **number writer** puts those texts and numbers together: it receives the values to
//! write and appends the content they are shown as. One writer is built in,
//! the **default writer**, which writes what the counter style's data
//! describes; a feature may register another writer by name, and a counter
//! style may ask for it
//! ([`CounterStyleDecl::with_number_writer`](super::CounterStyleDecl::with_number_writer)).
//!
//! A number writer exists in two forms, as a declaration and as a compiled
//! value, as a [within-joiner](super::WithinJoiner) does. [`NumberWriter`] is
//! the declaration: a name, and a method that compiles the writer once the
//! language is built, when the writer parses the markup of its own texts.
//! [`CompiledNumberWriter`] is the compiled writer, with two entry points: one
//! value, and several values of one counter style.
//!
//! For each value, a writer receives a [`NumberedValue`] (the counter value
//! and the counters it is numbered within), together with the caller's
//! [`FormatOptions`] and a [`WriteCx`], the writing context that holds the
//! counter style's data and the library's operations. The writer appends what
//! it writes to a [`NumberOut`], item by item, each item marked with the
//! value it links to or with none; the library merges neighboring items of
//! one link into the [`Segment`](super::Segment)s that the caller receives.
//!
//! The parts written for the counters a number is numbered within are
//! computed **on demand**: [`WriteCx::parts`] runs the value's within-joiner
//! and returns its [`Part`]s. A writer that arranges the ancestors itself
//! never calls it, and the within-joiner is then never run. The parts of one
//! list of ancestors are computed once per call and kept for the rest of it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

use techy::core::node::NodeTree;

use crate::lang::{BuiltLang, Flm};
use crate::model::addr::NodeAddr;
use crate::model::names::Name;

use super::formats::NumberFormat;
use super::joiner::{JoinCx, Part};
use super::many::default_many;
use super::style::{
    default_one, number_tree, parse_text, Ancestor, CounterStyle, FormatError, FormatOptions,
    Item, Mark, Parent, Piece, StyleError,
};
use super::value::CounterValue;

// --- what a writer is asked to write ------------------------------------------------------

/// One value that a number writer is asked to write: the counter value, the
/// counters it is numbered within, the scoped instance it was counted in if
/// any, and its position among the values the caller passed in.
///
/// The position is what the link mark of a written item carries: `0` for the
/// only value of [`format`](super::format), and the position in the caller's
/// list for [`format_many`](super::format_many). The library sets it; a caller
/// builds a value with [`new`](NumberedValue::new) and, where the counter is
/// numbered within others, [`within`](NumberedValue::within).
#[derive(Clone, Copy, Debug)]
pub struct NumberedValue<'a> {
    value: &'a CounterValue,
    parent: Option<Parent<'a>>,
    instance: Option<NodeAddr>,
    index: usize,
}

impl<'a> NumberedValue<'a> {
    /// Returns `value`, numbered within nothing and counted in no scoped
    /// instance.
    pub fn new(value: &'a CounterValue) -> NumberedValue<'a> {
        NumberedValue { value, parent: None, instance: None, index: 0 }
    }

    /// Builder: this value is numbered within `parent`, which holds the
    /// within-joiner that writes the ancestors' numbers and the ancestors
    /// themselves.
    pub fn within(mut self, parent: Parent<'a>) -> NumberedValue<'a> {
        self.parent = Some(parent);
        self
    }

    /// Builder: this value was counted in the scoped instance attached to the
    /// node `instance` (see
    /// [`NumberSource::instances`](crate::model::aspects::NumberSource::instances)).
    ///
    /// Values of two different instances belong to two different sequences of
    /// the counter, so the default writer never groups them together,
    /// regardless of their ancestors: the first items of two sibling lists
    /// are `1` and `1`, never a range.
    pub fn in_instance(mut self, instance: NodeAddr) -> NumberedValue<'a> {
        self.instance = Some(instance);
        self
    }

    /// Returns the scoped instance this value was counted in, or `None` for a
    /// value of the counter's single sequence through the document.
    pub fn instance(&self) -> Option<NodeAddr> {
        self.instance
    }

    /// Returns the counter value itself.
    pub fn value(&self) -> &'a CounterValue {
        self.value
    }

    /// Returns the counters this value is numbered within, outermost first,
    /// or an empty list for a counter numbered within nothing.
    pub fn ancestors(&self) -> &'a [Ancestor] {
        match &self.parent {
            Some(parent) => parent.ancestors,
            None => &[],
        }
    }

    /// Returns the position of this value among the values the caller passed
    /// in, which a link mark carries.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Returns the within-joiner and the ancestors, as the library recorded
    /// them.
    pub(super) fn parent(&self) -> Option<Parent<'a>> {
        self.parent
    }

    /// Returns this value at position `index` among the caller's values.
    pub(super) fn at(mut self, index: usize) -> NumberedValue<'a> {
        self.index = index;
        self
    }
}

// --- what a writer writes into ------------------------------------------------------------

/// The output to which a number writer appends what it writes.
///
/// A writer appends content ([`content`](NumberOut::content)) or characters
/// ([`text`](NumberOut::text)), each marked with the position of the value it
/// links to, or with `None` for an item that is part of no link. The library
/// merges neighboring items that carry the same mark and turns each merged
/// sequence into one [`Segment`](super::Segment); an item that holds no
/// content and links to nothing is left out.
pub struct NumberOut<'a> {
    items: Vec<Item<'a>>,
}

impl<'a> NumberOut<'a> {
    /// Returns an empty output.
    pub(super) fn new() -> NumberOut<'a> {
        NumberOut { items: Vec::new() }
    }

    /// Appends `content`, linking to the value at `link` among the caller's
    /// values, or to nothing.
    pub fn content(&mut self, content: Arc<NodeTree<Flm>>, link: Option<usize>) {
        self.items.push(Item { pieces: vec![Piece::Tree(content)], mark: mark_of(link) });
    }

    /// Appends `text` as characters, linking to the value at `link` among the
    /// caller's values, or to nothing.
    ///
    /// The characters become one text node when the result is assembled;
    /// [`WriteCx::literal`] builds the same content for a writer that needs
    /// the content itself.
    pub fn text(&mut self, text: &str, link: Option<usize>) {
        self.items.push(Item { pieces: vec![Piece::Digits(text.to_string())], mark: mark_of(link) });
    }

    /// Appends one item of the library's own, which may carry a mark that
    /// joins it to the link of its neighbors, if any.
    pub(super) fn item(&mut self, item: Item<'a>) {
        self.items.push(item);
    }

    /// Appends several items of the library's own.
    pub(super) fn items(&mut self, items: impl IntoIterator<Item = Item<'a>>) {
        self.items.extend(items);
    }

    /// Returns everything that was appended, in order.
    pub(super) fn into_items(self) -> Vec<Item<'a>> {
        self.items
    }
}

impl fmt::Debug for NumberOut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NumberOut").field("pieces", &self.items.len()).finish()
    }
}

/// Returns the mark that a caller's link position gives.
fn mark_of(link: Option<usize>) -> Mark {
    match link {
        Some(index) => Mark::Value(index),
        None => Mark::Never,
    }
}

// --- the two traits -----------------------------------------------------------------------

/// A number writer as a feature declares it: its name, and how it compiles
/// itself once the language is built.
///
/// A feature contributes a writer, wrapped in a [`NumberWriterDecl`], to the
/// service slot [`NUMBER_WRITERS_SLOT`](super::NUMBER_WRITERS_SLOT), and a
/// counter style asks for it by name with
/// [`CounterStyleDecl::with_number_writer`](super::CounterStyleDecl::with_number_writer).
/// Two writers of one name fail the registry build, and so does a counter
/// style that asks for a writer no feature registers. The [numbering module
/// documentation](super#the-number-writer) describes what a writer receives
/// and writes.
pub trait NumberWriter: Send + Sync {
    /// Returns the name by which counter styles ask for this writer. It is
    /// dotted and owned by the feature that declares it, such as
    /// `example.numbering.in-section`, as a counter style's name is.
    fn name(&self) -> &Name;

    /// Returns this writer compiled against the built language, which `cx`
    /// holds; every text of the writer's own is parsed here. The library
    /// compiles a writer once per counter style that asks for it, when the
    /// registry is built.
    ///
    /// # Errors
    ///
    /// The [`StyleError`] of a text of the writer's own that is not valid FLM.
    fn compile(&self, cx: &WriterCx<'_>) -> Result<Arc<dyn CompiledNumberWriter>, StyleError>;
}

/// A number writer as it is contributed to the service slot
/// [`NUMBER_WRITERS_SLOT`](super::NUMBER_WRITERS_SLOT): a value that holds the
/// writer.
///
/// A service slot hands out its values as borrowed references, and the
/// numbering feature uses a writer long after the registry build has released
/// them, so the service slot holds this value, of which the numbering feature
/// keeps a copy.
#[derive(Clone)]
pub struct NumberWriterDecl {
    writer: Arc<dyn NumberWriter>,
}

impl NumberWriterDecl {
    /// Returns the declaration of `writer`.
    pub fn new(writer: Arc<dyn NumberWriter>) -> NumberWriterDecl {
        NumberWriterDecl { writer }
    }

    /// Returns the writer it holds.
    pub fn writer(&self) -> &Arc<dyn NumberWriter> {
        &self.writer
    }
}

impl fmt::Debug for NumberWriterDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NumberWriterDecl").field("name", &self.writer.name().as_str()).finish()
    }
}

/// A compiled number writer: what writes the numbers of a counter style.
///
/// The two entry points are the two ways a number is asked for. One value
/// comes through [`format`](super::format), which a construct showing its own
/// number and a reference to one point use. Several values of one counter
/// style come through [`format_many`](super::format_many), which writes a
/// reference to several points: the writer receives all of them, with their
/// ancestors, and decides which of them are written together.
pub trait CompiledNumberWriter: Send + Sync + fmt::Debug {
    /// Writes one value into `out`.
    ///
    /// # Errors
    ///
    /// The [`FormatError`] of any operation the writer asked `cx` for.
    fn write_one<'a>(
        &self,
        value: &NumberedValue<'a>,
        options: &FormatOptions,
        cx: &WriteCx<'a>,
        out: &mut NumberOut<'a>,
    ) -> Result<(), FormatError>;

    /// Writes several values of one counter style into `out`, in the order the
    /// writer chooses.
    ///
    /// The values come in the caller's order, and each carries its own
    /// ancestors. The list may be empty; the default writer then writes the
    /// counter style's text for an empty list ([`JoinText::Empty`]).
    ///
    /// # Errors
    ///
    /// The [`FormatError`] of any operation the writer asked `cx` for.
    fn write_many<'a>(
        &self,
        values: &[NumberedValue<'a>],
        options: &FormatOptions,
        cx: &WriteCx<'a>,
        out: &mut NumberOut<'a>,
    ) -> Result<(), FormatError>;
}

// --- compiling ------------------------------------------------------------------------------

/// The context against which a number writer is compiled: the built language,
/// and the name of the counter style the writer is compiled for.
///
/// A writer is compiled once per counter style that asks for it, when the
/// registry is built, so a text of its own is parsed then and never again
/// while a number is written.
pub struct WriterCx<'a> {
    lang: &'a BuiltLang,
    style: &'a Name,
}

impl<'a> WriterCx<'a> {
    /// Returns the context over `lang`, for the counter style named `style`.
    pub(super) fn new(lang: &'a BuiltLang, style: &'a Name) -> WriterCx<'a> {
        WriterCx { lang, style }
    }

    /// Returns the built language.
    pub fn lang(&self) -> &BuiltLang {
        self.lang
    }

    /// Returns the name of the counter style this writer is being compiled
    /// for.
    pub fn style(&self) -> &Name {
        self.style
    }

    /// Returns `text` parsed as FLM, as every text of a counter style is
    /// parsed. Warnings of the parse are dropped.
    ///
    /// # Errors
    ///
    /// The [`StyleError`] of a text that is not valid FLM.
    pub fn parse(&self, text: &str) -> Result<Arc<NodeTree<Flm>>, StyleError> {
        let mut warnings = Vec::new();
        parse_text(text, self.lang, &mut warnings)
    }
}

// --- the toolbox ------------------------------------------------------------------------------

/// The name of one of the join texts with which a counter style joins
/// several numbers ([`WriteCx::join_text`]).
///
/// The texts are the ones [`JoinDecl`](super::JoinDecl) declares, with its
/// variables already substituted. A writer decides which of them it uses;
/// [`JoinDecl`](super::JoinDecl) describes how the default writer uses them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JoinText {
    /// Before a single run of numbers.
    OnePre,
    /// After a single run of numbers.
    OnePost,
    /// Before two runs.
    PairPre,
    /// Between two runs.
    PairMid,
    /// After two runs.
    PairPost,
    /// Before a run of several numbers.
    RangePre,
    /// Between the ends of a run of three or more.
    RangeMid,
    /// Between the ends of a run of exactly two.
    RangePairMid,
    /// After a run of several numbers.
    RangePost,
    /// Before three runs or more.
    ListPre,
    /// Between the runs of a list, except before the last.
    ListMid,
    /// Before the last run of a list.
    ListMidLast,
    /// After three runs or more.
    ListPost,
    /// The text written when there is no number at all.
    Empty,
}

/// The writing context of a number writer: the counter style's data, the
/// library's operations that build content, the within-joiner on demand, and
/// the default writer to delegate to.
///
/// Everything a reader sees is content, not characters, and a writer builds
/// content only through this context.
pub struct WriteCx<'a> {
    style: &'a CounterStyle,
    /// The parts already written in this call, per chain of ancestors: the
    /// within-joiner of one chain runs once, however many values are written
    /// from it.
    parts: RefCell<BTreeMap<PartsKey, Arc<Vec<Part>>>>,
}

/// The key that tells one chain of ancestors from another within one call:
/// the within-joiner that writes them and the list itself, both by address,
/// since a call writes values from records that do not move while it runs.
///
/// The within-joiner's address here is the address of its data alone, without
/// the table of methods a trait object also carries. Two within-joiners that
/// hold no data of their own therefore have the same address, so the key
/// relies on an assumption: two values whose ancestors are the same list, at
/// the same address, are numbered within it by the same within-joiner. The
/// assumption holds because a record holds the within-joiner of its counter
/// beside its chain of ancestors, and the records do not move while the call
/// runs.
type PartsKey = (usize, usize, usize);

impl<'a> WriteCx<'a> {
    /// Returns the writing context over `style`, the counter style in which
    /// every value of this call is written.
    pub(super) fn new(style: &'a CounterStyle) -> WriteCx<'a> {
        WriteCx { style, parts: RefCell::new(BTreeMap::new()) }
    }

    /// Returns the counter style in which every value of this call is
    /// written.
    pub fn style(&self) -> &'a CounterStyle {
        self.style
    }

    /// Returns the word written before `count` numbers in the variant
    /// `variant` of the prefix words, or `None` when that word is the empty
    /// text and nothing is written. A variant the counter style does not
    /// declare gives the default words.
    pub fn word(&self, count: usize, variant: Option<&str>) -> Option<&'a Arc<NodeTree<Flm>>> {
        self.style.word_text(count, variant)
    }

    /// Returns the counter style's opening delimiter, the text written before
    /// a number.
    pub fn open(&self) -> &'a Arc<NodeTree<Flm>> {
        self.style.open()
    }

    /// Returns the counter style's closing delimiter, the text written after
    /// a number.
    pub fn close(&self) -> &'a Arc<NodeTree<Flm>> {
        self.style.close()
    }

    /// Returns whether the counter style says that the word written before
    /// several numbers belongs to the link to the first number written. A
    /// writer that writes a word itself marks it with the position of the first
    /// number it writes when this is true, and with none when it is false.
    /// For one value written with [`format`](super::format), the default
    /// writer makes the whole text one link regardless of this setting.
    pub fn name_in_link(&self) -> bool {
        self.style.name_in_link()
    }

    /// Returns whether the counter style says that the end of a range repeats
    /// the parts that come from the counters the value is numbered within:
    /// `A.I–A.III` instead of `A.I–III`. A writer that builds a range itself
    /// decides with this whether to write the end's parts again.
    pub fn repeat_parents_in_range(&self) -> bool {
        self.style.repeat_parents_in_range()
    }

    /// Returns whether the counter style says that consecutive numbers
    /// collapse into a range (`1–3`) instead of each being written separately
    /// (`1,2,3`). A writer that builds ranges itself decides with this whether
    /// to build any.
    pub fn ranges(&self) -> bool {
        self.style.ranges()
    }

    /// Returns the counter style's join text `which`.
    pub fn join_text(&self, which: JoinText) -> &'a Arc<NodeTree<Flm>> {
        self.style.join().text(which)
    }

    /// Returns `value` written with the counter style's own number template,
    /// with its sub-numbers: the number alone, with no word, no delimiters and
    /// nothing of the counters it is numbered within.
    ///
    /// # Errors
    ///
    /// [`FormatError::UndeclaredSubLevel`] when the value carries a sub-number
    /// at a level the counter style declares no format for, and a failure to
    /// build the content.
    pub fn number(&self, value: &CounterValue) -> Result<Arc<NodeTree<Flm>>, FormatError> {
        number_tree(self.style, value, None)
    }

    /// Returns `value` written in `format` instead of with the counter style's
    /// own number template, its sub-numbers in that format too.
    ///
    /// # Errors
    ///
    /// The same as [`number`](WriteCx::number).
    pub fn number_in(
        &self,
        value: &CounterValue,
        format: NumberFormat,
    ) -> Result<Arc<NodeTree<Flm>>, FormatError> {
        number_tree(self.style, value, Some(format))
    }

    /// Returns `ancestor`'s own number, written in the ancestor's own counter
    /// style: the number alone, with no word, no delimiters and nothing of its
    /// own ancestors.
    ///
    /// # Errors
    ///
    /// The same as [`number`](WriteCx::number), for the ancestor's counter
    /// style.
    pub fn number_of(&self, ancestor: &Ancestor) -> Result<Arc<NodeTree<Flm>>, FormatError> {
        number_tree(&ancestor.style, &ancestor.value, None)
    }

    /// Returns `ancestor`'s own number written in `format` instead of the
    /// format of its own counter style.
    ///
    /// # Errors
    ///
    /// The same as [`number`](WriteCx::number), for the ancestor's counter
    /// style.
    pub fn number_of_in(
        &self,
        ancestor: &Ancestor,
        format: NumberFormat,
    ) -> Result<Arc<NodeTree<Flm>>, FormatError> {
        number_tree(&ancestor.style, &ancestor.value, Some(format))
    }

    /// Returns `text` as a small tree of content, built as the characters of
    /// a number are: one text node attributed to the counter style's source.
    ///
    /// # Errors
    ///
    /// [`FormatError`] when the content could not be built.
    pub fn literal(&self, text: &str) -> Result<Arc<NodeTree<Flm>>, FormatError> {
        self.style.literal(text)
    }

    /// Returns the parts written before `value`'s own number: what its
    /// [within-joiner](super::WithinJoiner) writes of the counters it is
    /// numbered within.
    ///
    /// The within-joiner runs the first time this method is called for a list
    /// of ancestors, and its result is kept for the rest of the call: a writer
    /// that never calls it never runs the within-joiner, and one that calls it
    /// for two values that share one list of ancestors runs it once. A value
    /// numbered within nothing has no part.
    ///
    /// # Errors
    ///
    /// [`FormatError::UnknownAncestor`] when the within-joiner refers to a
    /// counter that is not in the chain, and a failure to build the content.
    pub fn parts(&self, value: &NumberedValue<'_>) -> Result<Vec<Part>, FormatError> {
        let Some(parent) = value.parent() else { return Ok(Vec::new()) };
        let key: PartsKey = (
            std::ptr::from_ref(parent.joiner).cast::<()>() as usize,
            parent.ancestors.as_ptr() as usize,
            parent.ancestors.len(),
        );
        if let Some(written) = self.parts.borrow().get(&key) {
            return Ok(written.as_ref().clone());
        }
        let cx = JoinCx::new(self.style);
        let written = Arc::new(parent.joiner.parts(parent.ancestors, &cx)?);
        self.parts.borrow_mut().insert(key, Arc::clone(&written));
        Ok(written.as_ref().clone())
    }

    /// Writes `value` into `out` as the default writer would: the word, the
    /// delimiters, the parts and the number.
    ///
    /// A writer calls it to add to the default text instead of replacing it:
    /// `Eq.~(5)`, followed by ` in Section~V` of its own.
    ///
    /// # Errors
    ///
    /// The same as [`format`](super::format).
    pub fn default_one(
        &self,
        value: &NumberedValue<'a>,
        options: &FormatOptions,
        out: &mut NumberOut<'a>,
    ) -> Result<(), FormatError> {
        default_one(value, options, self, out)
    }

    /// Writes `values` into `out` as the default writer would: grouped by the
    /// counters they are numbered within, sorted, collapsed into ranges and
    /// joined.
    ///
    /// # Errors
    ///
    /// The same as [`format_many`](super::format_many).
    pub fn default_many(
        &self,
        values: &[NumberedValue<'a>],
        options: &FormatOptions,
        out: &mut NumberOut<'a>,
    ) -> Result<(), FormatError> {
        default_many(values, options, self, out)
    }
}

impl fmt::Debug for WriteCx<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteCx").field("style", &self.style.name().as_str()).finish_non_exhaustive()
    }
}

// --- the default writer -------------------------------------------------------------------

/// The default writer, which writes every counter style that asks for no
/// other writer: it writes what the counter style's data describes and
/// nothing else.
///
/// One value is written as the word, the opening delimiter, the parts the
/// value's within-joiner writes, the number and its sub-numbers, and the
/// closing delimiter. Several values are grouped by the counters they are
/// numbered within, each group sorted and its consecutive values collapsed
/// into ranges, and everything joined by the counter style's join texts. A
/// range never spans two groups; a registered writer may write a range across
/// a parent.
#[derive(Debug)]
pub(super) struct DefaultWriter;

impl CompiledNumberWriter for DefaultWriter {
    fn write_one<'a>(
        &self,
        value: &NumberedValue<'a>,
        options: &FormatOptions,
        cx: &WriteCx<'a>,
        out: &mut NumberOut<'a>,
    ) -> Result<(), FormatError> {
        default_one(value, options, cx, out)
    }

    fn write_many<'a>(
        &self,
        values: &[NumberedValue<'a>],
        options: &FormatOptions,
        cx: &WriteCx<'a>,
        out: &mut NumberOut<'a>,
    ) -> Result<(), FormatError> {
        default_many(values, options, cx, out)
    }
}

/// Returns the default writer, created once and shared by every counter style
/// that asks for no other writer.
pub(super) fn default_writer() -> &'static Arc<dyn CompiledNumberWriter> {
    static WRITER: OnceLock<Arc<dyn CompiledNumberWriter>> = OnceLock::new();
    WRITER.get_or_init(|| Arc::new(DefaultWriter))
}
