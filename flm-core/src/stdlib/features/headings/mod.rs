//! This module defines the `headings` feature, which provides the commands that create
//! the headings of a document, `\section` to `\subparagraph` by default. A program can
//! declare its own heading kinds and heading commands in the configuration of the
//! feature, [`HeadingsConfig`].
//!
//! # Writing a heading
//!
//! A heading consists of a heading command, an optional star, an optional key-value
//! argument in square brackets, the title, and optional `\label` commands after the
//! title:
//!
//! ```text
//! \section{Introduction}\label{sec:intro}
//! \subsection*{Notation}
//! \section[toc=false]{Acknowledgments}
//! ```
//!
//! The star and the key-value argument set the options of the heading (see [The options
//! of a heading](#the-options-of-a-heading)). A label written after the title can be
//! referred to with `\ref`. The language guide describes headings for authors
//! ([headings](crate::guide::language_syntax#headings)).
//!
//! # Heading kinds and heading commands
//!
//! A *heading kind* is a type of heading, such as a section, a subsection or an
//! appendix. A heading kind has a name and a *level*. Levels count from 0, and an outer
//! heading has a smaller level than the headings inside it. A heading kind is declared
//! with a [`HeadingDecl`]. This declaration also contains the settings for the numbering
//! of the headings of the kind, for their listing in a table of contents, for run-in
//! headings, and for the labels that the headings accept.
//!
//! A *heading command* is a macro, such as `\section`, that an author writes to create a
//! heading of one heading kind. A heading command is declared with a [`HeadingCommand`],
//! separately from the heading kind, because headings of the same kind can be written
//! with several commands. For instance, the sections written with a second command for
//! the kind `section` can be left out of the table of contents unless the author writes
//! `toc=true`. A heading kind can also have no command at all. A program then creates
//! the headings of this kind itself, for instance in a stage.
//!
//! The heading kinds and the heading commands are part of the configuration of the
//! feature, [`HeadingsConfig`]. A program passes the configuration to [`Headings::new`]
//! or to [`StdlibConfig::headings`](crate::stdlib::StdlibConfig::headings). The
//! configuration is fixed when the registry is built (see [settings, options and
//! configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
//! The default configuration ([`HeadingsConfig::new`]) contains the five heading kinds
//! `section`, `subsection`, `subsubsection`, `paragraph` and `subparagraph` at the
//! levels 1 to 5, as in LaTeX, with one heading command of the same name for each kind.
//!
//! The following example adds a heading kind `appendix` and a second heading command
//! for sections:
//!
//! ```
//! use flm_core::stdlib::features::headings::{HeadingCommand, HeadingDecl, HeadingsConfig};
//! use flm_core::stdlib::features::numbering::NumberTemplate;
//!
//! // The five standard kinds, an `\appendix` numbered A, B, C, and a second
//! // command for sections that are not listed in the table of contents.
//! let mut kinds = HeadingsConfig::new().headings;
//! kinds.push(HeadingDecl::new("appendix", 1).number_format(NumberTemplate::formats("#{Alph}")));
//! let config = HeadingsConfig::new()
//!     .headings(kinds)
//!     .command(HeadingCommand::new("quietsection", "section").default_value("toc", "false"));
//! assert_eq!(config.heading("appendix").map(|kind| kind.level), Some(1));
//! assert_eq!(config.commands_of("section").count(), 2);
//! ```
//!
//! In LaTeX, `\appendix` is a switch and not a sectioning command. A program that
//! declares a heading kind `appendix` therefore maps it to a LaTeX sectioning command for
//! the `latex-recomposed` backend, with the render option
//! [`HeadingsLatexOptions`](latex::HeadingsLatexOptions). The extension guide gives a
//! complete example ([heading kinds and commands of your
//! own](crate::guide::extending#heading-kinds-and-commands-of-your-own)).
//!
//! # The options of a heading
//!
//! A heading has two options, which [`heading_options`] returns as a
//! [`HeadingOccurrence`]:
//!
//! - `numbered`: whether the heading is numbered, provided that its heading kind is
//!   numbered. A star after the command, as in `\section*{…}`, is equivalent to
//!   `numbered=false`.
//! - `toc`: whether the heading is listed in a table of contents. The star does not
//!   change this option: `\section*{…}` is listed, and `\section*[toc=false]{…}` is
//!   neither numbered nor listed. The headings of a heading kind declared with
//!   [`skip_toc`](HeadingDecl::skip_toc) are never listed, and the option `toc` has no
//!   effect on them.
//!
//! Both options are `true` by default. They are set, in this order, by the default
//! options of the heading command ([`HeadingCommand::defaults`]), by the star, and by
//! the key-value argument from left to right. A later setting replaces an earlier one.
//! A key written without a value, as in `[numbered]`, is equivalent to the value `true`.
//!
//! With the key `alt` ([`ALT_KEY`]), an author gives the heading an *alternative
//! title*, as in `\section[alt={Short title}]{A long title with \emph{markup}}`. The
//! value is FLM content, which [`heading_alt`] returns. In a table of contents, the
//! alternative title is displayed in place of the title ([`Sectioning::toc_title`]).
//! The full title is displayed at the heading itself and in the references to the
//! heading, and the anchor of a heading without a label is derived from the full title.
//! The `latex-recomposed` backend passes the alternative title to LaTeX ([`latex`]).
//!
//! The key `pdfbookmark` (the title of a PDF bookmark) is reserved and not implemented
//! ([`RESERVED_KEYS`]).
//!
//! The lowering rule of a heading reports the following problems in the key-value
//! argument:
//!
//! - a reserved key: the warning `flm.heading.key-not-implemented`
//!   ([`KeyNotImplemented`]);
//! - the key `alt` without a value: the error `flm.heading.missing-value`
//!   ([`MissingValue`]);
//! - a key other than `numbered`, `toc`, `alt` and the reserved keys: the error
//!   `flm.heading.unknown-key` ([`UnknownKey`]);
//! - a value of `numbered` or `toc` other than `true` and `false`: the error
//!   `flm.heading.bad-value` ([`BadValue`]);
//! - a key that is not plain text: the error `flm.heading.unreadable-options`
//!   ([`UnreadableOptions`]).
//!
//! In each case, the heading is still written. The key in question is ignored; for the
//! last error, the whole key-value argument is ignored. Each problem is reported once
//! per render, by the lowering rule. [`heading_options`] and the aspects of the heading
//! element report nothing.
//!
//! # Numbering
//!
//! The headings of each standard heading kind are counted in a *counter* of their own (a
//! dotted name under which things are counted in document order; see [counters and
//! counter styles](crate::guide::concepts_overview#counters-and-counter-styles)). The
//! name of the counter is the name of the heading kind after the prefix
//! [`COUNTER_PREFIX`], as in `flm.headings.section`. The counter of a heading kind is
//! numbered within the counter of the kind above it, so the second subsection of the
//! first section is numbered `1.2`. The submodule [`numbers`] declares the counters and
//! their *counter styles*, with which the numbers are formatted, and describes the
//! numbering of the standard heading kinds in detail.
//!
//! The heading kinds that are numbered are selected with the setting
//! [`HeadingsConfig::numbering`] of the configuration, a [`HeadingNumbering`]:
//!
//! - [`NothingNumbered`](HeadingNumbering::NothingNumbered), the default: no heading is
//!   numbered;
//! - [`AllNumbered`](HeadingNumbering::AllNumbered): every heading kind that has a
//!   counter is numbered;
//! - [`UpToDepth(n)`](HeadingNumbering::UpToDepth): every heading kind that has a
//!   counter and a level of at most `n` is numbered.
//!
//! Like the rest of the configuration, this setting is fixed when the registry is
//! built:
//!
//! ```
//! use flm_core::stdlib::features::headings::{HeadingNumbering, HeadingsConfig};
//!
//! // Number the sections and the subsections, and leave the three deeper
//! // levels unnumbered.
//! let config = HeadingsConfig::new().numbering(HeadingNumbering::UpToDepth(2));
//! assert!(config.numbers(config.heading("subsection").expect("shipped")));
//! assert!(!config.numbers(config.heading("subsubsection").expect("shipped")));
//! ```
//!
//! A heading that is not numbered does not advance its counter. This applies to the
//! headings of a heading kind that is not numbered, and to a heading written with a star
//! or with `numbered=false`. The headings after such a heading continue the numbering
//! from the last number given, as in LaTeX. A heading kind declared with
//! [`HeadingDecl::no_counter`] has no counter and is never numbered.
//!
//! The counter and the counter style of a heading kind that is not numbered are
//! declared all the same. Therefore, another counter can be numbered within this
//! counter with the numbering options of a document.
//!
//! The *parent set* [`LEVELS_PARENT_SET`] (`flm.headings.levels`) is also declared in
//! the `headings` feature. It contains the counters of all heading kinds. A counter
//! numbered within this parent set ([`Within::parent_set`]) is numbered within the most
//! recent heading, whatever its level.
//!
//! # Run-in headings
//!
//! A *run-in heading* is a heading set at the start of the paragraph that it introduces,
//! on the first line of the paragraph. For instance, if `paragraph` is a run-in heading
//! kind, the plain-text output of `\paragraph{Note} Some content.` is
//! `Note. Some content.`. Other headings are set as blocks, on lines of their own.
//!
//! The headings of a kind are run-in headings if the field [`HeadingDecl::run_in`] of
//! the kind is `true`. This field is `false` for every standard heading kind.
//! [`HeadingsConfig::run_in_levels`] sets it by level.
//!
//! A run-in heading differs from a block heading in two respects:
//!
//! - Its block role ([`BlockRoleAspect`]) is [`BlockRole::BlockDecorator`] at
//!   [`Placement::Leads`], and not [`BlockRole::Block`]. The block-structure pass
//!   ([`crate::lang::rewrites::blocks`]) therefore starts a paragraph at the heading, and
//!   the content after the heading becomes part of this paragraph. A run-in heading that
//!   is followed by no content, for instance by a blank line, is placed in a paragraph
//!   of its own.
//! - The lowering rule [`RULE`] produces a
//!   [`flm.pres.deco.runin`](crate::stdlib::backends::pres::DECO_RUNIN) decoration that
//!   contains a [`flm.pres.runin-heading`](crate::stdlib::backends::pres::RUNIN_HEADING)
//!   node, and not a
//!   [`flm.pres.block-heading`](crate::stdlib::backends::pres::BLOCK_HEADING) node. A
//!   *decoration* is a node that the lowering driver moves from the content of the
//!   enclosing presentation node into an attached slot of that node
//!   ([`Decoration`](crate::stdlib::backends::pres::Decoration); see [decorations,
//!   roles and kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)). The
//!   backend therefore writes the heading at the start of the paragraph.
//!
//! In all other respects, a run-in heading is the same as a block heading. It has a
//! level, it is listed in a table of contents unless `toc=false` is given, and it has a
//! reference point.
//!
//! # The reference point
//!
//! Every heading has one *reference point*: a place in the document that can be referred
//! to, numbered, or given an anchor ([`RefPoint`]; see [reference points, labels and
//! anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)). The
//! reference point of a heading has the following properties:
//!
//! - Its reference kind ([`RefKind`]) is the [`ref_kind`](HeadingDecl::ref_kind) of the
//!   heading kind, [`KIND`] (`section`) by default.
//! - Its labels are the `\label` commands written after the title. They are read with
//!   [`label_uses`](refs::label_uses) from a trailing argument of the heading that
//!   contains them (see
//!   [fields](crate::guide::concepts_overview#fields-data-a-construct-reads)). A label
//!   must have the form `type:label`, as in `\label{sec:intro}`. The label check of the
//!   `refs` feature reports a label that does not have this form.
//! - Its accepted label types are the [`label_types`](HeadingDecl::label_types) of the
//!   heading kind, [`LABEL_TYPES`] (`sec` and `topic`) by default.
//! - A reference to the heading (`\ref`) expands to the number of the heading, or to its
//!   title if the heading has no number.
//! - Its anchor (the identifier that a link points to) is derived from the first label of
//!   the heading, as in `sec-intro`. The anchor of a heading without a label is derived
//!   from its title, after the [`anchor_prefix`](HeadingDecl::anchor_prefix) of the
//!   heading kind ([`ANCHOR_PREFIX`] by default), as in `sec--Introduction`. If two
//!   headings without a label have the same title, the anchors stage of the `refs`
//!   feature appends a number to the anchor of the second one
//!   (`sec--Introduction-2`).
//!
//! # The element
//!
//! Every heading is a node of the element type [`HEADING`] (`flm.heading`). Each heading
//! kind is a separate element value of this type, which contains a [`HeadingElement`]:
//! the declaration of the heading kind and its heading commands. The names of the heading
//! commands are the names of the element value. The element value of a heading kind
//! without a command has no name and is synthesized ([`SyntaxDef::Synthesized`]), so no
//! document can write it.
//!
//! Every heading command takes the arguments returned by [`heading_args`]: the star, the
//! optional key-value argument, the title, and the trailing argument that contains the
//! `\label` commands.
//!
//! The element value has three aspects:
//!
//! - its block role: a block, or for a run-in heading kind a decorator that leads a
//!   paragraph;
//! - [`Sectioning`], which returns the level of the heading and whether the heading is
//!   listed in a table of contents;
//! - [`Referenceable`], which returns the reference point of the heading.
//!
//! A second element, [`title::NUMBER_ELEMENT`] (`flm.heading.number`), is declared in
//! the `headings` feature. It contains the number of a numbered heading in the text
//! written before its title. No document can write it; see [`title`].
//!
//! # The output on each backend
//!
//! - On the `text` backend, a heading at the root heading level
//!   ([`TextRenderOptions::root_heading_level`](crate::stdlib::backends::text::TextRenderOptions::root_heading_level),
//!   1 by default) or one level deeper is written as its title, underlined. A deeper
//!   heading is written as its title after `#` characters. A run-in heading is written
//!   as its title followed by a period
//!   ([`TextRenderOptions::run_in_stop`](crate::stdlib::backends::text::TextRenderOptions::run_in_stop)),
//!   at the start of its paragraph.
//! - On the `html` backend, a heading is an element `<h1>` to `<h6>` that corresponds to
//!   its level relative to the root heading level. Another element can be set for a
//!   heading kind with the render option
//!   [`heading_tags`](crate::stdlib::backends::html::HtmlRenderOptions::heading_tags).
//!   The element has the anchor as its `id`, and the
//!   attributes `data-flm-heading`, `data-flm-level` and `data-flm-toc`. A run-in heading
//!   is a `<span>` with the same attributes at the start of its paragraph ([headings in
//!   HTML](crate::guide::rendering#headings-in-html)).
//! - On the `latex-recomposed` backend, a heading is the LaTeX sectioning command of its
//!   heading kind, inside the LaTeX hook `\flmHeading` (see [`latex`] and
//!   [hooks](crate::stdlib::backends::latex::preamble)).
//!
//! On the `text` and `html` backends, the number of a numbered heading is written before
//! its title, as in `1.2. Notation`. On the `latex-recomposed` backend, the numbering
//! setting of the configuration has no effect. A heading written with a star or with
//! `numbered=false` is written with a starred sectioning command, every other heading
//! with an unstarred one, and LaTeX numbers the headings according to the document
//! class.
//!
//! # The lowering rules
//!
//! [`RULE`] (`flm.heading.lower.default`), implemented by [`HeadingRule`], is the
//! lowering rule of headings on every backend except `latex-recomposed`. It produces a
//! [`flm.pres.block-heading`](crate::stdlib::backends::pres::BLOCK_HEADING) node, or
//! for a run-in heading the decoration described in [Run-in
//! headings](#run-in-headings). The payload of the node ([`HeadingPayload`]) contains the
//! name and the level of the heading kind, whether the heading is listed in a table of
//! contents, and the anchor.
//!
//! The content of the node is the title, preceded by the number if the heading is
//! numbered. The number is written with the *heading title template* of the heading kind
//! ([`title_number_format`](HeadingDecl::title_number_format), `#{number}. ` by
//! default). In the output of the template, the number is a
//! [`flm.pres.span`](crate::stdlib::backends::pres::SPAN) node with the role
//! [`NUMBER_ROLE`](numbers::NUMBER_ROLE). Since the number is part of the content, a
//! backend that measures the heading includes the number: on the `text` backend, the
//! line under a numbered heading is as wide as `1. Introduction`, and not only as wide
//! as `Introduction`.
//!
//! The rule reads the anchors table [`ANCHORS`] of the `refs` feature and the counter
//! table [`COUNTERS`] of the `numbering` feature. Both tables are declared as peer
//! declarations ([`PeerName`](crate::core::registry::PeerName)) and read optionally, so
//! a registry without one of these features still builds. Without the `numbering`
//! feature, headings have no numbers, and without the `refs` feature, headings have no
//! anchors. In both cases, the registry build reports one note about the missing table,
//! and nothing is reported at the headings.
//!
//! A heading can lack an anchor even if the anchors table is present:
//!
//! - The anchors stage of the `refs` feature gave the heading no anchor, for instance
//!   because every label of the heading is also defined at an earlier place. A stage of
//!   the `refs` feature reports the cause. The heading is written without an anchor, so
//!   no link can point to it. This cannot happen to a heading without a label, since its
//!   anchor is derived from its title.
//! - The anchors table has no entry for the heading. This means that the rule lowers
//!   content on which the stages did not run (see [value
//!   catch-up](crate::guide::concepts_overview#value-catch-up)). The rule reports
//!   `flm.run.missing-entry` ([`MissingEntry`]) and fails.
//!
//! [`latex::LATEX_RULE`] (`flm.heading.lower.latex-recomposed`) is the lowering rule of
//! headings on the `latex-recomposed` backend. It writes each heading as the LaTeX
//! sectioning command of its heading kind
//! ([`HeadingsLatexOptions`](latex::HeadingsLatexOptions)). The rule is registered with
//! peer declarations of the backend and of the LaTeX elements that it produces. A
//! registry without the LaTeX features therefore still builds, and the rule is then not
//! used. See [`latex`].
//!
//! # Settings
//!
//! The settings of the feature (its configuration and options in data form; see
//! [`crate::core::settings`]) consist of two parts:
//!
//! - the configuration [`HeadingsConfig`], with the keys `headings`, `commands` and
//!   `numbering`, which is fixed when the registry is built;
//! - the render option [`HeadingsLatexOptions`](latex::HeadingsLatexOptions), with the
//!   key `latex_commands`, which is set for each render.
//!
//! In settings data, the values of `headings` and `commands` are maps from the names of
//! the heading kinds and heading commands to their declarations.
//!
//! # Related features
//!
//! - `refs` ([`refs`]): The labels of a heading are read with functions of the `refs`
//!   feature, and its anchor is read from the anchors table. On the `latex-recomposed`
//!   backend, each label is written as its safe label, the form of the label used in the
//!   LaTeX output ([`SafeLabels`](crate::stdlib::features::refs::SafeLabels)).
//! - `numbering` ([`numbering`](crate::stdlib::features::numbering)): The counters, the
//!   counter styles and the parent set of the heading kinds are declared in the
//!   `headings` feature as peer contributions to the service slots of the `numbering`
//!   feature (see [services and service
//!   slots](crate::guide::concepts_overview#services-and-service-slots) and
//!   [`numbers`]). The lowering rules read the number of each heading from the counter
//!   table.
//! - `blocks` ([`blocks`](crate::stdlib::features::blocks)): The block-structure pass
//!   places a heading according to its block role.
//! - `userdefs` ([`userdefs`](crate::stdlib::features::userdefs)): If the value of
//!   `numbered` or `toc` is given by a placeholder of a user definition, the value
//!   cannot be read until the definition is expanded. The lowering rules then return
//!   [`LowerError::PlaceholderCharacters`], so that the use sites of the definition are
//!   expanded first.
//! - The backends: [`RULE`] requires the style identifier [`STYLE`] for every heading.
//!   On the `html` backend, [`STYLE`] refers to an empty style definition
//!   ([`stylesheet`]). On the `latex-rendered` backend, [`STYLE`] is defined in the
//!   definition piece [`latex_rendered::piece`]. [`latex::LATEX_RULE`] requires the
//!   definition piece of the LaTeX hook `\flmHeading`
//!   ([`heading_piece`](crate::stdlib::backends::latex::preamble::heading_piece)).

