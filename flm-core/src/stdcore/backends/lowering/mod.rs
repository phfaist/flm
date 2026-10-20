//! Lowering: the stage that rewrites each frozen tree into the presentation
//! tree one backend emits, the driver that runs it, and the rules and helpers
//! every backend shares.
//!
//! **Lowering** rewrites each frozen tree into a presentation tree for one
//! backend, by applying the lowering rule selected for each element, in
//! rounds, until only elements the backend writes natively remain; the result
//! is computed data in the table `flm.lowered.<backend>` (see [backends,
//! lowering, and
//! outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)).
//! The frozen trees themselves never change (see [frozen trees and computed
//! data](crate::guide::concepts_overview#frozen-trees-and-computed-data)).
//! After lowering, the backend's emitter writes the output from the lowered
//! trees (for the text backend, [`mod@super::text::emit`]).
//!
//! The terms this module relies on:
//!
//! - A **backend** is the registration bundle for one output format
//!   ([`BackendDef`](crate::core::registry::BackendDef)). Every backend
//!   registers exactly one lowering stage, [`LoweringStage`], a `Compute`
//!   stage of `PerTree` scope named `flm.<backend>.lower`.
//! - A **lowering rule** is a named registry entity written for one element:
//!   it declares what it requires, reads and produces, and implements `Lower`,
//!   which stages (builds into the lowered tree) the element's presentation
//!   for one backend ([`LoweringRule`](crate::core::lowering::LoweringRule)).
//!   The plan selects one rule per element and backend.
//! - A **presentation node** is a node of a presentation element, an element
//!   that never occurs in a source document ([`super::pres`]). A lowered tree
//!   is built from presentation nodes and from nodes of the elements the
//!   backend writes natively: where the frozen tree holds a paragraph, the
//!   lowered tree holds `flm.pres.par`; where it holds a heading,
//!   `flm.pres.block-heading`; where it holds a reference, a `flm.pres.link`
//!   around the reference's displayed content.
//! - A **decoration** is a node whose element carries the
//!   [`Decoration`](super::pres::Decoration) aspect. When a rule stages a
//!   presentation node, the driver moves every decoration out of the node's
//!   content slots into an attached slot of that node (see [decorations,
//!   roles and kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)).
//! - A **piece** is a document-level synthesized value that a backend places
//!   before or after the document's content
//!   ([`Piece`](crate::core::piece::Piece)); [`lower_value`] lowers one, for
//!   the pieces stage ([`super::pieces`]).
//! - A **node region** is one of a node's arguments or one of its slots
//!   ([`NodeRegion`](crate::core::lowering::NodeRegion)). A rule may lower a
//!   node region of another node where its own node is placed, which is how a
//!   [content pointer](ContentPointer) shows content written elsewhere.
//!
//! Every rule works through the [`Lowerer`], the only interface a lowering
//! rule has to the run, which the driver implements: through it the rule
//! lowers parts of its node, stages presentation nodes, splices a synthesized
//! value and lowers a node region of another node.
//!
//! # Module map
//!
//! - [`stage`] — [`LoweringStage`], the per-backend stage. The needs it
//!   declares when the plan is built are the union of the selected rules'
//!   declarations ([`rules_needs`], shared with every stage that lowers for a
//!   backend), and the number of rounds it allows is [`rank`].
//! - [`driver`] — [`lower_tree`], which runs the rounds over one tree;
//!   [`lower_value`], which runs them over a synthesized value, such as a
//!   piece, after bringing it up to date; the implementation of the
//!   [`Lowerer`] (lowering of a node's parts, staging of presentation nodes
//!   with their provenance, decorations moved into attached slots, splicing a
//!   synthesized value, and the checks that a rule reads only what it
//!   declared). Also the helpers a rule calls: [`copy_subtree_verbatim`] and
//!   [`copy_run_verbatim`] for opaque content; [`read_verbatim_content`] and
//!   [`stage_verbatim_content`], through which every rule that stages
//!   verbatim content reads it as characters, reporting content that is not
//!   characters ([`ContentNotCharacters`]); [`lower_run`], which hands a run
//!   of sibling nodes back to the ordinary rules; and [`lower_region_content`],
//!   the plain shape of a node region lowered through
//!   [`Lowerer::lower_region`](crate::core::lowering::Lowerer::lower_region).
//! - [`table`] — [`LoweredTrees`], the `flm.lowered.<backend>` table, and
//!   [`LoweredAddr`], the address of a node inside a lowered tree.
//! - [`rules`] — the default lowering rules of the structural elements and of
//!   the field element, and the helpers that stage paragraphs and trimmed
//!   inline content for other rules.
//! - [`mod@pointer`] — [`ContentPointer`], the payload of a synthesized node
//!   that shows content written elsewhere without copying it, and
//!   [`lower_content_pointer`], the lowering every such node's rule calls,
//!   with a check supplied by the rule's own feature.
//! - [`error`] — the `flm.lowering.*` conditions and
//!   `flm.verbatim.content-not-characters`, and the recovery after each.
//! - [`mod@feature`] — the `lowering` feature, which registers the
//!   presentation vocabulary, the field element, the default rules, the two
//!   native-set implications the driver relies on ([`native_implications`])
//!   and two service slots.
//!
//! [`Lowerer`]: crate::core::lowering::Lowerer

pub mod driver;
pub mod error;
pub mod feature;
pub mod pointer;
pub mod rules;
pub mod stage;
pub mod table;

pub use driver::{
    LoweringInputs, LoweringOutcome, VerbatimContent, copy_run_verbatim, copy_subtree_verbatim,
    lower_region_content, lower_run, lower_tree, lower_value, non_native, read_verbatim_content,
    stage_verbatim_content,
};
pub(crate) use driver::lower_written_pieces;
pub use error::{
    ContentNotCharacters, FieldNotRead, LoweringFailed, MathUnprocessed, NestingTooDeep, NoRule,
    RegionNotFound, RoundsExceeded, RuleFailed, UnsupportedElement,
};
pub use feature::{
    FEATURE_NAME, LoweringFeature, NAMER_SLOT, REGIONS_SLOT, feature, native_implications,
};
pub use pointer::{ContentPointer, PointerCheck, lower_content_pointer};
pub use rules::{
    BLOCKS_STRUCTURED, FIELD_RULE, FieldRule, MATH_RULE, MathGroupRule, PARAGRAPH_RULE,
    ParagraphRule, UNKNOWN_ENVIRONMENT_RULE, UNKNOWN_MACRO_RULE, UnknownEnvironmentRule,
    UnknownMacroRule, VERBATIM_RULE, VerbatimRule, default_rules, is_blank, lower_arg_named_or_nothing,
    lower_arg_or_nothing, lower_blocks_with_edges, lower_paragraph, stage_trimmed, stage_trimmed_runs,
};
pub use stage::{LoweringStage, rank, rules_needs, selection_requires};
pub use table::{LoweredAddr, LoweredTree, LoweredTrees, lowered_table, lowered_table_name};
