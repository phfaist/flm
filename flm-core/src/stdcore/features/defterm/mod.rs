//! The `defterm` feature: defined terms. An author defines a term where it is
//! explained, with the `defterm` environment, and links to that term
//! definition from anywhere in the document with `\term`.
//!
//! # What an author writes
//!
//! ```text
//! \begin{defterm}{Markov chain}\label{topic:markov}\aliasterm{Markov chains}
//!   A random process whose next state depends on its present state alone.
//! \end{defterm}
//!
//! Every \term{Markov chain} forgets its past, as \term{Markov chains} do;
//! \term[Markov chain]{such a process} does too. See \ref{topic:markov}.
//! ```
//!
//! A **term definition** is the environment `defterm`. Its one argument is
//! the **term** being defined, inline content (`\(\alpha\)-decay`,
//! `\emph{Markov} chain`), and its body holds the explanation's paragraphs.
//! After the term, the environment accepts two repeatable fields, in any
//! order:
//!
//! - `\label{…}`, a label by which `\ref` and `\hyperref` refer to the term
//!   definition, of the label type `term` or `topic` ([`LABEL_TYPES`]); a
//!   reference to a term definition shows its term;
//! - `\aliasterm{…}`, an **alias**: another way of writing the term (a
//!   plural, an inflected form), under which a term link finds the same term
//!   definition.
//!
//! A **term link** is `\term[ref]{shown}`: a link to the definition of the
//! term `ref`, or of the term `shown` where the bracket is left out or blank,
//! showing the text `shown`. Terms are matched by their **identity**: the source text
//! they are written as, with whitespace collapsed
//! ([`spelling_identity`](crate::stdcore::backends::pres::spelling_identity)).
//! The match is exact, case and markup included, so `\term{Entropy}` finds no
//! definition of `entropy`. A term's other forms are declared with
//! `\aliasterm`; nothing else matches a term to a term definition. The
//! language guide describes these commands for authors under [Defined
//! terms](crate::guide::language_syntax#defined-terms).
//!
//! # A term link is a reference
//!
//! A term definition declares one [reference
//! point](crate::model::aspects::RefPoint) and a term link declares one
//! [reference site](crate::model::aspects::RefSite) ([`elements`] gives the
//! labels; see [reference points, labels and
//! anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)).
//! The [references](crate::stdcore::features::refs) feature resolves the site
//! against the point through its stages and tables, as it resolves `\ref`;
//! no code of this feature looks a term definition up. A term definition's
//! reference point carries a label computed from its term, one computed from
//! each alias, then the labels the author wrote ([`elements`]). Therefore:
//!
//! - a term link that nothing resolves is reported by the references feature
//!   as `flm.refs.unresolved`, an Error by default;
//! - two term definitions of one term carry the same computed label, which
//!   the references feature reports as `flm.refs.duplicate-label`, and every
//!   term link finds the first one;
//! - a term definition's anchor is the one that feature computes from the
//!   point's first label: the label computed from the term, where the term is
//!   not empty.
//!
//! # How they are written
//!
//! A term definition is written as a block holding its body, whose first
//! paragraph opens with the **lead-in**: the configured lead-in template
//! ([`DeftermConfig`]), `#{term}: ` by default, so that the term definition
//! reads `Markov chain: A random process …`. The lead-in is set as a run-in
//! heading is, and shows the term where the term definition writes it, never
//! a copy ([`lead_in`]). A term link is a link of the kind `term` around the
//! text written for it, except inside the definition of its own term, where
//! it is the term being defined: a span of role `flm.defterm.term`, linked to
//! nothing ([`rules`]).
//!
//! On the text and `html` backends the lead-in is followed by the backend's
//! run-in stop (the render option `run_in_stop`, a period by default) only
//! where it ends without punctuation.
//!
//! - On the **text backend**, the lead-in is printed before the first
//!   paragraph's words, and a term link is its text.
//! - On the **`html` backend**, a term definition is a block carrying its
//!   anchor and a term link is a link of kind `term`; the term being defined
//!   is a span that the feature's stylesheet sets in italics ([`html`]).
//! - On the **`latex-recomposed` backend**, a term definition is written back
//!   as the environment `defterm`, its labels made to refer to it with the
//!   term as their display text, and the style file defines the environment
//!   from the configured lead-in. A term link is the LaTeX hook
//!   `\flmTerm{<label>}{<shown>}`, and, inside the definition of its own
//!   term, `\flmTermInDefterm{<shown>}`; an author redefines either
//!   ([`latex`]).
//!
//! The rendering guide describes the outputs under [Defined
//! terms](crate::guide::rendering#defined-terms), [Defined terms in
//! HTML](crate::guide::rendering#defined-terms-in-html) and [Defined terms in
//! the recomposed LaTeX](crate::guide::rendering#defined-terms-in-the-recomposed-latex).
//!
//! # What the feature provides to the pipeline
//!
//! - **Elements.** [`DEFTERM`] (`flm.defterm`), the environment `defterm`,
//!   and [`TERM`] (`flm.term`), the command `\term`, both available in text
//!   mode; and the synthesized element [`TERM_POINTER`], which shows the term
//!   inside a lead-in ([`lead_in`]); no document can write it.
//! - **Two argument roles**, each stating what an argument holds
//!   ([`ArgDef::role`](crate::lang::mint::ArgDef::role)).
//!   [`DEFINED_TERM_ARG_ROLE`] (`flm.defterm.defined-term`) is the role of
//!   the term a `defterm` environment defines and of its `\aliasterm`
//!   field's argument; [`TARGET_ARG_ROLE`] (`flm.defterm.target`) is the role
//!   of the term a `\term` points to.
//! - **The general lowering rules**, [`DEFINITION_RULE`], [`TERM_RULE`] and
//!   [`POINTER_RULE`], each the default on every backend ([`rules`],
//!   [`lead_in`]).
//!   The term definition's general rule requires the feature's style
//!   identifier `flm.defterm.style` ([`html::STYLE`]) at every definition it
//!   stages, and the term link's where it writes the term inside its own
//!   definition.
//! - **On the `html` backend**, the feature's stylesheet,
//!   `flm.defterm.style` ([`html`]).
//! - **On the `latex-rendered` backend**, the definition piece
//!   `flm.latex.def.flmx.defterm` ([`latex_rendered`]), which the same
//!   requirement brings into the style file.
//! - **On the `latex-recomposed` backend**, the rules of the three elements
//!   and the feature's definition piece `flm.latex.def.defterm` ([`latex`]).
//! - **A configuration**, [`DeftermConfig`], whose one field, the lead-in
//!   template, is the feature's settings key. It is fixed when the registry is
//!   built (see [settings, options and
//!   configuration](crate::guide::concepts_overview#settings-options-and-configuration)),
//!   and a build hook ([`LateInit`](crate::core::registry::LateInit))
//!   compiles the template once the language is built.
//!
//! # Interactions with other features
//!
//! The feature reads the references feature's anchors and resolutions tables
//! optionally, and declares them through peer declarations
//! ([`PeerName`](crate::core::registry::PeerName)); it declares the same way
//! the argument role that the references feature gives the argument of the
//! `\label` field ([`DEFINED_LABEL_ARG_ROLE`](refs::DEFINED_LABEL_ARG_ROLE)).
//! It therefore builds and renders in a registry without the references
//! feature: a term definition declares its reference point, which nothing
//! reads, and is written with no anchor, and a term link is its text. The rules of the `latex-recomposed`
//! backend are registered under peer declarations of their backend and of
//! the LaTeX family's elements; in a registry without them, those rules are
//! set aside (excluded from rule selection; see
//! [`PeerName`](crate::core::registry::PeerName)).
//!
//! # Map of the items
//!
//! - The feature: [`Defterm`], [`feature`], [`FEATURE`].
//! - The configuration: [`DeftermConfig`], [`DEFAULT_LEAD_IN`].
//! - The elements, their arguments and the identity of a term: [`elements`]
//!   ([`DEFTERM`], [`TERM`], [`term_target`], [`in_own_definition`]).
//! - The lead-in and the term pointer: [`lead_in`].
//! - The general rules: [`rules`].
//! - The conditions: [`EmptyTerm`], [`PointerRefused`].
//! - The stylesheet: [`html`]. The `latex-recomposed` backend's own rules:
//!   [`latex`].

