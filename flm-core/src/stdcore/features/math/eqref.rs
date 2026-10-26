//! `\eqref{eq:euler}`: a reference to an equation line that shows its number and
//! nothing else, `(13)` where `\ref{eq:euler}` shows `Eq.~(13)`.
//!
//! The command belongs to the [`math`](super) feature because what it refers to,
//! one line of a display-math environment, belongs to that feature. It takes
//! **one** label, whose label type (the part before the colon; see [reference
//! points, labels and
//! anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors))
//! must be `eq` ([`TARGET_TYPE`]), the label type that an equation line accepts
//! ([`LABEL_TYPES`]). A label of any other type is an Error at the command
//! ([`NotAnEquation`], `flm.math.eqref-not-an-equation`), and the command then
//! shows the label's text, unlinked, as an unresolved reference does.
//!
//! # What the command shows
//!
//! An `\eqref` shows what `\ref[noprefix]` shows for the same label: the number
//! the document gave the line, between the parentheses of the equation counter
//! style ([`COUNTER_STYLE`](super::COUNTER_STYLE)) and without the prefix words;
//! for a line with a `\tag`, the tag's content; and when the label refers to no
//! equation line, or the references feature is not registered, the label as
//! written, unlinked. The default rule [`RULE`] does not decide this itself: it
//! calls the references feature's function for reference commands,
//! [`lower_reference`], so that `\eqref` and `\ref` always show a label the same
//! way. Unlike a `\ref` with one label, an `\eqref` adds none of the texts that
//! [`RefsConfig`](crate::stdcore::features::refs::RefsConfig) writes around a
//! single reference (`one_pre`, `one_post`): it shows the number and nothing
//! else.
//!
//! # Recomposing to LaTeX
//!
//! On the `latex-recomposed` backend, [`LATEX_RULE`] writes `\eqref{…}` around
//! the label's safe label, the key under which the recomposed LaTeX writes every
//! label ([`SafeLabels`](crate::stdcore::features::refs::SafeLabels)), and
//! requests the LaTeX package `amsmath`, which defines `\eqref`. The numbering is
//! LaTeX's there, so the compiled document shows the number LaTeX gave the
//! equation, not the one FLM computed.
//!
//! An `\eqref` whose label no equation line carries is written under the safe
//! label assigned to that label, as every reference to a missing label is, and
//! LaTeX reports it as an undefined reference. A label of a type other than `eq`
//! is the same Error as on every other backend, and the label is written as the
//! document wrote it, never as the safe label of the construct that carries that label, so
//! a recomposed `\eqref` never points at a heading.
//!
//! The label is read as characters first, as a `\ref`'s is
//! ([`read_target_at_lowering`]). A label that holds a placeholder of a user
//! definition's template cannot be read that way; the uses of such a definition
//! are then expanded instead of written as calls, and the label of an expanded
//! use is written through [`target_spelling`]. That function also handles a
//! label that appears only inside a [synthesized
//! value](crate::guide::concepts_overview#synthesized-values) (a small tree that
//! a stage builds during a run): it reads the label's name from the references
//! feature's value names table ([`ValueSafeLabels`], `flm.labels.safe.value`).
//!
//! # Without the references feature
//!
//! The rules read the tables of the references feature optionally, under peer
//! declarations ([`PeerName`](crate::core::registry::PeerName)): when that
//! feature is registered, the stages that write the tables run before the rules;
//! when it is not, the command shows its label as the document wrote it,
//! unlinked. A registry with mathematics and without references therefore
//! renders an `\eqref` instead of refusing the plan.
//!
//! Rendering reports nothing about the missing references feature; the registry
//! build has already reported one Note for each unresolved peer declaration. The
//! check of the label type reads what the author wrote, before any table is read, so
//! `\eqref{eq:euler}` in such a registry shows `eq:euler` unlinked and reports
//! nothing, and only a label of another type is an Error.
//!
//! # Map of the module
//!
//! - [`EQREF`], [`element`], [`EqrefElement`], [`args`] — the element, its only
//!   element value and its arguments; [`eqref_target`] reads the label of a node.
//! - [`RULE`], [`EqrefRule`], [`LATEX_RULE`], [`EqrefLatexRule`], [`rules`] — the
//!   two lowering rules and their bodies.
//! - [`NotAnEquation`] — the condition of a label of the wrong type.
//! - [`register`] — registers the element, the rules and the peer declarations.