pub mod latex;
pub mod latex_rendered;
pub mod numbers;
pub mod title;

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::extract::parse_keyval_drop_annotations;

use crate::core::settings::{SettingsDecl, named};
use crate::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, BlockRole, BlockRoleAspect, ContentRef, FixedBlockRole,
    NumberSource, Placement, PointId, RefPoint, Referenceable, Sectioning,
};
use crate::model::element::{ElementSpec, element_value};
use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
    chars_at_lowering,
};
use crate::model::names::{CapName, ElementName, Name, RefKind};
use crate::model::options::Bind;
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature, StyleDef};
use crate::core::run::MissingEntry;
use crate::lang::Flm;
use crate::lang::copy::{
    ValueContent, ValuePiece, WrittenPiece, copy_pieces_into_value, key_value_pieces,
};
use crate::lang::guard::{GuardPolicy, on_worker_thread};
use crate::lang::mint::{ArgDef, ArgStdKind, ElementDef, SyntaxDef};
use crate::lang::text::is_space;
use crate::stdlib::backends::lowering::{lower_arg_named_or_nothing, lower_written_pieces};
use crate::stdlib::backends::pres::{self, DecorationPayload, HeadingPayload};
use crate::stdlib::features::numbering::{
    COUNTERS, Counters, FormatOptions, NumberTemplate, Within,
};
use crate::stdlib::features::refs::{self, ANCHORS};

use title::{CompileTitles, NumberRule, Titles, DEFAULT_TITLE_NUMBER_FORMAT};

/// The feature's registered name, `headings`.
pub const FEATURE: &str = "headings";

/// Returns the declaration of the settings of the `headings` feature. The
/// settings consist of the configuration [`HeadingsConfig`] and the render
/// option [`HeadingsLatexOptions`](latex::HeadingsLatexOptions); the fields of
/// these two types are the keys of the settings. See [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<HeadingsConfig>(
            "The heading kinds, the heading commands, and the heading kinds that are numbered. \
            For each heading kind, the configuration contains its level, its counter and \
            counter style, the counter it is numbered within, its number format, the text \
            written before the title of a numbered heading, whether its headings are listed in \
            a table of contents or set as run-in headings, and the labels and anchors of its \
            headings. For each heading command, it contains the heading kind of the headings \
            written with the command and the default values of the options `numbered` and \
            `toc`. Fixed when the registry is built; it cannot vary per document or per render.",
        )
        .options::<latex::HeadingsLatexOptions>(
            Bind::Render,
            "The LaTeX sectioning command of each heading kind in the recomposed LaTeX, given \
         by the name of the heading kind. A heading kind that is not listed is written as the \
         command of the same name, such as `\\section` for the kind `section`. Set per render \
         request.",
        )
        .merge_entries("headings", || named::to_data(&HeadingsConfig::default().headings, "name"))
        .remove_false("headings")
        .merge_entries("commands", || named::to_data(&HeadingsConfig::default().commands, "name"))
        .remove_false("commands")
        .markup("headings.*.within.joiner.joined")
        .markup("headings.*.within.joiner.template")
        .markup("headings.*.number_format")
        .markup("headings.*.title_number_format")
}

/// The name of the element type of all headings, `flm.heading`. Each heading
/// kind is a separate element value of this type, which contains a
/// [`HeadingElement`].
pub const HEADING: &str = "flm.heading";

/// The name of the default lowering rule of headings,
/// `flm.heading.lower.default`, implemented by [`HeadingRule`]. This rule lowers
/// the headings on every backend except `latex-recomposed`, which uses
/// [`latex::LATEX_RULE`].
pub const RULE: &str = "flm.heading.lower.default";

/// The style identifier of the `headings` feature, `flm.headings.style`.
///
/// The lowering rule [`RULE`] requires this identifier for every heading. On
/// the `html` backend, the identifier refers to an empty style definition
/// ([`stylesheet`]). On the `latex-rendered` backend, the identifier is defined
/// in the definition piece [`latex_rendered::piece`]. The names of the counter styles of
/// the heading kinds ([`STYLE_PREFIX`]) begin with the same text, but they are
/// counter styles and not style definitions.
pub const STYLE: &str = "flm.headings.style";

/// Returns the style definition of [`STYLE`] for the `html` backend.
///
/// The definition contains no style rules, since headings need no stylesheet of
/// their own. It exists so that the identifier that [`RULE`] requires is
/// defined on the `html` backend.
pub fn stylesheet() -> StyleDef {
    StyleDef {
        id: Name::from(STYLE),
        content: Vec::new(),
        docs: String::from("headings, which need no style rules"),
    }
}

/// The default reference kind of the reference point of a heading, `section`.
/// A reference kind is the category of a reference point ([`RefPoint`]), such
/// as a section. Another reference kind can be declared for a heading kind with
/// [`HeadingDecl::ref_kind`].
pub const KIND: &str = "section";

