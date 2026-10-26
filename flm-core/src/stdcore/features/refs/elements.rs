//! The `\label` field and the two referencing elements of the
//! [`refs`](super) feature, `\ref` and `\hyperref`: their declarations and
//! the functions that read what an author wrote in them.
//!
//! # The label field
//!
//! `\label{type:label}` is a
//! [field](crate::guide::concepts_overview#fields-data-a-construct-reads): a
//! macro that a construct absorbs as data after its own arguments, as in
//! `\section{Introduction}\label{sec:intro}`. The parser builds it, inside
//! the construct's trailing argument, as a node of the language's field
//! element ([`FIELD`](crate::lang::mint::FIELD)), whose per-value data is the
//! field's name, `label`. It can be written only after a construct that
//! declares it.
//!
//! A field is data that its parent construct reads, so no function here looks
//! for `\label` nodes across a document: the construct that declared the
//! field reads it with [`label_uses`] and turns what it read into the
//! reference point it declares. A `\label` node that reaches a backend's
//! lowering on its own is an error, reported by the field rule
//! ([`FieldRule`](crate::stdcore::backends::lowering::FieldRule)).
//!
//! A construct that accepts labels declares [`label_fields_arg`] as its last
//! argument: a tack-on argument (the trailing argument that absorbs field
//! macros; [`ArgStdKind::TackOn`]) named [`FIELDS_ARG`], holding a repeatable
//! field named [`LABEL_FIELD`] whose one character-group argument is named
//! [`TARGET_ARG`]. [`label_uses`] reads that shape, so the declaration is
//! written once, here.
//!
//! [`label_uses`] returns every field as written together with its parsed
//! label, and [`point_labels`] splits the result into the two lists a
//! reference point holds: the labels, and the texts that are not spelled
//! `type:label`, which the label scan reports.
//!
//! A `\label` that is not a tack-on field can be written in two other places.
//! In both, its node carries the same field element with the same name, so
//! that one function, [`label_use`], reads a `\label` node wherever it is
//! written:
//!
//! - inside the body of a display-math environment, where the
//!   [`math`](crate::stdcore::features::math) feature declares `\label` as a
//!   command of its own, so that the environment reads its lines' labels
//!   exactly as a heading reads its own;
//! - inside an argument that refers to the scoped package [`LABEL_PACKAGE`]
//!   (`flm.refs.label`), which this feature declares ([`label_package`]). A
//!   scoped package ([`ScopedPackageDef`]) is a set of element declarations
//!   that can be written only inside the environment bodies and arguments
//!   whose declarations refer to it by name
//!   ([scoped packages](crate::guide::concepts_overview#scoped-packages)). This
//!   one holds one command, `\label`, and the optional argument of `\footnote`
//!   refers to it, so that `\footnote[\label{fn:x}]{…}` labels the note. The
//!   construct reads the labels of such an argument with [`label_uses_in`],
//!   which reads a run of nodes.
//!
//! # The `\ref` element
//!
//! `\ref[options]{type:label}` is the element [`REF`] (`flm.ref`), one shared
//! element value ([`ref_element`]) whose per-value data is [`RefElement`],
//! which implements the [`Referencing`] aspect. **A `\ref` may point at
//! several targets**, written as one text with commas between them
//! (`\ref{eq:a,eq:b}`): the text is split at the commas ([`split_targets`])
//! and each part is a target of its own. The node declares one reference site
//! per target that is spelled `type:label`, in the order they were written; a
//! target that is not spelled that way declares no site, and the label scan
//! reports it.
//!
//! The key-value options argument holds the display options, read with
//! [`ref_display_options`]: a bare key requests a wording variant of the word
//! before a number, and the key [`NO_PREFIX_KEY`] (`noprefix`) leaves that
//! word out. The argument is declared with an opaque wrapper
//! ([`ArgDef::with_opaque_wrapper`]), because its keys are read as data: the
//! typographic substitutions of the document's text do not apply to them.
//! They are recorded as the document wrote them: the key
//! [`CAPITAL_KEY`] (`S`) is kept as written, and the `\ref` lowering rule
//! turns it into a request for the variant [`CAPITAL_VARIANT`] where the
//! counter style declares it, and for the usual wording otherwise. The
//! recomposed LaTeX leaves the typesetting of a reference to LaTeX and writes the
//! options back as the document wrote them; the default definition of the
//! LaTeX hook `\flmRef` translates them for the LaTeX package that sets the
//! reference
//! ([`refs_piece`](crate::stdcore::backends::latex::preamble::refs_piece)).
//!
//! # The `\hyperref` element
//!
//! `\hyperref[type:label]{text}` is the element [`HYPERREF`]
//! (`flm.hyperref`), one shared element value ([`hyperref_element`]) whose
//! per-value data is [`HyperrefElement`], which implements the
//! [`Referencing`] aspect as [`RefElement`] does: one reference site per node,
//! whose target is the label in the brackets. A `\hyperref` differs from a
//! `\ref` in one respect: it displays the text the author wrote in the
//! braces, not what its target has to show. Its site is therefore recorded
//! without the word before a number, so that nothing is placed before the
//! author's text, and the command takes no display options.
//!
//! The brackets are an *optional* argument, parsed as FLM code rather than as
//! the characters-only group a `\ref` target is written in, and declared with
//! an opaque wrapper ([`ArgDef::with_opaque_wrapper`]), so that the label in
//! them is kept as typed, as a `\ref` target is. [`hyperref_target`] reads it
//! as characters: a target that holds anything but characters (a command, or
//! a special character such as `~`) reads as the empty text, and so does an
//! absent argument. The empty text is not spelled `type:label`, so it declares
//! no site, and the label scan reports it as it reports any other text that is
//! not a label.
//!
//! [`reference_targets`] reads the targets of either element. The places that
//! walk every referencing node of the document call it: the label scan, the
//! safe-labels stage and the recomposition rules.