use std::sync::Arc;
use std::sync::LazyLock;

use techy::core::node::{BuildId, NodeRef};
use techy::error::{Diagnostic, DiagnosticInfo};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::lang::text::{is_space, read_chars};
use crate::lang::mint::{ArgDef, ArgStdKind, ElementDef, SyntaxDef};
use crate::model::aspects::{
    ApartFromText, BlockRole, BlockRoleAspect, ContentRef, FixedBlockRole, Label, RefDisplayOptions,
    RefSite, Referencing,
};
use crate::model::element::{ElementSpec, element_value};
use crate::model::names::{ElementName, FeatureName, Name};
use crate::stdcore::backends::latex::construct::{ARG_SLOT, MACRO, RAW};
use crate::stdcore::backends::latex::{LatexArgShape, LatexMacroPayload, packages};
use crate::stdcore::backends::latex_recomposed::BACKEND as LATEX_BACKEND;
use crate::stdcore::backends::pres;
use crate::stdcore::features::numbering::COUNTERS;
use crate::stdcore::features::refs::{
    LabelSpelling, REF_RESOLUTIONS, RefResolutions, RefTargetText, SAFE_LABELS, TARGET_ARG,
    TARGET_ARG_ROLE, VALUE_SAFE_LABELS, ValueSafeLabels, lower_reference, read_target_at_lowering,
    stage_spelling, target_spelling,
};

use super::LABEL_TYPES;

/// The name of the element of `\eqref`, `flm.math.eqref`. Its element value
/// carries an [`EqrefElement`], which is also the element's [`Referencing`]
/// aspect.
pub const EQREF: &str = "flm.math.eqref";

/// The name of the default lowering rule of [`EQREF`] on every backend,
/// `flm.math.eqref.lower.default`, whose body is [`EqrefRule`].
pub const RULE: &str = "flm.math.eqref.lower.default";

/// The name of the recomposition rule of [`EQREF`] for the `latex-recomposed`
/// backend, `flm.math.eqref.lower.latex-recomposed`, whose body is
/// [`EqrefLatexRule`].
pub const LATEX_RULE: &str = "flm.math.eqref.lower.latex-recomposed";

/// The LaTeX command that the recomposed LaTeX writes, `eqref`: the `amsmath`
/// package's reference to an equation, which prints the number in
/// parentheses.
pub const LATEX_COMMAND: &str = "eqref";

/// The label type that the label of an `\eqref` must have, `eq`: the type of
/// the labels an equation line accepts.
pub const TARGET_TYPE: &str = LABEL_TYPES[0];

/// The data of the element value of `flm.math.eqref`, and the element's
/// [`Referencing`] aspect: it declares the reference site
/// ([`RefSite`]) of an `\eqref` node, a place that refers to a reference
/// point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EqrefElement;

/// The label of an `\eqref` node: its argument read as characters, trimmed of
/// the whitespace around it, and parsed when the text has the form
/// `type:label`. `None` when `node` is not an `\eqref`, or when its argument
/// holds a placeholder of a user definition's template that was not expanded:
/// such an argument has no characters to read, declares no reference site,
/// and nothing is reported for it.
///
/// The text is **not** split at commas: an `\eqref` takes one label, and a
/// comma in the text is part of that label.
pub fn eqref_target(node: &NodeRef<'_, Flm>) -> Option<RefTargetText> {
    element_value::<EqrefElement>(node)?;
    let text = match node.argument_content_nodes_named(TARGET_ARG) {
        Ok(Some(content)) => match read_chars(content.iter()) {
            Ok(read) if read.has_placeholder() => return None,
            Ok(read) => read.text().unwrap_or_default().trim_matches(is_space).to_string(),
            Err(_) => String::new(),
        },
        _ => String::new(),
    };
    let label = Label::parse(&text);
    Some(RefTargetText { text, label })
}