/// The label types that a heading accepts by default, `sec` and `topic`. Other
/// label types can be declared for a heading kind with
/// [`HeadingDecl::label_types`].
///
/// The type of a label is the part before the colon. A heading therefore
/// accepts labels such as `\label{sec:intro}` and `\label{topic:intro}`. A
/// label of another type defines no label, and the label check of the `refs`
/// feature ([`CHECK_LABELS`](crate::stdlib::features::refs::CHECK_LABELS))
/// reports it as `flm.refs.label-type-not-accepted`. The type `topic` is
/// accepted so that the heading that introduces a subject can be labeled for
/// references from other documents.
pub const LABEL_TYPES: [&str; 2] = ["sec", "topic"];

/// The default text at the start of the anchor of a heading without a label,
/// `sec`. Another prefix can be declared for a heading kind with
/// [`HeadingDecl::anchor_prefix`].
///
/// The anchor of a heading without a label is derived from its title: the
/// heading `\section{Examples}` has the anchor `sec--Examples`. The prefix is
/// the first of the [`LABEL_TYPES`], so that the anchors of labeled headings
/// begin in the same way: `\section{Examples}\label{sec:examples}` has the
/// anchor `sec-examples`.
pub const ANCHOR_PREFIX: &str = LABEL_TYPES[0];

/// The text at the start of the name of the counter style of a heading kind,
/// `flm.headings.style.`. Another counter style can be declared for a heading
/// kind with [`HeadingDecl::style`]. By default, the numbers of the heading
/// kind `section` are formatted with the counter style
/// `flm.headings.style.section`.
pub const STYLE_PREFIX: &str = "flm.headings.style.";

/// The text at the start of the name of the counter of a heading kind,
/// `flm.headings.`. Another counter can be declared for a heading kind with
/// [`HeadingDecl::counter`]. By default, the headings of the kind `section` are
/// counted in the counter `flm.headings.section`.
///
/// The name of the counter is distinct from the name of the heading kind. A
/// counter is always given by its full name: in a [`Within`], in a document's
/// [numbering options](crate::stdlib::features::numbering::NumberingOptions),
/// and in a placeholder such as `#{flm.headings.section}` of a within template.
pub const COUNTER_PREFIX: &str = "flm.headings.";

/// The name of the star argument of every heading command, `star`.
pub const STAR_ARG: &str = "star";

/// The name of the optional key-value argument of every heading command,
/// `options`. In `\section[toc=false]{…}`, this argument is `[toc=false]`.
pub const OPTIONS_ARG: &str = "options";

/// The name of the title argument of every heading command, `title`.
pub const TITLE_ARG: &str = "title";

/// The option key with which an author selects whether a heading is numbered,
/// `numbered`. Its value is `true` or `false`. A star after the heading command
/// is equivalent to `numbered=false`.
pub const NUMBERED_KEY: &str = "numbered";

/// The option key with which an author selects whether a heading is listed in a
/// table of contents, `toc`. Its value is `true` or `false`.
pub const TOC_KEY: &str = "toc";

/// The option key with which an author gives a heading an alternative title,
/// `alt`, as in `\section[alt={Short \emph{title}}]{…}`.
///
/// The value is FLM content, which [`heading_alt`] returns. In a table of
/// contents, the alternative title is displayed in place of the title
/// ([`Sectioning::toc_title`]). The key written without a value is reported as
/// [`MissingValue`].
pub const ALT_KEY: &str = "alt";

/// The option keys that are reserved for headings but not implemented:
/// `pdfbookmark`, the title of a PDF bookmark. A reserved key is ignored and
/// reported as the warning [`KeyNotImplemented`].
pub const RESERVED_KEYS: [&str; 1] = ["pdfbookmark"];

/// The name of the parent set declared in the `headings` feature,
/// `flm.headings.levels`.
///
/// A *parent set* is a named list of counters
/// ([`ParentSetDecl`](crate::stdlib::features::numbering::ParentSetDecl)). A
/// counter numbered within a parent set ([`Within::parent_set`]) is numbered
/// within the counter of the set that advanced most recently. This parent set
/// contains the counters of the heading kinds that have one, ordered by level
/// from the outermost level, and in the order of declaration within one level.
///
/// A counter numbered within this parent set is therefore numbered within the
/// most recent heading. For instance, if a program declares a heading kind
/// `chapter` above the sections, an equation after the heading of chapter 4 and
/// before any section is numbered `(4.1)`, and an equation after subsection
/// 4.1.2 is numbered `(4.1.2.1)`. The counter of a heading kind that is not
/// numbered never advances, so its headings are never the parent of such a
/// counter.
pub const LEVELS_PARENT_SET: &str = "flm.headings.levels";

/// A `HeadingNumbering` is the setting with which a program selects the heading
/// kinds that are numbered ([`HeadingsConfig::numbering`]).
///
/// A heading kind is numbered if it has a counter ([`HeadingDecl::counter`])
/// and its level is among the levels selected with this setting. A single
/// heading of a numbered kind is not numbered if it is written with a star or
/// with `numbered=false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HeadingNumbering {
    /// No heading kind is numbered. This is the default.
    #[default]
    NothingNumbered,
    /// Every heading kind that has a counter is numbered.
    AllNumbered,
    /// Every heading kind that has a counter and a level of at most the given
    /// number is numbered. For instance, with `UpToDepth(2)`, the sections and
    /// the subsections of the standard configuration are numbered.
    UpToDepth(u8),
}

impl HeadingNumbering {
    /// Returns whether a kind at `level` is numbered under this depth, provided
    /// the kind has a counter.
    pub fn admits(self, level: u8) -> bool {
        match self {
            HeadingNumbering::NothingNumbered => false,
            HeadingNumbering::AllNumbered => true,
            HeadingNumbering::UpToDepth(depth) => level <= depth,
        }
    }
}

// --- the declarations -----------------------------------------------------------------------

/// The declaration of a heading kind.
///
/// A *heading kind* is a type of heading, such as a section, a subsection or an
/// appendix. A heading kind has a name and a level. Its declaration also
/// contains the settings for the numbering of its headings, for their listing in
/// a table of contents, for run-in headings, and for the labels that its
/// headings accept. The heading kinds of a registry are listed in
/// [`HeadingsConfig::headings`].
///
/// [`HeadingDecl::new`] creates a heading kind from a name and a level, with
/// default values for all other fields. Each of these fields has a builder
/// method of the same name that changes it.
///
/// The declaration of a heading kind does not list the commands with which its
/// headings are written. These commands are declared separately, as
/// [`HeadingCommand`] values in [`HeadingsConfig::commands`], because headings of
/// the same kind can be written with several commands.
///
/// # Examples
///
/// ```
/// use flm_core::stdlib::features::headings::HeadingDecl;
/// use flm_core::stdlib::features::numbering::{NumberTemplate, Within};
///
/// // An appendix at the level of the sections, numbered A, B, C.
/// let appendix =
///     HeadingDecl::new("appendix", 1).number_format(NumberTemplate::formats("#{Alph}"));
/// assert_eq!(
///     appendix.counter.as_ref().map(|name| name.as_str()),
///     Some("flm.headings.appendix")
/// );
/// assert_eq!(
///     appendix.style.as_ref().map(|name| name.as_str()),
///     Some("flm.headings.style.appendix")
/// );
///
/// // A heading kind that is never numbered and never listed in a table of
/// // contents.
/// let aside = HeadingDecl::new("aside", 3).no_counter().skip_toc(true);
/// assert!(aside.counter.is_none());
///
/// // A heading kind numbered within the sections, as in `2.1`. The `Within`
/// // value refers to the counter of the sections by its full name.
/// let sub = HeadingDecl::new("subsection", 2).within(Within::parent("flm.headings.section"));
/// # let _ = sub;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(transform = heading_decl_schema)]
pub struct HeadingDecl {
    /// The name of the heading kind, such as `section`. The name identifies the
    /// heading kind in the attribute `data-flm-heading` of the HTML output and
    /// in the render option [`HeadingsLatexOptions`](latex::HeadingsLatexOptions).
    /// In settings data, the name is the key of the entry and not a field.
    #[schemars(skip)]
    pub name: Name,
    /// The level of the heading kind. Levels count from 0, and an outer heading
    /// has a smaller level than the headings inside it. The standard heading
    /// kinds have the levels 1 to 5, so a heading kind above the sections, such
    /// as a chapter, can have the level 0.
    ///
    /// The [`Sectioning`] aspect returns the level. The HTML element of a
    /// heading corresponds to its level, unless another element is set with a
    /// render option. The format of a heading in plain text also depends on its
    /// level. Whether the heading kind is numbered depends on its level and on
    /// the setting [`HeadingNumbering`].
    pub level: u8,
    /// The counter in which the headings of this kind are counted, given by its
    /// full name. The default is `flm.headings.<name>` ([`COUNTER_PREFIX`]).
    /// `None` means that the heading kind is never numbered; no counter is then
    /// declared for it in the `numbering` feature.
    pub counter: Option<Name>,
    /// The counter style with which the numbers of this heading kind are
    /// formatted. The default is `flm.headings.style.<name>` ([`STYLE_PREFIX`]).
    /// A heading kind with a counter must have a counter style.
    /// [`HeadingDecl::no_counter`] removes both the counter and the counter
    /// style.
    pub style: Option<Name>,
    /// The counter within which the counter of this heading kind is numbered,
    /// and how the number of that counter is written before the number of this
    /// heading kind. The default, `None`, means that the counter is not
    /// numbered within another counter. Counters are given by their full names,
    /// as in `Within::parent("flm.headings.section")`.
    pub within: Option<Within>,
    /// The format of the number of this heading kind, without the numbers of
    /// its parent counters, in the number-template language of
    /// [`NumberTemplate`]. The default is `#{arabic}`.
    pub number_format: NumberTemplate,
    /// The *heading title template* of this heading kind: the text written
    /// before the title of a numbered heading. The text is written in the
    /// template language (see [the template language shared by
    /// features](crate::guide::concepts_overview#the-template-language-shared-by-features)),
    /// and the placeholder `#{number}` stands for the number of the heading. The
    /// default is [`DEFAULT_TITLE_NUMBER_FORMAT`] (`#{number}. `), so a numbered
    /// section reads `1. Introduction`.
    ///
    /// The template can contain words and markup around the number, as in
    /// `Chapter~#{number}: `. With `#[bare]{number}`, the number is written
    /// without the numbers of the parent counters. With `#[Roman]{number}`, the
    /// number of this heading kind is written in Roman numerals. The number
    /// itself has the role [`NUMBER_ROLE`](numbers::NUMBER_ROLE), with which it
    /// can be formatted in a stylesheet or a lowering rule. A template without a
    /// placeholder is allowed: the headings are then counted, but their numbers
    /// are not written. The template is not used for a heading that is not
    /// numbered.
    ///
    /// The registry build fails if the template cannot be parsed, if it contains
    /// a placeholder other than `number`, or if `number` is written with a key
    /// that it does not accept. [`title`] describes the template in full.
    pub title_number_format: String,
    /// Whether the headings of this kind are never listed in a table of
    /// contents. If `true`, the option `toc` of these headings has no effect.
    /// The default is `false`.
    pub skip_toc: bool,
    /// Whether the headings of this kind are run-in headings, set at the start
    /// of the paragraph that they introduce (see [run-in
    /// headings](self#run-in-headings)). The default is `false`.
    pub run_in: bool,
    /// The reference kind of the reference point of a heading of this kind. The
    /// default is [`KIND`] (`section`).
    pub ref_kind: RefKind,
    /// The label types that a heading of this kind accepts. The default is
    /// [`LABEL_TYPES`] (`sec` and `topic`).
    pub label_types: Vec<String>,
    /// The text at the start of the anchor of a heading of this kind that has
    /// no label. The default is [`ANCHOR_PREFIX`] (`sec`).
    pub anchor_prefix: String,
    /// A description of the heading kind in one sentence, used in the
    /// description of its element value in the registry. The default is empty,
    /// and an empty description is replaced with `A heading of level <level>.`.
    pub docs: String,
}


