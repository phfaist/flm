//! Mirrors [`flm_core::stdcore::features::numbering`]: the names of the
//! numbering feature, the declarations a feature contributes to it as data,
//! and the [`Counters`] façade over the table of the points' numbers.
//!
//! A construct numbered by a counter of its own declares the counter
//! ([`CounterDecl`]) and its counter style ([`CounterStyleDecl`]) through
//! [`FeatureBuilder::peer_slot`](crate::api::core::registry::FeatureBuilder::peer_slot)
//! into [`COUNTERS_SLOT`] and [`STYLES_SLOT`], names them in the number source
//! of its reference points, and reads the number of a point back from the
//! table [`COUNTERS`] (a rule declares the read with `reads_opt(COUNTERS)`).
//! The word a `\ref` writes before the number is the style's
//! [`prefix_words`](CounterStyleDecl::prefix_words).
//!
//! What differs from `flm-core`: the table names are `&str` constants; the
//! declarations are `flm-core`'s own types, re-exported, which a binding
//! writes as data (the shapes are those of the service slots' readers,
//! [`FeatureBuilder::peer_slot`](crate::api::core::registry::FeatureBuilder::peer_slot));
//! the table is read through the [`Counters`] façade, whose record is data
//! and whose number segments are the extension API's nodes.
//!
//! The data forms across a JSON binding: a counter declaration `{ name,
//! default_style, docs?, within?, nesting? }`; a counter style `{ name,
//! number_format?, sub_levels?, prefix_words?, delimiters?, join?,
//! name_in_link?, repeat_parents_in_range?, ranges?, number_writer? }`, each
//! field given in its `flm-core` type's serde form (`prefix_words` is `{
//! base: { one, other }, variants? }`, `number_format` is `{ formats: text }`
//! or `{ initial: text }`); a parent set `{ name, counters: [name] }`; a
//! [`Within`] `{ parents: { one: name } | { any: [name] } | { set: name },
//! joiner, no_restart }`; [`FormatOptions`] `{ variant?, with_word?,
//! with_delimiters? }`; a point's identifier `{ addr: { tree, node }, index
//! }`, the address as a lowerer's `addr()` answers it, of a tree the reader
//! can see.
//!
//! Left out, each with the reason:
//!
//! - `Counters::format_in_style`: it takes a compiled `CounterStyle`; no
//!   construct formats a number in a style derived for it.
//! - `Counters::format_with_keys`: not public in `flm-core`.
//! - `Counters::format_many`, `ManyNumbers`, `format_many`: the `\ref`
//!   rule's list form; a construct formats its own number.
//! - `Counters::style`, `lookup`, `points`, `len`, `is_empty`, `insert`,
//!   `number_within`, `number_nested`: the stage's own writing interface,
//!   and readers no construct needs.
//! - `WithinJoinerDecl`, `NumberWriterDecl` and their traits: each wraps a
//!   trait object, behavior rather than data; their slot names are
//!   mirrored, so that a value given for them meets the slot table's
//!   message.
//! - `NumberingOptions`, `CounterOptions`, `WithinOption`: a document's
//!   options, not a feature's declaration.
//! - `identifier_text`, `CounterValue`, `Ancestor`, `Parent`, `Segment`,
//!   `format`, `parts`, the compiled types and the stage's conditions: the
//!   stage's internals.

use flm_core::model::addr::NodeAddr;
use flm_core::stdcore::features::numbering as core_numbering;

use crate::adapter::tables::PointsCore;
use crate::api::model::aspects::PointId;
use crate::api::techy::node::Node;
use crate::runtime::ExtError;
use crate::sealed::Sealed;

// --- the names ---------------------------------------------------------------------------------

/// Mirrors [`COUNTERS`](flm_core::stdcore::features::numbering::COUNTERS):
/// the numbering feature's table of the points' numbers, `flm.counters`,
/// read through the [`Counters`] façade. What differs: the name alone.
pub const COUNTERS: &str = core_numbering::COUNTERS.name();

/// Mirrors [`COUNTERS_SLOT`](flm_core::stdcore::features::numbering::COUNTERS_SLOT):
/// the service slot of the counters, `flm.numbering.counters`, which takes a
/// [`CounterDecl`]. Nothing differs.
pub const COUNTERS_SLOT: &str = core_numbering::COUNTERS_SLOT;

/// Mirrors [`STYLES_SLOT`](flm_core::stdcore::features::numbering::STYLES_SLOT):
/// the service slot of the counter styles, `flm.numbering.styles`, which
/// takes a [`CounterStyleDecl`]. Nothing differs.
pub const STYLES_SLOT: &str = core_numbering::STYLES_SLOT;

/// Mirrors [`WITHIN_JOINERS_SLOT`](flm_core::stdcore::features::numbering::WITHIN_JOINERS_SLOT):
/// the service slot of the registered within-joiners,
/// `flm.numbering.within-joiners`. What differs: no value an extension
/// gives as data fills it (a within-joiner is behavior).
pub const WITHIN_JOINERS_SLOT: &str = core_numbering::WITHIN_JOINERS_SLOT;