use std::sync::{Arc, LazyLock};

use techy::core::node::NodeRef;
use techy::extract::parse_keyval_drop_annotations;

use crate::model::aspects::{
    ApartFromText, BlockRole, BlockRoleAspect, ContentRef, FixedBlockRole, Label, MalformedLabel,
    RefDisplayOptions, RefSite, Referencing,
};
use crate::model::element::{ElementSpec, element_value};
use crate::lang::text::{chars_text, is_space, read_chars};
use crate::lang::{Flm, FlmSpan};
use crate::lang::mint::{
    ArgDef, ArgStdKind, ElementDef, ScopedPackageDef, SyntaxDef, TackOnField, field_element,
    is_field_named,
};
use crate::model::names::{FeatureName, Name};

/// The `\ref` element's type name, `flm.ref`.
///
/// Its per-value data is a [`RefElement`], which is also the element's
/// [`Referencing`] implementation. [`ref_element`] returns the element value.
pub const REF: &str = "flm.ref";

/// The `\label` field's name, `label`: what an author writes after the
/// backslash, and the name under which a construct declares the field.
pub const LABEL_FIELD: &str = "label";

/// The name of the trailing tack-on argument a construct holds its `\label`
/// fields in, `fields`: the argument [`label_fields_arg`] declares and
/// [`label_uses`] reads.
pub const FIELDS_ARG: &str = "fields";

/// The name of the target argument of a `\ref`, a `\hyperref` and a
/// `\label`, `target`: the argument that holds the label.
pub const TARGET_ARG: &str = "target";

/// The name of the scoped package that holds `\label` as a command,
/// `flm.refs.label`: an argument whose declaration refers to it
/// ([`ArgDef::scoped`](crate::lang::mint::ArgDef::scoped)) accepts
/// `\label{type:label}`. See [`label_package`].
pub const LABEL_PACKAGE: &str = "flm.refs.label";

/// The argument role of a label that a reference points to,
/// `flm.refs.target`: the role of the target argument ([`TARGET_ARG`]) of
/// `\ref` and `\hyperref`. The feature declares it; a feature with a
/// reference command of its own sets it on that command's target argument
/// and, so that a registry without this feature still builds, declares the
/// role as a peer
/// ([`FeatureBuilder::peer_argument_role`](crate::core::registry::FeatureBuilder::peer_argument_role)),
/// as the `math` feature does for `\eqref`.
pub const TARGET_ARG_ROLE: &str = "flm.refs.target";

