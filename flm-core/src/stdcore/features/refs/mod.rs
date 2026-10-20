//! The `refs` feature: labels and references. An author attaches a label to a
//! construct with `\label{sec:intro}` and refers to it with `\ref{sec:intro}`,
//! and the feature turns each reference into a link to the labeled place,
//! showing that place's number or title.
//!
//! The terms are those of the concepts overview, [Reference points, labels and
//! anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors):
//! a *reference point* ([`RefPoint`](crate::model::aspects::RefPoint)) is one
//! place in a document that can be referred to, numbered, or given an anchor
//! (a heading, an equation line, a theorem, a footnote); a *label* is the text
//! an author attaches to it with `\label{type:label}`; an *anchor* is the
//! identifier a link points at, computed once for every point ([`ANCHORS`]);
//! and a *reference site* ([`RefSite`](crate::model::aspects::RefSite)) is one
//! place that points at a reference point, such as a `\ref`.
//!
//! # What an author writes
//!
//! - **`\label{type:label}`**, written right after a construct that accepts
//!   labels (`\section{Introduction}\label{sec:intro}`). The *label type* is
//!   the part before the colon and says what sort of place the label marks;
//!   each construct accepts a fixed set of types (a heading accepts `sec` and
//!   `topic`). A label may be written only once in a document. A `\label` is a
//!   [field](crate::guide::concepts_overview#fields-data-a-construct-reads), a
//!   macro that the construct before it reads as data, so it is available only
//!   after a construct that declares it.
//! - **`\ref[options]{type:label}`**, a reference that shows what its target
//!   has to show: its number, else its content (a heading's title, for
//!   instance), else the label as written. Several targets may be written in
//!   one `\ref`, separated by commas (`\ref{eq:a,eq:b}`), and are shown as one
//!   list. The options choose the wording of the word before a number:
//!   `noprefix` leaves that word out, `S` asks for the capitalized wording
//!   where the [counter
//!   style](crate::stdcore::features::numbering::CounterStyleDecl) (how the
//!   numbering feature shows a counter's value; see [counters and counter
//!   styles](crate::guide::concepts_overview#counters-and-counter-styles))
//!   declares one, and any other key asks for the
//!   wording variant of that name (`abbreviated`, `lowercase`, `capital`).
//! - **`\hyperref[type:label]{text}`**, a reference that shows the author's
//!   own text instead of what the target has to show.
//!
//! The syntax guide shows these commands from an author's side, under
//! [Labels and references](crate::guide::language_syntax#labels-and-references).
//!
//! # What a reference displays
//!
//! A reference point states the two things a reference to it may show: a
//! number source ([`NumberSource`](crate::model::aspects::NumberSource)), the
//! counter and the counter style it is numbered by, where it can be numbered
//! at all; and content of the document, where it has any. For each target, a
//! `\ref` shows the first of these that exists:
//!
//! 1. the number this document gave the point;
//! 2. else the point's content;
//! 3. else the label the reference was written with, as written.
//!
//! A point that states a number source may still receive no number in a run:
//! no numbering feature is registered, nobody declares its counter, or a
//! counter it is numbered within has no value yet. A point that ends up with
//! neither a number nor content has nothing to show (an unnumbered float with
//! no caption, an equation in a registry without the numbering feature), and a
//! reference to it is the Error [`NothingToShow`] (`flm.refs.nothing-to-show`).
//!
//! The number is read from the counter table of the
//! [`numbering`](crate::stdcore::features::numbering) feature, which the
//! `\ref` lowering rule reads optionally, through a peer declaration
//! ([`PeerName`](crate::core::registry::PeerName)): in a registry without that
//! feature, a reference shows the point's content or its label. The counter
//! style of the point decides what a number looks like
//! (the word before it, the delimiters around it, how several numbers are
//! joined), and the options written on a `\ref` choose among the wording
//! variants that the style declares. The key `S` asks for the variant
//! [`CAPITAL_VARIANT`] where the style declares it and for the usual wording
//! otherwise. Any other key asks for the variant of its own name, and where
//! the counter style of a numbered target does not declare that variant, the
//! reference is the Error [`UndeclaredVariant`].
//!
//! **A `\ref` with several targets** is written as one list. Its first items
//! are the numbers, one item per counter style, each written by that style
//! with consecutive numbers collapsed into a range; then come the other
//! targets, one item each, in the order the reference wrote them. The items
//! are joined by the texts of the feature's configuration, [`RefsConfig`]:
//! `Eq. (1) and Thm. 1.1`, `Eq. (1), Thm. 1.1, and A note`. Another feature
//! that shows a reference from a command of its own calls
//! [`lower_reference`], which writes the items.
//!
//! A point's content (a heading's title, for instance) is copied out of the
//! frozen tree into a [synthesized
//! value](crate::guide::concepts_overview#synthesized-values), a small tree
//! held in a table, when the anchors stage writes the point's target. Content
//! that the point's element built itself because no node of the tree holds it
//! (the bullet beside a list's item,
//! [`ContentRef::Synthesized`](crate::model::aspects::ContentRef::Synthesized))
//! is already such a tree and is shared rather than copied. The `\ref` rule
//! splices the synthesized value into the nodes it stages (builds into the
//! lowered tree; see [`Lowerer`](crate::core::lowering::Lowerer)): the value is first
//! brought up to date by [value
//! catch-up](crate::guide::concepts_overview#value-catch-up), which runs the
//! plan's rewrites and the per-tree stages the lowering depends on over the
//! value alone, and is then lowered through the rules. A `\ref` written inside
//! a heading's title therefore resolves exactly like one written in the body.
//!
//! # What the feature registers
//!
//! The `refs` feature ([`Refs`], a [`FlmFeature`])
//! registers:
//!
//! - **Two elements.** [`REF`] (`flm.ref`), the `\ref` element, and
//!   [`HYPERREF`] (`flm.hyperref`), the `\hyperref` element. A node of either
//!   declares its reference sites through the
//!   [`Referencing`](crate::model::aspects::Referencing) aspect, one per
//!   target, and goes through the same stages; the two differ in what they
//!   display. The feature registers no element for `\label`: like every field,
//!   its node carries the language layer's field element
//!   ([`FIELD`](crate::lang::mint::FIELD)).
//! - **A scoped package.** A scoped package
//!   ([`ScopedPackageDef`](crate::lang::mint::ScopedPackageDef)) is a set of
//!   element declarations that can be written only inside the environment
//!   bodies and arguments whose declarations refer to it by name
//!   ([scoped packages](crate::guide::concepts_overview#scoped-packages)).
//!   [`LABEL_PACKAGE`] (`flm.refs.label`, built by [`label_package`]) holds
//!   `\label` as a command, for the arguments that refer to it, such as the
//!   optional argument of `\footnote`.
//! - **Two argument roles.** An argument role states what an argument holds
//!   ([`ArgDef::role`](crate::lang::mint::ArgDef::role)). [`TARGET_ARG_ROLE`]
//!   (`flm.refs.target`) is the role of the target argument of `\ref` and
//!   `\hyperref`, the label or labels a reference points to.
//!   [`DEFINED_LABEL_ARG_ROLE`] (`flm.refs.defined-label`) is the role of the
//!   argument of `\label`, the label it attaches to a construct, in the
//!   `\label` field and in the scoped package's `\label` command.
//! - **Four tables.** [`LABELS`] (`flm.labels`): which point keeps each label,
//!   the labels written more than once, and the labels refused for their type.
//!   [`ANCHORS`] (`flm.anchors`): each point's anchor, or none.
//!   [`REF_TARGETS`] (`flm.refs.targets`): for each label, the point, its link
//!   target and what a reference to it shows. [`REF_RESOLUTIONS`]
//!   (`flm.refs.resolutions`): how each reference site resolved.
//! - **An external table.** [`EXTERNAL_INDEX`] (`flm.refs.external-index`):
//!   labels of other documents, with their link targets and the content a
//!   reference to them shows. An external table is a table that no stage of
//!   the run writes: the caller installs its value when the run is
//!   constructed ([`Externals`](crate::core::run::Externals)). The resolve
//!   stage reads it when the caller has installed it.
//! - **Four stages.** [`SCAN`] records which point keeps each label;
//!   [`ANCHORS_STAGE`] gives every point its anchor and writes the targets;
//!   [`RESOLVE`] resolves every reference site; [`CHECK_LABELS`] reports the
//!   duplicate labels and the labels of a type their point does not accept.
//!   The check provides the capability of its own name, a member of the
//!   [`flm.validated`](crate::stdcore::VALIDATED) collection of capabilities,
//!   which every workflow of the standard package demands (asks the plan to
//!   provide; see [`PlanRequest`](crate::core::plan::PlanRequest)). The documentation of each
//!   constant says what its stage reads and writes.
//! - **Render options.** [`RefsRenderOptions`], set per render request:
//!   whether an unresolved reference is an Error or a Warning.
//! - **A configuration.** [`RefsConfig`], the texts that join the items of a
//!   reference to several targets. A feature's configuration is the plain
//!   Rust value the feature is constructed from; it is fixed when the registry
//!   is built ([settings, options and
//!   configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
//! - **Lowering rules, per backend.** On every backend, [`REF_RULE`] lowers a
//!   `\ref` to a [`flm.pres.link`](crate::stdcore::backends::pres::LINK) to
//!   each target, around the number, the content or the label it shows, and
//!   [`HYPERREF_RULE`] lowers a `\hyperref` to a link around the author's
//!   text; a target that nothing resolves becomes a
//!   [`flm.pres.span`](crate::stdcore::backends::pres::SPAN) styled as an
//!   unresolved reference instead of a link. The HTML backend writes the link
//!   as an `<a>` element ([References in
//!   HTML](crate::guide::rendering#references-in-html)). On the
//!   `latex-recomposed` backend, the rules of the submodule [`latex`] are the
//!   default instead: they leave the resolution to LaTeX and write `\label`,
//!   `\flmRef` and `\flmHyperref` around safe labels (see *Recomposing to
//!   LaTeX* below).
//! - **The stages and tables of the recomposed LaTeX**, in the submodule
//!   [`safe_labels`]: the safe labels the recomposed LaTeX writes in place of
//!   the document's labels.
//!
//! # Anchors
//!
//! An anchor is the identifier a link points at for a reference point. The
//! element of the point chooses how it is formed
//! ([`AnchorPolicy`](crate::model::aspects::AnchorPolicy)), and the anchors
//! stage computes it once:
//!
//! - `<type>-<label>` from the first label the point keeps: `sec-intro`
//!   ([`label_anchor`]);
//! - `<prefix>--<title>` for an unlabeled heading: `sec--Introduction`
//!   ([`title_anchor`]);
//! - `<prefix>--<position><four letters>` for an unlabeled point whose element
//!   asks for its position (an equation line, a list item, an xnote):
//!   `eq--8otbc` for the eighth equation line ([`auto_anchor`]);
//! - `<prefix>-<numbers>` from the number the document gave the point, for an
//!   element that asks for it (a theorem, a float): `theorem-2-3`
//!   ([`value_anchor`]); a point the document did not number takes its first
//!   label instead;
//! - an identifier the element gives itself, used as given.
//!
//! The element states the prefix, so that the anchor of a point begins the
//! same way whether it was labeled or not. The prefix, the parts of a label,
//! a title and a number text go through one encoder, [`encode_anchor`]: a text
//! made only of ASCII letters, digits, `.`,
//! `-` and single spaces is reproduced exactly, each space written `_`; any
//! other text is stripped of accents, every run of other characters is
//! replaced by `_` or `-`, and `~` and six hexadecimal digits of a hash of the
//! text as written are appended. No counter
//! takes part in a label's anchor, so one label always yields one anchor,
//! regardless of what else the document contains.
//!
//! An anchor made from a title is never refused: where another point already
//! holds the anchor it would take (another heading with the same title, or a
//! point whose anchor encodes alike), it takes the next free number
//! (`sec--Introduction-2`), and nothing is reported. Any other anchor that
//! another point already holds is a collision: the Error [`AnchorCollision`]
//! is reported, the first point keeps the anchor, and the second gets none.
//!
//! # Label types
//!
//! A reference point declares which label types may be written for it
//! ([`RefPoint::accepts`](crate::model::aspects::RefPoint::accepts)): a heading
//! accepts `sec` and `topic`, so `\section{X}\label{eq:x}` defines no label. A
//! point that declares
//! [`AcceptedRefTypes::Any`](crate::model::aspects::AcceptedRefTypes) accepts
//! every type; a feature declares it for a construct that fixes no set. A
//! label of a type the point does not accept defines nothing: no reference
//! resolves through it, and a point whose every label was refused gets no
//! anchor from a label. [`CHECK_LABELS`] reports it as
//! [`LabelTypeNotAccepted`] (`flm.refs.label-type-not-accepted`).
//!
//! A text that is not spelled `type:label` at all is no label: the construct
//! keeps it among the point's
//! [malformed](crate::model::aspects::RefPoint::malformed) texts, and the
//! label scan reports it at the field where it was written, as [`BadLabel`]
//! (`flm.refs.bad-label`). The scan reports a `\ref` or `\hyperref` target
//! that is not spelled `type:label` the same way.
//!
//! # Duplicate labels and unresolved references
//!
//! A label written for two points is the Error [`DuplicateLabelCondition`]
//! (`flm.refs.duplicate-label`), reported by [`CHECK_LABELS`]: the first point
//! in document order keeps the label. This depends on the document alone.
//!
//! A reference that nothing resolves is reported as [`Unresolved`]
//! (`flm.refs.unresolved`): an Error by default, a Warning under
//! [`RefsRenderOptions`] with `unresolved: allow`. Either way, the reference
//! shows its own text marked as unresolved: the label text for a `\ref`, the
//! author's text for a `\hyperref`. In a plan that renders nothing, such as
//! one of the [`check` workflow](crate::stdcore::CHECK_WORKFLOW), it is always
//! a Warning, because whether a label resolves is decided per render, and a
//! render may install an index of labels defined in other documents.
//!
//! # Declaring a labeled construct in another feature
//!
//! A feature gives one of its constructs labels in three steps:
//!
//! 1. The element's last argument is [`label_fields_arg`], the trailing
//!    tack-on argument (the trailing argument that absorbs field macros;
//!    [`ArgStdKind::TackOn`](crate::lang::mint::ArgStdKind::TackOn)) that holds
//!    the `\label` fields, so that `\thing{…}\label{x:y}` parses. The
//!    argument of the `\label` field has the argument role
//!    [`DEFINED_LABEL_ARG_ROLE`], which this feature declares; the other
//!    feature declares that role as a peer
//!    ([`FeatureBuilder::peer_argument_role`](crate::core::registry::FeatureBuilder::peer_argument_role)),
//!    so that a registry without the references feature still builds.
//! 2. The element carries the
//!    [`Referenceable`](crate::model::aspects::Referenceable) aspect, whose
//!    implementation reads those fields with [`label_uses`], splits them with
//!    [`point_labels`], and returns the point (or points) the node declares.
//! 3. Each point states the label types it accepts in its
//!    [`accepts`](crate::model::aspects::RefPoint::accepts) field: a list of
//!    types, or
//!    [`AcceptedRefTypes::Any`](crate::model::aspects::AcceptedRefTypes) for a
//!    construct that fixes none.
//!
//! A construct with a recomposition rule of its own writes its labels there
//! with [`stage_label`] and [`stage_label_group`] (submodule [`latex`]). No
//! other rule writes a label, because a field node that reaches a backend is
//! an error.
//!
//! Two kinds of `\label` are not tack-on fields. Inside a display-math
//! environment's body, the `math` feature declares `\label` as a command of
//! its own ([`math::lines`](crate::stdcore::features::math::lines)); inside an
//! argument that refers to the scoped package [`LABEL_PACKAGE`] (a construct
//! whose labels are written in an argument of their own, as in
//! `\footnote[\label{fn:x}]{…}`), this feature's package declares it. Both
//! nodes carry the same field element with the same name: [`label_use`] reads
//! one such node, and [`label_uses_in`] reads every one of a run of nodes.
//!
//! # Recomposing to LaTeX
//!
//! The `latex-recomposed` backend writes the document back as LaTeX and
//! leaves the resolution of references to LaTeX. LaTeX label keys must avoid
//! certain characters, so the recomposed LaTeX writes a *safe label*
//! ([`SafeLabels`]) in place of each FLM label: `ref1`, `ref2`, … in the order
//! the document first mentions the labels. The submodule [`safe_labels`] holds
//! the document-scope stage [`SAFE_LABELS_STAGE`], which writes the table
//! [`SAFE_LABELS`] (`flm.labels.safe`), and the per-tree stage
//! [`VALUE_SAFE_LABELS_STAGE`], which writes the table [`VALUE_SAFE_LABELS`]
//! (`flm.labels.safe.value`) for the references of a synthesized value. The
//! submodule [`latex`] holds the recomposition rules [`REF_LATEX_RULE`] and
//! [`HYPERREF_LATEX_RULE`], the helpers [`stage_label`] and
//! [`stage_label_group`] that a labeled construct's own rule writes its labels
//! with, and the render options [`RefsRecomposeOptions`]. This feature
//! registers those rules too; it declares the backend and the two LaTeX
//! constructs they stage as peer declarations
//! ([`PeerName`](crate::core::registry::PeerName)), so that a registry without
//! the LaTeX features still builds, with those rules set aside (excluded from
//! rule selection). The
//! rendering guide describes the result under [References and safe
//! labels](crate::guide::rendering#references-and-safe-labels).
//!
//! # Items for other features
//!
//! - Declaring and reading labels: [`label_fields_arg`], [`label_field`],
//!   [`label_uses`], [`label_uses_in`], [`label_use`], [`point_labels`],
//!   [`field_text`], [`label_package`].
//! - Reading references: [`ref_targets`], [`hyperref_target`],
//!   [`reference_targets`], [`split_targets`], [`ref_display_options`],
//!   [`ref_args`], [`hyperref_args`].
//! - Showing a reference from another command: [`lower_reference`].
//! - Anchors: [`encode_anchor`], [`label_anchor`], [`title_anchor`],
//!   [`auto_anchor`], [`value_anchor`]; copying content into a synthesized
//!   value: [`content_value`], [`value_of`].
//! - Writing labels and references in the recomposed LaTeX: [`stage_label`],
//!   [`stage_label_group`], [`stage_labels_with_display_text`],
//!   [`target_spelling`], [`label_spelling`], [`stage_spelling`],
//!   [`read_target_at_lowering`], [`reference_type_words`],
//!   [`stage_reference_type_setup`].