/// Changes the JSON schema of a heading kind in settings data so that only
/// `level` is required. The other fields are optional, because the entry is
/// read on top of the declaration that [`HeadingDecl::new`] creates from the
/// name and the level.
fn heading_decl_schema(schema: &mut schemars::Schema) {
    crate::core::settings::named::require_only(schema, &["level"]);
}
impl HeadingDecl {
    /// Returns the heading kind `name` at the level `level`, with the following
    /// default values of the other fields:
    ///
    /// - the counter `flm.headings.<name>` ([`COUNTER_PREFIX`]) and the counter
    ///   style `flm.headings.style.<name>` ([`STYLE_PREFIX`]);
    /// - no counter within which the counter is numbered;
    /// - the number format `#{arabic}` and the heading title template
    ///   [`DEFAULT_TITLE_NUMBER_FORMAT`];
    /// - headings listed in a table of contents and set as blocks;
    /// - the reference kind [`KIND`], the label types [`LABEL_TYPES`] and the
    ///   anchor prefix [`ANCHOR_PREFIX`];
    /// - an empty description.
    pub fn new(name: impl Into<Name>, level: u8) -> HeadingDecl {
        let name = name.into();
        HeadingDecl {
            counter: Some(Name::new(format!("{COUNTER_PREFIX}{}", name.as_str()))),
            style: Some(Name::new(format!("{STYLE_PREFIX}{}", name.as_str()))),
            name,
            level,
            within: None,
            number_format: NumberTemplate::formats("#{arabic}"),
            title_number_format: String::from(DEFAULT_TITLE_NUMBER_FORMAT),
            skip_toc: false,
            run_in: false,
            ref_kind: RefKind::from(KIND),
            label_types: LABEL_TYPES.iter().map(|label| String::from(*label)).collect(),
            anchor_prefix: String::from(ANCHOR_PREFIX),
            docs: String::new(),
        }
    }

    /// Sets the counter of this heading kind to `counter`, given by its full
    /// name.
    pub fn counter(mut self, counter: impl Into<Name>) -> HeadingDecl {
        self.counter = Some(counter.into());
        self
    }

    /// Removes the counter and the counter style of this heading kind. A heading
    /// kind without a counter is never numbered, and no counter is declared for
    /// it in the `numbering` feature.
    pub fn no_counter(mut self) -> HeadingDecl {
        self.counter = None;
        self.style = None;
        self
    }

    /// Sets the counter style of this heading kind to `style`.
    pub fn style(mut self, style: impl Into<Name>) -> HeadingDecl {
        self.style = Some(style.into());
        self
    }

    /// Sets the counter within which the counter of this heading kind is
    /// numbered (see the field [`within`](HeadingDecl::within)).
    pub fn within(mut self, within: Within) -> HeadingDecl {
        self.within = Some(within);
        self
    }

    /// Sets the format of the number of this heading kind.
    pub fn number_format(mut self, number_format: NumberTemplate) -> HeadingDecl {
        self.number_format = number_format;
        self
    }

    /// Sets the heading title template of this heading kind: the text written
    /// before the title of a numbered heading, in which `#{number}` stands for
    /// the number (see the field
    /// [`title_number_format`](HeadingDecl::title_number_format)).
    pub fn title_number_format(mut self, text: impl Into<String>) -> HeadingDecl {
        self.title_number_format = text.into();
        self
    }

    /// Sets whether the headings of this kind are never listed in a table of
    /// contents.
    pub fn skip_toc(mut self, skip_toc: bool) -> HeadingDecl {
        self.skip_toc = skip_toc;
        self
    }

    /// Sets whether the headings of this kind are run-in headings.
    pub fn run_in(mut self, run_in: bool) -> HeadingDecl {
        self.run_in = run_in;
        self
    }

    /// Sets the reference kind of the reference point of a heading of this
    /// kind.
    pub fn ref_kind(mut self, ref_kind: impl Into<RefKind>) -> HeadingDecl {
        self.ref_kind = ref_kind.into();
        self
    }

    /// Sets the label types that a heading of this kind accepts.
    pub fn label_types(mut self, types: impl IntoIterator<Item = impl Into<String>>) -> HeadingDecl {
        self.label_types = types.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the text at the start of the anchor of a heading of this kind that
    /// has no label.
    pub fn anchor_prefix(mut self, prefix: impl Into<String>) -> HeadingDecl {
        self.anchor_prefix = prefix.into();
        self
    }

    /// Sets the description of this heading kind (see the field
    /// [`docs`](HeadingDecl::docs)). If the crate is built without its cargo
    /// feature `docs`, the text is discarded and the field stays empty.
    pub fn docs(mut self, docs: impl Into<String>) -> HeadingDecl {
        self.docs = crate::kept_docs(docs);
        self
    }

    /// Returns the number source of a numbered heading of this kind: the
    /// counter and the counter style of the kind. The counter advances by one
    /// at each such heading. Returns `None` for a heading kind without a
    /// counter or without a counter style.
    fn number_source(&self) -> Option<NumberSource> {
        Some(NumberSource {
            counter: self.counter.clone()?,
            instances: Vec::new(),
            style: self.style.clone()?,
            advance: vec![0],
            number_format: None,
        })
    }
}

/// The declaration of a heading command.
///
/// A *heading command* is a macro, such as `\section`, that an author writes to
/// create a heading. The headings written with a heading command are of one
/// heading kind ([`HeadingDecl`]). A heading command can be declared with
/// default values for the options `numbered` and `toc` of these headings.
///
/// Headings of the same kind can be written with several heading commands. All
/// heading commands take the same arguments ([`heading_args`]), so two commands
/// of one kind differ only in their default options. The default options are
/// applied before the star and the key-value argument of the heading. An author
/// can therefore change them with the star and the key-value argument (see [the
/// options of a heading](self#the-options-of-a-heading)).
///
/// The key of each default option must be [`NUMBERED_KEY`] or [`TOC_KEY`], and
/// its value must be `true` or `false`. Otherwise, the registry build fails
/// ([`HeadingsConfig::validate`]).
///
/// # Examples
///
/// ```
/// use flm_core::stdlib::features::headings::HeadingCommand;
///
/// // A heading written with `\quietsection{…}` is a section that is not listed
/// // in the table of contents, unless the heading is written with `toc=true`.
/// let quiet = HeadingCommand::new("quietsection", "section").default_value("toc", "false");
/// assert_eq!(quiet.defaults, [(String::from("toc"), String::from("false"))]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HeadingCommand {
    /// The name of the command without the backslash, such as `section`. In
    /// settings data, the name is the key of the entry and not a field.
    #[schemars(skip)]
    pub name: String,
    /// The name of the heading kind of the headings written with the command.
    pub heading: Name,
    /// The default options of the headings written with this command, as
    /// key-value pairs. They are applied in order, before the star and the
    /// key-value argument of the heading.
    #[serde(default)]
    pub defaults: Vec<(String, String)>,
}

impl HeadingCommand {
    /// Returns the heading command `name`, with which headings of the kind
    /// `heading` are written, without default options.
    pub fn new(name: impl Into<String>, heading: impl Into<Name>) -> HeadingCommand {
        HeadingCommand { name: name.into(), heading: heading.into(), defaults: Vec::new() }
    }

    /// Adds the default option `key=value`. It is applied after the default
    /// options added before it.
    pub fn default_value(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> HeadingCommand {
        self.defaults.push((key.into(), value.into()));
        self
    }
}

/// The configuration of the [`headings`](self) feature. It contains the heading
/// kinds ([`HeadingDecl`]), the heading commands ([`HeadingCommand`]), and the
/// setting [`numbering`](HeadingsConfig::numbering), with which the heading kinds
/// that are numbered are selected.
///
/// A program passes the configuration to [`Headings::new`], or to
/// [`StdlibConfig::headings`](crate::stdlib::StdlibConfig::headings). The
/// configuration is fixed when the registry is built (see [settings, options and
/// configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
///
/// The default configuration, [`HeadingsConfig::new`], contains the five heading
/// kinds `section`, `subsection`, `subsubsection`, `paragraph` and `subparagraph`
/// at the levels 1 to 5, and one heading command with the same name for each
/// heading kind. No heading kind is numbered by default. The submodule
/// [`numbers`] describes the counters and the number formats of these heading
/// kinds.
///
/// When the registry is built, the `headings` feature checks the configuration
/// with [`HeadingsConfig::validate`], and the registry build fails if the check
/// fails. A heading kind without a heading command is accepted. No document can
/// write a heading of such a kind, but a program can create one, for instance in
/// a stage.
///
/// # Examples
///
/// ```
/// use flm_core::stdlib::features::headings::{HeadingNumbering, HeadingsConfig};
///
/// let config = HeadingsConfig::new()
///     .run_in_levels([4, 5])
///     .numbering(HeadingNumbering::UpToDepth(2));
/// assert!(config.heading("paragraph").is_some_and(|kind| kind.run_in));
/// assert!(!config.heading("subsubsection").is_some_and(|kind| kind.run_in));
/// assert_eq!(config.commands.len(), 5);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct HeadingsConfig {
    /// The heading kinds. The default is the five heading kinds `section`,
    /// `subsection`, `subsubsection`, `paragraph` and `subparagraph`.
    #[serde(deserialize_with = "crate::core::settings::named::deserialize")]
    #[schemars(
        transform = crate::core::settings::named::map_schema,
        with = "std::collections::BTreeMap<String, HeadingDecl>"
    )]
    pub headings: Vec<HeadingDecl>,
    /// The heading commands. The default is one heading command for each of the
    /// five default heading kinds, with the name of the heading kind.
    #[serde(deserialize_with = "crate::core::settings::named::deserialize")]
    #[schemars(
        transform = crate::core::settings::named::map_schema,
        with = "std::collections::BTreeMap<String, HeadingCommand>"
    )]
    pub commands: Vec<HeadingCommand>,
    /// The heading kinds that are numbered. The default is
    /// [`HeadingNumbering::NothingNumbered`]: no heading is numbered.
    pub numbering: HeadingNumbering,
}

impl Default for HeadingsConfig {
    fn default() -> HeadingsConfig {
        HeadingsConfig::new()
    }
}

impl HeadingsConfig {
    /// Returns the default configuration: the five heading kinds `section` to
    /// `subparagraph`, one heading command with the same name for each heading
    /// kind, and no numbered heading kind (see [`HeadingsConfig`]).
    // The formats, the hierarchy, the fourth level showing nothing of its
    // parents, and the texts of `numbers` are the Python product's shipped
    // defaults, read from its headings feature, except that `paragraph` is
    // numbered within whichever of the three kinds above it came last rather
    // than within `subsubsection` alone, so that a `\paragraph` directly under a
    // `\subsection` is numbered too, and restarts there as it does in LaTeX.
    pub fn new() -> HeadingsConfig {
        let letters = || NumberTemplate::formats("#{alph}");
        let counter = |kind: &str| format!("{COUNTER_PREFIX}{kind}");
        HeadingsConfig::default_commands_for(vec![
            HeadingDecl::new("section", 1),
            HeadingDecl::new("subsection", 2).within(Within::parent(counter("section"))),
            HeadingDecl::new("subsubsection", 3).within(Within::parent(counter("subsection"))),
            HeadingDecl::new("paragraph", 4)
                .number_format(letters())
                .within(
                    Within::parents(["section", "subsection", "subsubsection"].map(counter))
                        .displayed_bare(),
                ),
            HeadingDecl::new("subparagraph", 5)
                .number_format(letters())
                .within(Within::parent(counter("paragraph"))),
        ])
    }

    /// Returns the configuration with the heading kinds `headings`, one heading
    /// command with the same name and without default options for each heading
    /// kind, and no numbered heading kind.
    fn default_commands_for(headings: Vec<HeadingDecl>) -> HeadingsConfig {
        let commands = headings
            .iter()
            .map(|kind| HeadingCommand::new(kind.name.as_str(), kind.name.clone()))
            .collect();
        HeadingsConfig { headings, commands, numbering: HeadingNumbering::default() }
    }

    /// Replaces the heading kinds with `headings`, and replaces the heading
    /// commands with one heading command for each heading kind. Each of these
    /// commands has the name of its heading kind and no default options. The
    /// setting [`numbering`](HeadingsConfig::numbering) is kept.
    ///
    /// Call this method before [`command`](HeadingsConfig::command), since it
    /// discards the heading commands added before it.
    pub fn headings(self, headings: impl IntoIterator<Item = HeadingDecl>) -> HeadingsConfig {
        HeadingsConfig {
            numbering: self.numbering,
            ..HeadingsConfig::default_commands_for(headings.into_iter().collect())
        }
    }

    /// Sets the heading kinds that are numbered (see [`HeadingNumbering`]).
    pub fn numbering(mut self, numbering: HeadingNumbering) -> HeadingsConfig {
        self.numbering = numbering;
        self
    }

    /// Returns whether the heading kind `kind` is numbered under this
    /// configuration. This is the case if `kind` has a counter and the setting
    /// [`numbering`](HeadingsConfig::numbering) includes the level of `kind`.
    pub fn numbers(&self, kind: &HeadingDecl) -> bool {
        kind.counter.is_some() && self.numbering.admits(kind.level)
    }

    /// Replaces the heading commands with `commands`.
    pub fn commands(mut self, commands: impl IntoIterator<Item = HeadingCommand>) -> HeadingsConfig {
        self.commands = commands.into_iter().collect();
        self
    }

