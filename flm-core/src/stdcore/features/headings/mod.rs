//! The `headings` feature: the commands that write a document's headings, `\section`
//! to `\subparagraph` by default, and the heading kinds and heading commands a program
//! declares beside them or instead of them.
//!
//! # What an author writes
//!
//! A heading is written as a heading command followed by its title, with optional
//! labels after it:
//!
//! ```text
//! \section{Introduction}\label{sec:intro}
//! \subsection*{Notation}
//! \section[toc=false]{Acknowledgments}
//! ```
//!
//! A star marks a heading as not numbered, and the optional bracket sets the heading's
//! options `numbered`, `toc` and `alt` (see [What an occurrence
//! says](#what-an-occurrence-says-the-star-and-the-key-values) below). A `\label` after
//! the title gives the heading a label that `\ref` can refer to. By default no heading
//! is numbered on the plain-text and HTML outputs; a program chooses which heading
//! levels are numbered there ([`HeadingNumbering`]). The guide to the language
//! describes headings for authors ([headings](crate::guide::language_syntax#headings)).
//!
//! What a heading becomes on each output:
//!
//! - in plain text (the `text` backend), its title on a line of its own, underlined at
//!   the root heading level and one level deeper and preceded by `#` characters at
//!   deeper levels (see [The lowering rules](#the-lowering-rules)); a run-in heading is
//!   its title followed by a period, at the start of its paragraph;
//! - in HTML (the `html` backend), an element `<h1>` to `<h6>` chosen by its level
//!   counted from the root heading level, or the tag that the render option
//!   [`heading_tags`](crate::stdcore::backends::html::HtmlRenderOptions::heading_tags)
//!   maps its heading kind to, with its anchor as `id` and its heading kind, level and
//!   table-of-contents flag as attributes; a run-in heading is a `<span>` with the same
//!   attributes at the start of its paragraph ([headings in
//!   HTML](crate::guide::rendering#headings-in-html));
//! - in the recomposed LaTeX (the `latex-recomposed` backend), the LaTeX sectioning
//!   command its heading kind maps to, inside the LaTeX hook `\flmHeading` (a macro the
//!   style file defines and a user may redefine;
//!   [hooks](crate::stdcore::backends::latex::preamble); see [`latex`]).
//!
//! A numbered heading shows its number before its title (`1.2. Notation`) on the
//! plain-text and HTML outputs. On the recomposed LaTeX, the configuration's numbering
//! depth has no effect: a heading written with a star or with `numbered=false` is
//! written as a starred sectioning command, every other heading as an unstarred one,
//! and LaTeX numbers the unstarred headings as the document class says ([`latex`]).
//!
//! # Heading kinds and heading commands
//!
//! A **heading kind** is what a heading is: a section, a subsection, an appendix. It
//! is declared by name, with a level (a number counted from zero, smaller for outer
//! headings; every value is valid) and everything a heading of that kind needs: the
//! counter it counts in, the counter style its number is shown in, whether it is ever
//! listed in a table of contents, whether it is set as a run-in heading, and what its
//! reference point accepts. The declaration is a [`HeadingDecl`].
//!
//! A **heading command** is a macro an author writes, such as `\section`, that writes
//! a heading of one kind. It is declared separately from the kind, as a
//! [`HeadingCommand`], because several commands may write one kind: a second command
//! writing `section` can give its headings other defaults for the options of the
//! bracket (`toc=false`, for example) and is otherwise the same as `\section`. A kind
//! may also have no command at all: a program then creates its headings itself, for
//! instance in a stage.
//!
//! Both are part of the feature's configuration, [`HeadingsConfig`], which the feature
//! is constructed from ([`Headings::new`]) and which a program passes through
//! [`StdcoreConfig::headings`](crate::stdcore::StdcoreConfig::headings). A feature's
//! configuration is fixed when the registry is built (see [settings, options and
//! configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
//! The default configuration declares the five kinds `section`, `subsection`,
//! `subsubsection`, `paragraph` and `subparagraph`, at levels 1 to 5 as in LaTeX, with
//! one command each of the same name and no defaults ([`HeadingsConfig::new`]). A
//! program declares kinds and commands of its own through the same type:
//!
//! ```
//! use flm_core::stdcore::features::headings::{HeadingCommand, HeadingDecl, HeadingsConfig};
//! use flm_core::stdcore::features::numbering::NumberTemplate;
//!
//! // The five standard kinds, an `\appendix` numbered A, B, C, and a second
//! // command writing a section that stays out of the table of contents.
//! let mut kinds = HeadingsConfig::new().headings;
//! kinds.push(HeadingDecl::new("appendix", 1).number_format(NumberTemplate::formats("#{Alph}")));
//! let config = HeadingsConfig::new()
//!     .headings(kinds)
//!     .command(HeadingCommand::new("quietsection", "section").default_value("toc", "false"));
//! assert_eq!(config.heading("appendix").map(|kind| kind.level), Some(1));
//! assert_eq!(config.commands_of("section").count(), 2);
//! ```
//!
//! In LaTeX `\appendix` is a switch and not a sectioning command, so a program that
//! declares an `appendix` kind maps it to a sectioning command for the recomposed
//! LaTeX with the render option [`HeadingsLatexOptions`](latex::HeadingsLatexOptions).
//! The extension guide shows a complete example ([heading kinds and commands of your
//! own](crate::guide::extending#heading-kinds-and-commands-of-your-own)).
//!
//! # What an occurrence says: the star and the key-values
//!
//! One heading as written is an **occurrence**. What it says about itself is a
//! [`HeadingOccurrence`], two facts that [`heading_options`] reads:
//!
//! - `numbered`: whether the heading takes a number, where its kind is numbered at
//!   all. The star (`\section*{…}`) means exactly `numbered=false`, and nothing else.
//! - `toc`: whether the heading is listed in a table of contents. It is `true` unless
//!   `toc=false` is written, regardless of the star: a `\section*` is still listed, and
//!   `\section*[toc=false]{…}` is neither numbered nor listed. A kind declared with
//!   [`skip_toc`](HeadingDecl::skip_toc) is never listed; a `toc` written on one of its
//!   headings changes nothing and is not reported.
//!
//! Both facts are `true` unless something says otherwise. They are read in this order,
//! each later entry replacing an earlier one: the defaults of the command the heading
//! was written with, then the star, then the bracket's key-values in the order they
//! are written. A key written without a value (`[numbered]`) means `true`.
//!
//! A third key, `alt` ([`ALT_KEY`]), gives the heading an **alternative title**, whose
//! value is FLM content: `\section[alt={Short title}]{A long title with \emph{markup}}`.
//! It is what the table of contents of the [`toc`](crate::stdcore::features::toc)
//! feature shows for the heading
//! ([`Sectioning::toc_title`], read with [`heading_alt`]); the heading itself shows its
//! full title where it stands, and a reference to the heading shows the full title too.
//! A later `alt=` replaces an earlier one, as for the other keys; `alt={}` is an empty
//! alternative title. The recomposed LaTeX relays it to LaTeX through the heading hook
//! ([`latex`]). The anchor derived from a title keeps using the full title.
//!
//! The key `pdfbookmark` (the title of a PDF bookmark) is reserved and not implemented
//! yet ([`RESERVED_KEYS`]). The lowering rules report the problems of a heading's
//! bracket:
//!
//! - a reserved key, as the Warning `flm.heading.key-not-implemented`
//!   ([`KeyNotImplemented`]);
//! - `alt` written without a value, as the Error `flm.heading.missing-value`
//!   ([`MissingValue`]);
//! - any other key that is not `numbered`, `toc` or `alt`, as the Error
//!   `flm.heading.unknown-key` ([`UnknownKey`]);
//! - a value of `numbered` or `toc` that is neither `true` nor `false`, as the Error
//!   `flm.heading.bad-value` ([`BadValue`]);
//! - a bracket whose keys are not plain text, as the Error
//!   `flm.heading.unreadable-options` ([`UnreadableOptions`]).
//!
//! In each case the feature goes on with the key, or the whole bracket, ignored, and
//! keeps the rest of the heading (a recovery, see
//! [`lowering::error`](crate::stdcore::backends::lowering::error)). The aspects of the
//! element read what they can and report nothing, so each problem is reported once per
//! render, by the lowering rule that writes the heading.
//!
//! # Numbering
//!
//! A *counter* is a dotted name under which things are counted in document order,
//! declared by a feature together with the counter it is numbered within
//! ([`CounterDecl`](crate::stdcore::features::numbering::CounterDecl)), and a *counter
//! style* says how a counter value is shown
//! ([`CounterStyleDecl`](crate::stdcore::features::numbering::CounterStyleDecl); see
//! [counters and counter
//! styles](crate::guide::concepts_overview#counters-and-counter-styles)). Each standard
//! kind counts in a counter of its own, named after the kind with the prefix
//! [`COUNTER_PREFIX`] (`flm.headings.section`, `flm.headings.subsection`, and so on),
//! and numbered within the kind above it; `paragraph` is numbered within whichever of
//! the three kinds above it came last. The submodule [`numbers`] declares the counters
//! and their counter styles from the kinds' declarations.
//!
//! **Which kinds are numbered is set by the configuration's depth,**
//! [`HeadingsConfig::numbering`], a [`HeadingNumbering`]:
//!
//! - [`NothingNumbered`](HeadingNumbering::NothingNumbered), the default: no heading
//!   carries a number;
//! - [`AllNumbered`](HeadingNumbering::AllNumbered): every kind that has a counter is
//!   numbered;
//! - [`UpToDepth(n)`](HeadingNumbering::UpToDepth): every kind that has a counter and
//!   whose level is `n` or less is numbered.
//!
//! The depth is part of the configuration, so it is fixed when the registry is built:
//!
//! ```
//! use flm_core::stdcore::features::headings::{HeadingNumbering, HeadingsConfig};
//!
//! // Number the sections and the subsections, and leave the three deeper
//! // levels unnumbered.
//! let config = HeadingsConfig::new().numbering(HeadingNumbering::UpToDepth(2));
//! assert!(config.numbers(config.heading("subsection").expect("shipped")));
//! assert!(!config.numbers(config.heading("subsubsection").expect("shipped")));
//! ```
//!
//! A kind the depth leaves unnumbered still declares its counter and its counter
//! style, so that a document's numbering options may number another counter within
//! it. Its headings state no *number source* (the counter and counter style a
//! reference point is numbered by, [`NumberSource`]), so they never advance the
//! counter.
//!
//! A heading that is not numbered (written with a star or with `numbered=false`)
//! states no number source either: it takes no number and advances nothing, and the
//! headings after it continue from the last number given, as in LaTeX. A kind declared
//! without a counter ([`HeadingDecl::no_counter`]) never states one.
//!
//! The feature also declares the *parent set* [`LEVELS_PARENT_SET`]
//! (`flm.headings.levels`): the counters of its kinds, the outermost level first, so
//! that another counter can be numbered within whichever heading came last with
//! [`Within::parent_set`].
//!
//! # Run-in headings
//!
//! A **run-in heading** is a heading set at the start of the paragraph it introduces,
//! on the paragraph's first line, rather than on a line of its own:
//! `\paragraph{Note} Some content.` prints as `Note. Some content.` instead of as a
//! title above the paragraph.
//!
//! The field [`HeadingDecl::run_in`] of each kind says whether its headings are set
//! that way. It is `false` for every standard kind, so by default every heading is a
//! block of its own; [`HeadingsConfig::run_in_levels`] sets it by level.
//!
//! A run-in kind differs from other kinds in two ways only:
//!
//! - Its block role (how the block-structure pass, [`crate::lang::rewrites::blocks`],
//!   lays the node out; [`BlockRoleAspect`]) is [`BlockRole::BlockDecorator`] at
//!   [`Placement::Leads`] instead of
//!   [`BlockRole::Block`], so the block-structure pass opens a paragraph at the heading
//!   and the content that follows the heading joins that paragraph.
//! - [`RULE`] stages (builds into the lowered tree; see
//!   [`Lowerer`]) a
//!   [`flm.pres.deco.runin`](crate::stdcore::backends::pres::DECO_RUNIN) decoration
//!   holding a
//!   [`flm.pres.runin-heading`](crate::stdcore::backends::pres::RUNIN_HEADING),
//!   instead of a
//!   [`flm.pres.block-heading`](crate::stdcore::backends::pres::BLOCK_HEADING). A
//!   decoration is a node that the lowering driver moves out of the content of the
//!   presentation node that holds it into an attached slot of that node
//!   ([`Decoration`](crate::stdcore::backends::pres::Decoration); see [decorations,
//!   roles and kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)). A
//!   backend therefore writes the heading at the start of the paragraph.
//!
//! The [`Sectioning`] and [`Referenceable`] aspects are the same for both: a run-in
//! heading still has its level, still says whether it is listed in a table of
//! contents, and still carries its reference point, so a `\ref` to it resolves and
//! shows its number, or its title where it has none, as for a heading set as a block.
//!
//! A run-in heading followed by no content, for example one followed directly by a
//! blank line, makes a paragraph of its own that holds only the heading. Nothing is
//! dropped and no empty paragraph is produced.
//!
//! # The reference point
//!
//! A *reference point* is one place in a document that can be referred to, numbered,
//! or given an anchor ([`RefPoint`]; see [reference points, labels and
//! anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)).
//! Every heading carries one reference point, whose reference kind (what sort of thing
//! the point is, [`RefKind`]) is the kind's
//! [`ref_kind`](HeadingDecl::ref_kind), [`KIND`] (`section`) by default:
//!
//! - Its labels are the `\label` fields written after the title. The heading reads them
//!   as its own data ([`label_uses`](refs::label_uses)) through its tack-on argument, a
//!   trailing argument that absorbs the field macros written after the construct's
//!   other arguments (see [fields](crate::guide::concepts_overview#fields-data-a-construct-reads)).
//!   A `\label` whose text is not of the form `type:label` defines no label: the point
//!   carries it as a malformed label, and the label check of the `refs` feature reports
//!   it at the field.
//! - The label types it accepts are the kind's [`label_types`](HeadingDecl::label_types),
//!   [`LABEL_TYPES`] (`sec` and `topic`) by default.
//! - A reference to it displays the number the document gave it, else its title.
//! - Its anchor, the identifier a link points at
//!   ([`ANCHORS`]), is derived from its first
//!   label when it has one (`sec-intro`), and otherwise from its title, after the
//!   kind's [`anchor_prefix`](HeadingDecl::anchor_prefix), [`ANCHOR_PREFIX`] by default
//!   (`sec--Introduction`). Where two unlabeled headings have the same title, the
//!   anchors stage of the `refs` feature adds a number to the second
//!   (`sec--Introduction-2`).
//!
//! # The element
//!
//! Every heading is a node of one element type, [`HEADING`] (`flm.heading`). Each kind
//! is one element value of that type, carrying a [`HeadingElement`] (the kind's
//! declaration and the commands that write it), and every command of the kind is a
//! name of that one value; the value of a kind no command writes has no name and is
//! synthesized ([`SyntaxDef::Synthesized`]).
//! Every command takes, in order, a star, an optional
//! key-value argument (`\section[toc=false]{…}`), a title, and the tack-on argument
//! that holds the `\label` fields; [`heading_args`] returns that argument list.
//!
//! The element value carries three aspects: its block role (a block, or for a run-in
//! kind a decorator leading a paragraph); [`Sectioning`], which gives the heading's
//! level and whether it is listed in a table of contents; and [`Referenceable`], which
//! declares its reference point. The two last are computed from the kind's
//! declaration and from the node alone.
//!
//! The feature declares a second element, [`title::NUMBER_ELEMENT`]
//! (`flm.heading.number`), which holds a numbered heading's number inside the text
//! written before its title. No document can write it; see [`title`].
//!
//! # The lowering rules
//!
//! [`RULE`] (`flm.heading.lower.default`), implemented by [`HeadingRule`], is the
//! heading's rule on every backend except `latex-recomposed`. It reads the anchors
//! table [`ANCHORS`] and the counter table [`COUNTERS`] at the heading's own address,
//! lowers the title through the lowering rules, and stages a
//! [`flm.pres.block-heading`](crate::stdcore::backends::pres::BLOCK_HEADING) whose
//! payload ([`HeadingPayload`]) carries the kind's name and level, whether the heading
//! is listed in a table of contents, and the anchor. For a run-in kind it stages the
//! decoration described above instead, with the same payload on the
//! `flm.pres.runin-heading` it holds, because a decoration's own payload holds nothing.
//!
//! Where the document numbered the heading, the content of that presentation node
//! begins with the kind's *heading title template*
//! ([`title_number_format`](HeadingDecl::title_number_format), `#{number}. ` by
//! default), in which the number is a
//! [`flm.pres.span`](crate::stdcore::backends::pres::SPAN) of role
//! [`NUMBER_ROLE`](numbers::NUMBER_ROLE) holding the number without its word, followed
//! by `. ` as text ([`title`] describes the template). Because the number is content
//! of the heading, a backend that measures the heading measures it with the title: the
//! plain-text backend draws the rule under a level-one heading the width of
//! `1. Introduction`, not of `Introduction`. The number is written from the counter
//! table each time it is shown, and is stored nowhere else.
//!
//! The two tables belong to other features, so the rule declares each as a peer
//! declaration ([`PeerName`](crate::core::registry::PeerName)) of a table another
//! feature owns, and reads it optionally (`reads_opt`). With a registry that lacks the
//! `numbering` feature, headings are printed without numbers, and with one that lacks
//! the `refs` feature, without anchors; in both cases nothing is reported at the
//! heading, and the registry build's one Note about the unresolved peer declaration is
//! the only report. An optional read asks the plan for nothing, so the anchors are
//! computed only when something else in the plan requires them, which every resolution
//! of references does.
//!
//! When the anchors table is present, a heading can still lack an anchor in two ways:
//!
//! - The table holds the heading's point *without* an anchor: every label of the
//!   heading belongs to an earlier point or was refused for its type, which the label
//!   check reported. The heading is staged without an anchor and printed normally;
//!   nothing can link to it. An unlabeled heading never has this problem: its title
//!   always gives an anchor.
//! - The table does not hold the heading's point at all. No stage computed an anchor
//!   for it, so the rule is running over content that was not brought up to date (see
//!   [value catch-up](crate::guide::concepts_overview#value-catch-up)); the rule reports
//!   `flm.run.missing-entry` ([`MissingEntry`]) and fails. A heading inside a footnote
//!   is not such a case: it belongs to the document's trees, so the anchors stage gives
//!   it an anchor, and the anchor is written where the list of footnotes shows the
//!   footnote's content.
//!
//! On the plain-text backend, a heading at the root heading level
//! ([`TextRenderOptions::root_heading_level`](crate::stdcore::backends::text::TextRenderOptions::root_heading_level),
//! 1 by default) or one level deeper is printed as an underlined title, and a deeper
//! heading with `#` characters before it; a run-in heading is printed as its title
//! followed by a period and a space, before the first word of the paragraph.
//!
//! [`latex::LATEX_RULE`] (`flm.heading.lower.latex-recomposed`) is the heading's rule on
//! the `latex-recomposed` backend: it writes the heading back as the LaTeX sectioning
//! command its kind maps to ([`HeadingsLatexOptions`](latex::HeadingsLatexOptions)).
//! The feature registers it with peer declarations of the backend and of the LaTeX
//! constructs the rule stages, so that a registry without the LaTeX features still
//! builds, with the rule set aside (excluded from rule selection; see
//! [`PeerName`](crate::core::registry::PeerName)). See [`latex`].
//!
//! # Settings
//!
//! The feature's settings (its configuration and option values written as data; see
//! [`crate::core::settings`]) have two parts: the
//! configuration [`HeadingsConfig`] (the keys `headings`, `commands` and `numbering`,
//! fixed when the registry is built), and the render option
//! [`HeadingsLatexOptions`](latex::HeadingsLatexOptions) (the key `latex_commands`, set
//! per render). In data, a heading kind and a heading command are each written under
//! their name, in the maps `headings` and `commands`; see [`crate::core::settings`].
//!
//! # Interactions with other features
//!
//! - `refs` ([`refs`]): the heading reads its labels with that feature's readers, and
//!   its anchor from the anchors table; on the recomposed LaTeX it writes each label as
//!   its safe label (the key the recomposed LaTeX writes for a label,
//!   [`SafeLabels`](crate::stdcore::features::refs::SafeLabels)).
//! - `numbering` ([`numbering`](crate::stdcore::features::numbering)): the feature
//!   contributes its counters, counter styles and parent set to that feature's service
//!   slots as peer contributions (values for a service slot another feature declares;
//!   see [services and service
//!   slots](crate::guide::concepts_overview#services-and-service-slots) and
//!   [`numbers`]), and reads the number of each heading
//!   from the counter table.
//! - `blocks` ([`blocks`](crate::stdcore::features::blocks)): the block role of a
//!   heading decides how the block-structure pass lays it out.
//! - `userdefs` ([`userdefs`](crate::stdcore::features::userdefs)): a value of `numbered`
//!   or `toc` that a user definition's placeholder supplies cannot be read until the
//!   definition is expanded, so the rules then ask for the definition's use sites to be
//!   expanded ([`LowerError::PlaceholderCharacters`]).
//! - The backends: [`RULE`] requires the feature's style identifier [`STYLE`] at
//!   every heading it stages. On the `html` backend it names an empty style
//!   definition ([`stylesheet`]); on the `latex-rendered` backend the feature's
//!   definition piece ([`latex_rendered::piece`]) delivers it.
//! - The LaTeX family ([`latex`](crate::stdcore::backends::latex)): the rule
//!   [`latex::LATEX_RULE`] requires the definition piece of the LaTeX hook `\flmHeading`
//!   ([`heading_piece`](crate::stdcore::backends::latex::preamble::heading_piece)).
//!
//! # Items
//!
//! - The feature: [`Headings`], [`feature`], [`FEATURE`].
//! - The configuration: [`HeadingsConfig`], [`HeadingDecl`], [`HeadingCommand`],
//!   [`HeadingNumbering`].
//! - The element: [`HEADING`], [`HeadingElement`], [`heading_args`], the argument names
//!   [`STAR_ARG`], [`OPTIONS_ARG`], [`TITLE_ARG`].
//! - Reading an occurrence: [`heading_options`], [`heading_alt`], [`starred`],
//!   [`HeadingOccurrence`], [`NUMBERED_KEY`], [`TOC_KEY`], [`ALT_KEY`], [`RESERVED_KEYS`].
//! - The diagnostics: [`KeyNotImplemented`], [`MissingValue`], [`UnknownKey`],
//!   [`BadValue`], [`UnreadableOptions`].
//! - The defaults of a kind: [`KIND`], [`LABEL_TYPES`], [`ANCHOR_PREFIX`],
//!   [`COUNTER_PREFIX`], [`STYLE_PREFIX`], and the parent set [`LEVELS_PARENT_SET`].
//! - The lowering rule: [`RULE`], [`HeadingRule`]; the style identifier [`STYLE`] and
//!   its definition [`stylesheet`].
//! - Submodules: [`numbers`] (counters and counter styles), [`title`] (the heading title
//!   template and the number element), [`latex`] (the recomposed LaTeX).

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
use crate::stdcore::backends::lowering::{lower_arg_named_or_nothing, lower_written_pieces};
use crate::stdcore::backends::pres::{self, DecorationPayload, HeadingPayload};
use crate::stdcore::features::numbering::{
    COUNTERS, Counters, FormatOptions, NumberTemplate, Within,
};
use crate::stdcore::features::refs::{self, ANCHORS};