/// Whether `target` is a label that an `\eqref` accepts: one of the form
/// `type:label` whose type is [`TARGET_TYPE`].
///
/// Both the reference site that the element declares and the Error that the
/// rules report use this function, so the two always agree.
fn names_an_equation(target: &RefTargetText) -> bool {
    target.label.as_ref().is_some_and(|label| label.ref_type == TARGET_TYPE)
}

impl Referencing for EqrefElement {
    /// The reference site of an `\eqref` whose label has the type `eq`, with
    /// the display options of `\ref[noprefix]`: no prefix words before the
    /// number, and no variant.
    ///
    /// A label of any other type declares no site: the rules report it as an
    /// Error at the command, and nothing resolves it.
    fn sites(&self, node: &NodeRef<'_, Flm>) -> Vec<RefSite> {
        let Some(target) = eqref_target(node).filter(names_an_equation) else {
            return Vec::new();
        };
        let Some(label) = target.label else { return Vec::new() };
        vec![RefSite {
            target: label,
            display: RefDisplayOptions { variant: None, with_prefix: false },
            shown: true,
        }]
    }
}

/// The label of an `\eqref`, its argument [`TARGET_ARG`], is not part of the
/// text around the command, as a `\ref`'s label is not.
impl ApartFromText for EqrefElement {
    fn apart_from_text(&self, _node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        vec![ContentRef::ArgumentNamed(TARGET_ARG.to_string())]
    }
}

static EQREF_ELEMENT: LazyLock<Arc<ElementSpec>> = LazyLock::new(|| {
    Arc::new(
        ElementSpec::new(EQREF)
            .docs("A reference to an equation line that shows its number in parentheses.")
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT)))
            .with::<dyn ApartFromText>(Arc::new(EqrefElement))
            .with::<dyn Referencing>(Arc::new(EqrefElement))
            .with_value(Arc::new(EqrefElement)),
    )
});

/// The element value of every `\eqref` node. It is one shared value, so that no
/// second element value named `flm.math.eqref` is ever created.
pub fn element() -> Arc<ElementSpec> {
    Arc::clone(&EQREF_ELEMENT)
}

/// The arguments of `\eqref`: its one label. There are no display options,
/// because an `\eqref` always shows the number alone.
pub fn args() -> Vec<ArgDef> {
    vec![
        ArgDef::named(TARGET_ARG, ArgStdKind::CharsGroup)
            .docs("The equation's label, `eq:label`.")
            .role(TARGET_ARG_ROLE),
    ]
}

/// Condition `flm.math.eqref-not-an-equation` (an Error): the label of an
/// `\eqref` does not have the label type `eq`.
///
/// Both rules report it at the command, on every backend. The recovery: the
/// command shows the label's text, unlinked, as an unresolved reference does,
/// and the recomposed LaTeX writes the label as the document wrote it.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.math.eqref-not-an-equation",
    message = "`\\eqref` refers to equations: the label `{label}` must begin with `{expected}:`"
)]
pub struct NotAnEquation {
    /// The label as the document wrote it.
    pub label: String,
    /// The label type the label must have, `eq`.
    pub expected: String,
}

/// The body of [`RULE`]: it shows the line's number through the references
/// feature's [`lower_reference`], and reports [`NotAnEquation`] for a label of
/// another type. See the module documentation, *What the command shows*.
///
/// The rule returns a [`LowerError`] when the node is not an `\eqref`, when a
/// table cannot be read, or when the reference could not be lowered.
#[derive(Debug, Default, Clone, Copy)]
pub struct EqrefRule;