/// The argument role of a label that a `\label` attaches to a construct,
/// `flm.refs.defined-label`: the role of the argument of the `\label` field
/// ([`label_field`]) and of the `\label` command of the scoped package
/// [`LABEL_PACKAGE`]. The feature declares it; a feature whose declarations
/// hold either declares the role as a peer
/// ([`FeatureBuilder::peer_argument_role`](crate::core::registry::FeatureBuilder::peer_argument_role)),
/// so that a registry without this feature still builds.
pub const DEFINED_LABEL_ARG_ROLE: &str = "flm.refs.defined-label";

/// The name of a `\ref`'s options argument, `options`.
pub const OPTIONS_ARG: &str = "options";

/// The `\hyperref` element's type name, `flm.hyperref`.
///
/// Its per-value data is a [`HyperrefElement`], which is also the element's
/// [`Referencing`] implementation. [`hyperref_element`] returns the element
/// value.
pub const HYPERREF: &str = "flm.hyperref";

/// The name of the argument holding a `\hyperref`'s displayed text, `text`.
pub const TEXT_ARG: &str = "text";

/// The key of a `\ref`'s options that leaves out the word before a number
/// (the prefix, such as "Eq."), `noprefix`.
pub const NO_PREFIX_KEY: &str = "noprefix";

/// The key of a `\ref`'s options that asks for the capitalized wording, `S`.
///
/// It is the only key of the display options that differs from the name of
/// the variant it selects, and the only one that is a preference rather than
/// a requirement: it selects [`CAPITAL_VARIANT`] where the counter style
/// declares that variant, and the style's usual wording where it does not,
/// with nothing reported. Every other bare key requests the variant of its
/// own name, and a counter style that does not declare it makes the reference
/// an Error ([`UndeclaredVariant`](super::UndeclaredVariant)).
pub const CAPITAL_KEY: &str = "S";

/// The counter style variant that the key [`CAPITAL_KEY`] selects, `capital`:
/// the wording a counter style declares for a reference that opens a sentence
/// (`Equation~(3)` rather than `Eq.~(3)`).
///
/// Requested through `S`, it is written where the counter style declares it,
/// and the style's usual wording is written otherwise, with nothing reported.
/// Written as a key itself (`\ref[capital]{…}`), it is a requirement like any
/// other variant: where the counter style of a numbered target does not
/// declare it, the reference is an Error, and that style writes its usual
/// wording.
pub const CAPITAL_VARIANT: &str = "capital";

/// The counter style variant of the words written all in lowercase,
/// `lowercase` (`lemma~4` rather than `Lemma~4`), which `\ref[lowercase]{…}`
/// requests.
pub const LOWERCASE_VARIANT: &str = "lowercase";

/// The counter style variant of the abbreviated words, `abbreviated`
/// (`Lem.~4` rather than `Lemma~4`), which `\ref[abbreviated]{…}` requests.
pub const ABBREVIATED_VARIANT: &str = "abbreviated";

/// The per-value data of the `flm.ref` element, and the element's
/// [`Referencing`] implementation: it returns the reference sites a `\ref`
/// node declares, one per target spelled `type:label`. See the module
/// documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefElement;

/// The per-value data of the `flm.hyperref` element, and the element's
/// [`Referencing`] implementation: it returns the one reference site a
/// `\hyperref` node declares, or none when its target is not spelled
/// `type:label`. See the module documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HyperrefElement;

/// The trailing tack-on argument that a labeled construct declares as its
/// last argument, so that `\thing{…}\label{x:y}` parses: one repeatable
/// `\label` field with one character-group argument, under the names that
/// [`label_uses`] reads ([`FIELDS_ARG`], [`LABEL_FIELD`], [`TARGET_ARG`]).
///
/// The field carries no element of this feature: like every field, its node
/// carries the language layer's field element
/// ([`FIELD`](crate::lang::mint::FIELD)) with the field's name as its
/// per-value data, and only the construct that declared it reads it, through
/// [`label_uses`]. A construct that accepts other fields beside `\label`
/// builds the argument itself and lists the same field ([`label_field`])
/// among them.
///
/// The field's argument has the argument role [`DEFINED_LABEL_ARG_ROLE`],
/// which this feature declares. A feature whose declarations use this
/// argument also declares that role as a peer
/// ([`FeatureBuilder::peer_argument_role`](crate::core::registry::FeatureBuilder::peer_argument_role)),
/// as the standard features do, so that a registry without this feature
/// still builds; without the peer declaration, building such a registry
/// fails.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::refs::{FIELDS_ARG, label_fields_arg};
///
/// let arg = label_fields_arg();
/// assert_eq!(arg.name.as_deref(), Some(FIELDS_ARG));
/// assert_eq!(arg.kind.tack_on_fields().map(<[_]>::len), Some(1));
/// ```
pub fn label_fields_arg() -> ArgDef {
    ArgDef::named(FIELDS_ARG, ArgStdKind::TackOn { fields: vec![label_field()] })
        .docs("`\\label{…}` fields written after the construct, each attaching a label to it.")
}