use title::{CompileTitles, NumberRule, Titles, DEFAULT_TITLE_NUMBER_FORMAT};

/// The feature's registered name, `headings`.
pub const FEATURE: &str = "headings";

/// Returns the declaration of the feature's settings: the fields of the
/// configuration [`HeadingsConfig`] and of the render option
/// [`HeadingsLatexOptions`](latex::HeadingsLatexOptions) are its keys. See
/// [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<HeadingsConfig>(
            "The heading kinds and the heading commands that write them: each kind's level, \
            counter, counter style, the counter it is numbered within, number format, \
            heading title template, table-of-contents and run-in settings and reference \
            point; each command's heading kind and default options; and which kinds are \
            numbered. Fixed when the registry is built: it cannot vary per document or per \
            render.",
        )
        .options::<latex::HeadingsLatexOptions>(
            Bind::Render,
            "The LaTeX sectioning command each heading kind is written as in the recomposed LaTeX, \
         keyed by the kind's name; a kind not listed is written as the command of its own \
         name. Set per render request.",
        )
        .merge_entries("headings", || named::to_data(&HeadingsConfig::default().headings, "name"))
        .merge_entries("commands", || named::to_data(&HeadingsConfig::default().commands, "name"))
        .markup("headings.*.within.joiner.joined")
        .markup("headings.*.within.joiner.template")
        .markup("headings.*.number_format")
        .markup("headings.*.title_number_format")
}