    /// Adds the heading command `command` after the heading commands that the
    /// configuration already contains.
    pub fn command(mut self, command: HeadingCommand) -> HeadingsConfig {
        self.commands.push(command);
        self
    }

    /// Makes the heading kinds at the levels `levels` run-in heading kinds, and
    /// all other heading kinds block heading kinds (see
    /// [`HeadingDecl::run_in`]).
    ///
    /// This method changes the heading kinds that the configuration contains
    /// when the method is called. Therefore, call it after
    /// [`headings`](HeadingsConfig::headings).
    pub fn run_in_levels(mut self, levels: impl IntoIterator<Item = u8>) -> HeadingsConfig {
        let levels: BTreeSet<u8> = levels.into_iter().collect();
        for kind in &mut self.headings {
            kind.run_in = levels.contains(&kind.level);
        }
        self
    }

    /// Returns the heading kind with the name `name`, or `None` if the
    /// configuration contains no such heading kind.
    pub fn heading(&self, name: &str) -> Option<&HeadingDecl> {
        self.headings.iter().find(|kind| kind.name.as_str() == name)
    }

    /// Returns the heading commands of the heading kind with the name `name`, in
    /// the order of [`commands`](HeadingsConfig::commands).
    pub fn commands_of<'c>(&'c self, name: &'c str) -> impl Iterator<Item = &'c HeadingCommand> {
        self.commands.iter().filter(move |command| command.heading.as_str() == name)
    }

    /// Checks that the configuration is consistent. The `headings` feature calls
    /// this method when the registry is built, and the registry build fails if
    /// the method returns an error.
    ///
    /// # Errors
    ///
    /// Returns an error message for the first of the following problems that is
    /// found:
    ///
    /// - two heading kinds with the same name;
    /// - two heading kinds with the same counter, or with the same counter style;
    /// - a heading kind with a counter and without a counter style;
    /// - two heading commands with the same name;
    /// - a heading command whose heading kind is not in the configuration;
    /// - a default option of a heading command whose key is not
    ///   [`NUMBERED_KEY`] or [`TOC_KEY`], or whose value is not `true` or
    ///   `false`.
    pub fn validate(&self) -> Result<(), String> {
        let mut names = BTreeSet::new();
        let mut counters = BTreeSet::new();
        let mut styles = BTreeSet::new();
        for kind in &self.headings {
            if !names.insert(kind.name.as_str()) {
                return Err(format!("Duplicate heading kind ‘{}’", kind.name));
            }
            if let Some(counter) = &kind.counter {
                if !counters.insert(counter.as_str()) {
                    return Err(format!(
                        "Duplicate counter ‘{counter}’ for heading kind ‘{}’: each heading kind \
                         must have a counter of its own",
                        kind.name
                    ));
                }
                if kind.style.is_none() {
                    return Err(format!(
                        "Missing counter style for heading kind ‘{}’ with counter ‘{counter}’",
                        kind.name
                    ));
                }
            }
            if let Some(style) = &kind.style {
                if !styles.insert(style.as_str()) {
                    return Err(format!(
                        "Duplicate counter style ‘{style}’ for heading kind ‘{}’: each heading \
                         kind must have a counter style of its own",
                        kind.name
                    ));
                }
            }
        }
        let mut commands = BTreeSet::new();
        for command in &self.commands {
            if !commands.insert(command.name.as_str()) {
                return Err(format!("Duplicate heading command ‘\\{}’", command.name));
            }
            if !names.contains(command.heading.as_str()) {
                return Err(format!(
                    "Unknown heading kind ‘{}’ of heading command ‘\\{}’",
                    command.heading, command.name
                ));
            }
            for (key, value) in &command.defaults {
                if key != NUMBERED_KEY && key != TOC_KEY {
                    return Err(format!(
                        "Invalid default option ‘{key}’ of heading command ‘\\{}’: default \
                         options must be ‘{NUMBERED_KEY}’ or ‘{TOC_KEY}’",
                        command.name
                    ));
                }
                if boolean(value).is_none() {
                    return Err(format!(
                        "Invalid default option ‘{key}={value}’ of heading command ‘\\{}’: the \
                         value must be ‘true’ or ‘false’",
                        command.name
                    ));
                }
            }
        }
        Ok(())
    }

    /// Returns the configuration encoded as bytes. The `headings` feature adds
    /// these bytes to the registry digest ([`crate::model::digest`]), a number
    /// with which a program can tell whether two registries produce the same
    /// output. The [`Hash`] implementation of `HeadingsConfig` also hashes these
    /// bytes.
    ///
    /// The bytes encode every field of every heading kind and of every heading
    /// command, in the order of the lists, except the descriptions of the
    /// heading kinds ([`HeadingDecl::docs`]). A description has no effect on a
    /// parse or a render, so a change of a description does not change the
    /// registry digest.
    ///
    /// Two configurations give the same bytes if and only if they are equal
    /// apart from the descriptions. For instance, two configurations that list
    /// the same heading kinds in different orders give different bytes.
    pub fn digest_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        push_len(&mut out, self.headings.len());
        for kind in &self.headings {
            push_text(&mut out, kind.name.as_str());
            out.push(kind.level);
            push_opt_text(&mut out, kind.counter.as_ref().map(|name| name.as_str()));
            push_opt_text(&mut out, kind.style.as_ref().map(|name| name.as_str()));
            match &kind.within {
                None => out.push(0),
                Some(within) => {
                    out.push(1);
                    within.digest_into(&mut out);
                }
            }
            match &kind.number_format {
                NumberTemplate::Formats(text) => {
                    out.push(0);
                    push_text(&mut out, text);
                }
                NumberTemplate::Initial(text) => {
                    out.push(1);
                    push_text(&mut out, text);
                }
            }
            push_text(&mut out, &kind.title_number_format);
            out.push(u8::from(kind.skip_toc));
            out.push(u8::from(kind.run_in));
            push_text(&mut out, kind.ref_kind.as_str());
            push_len(&mut out, kind.label_types.len());
            for label in &kind.label_types {
                push_text(&mut out, label);
            }
            push_text(&mut out, &kind.anchor_prefix);
        }
        push_len(&mut out, self.commands.len());
        for command in &self.commands {
            push_text(&mut out, &command.name);
            push_text(&mut out, command.heading.as_str());
            push_len(&mut out, command.defaults.len());
            for (key, value) in &command.defaults {
                push_text(&mut out, key);
                push_text(&mut out, value);
            }
        }
        match self.numbering {
            HeadingNumbering::NothingNumbered => out.push(0),
            HeadingNumbering::AllNumbered => out.push(1),
            HeadingNumbering::UpToDepth(depth) => {
                out.push(2);
                out.push(depth);
            }
        }
        out
    }
}

/// Appends `len` to `out` as eight little-endian bytes.
fn push_len(out: &mut Vec<u8>, len: usize) {
    out.extend_from_slice(&(len as u64).to_le_bytes());
}

/// Appends `text` to `out`: first its length in bytes ([`push_len`]), then its
/// bytes.
fn push_text(out: &mut Vec<u8>, text: &str) {
    push_len(out, text.len());
    out.extend_from_slice(text.as_bytes());
}

/// Appends `text` to `out`: the byte 0 if `text` is `None`, and otherwise the
/// byte 1 followed by the text ([`push_text`]).
fn push_opt_text(out: &mut Vec<u8>, text: Option<&str>) {
    match text {
        None => out.push(0),
        Some(text) => {
            out.push(1);
            push_text(out, text);
        }
    }
}

impl Hash for HeadingsConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.digest_bytes());
    }
}

// --- the element ----------------------------------------------------------------------------

/// The data of the element value of one heading kind (see [the
/// element](self#the-element)). It contains the declaration of the heading kind,
/// its heading commands, and whether the heading kind is numbered.
///
/// All heading commands of a heading kind share one element value. A lowering
/// rule obtains the `HeadingElement` of a heading with [`element_value`].
/// `HeadingElement` also implements the [`Sectioning`] and [`Referenceable`]
/// aspects of the element value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingElement {
    /// The declaration of the heading kind.
    pub decl: HeadingDecl,
    /// The heading commands of the heading kind, in the order of
    /// [`HeadingsConfig::commands`].
    pub commands: Vec<HeadingCommand>,
    /// Whether the heading kind is numbered under the configuration of the
    /// `headings` feature ([`HeadingsConfig::numbers`]).
    pub numbered: bool,
}

impl HeadingElement {
    /// Returns the heading command with which the heading `node` is written.
    ///
    /// Returns `None` if the node has no name, for instance a heading that a
    /// program created, or if no heading command of this heading kind has the
    /// name of the node.
    pub fn command_of(&self, node: &NodeRef<'_, Flm>) -> Option<&HeadingCommand> {
        let name = node.name()?;
        self.commands.iter().find(|command| command.name == name)
    }

    /// Returns the options of the heading `node`. This method calls
    /// [`heading_options`] with the heading command of the node
    /// ([`command_of`](HeadingElement::command_of)).
    pub fn occurrence(&self, node: &NodeRef<'_, Flm>) -> HeadingOccurrence {
        heading_options(node, self.command_of(node))
    }
}

/// The options of one heading: whether the heading is numbered, and whether it
/// is listed in a table of contents. [`heading_options`] returns these options
/// (see [the options of a heading](self#the-options-of-a-heading)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadingOccurrence {
    /// The option `numbered`. The heading is numbered only if this option is
    /// `true` and its heading kind is numbered.
    pub numbered: bool,
    /// The option `toc`. The heading is listed in a table of contents only if
    /// this option is `true` and its heading kind is not declared with
    /// [`skip_toc`](HeadingDecl::skip_toc).
    pub toc: bool,
}

/// Returns whether the heading `node` is written with a star, as in
/// `\section*{…}`. Returns `false` for a node without a star argument.
///
/// Writing a star is equivalent to writing `numbered=false`.
/// [`heading_options`] takes the star into account.
pub fn starred(node: &NodeRef<'_, Flm>) -> bool {
    node.arguments()
        .and_then(|args| args.get_named(STAR_ARG))
        .is_some_and(|arg| arg.is_provided())
}

/// Returns the options of the heading `node`, which is written with the heading
/// command `command`.
///
/// The options are read, in this order, from the default options of `command`,
/// from the star, and from the key-value argument of the heading. A later value
/// replaces an earlier one (see [the options of a
/// heading](self#the-options-of-a-heading)). Pass `None` as `command` for a
/// heading without a heading command, for instance a heading that a program
/// created; no default options are then applied. [`HeadingElement::occurrence`]
/// finds the heading command of a node and calls this function.
///
/// This function reports no diagnostic. It ignores every key other than
/// `numbered` and `toc`, and every value that it cannot read. The lowering rules
/// of headings report these problems.
pub fn heading_options(
    node: &NodeRef<'_, Flm>,
    command: Option<&HeadingCommand>,
) -> HeadingOccurrence {
    read_options(node, command).0
}

/// Returns the alternative title of the heading `node`, which is the value of
/// the key `alt` ([`ALT_KEY`]) in the key-value argument of the heading.
///
/// The alternative title is returned as a list of [`WrittenPiece`]s, which
/// refer to the nodes and to the characters of the document
/// ([`key_value_pieces`]). Whitespace at the start and at the end of the value
/// is left out. If the value is a single group, the content of the group is
/// returned, as in LaTeX's key-value lists. For instance, for
/// `alt={Short \emph{title}}` the function returns the text `Short ` and the
/// `\emph` node, and for `alt={}` it returns an empty list. If the key `alt` is
/// given several times, the last `alt` with a value is used.
///
/// Returns `None` if the heading has no key `alt` with a value, or if the
/// key-value argument cannot be read. This function reports no diagnostic; the
/// lowering rules of headings report the problems in the key-value argument.
/// The aspect method [`Sectioning::toc_title`] of a heading returns a copy of
/// the alternative title.
pub fn heading_alt<'t>(node: &NodeRef<'t, Flm>) -> Option<Vec<WrittenPiece<'t>>> {
    let content = node.argument_content_nodes_named(OPTIONS_ARG).ok().flatten()?;
    let keyvals = parse_keyval_drop_annotations(content).ok()?;
    let entry = keyvals.iter().filter(|entry| entry.key() == ALT_KEY && entry.value().is_some()).last()?;
    let pieces = key_value_pieces(&content, entry.value()?);
    // A value written as one group, whitespace around it dropped, is the
    // group's content: the group's children, trimmed the same way.
    match pieces.as_slice() {
        [WrittenPiece::Node(group)] if group.is_group() => {
            Some(key_value_pieces(&group.children(), group.children()))
        }
        _ => Some(pieces),
    }
}

