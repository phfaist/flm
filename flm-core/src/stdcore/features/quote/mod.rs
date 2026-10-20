//! The `quote` feature: quotations, blocks set apart from the running text
//! whose content is organized in quotation sections (a quoted passage, lines
//! set as written, an attribution, ordinary paragraphs). The feature declares
//! the quote environments `quote`, `blockquote` and `address` by default, and
//! any other quote environment a host configures.
//!
//! # What an author writes
//!
//! A **quote environment** is an environment whose content is a quotation.
//! The content is made of **quotation sections**, each written with a
//! quotation-section command:
//!
//! - `\text{…}` — a quoted passage, one paragraph;
//! - `\lines{…}` — lines set as written, such as a poem or an address: each
//!   `\\` ends a line, and each `\indent` at the start of a line indents that
//!   line by one tab, a unit of indentation whose width each backend decides
//!   ([`Length`](crate::stdcore::backends::pres::Length));
//! - `\attributed{…}` — the attribution: an author or a source;
//! - `\block{…}` — ordinary paragraphs and blocks, lists included.
//!
//! ```text
//! \begin{quote}                          To be or not to be.
//!   \text{To be or not to be.}
//!   \attributed{Shakespeare}             Shakespeare
//! \end{quote}
//!
//! \begin{address}                        123 Main St
//! 123 Main St\\                          City, ST 12345
//! City, ST 12345
//! \end{address}
//! ```
//!
//! The right column is what the text backend prints. Content written in the
//! body of a quote environment outside any quotation-section command is the
//! environment's **bare content**. The configuration of each environment
//! ([`QuoteEnvironment`]) states which quotation-section commands its body
//! accepts, and which kind of quotation section its bare content is read as,
//! if any; where it states none, bare content is an error. The default
//! configuration ([`QuoteConfig`]) declares three environments:
//!
//! - `quote` accepts the four quotation-section commands and reads no bare
//!   content;
//! - `blockquote` accepts no quotation-section command and reads its content
//!   as one block section;
//! - `address` accepts no quotation-section command and reads its content as
//!   one lines section.
//!
//! A host declares other environments with any combination.
//!
//! A quotation-section command can be written only in text mode, and only
//! inside the body of a quote environment that accepts it: the commands of an
//! environment are declared in a scoped package
//! ([`ScopedPackageDef`](crate::lang::mint::ScopedPackageDef)) of that
//! environment, a set of element declarations that can be written only inside
//! the environment bodies and arguments whose declarations refer to the
//! package ([scoped packages](crate::guide::concepts_overview#scoped-packages)).
//! Inside a formula, `\text{…}` is therefore mathematics' own text command,
//! never a quotation section. The guide for authors describes quotations
//! under [Quotations](crate::guide::language_syntax#quotations).
//!
//! # What the feature provides to the pipeline
//!
//! **Elements.** Every quote environment is one element value of the element
//! type [`QUOTE`] (`flm.quote`), carrying a [`QuoteElement`]. It is a block,
//! takes no argument, and is declared in text mode. Its body is laid out as
//! paragraphs and blocks, or as inline content where its bare content is read
//! as a passage, lines or an attribution
//! ([`QuoteEnvironment::holds_blocks`]). The four quotation-section commands
//! are four element values of the element type [`SECTION`]
//! (`flm.quote.section`), each carrying a [`SectionElement`]; each is a macro
//! with one mandatory argument, [`CONTENT_ARG`]. A quotation-section command
//! is in **section position** when it is written directly in the body of a
//! quote environment that accepts it. There it opens a quotation section, and
//! where the body is laid out as paragraphs and blocks, it is a block of its
//! own between the others. Anywhere else it is inline content.
//!
//! **Scoped packages.** The feature declares one scoped package per quote
//! environment that accepts at least one quotation-section command, whose
//! name [`sections_package`] returns (`flm.quote.sections.quote` for
//! `quote`), holding exactly the commands that environment accepts. It also declares
//! the scoped package [`LINES_PACKAGE`] (`flm.quote.lines`), which holds
//! `\\` and `\indent`, the two commands of a lines section. Both are field
//! macros, macros that carry data rather than content
//! ([fields](crate::guide::concepts_overview#fields-data-a-construct-reads)):
//! they write nothing of their own, and the lowering rules read them in place
//! among the line's content. The argument of `\lines` and the body of an
//! environment whose bare content is lines refer to the package.
//!
//! **Reading a quotation's body.** The rules below read the body's direct
//! children in order. A quotation-section command in section position opens
//! a quotation section. Each maximal run of other content between two
//! quotation sections, without the whitespace and source comments at its
//! edges, is one **bare run**: it is lowered exactly as a quotation section
//! of the environment's bare-content kind would be lowered over the same
//! content. The rules read the body each time they lower a quotation, and
//! store nothing.
//!
//! **Lowering rules.** A lowering rule ([`LoweringRule`]) says how an element
//! becomes output on a backend. The feature registers these rules. A rule
//! marked as the default for one backend ranks above a rule marked as the
//! default everywhere ([`DefaultMark`]):
//!
//! - [`QUOTE_RULE`] ([`QuoteRule`]), the default on every backend that has no
//!   rule of its own for a quotation: it stages
//!   (builds into the lowered tree; [`Lowerer`](crate::core::lowering::Lowerer))
//!   one `flm.pres.block` of role [`QUOTE_ROLE`] (`flm.quote`), whose one kind
//!   token is the environment's name, holding one `flm.pres.block` per
//!   quotation section in the order written, of the role of the section's
//!   kind ([`TEXT_ROLE`], [`LINES_ROLE`], [`ATTRIBUTED_ROLE`] or
//!   [`BLOCK_ROLE`]). A role is a feature-owned dotted name on a presentation
//!   payload that says what a container is, and kind tokens classify it
//!   further ([`payload`](crate::stdcore::backends::pres::payload)). A passage
//!   and an attribution hold one paragraph, a block section holds its
//!   paragraphs and blocks, and a lines section holds one `flm.pres.lines`
//!   with one `flm.pres.line` per line, the line's left indentation a
//!   [`Length`](crate::stdcore::backends::pres::Length) of as many tabs as
//!   the line has leading `\indent`s. Every quote environment has the same
//!   role, so that a stylesheet or a lowering rule treats them alike; the
//!   kind token selects one environment. The text backend uses this rule: it
//!   prints a quotation as its quotation sections, one blank line apart, with
//!   no prefix and no indentation of the whole.
//! - [`SECTION_RULE`] ([`SectionRule`]), the default on every backend that
//!   has no rule of its own for a quotation-section command: a
//!   quotation-section command outside section position is lowered as its
//!   argument's content.
//! - On the `html` backend, the feature's stylesheet `flm.quote.style`
//!   ([`html`]), which [`QuoteRule`] requires at every quotation. The HTML
//!   backend writes the quotation as a `<blockquote>` and each quotation
//!   section as a `<div>`.
//! - On the `latex-rendered` backend, the definition piece
//!   `flm.latex.def.flmx.quote` ([`latex_rendered`]), which the same
//!   requirement brings into the style file.
//! - [`latex::LATEX_RULE`] ([`latex::QuoteLatexRule`]), the default on the
//!   `latex-recomposed` backend: it writes the environment as the author
//!   wrote it, each quotation section through a LaTeX hook (`\flmQuoteText`,
//!   `\flmQuoteLines`, `\flmQuoteAttributed`, `\flmQuoteBlock`) and a line's
//!   indentation through `\flmIndent`; the feature's definition piece
//!   `flm.latex.def.quote` defines them in the style file ([`latex`]).
//!
//! **Settings.** The feature's configuration, [`QuoteConfig`], is the value
//! the feature is constructed from; it is fixed when the registry is built
//! ([configuration](crate::guide::concepts_overview#settings-options-and-configuration)),
//! because the environments and their commands are part of the language. The
//! same value written as data is the feature's settings: the settings key
//! `environments` maps each environment's name to its [`QuoteEnvironment`],
//! and a host that combines several sources of settings combines that map
//! one environment at a time
//! ([`SettingsDecl::merge_entries`](crate::core::settings::SettingsDecl::merge_entries)).
//! The feature feeds the configuration into the [registry
//! digest](crate::model::digest), the number that changes whenever something
//! about the registry that can change what processing produces changes
//! ([`FeatureBuilder::digest_input`]); [`QuoteConfig::digest_bytes`] gives the
//! bytes. It declares no option type.
//!
//! # Errors
//!
//! The feature reports three conditions
//! ([diagnostics](crate::guide::concepts_overview#diagnostics)), each an Error, while a quotation is
//! lowered, on every backend; [`diagnostics`] gives each one's recovery, the
//! way the lowering goes on after it
//! ([recovery](crate::stdcore::backends::lowering::error)):
//!
//! - [`BareContent`] (`flm.quote.bare-content`) — bare content in an
//!   environment that reads none, such as text written directly in `quote`;
//!   reported once per bare run;
//! - [`NestedSection`] (`flm.quote.nested-section`) — a quotation-section
//!   command written anywhere other than directly in the body of a quote
//!   environment, such as inside another quotation section's argument
//!   (`\lines{\text{x}}`);
//! - [`IndentMidLine`] (`flm.quote.indent-mid-line`) — an `\indent` after the
//!   first content of its line.
//!
//! A quotation-section command that an environment does not accept is not
//! declared in that environment's body, so writing one there is an unknown
//! command, part of the bare content. One case differs. Inside a quotation
//! nested in a quotation section of another quotation, the enclosing
//! quotation's commands can still be written, because the scoped packages of
//! the enclosing bodies remain available. Such a command, written directly in
//! the nested body that does not accept it, is part of the nested body's bare
//! content: where the nested environment reads no bare content, it is
//! reported with its bare run, and it is shown as its argument's content.
//!
//! # Interactions with other features
//!
//! - **Block structure.** The block role of a quotation-section command is
//!   decided per occurrence ([`SectionElement`]), and the block-structure
//!   pass ([`blocks`](crate::lang::rewrites::blocks)) lays the body out from
//!   it.
//! - **Lists.** The environment's name becomes a kind token of the
//!   quotation's block, so it may not be one of the kind tokens a
//!   presentation list reserves
//!   ([`LIST_RESERVED_KINDS`](crate::stdcore::backends::pres::LIST_RESERVED_KINDS));
//!   [`QuoteConfig::validate`] refuses such a name.
//! - **Other features' environments.** An environment name that another
//!   feature also declares, such as `itemize`, makes the registry build fail,
//!   as any two declarations of one name do.
//! - **Margin notes on the page.** The feature's stylesheet sets
//!   `--flm-margin-note-inset` on a quotation, so that a note shown in the
//!   margin and written inside the quotation still reaches the page's column
//!   of notes ([`html`]).
//! - **The LaTeX family.** The rule of the `latex-recomposed` backend
//!   ([`latex::QuoteLatexRule`]) uses the LaTeX hooks and definition pieces
//!   of the [LaTeX family](crate::stdcore::backends::latex), the code shared
//!   by the LaTeX backends, and writes a line's indentation with the family's
//!   length `\flmTabWidth` ([`latex`]).
//! - **The standard package.** The [standard package](crate::stdcore)
//!   lists the feature under the configuration
//!   [`StdcoreConfig::quote`](crate::stdcore::StdcoreConfig::quote).
//!
//! # Items
//!
//! - [`Quote`] and [`feature`] — the feature, under a configuration or the
//!   default one.
//! - [`QuoteConfig`], [`QuoteEnvironment`], [`QuoteSection`] — the
//!   configuration: the environments, what each accepts, the four kinds of
//!   quotation section.
//! - [`QuoteElement`], [`SectionElement`] — the per-value data of the two
//!   element types [`QUOTE`] and [`SECTION`].
//! - [`QuoteRule`], [`SectionRule`] — the general lowering rules; [`html`]
//!   and [`latex`] — the rules of the `html` and `latex-recomposed` backends.
//! - [`diagnostics`] — the three conditions.
//! - The names: [`FEATURE`], [`QUOTE`], [`SECTION`], [`CONTENT_ARG`],
//!   [`LINES_PACKAGE`], [`SECTIONS_PACKAGE_PREFIX`] and [`sections_package`],
//!   [`LINEBREAK_COMMAND`], [`INDENT_COMMAND`], [`QUOTE_RULE`],
//!   [`SECTION_RULE`], and the roles [`QUOTE_ROLE`], [`TEXT_ROLE`],
//!   [`LINES_ROLE`], [`ATTRIBUTED_ROLE`], [`BLOCK_ROLE`].