/// The `\label` field of [`label_fields_arg`]: repeatable, with one
/// character-group argument named [`TARGET_ARG`]. A construct that accepts
/// other fields beside `\label` lists this one among them in its own tack-on
/// argument, which it declares under the name [`FIELDS_ARG`] so that
/// [`label_uses`] reads it.
///
/// The field's argument has the argument role [`DEFINED_LABEL_ARG_ROLE`]:
/// a feature that lists the field also declares that role as a peer, as for
/// [`label_fields_arg`].
pub fn label_field() -> TackOnField {
    TackOnField::new(
        LABEL_FIELD,
        vec![
            ArgDef::named(TARGET_ARG, ArgStdKind::CharsGroup)
                .docs("The label, written `type:label` (`sec:intro`).")
                .role(DEFINED_LABEL_ARG_ROLE),
        ],
    )
    .repeatable()
    .docs("Attaches a label to the construct, which a reference can point at; may be repeated.")
}

/// One `\label` written on a node, as [`label_uses`], [`label_uses_in`] and
/// [`label_use`] return it: the text as written, the span of the `\label`
/// node, and the label when the text is spelled `type:label`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelUse {
    /// The text between the braces.
    pub text: String,
    /// The span of the `\label` node.
    pub span: FlmSpan,
    /// The parsed label, when the text is `type:label`.
    pub label: Option<Label>,
}

/// The text of a `\label` node (the characters of its target argument,
/// trimmed), or `None` when the node is not a `\label` field, has no target
/// argument, or holds something that is not characters: a command, or a
/// placeholder of a user definition's template that was not expanded
/// ([`chars_text`]). The text is not checked for the `type:label` spelling.
///
/// Returning `None` for a placeholder does not let a labeled template
/// through unnoticed: the user-definitions feature refuses a template, kept
/// rather than expanded, that holds a `\label` completed by a placeholder
/// ([`KeptTemplateDeclaresPoint`](crate::stdcore::features::userdefs::KeptTemplateDeclaresPoint)).
pub fn field_text(node: &NodeRef<'_, Flm>) -> Option<String> {
    if !is_field_named(node, LABEL_FIELD) {
        return None;
    }
    let content = node.argument_content_nodes_named(TARGET_ARG).ok()??;
    chars_text(content.iter()).map(|text| text.trim_matches(is_space).to_string())
}

/// Every `\label` field written on `node`, in its [`FIELDS_ARG`] argument, in
/// document order. Empty for a node with no such argument.
///
/// A construct that declared the field ([`label_fields_arg`]) calls this
/// function to read its labels. Pass the result to [`point_labels`] to build
/// the reference point the construct declares.
pub fn label_uses(node: &NodeRef<'_, Flm>) -> Vec<LabelUse> {
    let Ok(Some(fields)) = node.argument_content_nodes_named(FIELDS_ARG) else {
        return Vec::new();
    };
    label_uses_in(fields.iter())
}