// Why a term link is a reference resolved by the references feature: see
// decision rationale at [§flm-dd-dr:std-defterm-term-reference].

mod config;
mod diagnostics;
pub mod elements;
pub mod html;
pub mod latex;
pub mod latex_rendered;
pub mod lead_in;
pub mod rules;

use std::sync::Arc;

use crate::core::settings::SettingsDecl;
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::lang::mint::{BodyDef, ElementDef, SyntaxDef};
use crate::stdcore::features::refs;

pub use config::{DEFAULT_LEAD_IN, DeftermConfig};
pub use diagnostics::{EmptyTerm, PointerRefused};
pub use elements::{
    ALIAS_ARG, ALIAS_FIELD, DEFINED_TERM_ARG_ROLE, DEFINITION_KIND, DEFTERM, DeftermElement,
    LABEL_TYPES, REF_ARG, SHOWN_ARG, TARGET_ARG_ROLE, TERM, TERM_ARG, TermElement, defterm_args, defterm_element, in_own_definition,
    term_args, term_element, term_target,
};
pub use lead_in::{LEAD_IN_ROLE, POINTER_RULE, PointerRule, TERM_PLACEHOLDER, TERM_POINTER};
pub use rules::{
    DEFINITION_RULE, DefinitionRule, TERM_LINK_KIND, TERM_ROLE, TERM_RULE, TermRule,
};