mod config;
pub mod diagnostics;
mod elements;
pub mod html;
pub mod latex;
pub mod latex_rendered;
mod lower;

use std::sync::Arc;

use crate::core::settings::SettingsDecl;
use crate::core::lowering::{DefaultMark, LoweringDecl, LoweringRule};
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};

pub use config::{QuoteConfig, QuoteEnvironment, QuoteSection};
pub use diagnostics::{BareContent, IndentMidLine, NestedSection};
pub use elements::{QuoteElement, SectionElement};
pub use lower::{QuoteRule, SectionRule};

/// The feature's registered name, `quote`.
pub const FEATURE: &str = "quote";

/// The declaration of the feature's settings ([`crate::core::settings`]): the
/// fields of [`QuoteConfig`] are its settings keys, and the key
/// `environments` is combined one environment at a time.
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<QuoteConfig>(
            "The quote environments, by name: for each, the quotation-section commands its body \
            accepts (`text`, `lines`, `attributed`, `block`) and the kind of quotation section \
            that content written outside those commands is read as (the key `bare`; when it is \
            absent, such content is an error). Fixed when the registry is built: it cannot vary \
            per document or per render.",
        )
        .merge_entries("environments", || serde_json::to_value(&QuoteConfig::default().environments).map_err(|error| error.to_string()))
}

