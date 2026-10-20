//! The `citations` feature: citing works by key, numbering the cited works in
//! order of first citation, showing a citation mark where each citation is
//! written, and listing the cited works after the document's content.
//!
//! # What an author writes
//!
//! `\cite[note]{key1,key2}` cites the works whose citation keys are `key1`
//! and `key2`; it is shown as a **citation mark**, such as `[1,3–5]`, that holds the numbers of the cited
//! works, each number a link to the work in the list of cited works.
//! `\R[flags]\cite{keys}` writes the citation as a noun of its sentence: a
//! word, `ref.~` for one number and `refs.~` for several, comes before the
//! citation mark (the citation counter style holds the words; see "The
//! configuration" below), so `\R\cite{a,b}` reads `refs.~[1,2]`. After the content, the list of cited works shows each work's
//! **citation entry**, such as `Knuth, The Art of Computer Programming, 1984`,
//! labeled with its number. The language guide describes both commands for
//! authors ([Citations](crate::guide::language_syntax#citations)).
//!
//! # Citation keys and notes
//!
//! A **citation key** refers to a cited work: it is the FLM code written for it
//! between the braces of `\cite`, recomposed from the parsed nodes, with
//! every run of whitespace made one space and the ends trimmed
//! ([`spelling_identity`](crate::stdcore::backends::pres::spelling_identity)):
//! `arxiv:1234.5678`, `knuth84`, `manual:{A. Author, Some title (2024)}`. A
//! comma inside a group, a command's argument or a formula belongs to the
//! citation key, and a citation key may hold a command. The feature reads no
//! meaning into a citation key: the citation providers decide what it refers to
//! (see "Citation entries and citation providers" below).
//!
//! A **note** is the optional argument of a `\cite`: content shown after the
//! numbers (`[31; Thm 4]`). It belongs to every citation key of that call.
//!
//! # Citation chunks
//!
//! `\cite` calls written directly one after the other, with nothing between
//! them but whitespace, a single line break and comments, are one **citation
//! chunk**, shown as one citation mark. A citation chunk is one `\cite` node
//! ([`CITE`], `flm.citations.cite`) whose last argument, a tack-on argument
//! (the trailing argument that absorbs field macros written after a
//! construct; see [fields](crate::guide::concepts_overview#fields-data-a-construct-reads))
//! named [`MORE_ARG`], holds the further calls as fields named `cite`
//! ([`CITE_FIELD`]). A paragraph break or any other content ends the citation
//! chunk.
//!
//! [`read_chunk`] reads a citation chunk as a list of **items**, each a
//! citation key and the note of its call, in the order written:
//! `\cite{b}\cite[Thm 4]{a,c}` gives `(b)`, `(a, Thm 4)`, `(c, Thm 4)`. The
//! keys argument of a call is content, split at the commas that are outside
//! every group (`\cite{a,{b,c},d}` gives `a`, `{b,c}`, `d`); each part is
//! recomposed into its citation key, and an empty part is skipped. Because a
//! citation key is recomposed from the nodes rather than copied from the
//! source text, a citation key that a user definition writes into a `\cite`
//! is the one the expansion produced: `\cite{manual:\emph{#1}}`, used with the
//! argument `{Title}`, gives `manual:\emph{Title}`.
//!
//! `\R` ([`REFCITE`], `flm.citations.refcite`) takes an optional flags
//! argument (`S` for the capitalized word, `Refs.~`; `I` for no word) and,
//! as a field of its tack-on argument [`CITATION_ARG`], the `\cite`
//! written directly after it, together with that call's further calls. In
//! `\R[S]{\cite{a}}` the `\cite` is inside a group, so the `\R` holds no
//! citation.
//!
//! The citation keys and the flags are not words of the text around them
//! ([`ApartFromText`](crate::model::aspects::ApartFromText)); the notes are. A
//! heading without a label gets an identifier encoded from the text of its
//! title; when the title holds `\cite[Thm 4]{a}`, that text is the title's own
//! words and `Thm 4`, without the citation key `a`.
//!
//! # Cited works, numbers and identifiers
//!
//! A **cited work** is the article or book that a citation key refers to.
//! Two citation keys may refer to one work (`arXiv:1211.1037` and
//! `arxiv:1211.1037`), and only a citation provider can know it: its answer
//! gives the work's **identity**, one spelling that every citation key of the
//! work shares ([`CitationAnswer`]). The library compares identities as
//! characters. A citation key that no citation provider answered refers to the work
//! whose identity is the citation key as written, so a citation key spelled
//! exactly like a work's identity is a citation key of that work.
//!
//! The collect stage `flm.citations.collect` ([`COLLECT`]) walks every
//! citation chunk of the document in document order and records each distinct
//! citation key at its first citation, with where it is first cited, in the
//! table [`KEYS`] (`flm.citations.keys`).
//!
//! After the citation providers have been asked (below), the retrieve stage
//! numbers the cited works ([`number_works`]): one work per distinct identity,
//! in the order of the first citation of any of its citation keys. It writes
//! the table [`CITATIONS`] (`flm.citations`): for each citation key, its
//! work's number, its work's identifier `citation-<n>` and its work's
//! identity, the citation keys of one work sharing all three. The numbers
//! therefore depend only on the document and on the citation providers'
//! answers. The identifier prefix [`IDENTIFIER_PREFIX`], `citation-`, belongs
//! to this feature, and no other feature creates identifiers with it. A cited
//! work is not a [reference point](crate::model::aspects::RefPoint), and a
//! citation is not a reference: nothing refers to a cited work by label.
//!
//! # The citation mark
//!
//! [`stage_mark`] stages (builds into the lowered tree; see
//! [`Lowerer`](crate::core::lowering::Lowerer)) a citation chunk's citation
//! mark: a span of role [`MARK_ROLE`] (a role is a name on a presentation
//! node that says what it is; see [roles and kind
//! tokens](crate::stdcore::backends::pres::payload)) whose numbers are links of
//! kind `cite` to their works' identifiers, with the delimiters and the join
//! texts outside the links. The items without a note come first, in one group,
//! their numbers sorted and compressed into ranges as the citation counter
//! style says (see "The configuration" below) (`[1–3,5]`); then
//! one group per distinct note, in the order of the note's first appearance.
//! With `a`, `b`, `c`, `d`, `e` cited in that order earlier,
//! `\cite{c,b}\cite[Thm 4]{a}\cite{d}\cite[Thm 4]{e}` reads `[2–4][1,5; Thm
//! 4]`. Each work appears once in a group: with `arXiv:1211.1037` and
//! `arxiv:1211.1037` one work, `\cite{arXiv:1211.1037,arxiv:1211.1037}` reads
//! `[1]`.
//!
//! The citation mark of a `\cite` is **attached** to the content before it
//! (kind token [`ATTACHED`]): the whitespace written before `\cite` is
//! removed, and a no-break space is written before the citation mark
//! ([`space_before`]), as a character in a span of role [`SPACE_ROLE`], under
//! the bracketed form of the citation mark and under the in-place placement;
//! under the superscript form nothing is written (the form and the placement
//! are settings of the configuration; see "The configuration" below). The
//! content around a citation mark is not read: a `\cite` at the start of a
//! paragraph or after `(` also has its space written, and an author writes
//! `\R[I]\cite{a}` there instead. The citation mark of an `\R` is
//! **standalone** (kind token [`STANDALONE`]): shown inline, never raised, with
//! no space written before it and none removed.
//!
//! # Citation entries and citation providers
//!
//! The **citation entry** of a cited work ([`CitationEntry`]) is what is shown
//! for it: inline content written with the commands of the registry's
//! language (text, `\emph`, a link, a formula), which may not declare a label
//! (see [`CitationEntry`]). The library performs no input or output: the citation
//! entries come from **citation providers** ([`CitationProvider`]), which the
//! host contributes to the service slot [`PROVIDERS_SLOT`]
//! (`flm.citations.providers`; see
//! [`service_slot`](crate::core::registry::FeatureBuilder::service_slot) and
//! [services](crate::core::services)). The service slot takes any number of
//! citation providers and the feature never requires one. A citation provider
//! returns, for a citation key, an answer holding its citation entry, "not
//! mine", or an error.
//!
//! The retrieve stage `flm.citations.retrieve` ([`RETRIEVE`]) asks the
//! citation providers about the cited keys, in the order of precedence of the
//! service slot (the providers of the feature placed last in the feature order
//! first, each feature's in the order it contributed them, the host's last;
//! [`PROVIDERS_SLOT`]): the first citation provider receives every citation key in one call, in
//! order of first citation ([`CitationProvider::answer_all`]), and each later
//! one receives, in one call, the citation keys that every citation provider
//! before it answered "not mine". A citation provider written for one citation
//! key at a time implements [`CitationProvider::answer`] alone. The first
//! citation provider that claims a citation key decides it: its answer
//! ([`CitationAnswer`]: the citation entry and the work's identity), or an
//! error that is final for that citation key. The stage writes the table
//! [`ENTRIES`] (`flm.citations.entries`): for each citation key, its answer or
//! the reason it has no citation entry ([`MissingEntry`]); then it numbers the
//! cited works. Each citation entry is read as the document would show it,
//! with user definitions expanded, and is refused when it holds a citation or
//! block content (a paragraph break, a list, a heading). A citation key with no
//! citation entry is numbered and shown in citation marks like any other.
//!
//! A citation provider makes a citation entry from FLM source
//! ([`ProviderCx::parse_entry`]) or from parts ([`EntryPiece`],
//! [`ProviderCx::build_entry`]): characters, FLM source, emphasis, strong
//! importance and links. Each part with a meaning is written as the command of
//! the feature that owns that meaning (`\emph`, `\textbf`, `\href`), so the
//! citation entry is shown exactly as the same content written in FLM.
//!
//! # The check that every cited key has a citation entry
//!
//! The check stage `flm.citations.check` ([`CHECK`]) reports every cited key
//! that has no citation entry ([`NoEntry`]), one report per citation key, even
//! when several citation keys refer to one work. It provides a member of the
//! collection [`flm.validated`](crate::stdcore::VALIDATED), which every
//! workflow demands (asks the plan to run a stage that provides it; see
//! [`PlanRequest::demand`](crate::core::plan::PlanRequest::demand)), so the
//! check runs under every workflow, the `check` workflow included; because it
//! reads the citation entries, the citation providers are asked under every
//! workflow.
//!
//! # The list of cited works
//!
//! The **bibliography** is a piece ([`BIBLIOGRAPHY`],
//! `flm.citations.bibliography`): a document-level [synthesized
//! value](crate::guide::concepts_overview#synthesized-values) that a
//! backend places beside the document's content
//! ([`Piece`](crate::core::piece::Piece)), here after it. It holds a heading
//! (the configured title, `References` by default, at the configured level, 2
//! by default; no heading where the configuration sets no title) and one
//! citation entry per cited work, in the order of the numbers, each labeled
//! with its number (`[1]`) and carrying its work's identifier. The citation
//! entry shown for a work is the one answered for the citation key under which
//! the work was first cited; a work with no citation entry shows a dropped
//! mark (a `flm.pres.dropped` node; see
//! [recovery](crate::stdcore::backends::lowering::error)) in its place.
//!
//! [`build_list`] builds such a list from any set of cited works; the builder
//! stage `flm.citations.build-bibliography` ([`BUILD_BIBLIOGRAPHY`]) calls it
//! over every cited work of the document. The feature contributes the piece's
//! declaration to the `pieces` feature's service slot as a peer contribution
//! ([`FeatureBuilder::peer_slot`](crate::core::registry::FeatureBuilder::peer_slot)):
//! without the `pieces` feature, the list is built and no backend places it.
//! Two pieces placed after the content are placed in the order their features
//! contributed them; in the standard package
//! ([`stdcore::package`](crate::stdcore::package)) the `xnotes` feature comes
//! first, so the list of cited works follows the lists of xnotes (notes set
//! apart from the running text, such as footnotes; see
//! [`xnotes`](crate::stdcore::features::xnotes)). On the `latex-recomposed`
//! backend ([`latex`]), no list is placed and no citation entry is written:
//! LaTeX writes its own list of cited works there.
//!
//! # The placements of the citation entries
//!
//! The configuration's **placement** ([`CitationPlacement`]) decides where
//! the citation entries are shown. Under *listed*, the default, the
//! bibliography is placed after the content and the citation marks show
//! numbers. Under *in place*, a citation mark shows, for each item in the
//! order written, the item's citation entry between the citation counter
//! style's delimiters, then the note separator and the note:
//! `[Knuth, 1984][Lamport, 1994; Thm 4]`. The citation mark then holds no
//! link and carries the kind token [`IN_PLACE`], and no list is placed. A
//! `\cite` writes its no-break space before an in-place citation mark, and an
//! `\R` writes its word.
//!
//! # The configuration
//!
//! [`CitationsConfig`] is the feature's configuration, fixed when the registry
//! is built ([configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
//! It holds:
//!
//! - the placement of the citation entries (see above);
//! - the **form** of a citation mark ([`MarkForm`]): bracketed, the default,
//!   or superscript. Under the superscript form, no delimiters are written
//!   wherever numbers are shown, the numbers of a `\cite` are raised, and
//!   those of an `\R` stay inline. The form has no effect under the in-place
//!   placement;
//! - the **citation counter style**, the counter style
//!   ([`CounterStyleDecl`](crate::stdcore::features::numbering::CounterStyleDecl))
//!   that writes the numbers of the cited works, the delimiters, the texts
//!   joining several numbers and the words `\R` writes. This feature compiles
//!   it when the registry is built, and the counter styles a host sets in the
//!   numbering feature's configuration
//!   ([`NumberingConfig::styles`](crate::stdcore::features::numbering::NumberingConfig::styles))
//!   do not replace it;
//! - the **heading** above the list of cited works (its title and its level,
//!   or no heading);
//! - the text written before a note.
//!
//! # What the feature registers
//!
//! - Elements: `\cite` ([`CITE`]) and `\R` ([`REFCITE`]), available in text
//!   mode; the synthesized list and list-entry elements of the bibliography
//!   ([`BIBLIOGRAPHY`], [`BIBLIOGRAPHY_ENTRY`]), which no author writes.
//! - The argument role [`KEY_ARG_ROLE`] (`flm.citations.key`), which states
//!   that an argument holds citation keys
//!   ([`ArgDef::role`](crate::lang::mint::ArgDef::role)): the role of the keys
//!   argument of `\cite` and of the `cite` fields of `\cite` and `\R`.
//! - Tables: [`KEYS`], [`ENTRIES`], [`CITATIONS`] and the piece table
//!   [`BIBLIOGRAPHY`] ([`bibliography_table`]).
//! - Stages, all of document scope: collect ([`COLLECT`]); retrieve
//!   ([`RETRIEVE`]), which reads what collect writes; and the bibliography's
//!   builder ([`BUILD_BIBLIOGRAPHY`]) and check ([`CHECK`], which also
//!   provides the capability of that name), which read what retrieve writes.
//! - The service slot [`PROVIDERS_SLOT`], which the host fills with citation
//!   providers.
//! - The settings of the feature (see "The configuration").
//! - The stylesheet `flm.citations.style` ([`html::stylesheet`]) and the
//!   definition piece `flm.latex.def.citations` ([`latex::definition_piece`]),
//!   the latter contributed to the style file's service slot as a peer
//!   contribution.
//! - Lowering rules: on every backend, the general rules of `\cite` and `\R`
//!   ([`CITE_RULE`], [`REFCITE_RULE`]), which stage the citation mark, and of
//!   the list of cited works ([`BIBLIOGRAPHY_RULE`]), which stages a block
//!   holding a heading and a labeled list. The general rules of `\cite` and
//!   `\R` require the feature's style identifier `flm.citations.style`
//!   ([`html::STYLE`]), which on the `html` backend names the stylesheet, so
//!   a page holding no citation does not carry it.
//!   On the `latex-recomposed` backend, rules ([`latex`]) that write a
//!   citation as the author wrote it through three LaTeX hooks, and write
//!   nothing for the list. The rendering guide shows the output on each
//!   backend ([Citations](crate::guide::rendering#citations), [Citations in
//!   HTML](crate::guide::rendering#citations-in-html), [Citations in the
//!   recomposed LaTeX](crate::guide::rendering#citations-in-the-recomposed-latex)).
//! - A build hook ([`LateInit`](crate::core::registry::LateInit)) that
//!   compiles the citation counter style, the note separator and the title of
//!   the list: a text that is not valid FLM fails
//!   the registry build.
//!
//! # Dependencies
//!
//! The feature requires the element index (the `index` feature,
//! [`index`](crate::stdcore::features::index)), whose table the stages walk,
//! and the presentation vocabulary of the `lowering` feature. It does not
//! depend on the `numbering` feature (it uses the counter-style code, not the
//! counters), nor on `refs` or `xnotes`. The `latex-recomposed` backend,
//! the LaTeX family's constructs and the `pieces` feature are peer
//! declarations ([`PeerName`](crate::core::registry::PeerName)) and peer
//! contributions: the feature builds without them, with its rules for an
//! absent backend set aside (excluded from rule selection; see
//! [`PeerName`](crate::core::registry::PeerName)).
//!
//! # Map of the items
//!
//! - The feature: [`Citations`], [`feature`], [`FEATURE`]; its configuration
//!   [`CitationsConfig`], [`CitationPlacement`], [`MarkForm`],
//!   [`default_style`].
//! - The elements and the citation chunk reader: [`CITE`], [`REFCITE`],
//!   [`CitationElement`], [`read_chunk`], [`calls`], [`leading_call`],
//!   [`Chunk`], [`CitationItem`], [`ChunkMistake`], [`Reading`].
//! - The cited keys and the numbers: [`KEYS`], [`CitedKeys`], [`CITATIONS`],
//!   [`CitedWorks`], [`CitedWork`], [`number_works`].
//! - The citation providers: [`PROVIDERS_SLOT`], [`CitationProvider`],
//!   [`CitationRequest`], [`CitationAnswer`], [`CitationEntry`],
//!   [`EntryPiece`], [`ProviderCx`], [`CitationProviderError`].
//! - The citation entries: [`ENTRIES`], [`CitationEntries`], [`Retrieval`],
//!   [`MissingEntry`].
//! - The citation mark: [`stage_mark`], [`StagedMark`], [`MarkSettings`],
//!   [`space_before`], [`Standing`], [`CiteRule`], [`RefCiteRule`],
//!   [`report_mistakes`], and the role and kind-token constants.
//! - The list of cited works: [`build_list`], [`ListedWork`],
//!   [`ListedEntry`], [`BibliographyRule`], [`BIBLIOGRAPHY`].
//! - The diagnostics: [`NoKey`], [`NoCitation`], [`UnknownFlag`],
//!   [`Uncollected`], [`NoProvider`], [`ProviderFailed`],
//!   [`ProviderWarning`], [`EntryCites`], [`EntryNotInline`], [`NoEntry`],
//!   [`StageFailed`].
//! - The stylesheet: [`html`]. The `latex-recomposed` backend's rules:
//!   [`latex`]. The `latex-rendered` backend's definition piece:
//!   [`latex_rendered`].