/// Every `\label` node of `nodes`, in their order, read as [`label_uses`]
/// reads the fields written on a node. A construct whose labels are written
/// in a run of nodes of their own calls it: the content of an argument that
/// refers to the scoped package [`LABEL_PACKAGE`], such as the optional
/// argument of `\footnote`. Every other node of the run is skipped; a
/// construct that must refuse anything but labels in that argument looks for
/// the other nodes itself.
///
/// Pass the result to [`point_labels`] to build the reference point the
/// construct declares.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
/// use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
/// use flm_core::model::aspects::Label;
/// use flm_core::model::doc::FragmentLayout;
/// use flm_core::stdcore::{self, StdcoreConfig};
/// use flm_core::stdcore::features::refs::label_uses_in;
///
/// let mut builder = RegistryBuilder::new();
/// builder.use_package(stdcore::package(StdcoreConfig::new()));
/// builder.use_package(flm_core::math_passthrough::package());
/// let registry = builder.build(&RegistryInputs::new())?;
/// let source = FlmSource::new(r"Text.\footnote[\label{fn:a} \label{fn:b}]{A note.}");
/// let fragment = parse_fragment(
///     registry.lang(),
///     Arc::new(source.with_origin(FlmOrigin::none())),
///     FragmentLayout::default(),
///     &ParseOptions::default(),
/// )?;
/// let note = fragment.tree().root().descendants()
///     .find(|node| node.name() == Some("footnote"))
///     .expect("the note");
/// let run = note.argument_content_nodes_named("labels")?.expect("written");
/// let uses = label_uses_in(run.iter());
/// assert_eq!(uses.len(), 2);
/// assert_eq!(uses[1].label, Some(Label::new("fn", "b")));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn label_uses_in<'t>(nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>) -> Vec<LabelUse> {
    // One reader of one `\label` node, [`label_use`], is what reads each of
    // them, wherever they stand, so the forms can never answer differently.
    nodes.into_iter().filter_map(|node| label_use(&node)).collect()
}

/// The scoped package [`LABEL_PACKAGE`], declared by `owner`: one command,
/// `\label{type:label}`, whose one character-group argument is named
/// [`TARGET_ARG`] and whose element is the field element named
/// [`LABEL_FIELD`], the same node that a tack-on `\label` produces, so that
/// [`label_use`] reads it as it reads any other.
///
/// The `refs` feature registers it. A scoped package
/// ([`ScopedPackageDef`]) can be written only inside the bodies and arguments
/// whose declarations refer to it, so `\label{…}` is accepted in such an
/// argument and nowhere else: `\label` written in running text is an unknown
/// command, as everywhere outside a construct that accepts labels. A feature
/// that refers to the package from an argument of its own declares it with a
/// peer declaration
/// ([`FeatureBuilder::peer_scoped_package`](crate::core::registry::FeatureBuilder::peer_scoped_package)),
/// so that a registry without this feature still builds; the argument then
/// accepts no `\label`.
///
/// The argument of the package's `\label` has the argument role
/// [`DEFINED_LABEL_ARG_ROLE`], which this feature declares. A feature that
/// places the package's declaration in a scoped package of its own, as the
/// `floats` feature does, also declares that role as a peer
/// ([`FeatureBuilder::peer_argument_role`](crate::core::registry::FeatureBuilder::peer_argument_role)),
/// so that a registry without this feature still builds.
///
/// The registry's feature builder attributes the package and its declaration
/// to the registering feature regardless of `owner`; the parameter matters
/// only for a package built by hand.
pub fn label_package(owner: impl Into<FeatureName>) -> ScopedPackageDef {
    let owner = owner.into();
    let label = ElementDef::new(
        owner.clone(),
        [LABEL_FIELD],
        SyntaxDef::Macro {
            args: vec![
                ArgDef::named(TARGET_ARG, ArgStdKind::CharsGroup)
                    .docs("The label, written `type:label` (`fn:intro`).")
                    .role(DEFINED_LABEL_ARG_ROLE),
            ],
        },
        field_element(
            LABEL_FIELD,
            "A `\\label{type:label}` inside an argument that accepts labels: a label \
             attached to the construct the argument belongs to.",
        ),
    )
    .docs("`\\label{type:label}`: attaches a label to the construct whose argument holds it.");
    ScopedPackageDef {
        owner,
        name: Name::from(LABEL_PACKAGE),
        elements: vec![label],
        docs: String::from(
            "The command available in an argument that accepts labels: \
             `\\label{type:label}`.",
        ),
    }
}