/// A problem in the key-value argument of a heading, found by
/// [`read_options`]. Each variant except `Placeholder` corresponds to a
/// diagnostic condition.
#[derive(Debug, Clone, PartialEq, Eq)]
enum OptionProblem {
    /// A reserved key, with the key ([`KeyNotImplemented`]).
    NotImplemented(String),
    /// The key [`ALT_KEY`] without a value, with the key ([`MissingValue`]).
    MissingValue(String),
    /// An unknown key, with the key ([`UnknownKey`]).
    Unknown(String),
    /// A value of `numbered` or `toc` other than `true` and `false`, with the
    /// key and the value ([`BadValue`]).
    BadValue(String, String),
    /// A key that is not plain text ([`UnreadableOptions`]).
    Unreadable,
    /// A value of `numbered` or `toc` that contains a placeholder of a user
    /// definition, lowered without expansion. The characters of the value are
    /// known only at a use site of the definition
    /// ([`LowerError::PlaceholderCharacters`]).
    Placeholder,
}

/// Returns `Some(true)` for the text `true`, `Some(false)` for the text
/// `false`, and `None` for any other text.
fn boolean(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Sets the option `key` of `occurrence` to `value`, if `key` is
/// [`NUMBERED_KEY`] or [`TOC_KEY`]. Any other key leaves `occurrence`
/// unchanged.
fn apply(occurrence: &mut HeadingOccurrence, key: &str, value: bool) {
    if key == NUMBERED_KEY {
        occurrence.numbered = value;
    } else if key == TOC_KEY {
        occurrence.toc = value;
    }
}

/// Reads the options of the heading `node` as [`heading_options`] does, and
/// also returns the problems found in the key-value argument of the heading,
/// in the order in which they are written.
fn read_options(
    node: &NodeRef<'_, Flm>,
    command: Option<&HeadingCommand>,
) -> (HeadingOccurrence, Vec<OptionProblem>) {
    let mut occurrence = HeadingOccurrence { numbered: true, toc: true };
    let mut problems = Vec::new();
    // The command's defaults were checked when the registry was built.
    for (key, value) in command.map(|command| command.defaults.as_slice()).unwrap_or_default() {
        if let Some(value) = boolean(value) {
            apply(&mut occurrence, key, value);
        }
    }
    if starred(node) {
        apply(&mut occurrence, NUMBERED_KEY, false);
    }
    let Ok(Some(content)) = node.argument_content_nodes_named(OPTIONS_ARG) else {
        return (occurrence, problems);
    };
    let Ok(keyvals) = parse_keyval_drop_annotations(content) else {
        problems.push(OptionProblem::Unreadable);
        return (occurrence, problems);
    };
    for entry in keyvals.iter() {
        let key = entry.key();
        if key.is_empty() {
            continue;
        }
        if RESERVED_KEYS.contains(&key) {
            problems.push(OptionProblem::NotImplemented(key.to_string()));
            continue;
        }
        // The alternative title is content, read by `heading_alt`; only its
        // absence is a problem here.
        if key == ALT_KEY {
            if entry.value().is_none() {
                problems.push(OptionProblem::MissingValue(key.to_string()));
            }
            continue;
        }
        if key != NUMBERED_KEY && key != TOC_KEY {
            problems.push(OptionProblem::Unknown(key.to_string()));
            continue;
        }
        // A key written without a value means `true`.
        let text = match entry.value_content() {
            None => Some(String::from("true")),
            Some(value) => match chars_at_lowering(value.iter()) {
                Ok(read) => read.ok().map(|text| text.trim_matches(is_space).to_string()),
                Err(_) => {
                    problems.push(OptionProblem::Placeholder);
                    continue;
                }
            },
        };
        match text.as_deref().and_then(boolean) {
            Some(value) => apply(&mut occurrence, key, value),
            None => problems.push(OptionProblem::BadValue(
                key.to_string(),
                text.unwrap_or_else(|| String::from("(not plain text)")),
            )),
        }
    }
    (occurrence, problems)
}

/// The warning that is reported when the key-value argument of a heading
/// contains a reserved key (`flm.heading.key-not-implemented`).
///
/// A reserved key is set aside for an option of headings that is not
/// implemented. The reserved keys are listed in [`RESERVED_KEYS`]: the only
/// reserved key is `pdfbookmark`. Reported by the lowering rule of the heading
/// when it is invoked. The key is ignored, and the heading is otherwise written
/// as usual.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.heading.key-not-implemented",
    message = "Unsupported heading option ‘{key}’"
)]
pub struct KeyNotImplemented {
    /// The key as written.
    pub key: String,
}

/// The error that is reported when the key-value argument of a heading
/// contains an unknown key (`flm.heading.unknown-key`).
///
/// The keys of a heading are `numbered`, `toc` and `alt`, as in
/// `\section[toc=false]{Acknowledgments}`, and the reserved keys of
/// [`RESERVED_KEYS`]. Reported by the lowering rule of the heading when it is
/// invoked. The key is ignored, and the heading is otherwise written as usual.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.heading.unknown-key",
    message = "Unknown heading option ‘{key}’: the heading options are ‘numbered’, ‘toc’ \
               and ‘alt’"
)]
pub struct UnknownKey {
    /// The key as written.
    pub key: String,
}

/// The error that is reported when the option `numbered` or `toc` of a
/// heading has a value other than `true` and `false` (`flm.heading.bad-value`).
///
/// The value of these options must be `true` or `false`, as in
/// `\section[numbered=false]{Notation}`. A key without a value, as in
/// `\section[numbered]{Notation}`, has the value `true`. Reported by the
/// lowering rule of the heading when it is invoked. The key is ignored: the
/// option has the value that was read before the key, from the default options
/// of the heading command, from the star, or from an earlier key.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.heading.bad-value",
    message = "Invalid value ‘{value}’ of heading option ‘{key}’: the value must be ‘true’ \
               or ‘false’"
)]
pub struct BadValue {
    /// The key as written.
    pub key: String,
    /// The value as written, without whitespace at the start and at the end.
    /// If the value is not plain text, this field contains the text
    /// `(not plain text)`.
    pub value: String,
}

/// The error that is reported when the key `alt` of a heading has no value
/// (`flm.heading.missing-value`).
///
/// With the key `alt`, an author gives the heading an alternative title. The
/// key must be given a value, as in `\section[alt={Short title}]{A long title}`.
/// Reported by the lowering rule of the heading when it is invoked. The key is
/// ignored, and the heading is otherwise written as usual.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.heading.missing-value",
    message = "Missing value for heading option ‘{key}’"
)]
pub struct MissingValue {
    /// The key as written.
    pub key: String,
}

/// The error that is reported when a key in the key-value argument of a
/// heading is not plain text (`flm.heading.unreadable-options`).
///
/// Each key must be plain text, as `toc` in `\section[toc=false]{Notation}`.
/// Reported by the lowering rule of the heading when it is invoked. The whole
/// key-value argument is ignored: the options of the heading are read only from
/// the default options of the heading command and from the star.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.heading.unreadable-options",
    message = "Invalid heading options: each key must be plain text"
)]
pub struct UnreadableOptions;

/// Reports one diagnostic for each problem in the key-value argument of the
/// heading `node`, in the order in which the problems are written. `value` is
/// the element value of the heading.
///
/// Both lowering rules of headings, [`RULE`] and [`latex::LATEX_RULE`], call
/// this function. During a render, one of these rules is applied to each
/// heading, so each problem is reported once per render.
///
/// # Errors
///
/// Returns [`LowerError::PlaceholderCharacters`], and reports no diagnostic, if
/// the value of `numbered` or `toc` contains a placeholder of a user definition
/// that is lowered without expansion. The value is known only at the use sites
/// of the definition. This error causes the use sites of the definition to be
/// expanded, and the heading is then lowered at each use site.
fn report_option_problems(
    node: &NodeRef<'_, Flm>,
    value: &HeadingElement,
    lw: &mut dyn Lowerer,
) -> Result<(), LowerError> {
    let (_, problems) = read_options(node, value.command_of(node));
    if problems.contains(&OptionProblem::Placeholder) {
        return Err(LowerError::PlaceholderCharacters);
    }
    for problem in problems {
        let span = node.span().clone();
        lw.diagnose(match problem {
            OptionProblem::NotImplemented(key) => {
                Diagnostic::warning(KeyNotImplemented { key }, span)
            }
            OptionProblem::MissingValue(key) => Diagnostic::error(MissingValue { key }, span),
            OptionProblem::Unknown(key) => Diagnostic::error(UnknownKey { key }, span),
            OptionProblem::BadValue(key, value) => {
                Diagnostic::error(BadValue { key, value }, span)
            }
            OptionProblem::Unreadable => Diagnostic::error(UnreadableOptions, span),
            OptionProblem::Placeholder => continue,
        });
    }
    Ok(())
}

impl Sectioning for HeadingElement {
    fn level(&self, _node: &NodeRef<'_, Flm>) -> u32 {
        u32::from(self.decl.level)
    }

    fn in_toc(&self, node: &NodeRef<'_, Flm>) -> bool {
        !self.decl.skip_toc && self.occurrence(node).toc
    }

    /// Returns the alternative title of the heading ([`heading_alt`]) if the
    /// heading has one, and the title argument otherwise.
    ///
    /// The alternative title is copied into a synthesized value, as content
    /// that is displayed a second time ([`ValueContent::Shown`]). For instance,
    /// a footnote in the alternative title is copied as its mark only. Returns
    /// `None` if the copy fails, which happens only for content nested deeper
    /// than the [recursion guard](crate::lang::guard) allows. No diagnostic is
    /// reported in this case.
    fn toc_title(&self, node: &NodeRef<'_, Flm>) -> Option<ContentRef> {
        let Some(alt) = heading_alt(node) else {
            return Some(ContentRef::ArgumentNamed(TITLE_ARG.to_string()));
        };
        let pieces: Vec<ValuePiece<'_, '_>> = alt.iter().map(WrittenPiece::as_value_piece).collect();
        on_worker_thread(|| {
            copy_pieces_into_value(&pieces, node, ValueContent::Shown, GuardPolicy::StackBudget)
        })
        .ok()?
        .ok()
        .map(ContentRef::Synthesized)
    }
}

impl Referenceable for HeadingElement {
    fn points(&self, node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        let (labels, malformed) = refs::point_labels(refs::label_uses(node).iter());
        let anchor = if labels.is_empty() {
            AnchorPolicy::Slug {
                prefix: self.decl.anchor_prefix.clone(),
                content: ContentRef::ArgumentNamed(TITLE_ARG.to_string()),
            }
        } else {
            AnchorPolicy::FromFirstLabel
        };
        // A heading of a kind the depth leaves unnumbered, or one that opts
        // out — a star, or `numbered=false` — states no number source, so it
        // advances nothing and the headings after it go on under the last
        // number given. A numbered one counts in its kind's counter, is shown
        // in its kind's style, and advances the number itself — level 0 — by
        // one.
        let numbered = self.numbered && self.occurrence(node).numbered;
        let number = if numbered { self.decl.number_source() } else { None };
        vec![RefPoint {
            kind: self.decl.ref_kind.clone(),
            labels,
            malformed,
            computed: 0,
            anchor,
            number,
            content: Some(ContentRef::ArgumentNamed(TITLE_ARG.to_string())),
            accepts: AcceptedRefTypes::these(self.decl.label_types.iter().map(String::as_str)),
        }]
    }
}

/// Returns the block role of the headings of `kind`: [`BlockRole::Block`], or
/// for a run-in heading kind [`BlockRole::BlockDecorator`] at
/// [`Placement::Leads`], a decorator at the start of the paragraph that the
/// heading introduces.
fn block_role(kind: &HeadingDecl) -> BlockRole {
    if kind.run_in { BlockRole::BlockDecorator(Placement::Leads) } else { BlockRole::Block }
}

/// Returns the description of the heading kind `kind`, for the description of
/// its element value: the field [`docs`](HeadingDecl::docs) of `kind`, or
/// `A heading of level <level>.` if this field is empty.
fn kind_docs(kind: &HeadingDecl) -> String {
    if kind.docs.is_empty() { format!("A heading of level {}.", kind.level) } else { kind.docs.clone() }
}

/// Returns the description of the heading commands of the heading kind
/// `kind`: `Starts a heading of level <level>.` The field
/// [`docs`](HeadingDecl::docs) of `kind` is used only in the description of
/// the element value ([`kind_docs`]).
fn command_docs(kind: &HeadingDecl) -> String {
    format!("Starts a heading of level {}.", kind.level)
}

