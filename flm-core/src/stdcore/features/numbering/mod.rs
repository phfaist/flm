//! The `numbering` feature: counters, counter styles, and the number that
//! every numbered reference point of a document carries; and the functions
//! that write a counter value as content, which a caller may use without the
//! feature.
//!
//! A **counter** ([`CounterDecl`]) is a dotted name under which things are
//! counted in document order, declared by a feature together with the counter
//! it is numbered within; its value at a reference point is a number with
//! optional sub-numbers ([`CounterValue`]). A **counter style**
//! ([`CounterStyleDecl`]) says how a counter value is shown: `(3)`, `Eq.~(3)`,
//! `2.13`, `Eqs.~(1–3 and 7)`. A **reference point**
//! ([`RefPoint`](crate::model::aspects::RefPoint)) is one place in a document
//! that can be referred to, numbered, or given an anchor; a point that is to
//! carry a number states its **number source**
//! ([`NumberSource`](crate::model::aspects::NumberSource)): the counter it
//! counts in and the counter style its number is shown in. The concepts
//! overview presents these terms together ([counters and counter
//! styles](crate::guide::concepts_overview#counters-and-counter-styles);
//! [reference points, labels and
//! anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)),
//! and the [numbering guide](crate::guide::numbering) describes the mechanism
//! for the author of a document and of an extension.
//!
//! # What the feature provides to an author of documents
//!
//! The numbering feature declares no command and no environment. It numbers
//! the constructs that other features declare as numbered — headings, the
//! lines of display mathematics, theorems, list items, floats, and the
//! footnotes and other notes of the [`xnotes`](crate::stdcore::features::xnotes)
//! feature — and the number such a construct shows, beside itself
//! and in every reference to it, is written from the counter value this
//! feature records.
//!
//! A document changes the numbering through its numbering options
//! ([`NumberingOptions`]). For each counter, the options say which counter it
//! is numbered within (the equations within the sections, so that an equation
//! reads `(2.13)`), or that it is numbered within nothing, and the value the
//! counter has before it is first advanced (the value `0` for the sections
//! makes a subsection written before the first section read `0.1`). Whether a
//! construct is numbered at all is decided by the feature that owns the
//! construct (the headings feature's numbering depth, the mathematics
//! feature's switch for numbered equations), never by this feature.
//!
//! # What the feature provides to the pipeline
//!
//! The `numbering` feature ([`Numbering`], an
//! [`FlmFeature`](crate::core::registry::FlmFeature)) registers the following
//! into the registry:
//!
//! - **Five service slots**, through which other features declare what is
//!   numbered and how. A service slot is the declaration, by the feature that
//!   owns an interaction, of a dotted name, the type of the values the slot
//!   takes and its arity ([`services`](crate::core::services)); each of these
//!   five takes any number of values, in contribution order:
//!   - the service slot `flm.numbering.counters` ([`COUNTERS_SLOT`]) takes the
//!     counters ([`CounterDecl`]);
//!   - `flm.numbering.styles` ([`STYLES_SLOT`]) takes the counter styles
//!     ([`CounterStyleDecl`]);
//!   - `flm.numbering.within-joiners` ([`WITHIN_JOINERS_SLOT`]) takes the
//!     within-joiners registered by name ([`WithinJoinerDecl`]);
//!   - `flm.numbering.number-writers` ([`NUMBER_WRITERS_SLOT`]) takes the
//!     number writers ([`NumberWriterDecl`]);
//!   - `flm.numbering.parent-sets` ([`PARENT_SETS_SLOT`]) takes the parent
//!     sets ([`ParentSetDecl`]).
//!
//!   A feature contributes to these service slots with
//!   [`FeatureBuilder::peer_slot`](crate::core::registry::FeatureBuilder::peer_slot),
//!   a peer declaration ([`PeerName`](crate::core::registry::PeerName)) of
//!   the service slot: in a registry without the numbering feature, the
//!   value is dropped with one Note and the contributing feature still
//!   registers ([services and service
//!   slots](crate::guide::concepts_overview#services-and-service-slots)).
//! - **The stage [`NUMBER`]** (`flm.numbering.number`), the only stage of the
//!   library that advances a counter. It runs once per document: it reads
//!   every reference point of the document in document order from the table
//!   `flm.refpoints`
//!   ([`REFPOINTS`](crate::stdcore::features::refpoints::REFPOINTS)), reads the
//!   numbering options, and writes the counter table. Its behavior is
//!   described below, under "What the stage does with a document".
//! - **The counter table [`COUNTERS`]** (`flm.counters`, a [`Counters`]). For
//!   every reference point the stage read, the table holds a
//!   [`CounterRecord`] (the counter, its value, the counters it is numbered
//!   within with their values, the counter style) or nothing for a point that
//!   is not numbered. A record holds a value and never text: a lowering rule
//!   writes the number where it shows it, with [`Counters::format`],
//!   [`Counters::format_in_style`] or [`Counters::format_many`], and splices
//!   the resulting [`NumberSegment`]s with
//!   [`Lowerer::splice`](crate::core::lowering::Lowerer::splice). The table
//!   also carries every declared counter style, compiled ([`Counters::style`]).
//! - **The option type [`NumberingOptions`]**, bound on the document; a render
//!   request may set it again, and its value then replaces the document's for
//!   that render ([`Bind::Either`](crate::model::options::Bind::Either)).
//! - **The configuration [`NumberingConfig`]**, the counter styles a host
//!   replaces by name. It is the feature's configuration, fixed when the
//!   registry is built ([settings, options and
//!   configuration](crate::guide::concepts_overview#settings-options-and-configuration)),
//!   and it is how a host changes what a number looks like without writing a
//!   feature. The settings keys are the fields of [`NumberingConfig`]
//!   (`styles`) and of [`NumberingOptions`] (`counters`)
//!   ([`settings`](crate::core::settings)).
//!
//! The feature registers no element and no lowering rule, so it has no rule
//! on any backend. The number of a construct is shown by the lowering rules of
//! the feature that owns the construct, on every backend, through the counter
//! table.
//!
//! ## Checks made when the registry is built
//!
//! The feature reads its five service slots once every feature has
//! registered, and each of the following mistakes fails the registry build,
//! with a message that gives the name concerned:
//!
//! - two declarations of one counter, counter style, parent set, within-joiner
//!   or number writer;
//! - a counter or a parent set whose name begins with `_`
//!   ([`RESERVED_PREFIX`]);
//! - a counter whose default counter style no feature declares;
//! - a counter numbered within a counter no feature declares, within a parent
//!   set no feature registers, or within an empty list of counters;
//! - a parent set that holds a counter no feature declares;
//! - counters numbered within each other, through any of a counter's parents;
//! - a counter numbered within another through a within-joiner that no
//!   feature registers, a nesting joiner that no feature registers, and a
//!   counter style written by a number writer that no feature registers;
//! - a text of a counter style, of a within-joiner or of a nesting joiner that
//!   is not valid FLM or does not compile;
//! - a within-joiner or a nesting joiner that writes a counter which, under
//!   one of the chains of counters the counter may be numbered within, is not
//!   in that chain;
//! - a replacement in [`NumberingConfig`] for a counter style that no feature
//!   declares.
//!
//! The registry build compiles these texts once, through the feature's build
//! hook ([`LateInit`](crate::core::registry::LateInit)), which runs once the
//! language that documents are parsed with is built; nothing is parsed again
//! while a number is written.
//!
//! ## What the stage does with a document
//!
//! - **The options are applied first.** Before it reads any reference point,
//!   the stage applies the document's numbering options to the declared
//!   hierarchy, and reports every option it cannot apply: a counter no
//!   feature declares ([`UnknownCounter`], a Warning), a parent set no feature
//!   registers ([`UnknownParentSet`]), a within-joiner that does not compile
//!   ([`BadWithinJoiner`]) or that no feature registers
//!   ([`UnknownWithinJoiner`]), counters numbered within each other
//!   ([`HierarchyCycle`]), and a within-joiner that writes a counter which,
//!   under the options, is not always one of the counters the counter is
//!   numbered within ([`NotAnAncestor`]).
//! - **Every reference point that states a number source is numbered.** The
//!   stage reads the points in document order and advances the levels of the
//!   counter that each point's number source lists. A counter has no value
//!   until it is first advanced, or until the options give it an
//!   [`initial`](CounterOptions::initial) value; a point whose number needs an
//!   ancestor with no value is not numbered and is reported
//!   ([`ParentUninitialized`]), once per counter and run. Where a counter has
//!   several possible parents ([`Parents`]), the parent that applies at a
//!   point is the one advanced most recently in document order.
//! - **A point that counts in a counter no feature declares is an error**
//!   ([`UndeclaredCounter`]), reported once per run and per counter name. A
//!   point that gives a counter asks to be numbered, so a registry that cannot
//!   number it is in error; a feature whose points may go unnumbered gives its
//!   configuration a switch under which those points give no counter. A point
//!   whose counter style no feature declares is an error at that point
//!   ([`UndeclaredStyle`]), because the counter is declared and the counter
//!   style asked for is a mistake. A point that would carry a sub-number its
//!   counter style has no format for is an error too ([`UndeclaredSubLevel`]).
//! - **A point may count in a scoped instance of its counter.** A scoped
//!   instance is an independent sequence of the counter's values attached to
//!   an enclosing node, such as a list environment
//!   ([`NumberSource::instances`](crate::model::aspects::NumberSource::instances)).
//!   The stage keeps one count per instance; an instance nested in another is
//!   numbered within the enclosing instance's current value and written with
//!   the counter's nesting joiner ([`CounterDecl::nesting`]). Instances that
//!   do not enclose the point one within the next are an error
//!   ([`InstanceNotEnclosing`]).
//! - **A point may carry a number template of its own**
//!   ([`NumberSource::number_format`](crate::model::aspects::NumberSource::number_format)).
//!   The stage compiles each distinct template text once per run and counter
//!   style, and records the point with a counter style derived from the
//!   declared one ([`CounterStyle::with_number_format`]). A template that does
//!   not compile is an error ([`BadNumberFormat`]), and the point is numbered
//!   in the declared counter style.
//!
//! Every diagnostic of the stage is an Error unless stated otherwise above.
//!
//! # Interactions with other features
//!
//! - The stage requires the table `flm.refpoints` of the
//!   [`refpoints`](crate::stdcore::features::refpoints) feature: a plan that
//!   schedules the stage in a registry without that feature fails. The stage
//!   reads nothing of the [`refs`](crate::stdcore::features::refs) feature, so
//!   a registry without references still numbers.
//! - The [`headings`](crate::stdcore::features::headings),
//!   [`math`](crate::stdcore::features::math),
//!   [`lists`](crate::stdcore::features::lists),
//!   [`theorems`](crate::stdcore::features::theorems),
//!   [`floats`](crate::stdcore::features::floats) and
//!   [`xnotes`](crate::stdcore::features::xnotes) features declare their
//!   counters and counter styles into the service slots above, and their
//!   lowering rules read the counter table to show their numbers. The headings
//!   feature also contributes the parent set `flm.headings.levels`.
//! - The [`refs`](crate::stdcore::features::refs) feature reads the counter
//!   table to write the number of a reference's target, and builds the anchor
//!   of a point whose anchor is derived from its number with
//!   [`identifier_text`].
//! - Every text a counter style holds is FLM; number templates in the
//!   `Formats` form ([`NumberTemplate::Formats`]) and within templates are
//!   written in the template language that user definitions use ([the
//!   template language shared by
//!   features](crate::guide::concepts_overview#the-template-language-shared-by-features)).
//!
//! # Writing a counter value without the feature
//!
//! The functions that write a counter value hold no counter and count
//! nothing, so a caller may use them on values of its own. A counter style
//! declaration is compiled against the built language
//! ([`CounterStyleDecl::compile`]) into a [`CounterStyle`]; [`format()`]
//! writes one value with it and [`format_many`] writes several. Both return a
//! list of [`Segment`]s, each a small tree of content with the position of the
//! value it links to among the values the caller passed in, or none.
//!
//! ## What a counter style is made of
//!
//! - The **number template** ([`NumberTemplate`], the field `number_format`)
//!   says how the number itself is written: a text with a place for the
//!   number, which writes it in one of the eight number formats
//!   ([`NumberFormat`]). `#{arabic}.` writes `3.` for the number three, and
//!   the single-letter template `(a)` ([`NumberTemplate::Initial`]) writes
//!   `(c)`.
//! - The **sub-levels** ([`SubLevelDecl`]) say how the sub-numbers of a value
//!   are written and what separates each from what comes before it. The value
//!   `(3, 2)` is written `3b` in a counter style whose first sub-level is
//!   written in letters with no separator.
//! - The **prefix words** ([`PrefixWords`]) are the word before the number, in
//!   one form for a single number and one for several (`Eq.~`, `Eqs.~`), and
//!   optional named variants (`capital`: `Equation~`, `Equations~`).
//! - The **delimiters** ([`Delimiters`]) are the texts around the number, such
//!   as `(` and `)`.
//! - The **join texts** ([`JoinDecl`]) say how several numbers are written as
//!   one text, down to the word between the last two.
//!
//! Every one of these texts is FLM, and every one is parsed once, when the
//! counter style is compiled.
//!
//! ## Numbering within another counter
//!
//! A counter may be numbered within another: an equation numbered within the
//! sections is written `(2.13)`. The counter's [`Within`] declares both its
//! parent (the counter whose advance restarts it) and its **within-joiner**,
//! what is written of its ancestors before its own number, in one of four
//! forms:
//!
//! - a **joining text**: `Within::parent("flm.headings.section")` writes the
//!   section's number and a full stop, and [`joined_by`](Within::joined_by)
//!   puts another text in place of the full stop;
//! - a **within template** ([`template`](Within::template)): FLM text in
//!   which `#{flm.headings.section}` is the section's number in the section's
//!   own counter style, without word or delimiters. Markup may surround a
//!   substitution (`\textbf{#{flm.headings.section}}.`), and the keys in
//!   brackets of `#[Roman]{…}` and `#[bare]{…}` write the counter in a named
//!   [`NumberFormat`] and without its own ancestors;
//! - **nothing** ([`displayed_bare`](Within::displayed_bare)): the counter
//!   still restarts under its parent, and its number is shown without its
//!   ancestors' numbers;
//! - a **registered joiner** ([`joiner_named`](Within::joiner_named)): a
//!   [`WithinJoiner`] that a feature contributed to [`WITHIN_JOINERS_SLOT`],
//!   which writes what its code writes.
//!
//! The parent is one counter ([`Within::parent`]), any of several
//! ([`Within::parents`]) or any counter of a named [`ParentSetDecl`]
//! ([`Within::parent_set`]). Among several, the one advanced most recently in
//! document order applies at a point, and a within template writes it as
//! `#{_parent}`. [`no_restart`](Within::no_restart) writes the parent's number
//! without letting the parent restart the count.
//!
//! An ancestor is written together with its own ancestors, so a within-joiner
//! refers only to the counter immediately above: a theorem numbered within the
//! subsections, which are numbered within the sections, is joined to
//! `flm.headings.subsection` alone and is written `2.3.4`.
//!
//! A compiled within-joiner returns a list of [`Part`]s, written in order
//! before the point's own number, which is never inside a part. Each part
//! lists the ancestors it shows, so a caller writing several numbers at once
//! can find which parts two numbers share. A joining text gives one part per
//! ancestor; a within template gives one part per substitution, with the text
//! after it; a registered joiner gives what its code gives.
//!
//! A caller passes the within-joiner and the ancestors together as a
//! [`Parent`]; each ancestor is an [`Ancestor`] carrying its own value,
//! counter style and within-joiner. The ancestors are the whole chain,
//! outermost first, and the order tells which counters each ancestor may
//! itself be written within. [`parts`] returns the parts alone, without a
//! number around them.
//!
//! ## The number writer
//!
//! A counter style says what a number is made of; the **number writer** puts
//! those texts and numbers together into the content a reader sees. The
//! **default writer** writes every counter style that asks for no other
//! writer:
//!
//! - for one value: the word, the opening delimiter, the parts, the number
//!   with its sub-numbers, and the closing delimiter;
//! - for several values of one counter style: the values grouped by the
//!   counters they are numbered within, each group sorted, its consecutive
//!   values collapsed into ranges, and the runs joined by the counter style's
//!   join texts. The end of a range is written without the parts and without
//!   the leading components it shares with the start, unless the range holds
//!   exactly two consecutive values or the counter style repeats them, and a
//!   range never spans two groups.
//!
//! A feature may register another writer, as it registers a within-joiner:
//! [`NumberWriter`] is the declaration and [`CompiledNumberWriter`] what it
//! compiles to, with one entry point for one value and one for several values
//! of one counter style. A [`NumberWriterDecl`] goes into
//! [`NUMBER_WRITERS_SLOT`], and a counter style asks for the writer by name
//! with [`with_number_writer`](CounterStyleDecl::with_number_writer). A writer
//! receives each value as a [`NumberedValue`], with the caller's
//! [`FormatOptions`], a [`WriteCx`] (the counter style's data, the library's
//! operations, and [`parts`](WriteCx::parts), which runs a value's
//! within-joiner when asked and keeps the result for the rest of the call), and
//! a [`NumberOut`] to which it appends content or characters, each marked with
//! the value it links to or with none. [`WriteCx::default_one`] and
//! [`WriteCx::default_many`] write what the default writer would, so a writer
//! can add to that text instead of repeating it.
//!
//! # Map of the items
//!
//! - The feature: [`Numbering`], [`feature()`], [`FEATURE`],
//!   [`NumberingConfig`]; the options [`NumberingOptions`], [`CounterOptions`],
//!   [`WithinOption`]; the stage name [`NUMBER`].
//! - Declarations: [`CounterDecl`], [`Within`], [`Parents`],
//!   [`WithinJoinerSpec`], [`ParentSetDecl`]; the service slot names
//!   [`COUNTERS_SLOT`], [`STYLES_SLOT`], [`WITHIN_JOINERS_SLOT`],
//!   [`NUMBER_WRITERS_SLOT`], [`PARENT_SETS_SLOT`]; [`RESERVED_PREFIX`],
//!   [`PARENT_PLACEHOLDER`].
//! - Counter styles: [`CounterStyleDecl`] with [`NumberTemplate`],
//!   [`NumberFormat`], [`SubLevelDecl`], [`PrefixWords`], [`PluralWords`],
//!   [`Delimiters`], [`JoinDecl`]; the compiled [`CounterStyle`];
//!   [`StyleError`].
//! - Writing values: [`CounterValue`], [`format()`], [`format_many`],
//!   [`parts`], [`FormatOptions`], [`Segment`], [`Ancestor`], [`Parent`],
//!   [`FormatError`], [`identifier_text`].
//! - Within-joiners: [`WithinJoiner`], [`WithinJoinerDecl`],
//!   [`CompiledWithinJoiner`], [`Part`], [`WithinCx`], [`JoinCx`].
//! - Number writers: [`NumberWriter`], [`NumberWriterDecl`],
//!   [`CompiledNumberWriter`], [`NumberedValue`], [`NumberOut`], [`WriteCx`],
//!   [`WriterCx`], [`JoinText`].
//! - The counter table: [`COUNTERS`], [`Counters`], [`CounterRecord`],
//!   [`NumberSegment`], [`ManyNumbers`].
//! - The stage's diagnostic conditions: [`UnknownCounter`],
//!   [`UndeclaredCounter`], [`UndeclaredStyle`], [`UndeclaredSubLevel`],
//!   [`ParentUninitialized`], [`UnknownParentSet`], [`NotAnAncestor`],
//!   [`HierarchyCycle`], [`BadWithinJoiner`], [`UnknownWithinJoiner`],
//!   [`InstanceNotEnclosing`], [`BadNumberFormat`], [`StageFailed`].
//!
//! # Examples
//!
//! One equation number, with the word and the delimiters the counter style
//! declares. The text a segment holds is read back here with the source
//! recomposer ([`recompose_source`](crate::stdcore::backends::pres::read::recompose_source)),
//! which writes content back as the FLM it was parsed from.
//!
//! ```
//! use flm_core::lang::{build_language, LangInputs};
//! use flm_core::stdcore::backends::pres::read::recompose_source;
//! use flm_core::stdcore::features::numbering::{
//!     format, CounterStyleDecl, CounterValue, Delimiters, FormatOptions, NumberTemplate,
//!     PrefixWords, Segment,
//! };
//!
//! /// The FLM source a list of segments writes, segment by segment.
//! fn written(segments: &[Segment]) -> String {
//!     segments
//!         .iter()
//!         .map(|segment| {
//!             let nodes: Vec<_> = segment.content().root().children().iter().collect();
//!             recompose_source(&nodes)
//!         })
//!         .collect()
//! }
//!
//! let lang = build_language(LangInputs::new());
//!
//! let mut decl = CounterStyleDecl::new("example.style.equation");
//! decl.number_format = NumberTemplate::formats("#{arabic}");
//! decl.delimiters = Delimiters::new("(", ")");
//! decl.prefix_words = PrefixWords::new("Eq.~", "Eqs.~");
//! let style = decl.compile(&lang).expect("the style compiles");
//!
//! let segments = format(&CounterValue::new(13), &style, None, &FormatOptions::new())?;
//! assert_eq!(written(&segments), "Eq.~(13)");
//!
//! // Without the word, and without the delimiters.
//! let bare = FormatOptions::new().without_word().without_delimiters();
//! let segments = format(&CounterValue::new(13), &style, None, &bare)?;
//! assert_eq!(written(&segments), "13");
//! # Ok::<(), flm_core::stdcore::features::numbering::FormatError>(())
//! ```
//!
//! Several numbers at once: they are sorted, consecutive ones are collapsed
//! into a range, and the word takes its plural form.
//!
//! ```
//! # use flm_core::lang::{build_language, LangInputs};
//! # use flm_core::stdcore::backends::pres::read::recompose_source;
//! # use flm_core::stdcore::features::numbering::{
//! #     format_many, CounterStyleDecl, CounterValue, Delimiters, FormatOptions,
//! #     JoinDecl, NumberedValue, PrefixWords, Segment,
//! # };
//! # fn written(segments: &[Segment]) -> String {
//! #     segments.iter().map(|segment| {
//! #         let nodes: Vec<_> = segment.content().root().children().iter().collect();
//! #         recompose_source(&nodes)
//! #     }).collect()
//! # }
//! # let lang = build_language(LangInputs::new());
//! let mut decl = CounterStyleDecl::new("example.style.equation");
//! decl.delimiters = Delimiters::new("(", ")");
//! decl.prefix_words = PrefixWords::new("Eq.~", "Eqs.~");
//! decl.join = JoinDecl::compact();
//! let style = decl.compile(&lang).expect("the style compiles");
//!
//! let values = [
//!     CounterValue::new(2),
//!     CounterValue::new(9),
//!     CounterValue::new(1),
//!     CounterValue::new(3),
//! ];
//! let written_values: Vec<NumberedValue<'_>> = values.iter().map(NumberedValue::new).collect();
//! let segments = format_many(&style, &written_values, &FormatOptions::new())?;
//! assert_eq!(written(&segments), "Eqs.~(1–3,9)");
//! # Ok::<(), flm_core::stdcore::features::numbering::FormatError>(())
//! ```