/// The feature's registered name, `defterm`.
pub const FEATURE: &str = "defterm";

/// The declaration of the feature's settings: the fields of [`DeftermConfig`] are its keys, and
/// the lead-in template is FLM markup. See [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<DeftermConfig>(
            "The lead-in of a term definition (the `defterm` environment): a template in which \
            `#{term}` is the term being defined, or none for no lead-in. Fixed when the \
            registry is built: it cannot vary per document or per render.",
        )
        .markup("lead_in")
}

/// The `defterm` feature ([`FlmFeature`]): under a [`DeftermConfig`], it
/// registers the environment `defterm` and the command `\term`, the term
/// pointer, their lowering rules on every backend, and the build hook that
/// compiles the lead-in template. See the [module documentation](self).
///
/// Construct it with [`Defterm::new`], or use [`feature`] for the default
/// configuration. The registry build fails when the lead-in template does
/// not compile.
#[derive(Debug, Default, Clone)]
pub struct Defterm {
    config: DeftermConfig,
}

impl Defterm {
    /// Creates the feature with the configuration `config`, which is fixed for
    /// the life of the registry the feature is registered into.
    pub fn new(config: DeftermConfig) -> Defterm {
        Defterm { config }
    }

    /// The configuration this feature was constructed with.
    pub fn config(&self) -> &DeftermConfig {
        &self.config
    }
}

impl FlmFeature for Defterm {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Defined terms").docs(
            "Defined terms: the `defterm` environment, which defines a term and carries one \
             reference point, and `\\term`, a link to a term's definition, which the references \
             feature resolves.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        b.argument_role(
            DEFINED_TERM_ARG_ROLE,
            "A term that a definition defines, by which term links point to the definition.",
        )?;
        b.argument_role(TARGET_ARG_ROLE, "The term that a term link points to.")?;
        // The `\label` field of the environment is the references feature's,
        // with that feature's role on its argument; declared as a peer, so
        // that a registry without that feature still builds.
        b.peer_argument_role(refs::DEFINED_LABEL_ARG_ROLE)?;
        b.element(
            ElementDef::new(
                owner.clone(),
                ["defterm"],
                SyntaxDef::Environment { args: defterm_args(), body: BodyDef::new().blocks() },
                defterm_element(),
            )
            .docs(
                "`\\begin{defterm}{term}\\label{…}\\aliasterm{…}…\\end{defterm}`: the definition \
                 of a term, with its labels and aliases.",
            ),
        )?;
        b.element(
            ElementDef::new(owner.clone(), ["term"], SyntaxDef::Macro { args: term_args() }, term_element())
                .docs(
                    "`\\term[ref]{shown}`: a link to the definition of the term `ref`, or of the \
                     term `shown` where the bracket is left out or blank, showing `shown`.",
                ),
        )?;
        // The term pointer a lead-in holds: synthesized, so no document can
        // write it.
        let pointer = lead_in::pointer_element();
        b.element(
            ElementDef::new(
                owner.clone(),
                Vec::<&str>::new(),
                SyntaxDef::Synthesized,
                Arc::clone(&pointer),
            )
            .docs(
                "The term of a term definition, shown where the lead-in template places \
                 `#{term}`. It is never written in a document: the term definition's rule puts it \
                 there.",
            ),
        )?;
        // The two tables belong to the references feature, which may be
        // absent: the rules read them optionally, and write a definition with
        // no anchor and a term link as its text.
        b.peer_table::<refs::Anchors>(
            refs::ANCHORS.name(),
            "the references feature's anchors table, which the term definition's rule reads for \
             the anchor of the term definition's reference point",
        )?;
        b.peer_table::<refs::RefResolutions>(
            refs::REF_RESOLUTIONS.name(),
            "the references feature's resolutions table, which the term link's rule reads for \
             the term definition a term link resolved to",
        )?;
        let lead_in = Arc::new(lead_in::LeadIn::new(self.config.lead_in.clone(), pointer));
        b.late_init(Arc::new(lead_in::CompileLeadIn { lead_in: Arc::clone(&lead_in) }))?;
        b.lowering_rule(rules::definition_rule(Arc::clone(&lead_in)))?;
        b.lowering_rule(rules::term_rule())?;
        b.lowering_rule(lead_in::pointer_rule())?;
        b.digest_input(&self.config.digest_bytes())?;
        html::register(b)?;
        latex_rendered::register(b)?;
        latex::register(b, lead_in)?;
        Ok(())
    }
}

/// The [`Defterm`] feature under the default configuration (the lead-in
/// [`DEFAULT_LEAD_IN`]), ready to be listed in a package entry
/// ([`PackageEntry`](crate::core::registry::PackageEntry)).
///
/// Use `Arc::new(Defterm::new(config))` to list the feature under a
/// [`DeftermConfig`] of your own.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Defterm::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_feature_names_itself() {
        assert_eq!(Defterm::default().info().name, FEATURE);
    }
}