// Why the label is a declared field a construct reads rather than an element
// of this feature's: see design rationale at [§flm-dd-dr:std-label-field].
// Anchors: [§flm-dd-dr:std-anchor-encoding], [§flm-dd-dr:std-anchor-collision],
// [§flm-dd-dr:std-anchor-policies], [§flm-dd-dr:std-anchor-counters].
// Severities: [§flm-dd-dr:std-check-unresolved], [§flm-dd-dr:std-refs-unresolved].

mod content;
mod elements;
mod list;
mod rules;
mod stages;
mod tables;
pub mod latex;
pub mod safe_labels;

use std::sync::Arc;

use crate::core::settings::SettingsDecl;
use crate::model::options::Bind;
use crate::core::registry::{BuildError, CapabilityDef, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::lang::mint::{ElementDef, SyntaxDef};
use crate::stdcore::VALIDATED;
use crate::stdcore::features::numbering::{COUNTERS, Counters};

use list::{CompileList, ListTexts};

pub use content::{
    CopyError, auto_anchor, content_value, encode_anchor, label_anchor,
    title_anchor, value_anchor, value_of,
};
pub use elements::{
    ABBREVIATED_VARIANT, CAPITAL_KEY, CAPITAL_VARIANT, DEFINED_LABEL_ARG_ROLE, FIELDS_ARG, HYPERREF,
    HyperrefElement, LABEL_FIELD, LABEL_PACKAGE, LOWERCASE_VARIANT, LabelUse, NO_PREFIX_KEY, OPTIONS_ARG, REF,
    RefElement, RefTargetText, TARGET_ARG, TARGET_ARG_ROLE, TEXT_ARG, field_text, hyperref_args, hyperref_element, hyperref_target,
    label_field, label_fields_arg, label_package, label_use, label_uses, label_uses_in, point_labels,
    ref_args, ref_display_options, ref_element, ref_targets, reference_targets, split_targets,
};
pub use list::RefsConfig;
pub use rules::{HYPERREF_RULE, HyperrefRule, LINK_KIND, REF_RULE, RefRule, lower_reference};
pub use stages::{
    ANCHORS_STAGE, AnchorCollision, BadLabel, CHECK_LABELS, DisplayUnavailable,
    DuplicateLabelCondition, LabelTypeNotAccepted, RESOLVE, RefsRenderOptions, SCAN, StageFailed,
    NothingToShow, UndeclaredVariant, Unresolved, UnresolvedRefs,
};
pub use tables::{
    ANCHORS, Anchors, DuplicateLabel, EXTERNAL_INDEX,
    ExternalIndex, ExternalTarget, LABELS, Labels, PointDisplay, REF_RESOLUTIONS, REF_TARGETS,
    RefResolutions, RefTarget, RefTargets, RefusedLabel, Resolution, SiteId,
};
pub use latex::{
    HYPERREF_LATEX_RULE, HyperrefLatexRule, LABEL_COMMAND, LabelSpelling, REF_LATEX_RULE,
    RefLatexRule, RefsRecomposeOptions, SafeLabelMode, ZCREF_COMMAND, label_spelling,
    read_target_at_lowering, reference_type_words, stage_label, stage_label_group,
    stage_labels_with_display_text, stage_ref_options, stage_reference_type_setup, stage_spelling,
    target_spelling,
};
pub use safe_labels::{
    REF_DOMAIN, SAFE_LABELS, STAGE as SAFE_LABELS_STAGE, SafeLabels, SafeLabelsStage,
    VALUE_SAFE_LABELS, VALUE_STAGE as VALUE_SAFE_LABELS_STAGE, ValueSafeLabels,
    ValueSafeLabelsStage, label_key, value_name,
};

/// The feature's registered name, `refs`.
pub const FEATURE: &str = "refs";

/// The declaration of the feature's settings: the fields of [`RefsConfig`],
/// [`RefsRenderOptions`] and [`RefsRecomposeOptions`] are its settings keys.
/// See [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<RefsConfig>(
            "The texts that join the items of a reference to several targets. Fixed when the \
            registry is built: it cannot vary per document or per render.",
        )
        .options::<RefsRenderOptions>(
            Bind::Render,
            "Whether a reference that points at no label is reported as an Error (the \
            default) or, under `unresolved: allow`, as a Warning. Set per render request.",
        )
        .options::<RefsRecomposeOptions>(
            Bind::Render,
            "How the recomposed LaTeX writes labels and references: the label types written \
            as the document wrote them instead of as generated keys, and whether a \
            reference is protected against a command that changes the case of its \
            argument. Set per render request.",
        )
        .markup("join")
}

