//! The `theorems` feature: the theorem-like environments of mathematical
//! writing. These are numbered, titled block environments such as `theorem`,
//! `lemma` and `definition`, and the `proof` environment, whose title can refer
//! back to the statement it proves. Every environment is declared from the
//! feature's configuration ([`TheoremsConfig`]), so a host can add environments
//! of its own (an `exercise`) or remove shipped ones.
//!
//! # What an author writes
//!
//! ```text
//! \begin{lemma}[Key lemma]\label{lem:key}
//!   Every bounded monotone sequence converges.
//! \end{lemma}
//!
//! \begin{proof}[*lem:key]
//!   By completeness.
//! \end{proof}
//! ```
//!
//! prints, on the text backend,
//!
//! ```text
//! Lemma 1 (Key lemma). Every bounded monotone sequence converges.
//!
//! Proof (Lemma 1). By completeness. □
//! ```
//!
//! Each environment is one **theorem kind** (`lemma` above). It takes an
//! optional title in brackets, then two fields in any order, `\label{…}`
//! (repeatable) and `\noproofref` (which says that no proof of the statement
//! is coming), and a body laid out as blocks ([`elements`]). A proof's title
//! written `[*lem:key]` is a **back-reference**: it refers to the statement
//! labeled `lem:key` and shows that statement's word and number in the proof's
//! heading; `[**lem:key]` refers to the same statement and shows nothing in
//! the heading. The
//! language guide describes these environments for authors under
//! [Theorems](crate::guide::language_syntax#theorems).
//!
//! # Theorem kinds and theorem classes
//!
//! Every theorem kind belongs to one **theorem class** ([`TheoremClass`]),
//! which holds what a family of kinds shares: whether its kinds are numbered
//! and in which counter, the heading template, the closing mark, and whether a
//! title beginning with `*` is read as a back-reference. A kind
//! ([`TheoremKind`]) adds its words (`Lemma`, `Lemmas`, and variants such as
//! the abbreviation `Lem.`) and whether each of its statements expects a
//! proof. The default configuration ([`TheoremsConfig::new`]) declares:
//!
//! - `theorem`, `proposition`, `lemma`, `corollary` and `conjecture` in the
//!   class `theoremlike`, and `definition` and `remark` in the class
//!   `definitionlike`, all numbered in one shared counter;
//! - `proof` in the class `prooflike`: unnumbered, closed by the mark `□`,
//!   its title read as a back-reference.
//!
//! The feature's code refers to no kind and no class by name: a host's
//! `exercise` is an environment in its own right, declared in the
//! configuration. The configuration is fixed when the registry is built
//! (see [settings, options and
//! configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
//!
//! # Numbers, labels and back-references
//!
//! Every theorem is one [reference point](crate::model::aspects::RefPoint) of
//! the reference kind `theorem` (see [reference points, labels and
//! anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)).
//! A theorem of a numbered kind is numbered by the
//! [`numbering`](crate::stdcore::features::numbering) feature in its class's
//! [counter](crate::stdcore::features::numbering::CounterDecl) and shown in a
//! [counter style](crate::stdcore::features::numbering::CounterStyleDecl) of
//! its kind, whose words are the kind's ([`numbers`]): `\ref{lem:key}` reads
//! `Lemma~1`; in a document that numbers theorems within sections it reads
//! `Lemma~2.3` on every output except the recomposed LaTeX, where LaTeX
//! numbers the theorems (see [the
//! limitation](latex#limitation-theorems-numbered-within-sections)). Its anchor is the
//! environment's name and its number, `lemma-1`. A theorem of an unnumbered
//! kind accepts no label and has no anchor.
//!
//! A back-reference is a [reference site](crate::model::aspects::RefSite),
//! which the [references](crate::stdcore::features::refs) feature resolves as
//! it resolves `\ref`. [`title_reading`] is the function that decides whether
//! a title is a back-reference. On a class that reads back-references, a title
//! that begins with a star is kept as typed: its reader places it under an
//! opaque wrapper, so that its label meets the label of a `\label` as written,
//! and `[*lem:a--b]` refers to `\label{lem:a--b}`. Every other title is content
//! (see [a starred title is kept as
//! typed](elements#a-starred-title-is-kept-as-typed)).
//!
//! # The check of proofs
//!
//! A statement of a kind that [expects a proof](TheoremKind::expects_proof)
//! (by default `theorem`, `proposition`, `lemma` and `corollary`) should have
//! a proof whose back-reference refers to it, unless it carries `\noproofref`.
//! The feature's check stage ([`check`]) reports a statement that has neither
//! as the Warning `flm.theorems.missing-proof`, and a title that begins with a
//! star but is not a well-formed back-reference as the Error
//! `flm.theorems.bad-relation-ref`. The stage is a member of the
//! [`flm.validated`](crate::stdcore::VALIDATED) collection, which every
//! standard workflow demands, so it runs, with the same reports, under the
//! `check` workflow and under every render. It reads the document tree: a
//! theorem inside a user definition that the plan keeps unexpanded is not
//! checked (see [`check`]).
//!
//! # What the feature provides to the pipeline
//!
//! - **Elements.** [`THEOREM`] (`flm.theorems.theorem`), with one element
//!   value per configured kind, available in text mode. Three synthesized
//!   elements, [`NAME_ELEMENT`], [`TITLE_POINTER`] and [`RELATION`], hold the
//!   parts of a theorem's heading ([`heading`]); no document can write them.
//! - **The general lowering rules**, each the default on every backend:
//!   [`THEOREM_RULE`] and the rules of the three heading elements,
//!   [`NAME_RULE`], [`TITLE_POINTER_RULE`] and [`RELATION_RULE`]. A theorem
//!   becomes a `flm.pres.block` of role `flm.theorems.theorem` whose kind
//!   tokens are the class's name and the kind's (`theoremlike lemma`), with
//!   its heading as a run-in decoration at the head of its first paragraph
//!   and, for a class with a closing mark, the mark as a final decoration at
//!   the end of its last paragraph ([`rules`]; see
//!   [decorations](crate::stdcore::backends::pres::Decoration) and [roles and
//!   kinds](crate::stdcore::backends::pres::payload)). The
//!   heading is the class's heading template, with the kind's word and number
//!   in a span of role `flm.theorems.name` and the title in a span of role
//!   `flm.theorems.title`; the title is lowered where the heading shows it,
//!   never copied ([`heading`]). The text and html backends write a stop
//!   after the heading (their render option `run_in_stop`, a period by
//!   default; see
//!   [`TextRenderOptions::run_in_stop`](crate::stdcore::backends::text::stages::TextRenderOptions::run_in_stop)
//!   and
//!   [`HtmlRenderOptions::run_in_stop`](crate::stdcore::backends::html::feature::HtmlRenderOptions::run_in_stop))
//!   unless the heading already ends in punctuation; the text backend writes a
//!   non-breaking space before the closing mark.
//!   At every theorem the general rule requires the feature's style
//!   identifier `flm.theorems.style` ([`html::STYLE`]).
//! - **On the `html` backend**, the stylesheet `flm.theorems.style`
//!   ([`html`]), which the requirement brings into a page holding a theorem
//!   and which sets the kind's word and number apart from the title.
//! - **On the `latex-rendered` backend**, the definition piece
//!   `flm.latex.def.flmx.theorems` ([`latex_rendered`]), which the same
//!   requirement brings into the style file and which registers the look of
//!   each class of theorem.
//! - **On the `latex-recomposed` backend**, rules of their own ([`latex`]): a
//!   theorem is written back as the environment the author wrote, and the
//!   style file declares each kind the document uses. A proof's
//!   back-reference is written in one of two forms, which the backend's render
//!   option `resolved_proof_refs` chooses: as the author wrote it, for the
//!   `phfthm` LaTeX package to read (the default), or resolved into the
//!   proof's title, over the `amsthm` package alone. The rendering guide
//!   describes this output under [Theorems in the recomposed
//!   LaTeX](crate::guide::rendering#theorems-in-the-recomposed-latex).
//! - **Counters and counter styles** ([`numbers`]), contributed to the
//!   numbering feature's service slots.
//! - **A check stage**, [`CHECK_PROOFS`] (`flm.theorems.check-proofs`),
//!   providing the capability of the same name, a member of the
//!   [`flm.validated`](crate::stdcore::VALIDATED) collection ([`check`]).
//! - **A configuration**, [`TheoremsConfig`], whose fields are the feature's
//!   settings keys. A build hook ([`LateInit`](crate::core::registry::LateInit))
//!   compiles the heading templates, the closing marks and the kinds' words
//!   once the language is built; a text that does not compile fails the
//!   registry build.
//!
//! # Interactions with other features
//!
//! - **The numbering feature.** Without it, no theorem is numbered: a heading
//!   shows the kind's word alone (`Theorem`), and a theorem's anchor comes from
//!   its first label.
//! - **The references feature.** It resolves labels and back-references and
//!   computes anchors. Without it, no label resolves, a theorem has no anchor,
//!   a back-reference shows its label's text, and the check reports no missing
//!   proof.
//! - The feature reads the tables of these two features optionally, and
//!   declares them through peer declarations
//!   ([`PeerName`](crate::core::registry::PeerName)); it declares the same
//!   way the argument role that the references feature gives the argument of
//!   a theorem's `\label` field
//!   ([`DEFINED_LABEL_ARG_ROLE`](crate::stdcore::features::refs::DEFINED_LABEL_ARG_ROLE)).
//!   It therefore builds and renders in a registry that holds neither. The rules of the `latex-recomposed`
//!   backend are registered under peer declarations of their backend and of
//!   the LaTeX family's elements; in a registry without them, those rules are
//!   set aside (excluded from rule selection; see
//!   [`PeerName`](crate::core::registry::PeerName)).
//! - **User definitions.** A theorem written in a user definition's template is
//!   checked and numbered where the template is expanded in the tree. The
//!   title's reader sees a star written in the template (`[*#1]`), and keeps
//!   that title as typed. It does not see a star that arrives through a
//!   placeholder (a template `[#1]` used as `\pf{*lem:a--b}`): that title is
//!   laid out as content, and a label holding `--`, quotes, an apostrophe or
//!   `...` does not resolve through it. A
//!   theorem in the template of a kept definition (a user definition whose use
//!   sites the plan leaves in the tree; see [kept
//!   definitions](crate::stdcore::features::userdefs::kept)) whose title holds
//!   a placeholder where a back-reference's star could be is expanded at each
//!   use site instead ([`TitleReading::Unknown`]).
//!
//! # Map of the items
//!
//! - The feature: [`Theorems`], [`feature`], [`FEATURE`].
//! - The configuration: [`TheoremsConfig`], [`TheoremClass`], [`TheoremKind`]
//!   and their constants ([`DEFAULT_HEADING`], [`DEFAULT_CLOSING_MARK`],
//!   [`DEFAULT_LABEL_TYPES`], [`LOWERCASE_VARIANT`], [`CAPITAL_VARIANT`],
//!   [`ABBREVIATED_VARIANT`]).
//! - The element and its reading: [`elements`] ([`THEOREM`],
//!   [`TheoremElement`], [`title_reading`], [`TitleReading`],
//!   [`has_noproofref`], [`theorem_args`]).
//! - The heading: [`heading`]. The general rule of a theorem: [`rules`].
//! - The counters and counter styles: [`numbers`].
//! - The check of proofs: [`check`] ([`CHECK_PROOFS`]).
//! - The conditions: [`MissingProof`], [`BadRelationRef`], [`PointerRefused`],
//!   [`StageFailed`].
//! - The stylesheet: [`html`]. The `latex-recomposed` backend's own rules:
//!   [`latex`].