/// The element type shared by every quote environment, `flm.quote`. Each
/// configured environment is one element value of this type, carrying a
/// [`QuoteElement`].
pub const QUOTE: &str = "flm.quote";

/// The element type shared by the four quotation-section commands,
/// `flm.quote.section`. Each command is one element value of this type,
/// carrying a [`SectionElement`].
pub const SECTION: &str = "flm.quote.section";

/// The name of the one argument of a quotation-section command, `content`:
/// the content of the quotation section.
pub const CONTENT_ARG: &str = "content";

/// The name of the scoped package holding the two commands of a lines
/// section, `\\` and `\indent`: `flm.quote.lines`. Both are field macros:
/// they write nothing, and the lowering rules read them in place
/// ([fields](crate::guide::concepts_overview#fields-data-a-construct-reads)). The declaration of the
/// argument of `\lines` refers to it, and so does the declaration of the body
/// of an environment whose bare content is read as lines.
pub const LINES_PACKAGE: &str = "flm.quote.lines";

/// The prefix of the name of the scoped package holding the
/// quotation-section commands of one quote environment, `flm.quote.sections.`:
/// the package of `quote` is `flm.quote.sections.quote` ([`sections_package`]).
pub const SECTIONS_PACKAGE_PREFIX: &str = "flm.quote.sections.";