/// Mirrors [`NUMBER_WRITERS_SLOT`](flm_core::stdcore::features::numbering::NUMBER_WRITERS_SLOT):
/// the service slot of the number writers, `flm.numbering.number-writers`.
/// What differs: no value an extension gives as data fills it (a number
/// writer is behavior).
pub const NUMBER_WRITERS_SLOT: &str = core_numbering::NUMBER_WRITERS_SLOT;

/// Mirrors [`PARENT_SETS_SLOT`](flm_core::stdcore::features::numbering::PARENT_SETS_SLOT):
/// the service slot of the parent sets, `flm.numbering.parent-sets`, which
/// takes a [`ParentSetDecl`]. Nothing differs.
pub const PARENT_SETS_SLOT: &str = core_numbering::PARENT_SETS_SLOT;

// --- the declarations --------------------------------------------------------------------------

/// Mirrors the declarations of [`flm_core::stdcore::features::numbering`]:
/// [`CounterDecl`], [`CounterStyleDecl`], [`ParentSetDecl`], the place of a
/// counter ([`Within`], [`Parents`], [`WithinJoinerSpec`]), the words and
/// delimiters of a style ([`PrefixWords`], [`PluralWords`], [`Delimiters`]),
/// its number template ([`NumberTemplate`]) and the options of a number's
/// formatting ([`FormatOptions`]). Nothing differs: they are `flm-core`'s
/// own, re-exported; their data forms are given in the module's text.
pub use flm_core::stdcore::features::numbering::{
    CounterDecl, CounterStyleDecl, Delimiters, FormatOptions, NumberTemplate, ParentSetDecl, Parents, PluralWords,
    PrefixWords, Within, WithinJoinerSpec,
};

// --- the table ---------------------------------------------------------------------------------

/// Mirrors [`CounterRecord`](flm_core::stdcore::features::numbering::CounterRecord):
/// what the table holds for a numbered point. What differs: it is the
/// record's projection as data, as the table's projection writes it: the
/// counter's name, the value's components (the number, then the
/// sub-numbers), the counter style's name, and the address of the node of
/// the scoped instance the point was counted in, if any. The ancestors are
/// not carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterRecord {
    /// The counter the point counts in.
    pub counter: String,
    /// The counter's value at the point: the number, then the sub-numbers.
    pub value: Vec<i64>,
    /// The name of the counter style the point's number is shown in.
    pub style: String,
    /// The node the scoped instance the point was counted in is attached to,
    /// or `None` for the counter's single sequence through the document.
    pub instance: Option<NodeAddr>,
}

/// Mirrors [`NumberSegment`](flm_core::stdcore::features::numbering::NumberSegment):
/// one stretch of a number, its content and the reference point it links
/// to. What differs: the content is the extension API's node, the root of
/// the segment's tree (a list node over the content's nodes), which a rule
/// places with [`Lowerer::splice`](crate::api::core::lowering::Lowerer::splice);
/// the fields are public.
pub struct NumberSegment<'t> {
    /// The root of the segment's content.
    pub content: Box<dyn Node<'t> + 't>,
    /// The reference point the segment links to, or `None`.
    pub link: Option<PointId>,
}

/// Mirrors [`Counters`](flm_core::stdcore::features::numbering::Counters):
/// the table of the points' numbers, as a stage's context or a rule's
/// lowerer answers it for [`COUNTERS`]
/// ([`TableValue::Counters`](crate::api::core::data::table::TableValue::Counters)).
///
/// What differs: the two readers below; the record is data. Across a JSON
/// binding a point is read back from its data form against the trees the
/// reader can see: in a rule, the tree being lowered; in a stage, the trees
/// of the document. The trait is sealed: the adapter alone implements it.
pub trait Counters: Sealed + PointsCore {
    /// Mirrors [`Counters::record`](flm_core::stdcore::features::numbering::Counters::record):
    /// what the table holds for `point`, or `None` for a point the table
    /// holds no number for (an unnumbered point). What differs: the record
    /// is [`CounterRecord`], its projection.
    fn record(&self, point: PointId) -> Option<CounterRecord>;

    /// Mirrors [`Counters::format`](flm_core::stdcore::features::numbering::Counters::format):
    /// the number of `point` in the counter style its record holds, as its
    /// segments, or `None` for a point the table holds no number for.
    /// `options` says which variant of the prefix words to use, and whether
    /// the word and the delimiters are written. What differs: the segments
    /// are [`NumberSegment`]s, whose content each owns its tree, and the
    /// result's order is `Result<Option<…>>`.
    ///
    /// # Errors
    ///
    /// `flm-core`'s `FormatError` ([`ExtError::Core`] of kind `format`).
    fn format<'s>(&self, point: PointId, options: &FormatOptions) -> Result<Option<Vec<NumberSegment<'s>>>, ExtError>;
}

fn _object_safe(_: &dyn Counters) {}