pub mod check;
mod config;
mod diagnostics;
pub mod elements;
pub mod heading;
pub mod html;
pub mod latex;
pub mod latex_rendered;
pub mod numbers;
pub mod rules;

use std::sync::Arc;

use crate::core::settings::SettingsDecl;
use crate::core::registry::{BuildError, CapabilityDef, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::lang::mint::{BodyDef, ElementDef, SyntaxDef};
use crate::stdcore::features::numbering::{COUNTERS, Counters};
use crate::stdcore::VALIDATED;
use crate::stdcore::features::refs;

pub use config::{
    ABBREVIATED_VARIANT, CAPITAL_VARIANT, DEFAULT_CLOSING_MARK, DEFAULT_HEADING,
    DEFAULT_LABEL_TYPES, LOWERCASE_VARIANT, TheoremClass, TheoremKind, TheoremsConfig,
};
pub use check::CHECK_PROOFS;
pub use diagnostics::{BadRelationRef, MissingProof, PointerRefused, StageFailed};
pub use elements::{
    NOPROOFREF_FIELD, POINT_KIND, THEOREM, TITLE_ARG, TheoremElement, TitleReading, has_noproofref,
    theorem_args, title_reading,
};
pub use heading::{
    NAME_ELEMENT, NAME_PLACEHOLDER, NAME_ROLE, NAME_RULE, NameRule, RELATION, RELATION_RULE,
    RelationRule, TITLE_PLACEHOLDER, TITLE_POINTER, TITLE_POINTER_RULE, TITLE_ROLE,
    TitlePointerRule,
};
pub use rules::{THEOREM_RULE, TheoremRule};

// Why theorem kinds are values of one element type grouped into classes: see
// decision rationale at [§flm-dd-dr:std-theorem-kinds].

/// The feature's registered name, `theorems`.
pub const FEATURE: &str = "theorems";

/// The declaration of the feature's settings: the fields of [`TheoremsConfig`] are its keys, and
/// the texts a kind's words, a class's heading template and closing mark, and the number format
/// hold are FLM markup. See [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<TheoremsConfig>(
            "The theorem-like environments: the theorem classes (whether numbered, whether \
            in the shared counter, the heading template, the closing mark, whether a proof's \
            title is read as a back-reference), the theorem kinds (the class, the words, \
            whether a proof is expected), the number format and the label types a numbered \
            kind accepts. Fixed when the registry is built: it cannot vary per document or \
            per render.",
        )
        .merge_entries("classes", || serde_json::to_value(&TheoremsConfig::default().classes).map_err(|error| error.to_string()))
        .merge_entries("kinds", || serde_json::to_value(&TheoremsConfig::default().kinds).map_err(|error| error.to_string()))
        .markup("kinds.*.words")
        .markup("classes.*.heading")
        .markup("classes.*.closing_mark")
        .markup("number_format")
}