/// The `refs` feature ([`FlmFeature`]): it registers the `\ref` and
/// `\hyperref` elements, the scoped package of `\label`, six stages (four that
/// check and resolve labels and references, and two that serve the recomposed
/// LaTeX), the tables they write, the
/// render options and the lowering rules. See the module documentation.
///
/// Its configuration, [`RefsConfig`], holds the texts that join the items of
/// a reference to several targets; `Refs::default()` uses the default texts.
///
/// The feature reads the tables of the
/// [`refpoints`](crate::stdcore::features::refpoints) and
/// [`index`](crate::stdcore::features::index) features as required reads, so
/// a registry that holds this feature must hold those two. The
/// [`numbering`](crate::stdcore::features::numbering) feature is optional: with
/// it, a reference shows its target's number.
#[derive(Debug, Default, Clone)]
pub struct Refs {
    config: RefsConfig,
}

impl Refs {
    /// The feature with the configuration `config`.
    pub fn new(config: RefsConfig) -> Refs {
        Refs { config }
    }
}

impl FlmFeature for Refs {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "References").docs(
            "Labels and references: the `\\ref` and `\\hyperref` commands, the labels \
             written after constructs, the link identifier of every reference point, the \
             resolution of every reference, and the check of duplicate labels and label \
             types.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        b.argument_role(
            TARGET_ARG_ROLE,
            "The label, or several labels separated by commas, that a reference points to, \
             each written `type:label` (`sec:intro`).",
        )?;
        b.argument_role(
            DEFINED_LABEL_ARG_ROLE,
            "A label attached to a construct, written `type:label` (`sec:intro`), by which \
             references point to the construct.",
        )?;
        b.element(
            ElementDef::new(
                owner.clone(),
                ["ref"],
                SyntaxDef::Macro { args: ref_args() },
                ref_element(),
            )
            .docs(
                "`\\ref[options]{type:label}`: a reference to the labeled place, showing \
                 its number, else its content, else the label.",
            ),
        )?;
        b.element(
            ElementDef::new(
                owner.clone(),
                ["hyperref"],
                SyntaxDef::Macro { args: hyperref_args() },
                hyperref_element(),
            )
            .docs(
                "`\\hyperref[type:label]{text}`: a reference to the labeled place, showing \
                 the text written in the braces.",
            ),
        )?;
        b.scoped_package(label_package(owner.clone()))?;
        b.table(
            LABELS,
            "For every label, the reference point that keeps it; the labels written more \
             than once; and the labels whose type their point does not accept.",
        )?;
        b.table(
            ANCHORS,
            "Every reference point's anchor (the identifier a link points at), formed from \
             its first label, its title, its position or its number, or given by its \
             element, or none when the point has nothing to form it from or an earlier point \
             holds the same one.",
        )?;
        b.table(
            REF_TARGETS,
            "For every label whose point received an anchor: the reference point that keeps \
             it, its link target (`#` and the anchor) and what a reference to it shows; read \
             by the resolve stage.",
        )?;
        b.table(
            REF_RESOLUTIONS,
            "How every reference site resolved: to a reference point of this document, to \
             a label of another document from the external index, or not at all.",
        )?;
        b.external_table(
            EXTERNAL_INDEX,
            "Labels of other documents, each with its link target and the content a \
             reference to it shows; installed by the caller when the run is constructed, \
             and read only when installed.",
        )?;
        b.capability(CapabilityDef::new(CHECK_LABELS).docs(
            "The labels have been checked: every label written for two reference points and \
             every label whose type its point does not accept has been reported as an \
             Error, and every reference has been resolved, a reference that points at no \
             label being reported.",
        ))?;
        b.collection_member(VALIDATED, CHECK_LABELS)?;
        // The counter table belongs to the `numbering` feature, which a
        // registry need not hold: the `\ref` rule reads it optionally, under
        // this peer declaration, and shows a target's content or its label
        // where nothing numbers.
        b.peer_table::<Counters>(
            COUNTERS.name(),
            "The counter values, read to show the number that the target of a reference \
             was given.",
        )?;
        b.stage(Arc::new(stages::Scan))?;
        b.stage(Arc::new(stages::AnchorsStage))?;
        b.stage(Arc::new(stages::Resolve))?;
        b.stage(Arc::new(stages::CheckLabels))?;
        // The joining texts are parsed once the language is built, and the
        // `\ref` rule reads them from there.
        let list = Arc::new(ListTexts::default());
        for rule in rules::rules(&list) {
            b.lowering_rule(rule)?;
        }
        b.digest_input(&self.config.digest_bytes())?;
        b.late_init(Arc::new(CompileList { texts: list, decl: self.config.join.clone() }))?;
        b.table(
            SAFE_LABELS,
            "For the recomposed LaTeX: the safe label (`ref1`, `ref2`, …) written in place of \
             every label whose type is not written raw, numbered in the order the document \
             first mentions the labels, and the reverse correspondence.",
        )?;
        b.stage(Arc::new(SafeLabelsStage))?;
        b.table(
            VALUE_SAFE_LABELS,
            "For the recomposed LaTeX: for every reference node of a synthesized value, the \
             name written for each target that has no safe label in the document \
             (`ref-v1`, …); held in the value's catch-up store, and empty for a document \
             tree.",
        )?;
        b.stage(Arc::new(ValueSafeLabelsStage))?;
        latex::register(b)?;
        Ok(())
    }
}

/// The [`Refs`] feature with the default configuration, ready to be listed in
/// a package entry ([`PackageEntry`](crate::core::registry::PackageEntry)).
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Refs::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_feature_names_itself() {
        assert_eq!(Refs::default().info().name, FEATURE);
    }

    #[test]
    fn the_two_referencing_elements_are_the_feature_s_source_elements() {
        assert_eq!(ref_element().name, REF);
        assert_eq!(hyperref_element().name, HYPERREF);
    }
}