/// Returns the element value of the heading kind `kind`, an element of type
/// [`HEADING`]. Its value is a [`HeadingElement`] that contains `kind`,
/// `commands` and `numbered`. The element value has the block role of the
/// heading kind ([`block_role`]) and the [`Sectioning`] and [`Referenceable`]
/// aspects.
fn element(kind: &HeadingDecl, commands: Vec<HeadingCommand>, numbered: bool) -> Arc<ElementSpec> {
    let value = Arc::new(HeadingElement { decl: kind.clone(), commands, numbered });
    Arc::new(
        ElementSpec::new(HEADING)
            .docs(format!("`{}`: {}", kind.name, kind_docs(kind)))
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(block_role(kind))))
            .with::<dyn Sectioning>(Arc::clone(&value) as Arc<dyn Sectioning>)
            .with::<dyn Referenceable>(Arc::clone(&value) as Arc<dyn Referenceable>)
            .with_value(value),
    )
}

/// Returns the arguments of a heading command, in this order: the star, the
/// optional key-value argument, the title, and a trailing argument that
/// contains the `\label` commands written after the title (see
/// [fields](crate::guide::concepts_overview#fields-data-a-construct-reads)).
///
/// Use this list for a command of your own feature that is written like a
/// heading command. For a command with other arguments that also accepts
/// `\label` commands after it, declare the trailing argument with
/// [`label_fields_arg`](refs::label_fields_arg).
pub fn heading_args() -> Vec<ArgDef> {
    vec![
        ArgDef::named(STAR_ARG, ArgStdKind::Star)
            .docs("With a star, the heading is not numbered, as with `numbered=false`."),
        ArgDef::named(OPTIONS_ARG, ArgStdKind::KeyVals).docs(
            "The options of the heading, as key-value pairs: `numbered=true|false`, \
             `toc=true|false`, and `alt={…}` for the title in a table of contents.",
        ),
        ArgDef::mandatory(TITLE_ARG).docs("The heading's title."),
        refs::label_fields_arg(),
    ]
}

/// The lowering rule [`RULE`] of headings, which is used on every backend except
/// `latex-recomposed` (see [the lowering rules](self#the-lowering-rules)).
///
/// The rule lowers a heading to a
/// [`flm.pres.block-heading`](crate::stdlib::backends::pres::BLOCK_HEADING)
/// node. The payload of this node ([`HeadingPayload`]) contains the name and the
/// level of the heading kind, whether the heading is listed in a table of
/// contents, the anchor of the heading, and the number of the heading as plain
/// text. The content of the node is the title of the heading, preceded by the
/// number if the heading is numbered.
///
/// If the heading has an alternative title (`alt=`, [`heading_alt`]), the rule
/// also lowers the alternative title, into the slot [`ALT_SLOT`](pres::ALT_SLOT)
/// of the node. The title is in the slot [`BODY_SLOT`](pres::BODY_SLOT). The
/// `latex-rendered` backend writes the alternative title as the key `alt` of the
/// heading. The `html` and `text` backends write only the title.
///
/// The rule lowers a run-in heading to a
/// [`flm.pres.deco.runin`](crate::stdlib::backends::pres::DECO_RUNIN)
/// decoration. The payload of a decoration is empty. Therefore, the decoration
/// contains a
/// [`flm.pres.runin-heading`](crate::stdlib::backends::pres::RUNIN_HEADING)
/// node, with the same payload and the same content as a block heading.
///
/// The rule also reports the problems in the key-value argument of the heading
/// (see [the options of a heading](self#the-options-of-a-heading)).
///
/// A `HeadingRule` can be constructed only in the `headings` feature, because
/// the rule contains the compiled heading title templates of the heading kinds
/// ([`HeadingDecl::title_number_format`]).
#[derive(Debug, Clone)]
pub struct HeadingRule {
    titles: Arc<Titles>,
}

impl HeadingRule {
    /// Builds the nodes written before the title of a heading in the lowered
    /// tree (see [`Lowerer`]), and returns them. The heading has the heading
    /// kind `kind` and the reference point `point`.
    ///
    /// The nodes are the output of the [heading title
    /// template](HeadingDecl::title_number_format) of the heading kind, in which
    /// each `#{number}` is replaced with the number of the heading. The number is
    /// placed in a span with the role [`NUMBER_ROLE`](numbers::NUMBER_ROLE). The
    /// number is formatted without the word of its counter style (such as `§`),
    /// which belongs to a reference to the heading, and without delimiters.
    ///
    /// Returns an empty list if the heading has no number. This is the case for
    /// a heading that is not numbered, for every heading of a heading kind that
    /// is not numbered ([`HeadingsConfig::numbering`]), and for every heading in
    /// a registry without the `numbering` feature.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError`] if the counter table cannot be read, if a node
    /// cannot be built, if the heading title template of the heading kind is not
    /// compiled, or if the number cannot be formatted from the counter record.
    /// The last case is a fault in the program: the numbering stage refuses a
    /// counter record that its counter style cannot format.
    fn number(
        &self,
        kind: &Name,
        point: PointId,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Vec<BuildId>, LowerError> {
        // The table is absent in a registry holding no numbering feature, and
        // holds no record for a heading whose counter this document switched
        // off; both are a heading with no number. The number is staged inside
        // the heading's content rather than in its payload, so that a backend
        // measuring the heading measures it.
        let options = FormatOptions::new().without_word().without_delimiters();
        let written = {
            let Some(counters) = lw.get(COUNTERS)? else { return Ok(Vec::new()) };
            if counters.record(point).is_none() {
                return Ok(Vec::new());
            }
            title::title_number(&self.titles, kind, |keys| {
                counters
                    .format_with_keys(point, keys, &options)
                    .map(|result| result.map_err(|error| error.to_string()))
            })?
        };
        let Some(written) = written else { return Ok(Vec::new()) };
        lw.splice(cx, &written)
    }
}

impl Lower for HeadingRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<HeadingElement>(&node)
            .ok_or_else(|| LowerError::structure("a heading without its element's data"))?;
        report_option_problems(&node, value, lw)?;
        let point = PointId { addr: lw.addr(), index: 0 };
        // The table itself is absent in a registry that holds no feature
        // computing anchors: the heading is printed with no anchor, and the
        // registry build's Note about the unresolved declaration is the whole
        // report. With the table present, `Some(None)` is a heading the
        // anchors stage saw and gave no anchor, because another point had
        // claimed the same one: the heading is printed, unlinkable. `None` is
        // a heading the stage never saw, which is the access-site error.
        let anchor: Option<String> = match lw.get(ANCHORS)? {
            None => None,
            Some(anchors) => {
                let Some(entry) = anchors.lookup(point) else {
                    let table = CapName::from(ANCHORS.name());
                    let producer = lw
                        .producer_of(&table)
                        .map(|stage| stage.to_string())
                        .unwrap_or_else(|| "nobody".to_string());
                    lw.diagnose(Diagnostic::error(
                        MissingEntry {
                            table: table.to_string(),
                            producer,
                            element: HEADING.to_string(),
                        },
                        node.span().clone(),
                    ));
                    return Err(LowerError::structure("the heading has no anchor entry"));
                };
                entry.map(String::from)
            }
        };
        self.write(&node, value, point, anchor, cx, lw)
    }
}

impl HeadingRule {
    /// Lowers the heading `node` as described in [`HeadingRule`]. `value` is the
    /// element value of the heading, `point` its reference point, and `anchor`
    /// its anchor.
    fn write(
        &self,
        node: &NodeRef<'_, Flm>,
        value: &HeadingElement,
        point: PointId,
        anchor: Option<String>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let level = value.decl.level;
        // A title a parse recovery left absent lowers to nothing: an empty
        // heading, and no second report of what the parse already reported.
        // The number, where this document gave the heading one, stands before
        // the title inside the heading's own content.
        let mut body = self.number(&value.decl.name, point, cx, lw)?;
        body.extend(lower_arg_named_or_nothing(cx, lw, node, TITLE_ARG)?);
        let alt = match heading_alt(node) {
            Some(pieces) => Some(lower_written_pieces(cx, lw, &pieces)?),
            None => None,
        };
        let payload = HeadingPayload {
            name: value.decl.name.clone(),
            level,
            in_toc: value.in_toc(node),
            anchor,
            number: number_text(point, lw)?,
        };
        lw.require(&Name::from(STYLE));
        let element = ElementName::from(if value.decl.run_in {
            pres::RUNIN_HEADING
        } else {
            pres::BLOCK_HEADING
        });
        let heading = match alt {
            None => lw.pres_with(cx, &element, payload, body)?,
            Some(alt) => lw.pres_slots_with(
                cx,
                &element,
                payload,
                vec![(String::from(pres::BODY_SLOT), body), (String::from(pres::ALT_SLOT), alt)],
            )?,
        };
        if value.decl.run_in {
            let title = heading;
            let deco = lw.pres_with(
                cx,
                &ElementName::from(pres::DECO_RUNIN),
                DecorationPayload,
                vec![title],
            )?;
            return Ok(Lowering::Emit(vec![deco]));
        }
        Ok(Lowering::Emit(vec![heading]))
    }
}

/// Returns the number of the heading with the reference point `point` as plain
/// text, for the payload of the heading.
///
/// The number is formatted without the word and the delimiters of its counter
/// style, and converted to plain text with [`plain_text`](pres::plain_text).
/// Returns `None` if the heading has no number, that is, if the registry has no
/// counter table, or if the counter table has no number for `point`.
///
/// # Errors
///
/// Returns [`LowerError`] if the counter table cannot be read, or if the number
/// cannot be formatted from the counter record. The second case is a fault in
/// the program, as for [`HeadingRule::number`].
fn number_text(point: PointId, lw: &mut dyn Lowerer) -> Result<Option<String>, LowerError> {
    let options = FormatOptions::new().without_word().without_delimiters();
    let Some(counters) = lw.get(COUNTERS)? else { return Ok(None) };
    let Some(segments) = counters.format(point, &options) else { return Ok(None) };
    let segments = segments.map_err(|error| LowerError::other(error.to_string()))?;
    let text = segments
        .iter()
        .map(|segment| pres::plain_text(segment.content().root().children().iter()))
        .collect::<String>();
    Ok(Some(text))
}

// --- the feature ----------------------------------------------------------------------------

/// The `headings` feature, which provides the headings of a document (see the
/// [module documentation](self)). The feature is constructed from a
/// [`HeadingsConfig`].
///
/// The feature contributes the following items to the registry:
///
/// - one element value of type [`HEADING`] for each heading kind of the
///   configuration, with the names of the heading commands of the kind;
/// - the lowering rules of headings, [`RULE`] and [`latex::LATEX_RULE`];
/// - the element [`title::NUMBER_ELEMENT`], which contains the number of a
///   heading in its title, with its lowering rules;
/// - the counters, the counter styles and the parent set of the heading kinds
///   ([`numbers`]);
/// - a late-initialization hook ([`LateInit`](crate::core::registry::LateInit))
///   that compiles the heading title templates of the heading kinds.
#[derive(Debug, Default, Clone)]
pub struct Headings {
    config: HeadingsConfig,
}

impl Headings {
    /// Returns the `headings` feature with the configuration `config`.
    pub fn new(config: HeadingsConfig) -> Headings {
        Headings { config }
    }

    /// Returns the configuration of this feature.
    pub fn config(&self) -> &HeadingsConfig {
        &self.config
    }
}