pub mod html;
pub mod latex;
pub mod latex_rendered;

mod bibliography;
mod check;
mod config;
mod diagnostics;
mod elements;
mod entries;
mod mark;
mod numbers;
mod providers;

use std::sync::Arc;

use crate::core::settings::SettingsDecl;
use crate::core::registry::{BuildError, CapabilityDef, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::core::services::Arity;
use crate::lang::mint::{ElementDef, SyntaxDef};
use crate::model::names::FeatureName;
use crate::stdcore::VALIDATED;

pub use bibliography::{
    bibliography_table, build_list, BibliographyRule, ListedEntry, ListedWork, BIBLIOGRAPHY,
    BIBLIOGRAPHY_ENTRY, BIBLIOGRAPHY_ENTRY_RULE, BIBLIOGRAPHY_RULE, BUILD_BIBLIOGRAPHY,
    ENTRY_NUMBER_ROLE, KIND_CITATIONS,
};
pub use check::CHECK;
pub use config::{
    default_style, CitationPlacement, CitationsConfig, MarkForm, CAPITAL_VARIANT, STYLE_NAME,
};
pub use diagnostics::{
    EntryCites, EntryNotInline, NoCitation, NoEntry, NoKey, NoProvider,
    ProviderFailed, ProviderWarning, Uncollected, UnknownFlag,
};
pub use elements::{
    calls, cite_args, cite_element, leading_call, read_chunk, refcite_args, refcite_element,
    CitationElement, CitationItem, Chunk, ChunkMistake, Reading, CITATION_ARG, CITE, CITE_FIELD,
    FLAGS_ARG, KEYS_ARG, KEY_ARG_ROLE, MORE_ARG, NOTE_ARG, REFCITE,
};
pub use entries::{CitationEntries, MissingEntry, Retrieval, ENTRIES, RETRIEVE};
pub use providers::{
    CitationAnswer, CitationEntry, CitationProvider, CitationProviderError, CitationRequest,
    CitationResult, EntryPiece, ProviderCx, PROVIDERS_SLOT,
};
pub use mark::{
    report_mistakes, space_before, stage_mark, CiteRule, MarkSettings, RefCiteRule, Standing,
    StagedMark, ATTACHED, CAPITAL_FLAG, CITE_RULE, INLINE_FLAG, IN_PLACE, LINK_KIND, MARK_ROLE,
    NO_BREAK_SPACE, REFCITE_RULE, SPACE_ROLE, STANDALONE, WORD_ROLE,
};
pub use numbers::{
    number_works, CitedKeys, CitedWork, CitedWorks, StageFailed, CITATIONS, COLLECT, KEYS,
    IDENTIFIER_PREFIX,
};

/// The feature's registered name, `citations`.
pub const FEATURE: &str = "citations";

/// The declaration of the feature's settings: the fields of [`CitationsConfig`]
/// are its settings keys, and the texts among them are FLM markup. See
/// [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<CitationsConfig>(
            "Where the citation entries are shown (`placement`: `listed` after the content, or \
            `in-place` in the citation marks), how a citation mark shows its numbers (`mark`: \
            `bracketed` or `superscript`), the citation counter style (`style`: the number format, the \
            delimiters, the texts joining several numbers, whether consecutive numbers make a \
            range, and the words `\\R` writes), the heading above the list of cited works \
            (`heading_title`, or null for no heading, and `heading_level`), and the text written \
            between the numbers and a note (`note_separator`). Fixed when the registry is built: \
            it cannot vary per document or per render.",
        )
        .markup("heading_title")
        .markup("note_separator")
        .markup("style.prefix_words")
        .markup("style.number_format")
        .markup("style.sub_levels")
        .markup("style.delimiters")
        .markup("style.join")
}