/// The name of the field macro that ends a line of a lines section: a single
/// backslash, the name written after the escape character, so that an author
/// writes `\\`.
pub const LINEBREAK_COMMAND: &str = "\\";

/// The name of the field macro that indents a line of a lines section by one
/// tab, `indent`.
pub const INDENT_COMMAND: &str = "indent";

/// The name of the general lowering rule of a quotation,
/// `flm.quote.lower.default` ([`QuoteRule`]), the default on every backend.
pub const QUOTE_RULE: &str = "flm.quote.lower.default";

/// The name of the lowering rule of a quotation-section command outside
/// section position, `flm.quote.section.lower.default` ([`SectionRule`]), the
/// default on every backend.
pub const SECTION_RULE: &str = "flm.quote.section.lower.default";

/// Returns the name of the scoped package holding the quotation-section
/// commands that the quote environment `environment` accepts:
/// [`SECTIONS_PACKAGE_PREFIX`] followed by the environment's name.
///
/// Because of the prefix, no environment name, `lines` included, can produce
/// the name of [`LINES_PACKAGE`].
pub fn sections_package(environment: &str) -> String {
    format!("{SECTIONS_PACKAGE_PREFIX}{environment}")
}

/// The role of the block a quotation is staged as, `flm.quote`, for every
/// quote environment; the environment's name is the block's kind token.
pub const QUOTE_ROLE: &str = "flm.quote";