/// Splits the `\label`s a construct read into the two lists a reference
/// point holds: the labels spelled `type:label`, in the order they were
/// written, and the texts that are not, each with the span to report it at
/// and its position in the whole list.
///
/// A construct's [`Referenceable`](crate::model::aspects::Referenceable)
/// implementation passes the two lists unchanged into the point it declares
/// ([`RefPoint::labels`](crate::model::aspects::RefPoint::labels) and
/// [`RefPoint::malformed`](crate::model::aspects::RefPoint::malformed)). A
/// malformed text defines nothing; the label scan ([`SCAN`](super::SCAN))
/// reports it once, at its own
/// `\label`, when it reads the point. The positions put the two lists back
/// into the order the document wrote them
/// ([`RefPointRecord::mentions`](crate::stdcore::features::refpoints::RefPointRecord::mentions)).
pub fn point_labels<'u>(
    uses: impl IntoIterator<Item = &'u LabelUse>,
) -> (Vec<Label>, Vec<MalformedLabel>) {
    let mut labels = Vec::new();
    let mut malformed = Vec::new();
    for (position, us) in uses.into_iter().enumerate() {
        match &us.label {
            Some(label) => labels.push(label.clone()),
            None => malformed.push(MalformedLabel {
                text: us.text.clone(),
                span: us.span.clone(),
                position,
            }),
        }
    }
    (labels, malformed)
}

/// Reads one `\label` node, wherever it is written (a field absorbed by a
/// labeled element, the `\label` command the
/// [`math`](crate::stdcore::features::math) feature declares inside a
/// display-math environment's body, or the command of the scoped package
/// [`LABEL_PACKAGE`]): its text as written, its span, and the label when the
/// text is spelled `type:label`. `None` when `node` is not a `\label` node,
/// has no target argument, or holds something that is not characters
/// ([`field_text`]).
///
/// [`label_uses`] reads the fields attached to a labeled element;
/// [`label_uses_in`] reads a run of nodes; this function reads one `\label`
/// node.
pub fn label_use(node: &NodeRef<'_, Flm>) -> Option<LabelUse> {
    let text = field_text(node)?;
    let label = Label::parse(&text);
    Some(LabelUse { text, span: node.span().clone(), label })
}

/// One target written in a reference command: the text as written, with the
/// whitespace around it trimmed, and the label when that text is spelled
/// `type:label`.
///
/// A `\ref` may point at several targets, so its target argument is read as
/// a list of these ([`ref_targets`]); a `\hyperref` has one
/// ([`hyperref_target`]).
///
/// A target whose text is not spelled `type:label` points at nothing: it
/// declares no reference site, the label scan ([`SCAN`](super::SCAN)) reports it, and the output
/// shows the text as written, marked unresolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefTargetText {
    /// The text as written, trimmed.
    pub text: String,
    /// The label, when the text is spelled `type:label`.
    pub label: Option<Label>,
}

/// The targets written in the text of a reference's target argument: the text
/// split at the commas, each part trimmed and parsed as a label.
///
/// The empty text is one target, not none, so that `\ref{}` has one target,
/// which points at nothing, rather than no target at all.
///
/// # Examples
///
/// ```
/// use flm_core::model::aspects::Label;
/// use flm_core::stdcore::features::refs::split_targets;
///
/// let targets = split_targets("sec:intro, eq:euler");
/// assert_eq!(targets.len(), 2);
/// assert_eq!(targets[0].label, Some(Label::new("sec", "intro")));
/// assert_eq!(targets[1].text, "eq:euler");
///
/// // A part that is not spelled `type:label` has no label.
/// assert_eq!(split_targets("nolabel")[0].label, None);
/// ```
pub fn split_targets(text: &str) -> Vec<RefTargetText> {
    text.split(',')
        .map(|piece| {
            let text = piece.trim_matches(is_space).to_string();
            let label = Label::parse(&text);
            RefTargetText { text, label }
        })
        .collect()
}

/// The targets of a `\ref` node, in the order they were written: its target
/// argument read as characters and split at the commas ([`split_targets`]).
///
/// `None` when the node is not a `\ref`, has no target argument, or holds
/// something that is not characters: a command, or a placeholder of a user
/// definition's template that was not expanded ([`chars_text`]). Such a
/// target has no text to read, declares no site, and nothing is reported for
/// it.
pub fn ref_targets(node: &NodeRef<'_, Flm>) -> Option<Vec<RefTargetText>> {
    element_value::<RefElement>(node)?;
    let content = node.argument_content_nodes_named(TARGET_ARG).ok()??;
    let text = chars_text(content.iter())?;
    Some(split_targets(&text))
}