impl Lower for EqrefRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let mut target = eqref_target(&node)
            .ok_or_else(|| LowerError::structure("an `\\eqref` without its element's data"))?;
        // The condition is read from what the author wrote, before any table
        // is consulted, so that a correct target is never reported because a
        // registry holds no feature to resolve it.
        let names_equation = names_an_equation(&target);
        if !names_equation {
            let condition = NotAnEquation {
                label: target.text.clone(),
                expected: String::from(TARGET_TYPE),
            };
            lw.diagnose(Diagnostic::error(condition, node.span().clone()));
        }
        // Two ways to have nothing to point at: the element declared no site
        // for a target of the wrong type, and where no feature resolves
        // references no target resolves at all. Either way the command is
        // shown the way a target that is no label at all is shown: its text,
        // unlinked.
        if !names_equation || lw.get(REF_RESOLUTIONS)?.is_none() {
            target.label = None;
        }
        let display = RefDisplayOptions { variant: None, with_prefix: false };
        // `\eqref` shows its one item bare, without the `one_pre` and
        // `one_post` a one-target `\ref` takes.
        let items = lower_reference(cx, lw, &node, std::slice::from_ref(&target), &display)?;
        Ok(Lowering::Emit(items.into_iter().flatten().collect()))
    }
}

/// The body of [`LATEX_RULE`]: `\eqref` around the label's safe label, and a
/// request for the LaTeX package `amsmath`. See the module documentation,
/// *Recomposing to LaTeX*.
///
/// The rule returns a [`LowerError`] when the label holds a placeholder of a
/// template ([`read_target_at_lowering`]), when a table cannot be read, or when
/// a node could not be staged.
#[derive(Debug, Default, Clone, Copy)]
pub struct EqrefLatexRule;

impl Lower for EqrefLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        read_target_at_lowering(&node)?;
        lw.require(&Name::from(packages::AMSMATH));
        let target = eqref_target(&node).unwrap_or(RefTargetText {
            text: String::new(),
            label: None,
        });
        let spelling = if names_an_equation(&target) {
            target_spelling(lw, &target.text, target.label.as_ref())?
        } else {
            // The same condition as on every other backend, at the same node,
            // so that the one backend whose output a LaTeX compiler reads is
            // not the one where a wrong target passes. The label is written as
            // the document wrote it, never as the name allocated for whatever
            // does carry it, so that the reference cannot resolve to a heading.
            let condition = NotAnEquation {
                label: target.text.clone(),
                expected: String::from(TARGET_TYPE),
            };
            lw.diagnose(Diagnostic::error(condition, node.span().clone()));
            LabelSpelling::Raw(target.text.clone())
        };
        let key = stage_spelling(cx, lw, &spelling)?;
        Ok(Lowering::Emit(vec![eqref_command(cx, lw, key)?]))
    }
}

/// Stages the macro construct `\eqref` around `key`, the staged label.
fn eqref_command(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    key: Vec<BuildId>,
) -> Result<BuildId, LowerError> {
    lw.pres_slots_with(
        cx,
        &ElementName::from(MACRO),
        LatexMacroPayload::new(LATEX_COMMAND, [LatexArgShape::Mandatory]),
        vec![(String::from(ARG_SLOT), key)],
    )
}