mod decl;
mod feature;
mod formats;
mod joiner;
mod many;
mod options;
mod stage;
mod style;
mod table;
mod template;
mod value;
mod writer;

pub use decl::{
    CounterDecl, ParentSetDecl, Parents, Within, WithinJoinerSpec, COUNTERS_SLOT,
    NUMBER_WRITERS_SLOT, PARENT_PLACEHOLDER, PARENT_SETS_SLOT, RESERVED_PREFIX, STYLES_SLOT,
    WITHIN_JOINERS_SLOT,
};
pub use joiner::{
    CompiledWithinJoiner, JoinCx, Part, WithinCx, WithinJoiner, WithinJoinerDecl,
};
pub use writer::{
    CompiledNumberWriter, JoinText, NumberOut, NumberWriter, NumberWriterDecl, NumberedValue,
    WriteCx, WriterCx,
};
pub use feature::{feature, Numbering, NumberingConfig, FEATURE};
pub(crate) use feature::digest_style;
pub use formats::{NumberFormat, NumberTemplate};
pub use many::format_many;
pub use options::{CounterOptions, NumberingOptions, WithinOption};
pub use style::{
    format, parts, Ancestor, CounterStyle, CounterStyleDecl, Delimiters, FormatError, FormatOptions,
    JoinDecl, Parent, PluralWords, PrefixWords, Segment, StyleError, SubLevelDecl,
};
pub use stage::{
    BadNumberFormat, BadWithinJoiner, HierarchyCycle, InstanceNotEnclosing, NotAnAncestor, ParentUninitialized,
    StageFailed, UndeclaredCounter, UndeclaredStyle, UndeclaredSubLevel, UnknownCounter,
    UnknownParentSet, UnknownWithinJoiner, NUMBER,
};
pub use table::{CounterRecord, Counters, ManyNumbers, NumberSegment, COUNTERS};
pub(crate) use style::{list_shape, CompiledJoin, ListPiece};
pub(crate) use template::{
    compile_text, instantiate_fixed, instantiate_template, number_vocabulary, occurrences_of, parse_keys,
    AncestorKeys, Binding, Bindings,
};
pub use value::{identifier_text, CounterValue};