/// The name of the element type of every heading, `flm.heading`. Each heading
/// kind is one element value of this type ([`HeadingElement`]).
pub const HEADING: &str = "flm.heading";

/// The name of the default lowering rule of the element [`HEADING`],
/// `flm.heading.lower.default` ([`HeadingRule`]). It is the heading's rule on every
/// backend except `latex-recomposed`, which has its own
/// ([`latex::LATEX_RULE`]).
pub const RULE: &str = "flm.heading.lower.default";

/// The feature's style identifier, `flm.headings.style`, which the heading
/// rule ([`HeadingRule`]) requires at every heading it stages. On the `html`
/// backend it names an empty style definition ([`stylesheet`]), so that the
/// page carries nothing for it; on the `latex-rendered` backend the feature's
/// definition piece ([`latex_rendered::piece`]) delivers it. The counter
/// styles of the kinds ([`STYLE_PREFIX`]) share its beginning and are other
/// definitions.
pub const STYLE: &str = "flm.headings.style";

/// The style definition [`STYLE`], holding no part: the headings have no
/// stylesheet of their own on the `html` backend, and the definition lets that
/// backend's resolver know the identifier the heading rule requires.
pub fn stylesheet() -> StyleDef {
    StyleDef {
        id: Name::from(STYLE),
        content: Vec::new(),
        docs: String::from("headings: no stylesheet of their own"),
    }
}

/// The reference kind of a heading's reference point, unless its heading kind
/// declares another ([`HeadingDecl::ref_kind`]): `section`. A reference kind says
/// what sort of thing a reference point is ([`RefPoint`]).
pub const KIND: &str = "section";