impl FlmFeature for Headings {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Headings").docs(
            "The headings of a document, such as sections and subsections, written with the \
             heading commands `\\section` to `\\subparagraph` by default. Each heading kind has \
             a level. A heading can be numbered, listed in a table of contents, and referred \
             to with a label.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        self.config
            .validate()
            .map_err(|detail| BuildError::Feature { feature: owner.clone(), detail })?;
        // The `\label` field of a heading command is the references
        // feature's, with that feature's role on its argument; declared as a
        // peer, so that a registry without that feature still builds.
        b.peer_argument_role(refs::DEFINED_LABEL_ARG_ROLE)?;
        // One value per kind, written under the names of every command of the
        // kind. A kind no command writes still has its value, synthesized (no
        // name, so no document can write it): a program may create its
        // headings itself, for instance in a stage.
        for kind in &self.config.headings {
            let commands: Vec<HeadingCommand> =
                self.config.commands_of(kind.name.as_str()).cloned().collect();
            let names: Vec<String> = commands.iter().map(|command| command.name.clone()).collect();
            let (syntax, docs) = if names.is_empty() {
                let docs = format!(
                    "`{}`: {} This heading kind has no heading command; only a program can \
                     create its headings.",
                    kind.name,
                    kind_docs(kind)
                );
                (SyntaxDef::Synthesized, docs)
            } else {
                (SyntaxDef::Macro { args: heading_args() }, command_docs(kind))
            };
            b.element(
                ElementDef::new(
                    owner.clone(),
                    names,
                    syntax,
                    element(kind, commands, self.config.numbers(kind)),
                )
                .docs(docs),
            )?;
        }
        // The anchors table belongs to the references feature, so the rule
        // declares it as a peer table and reads it optionally: without that
        // feature the rule still runs and the heading is printed with no
        // anchor.
        b.peer_table::<refs::Anchors>(
            ANCHORS.name(),
            "the anchors table of the `refs` feature, from which the lowering rule of \
             headings reads the anchor of each heading",
        )?;
        // The counter table belongs to the numbering feature, which may not be
        // there either; the rule then shows no number. The heading counters
        // and their styles are declared into that feature's slots the same
        // way, as peer contributions.
        b.peer_table::<Counters>(
            COUNTERS.name(),
            "the counter table of the `numbering` feature, from which the lowering rule of \
             headings reads the number of each heading",
        )?;
        numbers::declare(b, &self.config)?;
        // The number node a kind's title number format puts where it writes
        // `#{number}`: synthesized, so no document can write it, and lowered
        // by a rule of its own to the span of the number's role. The rule is
        // registered whatever the configuration says, so the span is produced
        // at any level.
        let number_element = title::number_element();
        b.element(
            ElementDef::new(
                owner.clone(),
                Vec::<&str>::new(),
                SyntaxDef::Synthesized,
                Arc::clone(&number_element),
            )
            .docs(
                "The number of a heading, placed in its title where the heading title template \
                 of the heading kind contains `#{number}`. This element cannot be written in a \
                 document; the lowering rule of the heading inserts it.",
            ),
        )?;
        b.element_type_text(
            HEADING,
            "A heading that starts a part of the document, such as a section or a subsection. \
             A heading has the level of its heading kind, and it can be numbered.",
        )?;
        b.element_type_text(
            title::NUMBER_ELEMENT,
            "The number of a heading, placed in its title where the heading title template of \
             the heading kind contains `#{number}`.",
        )?;
        b.lowering_rule(
            LoweringRule::new(
                title::NUMBER_RULE,
                title::NUMBER_ELEMENT,
                LoweringDecl::new().produces(pres::SPAN),
                Arc::new(NumberRule),
                DefaultMark::Everywhere,
            )
            .docs(
                "the number of a heading lowers to a `flm.pres.span` node with the role \
                 `flm.heading.number` that contains the number",
            ),
        )?;
        // Every kind's title number format is compiled once the language is
        // built, and the heading rule reads the compiled templates.
        let titles = Arc::new(Titles::new(
            self.config
                .headings
                .iter()
                .map(|kind| (kind.name.clone(), kind.title_number_format.clone()))
                .collect(),
            number_element,
        ));
        b.late_init(Arc::new(CompileTitles { titles: Arc::clone(&titles) }))?;
        // The decoration and the run-in title inside it are produced only
        // where a kind is set run in, since a registry in which none can occur
        // must not demand that its backend render them.
        let mut decl = LoweringDecl::new()
            .reads_opt(ANCHORS)
            .reads_opt(COUNTERS)
            .produces(pres::BLOCK_HEADING);
        if self.config.headings.iter().any(|kind| kind.run_in) {
            decl = decl.produces(pres::DECO_RUNIN).produces(pres::RUNIN_HEADING);
        }
        b.lowering_rule(
            LoweringRule::new(
                RULE,
                HEADING,
                decl,
                Arc::new(HeadingRule { titles }),
                DefaultMark::Everywhere,
            )
            .docs(
                "a heading lowers to a `flm.pres.block-heading` node that contains the number of \
                 the heading, if the heading is numbered, followed by the title; a run-in \
                 heading lowers to a `flm.pres.deco.runin` decoration that contains a \
                 `flm.pres.runin-heading` node with the same content",
            ),
        )?;
        b.style_def(stylesheet())?;
        b.digest_input(&self.config.digest_bytes())?;
        latex::register(b)?;
        latex_rendered::register(b)?;
        Ok(())
    }
}

/// Returns the [`Headings`] feature with the default configuration
/// ([`HeadingsConfig::new`]), for the list of features of a package entry.
///
/// To use a [`HeadingsConfig`] of your own, list
/// `Arc::new(Headings::new(config))` in place of this feature.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Headings::default())
}

/// In settings data, a heading kind is an entry of the map
/// [`HeadingsConfig::headings`], with the name of the heading kind as its key.
/// The entry must contain the field `level`. The other fields are optional; a
/// field that is left out has the value set by [`HeadingDecl::new`].
impl named::NamedEntry for HeadingDecl {
    fn read_entry(name: &str, data: serde_json::Value) -> Result<HeadingDecl, String> {
        let level = named::required(name, &data, "level")?;
        let mut kind: HeadingDecl = named::read_over(&HeadingDecl::new(name, level), "name", name, data)?;
        // The text read from data goes through the builder, as a text given in
        // Rust does, so that the cargo feature `docs` decides for both.
        let docs = std::mem::take(&mut kind.docs);
        Ok(kind.docs(docs))
    }
}

/// In settings data, a heading command is an entry of the map
/// [`HeadingsConfig::commands`], with the name of the command as its key.
impl named::NamedEntry for HeadingCommand {
    fn read_entry(name: &str, data: serde_json::Value) -> Result<HeadingCommand, String> {
        named::read_with_name("name", name, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heading_value_is_a_block_carrying_its_declaration_and_aspects() {
        let config = HeadingsConfig::new();
        let kind = config.heading("subsection").expect("declared");
        let element = element(kind, config.commands_of("subsection").cloned().collect(), true);
        assert_eq!(element.name, HEADING);
        let value = element.value_as::<HeadingElement>().expect("valued");
        assert_eq!(value.decl.level, 2);
        assert!(value.numbered);
        assert_eq!(value.commands.len(), 1);
        assert!(element.has_aspect::<dyn Sectioning>());
        assert!(element.has_aspect::<dyn Referenceable>());
    }

    #[test]
    fn a_run_in_kind_is_a_leading_decorator_and_every_other_kind_a_block() {
        let config = HeadingsConfig::new().run_in_levels([4]);
        let role = |name: &str| block_role(config.heading(name).expect("declared"));
        assert_eq!(role("paragraph"), BlockRole::BlockDecorator(Placement::Leads));
        assert_eq!(role("subsubsection"), BlockRole::Block);
        let shipped = HeadingsConfig::new();
        assert_eq!(block_role(shipped.heading("paragraph").expect("declared")), BlockRole::Block);
    }

    #[test]
    fn the_digest_bytes_change_with_every_field_but_the_documentation_texts() {
        let bytes = |config: &HeadingsConfig| config.digest_bytes();
        let shipped = bytes(&HeadingsConfig::new());
        assert_eq!(shipped, bytes(&HeadingsConfig::new()));
        assert_ne!(shipped, bytes(&HeadingsConfig::new().run_in_levels([4])));
        let edit = |f: fn(HeadingDecl) -> HeadingDecl| {
            let mut config = HeadingsConfig::new();
            config.headings[0] = f(config.headings[0].clone());
            bytes(&config)
        };
        for changed in [
            edit(|kind| HeadingDecl { level: 7, ..kind }),
            edit(|kind| kind.counter("other")),
            edit(|kind| kind.no_counter()),
            edit(|kind| kind.style("other.style")),
            edit(|kind| kind.within(Within::parent("part"))),
            edit(|kind| kind.within(Within::parents(["part"]))),
            edit(|kind| kind.within(Within::parent_set("part"))),
            edit(|kind| kind.within(Within::parent("part").no_restart())),
            edit(|kind| kind.number_format(NumberTemplate::initial("1"))),
            edit(|kind| kind.title_number_format("#{number} ")),
            edit(|kind| kind.skip_toc(true)),
            edit(|kind| kind.run_in(true)),
            edit(|kind| kind.ref_kind("chapter")),
            edit(|kind| kind.label_types(["sec"])),
            edit(|kind| kind.anchor_prefix("s")),
        ] {
            assert_ne!(shipped, changed);
        }
        // A kind's documentation text is left out.
        assert_eq!(shipped, edit(|kind| HeadingDecl { docs: String::from("Other."), ..kind }));
        let with_default =
            HeadingsConfig::new().command(HeadingCommand::new("q", "section").default_value("toc", "false"));
        let with_other_default =
            HeadingsConfig::new().command(HeadingCommand::new("q", "section").default_value("toc", "true"));
        assert_ne!(bytes(&with_default), bytes(&with_other_default));
        let depths = [
            HeadingNumbering::NothingNumbered,
            HeadingNumbering::AllNumbered,
            HeadingNumbering::UpToDepth(1),
            HeadingNumbering::UpToDepth(2),
        ];
        for (index, depth) in depths.iter().enumerate() {
            for other in &depths[index + 1..] {
                assert_ne!(
                    bytes(&HeadingsConfig::new().numbering(*depth)),
                    bytes(&HeadingsConfig::new().numbering(*other))
                );
            }
        }
    }

    #[test]
    fn the_depth_numbers_the_kinds_with_a_counter_it_admits() {
        let config = HeadingsConfig::new();
        let numbered = |config: &HeadingsConfig| {
            config
                .headings
                .iter()
                .filter(|kind| config.numbers(kind))
                .map(|kind| kind.name.to_string())
                .collect::<Vec<_>>()
        };
        assert!(numbered(&config).is_empty());
        assert_eq!(numbered(&config.clone().numbering(HeadingNumbering::UpToDepth(2))), ["section", "subsection"]);
        assert_eq!(numbered(&config.clone().numbering(HeadingNumbering::AllNumbered)).len(), 5);
        // A kind with no counter is never numbered, whatever the depth.
        let mut kinds = config.headings.clone();
        kinds.push(HeadingDecl::new("aside", 1).no_counter());
        let with_aside = config.headings(kinds).numbering(HeadingNumbering::AllNumbered);
        assert!(!with_aside.numbers(with_aside.heading("aside").expect("declared")));
        // Replacing the kinds keeps the depth.
        assert_eq!(with_aside.numbering, HeadingNumbering::AllNumbered);
    }

    #[test]
    fn the_shipped_kinds_run_from_section_to_subparagraph() {
        let config = HeadingsConfig::new();
        let kinds: Vec<(&str, u8)> =
            config.headings.iter().map(|kind| (kind.name.as_str(), kind.level)).collect();
        assert_eq!(
            kinds,
            [
                ("section", 1),
                ("subsection", 2),
                ("subsubsection", 3),
                ("paragraph", 4),
                ("subparagraph", 5)
            ]
        );
        let commands: Vec<&str> =
            config.commands.iter().map(|command| command.name.as_str()).collect();
        assert_eq!(commands, ["section", "subsection", "subsubsection", "paragraph", "subparagraph"]);
        assert!(config.commands.iter().all(|command| command.defaults.is_empty()));
        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn a_configuration_that_clashes_is_refused() {
        let refused = |config: HeadingsConfig| config.validate().is_err();
        assert!(refused(HeadingsConfig::new().command(HeadingCommand::new("x", "nokind"))));
        let mut kinds = HeadingsConfig::new().headings;
        kinds.push(HeadingDecl::new("section", 1));
        assert!(refused(HeadingsConfig::new().headings(kinds)));
        let twice = HeadingsConfig::new().command(HeadingCommand::new("section", "section"));
        assert!(refused(twice));
        let mut kinds = HeadingsConfig::new().headings;
        kinds.push(HeadingDecl::new("appendix", 1).counter("flm.headings.section"));
        assert!(refused(HeadingsConfig::new().headings(kinds)));
        let mut kinds = HeadingsConfig::new().headings;
        kinds.push(HeadingDecl::new("appendix", 1).style("flm.headings.style.section"));
        assert!(refused(HeadingsConfig::new().headings(kinds)));
        let bad_default = HeadingsConfig::new()
            .command(HeadingCommand::new("q", "section").default_value("alt", "x"));
        assert!(refused(bad_default));
        let bad_value = HeadingsConfig::new()
            .command(HeadingCommand::new("q", "section").default_value("toc", "maybe"));
        assert!(refused(bad_value));
        let mut kinds = HeadingsConfig::new().headings;
        kinds.push(HeadingDecl::new("part", 0));
        let part = HeadingsConfig::new().headings(kinds);
        assert_eq!(part.validate(), Ok(()), "a kind of level 0 is accepted");
        let mut kinds = HeadingsConfig::new().headings;
        kinds.push(HeadingDecl::new("orphan", 2));
        let commands = HeadingsConfig::new().commands;
        let orphan = HeadingsConfig::new().headings(kinds).commands(commands);
        assert_eq!(orphan.validate(), Ok(()), "a kind no command writes is accepted");
    }
}