/// The two rules of `\eqref`, ready to be registered: [`RULE`], the default on
/// every backend, and [`LATEX_RULE`], the default on the `latex-recomposed`
/// backend only.
///
/// [`RULE`] reads the resolutions table of the references feature and the
/// counter table of the numbering feature, both optionally: when the registry
/// holds the feature that writes a table, the plan runs that stage before the
/// rule; when it does not, the command shows the label as the document wrote
/// it. [`LATEX_RULE`] reads the safe labels and the value names of the
/// references feature in the same way.
pub fn rules() -> Vec<LoweringRule> {
    vec![
        LoweringRule::new(
            RULE,
            EQREF,
            LoweringDecl::new()
                .reads_opt(REF_RESOLUTIONS)
                .reads_opt(COUNTERS)
                .produces(pres::LINK)
                .produces(pres::SPAN),
            Arc::new(EqrefRule),
            DefaultMark::Everywhere,
        )
        .docs(
            "an `\\eqref` lowers to what `\\ref[noprefix]` lowers to for the same label: the \
             line's number in parentheses, inside a link to the equation line",
        ),
        LoweringRule::new(
            LATEX_RULE,
            EQREF,
            LoweringDecl::new()
                .reads_opt(SAFE_LABELS)
                .reads_opt(VALUE_SAFE_LABELS)
                .produces(MACRO)
                .produces(RAW),
            Arc::new(EqrefLatexRule),
            DefaultMark::OnBackends(vec![LATEX_BACKEND.into()]),
        )
        .docs("an `\\eqref` lowers to the LaTeX `\\eqref` around its label's safe label"),
    ]
}

/// Registers `\eqref`: peer declarations
/// ([`PeerName`](crate::core::registry::PeerName)) of the references feature's
/// resolutions table and value names, the element, and its two rules.
///
/// The peer declarations that the recomposition rule also needs (the
/// `latex-recomposed` backend and the macro and raw constructs) are made by
/// the feature's [`latex`](super::latex) module, so a registry without the
/// LaTeX features sets this rule aside (excludes it from rule selection) with
/// the others.
///
/// # Errors
///
/// Returns the [`BuildError`] the feature builder raises, for instance for an
/// element or a rule already registered under the same name.
pub fn register(b: &mut FeatureBuilder<'_>, owner: &FeatureName) -> Result<(), BuildError> {
    // The resolutions table belongs to the references feature, and the rule
    // reads it optionally: where that feature is registered the declaration
    // resolves and the read is passed on as a required one, so the stage that
    // writes the table runs before the rule; where it is not, the declaration
    // goes unresolved with one Note and the rule still runs, showing the
    // label as the document wrote it. An optional read never sets a rule
    // aside.
    b.peer_table::<RefResolutions>(
        REF_RESOLUTIONS.name(),
        "the references feature's resolutions table, which the `\\eqref` rule reads for the \
         reference point its label refers to",
    )?;
    // The value names belong to the references feature too, and are read the
    // same way by the recomposition rule: a target an `\eqref` spliced at
    // lowering names is written under the name its value gives it.
    b.peer_table::<ValueSafeLabels>(
        VALUE_SAFE_LABELS.name(),
        "the references feature's value names table (`flm.labels.safe.value`), which the \
         `\\eqref` recomposition rule reads for the safe label of a label that appears only inside a synthesized value",
    )?;
    // The label an `\eqref` holds is a reference's target, the role the
    // references feature declares; declared as a peer, so that a registry
    // without that feature still builds.
    b.peer_argument_role(TARGET_ARG_ROLE)?;
    b.element(
        ElementDef::new(owner.clone(), ["eqref"], SyntaxDef::Macro { args: args() }, element())
            .docs("`\\eqref{eq:label}`: a reference that shows an equation line's number."),
    )?;
    for rule in rules() {
        b.lowering_rule(rule)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_element_carries_the_referencing_aspect_and_is_one_shared_value() {
        let element = element();
        assert!(element.has_aspect::<dyn Referencing>());
        assert!(element.value_as::<EqrefElement>().is_some());
        assert!(ElementSpec::same_value(&super::element(), &super::element()));
    }

    #[test]
    fn only_a_label_of_the_equation_type_names_an_equation() {
        let target = |text: &str| RefTargetText {
            text: String::from(text),
            label: Label::parse(text),
        };
        assert!(names_an_equation(&target("eq:euler")));
        assert!(!names_an_equation(&target("sec:intro")));
        assert!(!names_an_equation(&target("euler")));
        assert!(!names_an_equation(&target("")));
    }
}