/// The label types a heading accepts, unless its heading kind declares others
/// ([`HeadingDecl::label_types`]): `sec` and `topic`.
///
/// A label is written `type:label`, and its type says what sort of thing it
/// marks. A heading accepts `\label{sec:intro}` and `\label{topic:intro}`; a
/// label of any other type, such as `\label{eq:intro}`, defines nothing, and the
/// label check of the `refs` feature
/// ([`CHECK_LABELS`](crate::stdcore::features::refs::CHECK_LABELS)) reports it as
/// `flm.refs.label-type-not-accepted`. `topic` is accepted beside `sec` because
/// a heading is also how a document introduces a subject that other documents
/// refer to.
pub const LABEL_TYPES: [&str; 2] = ["sec", "topic"];

/// The text that begins the anchor of a heading without a label, whose anchor
/// is derived from its title, unless its heading kind declares another
/// ([`HeadingDecl::anchor_prefix`]): `sec`, the first of the [`LABEL_TYPES`].
///
/// An unlabeled `\section{Examples}` therefore has the anchor `sec--Examples`,
/// and `\section{Examples}\label{sec:examples}` the anchor `sec-examples`: the
/// same prefix for every heading, labeled or not.
pub const ANCHOR_PREFIX: &str = LABEL_TYPES[0];

/// The text that begins the name of a heading kind's counter style, unless the
/// kind declares another counter style ([`HeadingDecl::style`]):
/// `flm.headings.style.`. The kind `section` is therefore shown in the counter
/// style `flm.headings.style.section`.
pub const STYLE_PREFIX: &str = "flm.headings.style.";

/// The text that begins the name of a heading kind's counter, unless the kind
/// declares another counter ([`HeadingDecl::counter`]): `flm.headings.`. The kind
/// `section` therefore counts in the counter `flm.headings.section`.
///
/// A counter's name is dotted and belongs to the feature that declares it, as
/// every name the library registers does; the kind's own name, `section`, is
/// what a document writes, and is distinct from it. A counter is always written
/// by its full name: in a [`Within`], in a document's [numbering
/// options](crate::stdcore::features::numbering::NumberingOptions) and in a
/// within template's `#{flm.headings.section}`, which `#{_parent}` replaces
/// when the counter meant is the parent that applies.
pub const COUNTER_PREFIX: &str = "flm.headings.";

/// The name of the star argument of every heading command, `star`.
pub const STAR_ARG: &str = "star";

/// The name of the optional key-value argument of every heading command,
/// `options`: the bracket of `\section[toc=false]{…}`.
pub const OPTIONS_ARG: &str = "options";

/// The name of the title argument of every heading command, `title`.
pub const TITLE_ARG: &str = "title";

/// The key saying whether a heading is numbered, `numbered`: `true` or
/// `false`. The star is this key set to `false`.
pub const NUMBERED_KEY: &str = "numbered";

/// The key saying whether a heading is listed in a table of contents, `toc`:
/// `true` or `false`.
pub const TOC_KEY: &str = "toc";

/// The key giving a heading an alternative title, `alt`: the content a table
/// of contents shows for the heading in place of its title
/// ([`Sectioning::toc_title`]), read with [`heading_alt`]. Its value is FLM
/// content (`alt={Short \emph{title}}`); written without a value it is
/// reported as [`MissingValue`].
pub const ALT_KEY: &str = "alt";

/// The keys a heading's options reserve and do not implement yet:
/// `pdfbookmark`, the title of a PDF bookmark. Each is accepted, ignored, and
/// reported as [`KeyNotImplemented`].
pub const RESERVED_KEYS: [&str; 1] = ["pdfbookmark"];

/// The name of the parent set the feature declares, `flm.headings.levels`: the
/// counters of the configuration's heading kinds that have one, the outermost
/// level first, and the kinds of one level in the order they are declared.
///
/// A **parent set** is a named list of counters, any one of which may be the
/// parent of another counter; the one advanced most recently is the parent that
/// applies ([`ParentSetDecl`](crate::stdcore::features::numbering::ParentSetDecl)).
/// A counter numbered within this set with [`Within::parent_set`] is therefore
/// numbered within whichever heading came last: with a heading kind `chapter`
/// declared above the sections, an equation after a chapter heading and before
/// any section is `(4.1)`, and after a subsection `(4.1.2.1)`. A kind that is not
/// numbered never advances its counter, so it is never the parent that applies.
pub const LEVELS_PARENT_SET: &str = "flm.headings.levels";

/// Which heading kinds are numbered: the numbering depth of a [`HeadingsConfig`].
///
/// A kind is numbered when it has a counter and the depth admits its level.
/// The depth is fixed when the registry is built; a heading of a numbered kind
/// may still opt out with a star or `numbered=false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HeadingNumbering {
    /// No kind is numbered. The default.
    #[default]
    NothingNumbered,
    /// Every kind that has a counter is numbered.
    AllNumbered,
    /// Every kind that has a counter and whose level is this number or less
    /// is numbered: `UpToDepth(2)` numbers the sections and the subsections of
    /// the standard kinds.
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

/// The declaration of one heading kind: what a heading is (a section, a
/// subsection, an appendix), declared by name with its level and everything a
/// heading of the kind needs.
///
/// [`HeadingDecl::new`] fills in every field except the name and the level with
/// the defaults of the standard kinds; one builder method per field changes it. The
/// kinds a registry has are [`HeadingsConfig::headings`], and the commands that
/// write a kind are [`HeadingsConfig::commands`]: a heading kind's declaration
/// lists no command, because several commands may write one kind.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::headings::HeadingDecl;
/// use flm_core::stdcore::features::numbering::{NumberTemplate, Within};
///
/// // An appendix at the sections' level, in capital letters.
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
/// // A kind never numbered and never listed in a table of contents.
/// let aside = HeadingDecl::new("aside", 3).no_counter().skip_toc(true);
/// assert!(aside.counter.is_none());
///
/// // A kind numbered within the sections, `2.1`: a `Within` refers to the
/// // section's counter, `flm.headings.section`.
/// let sub = HeadingDecl::new("subsection", 2).within(Within::parent("flm.headings.section"));
/// # let _ = sub;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(transform = heading_decl_schema)]
pub struct HeadingDecl {
    /// The kind's name: `section`. It identifies the kind in the element value,
    /// it is the value of the attribute `data-flm-heading` of the heading in the
    /// HTML output, and it is the key of the kind in the LaTeX mapping
    /// ([`HeadingsLatexOptions`](latex::HeadingsLatexOptions)).
    /// In data, it is written as the entry's key, not as a field.
    #[schemars(skip)]
    pub name: Name,
    /// The level, a number counted from zero, smaller for outer headings: the
    /// level the [`Sectioning`] aspect returns, which, unless a render option maps the
    /// kind to a tag, chooses the HTML tag, and which chooses how
    /// the plain-text backend prints the heading. Every value is a level; the
    /// standard kinds have levels 1 to 5, and level 0 is available for a kind
    /// declared above the sections, such as a chapter.
    pub level: u8,
    /// The counter the kind's headings count in, or `None` for a kind that is
    /// never numbered, which declares nothing to the `numbering` feature.
    /// `flm.headings.<name>` by default ([`COUNTER_PREFIX`]).
    pub counter: Option<Name>,
    /// The counter style the kind's numbers are shown in:
    /// `flm.headings.style.<name>` by default ([`STYLE_PREFIX`]). A kind with
    /// a counter needs one; [`HeadingDecl::no_counter`] clears both.
    pub style: Option<Name>,
    /// The counter the kind's counter is numbered within, and how that
    /// counter's number is written before the kind's own; `None`, the default,
    /// for a counter numbered within nothing. It refers to counters by their
    /// full names: `Within::parent("flm.headings.section")`.
    pub within: Option<Within>,
    /// How the kind's own number is written, in the number-template language
    /// of [`NumberTemplate`]: `#{arabic}` by default.
    pub number_format: NumberTemplate,
    /// The kind's heading title template: what is written before the title of
    /// a numbered heading of the kind. It is a text in the template language
    /// (see [the template language shared by
    /// features](crate::guide::concepts_overview#the-template-language-shared-by-features))
    /// in which `#{number}` is the heading's number; [`DEFAULT_TITLE_NUMBER_FORMAT`]
    /// (`#{number}. `) by default, so a numbered section reads `1. Introduction`.
    ///
    /// It states how the number joins the title (the text around it, which may
    /// hold a word: `Chapter~#{number}: `), and not how the number looks, which
    /// a program's lowering rule or a stylesheet decides through the role
    /// [`NUMBER_ROLE`](numbers::NUMBER_ROLE) the number carries.
    /// `#[bare]{number}` writes the kind's own number without the numbers of
    /// its parent counters, and `#[Roman]{number}` writes it in another number
    /// format; markup may enclose the placeholder. A template without any
    /// placeholder is allowed: the heading then shows nothing of its number and
    /// still counts. Nothing is written for a heading that is not numbered. A
    /// template that does not parse, that uses another placeholder, or that
    /// gives `number` a key it does not take, fails the registry build with an
    /// error that gives the kind. See [`title`] for the full description.
    pub title_number_format: String,
    /// Whether the kind is never listed in a table of contents, regardless of
    /// what a heading writes for `toc`. `false` by default.
    pub skip_toc: bool,
    /// Whether the kind is set as a run-in heading, at the start of the
    /// paragraph it introduces. `false` by default.
    pub run_in: bool,
    /// The reference kind of the kind's reference point: [`KIND`] by default.
    pub ref_kind: RefKind,
    /// The label types the kind's reference point accepts: [`LABEL_TYPES`] by
    /// default.
    pub label_types: Vec<String>,
    /// The text that begins the anchor of a heading without a label:
    /// [`ANCHOR_PREFIX`] by default.
    pub anchor_prefix: String,
    /// What the kind is, in one sentence, for the description of its element
    /// value in the registry. Empty by default; an empty text is replaced there
    /// by `A heading of level <level>.`, with the kind's level.
    pub docs: String,
}