/// The role of the block a text section (a quoted passage) is staged as,
/// `flm.quote.text`.
pub const TEXT_ROLE: &str = "flm.quote.text";

/// The role of the block a lines section is staged as, `flm.quote.lines`.
pub const LINES_ROLE: &str = "flm.quote.lines";

/// The role of the block an attribution is staged as,
/// `flm.quote.attributed`.
pub const ATTRIBUTED_ROLE: &str = "flm.quote.attributed";

/// The role of the block a block section is staged as, `flm.quote.block`.
pub const BLOCK_ROLE: &str = "flm.quote.block";

/// The `quote` feature ([`FlmFeature`]), constructed from a [`QuoteConfig`]:
/// it registers the quote environments, their quotation-section commands and
/// the lowering rules of a quotation. See the [module documentation](self).
#[derive(Debug, Clone, Default)]
pub struct Quote {
    config: QuoteConfig,
}

impl Quote {
    /// Returns the feature constructed from `config`, which stays fixed for
    /// the life of the registry the feature is registered into.
    ///
    /// The configuration is checked when the feature registers: an invalid
    /// one ([`QuoteConfig::validate`]) makes the registry build fail with a
    /// [`BuildError::Feature`].
    pub fn new(config: QuoteConfig) -> Quote {
        Quote { config }
    }

    /// Returns the configuration this feature was constructed from.
    pub fn config(&self) -> &QuoteConfig {
        &self.config
    }
}

impl FlmFeature for Quote {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Quotations").docs(
            "Quotations: the quote environments (`quote`, `blockquote`, `address` and any a host \
             configures), their quotation-section commands `\\text`, `\\lines`, `\\attributed` \
             and `\\block`, and the commands `\\\\` and `\\indent` of a lines section.",
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
        for (name, environment) in &self.config.environments {
            b.element(elements::environment_def(&owner, name, environment))?;
        }
        let sections = elements::sections_packages(&owner, &self.config.environments);
        let any_section = !sections.is_empty();
        for package in sections {
            b.scoped_package(package)?;
        }
        b.scoped_package(elements::lines_package(&owner))?;
        if !self.config.environments.is_empty() {
            b.lowering_rule(
                LoweringRule::new(
                    QUOTE_RULE,
                    QUOTE,
                    lower::quote_decl(),
                    Arc::new(QuoteRule),
                    DefaultMark::Everywhere,
                )
                .docs(
                    "a quotation lowers to a `flm.pres.block` of role `flm.quote` whose kind token \
                     is the environment's name, holding one `flm.pres.block` per quotation section, \
                     of the role of the section's kind",
                ),
            )?;
            html::register(b)?;
            latex_rendered::register(b)?;
            latex::register(b)?;
        }
        if any_section {
            b.lowering_rule(
                LoweringRule::new(
                    SECTION_RULE,
                    SECTION,
                    LoweringDecl::new(),
                    Arc::new(SectionRule),
                    DefaultMark::Everywhere,
                )
                .docs("a quotation-section command outside section position lowers to its argument's content, and is reported unless it is part of a quotation body's bare content"),
            )?;
        }
        b.digest_input(&self.config.digest_bytes())?;
        Ok(())
    }
}

/// Returns the [`Quote`] feature under the default configuration (the
/// environments `quote`, `blockquote` and `address`), ready to be listed in a
/// package entry.
///
/// To list the feature under a [`QuoteConfig`] of your own, use
/// `Arc::new(Quote::new(config))`, or pass the configuration to the standard
/// package with [`StdcoreConfig::quote`](crate::stdcore::StdcoreConfig::quote).
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Quote::default())
}