/// A `\ref` node's display options, read from its key-value options
/// argument: a key other than [`NO_PREFIX_KEY`] requests the wording variant
/// of its own name (the last such key wins), and [`NO_PREFIX_KEY`] leaves out
/// the word before a number. An absent or unreadable argument gives the
/// default options.
pub fn ref_display_options(node: &NodeRef<'_, Flm>) -> RefDisplayOptions {
    let mut options = RefDisplayOptions::default();
    let Ok(Some(content)) = node.argument_content_nodes_named(OPTIONS_ARG) else {
        return options;
    };
    let Ok(keyvals) = parse_keyval_drop_annotations(content) else {
        return options;
    };
    for entry in keyvals.iter() {
        let key = entry.key();
        if key.is_empty() {
            continue;
        }
        if key == NO_PREFIX_KEY {
            options.with_prefix = false;
        } else {
            options.variant = Some(key.to_string());
        }
    }
    options
}

/// The target of a `\hyperref` node (the label written in the brackets) as
/// characters with the whitespace around it trimmed, and the label when that
/// text is spelled `type:label`.
///
/// `None` when the node is not a `\hyperref`, or when its target holds a
/// placeholder of a user definition's template that was not expanded: such a
/// target has no text to read, declares no site, and nothing is reported for
/// it.
///
/// The brackets are an optional argument parsed as FLM code and kept as
/// typed, so a target that is not made of characters (a command, a special
/// character such as `~`) reads as the empty text, and so does an absent
/// argument (`\hyperref{text}`). The empty text is not a label: the label scan
/// ([`SCAN`](super::SCAN)) reports it, and the reference is shown unresolved.
/// See the module documentation.
pub fn hyperref_target(node: &NodeRef<'_, Flm>) -> Option<RefTargetText> {
    element_value::<HyperrefElement>(node)?;
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

/// The targets of `node` when it is a `\ref` or a `\hyperref`:
/// [`ref_targets`] for a `\ref`, the one target of [`hyperref_target`] for a
/// `\hyperref`, and `None` for any other node.
///
/// The places that walk every node of a document to find the targets of
/// references call it: the label scan ([`SCAN`](super::SCAN)), which reports a target that
/// is not spelled `type:label`; the safe-labels stage
/// ([`SafeLabelsStage`](super::SafeLabelsStage)), which allocates a safe label
/// at a label's first mention; and the recomposition rules, which write those
/// safe labels.
pub fn reference_targets(node: &NodeRef<'_, Flm>) -> Option<Vec<RefTargetText>> {
    ref_targets(node).or_else(|| hyperref_target(node).map(|target| vec![target]))
}

impl Referencing for RefElement {
    /// One site per target of the `\ref` that is spelled `type:label`, in the
    /// order the targets were written, all with the node's own display
    /// options. A target that is not a label declares no site, so the index
    /// of a site is its place among the *labeled* targets, not among all
    /// targets; [`lower_reference`](super::lower_reference) counts the same
    /// way.
    fn sites(&self, node: &NodeRef<'_, Flm>) -> Vec<RefSite> {
        let display = ref_display_options(node);
        ref_targets(node)
            .into_iter()
            .flatten()
            .filter_map(|target| target.label)
            .map(|target| RefSite { target, display: display.clone(), shown: true })
            .collect()
    }
}

impl Referencing for HyperrefElement {
    /// The one site a `\hyperref` declares, or none when its target is not
    /// spelled `type:label`. Its display options leave out the word before a
    /// number, because the site shows the author's own text and nothing is
    /// placed before it.
    fn sites(&self, node: &NodeRef<'_, Flm>) -> Vec<RefSite> {
        match hyperref_target(node) {
            Some(RefTargetText { label: Some(target), .. }) => vec![RefSite {
                target,
                display: RefDisplayOptions { variant: None, with_prefix: false },
                shown: true,
            }],
            _ => Vec::new(),
        }
    }
}

/// The target of a `\ref` (its argument [`TARGET_ARG`]) and its display
/// options (its argument [`OPTIONS_ARG`]) are not words of the text around
/// the reference: they are instructions to the reference. A heading whose
/// title holds a `\ref` is identified by the title's own words, without the
/// target and the options.
impl ApartFromText for RefElement {
    fn apart_from_text(&self, _node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        vec![
            ContentRef::ArgumentNamed(TARGET_ARG.to_string()),
            ContentRef::ArgumentNamed(OPTIONS_ARG.to_string()),
        ]
    }
}

/// The target of a `\hyperref` (its argument [`TARGET_ARG`]) is not part of
/// the words of the text around the reference, as for a `\ref`; the text the
/// author wrote for it is, and is kept.
impl ApartFromText for HyperrefElement {
    fn apart_from_text(&self, _node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        vec![ContentRef::ArgumentNamed(TARGET_ARG.to_string())]
    }
}

static REF_ELEMENT: LazyLock<Arc<ElementSpec>> = LazyLock::new(|| {
    Arc::new(
        ElementSpec::new(REF)
            .docs(
                "A reference to a labeled place, showing its number, else its content, else \
                 the label.",
            )
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT)))
            .with::<dyn ApartFromText>(Arc::new(RefElement))
            .with::<dyn Referencing>(Arc::new(RefElement))
            .with_value(Arc::new(RefElement)),
    )
});