/// The schema of a heading kind in data: only `level` is required, because the
/// entry is read over the declaration [`HeadingDecl::new`] makes from the name and
/// the `level` written.
fn heading_decl_schema(schema: &mut schemars::Schema) {
    crate::core::settings::named::require_only(schema, &["level"]);
}
impl HeadingDecl {
    /// Returns the kind `name` at `level`, with the defaults: counting in the
    /// counter `flm.headings.<name>` ([`COUNTER_PREFIX`]), shown in the counter
    /// style `flm.headings.style.<name>` ([`STYLE_PREFIX`]), numbered within
    /// nothing, its number written `#{arabic}` and joined to the title by
    /// [`DEFAULT_TITLE_NUMBER_FORMAT`], listed in a table of contents, set as a
    /// block, and with the reference point of the standard kinds ([`KIND`],
    /// [`LABEL_TYPES`], [`ANCHOR_PREFIX`]).
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

    /// Builder: the kind counts in the counter named `counter`, in full.
    pub fn counter(mut self, counter: impl Into<Name>) -> HeadingDecl {
        self.counter = Some(counter.into());
        self
    }

    /// Builder: the kind is never numbered. It has no counter and no counter
    /// style, and declares nothing to the `numbering` feature.
    pub fn no_counter(mut self) -> HeadingDecl {
        self.counter = None;
        self.style = None;
        self
    }

    /// Builder: the kind's numbers are shown in the counter style `style`.
    pub fn style(mut self, style: impl Into<Name>) -> HeadingDecl {
        self.style = Some(style.into());
        self
    }

    /// Builder: the kind's counter is numbered within another counter.
    pub fn within(mut self, within: Within) -> HeadingDecl {
        self.within = Some(within);
        self
    }

    /// Builder: how the kind's own number is written.
    pub fn number_format(mut self, number_format: NumberTemplate) -> HeadingDecl {
        self.number_format = number_format;
        self
    }

    /// Builder: the kind's heading title template, what is written before the
    /// title of a numbered heading of the kind, in which `#{number}` is the
    /// number ([`title_number_format`](HeadingDecl::title_number_format)).
    pub fn title_number_format(mut self, text: impl Into<String>) -> HeadingDecl {
        self.title_number_format = text.into();
        self
    }

    /// Builder: whether the kind is never listed in a table of contents.
    pub fn skip_toc(mut self, skip_toc: bool) -> HeadingDecl {
        self.skip_toc = skip_toc;
        self
    }

    /// Builder: whether the kind is set as a run-in heading.
    pub fn run_in(mut self, run_in: bool) -> HeadingDecl {
        self.run_in = run_in;
        self
    }

    /// Builder: the reference kind of the kind's reference point.
    pub fn ref_kind(mut self, ref_kind: impl Into<RefKind>) -> HeadingDecl {
        self.ref_kind = ref_kind.into();
        self
    }

    /// Builder: the label types the kind's reference point accepts.
    pub fn label_types(mut self, types: impl IntoIterator<Item = impl Into<String>>) -> HeadingDecl {
        self.label_types = types.into_iter().map(Into::into).collect();
        self
    }

    /// Builder: the text that begins the anchor of a heading without a label.
    pub fn anchor_prefix(mut self, prefix: impl Into<String>) -> HeadingDecl {
        self.anchor_prefix = prefix.into();
        self
    }

    /// Builder: what the kind is, in one sentence, for the description of its
    /// element value ([`docs`](HeadingDecl::docs)). Without the cargo feature
    /// `docs`, the text is dropped and the field stays empty.
    pub fn docs(mut self, docs: impl Into<String>) -> HeadingDecl {
        self.docs = crate::kept_docs(docs);
        self
    }

    /// The counter and the style a numbered occurrence of this kind states,
    /// or `None` for a kind with no counter.
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

/// The declaration of one heading command: a macro an author writes, such as
/// `\section`, the heading kind it writes, and the options a heading written
/// with it has before the heading's own star and bracket are read.
///
/// Several commands may write one kind. Every command takes the same
/// arguments ([`heading_args`]); two commands of one kind differ only by their
/// defaults, which are read before the star and the bracket and are replaced by
/// them (see the [module
/// documentation](self#what-an-occurrence-says-the-star-and-the-key-values)).
/// A default's key is [`NUMBERED_KEY`] or [`TOC_KEY`] and its value `true` or
/// `false`; any other default fails the registry build.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::headings::HeadingCommand;
///
/// // `\quietsection{…}` writes a section left out of the table of contents,
/// // unless an occurrence writes `toc=true`.
/// let quiet = HeadingCommand::new("quietsection", "section").default_value("toc", "false");
/// assert_eq!(quiet.defaults, [(String::from("toc"), String::from("false"))]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HeadingCommand {
    /// The command's name, without the backslash: `section`.
    /// In data, it is written as the entry's key, not as a field.
    #[schemars(skip)]
    pub name: String,
    /// The name of the heading kind it writes.
    pub heading: Name,
    /// The options a heading written with this command has before its star and
    /// its bracket are read, as key-value pairs in the order they are read.
    #[serde(default)]
    pub defaults: Vec<(String, String)>,
}

impl HeadingCommand {
    /// Returns the command `name` writing the kind `heading`, with no defaults.
    pub fn new(name: impl Into<String>, heading: impl Into<Name>) -> HeadingCommand {
        HeadingCommand { name: name.into(), heading: heading.into(), defaults: Vec::new() }
    }

    /// Builder: adds one default key-value, read after the ones already
    /// added.
    pub fn default_value(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> HeadingCommand {
        self.defaults.push((key.into(), value.into()));
        self
    }
}

/// The configuration of the [`headings`](self) feature: the heading kinds the
/// registry declares, the heading commands that write them, and which kinds are
/// numbered.
///
/// [`HeadingsConfig::new`] is the standard configuration: the five kinds
/// `section`, `subsection`, `subsubsection`, `paragraph` and `subparagraph`, at
/// levels 1 to 5, each counting in the counter `flm.headings.<kind>`, the first
/// three written in decimal digits and the last two in letters. Each kind except
/// `section` is numbered within the kind above it, except `paragraph`, which is
/// numbered within whichever of `section`, `subsection` and `subsubsection` came
/// last and is shown without their numbers. There is one command per kind, with
/// the kind's own name and no defaults, and no kind is numbered
/// ([`numbering`](HeadingsConfig::numbering)).
///
/// [`Headings::new`] takes this value, and a program passes it through
/// [`StdcoreConfig::headings`](crate::stdcore::StdcoreConfig::headings). A
/// feature's configuration is fixed when the registry is built (see [settings,
/// options and
/// configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
/// This configuration must be fixed then, because the commands the language accepts
/// and the block structure a run freezes depend on it. For the same reason the
/// feature adds [`digest_bytes`](HeadingsConfig::digest_bytes) to the registry digest
/// ([`crate::model::digest`]), the fingerprint that decides whether a cached result may be reused.
///
/// The feature checks the configuration when it registers
/// ([`HeadingsConfig::validate`]). Each of these fails the registry build with an
/// error that says what clashed: two kinds with one name, two commands with one
/// name, a command that writes a kind no declaration has, two kinds with one
/// counter or one counter style, a kind with a counter and no counter style, and
/// a command default whose key is not `numbered` or `toc`, or whose value is not
/// `true` or `false`. A kind that no command writes is accepted: its element
/// value is registered as synthesized, with no name, and a program may create
/// its headings itself, for instance in a stage.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::headings::{HeadingNumbering, HeadingsConfig};
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
    /// The heading kinds, in the order they were given.
    #[serde(deserialize_with = "crate::core::settings::named::deserialize")]
    #[schemars(
        transform = crate::core::settings::named::map_schema,
        with = "std::collections::BTreeMap<String, HeadingDecl>"
    )]
    pub headings: Vec<HeadingDecl>,
    /// The heading commands, in the order they were given.
    #[serde(deserialize_with = "crate::core::settings::named::deserialize")]
    #[schemars(
        transform = crate::core::settings::named::map_schema,
        with = "std::collections::BTreeMap<String, HeadingCommand>"
    )]
    pub commands: Vec<HeadingCommand>,
    /// Which kinds are numbered: [`HeadingNumbering::NothingNumbered`] by
    /// default.
    pub numbering: HeadingNumbering,
}