/// The `theorems` feature ([`FlmFeature`]): it registers one element value of
/// [`THEOREM`] per kind of its [`TheoremsConfig`], each written as the
/// environment of the kind's name, together with the heading elements, the
/// lowering rules, the check stage, and the counters and counter styles of the
/// numbered kinds (contributed to the numbering feature). See the [module
/// documentation](self).
///
/// Construct it with [`Theorems::new`], or use [`feature`] for the default
/// configuration. Registering it fails with [`BuildError::Feature`] when the
/// configuration is refused by [`TheoremsConfig::validate`], and the registry
/// build fails when a heading template, a closing mark or a word does not
/// compile.
#[derive(Debug, Default, Clone)]
pub struct Theorems {
    config: TheoremsConfig,
}

impl Theorems {
    /// Creates the feature with the configuration `config`, which is fixed for
    /// the life of the registry the feature is registered into.
    pub fn new(config: TheoremsConfig) -> Theorems {
        Theorems { config }
    }

    /// The configuration this feature was constructed with.
    pub fn config(&self) -> &TheoremsConfig {
        &self.config
    }
}

impl FlmFeature for Theorems {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Theorems").docs(
            "Theorem-like environments (`theorem`, `lemma`, `proof`, …), one per configured \
             kind, all of the element `flm.theorems.theorem`: each with an optional title, \
             numbered and labeled where its kind's class is numbered, a heading at the start of \
             its first paragraph, an optional closing mark, and, on a proof, a back-reference \
             to the statement it proves.",
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
        // The `\label` field of a theorem environment is the references
        // feature's, with that feature's role on its argument; declared as a
        // peer, so that a registry without that feature still builds.
        b.peer_argument_role(refs::DEFINED_LABEL_ARG_ROLE)?;
        for name in self.config.kinds.keys() {
            // Every kind names a declared class: the configuration was checked.
            let Some(value) = TheoremElement::of(&self.config, name) else { continue };
            let args = theorem_args(value.class.relation_ref);
            b.element(
                ElementDef::new(
                    owner.clone(),
                    [name.as_str()],
                    SyntaxDef::Environment { args, body: BodyDef::new().blocks() },
                    elements::theorem_element(value),
                )
                .docs(format!(
                    "`\\begin{{{name}}}[title]\\label{{…}}\\noproofref…\\end{{{name}}}`: the \
                     theorem-like environment `{name}`, with an optional title, the fields \
                     `\\label` and `\\noproofref`, and a body of blocks."
                )),
            )?;
        }
        // The nodes an instantiated heading holds: synthesized, so no
        // document can write them.
        let synthesized = heading::synthesized_elements();
        for element in &synthesized {
            b.element(
                ElementDef::new(owner.clone(), Vec::<&str>::new(), SyntaxDef::Synthesized, Arc::clone(element))
                    .docs(element.docs.clone()),
            )?;
        }
        numbers::declare(b, &self.config)?;
        // The three tables belong to the numbering and the references
        // features, which may be absent: the rules read them optionally, and
        // write a theorem with no number, no anchor, and a back-reference as
        // its label's text.
        b.peer_table::<Counters>(
            COUNTERS.name(),
            "the numbering feature's counter table, which the theorem's rule reads for the \
             number its heading shows, and the back-reference's rule for the number of the \
             statement a proof refers to",
        )?;
        b.peer_table::<refs::Anchors>(
            refs::ANCHORS.name(),
            "the references feature's anchors table, which the theorem's rule reads for the \
             anchor of the theorem's reference point",
        )?;
        b.peer_table::<refs::RefResolutions>(
            refs::REF_RESOLUTIONS.name(),
            "the references feature's resolutions table, which the back-reference's rule \
             reads for the statement a proof refers to",
        )?;
        let headings = Arc::new(heading::Headings::new(self.config.clone(), &synthesized));
        b.late_init(Arc::new(heading::CompileHeadings { headings: Arc::clone(&headings) }))?;
        b.lowering_rule(rules::theorem_rule(Arc::clone(&headings)))?;
        for rule in heading::rules() {
            b.lowering_rule(rule)?;
        }
        b.digest_input(&self.config.digest_bytes())?;
        b.capability(CapabilityDef::new(CHECK_PROOFS).docs(
            "The theorems have been checked: every statement expecting a proof that no proof \
             refers back to and that carries no `\\noproofref` was reported as a Warning, and \
             every title that begins with a star but is not a well-formed back-reference as an \
             Error. A member of `flm.validated`.",
        ))?;
        b.stage(Arc::new(check::CheckProofs::new(&self.config)))?;
        b.collection_member(VALIDATED, CHECK_PROOFS)?;
        html::register(b)?;
        latex_rendered::register(b)?;
        latex::register(b, headings)?;
        Ok(())
    }
}

/// The [`Theorems`] feature under the default configuration (the eight kinds
/// of [`TheoremsConfig::new`]), ready to be listed in a package entry
/// ([`PackageEntry`](crate::core::registry::PackageEntry)).
///
/// Use `Arc::new(Theorems::new(config))` to list the feature under a
/// [`TheoremsConfig`] of your own.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Theorems::default())
}