/// The element value of every `\ref` node. It is one shared value, like the
/// label field's, so that no caller can create a second element value of type
/// `flm.ref` (an element is identified by its value, not by its name).
pub fn ref_element() -> Arc<ElementSpec> {
    Arc::clone(&REF_ELEMENT)
}

/// The arguments of `\ref`, in order: the key-value options, then the target
/// label. A feature declaring a reference-like command of its own can reuse
/// this list.
pub fn ref_args() -> Vec<ArgDef> {
    vec![
        ArgDef::named(OPTIONS_ARG, ArgStdKind::KeyVals)
            .docs(
                "Display options: the name of a wording variant of the word before a number, \
                 `S` for the capitalized wording, or `noprefix` to leave the word out.",
            )
            .with_opaque_wrapper(),
        ArgDef::named(TARGET_ARG, ArgStdKind::CharsGroup)
            .docs("The target label, `type:label`, or several separated by commas.")
            .role(TARGET_ARG_ROLE),
    ]
}

static HYPERREF_ELEMENT: LazyLock<Arc<ElementSpec>> = LazyLock::new(|| {
    Arc::new(
        ElementSpec::new(HYPERREF)
            .docs("A reference to a labeled place, showing the text written for it.")
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT)))
            .with::<dyn ApartFromText>(Arc::new(HyperrefElement))
            .with::<dyn Referencing>(Arc::new(HyperrefElement))
            .with_value(Arc::new(HyperrefElement)),
    )
});

/// The element value of every `\hyperref` node, shared as [`ref_element`]'s
/// is for every `\ref` node.
pub fn hyperref_element() -> Arc<ElementSpec> {
    Arc::clone(&HYPERREF_ELEMENT)
}

/// The arguments of `\hyperref`, in order: the optional target label, then
/// the text to display for it.
pub fn hyperref_args() -> Vec<ArgDef> {
    vec![
        ArgDef::named(TARGET_ARG, ArgStdKind::Optional)
            .docs("The target label, `type:label`.")
            .role(TARGET_ARG_ROLE)
            .with_opaque_wrapper(),
        ArgDef::named(TEXT_ARG, ArgStdKind::Mandatory).docs("The text shown for the target."),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ref_element_carries_the_referencing_aspect_and_its_value() {
        let element = ref_element();
        assert!(element.has_aspect::<dyn Referencing>());
        assert!(element.value_as::<RefElement>().is_some());
    }

    #[test]
    fn the_ref_element_is_one_shared_value() {
        assert!(ElementSpec::same_value(&ref_element(), &ref_element()));
    }

    #[test]
    fn the_hyperref_element_carries_the_referencing_aspect_and_is_one_shared_value() {
        let element = hyperref_element();
        assert!(element.has_aspect::<dyn Referencing>());
        assert!(element.value_as::<HyperrefElement>().is_some());
        assert!(ElementSpec::same_value(&hyperref_element(), &hyperref_element()));
        // Two elements, two values: a rule for one never fires at the other.
        assert!(!ElementSpec::same_value(&hyperref_element(), &ref_element()));
    }
}