impl Default for HeadingsConfig {
    fn default() -> HeadingsConfig {
        HeadingsConfig::new()
    }
}

impl HeadingsConfig {
    /// Returns the standard configuration: the five kinds and one command each
    /// (see the type's documentation).
    // The formats, the hierarchy, the fourth level showing nothing of its
    // parents, and the texts of `numbers` are the Python product's shipped
    // defaults, read from its headings feature, except that `paragraph` is
    // numbered within whichever of the three kinds above it came last rather
    // than within `subsubsection` alone. See decision rationale at
    // [§flm-dd-dr:std-heading-counters].
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

    /// The configuration holding `headings` and one command per kind, of the
    /// kind's own name, with no defaults, and no kind numbered.
    fn default_commands_for(headings: Vec<HeadingDecl>) -> HeadingsConfig {
        let commands = headings
            .iter()
            .map(|kind| HeadingCommand::new(kind.name.as_str(), kind.name.clone()))
            .collect();
        HeadingsConfig { headings, commands, numbering: HeadingNumbering::default() }
    }

    /// Builder: replaces the kinds with `headings`, and the commands with one
    /// per kind, of the kind's own name, with no defaults. Add commands after
    /// it with [`command`](HeadingsConfig::command), or replace them with
    /// [`commands`](HeadingsConfig::commands). The depth is kept.
    pub fn headings(self, headings: impl IntoIterator<Item = HeadingDecl>) -> HeadingsConfig {
        HeadingsConfig {
            numbering: self.numbering,
            ..HeadingsConfig::default_commands_for(headings.into_iter().collect())
        }
    }

    /// Builder: sets which kinds are numbered (see [`HeadingNumbering`]).
    pub fn numbering(mut self, numbering: HeadingNumbering) -> HeadingsConfig {
        self.numbering = numbering;
        self
    }

    /// Returns whether `kind` is numbered under this configuration: it has a
    /// counter and the depth ([`numbering`](HeadingsConfig::numbering)) admits its
    /// level.
    pub fn numbers(&self, kind: &HeadingDecl) -> bool {
        kind.counter.is_some() && self.numbering.admits(kind.level)
    }

    /// Builder: replaces the commands with `commands`.
    pub fn commands(mut self, commands: impl IntoIterator<Item = HeadingCommand>) -> HeadingsConfig {
        self.commands = commands.into_iter().collect();
        self
    }

    /// Builder: adds one command after the ones already listed.
    pub fn command(mut self, command: HeadingCommand) -> HeadingsConfig {
        self.commands.push(command);
        self
    }

    /// Builder: sets [`run_in`](HeadingDecl::run_in) on every kind whose
    /// level is among `levels`, and clears it on every other kind. It acts on
    /// the kinds already listed, so it comes after
    /// [`headings`](HeadingsConfig::headings).
    pub fn run_in_levels(mut self, levels: impl IntoIterator<Item = u8>) -> HeadingsConfig {
        let levels: BTreeSet<u8> = levels.into_iter().collect();
        for kind in &mut self.headings {
            kind.run_in = levels.contains(&kind.level);
        }
        self
    }

    /// Returns the kind whose name is `name`, if one is declared.
    pub fn heading(&self, name: &str) -> Option<&HeadingDecl> {
        self.headings.iter().find(|kind| kind.name.as_str() == name)
    }

    /// Returns the commands that write the kind whose name is `name`, in the
    /// order they are listed.
    pub fn commands_of<'c>(&'c self, name: &'c str) -> impl Iterator<Item = &'c HeadingCommand> {
        self.commands.iter().filter(move |command| command.heading.as_str() == name)
    }

    /// Checks the configuration, as the feature does when it registers.
    ///
    /// # Errors
    ///
    /// A sentence that says what clashed, for the first of these problems found:
    /// two kinds with one name; two commands with one name; a command that
    /// writes a kind no declaration has; two kinds with one counter, or with
    /// one counter style; a kind with a counter and no counter style; a
    /// command default whose key is not [`NUMBERED_KEY`] or [`TOC_KEY`],
    /// or whose value is not `true` or `false`.
    pub fn validate(&self) -> Result<(), String> {
        let mut names = BTreeSet::new();
        let mut counters = BTreeSet::new();
        let mut styles = BTreeSet::new();
        for kind in &self.headings {
            if !names.insert(kind.name.as_str()) {
                return Err(format!(
                    "the heading kind `{}` may not be declared twice",
                    kind.name
                ));
            }
            if let Some(counter) = &kind.counter {
                if !counters.insert(counter.as_str()) {
                    return Err(format!(
                        "the heading kind `{}` may not count in the counter `{counter}`, which an \
                         earlier heading kind already counts in",
                        kind.name
                    ));
                }
                if kind.style.is_none() {
                    return Err(format!(
                        "the heading kind `{}` has the counter `{counter}`, so it must have a \
                         counter style",
                        kind.name
                    ));
                }
            }
            if let Some(style) = &kind.style {
                if !styles.insert(style.as_str()) {
                    return Err(format!(
                        "the heading kind `{}` may not use the counter style `{style}`, which an \
                         earlier heading kind already uses",
                        kind.name
                    ));
                }
            }
        }
        let mut commands = BTreeSet::new();
        for command in &self.commands {
            if !commands.insert(command.name.as_str()) {
                return Err(format!(
                    "the heading command `{}` may not be declared twice",
                    command.name
                ));
            }
            if !names.contains(command.heading.as_str()) {
                return Err(format!(
                    "the heading command `{}` must write a declared heading kind, and the heading \
                     kind `{}` is not declared",
                    command.name, command.heading
                ));
            }
            for (key, value) in &command.defaults {
                if key != NUMBERED_KEY && key != TOC_KEY {
                    return Err(format!(
                        "the heading command `{}` may give a default only for `{NUMBERED_KEY}` \
                         or `{TOC_KEY}`, not for `{key}`",
                        command.name
                    ));
                }
                if boolean(value).is_none() {
                    return Err(format!(
                        "the default `{key}={value}` of the heading command `{}` must be `true` \
                         or `false`",
                        command.name
                    ));
                }
            }
        }
        Ok(())
    }

    /// Returns the canonical byte form of the configuration: what the feature
    /// adds to the registry digest, and what `Hash` hashes.
    ///
    /// Every field of every kind and of every command is written out, in the
    /// order they are listed, each text preceded by its length and each optional
    /// field by whether it is present, except each kind's documentation text
    /// ([`HeadingDecl::docs`]): it only describes the kind's element value in the
    /// registry and changes nothing in a parse or a render, so it is left out.
    /// Two configurations therefore give the same bytes exactly when they are
    /// equal apart from those texts. Rewording a kind's description does not
    /// change the registry digest, and neither does building the crate without
    /// its cargo feature `docs`, which drops the texts. Equal configurations
    /// always give equal bytes, so `Hash` agrees with `Eq`.
    ///
    /// Two lists that behave alike but are written differently (the same kinds
    /// in another order) give different bytes: they then do not share a cached
    /// result, and two configurations that behave differently never do. The
    /// bytes are written field by field rather than taken from the type's
    /// `Debug` output, which would change with a field rename and so change
    /// every registry digest without notice.
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

/// Writes a count as eight little-endian bytes.
fn push_len(out: &mut Vec<u8>, len: usize) {
    out.extend_from_slice(&(len as u64).to_le_bytes());
}

/// Writes a text as its length and its bytes.
fn push_text(out: &mut Vec<u8>, text: &str) {
    push_len(out, text.len());
    out.extend_from_slice(text.as_bytes());
}

/// Writes an optional text as a presence byte and, when present, the text.
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

/// The data of one element value of `flm.heading`: the declaration of one
/// heading kind, the commands that write it, and whether it is numbered.
///
/// There is one element value per kind, shared by every command of that kind. A
/// lowering rule reads it from a node with
/// [`element_value`]. The type also
/// implements the element's [`Sectioning`] and [`Referenceable`] aspects, which
/// are computed from this value and from the node alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingElement {
    /// The kind's declaration.
    pub decl: HeadingDecl,
    /// The commands writing the kind, in the order the configuration lists
    /// them.
    pub commands: Vec<HeadingCommand>,
    /// Whether the kind is numbered, as the configuration's depth decided
    /// when the registry was built ([`HeadingsConfig::numbers`]).
    pub numbered: bool,
}

impl HeadingElement {
    /// Returns the command `node` was written with: the one among this kind's
    /// commands whose name is the node's own. `None` for a node that records no
    /// name, such as one a program built directly, which then gets no defaults.
    pub fn command_of(&self, node: &NodeRef<'_, Flm>) -> Option<&HeadingCommand> {
        let name = node.name()?;
        self.commands.iter().find(|command| command.name == name)
    }