/// The `citations` feature ([`FlmFeature`]): it registers the elements
/// `\cite` and `\R`, the collect, retrieve, check and bibliography-builder
/// stages with their tables (the cited keys, the citation entries, the cited
/// works, the bibliography piece), the service slot of the citation
/// providers, and the lowering rules of the citation marks and of the list of
/// cited works on every backend, the `html` backend and the recomposed LaTeX.
/// The [module documentation](self) describes the feature in full.
///
/// Construct it with [`Citations::new`] from a [`CitationsConfig`], or take
/// the default configuration with [`feature`]; the standard package
/// ([`stdcore::package`](crate::stdcore::package)) registers it from
/// [`StdcoreConfig::citations`](crate::stdcore::StdcoreConfig::citations).
#[derive(Debug, Clone, Default)]
pub struct Citations {
    config: CitationsConfig,
}

impl Citations {
    /// The feature constructed from `config`, which stays fixed for the life
    /// of every registry the feature is built into.
    pub fn new(config: CitationsConfig) -> Citations {
        Citations { config }
    }

    /// The configuration this feature was constructed with.
    pub fn config(&self) -> &CitationsConfig {
        &self.config
    }
}

impl FlmFeature for Citations {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Citations").docs(
            "Citing works by key: `\\cite[note]{keys}`, adjacent calls read as one citation \
             chunk, and `\\R\\cite{keys}`, a citation written as a noun of its sentence. The \
             host's citation providers give each cited citation key its citation entry and say \
             which citation keys refer to the same work; the works are numbered in order of first citation, each \
             citation chunk is shown as a citation mark linking to its works, and the works are \
             listed after the content, or their citation entries are shown in the citation marks.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner: FeatureName = b.feature().clone();
        b.argument_role(
            KEY_ARG_ROLE,
            "Citation keys, each identifying a cited work, separated by commas.",
        )?;
        b.element(
            ElementDef::new(owner.clone(), ["cite"], SyntaxDef::Macro { args: cite_args() }, cite_element())
                .docs(
                    "`\\cite[note]{keys}`: cites works by key; adjacent calls are one citation \
                     chunk, attached to the content before it.",
                ),
        )?;
        b.element(
            ElementDef::new(owner.clone(), ["R"], SyntaxDef::Macro { args: refcite_args() }, refcite_element())
                .docs(
                    "`\\R[flags]\\cite[note]{keys}`: a citation chunk written as a noun of its \
                     sentence, after a word, `ref.~` or `refs.~` by default.",
                ),
        )?;
        b.table_named::<numbers::CitedKeys>(
            KEYS.name(),
            "Every citation key the document cites, once, in order of first citation, with where \
             it is first cited.",
        )?;
        b.table_named::<numbers::CitedWorks>(
            CITATIONS.name(),
            "The cited works of the document: for each citation key, the work it refers to, with \
             the work's number (in order of the first citation of any of its keys), its \
             identifier `citation-<n>` and its identity.",
        )?;
        b.stage(Arc::new(numbers::Collect))?;
        b.service_slot::<dyn CitationProvider>(
            PROVIDERS_SLOT,
            Arity::OrderedMany,
            "The citation providers, which give the citation entries of the cited works and are \
             asked in order for each cited key; the feature never requires one.",
        )?;
        b.table_named::<entries::CitationEntries>(
            ENTRIES.name(),
            "The citation entries of the cited works: for each citation key, its citation entry \
             or the reason it has none.",
        )?;
        b.stage(Arc::new(entries::Retrieve))?;
        b.capability(CapabilityDef::new(CHECK).docs(
            "The citations have been checked: every cited key with no citation entry was \
             reported as an Error. A member of `flm.validated`.",
        ))?;
        b.stage(Arc::new(check::Check))?;
        b.collection_member(VALIDATED, CHECK)?;
        let settings = Arc::new(mark::MarkSettings::new(self.config.clone()));
        b.late_init(Arc::new(mark::CompileSettings { settings: Arc::clone(&settings) }))?;
        for rule in mark::rules(&settings) {
            b.lowering_rule(rule)?;
        }
        bibliography::declare(b, &owner, &settings)?;
        html::register(b)?;
        latex_rendered::register(b)?;
        latex::register(b, &settings)?;
        b.digest_input(&self.config.digest_bytes())?;
        Ok(())
    }
}

/// The `citations` feature constructed from the default configuration
/// ([`CitationsConfig::new`]).
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Citations::default())
}