    /// Returns what the heading `node` says about itself: [`heading_options`]
    /// read with the command it was written with.
    pub fn occurrence(&self, node: &NodeRef<'_, Flm>) -> HeadingOccurrence {
        heading_options(node, self.command_of(node))
    }
}

/// What one heading as written says about itself: whether it is numbered and
/// whether it is listed in a table of contents, as [`heading_options`] reads
/// them. See the [module
/// documentation](self#what-an-occurrence-says-the-star-and-the-key-values).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadingOccurrence {
    /// Whether the heading takes a number, where its kind is numbered at all.
    pub numbered: bool,
    /// Whether the heading is listed in a table of contents, unless its kind
    /// is never listed ([`HeadingDecl::skip_toc`]).
    pub toc: bool,
}

/// Returns whether the heading `node` was written with a star (`\section*{…}`).
///
/// `false` for a node that is not a heading, or whose star argument is absent.
/// The star means `numbered=false` and nothing else; [`heading_options`] reads
/// it together with everything else the heading writes.
pub fn starred(node: &NodeRef<'_, Flm>) -> bool {
    node.arguments()
        .and_then(|args| args.get_named(STAR_ARG))
        .is_some_and(|arg| arg.is_provided())
}

/// Returns what the heading `node`, written with `command`, says about itself:
/// the command's defaults, then the star, then the bracket's key-values, each
/// later entry replacing an earlier one (see the [module
/// documentation](self#what-an-occurrence-says-the-star-and-the-key-values)).
///
/// `command` is `None` for a node written with no known command (a node a
/// program built directly), which gets no defaults. The reader reports nothing:
/// a key other than `numbered` and `toc`, or a value it cannot read, leaves the
/// value read before it unchanged. The lowering rules report those problems.
pub fn heading_options(
    node: &NodeRef<'_, Flm>,
    command: Option<&HeadingCommand>,
) -> HeadingOccurrence {
    read_options(node, command).0
}

/// Returns the alternative title of the heading `node`: the value of the last
/// [`ALT_KEY`] (`alt=`) its bracket writes with a value, as pieces of the
/// document ([`WrittenPiece`]): a group or a command of the value as the node
/// of the document it was read from, and characters cut from a run that also
/// holds the key as text, whitespace at either end dropped
/// ([`key_value_pieces`]). A value written as one group is read as the group's
/// content, as the values of `numbered` and `toc` are (the usual convention of
/// LaTeX's key-value lists), whitespace around the group included:
/// `alt={Short \emph{title}}` and `alt= {Short \emph{title}} ` give the text
/// `Short ` and the `\emph` node, and `alt={}` an empty list.
///
/// `None` where the bracket writes no `alt=` with a value, or cannot be read as
/// key-value pairs. The reader reports nothing; the lowering rules report the
/// problems of the bracket. What a table of contents shows for a heading is
/// read through the aspect instead ([`Sectioning::toc_title`]), which copies
/// these pieces.
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

/// One thing [`read_options`] could not take from a heading's bracket.
#[derive(Debug, Clone, PartialEq, Eq)]
enum OptionProblem {
    /// A reserved key, accepted and not implemented.
    NotImplemented(String),
    /// A key whose value is content ([`ALT_KEY`]) written without a value.
    MissingValue(String),
    /// A key the heading does not declare.
    Unknown(String),
    /// A value of `numbered` or `toc` that is neither `true` nor `false`.
    BadValue(String, String),
    /// The bracket's keys are not plain text.
    Unreadable,
    /// A value of `numbered` or `toc` holds a placeholder of a user
    /// definition's template lowered without expansion, whose characters only a
    /// use site supplies ([`LowerError::PlaceholderCharacters`]).
    Placeholder,
}

/// `Some(true)` for `true`, `Some(false)` for `false`, `None` otherwise.
fn boolean(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Applies one key-value to `occurrence`: [`NUMBERED_KEY`] or [`TOC_KEY`] set
/// to `value`; any other key changes nothing.
fn apply(occurrence: &mut HeadingOccurrence, key: &str, value: bool) {
    if key == NUMBERED_KEY {
        occurrence.numbered = value;
    } else if key == TOC_KEY {
        occurrence.toc = value;
    }
}

/// The reader behind [`heading_options`], with what it could not take from
/// the bracket, in the order written.
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

/// Condition `flm.heading.key-not-implemented`, a Warning: a heading's options
/// write a key that is reserved and not implemented yet, one of
/// [`RESERVED_KEYS`] (`pdfbookmark`). Recovery: the key is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.heading.key-not-implemented",
    message = "the heading option `{key}` is not implemented yet"
)]
pub struct KeyNotImplemented {
    /// The key as written.
    pub key: String,
}

/// Condition `flm.heading.unknown-key`, an Error: a heading's options write a
/// key the heading does not declare, one that is not [`NUMBERED_KEY`],
/// [`TOC_KEY`], [`ALT_KEY`] or one of [`RESERVED_KEYS`]. Recovery: the key is
/// ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.heading.unknown-key",
    message = "`{key}` is not a heading option; the heading options are `numbered`, `toc` and `alt`"
)]
pub struct UnknownKey {
    /// The key as written.
    pub key: String,
}

/// Condition `flm.heading.bad-value`, an Error: a heading's options give
/// [`NUMBERED_KEY`] or [`TOC_KEY`] a value that is neither `true` nor `false`.
/// Recovery: the key is ignored, and the value read before it is kept.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.heading.bad-value",
    message = "the heading option `{key}` must be `true` or `false`, not `{value}`"
)]
pub struct BadValue {
    /// The key as written.
    pub key: String,
    /// The value as written, trimmed.
    pub value: String,
}

/// Condition `flm.heading.missing-value`, an Error: a heading's options write
/// a key whose value is content, [`ALT_KEY`], without a value (`[alt]`).
/// Recovery: the key is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.heading.missing-value",
    message = "the heading option `{key}` must be given a value"
)]
pub struct MissingValue {
    /// The key as written.
    pub key: String,
}

/// Condition `flm.heading.unreadable-options`, an Error: a heading's bracket
/// cannot be read as key-value pairs, because a key is not plain text (a command
/// is written where a key is expected). Recovery: the whole bracket is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.heading.unreadable-options",
    message = "the heading's options must be key-value pairs whose keys are plain text"
)]
pub struct UnreadableOptions;

/// Reports what the bracket of the heading `node` holds that [`heading_options`]
/// could not read: one diagnostic per problem, in the order written. Both
/// lowering rules call it, and a render runs one of them per heading, so each
/// problem is reported once per render.
///
/// A value holding a placeholder of a user definition's template lowered
/// without expansion is not reported: the heading cannot be written before a use
/// site supplies the value, so the function returns
/// [`LowerError::PlaceholderCharacters`], which makes the plan expand the
/// definition's use sites instead.
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

    /// The alternative title where the heading writes `alt=` ([`heading_alt`]),
    /// copied into a synthesized value of its own as content shown a second
    /// time ([`ValueContent::Shown`]: a footnote becomes its mark), the copy
    /// built on a worker thread; otherwise the title argument. `None` where the
    /// copy fails, which only content nested deeper than the [recursion
    /// guard](crate::lang::guard) allows can cause; nothing is reported.
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

/// Returns the block role of a heading of `kind`: a block of its own, or, for a
/// run-in kind, a decorator leading the paragraph it opens, which is how a
/// run-in heading is laid out.
fn block_role(kind: &HeadingDecl) -> BlockRole {
    if kind.run_in { BlockRole::BlockDecorator(Placement::Leads) } else { BlockRole::Block }
}

/// Returns what `kind` is, for the description of its element value: its
/// [`docs`](HeadingDecl::docs), or `A heading of level <level>.` when that is
/// empty.
fn kind_docs(kind: &HeadingDecl) -> String {
    if kind.docs.is_empty() { format!("A heading of level {}.", kind.level) } else { kind.docs.clone() }
}

/// Returns the element value of one heading kind: a `flm.heading` element
/// carrying the kind's declaration, its commands and whether it is numbered,
/// with the kind's block role and the [`Sectioning`] and [`Referenceable`]
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

/// Returns the arguments of a heading command, in order: the star, the optional
/// key-value argument, the title, and the trailing tack-on argument that holds
/// the `\label` fields written after the title (see
/// [fields](crate::guide::concepts_overview#fields-data-a-construct-reads)).
///
/// A feature that declares a heading-like command of its own can use this list,
/// so that its command takes the same arguments as the standard ones. A feature
/// that declares a labeled construct of another form declares the same trailing
/// argument with [`label_fields_arg`](refs::label_fields_arg).
pub fn heading_args() -> Vec<ArgDef> {
    vec![
        ArgDef::named(STAR_ARG, ArgStdKind::Star).docs("A star: not numbered (`numbered=false`)."),
        ArgDef::named(OPTIONS_ARG, ArgStdKind::KeyVals)
            .docs("Key-values: `numbered=true|false`, `toc=true|false`."),
        ArgDef::mandatory(TITLE_ARG).docs("The heading's title."),
        refs::label_fields_arg(),
    ]
}

/// The implementation of the lowering rule [`RULE`]: it stages (builds into the
/// lowered tree; see [`Lowerer`]) a
/// [`flm.pres.block-heading`](crate::stdcore::backends::pres::BLOCK_HEADING)
/// whose payload ([`HeadingPayload`]) carries the heading kind's name and level,
/// whether the heading is listed in a table of contents, its anchor and its
/// number as plain text, and whose content is the heading's number, where the
/// document numbered it, followed by the title lowered through the lowering
/// rules. See the [module documentation](self#the-lowering-rules) for a heading
/// without an anchor.
///
/// Where the heading writes an alternative title (`alt=`, [`heading_alt`]), the
/// rule lowers it through the lowering rules into a second content slot,
/// [`ALT_SLOT`](pres::ALT_SLOT), beside the title's
/// [`BODY_SLOT`](pres::BODY_SLOT); a backend that sets a running head reads
/// it, and the HTML and text backends read the title alone.
///
/// For a run-in kind it stages a
/// [`flm.pres.deco.runin`](crate::stdcore::backends::pres::DECO_RUNIN)
/// decoration instead. A decoration's payload carries nothing, so the heading's
/// facts are carried by a
/// [`flm.pres.runin-heading`](crate::stdcore::backends::pres::RUNIN_HEADING)
/// around the number and the title inside the decoration, with the same payload
/// as the block heading.
///
/// The rule also reports what the heading's bracket holds that it cannot read
/// (see the [module
/// documentation](self#what-an-occurrence-says-the-star-and-the-key-values)).
///
/// Only the feature constructs the rule, because the rule holds the kinds'
/// compiled heading title templates ([`HeadingDecl::title_number_format`]).
#[derive(Debug, Clone)]
pub struct HeadingRule {
    titles: Arc<Titles>,
}

impl HeadingRule {
    /// Stages what is written before the title of the heading of kind `kind` at
    /// `point`: the kind's [heading title
    /// template](HeadingDecl::title_number_format) with the number this document
    /// gave the heading in place of each `#{number}`, the number alone staged in
    /// a span of role [`NUMBER_ROLE`](numbers::NUMBER_ROLE) and the rest of the
    /// template as ordinary content. Nothing at all for a heading this document
    /// did not number, which includes every heading of a kind the configuration's
    /// depth ([`HeadingsConfig::numbering`]) leaves unnumbered.
    ///
    /// The number is written without the word `§`, which belongs to a reference
    /// to the heading and not to the heading itself, and without delimiters, of
    /// which a heading's counter style has none. It is written from the counter
    /// table each time it is shown, and is stored nowhere.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError`] when the table could not be read, when a node
    /// could not be built, when the kind's template was not compiled, or when the
    /// number could not be written from the counter record. The last case means
    /// that the record and the counter style it was made with do not match: a
    /// fault in the program, since the numbering stage refuses a record its
    /// counter style cannot write.
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
        // See decision rationale at [§flm-dd-dr:std-heading-number-place].
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
    /// Stages the heading `node` of the element value `value`, whose reference
    /// point is `point` and whose anchor is `anchor`, as [`HeadingRule`]
    /// documents.
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

/// The number this document gave the heading at `point`, as plain text: the
/// number as a reference to the heading writes it, without the word and the
/// delimiters (a title template may show it otherwise), each segment's content read
/// through [`plain_text`](pres::plain_text) and the segments joined. `None`
/// where the heading has no number: no counter table, or no record for the
/// point.
///
/// # Errors
///
/// Returns [`LowerError`] when the table could not be read, or when the number
/// could not be written from the counter record (a fault in the program, as
/// for the number staged before the title).
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

/// The `headings` feature. Constructed from a [`HeadingsConfig`], it registers one
/// element value of `flm.heading` per declared heading kind, written with the
/// names of that kind's commands, and the heading's lowering rules [`RULE`] and
/// [`latex::LATEX_RULE`]; the element [`title::NUMBER_ELEMENT`], which holds a
/// heading's number, with its lowering rules; the counters, counter styles and
/// parent set of the kinds ([`numbers`]); and the late-initialization build hook
/// ([`LateInit`](crate::core::registry::LateInit)) that compiles every kind's
/// heading title template. See the [module documentation](self).
#[derive(Debug, Default, Clone)]
pub struct Headings {
    config: HeadingsConfig,
}

impl Headings {
    /// Returns the feature constructed from `config`, which stays fixed for the
    /// life of the registry the feature is built into.
    pub fn new(config: HeadingsConfig) -> Headings {
        Headings { config }
    }

    /// Returns the configuration this feature was constructed from.
    pub fn config(&self) -> &HeadingsConfig {
        &self.config
    }
}

impl FlmFeature for Headings {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Headings").docs(
            "Headings: one value of the element `flm.heading` per heading kind, written with \
             the kind's heading commands (`\\section` to `\\subparagraph` by default); each \
             heading carries a level, a reference point and its `\\label` fields.",
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
            let written: Vec<String> =
                names.iter().map(|name| format!("`\\{name}*[…]{{title}}\\label{{…}}`")).collect();
            let (syntax, docs) = if names.is_empty() {
                let docs =
                    format!("The heading kind `{}`, which no command writes: {}", kind.name, kind_docs(kind));
                (SyntaxDef::Synthesized, docs)
            } else {
                (SyntaxDef::Macro { args: heading_args() }, format!("{}: {}", written.join(", "), kind_docs(kind)))
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
            "the references feature's anchors table, which the heading rule reads for the \
             anchor of the heading's own reference point",
        )?;
        // The counter table belongs to the numbering feature, which may not be
        // there either; the rule then shows no number. The heading counters
        // and their styles are declared into that feature's slots the same
        // way, as peer contributions.
        b.peer_table::<Counters>(
            COUNTERS.name(),
            "the numbering feature's counter table, which the heading rule reads for the \
             number this document gave the heading",
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
                "A heading's number, where the kind's heading title template writes \
                 `#{number}`. No document can write it: the heading's lowering rule inserts it.",
            ),
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
                "a heading's number lowers to `flm.pres.span` of role `flm.heading.number` \
                 around the number",
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
                "a heading lowers to `flm.pres.block-heading` carrying its kind's name and \
                 level, whether it is listed in a table of contents, and its anchor, or, for a \
                 run-in kind, to a `flm.pres.deco.runin` decoration holding a \
                 `flm.pres.runin-heading` with the same payload; a numbered heading's content \
                 begins with its number",
            ),
        )?;
        b.style_def(stylesheet())?;
        b.digest_input(&self.config.digest_bytes())?;
        latex::register(b)?;
        latex_rendered::register(b)?;
        Ok(())
    }
}

/// Returns the [`Headings`] feature constructed from the default configuration
/// (the five standard kinds, none numbered and none set as a run-in heading),
/// ready to be listed in a package entry.
///
/// Use `Arc::new(Headings::new(config))` to list the feature constructed from a
/// [`HeadingsConfig`] of your own.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Headings::default())
}

/// A heading kind is written in data under its name, in the map of
/// [`HeadingsConfig::headings`], and read over the declaration
/// [`HeadingDecl::new`] makes from that name and the level written; the level
/// must be written.
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

/// A heading command is written in data under the command's name, in the map
/// of [`HeadingsConfig::commands`].
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
    fn a_configuration_that_clashes_is_refused_naming_the_clash() {
        let refused = |config: HeadingsConfig| config.validate().expect_err("refused");
        let undeclared = refused(HeadingsConfig::new().command(HeadingCommand::new("x", "nokind")));
        assert!(undeclared.contains("`nokind`"), "{undeclared}");
        let mut kinds = HeadingsConfig::new().headings;
        kinds.push(HeadingDecl::new("section", 1));
        assert!(refused(HeadingsConfig::new().headings(kinds)).contains("`section`"));
        let twice = HeadingsConfig::new().command(HeadingCommand::new("section", "section"));
        assert!(refused(twice).contains("the heading command `section` may not be declared twice"));
        let mut kinds = HeadingsConfig::new().headings;
        kinds.push(HeadingDecl::new("appendix", 1).counter("flm.headings.section"));
        assert!(
            refused(HeadingsConfig::new().headings(kinds)).contains("counter `flm.headings.section`")
        );
        let mut kinds = HeadingsConfig::new().headings;
        kinds.push(HeadingDecl::new("appendix", 1).style("flm.headings.style.section"));
        assert!(refused(HeadingsConfig::new().headings(kinds)).contains("style"));
        let bad_default = HeadingsConfig::new()
            .command(HeadingCommand::new("q", "section").default_value("alt", "x"));
        assert!(refused(bad_default).contains("`alt`"));
        let bad_value = HeadingsConfig::new()
            .command(HeadingCommand::new("q", "section").default_value("toc", "maybe"));
        assert!(refused(bad_value).contains("toc=maybe"));
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
